use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use gb_core::ExecutionMode;
use gb_test_runner::{
    BootRomVerificationMode, CaptureKind, MemoryTextOutputSpec, PassCondition, RomCaseFailure,
    RomCaseOutcome, RomExecutionError, RomRunner, RomTestCase, StartupMemoryWrite,
    TEST_ROM_ROOT_ENV_VAR, Timeout, phase_2_cpu_timing_suite, phase_2_interrupt_timing_suite,
    phase_6_cartridge_oracle_suite,
};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;

fn build_test_rom(program: &[u8]) -> Vec<u8> {
    build_test_rom_with_header(program, 0x00, 0x00, 0x00)
}

fn build_test_rom_with_header(
    program: &[u8],
    cartridge_type: u8,
    rom_size: u8,
    ram_size: u8,
) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    for (offset, byte) in program.iter().copied().enumerate() {
        rom[0x0100 + offset] = byte;
    }
    rom[0x0147] = cartridge_type;
    rom[0x0148] = rom_size;
    rom[0x0149] = ram_size;
    rom
}

fn build_single_byte_serial_rom(byte: u8) -> Vec<u8> {
    build_test_rom(&[
        0x3E, byte, // LD A,d8
        0xE0, 0x01, // LDH (SB),A
        0x3E, 0x81, // LD A,$81
        0xE0, 0x02, // LDH (SC),A
        0xC3, 0x08, 0x01, // JP $0108
    ])
}

fn build_serial_from_address_rom(address: u16) -> Vec<u8> {
    build_test_rom(&[
        0xFA,
        address as u8,
        (address >> 8) as u8, // LD A,(a16)
        0xE0,
        0x01, // LDH (SB),A
        0x3E,
        0x81, // LD A,$81
        0xE0,
        0x02, // LDH (SC),A
        0xC3,
        0x08,
        0x01, // JP $0108
    ])
}

fn build_lcd_off_then_serial_from_address_rom(address: u16) -> Vec<u8> {
    build_test_rom(&[
        0xAF, // XOR A
        0xE0,
        0x40, // LDH (LCDC),A
        0xFA,
        address as u8,
        (address >> 8) as u8, // LD A,(a16)
        0xE0,
        0x01, // LDH (SB),A
        0x3E,
        0x81, // LD A,$81
        0xE0,
        0x02, // LDH (SC),A
        0xC3,
        0x0B,
        0x01, // JP $010B
    ])
}

fn build_unsupported_opcode_rom(opcode: u8) -> Vec<u8> {
    build_test_rom(&[
        opcode, // unsupported opcode at entry
        0xC3, 0x00, 0x01, // JP $0100
    ])
}

fn build_memory_text_output_rom() -> Vec<u8> {
    build_test_rom_with_header(
        &[
            0x21, 0x00, 0xA0, // LD HL,$A000
            0x3E, 0x00, // LD A,$00
            0x22, // LD (HL+),A
            0x3E, 0xDE, // LD A,$DE
            0x22, // LD (HL+),A
            0x3E, 0xB0, // LD A,$B0
            0x22, // LD (HL+),A
            0x3E, 0x61, // LD A,$61
            0x22, // LD (HL+),A
            0x3E, b'P', // LD A,'P'
            0x22, // LD (HL+),A
            0x3E, b'a', // LD A,'a'
            0x22, // LD (HL+),A
            0x3E, b's', // LD A,'s'
            0x22, // LD (HL+),A
            0x3E, b's', // LD A,'s'
            0x22, // LD (HL+),A
            0x3E, b'e', // LD A,'e'
            0x22, // LD (HL+),A
            0x3E, b'd', // LD A,'d'
            0x22, // LD (HL+),A
            0x3E, 0x00, // LD A,$00
            0x22, // LD (HL+),A
            0xC3, 0x22, 0x01, // JP $0122
        ],
        0x08,
        0x00,
        0x02,
    )
}

