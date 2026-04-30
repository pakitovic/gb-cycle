use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use gb_core::ExecutionMode;
use gb_test_runner::{
    DifferentialCaseMismatch, DifferentialCaseOutcome, DifferentialExecutionError,
    DifferentialOracle, DifferentialRunner, PassCondition, RomSuite, RomTestCase, TestSubsystem,
    Timeout, phase_2_cpu_timing_suite,
};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
const SERIAL_HEX_CASE_ID: &str = "serial-hex-differential";
const SERIAL_HEX_EXPECTED: &str = "4F";

fn build_test_rom(program: &[u8]) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    for (offset, byte) in program.iter().copied().enumerate() {
        rom[0x0100 + offset] = byte;
    }
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
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

fn unique_temp_dir(label: &str) -> PathBuf {
    env::temp_dir().join(format!(
        "gb-cycle-differential-{}-{}-{}",
        label,
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}

fn single_phase_2_case_suite() -> gb_test_runner::RomSuite {
    let mut suite = phase_2_cpu_timing_suite();
    suite.cases.truncate(1);
    suite
}

fn serial_hex_suite(rom_path: impl Into<PathBuf>) -> RomSuite {
    RomSuite::new("serial-hex-differential", TestSubsystem::Serial).with_case(RomTestCase::new(
        SERIAL_HEX_CASE_ID,
        rom_path,
        Timeout::TCycles(5_000),
        PassCondition::SerialHexExact(SERIAL_HEX_EXPECTED.to_string()),
    ))
}

fn write_serial_hex_oracle(oracle_root: &Path) {
    let case_dir = oracle_root.join(SERIAL_HEX_CASE_ID);
    fs::create_dir_all(&case_dir).expect("oracle case dir should be creatable");
    fs::write(case_dir.join("serial_hex.txt"), SERIAL_HEX_EXPECTED)
        .expect("oracle serial hex should be writable");
}

#[test]
fn differential_runner_matches_imported_trace_artifact_for_phase_2_case() {
    let oracle_root = unique_temp_dir("phase2-match");
    let case_dir = oracle_root.join("phase2-fetch-immediate-order");
    fs::create_dir_all(&case_dir).expect("oracle case dir should be creatable");
    fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../gb-core/tests/fixtures/traces/phase2/phase2_fetch_immediate_order.trace"),
        case_dir.join("trace.txt"),
    )
    .expect("oracle trace should be writable");

    let report = DifferentialRunner::new(DifferentialOracle::SameBoy, &oracle_root)
        .run_suite(&single_phase_2_case_suite())
        .expect("differential suite should run");

    assert!(report.all_matched());
    assert_eq!(report.cases.len(), 1);
    assert!(report.cases[0].local_report.passed());
    assert_eq!(report.cases[0].outcome, DifferentialCaseOutcome::Matched);
}

#[test]
fn differential_runner_archives_context_when_trace_diverges() {
    let oracle_root = unique_temp_dir("phase2-mismatch-oracle");
    let case_dir = oracle_root.join("phase2-fetch-immediate-order");
    fs::create_dir_all(&case_dir).expect("oracle case dir should be creatable");
    fs::write(case_dir.join("trace.txt"), "wrong trace\n")
        .expect("oracle trace should be writable");

    let artifact_root = unique_temp_dir("phase2-mismatch-artifacts");
    let report = DifferentialRunner::new(DifferentialOracle::SameBoy, &oracle_root)
        .with_failure_artifact_root(&artifact_root)
        .run_suite(&single_phase_2_case_suite())
        .expect("differential suite should run");

    let case = &report.cases[0];
    assert!(!case.matched());
    assert!(matches!(
        case.outcome,
        DifferentialCaseOutcome::Diverged(DifferentialCaseMismatch::TraceMismatch { .. })
    ));
    assert!(case.local_report.passed());

    let archived = &case.archived_context_artifacts;
    assert!(
        archived
            .iter()
            .any(|path| path.ends_with("differential_summary.txt"))
    );
    assert!(
        archived
            .iter()
            .any(|path| path.ends_with("local/trace.txt"))
    );
    assert!(
        archived
            .iter()
            .any(|path| path.ends_with("local/snapshot.txt"))
    );
    assert!(
        archived
            .iter()
            .any(|path| path.ends_with("oracle/trace.txt"))
    );
    for path in archived {
        assert!(
            path.is_file(),
            "expected archived artifact to exist: {}",
            path.display()
        );
    }
}

#[test]
fn differential_runner_rejects_non_strict_cases() {
    let mut suite = single_phase_2_case_suite();
    suite.cases[0].execution_mode = ExecutionMode::Permissive;

    let error = DifferentialRunner::new(DifferentialOracle::SameBoy, unique_temp_dir("strict"))
        .run_suite(&suite)
        .expect_err("non-strict suites should be rejected");

    assert!(matches!(
        error,
        DifferentialExecutionError::NonStrictCase { .. }
    ));
}

#[test]
fn differential_runner_matches_imported_serial_hex_artifact() {
    let oracle_root = unique_temp_dir("serial-hex-match");
    fs::create_dir_all(&oracle_root).expect("oracle root should be creatable");
    write_serial_hex_oracle(&oracle_root);

    let rom_path = oracle_root.join("serial_hex.gb");
    fs::write(&rom_path, build_single_byte_serial_rom(b'O')).expect("ROM should be writable");
    let suite = serial_hex_suite(&rom_path);

    let report = DifferentialRunner::new(DifferentialOracle::SameBoy, &oracle_root)
        .run_suite(&suite)
        .expect("differential suite should run");

    assert!(report.all_matched(), "{report:#?}");
    assert_eq!(report.cases.len(), 1);
    assert!(report.cases.iter().all(|case| case.local_report.passed()));
}
