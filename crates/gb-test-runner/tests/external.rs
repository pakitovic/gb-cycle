use std::env;
use std::sync::{Mutex, MutexGuard};

use gb_core::{
    BootDirectBootState, BootRomAssets, ConsoleModel, CpuDiagnosticTrap, CpuExecutionState,
    CpuRegisters, Machine, MachineConfig, OperatingMode, StartupMode, TraceSummaryBuffer,
};
use gb_test_runner::{
    RomSuite, boot_rom_image_path, boot_rom_revision_for_console_model, discover_boot_rom_root,
    mealybug_tearoom_dmg_curated_suite, verify_boot_rom_file,
};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
const REAL_BOOT_HANDOFF_T_CYCLE_LIMIT: usize = 25_000_000;
const TEST_ROM_STARTUP_ENV_VAR: &str = "GB_CYCLE_TEST_ROM_STARTUP";
const VALIDATION_ENTRY_OPCODE: u8 = 0xC3;
const VALIDATION_PROGRAM_ADDRESS: u16 = 0x0150;
const VALIDATION_TRAP_OPCODE: u8 = 0xD3;
const VALIDATION_TRAP_ADDRESS: u16 = VALIDATION_PROGRAM_ADDRESS + 27;
const ENTRY_SENTINEL_ADDRESS: u16 = 0xC1F0;
const ENTRY_SENTINEL_VALUE: u8 = 0xA5;
const FINGERPRINT_BUFFER_ADDRESS: u16 = 0xC100;
const NINTENDO_LOGO: [u8; 48] = [
    0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B, 0x03, 0x73, 0x00, 0x83, 0x00, 0x0C, 0x00, 0x0D,
    0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E, 0xDC, 0xCC, 0x6E, 0xE6, 0xDD, 0xDD, 0xD9, 0x99,
    0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC, 0xDD, 0xDC, 0x99, 0x9F, 0xBB, 0xB9, 0x33, 0x3E,
];
const VALIDATION_PROGRAM: [u8; 28] = [
    0x3E,
    ENTRY_SENTINEL_VALUE, // LD A,$A5
    0xEA,
    0xF0,
    0xC1, // LD ($C1F0),A
    0x21,
    0x00,
    0xC1, // LD HL,$C100
    0x2A, // LD A,(HL+)
    0xFE,
    0x00, // CP $00
    0x28,
    0x0E, // JR Z,+14
    0xE0,
    0x01, // LDH ($01),A
    0x3E,
    0x81, // LD A,$81
    0xE0,
    0x02, // LDH ($02),A
    0xF0,
    0x02, // LDH A,($02)
    0xE6,
    0x80, // AND $80
    0x20,
    0xFA, // JR NZ,-6
    0x18,
    0xED,                   // JR -19
    VALIDATION_TRAP_OPCODE, // invalid opcode trap once the buffer terminator is reached
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidationRomProfile {
    Valid,
    ValidCgbNative,
    ValidCgbCompatible,
    InvalidLogo,
    InvalidCgbCheckedLogoPrefix,
    InvalidCgbUncheckedLogoSuffix,
    InvalidChecksum,
    FfFilledHeader,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfiguredTestRomStartup {
    Manifest,
    SkipBoot,
    CustomBoot,
    RealBoot,
}

static ENV_MUTEX: Mutex<()> = Mutex::new(());

fn lock_env() -> MutexGuard<'static, ()> {
    ENV_MUTEX.lock().expect("env mutex should not be poisoned")
}

fn set_env_var(key: &str, value: impl AsRef<std::ffi::OsStr>) {
    unsafe {
        env::set_var(key, value);
    }
}

fn remove_env_var(key: &str) {
    unsafe {
        env::remove_var(key);
    }
}

fn configured_test_rom_startup() -> Result<ConfiguredTestRomStartup, String> {
    match env::var(TEST_ROM_STARTUP_ENV_VAR) {
        Ok(value) => match value.as_str() {
            "skip-boot" => Ok(ConfiguredTestRomStartup::SkipBoot),
            "custom-boot" => Ok(ConfiguredTestRomStartup::CustomBoot),
            "real-boot" => Ok(ConfiguredTestRomStartup::RealBoot),
            other => Err(format!(
                "unsupported {TEST_ROM_STARTUP_ENV_VAR} value {other:?}; expected \"skip-boot\", \"custom-boot\", or \"real-boot\""
            )),
        },
        Err(env::VarError::NotPresent) => Ok(ConfiguredTestRomStartup::Manifest),
        Err(env::VarError::NotUnicode(_)) => Err(format!(
            "{TEST_ROM_STARTUP_ENV_VAR} must be valid UTF-8; expected \"skip-boot\", \"custom-boot\", or \"real-boot\""
        )),
    }
}

fn suite_for_configured_startup(suite: &RomSuite) -> Result<RomSuite, String> {
    let mut suite = suite.clone();
    match configured_test_rom_startup()? {
        ConfiguredTestRomStartup::Manifest => {}
        ConfiguredTestRomStartup::SkipBoot => {
            for case in &mut suite.cases {
                case.startup_mode = StartupMode::SkipBoot;
            }
        }
        ConfiguredTestRomStartup::CustomBoot => {
            for case in &mut suite.cases {
                case.startup_mode = StartupMode::CustomBoot;
            }
        }
        ConfiguredTestRomStartup::RealBoot => {
            for case in &mut suite.cases {
                case.startup_mode = StartupMode::RealBoot;
                case.startup_timer_state = None;
                case.startup_memory_writes.clear();
            }
        }
    }
    Ok(suite)
}

#[test]
fn test_rom_startup_defaults_to_manifest_startup_modes() {
    let _guard = lock_env();
    let previous_startup = env::var_os(TEST_ROM_STARTUP_ENV_VAR);
    remove_env_var(TEST_ROM_STARTUP_ENV_VAR);

    let suite = mealybug_tearoom_dmg_curated_suite();
    assert!(
        suite
            .cases
            .iter()
            .any(|case| case.startup_mode == StartupMode::CustomBoot),
        "fixture should cover a custom-boot manifest case"
    );

    let configured =
        suite_for_configured_startup(&suite).expect("default startup configuration should parse");

    assert_eq!(configured, suite);

    match previous_startup {
        Some(value) => set_env_var(TEST_ROM_STARTUP_ENV_VAR, value),
        None => remove_env_var(TEST_ROM_STARTUP_ENV_VAR),
    }
}

#[test]
fn test_rom_startup_real_boot_overrides_custom_boot_cases() {
    let _guard = lock_env();
    let previous_startup = env::var_os(TEST_ROM_STARTUP_ENV_VAR);
    set_env_var(TEST_ROM_STARTUP_ENV_VAR, "real-boot");

    let suite = mealybug_tearoom_dmg_curated_suite();
    assert!(
        suite
            .cases
            .iter()
            .any(|case| case.startup_mode == StartupMode::CustomBoot),
        "fixture should cover a custom-boot manifest case"
    );

    let configured =
        suite_for_configured_startup(&suite).expect("real-boot startup configuration should parse");

    assert!(
        configured
            .cases
            .iter()
            .all(|case| case.startup_mode == StartupMode::RealBoot)
    );
    assert!(
        configured
            .cases
            .iter()
            .all(|case| case.startup_memory_writes.is_empty())
    );

    match previous_startup {
        Some(value) => set_env_var(TEST_ROM_STARTUP_ENV_VAR, value),
        None => remove_env_var(TEST_ROM_STARTUP_ENV_VAR),
    }
}

#[test]
fn test_rom_startup_rejects_unknown_values() {
    let _guard = lock_env();
    let previous_startup = env::var_os(TEST_ROM_STARTUP_ENV_VAR);
    set_env_var(TEST_ROM_STARTUP_ENV_VAR, "warm-boot");

    let error = configured_test_rom_startup().expect_err("unknown startup mode should fail");

    assert!(error.contains(TEST_ROM_STARTUP_ENV_VAR));
    assert!(error.contains("skip-boot"));
    assert!(error.contains("custom-boot"));
    assert!(error.contains("real-boot"));

    match previous_startup {
        Some(value) => set_env_var(TEST_ROM_STARTUP_ENV_VAR, value),
        None => remove_env_var(TEST_ROM_STARTUP_ENV_VAR),
    }
}

fn build_real_boot_validation_rom(profile: ValidationRomProfile) -> Vec<u8> {
    let fill_byte = match profile {
        ValidationRomProfile::FfFilledHeader => 0xFF,
        ValidationRomProfile::Valid
        | ValidationRomProfile::ValidCgbNative
        | ValidationRomProfile::ValidCgbCompatible
        | ValidationRomProfile::InvalidLogo
        | ValidationRomProfile::InvalidCgbCheckedLogoPrefix
        | ValidationRomProfile::InvalidCgbUncheckedLogoSuffix
        | ValidationRomProfile::InvalidChecksum => 0x00,
    };
    let mut rom = vec![fill_byte; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    rom[0x0000] = 0x12;
    rom[0x0100..0x0103].copy_from_slice(&[
        VALIDATION_ENTRY_OPCODE,
        VALIDATION_PROGRAM_ADDRESS as u8,
        (VALIDATION_PROGRAM_ADDRESS >> 8) as u8,
    ]);
    rom[VALIDATION_PROGRAM_ADDRESS as usize
        ..VALIDATION_PROGRAM_ADDRESS as usize + VALIDATION_PROGRAM.len()]
        .copy_from_slice(&VALIDATION_PROGRAM);

    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;

    match profile {
        ValidationRomProfile::Valid
        | ValidationRomProfile::ValidCgbNative
        | ValidationRomProfile::ValidCgbCompatible
        | ValidationRomProfile::InvalidLogo
        | ValidationRomProfile::InvalidCgbCheckedLogoPrefix
        | ValidationRomProfile::InvalidCgbUncheckedLogoSuffix
        | ValidationRomProfile::InvalidChecksum => {
            rom[0x0104..0x0134].copy_from_slice(&NINTENDO_LOGO);
            rom[0x0134..0x013C].copy_from_slice(b"BOOTREAL");
            rom[0x0143] = match profile {
                ValidationRomProfile::ValidCgbNative => 0x80,
                ValidationRomProfile::ValidCgbCompatible => 0x00,
                _ => 0x00,
            };
            rom[0x0146] = 0x00;
            rom[0x014D] = compute_header_checksum(&rom);

            if matches!(
                profile,
                ValidationRomProfile::InvalidLogo
                    | ValidationRomProfile::InvalidCgbCheckedLogoPrefix
            ) {
                rom[0x0104] ^= 0xFF;
            }
            if profile == ValidationRomProfile::InvalidCgbUncheckedLogoSuffix {
                rom[0x0120] ^= 0xFF;
            }
            if profile == ValidationRomProfile::InvalidChecksum {
                rom[0x014D] = rom[0x014D].wrapping_add(1);
            }
        }
        ValidationRomProfile::FfFilledHeader => {
            rom[0x0146] = 0x00;
        }
    }

    rom
}

fn compute_header_checksum(rom: &[u8]) -> u8 {
    let mut checksum = 0_u8;
    for byte in &rom[0x0134..=0x014C] {
        checksum = checksum.wrapping_sub(*byte).wrapping_sub(1);
    }
    checksum
}

fn load_verified_boot_rom_assets(console_model: ConsoleModel) -> Option<BootRomAssets> {
    let Some(root) = discover_boot_rom_root() else {
        eprintln!("skipping ignored test because GB_CYCLE_BOOT_ROM_ROOT is not configured");
        return None;
    };
    let revision = boot_rom_revision_for_console_model(console_model);
    let image_path = boot_rom_image_path(&root, revision);
    verify_boot_rom_file(&image_path, revision).unwrap_or_else(|_| {
        panic!(
            "verified boot ROM asset should be readable: {}",
            image_path.display()
        )
    });
    Some(
        BootRomAssets::from_directory(&root)
            .unwrap_or_else(|_| panic!("boot ROM assets should load from {}", root.display())),
    )
}

fn expected_direct_boot_entry_state(
    console_model: ConsoleModel,
    rom_bytes: &[u8],
) -> BootDirectBootState {
    let mut machine = Machine::new_summary(
        MachineConfig::new(console_model).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom_bytes.to_vec())
        .expect("validation cartridge should load under the synthetic SkipBoot path");
    machine
        .boot()
        .direct_boot_state(Some(machine.cartridge()))
        .expect("SkipBoot controller should expose the centralized direct-boot snapshot")
}

fn render_entry_fingerprint(expected: &BootDirectBootState) -> String {
    format!(
        "AF={:02X}{:02X} BC={:02X}{:02X} DE={:02X}{:02X} HL={:02X}{:02X} SP={:04X} P1={:02X} DIV={:02X} TIMA={:02X} TMA={:02X} TAC={:02X} IF={:02X} LCDC={:02X} STAT={:02X} BGP={:02X} IE={:02X}\n",
        expected.cpu.a,
        expected.cpu.f,
        expected.cpu.b,
        expected.cpu.c,
        expected.cpu.d,
        expected.cpu.e,
        expected.cpu.h,
        expected.cpu.l,
        expected.cpu.sp,
        expected.io.p1,
        expected.io.div,
        expected.io.tima,
        expected.io.tma,
        expected.io.tac,
        expected.io.interrupt_flag,
        expected.io.lcdc,
        expected.io.stat,
        expected.io.bgp,
        expected.io.interrupt_enable,
    )
}

fn assert_real_boot_entry_matches_direct_boot_snapshot(
    machine: &mut Machine<TraceSummaryBuffer>,
    expected: &BootDirectBootState,
) {
    let actual = machine.cpu().registers();
    let mut mismatches = Vec::new();

    for (label, actual, expected) in [
        ("A", actual.a, expected.cpu.a),
        ("F", actual.f, expected.cpu.f),
        ("B", actual.b, expected.cpu.b),
        ("C", actual.c, expected.cpu.c),
        ("D", actual.d, expected.cpu.d),
        ("E", actual.e, expected.cpu.e),
        ("H", actual.h, expected.cpu.h),
        ("L", actual.l, expected.cpu.l),
    ] {
        if actual != expected {
            mismatches.push(format!(
                "{label}: actual=0x{actual:02X} expected=0x{expected:02X}"
            ));
        }
    }

    for (label, actual, expected) in [
        ("SP", actual.sp, expected.cpu.sp),
        ("PC", actual.pc, expected.cpu.pc),
    ] {
        if actual != expected {
            mismatches.push(format!(
                "{label}: actual=0x{actual:04X} expected=0x{expected:04X}"
            ));
        }
    }

    for (label, address, expected) in [
        ("P1", 0xFF00, expected.io.p1),
        ("DIV", 0xFF04, expected.io.div),
        ("TIMA", 0xFF05, expected.io.tima),
        ("TMA", 0xFF06, expected.io.tma),
        ("TAC", 0xFF07, expected.io.tac),
        ("IF", 0xFF0F, expected.io.interrupt_flag),
        ("LCDC", 0xFF40, expected.io.lcdc),
        ("STAT", 0xFF41, expected.io.stat),
        ("BGP", 0xFF47, expected.io.bgp),
        ("IE", 0xFFFF, expected.io.interrupt_enable),
    ] {
        let actual = machine.read_bus(address);
        if actual != expected {
            mismatches.push(format!(
                "{label}: actual=0x{actual:02X} expected=0x{expected:02X}"
            ));
        }
    }

    assert!(
        mismatches.is_empty(),
        "real boot entry state diverged from the centralized direct-boot snapshot:\n{}\nactual_timer={:#?}\nactual_ppu={:#?}\nactual_apu={:#?}\nactual_serial={:#?}\n{}",
        mismatches.join("\n"),
        machine.timer().snapshot(),
        machine.ppu().snapshot(),
        machine.apu().snapshot(),
        machine.serial().snapshot(),
        machine.snapshot().render_text()
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CgbBootEntrySnapshot {
    cpu: CpuRegisters,
    operating_mode: OperatingMode,
    key0: u8,
    key1: u8,
    vbk: u8,
    svbk: u8,
    hdma: [u8; 5],
    ff72_ff75: [u8; 4],
    cgb_palette_ports: [u8; 4],
    opri: u8,
    wave_ram: [u8; 16],
    vram_prefix: [u8; 32],
    wram_prefix: [u8; 32],
    hram_prefix: [u8; 16],
}

fn cgb_boot_entry_snapshot(machine: &mut Machine<TraceSummaryBuffer>) -> CgbBootEntrySnapshot {
    let mut hdma = [0; 5];
    for (index, address) in (0xFF51..=0xFF55).enumerate() {
        hdma[index] = machine.read_bus(address);
    }

    let mut ff72_ff75 = [0; 4];
    for (index, address) in (0xFF72..=0xFF75).enumerate() {
        ff72_ff75[index] = machine.read_bus(address);
    }

    let mut cgb_palette_ports = [0; 4];
    for (index, address) in (0xFF68..=0xFF6B).enumerate() {
        cgb_palette_ports[index] = machine.read_bus(address);
    }

    CgbBootEntrySnapshot {
        cpu: machine.cpu().registers(),
        operating_mode: machine.config().operating_mode,
        key0: machine.read_bus(0xFF4C),
        key1: machine.read_bus(0xFF4D),
        vbk: machine.read_bus(0xFF4F),
        svbk: machine.read_bus(0xFF70),
        hdma,
        ff72_ff75,
        cgb_palette_ports,
        opri: machine.read_bus(0xFF6C),
        wave_ram: machine.apu().snapshot().wave_ram,
        vram_prefix: machine.debug_vram_bytes()[0..32]
            .try_into()
            .expect("VRAM prefix length should be fixed"),
        wram_prefix: machine.debug_wram_bytes()[0..32]
            .try_into()
            .expect("WRAM prefix length should be fixed"),
        hram_prefix: machine.debug_hram_bytes()[0..16]
            .try_into()
            .expect("HRAM prefix length should be fixed"),
    }
}

fn assert_cgb_real_boot_entry_matches_skip_boot_snapshot(
    machine: &mut Machine<TraceSummaryBuffer>,
    rom_bytes: &[u8],
) {
    let actual = cgb_boot_entry_snapshot(machine);
    let mut skip_boot = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    skip_boot
        .load_cartridge(rom_bytes.to_vec())
        .expect("validation CGB cartridge should load under the synthetic SkipBoot path");
    let expected = cgb_boot_entry_snapshot(&mut skip_boot);

    assert_eq!(
        actual,
        expected,
        "CGB RealBoot handoff state diverged from the centralized SkipBoot startup snapshot\nactual={actual:#?}\nexpected={expected:#?}\nreal_boot={}",
        machine.snapshot().render_text()
    );
}

fn write_fingerprint_buffer(machine: &mut Machine<TraceSummaryBuffer>, fingerprint: &str) {
    for (index, byte) in fingerprint.as_bytes().iter().copied().enumerate() {
        machine.write_bus(FINGERPRINT_BUFFER_ADDRESS + index as u16, byte);
    }
    machine.write_bus(FINGERPRINT_BUFFER_ADDRESS + fingerprint.len() as u16, 0x00);
}

fn step_until_real_boot_handoff(machine: &mut Machine<TraceSummaryBuffer>) {
    for _ in 0..REAL_BOOT_HANDOFF_T_CYCLE_LIMIT {
        if !machine.boot().is_boot_rom_mapped() {
            return;
        }

        machine.step_t_cycle();

        if let CpuExecutionState::DiagnosticTrap { trap } = machine.cpu().execution_state() {
            panic!(
                "real boot trapped before FF50 handoff: {trap:?}\n{}",
                machine.snapshot().render_text()
            );
        }
    }

    let ly = machine.ppu().ly();
    let line_dot = machine.ppu().line_dot();
    let lcdc = machine.read_bus(0xFF40);
    let stat = machine.read_bus(0xFF41);
    let scy = machine.read_bus(0xFF42);
    let ly_readback = machine.read_bus(0xFF44);

    panic!(
        "real boot did not reach the FF50 handoff within {REAL_BOOT_HANDOFF_T_CYCLE_LIMIT} T-cycles\nppu.ly={ly} ppu.line_dot={line_dot} lcdc=0x{lcdc:02X} stat=0x{stat:02X} scy=0x{scy:02X} ly_readback=0x{ly_readback:02X}\n{}",
        machine.snapshot().render_text()
    );
}

fn step_until_serial_fingerprint_and_trap(
    machine: &mut Machine<TraceSummaryBuffer>,
    expected_fingerprint: &str,
) {
    let mut serial_bytes = Vec::new();
    let step_limit = expected_fingerprint.len() * 5_000 + 100_000;

    for _ in 0..step_limit {
        serial_bytes.extend(machine.take_serial_output_bytes());

        if serial_bytes.len() == expected_fingerprint.len()
            && matches!(
                machine.cpu().execution_state(),
                CpuExecutionState::DiagnosticTrap {
                    trap: CpuDiagnosticTrap::InvalidOpcode {
                        opcode: VALIDATION_TRAP_OPCODE,
                        address: VALIDATION_TRAP_ADDRESS,
                    },
                }
            )
        {
            let rendered = String::from_utf8(serial_bytes).expect("fingerprint should be UTF-8");
            assert_eq!(rendered, expected_fingerprint);
            return;
        }

        machine.step_t_cycle();
    }

    serial_bytes.extend(machine.take_serial_output_bytes());

    panic!(
        "validation program did not finish serial fingerprint emission within {step_limit} T-cycles\nserial_so_far={:?}\n{}",
        String::from_utf8_lossy(&serial_bytes),
        machine.snapshot().render_text()
    );
}

fn assert_real_boot_stays_mapped_without_false_handoff(
    machine: &mut Machine<TraceSummaryBuffer>,
    case_label: &str,
) {
    for _ in 0..REAL_BOOT_HANDOFF_T_CYCLE_LIMIT {
        if !machine.boot().is_boot_rom_mapped() {
            panic!(
                "{case_label} unexpectedly unmapped the boot ROM before the observation window ended\n{}",
                machine.snapshot().render_text()
            );
        }

        if machine.read_bus(ENTRY_SENTINEL_ADDRESS) == ENTRY_SENTINEL_VALUE {
            panic!(
                "{case_label} executed cartridge code at 0x0100 without a real FF50 handoff\n{}",
                machine.snapshot().render_text()
            );
        }

        machine.step_t_cycle();

        if let CpuExecutionState::DiagnosticTrap { trap } = machine.cpu().execution_state() {
            panic!(
                "{case_label} trapped before the non-handoff observation window ended: {trap:?}\n{}",
                machine.snapshot().render_text()
            );
        }
    }

    assert!(machine.boot().is_boot_rom_mapped());
    assert_eq!(machine.read_bus(ENTRY_SENTINEL_ADDRESS), 0x00);
}

fn run_real_boot_validation(console_model: ConsoleModel) {
    let Some(boot_rom_assets) = load_verified_boot_rom_assets(console_model) else {
        return;
    };
    let rom_bytes = build_real_boot_validation_rom(ValidationRomProfile::Valid);
    let expected_entry_state = expected_direct_boot_entry_state(console_model, &rom_bytes);
    let expected_fingerprint = render_entry_fingerprint(&expected_entry_state);

    let mut machine = Machine::new_summary(
        MachineConfig::new(console_model)
            .with_startup_mode(StartupMode::RealBoot)
            .with_boot_rom_assets(boot_rom_assets),
    );
    machine
        .load_cartridge(rom_bytes)
        .expect("validation cartridge should load as NoMBC");
    machine.write_bus(ENTRY_SENTINEL_ADDRESS, 0x00);

    assert!(machine.boot().is_boot_rom_mapped());
    assert_eq!(
        machine.read_bus(0x0000),
        machine.boot().read_boot_rom(0x0000)
    );

    step_until_real_boot_handoff(&mut machine);

    assert!(!machine.boot().is_boot_rom_mapped());
    assert_eq!(machine.cpu().registers().pc, 0x0100);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(machine.cpu().current_opcode(), None);
    assert_eq!(machine.read_bus(0x0000), 0x12);
    assert_eq!(machine.read_bus(0x0100), VALIDATION_ENTRY_OPCODE);
    assert_real_boot_entry_matches_direct_boot_snapshot(&mut machine, &expected_entry_state);

    write_fingerprint_buffer(&mut machine, &expected_fingerprint);
    step_until_serial_fingerprint_and_trap(&mut machine, &expected_fingerprint);
    assert_eq!(
        machine.read_bus(ENTRY_SENTINEL_ADDRESS),
        ENTRY_SENTINEL_VALUE
    );
}

fn run_cgb_real_boot_handoff_validation(
    profile: ValidationRomProfile,
    expected_mode: OperatingMode,
    compare_skip_boot_snapshot: bool,
) {
    let Some(boot_rom_assets) = load_verified_boot_rom_assets(ConsoleModel::GameBoyColor) else {
        return;
    };

    let rom_bytes = build_real_boot_validation_rom(profile);
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoyColor)
            .with_startup_mode(StartupMode::RealBoot)
            .with_boot_rom_assets(boot_rom_assets),
    );
    machine
        .load_cartridge(rom_bytes.clone())
        .expect("validation cartridge should load as NoMBC");
    machine.write_bus(ENTRY_SENTINEL_ADDRESS, 0x00);

    assert!(machine.boot().is_boot_rom_mapped());
    assert_eq!(
        machine.read_bus(0x0000),
        machine.boot().read_boot_rom(0x0000)
    );

    step_until_real_boot_handoff(&mut machine);

    assert!(!machine.boot().is_boot_rom_mapped());
    assert_eq!(machine.cpu().registers().pc, 0x0100);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(machine.cpu().current_opcode(), None);
    assert_eq!(machine.read_bus(0x0000), 0x12);
    assert_eq!(machine.read_bus(0x0100), VALIDATION_ENTRY_OPCODE);
    assert_eq!(machine.config().operating_mode, expected_mode);
    assert_eq!(machine.bus().operating_mode(), expected_mode);
    assert_eq!(machine.speed().operating_mode(), expected_mode);
    if compare_skip_boot_snapshot {
        assert_cgb_real_boot_entry_matches_skip_boot_snapshot(&mut machine, &rom_bytes);
    }
    assert_eq!(
        machine.read_bus(ENTRY_SENTINEL_ADDRESS),
        0x00,
        "the CGB real-boot smoke should stop at the firmware handoff and not execute cartridge code before the test owns the post-boot policy"
    );
}

fn run_cgb_real_boot_handoff_smoke() {
    run_cgb_real_boot_handoff_validation(
        ValidationRomProfile::ValidCgbCompatible,
        OperatingMode::GbCompatible,
        false,
    );
}

fn run_real_boot_non_handoff_validation(profile: ValidationRomProfile, case_label: &str) {
    let Some(boot_rom_assets) = load_verified_boot_rom_assets(ConsoleModel::GameBoy) else {
        return;
    };

    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy)
            .with_startup_mode(StartupMode::RealBoot)
            .with_boot_rom_assets(boot_rom_assets),
    );
    machine
        .load_cartridge(build_real_boot_validation_rom(profile))
        .expect("validation cartridge should load as NoMBC");
    machine.write_bus(ENTRY_SENTINEL_ADDRESS, 0x00);

    assert!(machine.boot().is_boot_rom_mapped());
    assert_eq!(
        machine.read_bus(0x0000),
        machine.boot().read_boot_rom(0x0000)
    );

    assert_real_boot_stays_mapped_without_false_handoff(&mut machine, case_label);
}