fn build_blargg_console_text_rom() -> Vec<u8> {
    build_test_rom(&[
        0x3E, 0x11, // LD A,$11
        0xE0, 0x40, // LDH (LCDC),A
        0xAF, // XOR A
        0xE0, 0x42, // LDH (SCY),A
        0x21, 0x00, 0x98, // LD HL,$9800
        0x3E, b'P', // LD A,'P'
        0x22, // LD (HL+),A
        0x3E, b'a', // LD A,'a'
        0x22, // LD (HL+),A
        0x3E, b's', // LD A,'s'
        0x22, // LD (HL+),A
        0x3E, b's', // LD A,'s'
        0x22, // LD (HL+),A
        0x3E, b'e', // LD A,'e'
        0x22, // LD (HL+),A
        0x3E, b'd', // LD A,'d'
        0x22, // LD (HL+),A
        0xC3, 0x18, 0x01, // JP $0118
    ])
}

fn build_real_boot_jump_stub() -> Vec<u8> {
    let mut boot_rom = vec![0x00; 0x0100];
    boot_rom[0] = 0xC3;
    boot_rom[1] = 0xFC;
    boot_rom[2] = 0x00;
    boot_rom[0xFC] = 0x3E;
    boot_rom[0xFD] = 0x01;
    boot_rom[0xFE] = 0xE0;
    boot_rom[0xFF] = 0x50;
    boot_rom
}

fn build_delayed_real_boot_handoff_stub() -> Vec<u8> {
    let mut boot_rom = build_real_boot_jump_stub();
    boot_rom[0] = 0x06; // LD B,$02
    boot_rom[1] = 0x02;
    boot_rom[2] = 0x0E; // LD C,$FF
    boot_rom[3] = 0xFF;
    boot_rom[4] = 0x0D; // DEC C
    boot_rom[5] = 0x20; // JR NZ,$0004
    boot_rom[6] = 0xFD;
    boot_rom[7] = 0x05; // DEC B
    boot_rom[8] = 0x20; // JR NZ,$0002
    boot_rom[9] = 0xF8;
    boot_rom[10] = 0xC3; // JP $00FC
    boot_rom[11] = 0xFC;
    boot_rom[12] = 0x00;
    boot_rom
}

