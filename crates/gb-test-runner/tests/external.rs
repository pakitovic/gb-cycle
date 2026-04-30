use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use gb_core::{
    BootDirectBootState, BootRomAssets, ConsoleModel, CpuDiagnosticTrap, CpuExecutionState,
    Machine, MachineConfig, StartupMode, TraceSummaryBuffer,
};
use gb_test_runner::{
    RomRunner, RomSuite, acid_dmg_curated_suite, blargg_dmg_repo_gated_suite, boot_rom_image_path,
    boot_rom_kind_for_console_model, built_in_rom_suite_by_name, cpp_dmg_curated_suite,
    daid_dmg_curated_suite, discover_boot_rom_root, discover_test_rom_store_root,
    hacktix_dmg_curated_suite, mealybug_tearoom_dmg_curated_suite,
    mooneye_acceptance_dmg_curated_suite, update_curated_test_report, verify_boot_rom_file,
};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
const REAL_BOOT_HANDOFF_T_CYCLE_LIMIT: usize = 25_000_000;
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
    InvalidLogo,
    InvalidChecksum,
    FfFilledHeader,
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should be two levels above gb-test-runner")
        .to_path_buf()
}

fn run_curated_suite(
    suite: &RomSuite,
    suite_label: &str,
    update_report: bool,
) -> Option<gb_test_runner::RomSuiteReport> {
    let workspace_root = workspace_root();

    let Some(store_root) = discover_test_rom_store_root(&workspace_root) else {
        eprintln!(
            "skipping ignored test because neither GB_CYCLE_TEST_ROM_ROOT nor the default curated test ROM store is configured"
        );
        return None;
    };
    let Some(family) = suite.family.as_deref() else {
        panic!("{suite_label} should declare its curated family");
    };
    if !store_root.join(family).exists() {
        eprintln!(
            "skipping ignored test because curated family {family} is not materialized under {}",
            store_root.display()
        );
        return None;
    }

    let report = RomRunner::new()
        .run_suite(suite)
        .unwrap_or_else(|_| panic!("{suite_label} should execute"));
    if update_report {
        update_curated_test_report(&workspace_root, &report)
            .expect("curated report should update after a repo-managed suite run");
    }
    Some(report)
}