fn run_cgb_real_boot_non_handoff_validation(profile: ValidationRomProfile, case_label: &str) {
    let Some(boot_rom_assets) = load_verified_boot_rom_assets(ConsoleModel::GameBoyColor) else {
        return;
    };

    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoyColor)
            .with_startup_mode(StartupMode::RealBoot)
            .with_boot_rom_assets(boot_rom_assets),
    );
    machine
        .load_cartridge(build_real_boot_validation_rom(profile))
        .expect("validation cartridge should load as NoMBC");
    machine.write_bus(ENTRY_SENTINEL_ADDRESS, 0x00);

    assert!(machine.boot().is_boot_rom_mapped());
    assert_eq!(
        machine.read_bus(0x0000),
        machine.boot().read_boot_rom(0x0000)
    );

    assert_real_boot_stays_mapped_without_false_handoff(&mut machine, case_label);
}

#[test]
#[ignore = "requires verified local dmg0 boot ROM asset via GB_CYCLE_BOOT_ROM_ROOT"]
fn real_boot_with_verified_dmg0_boot_rom_reaches_cartridge_entry_via_ff50_handoff() {
    run_real_boot_validation(ConsoleModel::GameBoy);
}

#[test]
#[ignore = "requires verified local dmg boot ROM asset via GB_CYCLE_BOOT_ROM_ROOT"]
fn real_boot_with_verified_dmg_boot_rom_reaches_cartridge_entry_via_ff50_handoff() {
    run_real_boot_validation(ConsoleModel::GameBoy);
}

