use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use gb_core::ExecutionMode;
use gb_test_runner::{
    DifferentialCaseMismatch, DifferentialCaseOutcome, DifferentialExecutionError,
    DifferentialOracle, DifferentialRunner, phase_2_cpu_timing_suite,
};

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