fn unique_temp_dir(label: &str) -> PathBuf {
    env::temp_dir().join(format!(
        "gb-cycle-test-runner-{}-{}-{}",
        label,
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}

#[test]
fn runner_executes_phase_2_cpu_timing_suite_against_reserved_fixtures() {
    let report = RomRunner::new()
        .run_suite(&phase_2_cpu_timing_suite())
        .expect("phase 2 cpu suite should execute");

    assert!(report.all_passed(), "{report:#?}");
}

#[test]
fn runner_executes_phase_2_interrupt_suite_with_typed_external_stimuli() {
    let report = RomRunner::new()
        .run_suite(&phase_2_interrupt_timing_suite())
        .expect("phase 2 interrupt suite should execute");

    assert!(report.all_passed(), "{report:#?}");
}

#[test]
fn runner_executes_phase_6_mbc1_standard_banking_fixture() {
    assert_phase_6_cartridge_oracle_case_passes("phase6-mbc1-standard-banking");
}

#[test]
fn runner_executes_phase_6_mbc1_small_rom_mask_and_ram_fixture() {
    assert_phase_6_cartridge_oracle_case_passes("phase6-mbc1-small-rom-mask-and-ram");
}

#[test]
fn runner_executes_phase_6_mbc2_control_decode_and_nibble_ram_fixture() {
    assert_phase_6_cartridge_oracle_case_passes("phase6-mbc2-control-decode-and-nibble-ram");
}

#[test]
fn runner_executes_phase_6_mbc3_banking_ram_and_rtc_fixture() {
    assert_phase_6_cartridge_oracle_case_passes("phase6-mbc3-banking-ram-and-rtc");
}

#[test]
fn runner_executes_phase_6_mbc5_rom_banking_rumble_and_ram_fixture() {
    assert_phase_6_cartridge_oracle_case_passes("phase6-mbc5-rom-banking-rumble-and-ram");
}

fn assert_phase_6_cartridge_oracle_case_passes(case_id: &str) {
    let suite = phase_6_cartridge_oracle_suite();
    let case = suite
        .cases
        .iter()
        .find(|case| case.id == case_id)
        .expect("phase 6 cartridge suite should include the requested case");

    let report = RomRunner::new()
        .run_case(case)
        .expect("phase 6 cartridge case should execute");

    assert_eq!(report.outcome, RomCaseOutcome::Passed, "{report:#?}");
}

#[test]
fn runner_captures_serial_output_from_a_minimal_rom() {
    let temp_dir = unique_temp_dir("serial-pass");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("serial_pass.gb");
    fs::write(&rom_path, build_single_byte_serial_rom(b'O'))
        .expect("serial test rom should be writable");

    let case = RomTestCase::new(
        "serial-pass",
        &rom_path,
        Timeout::TCycles(5_000),
        PassCondition::SerialContains("O".to_string()),
    );

    let report = RomRunner::new()
        .run_case(&case)
        .expect("serial case should execute");

    assert_eq!(report.outcome, RomCaseOutcome::Passed);
    assert_eq!(report.artifacts.serial.as_deref(), Some("O"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn runner_uses_execution_mode_compatibility_presets_for_loader_validation() {
    let temp_dir = unique_temp_dir("permissive-loader-validation");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("legacy_nomcb_ram_header.gb");
    fs::write(
        &rom_path,
        build_test_rom_with_header(
            &[
                0x3E, b'O', // LD A,'O'
                0xE0, 0x01, // LDH (SB),A
                0x3E, 0x81, // LD A,$81
                0xE0, 0x02, // LDH (SC),A
                0xC3, 0x08, 0x01, // JP $0108
            ],
            0x08,
            0x00,
            0x01,
        ),
    )
    .expect("permissive loader test rom should be writable");

    let case = RomTestCase::new(
        "permissive-loader-validation",
        &rom_path,
        Timeout::TCycles(5_000),
        PassCondition::SerialContains("O".to_string()),
    )
    .with_execution_mode(ExecutionMode::Permissive);

    let report = RomRunner::new()
        .run_case(&case)
        .expect("permissive execution mode should admit the ROM");

    assert_eq!(report.outcome, RomCaseOutcome::Passed);
    assert_eq!(report.artifacts.serial.as_deref(), Some("O"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn runner_applies_startup_memory_writes_before_execution() {
    let temp_dir = unique_temp_dir("startup-memory-write");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("startup_memory_write.gb");
    fs::write(&rom_path, build_serial_from_address_rom(0xC000))
        .expect("startup memory write test rom should be writable");

    let case = RomTestCase::new(
        "startup-memory-write",
        &rom_path,
        Timeout::TCycles(5_000),
        PassCondition::SerialContains("Z".to_string()),
    )
    .with_startup_memory_write(StartupMemoryWrite::new(0xC000, b'Z'));

    let report = RomRunner::new()
        .run_case(&case)
        .expect("startup memory write case should execute");

    assert_eq!(report.outcome, RomCaseOutcome::Passed);
    assert_eq!(report.artifacts.serial.as_deref(), Some("Z"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn runner_applies_startup_memory_writes_to_vram_before_execution() {
    let temp_dir = unique_temp_dir("startup-memory-write-vram");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("startup_memory_write_vram.gb");
    fs::write(&rom_path, build_serial_from_address_rom(0x8010))
        .expect("startup memory write vram test rom should be writable");

    let case = RomTestCase::new(
        "startup-memory-write-vram",
        &rom_path,
        Timeout::TCycles(5_000),
        PassCondition::SerialContains("Z".to_string()),
    )
    .with_startup_memory_write(StartupMemoryWrite::new(0x8010, b'Z'));

    let report = RomRunner::new()
        .run_case(&case)
        .expect("startup memory write vram case should execute");

    assert_eq!(report.outcome, RomCaseOutcome::Passed);
    assert_eq!(report.artifacts.serial.as_deref(), Some("Z"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn runner_can_read_startup_seeded_vram_after_disabling_lcd() {
    let temp_dir = unique_temp_dir("startup-memory-write-vram-lcd-off");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("startup_memory_write_vram_lcd_off.gb");
    fs::write(
        &rom_path,
        build_lcd_off_then_serial_from_address_rom(0x8010),
    )
    .expect("startup memory write vram lcd-off test rom should be writable");

    let case = RomTestCase::new(
        "startup-memory-write-vram-lcd-off",
        &rom_path,
        Timeout::TCycles(5_000),
        PassCondition::SerialContains("Z".to_string()),
    )
    .with_startup_memory_write(StartupMemoryWrite::new(0x8010, b'Z'));

    let report = RomRunner::new()
        .run_case(&case)
        .expect("startup memory write vram lcd-off case should execute");

    assert_eq!(report.outcome, RomCaseOutcome::Passed);
    assert_eq!(report.artifacts.serial.as_deref(), Some("Z"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn runner_captures_memory_text_output_from_a_ram_backed_rom() {
    let temp_dir = unique_temp_dir("memory-text-output-pass");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("memory_text_output.gb");
    fs::write(&rom_path, build_memory_text_output_rom())
        .expect("memory text output rom should be writable");

    let case = RomTestCase::new(
        "memory-text-output-pass",
        &rom_path,
        Timeout::TCycles(8_192),
        PassCondition::MemoryTextOutputContains {
            spec: MemoryTextOutputSpec::new(
                0xA000,
                0x80,
                0x00,
                0xA001,
                [0xDE, 0xB0, 0x61],
                0xA004,
                64,
            ),
            expected_substring: "Passed".to_string(),
        },
    );

    let report = RomRunner::new()
        .run_case(&case)
        .expect("memory text output case should execute");

    assert_eq!(report.outcome, RomCaseOutcome::Passed);
    assert_eq!(
        report
            .artifacts
            .memory_text_output
            .as_ref()
            .map(|captured| captured.status),
        Some(0x00)
    );
    assert_eq!(
        report
            .artifacts
            .memory_text_output
            .as_ref()
            .map(|captured| captured.signature),
        Some([0xDE, 0xB0, 0x61])
    );
    assert_eq!(
        report
            .artifacts
            .memory_text_output
            .as_ref()
            .map(|captured| captured.text.as_str()),
        Some("Passed")
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn runner_captures_blargg_console_text_from_bg_map_output() {
    let temp_dir = unique_temp_dir("blargg-console-text-pass");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("blargg_console_text.gb");
    fs::write(&rom_path, build_blargg_console_text_rom())
        .expect("blargg console text rom should be writable");

    let case = RomTestCase::new(
        "blargg-console-text-pass",
        &rom_path,
        Timeout::TCycles(8_192),
        PassCondition::BlarggConsoleTextContains("Passed".to_string()),
    );

    let report = RomRunner::new()
        .run_case(&case)
        .expect("blargg console text case should execute");

    assert_eq!(report.outcome, RomCaseOutcome::Passed);
    assert!(
        report
            .artifacts
            .blargg_console_text
            .as_deref()
            .is_some_and(|text| text.contains("Passed"))
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn runner_marks_informational_cases_as_non_failing_without_promoting_them_to_passed() {
    let temp_dir = unique_temp_dir("info-case");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("info_case.gb");
    fs::write(
        &rom_path,
        build_test_rom(&[
            0xC3, 0x00, 0x01, // JP $0100
        ]),
    )
    .expect("informational test rom should be writable");

    let case = RomTestCase::new(
        "info-case",
        &rom_path,
        Timeout::TCycles(128),
        PassCondition::Informational(CaptureKind::Snapshot),
    );

    let report = RomRunner::new()
        .run_case(&case)
        .expect("informational case should execute");

    assert_eq!(report.outcome, RomCaseOutcome::Informational);
    assert!(report.non_failing());
    assert!(!report.passed());
    assert!(report.artifacts.snapshot_text.is_some());

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn runner_resolves_roms_from_an_explicit_external_root() {
    let temp_dir = unique_temp_dir("external-root");
    let rom_root = temp_dir.join("test-rom-root");
    fs::create_dir_all(rom_root.join("blargg/cpu_instrs"))
        .expect("external root directory should be creatable");

    let rom_path = rom_root.join("blargg/cpu_instrs/01-special.gb");
    fs::write(&rom_path, build_single_byte_serial_rom(b'P'))
        .expect("external root rom should be writable");

    let case = RomTestCase::new(
        "external-root-pass",
        "blargg/cpu_instrs/01-special.gb",
        Timeout::TCycles(5_000),
        PassCondition::SerialContains("P".to_string()),
    )
    .with_external_rom_root_key(TEST_ROM_ROOT_ENV_VAR);

    let report = RomRunner::new()
        .with_external_rom_root(TEST_ROM_ROOT_ENV_VAR, &rom_root)
        .run_case(&case)
        .expect("external-root rom case should execute");

    assert_eq!(report.outcome, RomCaseOutcome::Passed);
    assert_eq!(report.artifacts.serial.as_deref(), Some("P"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn runner_resolves_roms_from_the_default_repo_managed_test_rom_store() {
    let temp_dir = unique_temp_dir("default-test-rom-store");
    let rom_root = temp_dir.join(".roms/test");
    fs::create_dir_all(rom_root.join("blargg/cpu_instrs"))
        .expect("default test rom store directory should be creatable");

    let rom_path = rom_root.join("blargg/cpu_instrs/01-special.gb");
    fs::write(&rom_path, build_single_byte_serial_rom(b'R'))
        .expect("default test rom store rom should be writable");

    let case = RomTestCase::new(
        "default-test-rom-root-pass",
        "blargg/cpu_instrs/01-special.gb",
        Timeout::TCycles(5_000),
        PassCondition::SerialContains("R".to_string()),
    )
    .with_external_rom_root_key(TEST_ROM_ROOT_ENV_VAR);

    let report = RomRunner::new()
        .with_workspace_root(&temp_dir)
        .run_case(&case)
        .expect("default test-rom-store case should execute");

    assert_eq!(report.outcome, RomCaseOutcome::Passed);
    assert_eq!(report.artifacts.serial.as_deref(), Some("R"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn runner_uses_explicit_boot_rom_root_for_real_boot_cases() {
    let temp_dir = unique_temp_dir("real-boot-store");
    let boot_rom_root = temp_dir.join("bootroms");
    fs::create_dir_all(&boot_rom_root).expect("boot rom root should be creatable");
    fs::write(
        boot_rom_root.join("dmg_boot.bin"),
        build_real_boot_jump_stub(),
    )
    .expect("boot rom should be writable");

    let rom_path = temp_dir.join("real_boot_serial.gb");
    fs::write(&rom_path, build_single_byte_serial_rom(b'B'))
        .expect("real boot rom should be writable");

    let case = RomTestCase::new(
        "real-boot-pass",
        &rom_path,
        Timeout::TCycles(5_000),
        PassCondition::SerialContains("B".to_string()),
    )
    .with_startup_mode(gb_core::StartupMode::RealBoot);

    let report = RomRunner::new()
        .with_workspace_root(&temp_dir)
        .with_boot_rom_root(&boot_rom_root)
        .with_boot_rom_verification_mode(BootRomVerificationMode::Off)
        .run_case(&case)
        .expect("real-boot case should execute");

    assert_eq!(report.outcome, RomCaseOutcome::Passed);
    assert_eq!(report.artifacts.serial.as_deref(), Some("B"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn runner_starts_case_timeout_after_real_boot_handoff() {
    let temp_dir = unique_temp_dir("real-boot-timeout-after-handoff");
    let boot_rom_root = temp_dir.join("bootroms");
    fs::create_dir_all(&boot_rom_root).expect("boot rom root should be creatable");
    fs::write(
        boot_rom_root.join("dmg_boot.bin"),
        build_delayed_real_boot_handoff_stub(),
    )
    .expect("boot rom should be writable");

    let rom_path = temp_dir.join("real_boot_serial_after_delay.gb");
    fs::write(&rom_path, build_single_byte_serial_rom(b'B'))
        .expect("real boot rom should be writable");

    let case = RomTestCase::new(
        "real-boot-timeout-after-handoff",
        &rom_path,
        Timeout::TCycles(5_000),
        PassCondition::SerialContains("B".to_string()),
    )
    .with_startup_mode(gb_core::StartupMode::RealBoot);

    let report = RomRunner::new()
        .with_workspace_root(&temp_dir)
        .with_boot_rom_root(&boot_rom_root)
        .with_boot_rom_verification_mode(BootRomVerificationMode::Off)
        .run_case(&case)
        .expect("real-boot case should execute");

    assert_eq!(report.outcome, RomCaseOutcome::Passed);
    assert_eq!(report.artifacts.serial.as_deref(), Some("B"));
    assert!(
        report.executed_t_cycles < 5_000,
        "post-handoff budget should exclude boot-ROM delay: {report:#?}"
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn runner_rejects_unexpected_boot_rom_hashes_in_strict_real_boot_mode() {
    let temp_dir = unique_temp_dir("real-boot-hash-mismatch");
    let boot_rom_root = temp_dir.join("bootroms");
    fs::create_dir_all(&boot_rom_root).expect("boot rom root should be creatable");
    fs::write(
        boot_rom_root.join("dmg_boot.bin"),
        build_real_boot_jump_stub(),
    )
    .expect("boot rom should be writable");

    let rom_path = temp_dir.join("real_boot_hash_mismatch.gb");
    fs::write(&rom_path, build_single_byte_serial_rom(b'B')).expect("rom should be writable");

    let case = RomTestCase::new(
        "real-boot-hash-mismatch",
        &rom_path,
        Timeout::TCycles(5_000),
        PassCondition::SerialContains("B".to_string()),
    )
    .with_startup_mode(gb_core::StartupMode::RealBoot);

    let error = RomRunner::new()
        .with_workspace_root(&temp_dir)
        .with_boot_rom_root(&boot_rom_root)
        .run_case(&case)
        .expect_err("strict real-boot should reject unexpected boot rom hashes");

    assert!(matches!(
        error,
        RomExecutionError::BootRomVerification { .. }
    ));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn runner_errors_when_external_root_is_missing() {
    let case = RomTestCase::new(
        "missing-external-root",
        "blargg/cpu_instrs/01-special.gb",
        Timeout::TCycles(64),
        PassCondition::SerialContains("Passed".to_string()),
    )
    .with_external_rom_root_key("GB_CYCLE_TEST_MISSING_ROOT");

    let error = RomRunner::new()
        .run_case(&case)
        .expect_err("missing external root should fail");

    match error {
        RomExecutionError::MissingExternalRomRoot { key, relative_path } => {
            assert_eq!(key, "GB_CYCLE_TEST_MISSING_ROOT");
            assert_eq!(
                relative_path,
                PathBuf::from("blargg/cpu_instrs/01-special.gb")
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn runner_reports_cpu_diagnostic_trap_before_timeout() {
    let temp_dir = unique_temp_dir("cpu-trap");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("cpu_trap.gb");
    fs::write(&rom_path, build_unsupported_opcode_rom(0xD3))
        .expect("diagnostic trap rom should be writable");

    let case = RomTestCase::new(
        "cpu-trap",
        &rom_path,
        Timeout::TCycles(5_000),
        PassCondition::SerialContains("Passed".to_string()),
    );

    let report = RomRunner::new()
        .run_case(&case)
        .expect("diagnostic trap case should execute");

    assert_eq!(
        report.outcome,
        RomCaseOutcome::Failed(RomCaseFailure::CpuDiagnosticTrap {
            trap: gb_core::CpuDiagnosticTrap::InvalidOpcode {
                opcode: 0xD3,
                address: 0x0100,
            },
        })
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn runner_persists_requested_failure_artifacts() {
    let rom_dir = unique_temp_dir("serial-fail-rom");
    fs::create_dir_all(&rom_dir).expect("rom temp dir should be creatable");
    let artifact_dir = unique_temp_dir("serial-fail-artifacts");

    let rom_path = rom_dir.join("serial_fail.gb");
    fs::write(&rom_path, build_single_byte_serial_rom(b'O'))
        .expect("serial failure rom should be writable");

    let case = RomTestCase::new(
        "serial-fail",
        &rom_path,
        Timeout::TCycles(5_000),
        PassCondition::SerialExact("K".to_string()),
    );

    let report = RomRunner::new()
        .with_failure_artifact_root(&artifact_dir)
        .run_case(&case)
        .expect("serial failure case should execute");

    match report.outcome {
        RomCaseOutcome::Failed(_) => {}
        other => panic!("expected a failing case outcome, got {other:?}"),
    }

    let serial_artifact = artifact_dir.join("serial-fail").join("serial.txt");
    let trace_artifact = artifact_dir.join("serial-fail").join("trace.txt");
    let snapshot_artifact = artifact_dir.join("serial-fail").join("snapshot.txt");

    assert_eq!(
        fs::read_to_string(&serial_artifact).expect("serial artifact should be readable"),
        "O"
    );
    assert!(
        fs::read_to_string(&trace_artifact)
            .expect("trace artifact should be readable")
            .contains("subsystem=serial")
    );
    assert!(
        fs::read_to_string(&snapshot_artifact)
            .expect("snapshot artifact should be readable")
            .contains("serial.sb=")
    );

    fs::remove_dir_all(rom_dir).expect("rom temp dir should be removable");
    fs::remove_dir_all(artifact_dir).expect("artifact temp dir should be removable");
}