#[test]
#[ignore = "requires verified local mgb boot ROM asset via GB_CYCLE_BOOT_ROM_ROOT"]
fn real_boot_with_verified_mgb_boot_rom_reaches_cartridge_entry_via_ff50_handoff() {
    run_real_boot_validation(ConsoleModel::GameBoyPocket);
}

#[test]
#[ignore = "requires verified local cgb boot ROM asset via GB_CYCLE_BOOT_ROM_ROOT"]
fn real_boot_with_verified_cgb_boot_rom_reaches_cartridge_entry_via_ff50_handoff() {
    run_cgb_real_boot_handoff_smoke();
}

#[test]
#[ignore = "requires verified local cgb boot ROM asset via GB_CYCLE_BOOT_ROM_ROOT"]
fn real_boot_with_verified_cgb_boot_rom_native_header_reaches_cartridge_entry_via_ff50_handoff() {
    run_cgb_real_boot_handoff_validation(
        ValidationRomProfile::ValidCgbNative,
        OperatingMode::Cgb,
        true,
    );
}

#[test]
#[ignore = "requires verified local cgb boot ROM asset via GB_CYCLE_BOOT_ROM_ROOT"]
fn real_boot_with_verified_cgb_boot_rom_compatible_header_reaches_cartridge_entry_via_ff50_handoff()
{
    run_cgb_real_boot_handoff_validation(
        ValidationRomProfile::ValidCgbCompatible,
        OperatingMode::GbCompatible,
        true,
    );
}

