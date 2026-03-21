use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use gb_test_runner::{
    EXTERNAL_ROM_SOURCE_MANIFEST_PATH, MemoryTextOutputSpec, PassCondition,
    RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR, RomCaseFailure, RomCaseOutcome, RomExecutionError, RomRunner,
    RomTestCase, Timeout, phase_2_cpu_timing_suite, phase_2_interrupt_timing_suite,
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

fn write_external_rom_manifest(workspace_root: &Path) {
    let manifest_path = workspace_root.join(EXTERNAL_ROM_SOURCE_MANIFEST_PATH);
    let manifest_parent = manifest_path
        .parent()
        .expect("manifest path should have a parent");
    fs::create_dir_all(manifest_parent).expect("manifest parent should be creatable");
    fs::write(
        manifest_path,
        concat!(
            "version = 1\n\n",
            "[[source]]\n",
            "id = \"retrio-gb-test-roms\"\n",
            "git_url = \"https://example.invalid/retrio/gb-test-roms.git\"\n",
            "git_rev = \"deadbeef\"\n",
            "local_dir = \"retrio-gb-test-roms\"\n",
            "root_env_var = \"GB_CYCLE_RETRIO_GB_TEST_ROMS_ROOT\"\n\n",
            "[[source.required_file]]\n",
            "path = \"cpu_instrs/individual/01-special.gb\"\n",
            "sha256 = \"unused-in-local-root-resolution-tests\"\n",
        ),
    )
    .expect("manifest should be writable");
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
fn runner_resolves_roms_from_an_explicit_external_root() {
    let temp_dir = unique_temp_dir("external-root");
    let rom_root = temp_dir.join("retrio-root");
    fs::create_dir_all(rom_root.join("cpu_instrs/individual"))
        .expect("external root directory should be creatable");

    let rom_path = rom_root.join("cpu_instrs/individual/01-special.gb");
    fs::write(&rom_path, build_single_byte_serial_rom(b'P'))
        .expect("external root rom should be writable");

    let case = RomTestCase::new(
        "external-root-pass",
        "cpu_instrs/individual/01-special.gb",
        Timeout::TCycles(5_000),
        PassCondition::SerialContains("P".to_string()),
    )
    .with_external_rom_root_key(RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR);

    let report = RomRunner::new()
        .with_external_rom_root(RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR, &rom_root)
        .run_case(&case)
        .expect("external-root rom case should execute");

    assert_eq!(report.outcome, RomCaseOutcome::Passed);
    assert_eq!(report.artifacts.serial.as_deref(), Some("P"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn runner_resolves_roms_from_the_default_repo_managed_external_store() {
    let temp_dir = unique_temp_dir("default-external-store");
    let rom_root = temp_dir.join(".roms/external-test/retrio-gb-test-roms");
    fs::create_dir_all(rom_root.join("cpu_instrs/individual"))
        .expect("default external store directory should be creatable");
    write_external_rom_manifest(&temp_dir);

    let rom_path = rom_root.join("cpu_instrs/individual/01-special.gb");
    fs::write(&rom_path, build_single_byte_serial_rom(b'R'))
        .expect("default external store rom should be writable");

    let case = RomTestCase::new(
        "default-external-root-pass",
        "cpu_instrs/individual/01-special.gb",
        Timeout::TCycles(5_000),
        PassCondition::SerialContains("R".to_string()),
    )
    .with_external_rom_root_key(RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR);

    let report = RomRunner::new()
        .with_workspace_root(&temp_dir)
        .run_case(&case)
        .expect("default external-root rom case should execute");

    assert_eq!(report.outcome, RomCaseOutcome::Passed);
    assert_eq!(report.artifacts.serial.as_deref(), Some("R"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn runner_errors_when_external_root_is_missing() {
    let case = RomTestCase::new(
        "missing-external-root",
        "cpu_instrs/individual/01-special.gb",
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
                PathBuf::from("cpu_instrs/individual/01-special.gb")
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
            trap: gb_core::CpuDiagnosticTrap::UnsupportedOpcode {
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