fn build_real_boot_validation_rom(profile: ValidationRomProfile) -> Vec<u8> {
    let fill_byte = match profile {
        ValidationRomProfile::FfFilledHeader => 0xFF,
        ValidationRomProfile::Valid
        | ValidationRomProfile::InvalidLogo
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
        | ValidationRomProfile::InvalidLogo
        | ValidationRomProfile::InvalidChecksum => {
            rom[0x0104..0x0134].copy_from_slice(&NINTENDO_LOGO);
            rom[0x0134..0x013C].copy_from_slice(b"BOOTREAL");
            rom[0x0143] = 0x00;
            rom[0x0146] = 0x00;
            rom[0x014D] = compute_header_checksum(&rom);

            if profile == ValidationRomProfile::InvalidLogo {
                rom[0x0104] ^= 0xFF;
            }
            if profile == ValidationRomProfile::InvalidChecksum {
                rom[0x014D] = rom[0x014D].wrapping_add(1);
            }
        }
        ValidationRomProfile::FfFilledHeader => {
            rom[0x0143] = 0x00;
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
    let Some(kind) = boot_rom_kind_for_console_model(console_model) else {
        panic!("expected a boot-ROM-backed console model, got {console_model:?}");
    };
    let image_path = boot_rom_image_path(&root, kind);
    verify_boot_rom_file(&image_path, kind).unwrap_or_else(|_| {
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

fn run_cgb_real_boot_handoff_smoke() {
    let Some(boot_rom_assets) = load_verified_boot_rom_assets(ConsoleModel::GameBoyColor) else {
        return;
    };

    let rom_bytes = build_real_boot_validation_rom(ValidationRomProfile::Valid);
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoyColor)
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
    assert_eq!(
        machine.read_bus(ENTRY_SENTINEL_ADDRESS),
        0x00,
        "the CGB real-boot smoke should stop at the firmware handoff and not execute cartridge code before the test owns the post-boot policy"
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

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn blargg_curated_suite_passes_from_repo_store() {
    let Some(report) = run_curated_suite(
        &blargg_dmg_repo_gated_suite(),
        "repo-gated blargg suite",
        true,
    ) else {
        return;
    };
    assert!(report.all_passed(), "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn blargg_cpu_instrs_chunk_passes_from_repo_store() {
    let suite = built_in_rom_suite_by_name("blargg-dmg-cpu-instrs")
        .expect("Blargg CPU instruction split suite should exist");
    let Some(report) = run_curated_suite(&suite, "curated blargg CPU instruction chunk", true)
    else {
        return;
    };
    assert_eq!(report.family.as_deref(), Some("blargg"), "{report:#?}");
    assert_eq!(report.cases.len(), 11, "{report:#?}");
    assert!(report.all_passed(), "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn blargg_dmg_sound_chunk_passes_from_repo_store() {
    let suite = built_in_rom_suite_by_name("blargg-dmg-sound")
        .expect("Blargg DMG sound split suite should exist");
    let Some(report) = run_curated_suite(&suite, "curated blargg DMG sound chunk", true) else {
        return;
    };
    assert_eq!(report.family.as_deref(), Some("blargg"), "{report:#?}");
    assert_eq!(report.cases.len(), 12, "{report:#?}");
    assert!(report.all_passed(), "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn blargg_timing_memory_oam_chunk_passes_from_repo_store() {
    let suite = built_in_rom_suite_by_name("blargg-dmg-timing-memory-oam")
        .expect("Blargg timing/memory/OAM split suite should exist");
    let Some(report) = run_curated_suite(&suite, "curated blargg timing/memory/OAM chunk", true)
    else {
        return;
    };
    assert_eq!(report.family.as_deref(), Some("blargg"), "{report:#?}");
    assert_eq!(report.cases.len(), 15, "{report:#?}");
    assert!(report.all_passed(), "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn acid_curated_suite_passes_from_repo_store() {
    let Some(report) = run_curated_suite(&acid_dmg_curated_suite(), "curated acid suite", true)
    else {
        return;
    };
    assert!(report.all_non_failing(), "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn mealybug_curated_suite_passes_from_repo_store() {
    let suite = mealybug_tearoom_dmg_curated_suite();
    let expected_case_count = suite.cases.len();
    let Some(report) = run_curated_suite(&suite, "curated mealybug suite", true) else {
        return;
    };
    assert_eq!(
        report.family.as_deref(),
        Some("mealybug-tearoom-tests"),
        "{report:#?}"
    );
    assert_eq!(report.cases.len(), expected_case_count, "{report:#?}");
    assert!(report.all_passed(), "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn mooneye_curated_suite_passes_from_repo_store() {
    let suite = mooneye_acceptance_dmg_curated_suite();
    let expected_case_count = suite.cases.len();
    let Some(report) = run_curated_suite(&suite, "curated mooneye suite", true) else {
        return;
    };
    assert_eq!(report.family.as_deref(), Some("mooneye"), "{report:#?}");
    assert_eq!(report.cases.len(), expected_case_count, "{report:#?}");
    assert!(report.all_passed(), "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn mooneye_acceptance_chunk_passes_from_repo_store() {
    let suite = built_in_rom_suite_by_name("mooneye-dmg-acceptance-manual")
        .expect("Mooneye acceptance/manual split suite should exist");
    let Some(report) = run_curated_suite(&suite, "curated mooneye acceptance chunk", true) else {
        return;
    };
    assert_eq!(report.family.as_deref(), Some("mooneye"), "{report:#?}");
    assert_eq!(report.cases.len(), 67, "{report:#?}");
    assert!(report.all_passed(), "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn mooneye_mbc1_mbc5_chunk_passes_from_repo_store() {
    let suite = built_in_rom_suite_by_name("mooneye-dmg-emulator-mbc1-mbc5")
        .expect("Mooneye MBC1/MBC5 split suite should exist");
    let Some(report) = run_curated_suite(&suite, "curated mooneye MBC1/MBC5 chunk", true) else {
        return;
    };
    assert_eq!(report.family.as_deref(), Some("mooneye"), "{report:#?}");
    assert_eq!(report.cases.len(), 21, "{report:#?}");
    assert!(report.all_passed(), "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn mooneye_mbc2_chunk_passes_from_repo_store() {
    let suite = built_in_rom_suite_by_name("mooneye-dmg-emulator-mbc2")
        .expect("Mooneye MBC2 split suite should exist");
    let Some(report) = run_curated_suite(&suite, "curated mooneye MBC2 chunk", true) else {
        return;
    };
    assert_eq!(report.family.as_deref(), Some("mooneye"), "{report:#?}");
    assert_eq!(report.cases.len(), 7, "{report:#?}");
    assert!(report.all_passed(), "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn daid_curated_suite_passes_from_repo_store() {
    let Some(report) = run_curated_suite(&daid_dmg_curated_suite(), "curated daid suite", true)
    else {
        return;
    };
    assert_eq!(report.family.as_deref(), Some("daid"), "{report:#?}");
    assert_eq!(report.cases.len(), 3, "{report:#?}");
    assert!(report.all_non_failing(), "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn cpp_curated_suite_passes_from_repo_store() {
    let Some(report) = run_curated_suite(&cpp_dmg_curated_suite(), "curated cpp suite", true)
    else {
        return;
    };
    assert_eq!(report.family.as_deref(), Some("cpp"), "{report:#?}");
    assert_eq!(report.cases.len(), 3, "{report:#?}");
    assert!(report.all_passed(), "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn hacktix_curated_suite_passes_from_repo_store() {
    let Some(report) =
        run_curated_suite(&hacktix_dmg_curated_suite(), "curated hacktix suite", true)
    else {
        return;
    };
    assert_eq!(report.family.as_deref(), Some("hacktix"), "{report:#?}");
    assert_eq!(report.cases.len(), 2, "{report:#?}");
    assert!(report.all_passed(), "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn blargg_01_special_copies_bank1_payload_to_wram_before_running() {
    let workspace_root = workspace_root();

    let Some(root) = discover_test_rom_store_root(&workspace_root) else {
        eprintln!(
            "skipping ignored test because neither GB_CYCLE_TEST_ROM_ROOT nor the default curated test ROM store is configured"
        );
        return;
    };

    let rom_path = root.join("blargg/cpu_instrs/01-special.gb");
    let rom = fs::read(&rom_path).expect("curated blargg ROM should be readable");

    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom.clone())
        .expect("curated blargg ROM should load");

    let mut saw_serial_output = false;
    for _ in 0..2_000_000_u64 {
        machine.step_t_cycle();
        if !machine.take_serial_output_bytes().is_empty() {
            saw_serial_output = true;
            break;
        }
    }

    assert!(
        saw_serial_output,
        "expected bank 1 payload to execute and emit serial output"
    );
}