#[test]
#[ignore = "requires verified local cgb boot ROM asset via GB_CYCLE_BOOT_ROM_ROOT"]
fn real_boot_with_verified_cgb_boot_rom_rejects_invalid_checked_logo_prefix_without_ff50_handoff() {
    run_cgb_real_boot_non_handoff_validation(
        ValidationRomProfile::InvalidCgbCheckedLogoPrefix,
        "CGB invalid checked logo prefix",
    );
}

#[test]
#[ignore = "requires verified local cgb boot ROM asset via GB_CYCLE_BOOT_ROM_ROOT"]
fn real_boot_with_verified_cgb_boot_rom_accepts_invalid_unchecked_logo_suffix_via_ff50_handoff() {
    run_cgb_real_boot_handoff_validation(
        ValidationRomProfile::InvalidCgbUncheckedLogoSuffix,
        OperatingMode::GbCompatible,
        false,
    );
}

#[test]
#[ignore = "requires verified local cgb boot ROM asset via GB_CYCLE_BOOT_ROM_ROOT"]
fn real_boot_with_verified_cgb_boot_rom_rejects_invalid_checksum_without_ff50_handoff() {
    run_cgb_real_boot_non_handoff_validation(
        ValidationRomProfile::InvalidChecksum,
        "CGB invalid checksum",
    );
}

#[test]
#[ignore = "requires verified local cgb boot ROM asset via GB_CYCLE_BOOT_ROM_ROOT"]
fn real_boot_with_verified_cgb_boot_rom_rejects_ff_filled_header_without_ff50_handoff() {
    run_cgb_real_boot_non_handoff_validation(
        ValidationRomProfile::FfFilledHeader,
        "CGB ff-filled header",
    );
}

#[test]
#[ignore = "requires verified local dmg boot ROM asset via GB_CYCLE_BOOT_ROM_ROOT"]
fn real_boot_with_verified_dmg_boot_rom_rejects_an_invalid_logo_without_ff50_handoff() {
    run_real_boot_non_handoff_validation(ValidationRomProfile::InvalidLogo, "invalid logo");
}

#[test]
#[ignore = "requires verified local dmg boot ROM asset via GB_CYCLE_BOOT_ROM_ROOT"]
fn real_boot_with_verified_dmg_boot_rom_rejects_an_invalid_checksum_without_ff50_handoff() {
    run_real_boot_non_handoff_validation(ValidationRomProfile::InvalidChecksum, "invalid checksum");
}

#[test]
#[ignore = "requires verified local dmg boot ROM asset via GB_CYCLE_BOOT_ROM_ROOT"]
fn real_boot_with_verified_dmg_boot_rom_rejects_an_ff_filled_header_without_ff50_handoff() {
    run_real_boot_non_handoff_validation(ValidationRomProfile::FfFilledHeader, "ff-filled header");
}
