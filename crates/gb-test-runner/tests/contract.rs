use std::path::PathBuf;

use gb_core::{ConsoleModel, ExecutionMode, StartupMode};
use gb_test_runner::{
    CaptureKind, CapturePlan, FailureArtifactPolicy, PassCondition, RomCaseValidationError,
    RomSuite, RomSuiteValidationError, RomTestCase, TestSubsystem, Timeout,
    phase_2_cpu_timing_suite, phase_2_interrupt_timing_suite,
};

#[test]
fn new_rom_test_case_defaults_to_dmg_skip_boot_strict_with_debug_artifacts() {
    let case = RomTestCase::new(
        "blargg_cpu_instrs",
        PathBuf::from("crates/gb-core/tests/fixtures/roms/blargg.gb"),
        Timeout::Frames(600),
        PassCondition::SerialContains("Passed".to_string()),
    );

    assert_eq!(case.console_model, ConsoleModel::Dmg);
    assert_eq!(case.startup_mode, StartupMode::SkipBoot);
    assert_eq!(case.execution_mode, ExecutionMode::Strict);
    assert!(case.capture_plan.contains(CaptureKind::Serial));
    assert!(case.capture_plan.contains(CaptureKind::Trace));
    assert!(case.capture_plan.contains(CaptureKind::Snapshot));
    assert!(case.failure_artifacts.contains(CaptureKind::Serial));
    assert!(case.failure_artifacts.contains(CaptureKind::Trace));
    assert!(case.failure_artifacts.contains(CaptureKind::Snapshot));
    assert_eq!(case.validate(), Ok(()));
}

#[test]
fn rom_test_case_requires_capture_matching_the_pass_condition() {
    let case = RomTestCase::new(
        "dmg_acid2",
        PathBuf::from("crates/gb-core/tests/fixtures/roms/dmg-acid2.gb"),
        Timeout::Frames(1200),
        PassCondition::FramebufferFixture(PathBuf::from("expected/dmg-acid2.png")),
    )
    .with_capture_plan(CapturePlan::new().with_capture(CaptureKind::Trace))
    .with_failure_artifacts(
        FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::Framebuffer)
            .with_artifact(CaptureKind::Trace),
    );

    assert_eq!(
        case.validate(),
        Err(RomCaseValidationError::MissingRequiredCapture(
            CaptureKind::Framebuffer
        ))
    );
}

#[test]
fn rom_test_case_rejects_failure_artifacts_that_are_not_captured() {
    let case = RomTestCase::new(
        "mealybug_ly",
        PathBuf::from("crates/gb-core/tests/fixtures/roms/mealybug.gb"),
        Timeout::TCycles(4_194_304),
        PassCondition::TraceFixture(PathBuf::from("expected/ly.trace")),
    )
    .with_capture_plan(CapturePlan::new().with_capture(CaptureKind::Trace))
    .with_failure_artifacts(
        FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::Trace)
            .with_artifact(CaptureKind::Snapshot),
    );

    assert_eq!(
        case.validate(),
        Err(RomCaseValidationError::ArtifactNotCaptured(
            CaptureKind::Snapshot
        ))
    );
}

#[test]
fn rom_suite_rejects_duplicate_case_ids() {
    let first = RomTestCase::new(
        "same-id",
        PathBuf::from("a.gb"),
        Timeout::Frames(10),
        PassCondition::SerialExact("Passed".to_string()),
    );
    let second = RomTestCase::new(
        "same-id",
        PathBuf::from("b.gb"),
        Timeout::Frames(10),
        PassCondition::SerialExact("Passed".to_string()),
    )
    .with_console_model(ConsoleModel::Mgb)
    .with_startup_mode(StartupMode::RealBoot)
    .with_execution_mode(ExecutionMode::Permissive);

    let suite = RomSuite::new("cpu", TestSubsystem::Cpu)
        .with_case(first)
        .with_case(second);

    assert_eq!(
        suite.validate(),
        Err(RomSuiteValidationError::DuplicateCaseId(
            "same-id".to_string()
        ))
    );
}

#[test]
fn rom_suite_validates_a_scheduler_grouped_contract() {
    let suite =
        RomSuite::new("scheduler-ordering", TestSubsystem::Scheduler).with_case(RomTestCase::new(
            "phase-order-trace",
            PathBuf::from("synthetic/scheduler-order.gb"),
            Timeout::TCycles(1024),
            PassCondition::TraceFixture(PathBuf::from("expected/scheduler-order.trace")),
        ));

    assert_eq!(suite.validate(), Ok(()));
}

#[test]
fn rom_test_case_rejects_empty_id_empty_rom_path_and_zero_timeout() {
    let empty_id = RomTestCase::new(
        "",
        PathBuf::from("rom.gb"),
        Timeout::Frames(1),
        PassCondition::SerialExact("ok".to_string()),
    );
    let empty_path = RomTestCase::new(
        "valid-id",
        PathBuf::new(),
        Timeout::Frames(1),
        PassCondition::SerialExact("ok".to_string()),
    );
    let zero_timeout = RomTestCase::new(
        "valid-id",
        PathBuf::from("rom.gb"),
        Timeout::TCycles(0),
        PassCondition::SerialExact("ok".to_string()),
    );

    assert_eq!(
        empty_id.validate(),
        Err(RomCaseValidationError::EmptyCaseId)
    );
    assert_eq!(
        empty_path.validate(),
        Err(RomCaseValidationError::MissingRomPath)
    );
    assert_eq!(
        zero_timeout.validate(),
        Err(RomCaseValidationError::InvalidTimeout)
    );
}

#[test]
fn rom_test_case_requires_failure_artifacts_and_the_required_result_channel() {
    let missing_all_artifacts = RomTestCase::new(
        "serial-case",
        PathBuf::from("serial.gb"),
        Timeout::Frames(10),
        PassCondition::SerialContains("Passed".to_string()),
    )
    .with_failure_artifacts(FailureArtifactPolicy::new());

    let missing_required_artifact = RomTestCase::new(
        "framebuffer-case",
        PathBuf::from("lcd.gb"),
        Timeout::Frames(10),
        PassCondition::FramebufferFixture(PathBuf::from("expected/frame.png")),
    )
    .with_failure_artifacts(
        FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::Trace)
            .with_artifact(CaptureKind::Snapshot),
    );

    assert_eq!(
        missing_all_artifacts.validate(),
        Err(RomCaseValidationError::MissingFailureArtifacts)
    );
    assert_eq!(
        missing_required_artifact.validate(),
        Err(RomCaseValidationError::MissingRequiredFailureArtifact(
            CaptureKind::Framebuffer
        ))
    );
}

#[test]
fn rom_suite_rejects_empty_name_and_invalid_cases() {
    let invalid_case = RomTestCase::new(
        "",
        PathBuf::from("rom.gb"),
        Timeout::Frames(1),
        PassCondition::SerialExact("Passed".to_string()),
    );
    let empty_name = RomSuite::new("", TestSubsystem::Cpu);
    let invalid_suite = RomSuite::new("invalid-cases", TestSubsystem::Cpu).with_case(invalid_case);

    assert_eq!(
        empty_name.validate(),
        Err(RomSuiteValidationError::EmptySuiteName)
    );
    assert_eq!(
        invalid_suite.validate(),
        Err(RomSuiteValidationError::InvalidCase {
            case_id: "".to_string(),
            error: RomCaseValidationError::EmptyCaseId,
        })
    );
}

#[test]
fn capture_and_artifact_builders_expose_their_registered_sets() {
    let capture_plan = CapturePlan::debugging_minimum_for(&PassCondition::TraceFixture(
        PathBuf::from("expected.trace"),
    ));
    let failure_artifacts = FailureArtifactPolicy::debugging_minimum_for(
        &PassCondition::SerialExact("Passed".to_string()),
    );

    assert_eq!(capture_plan.captures().len(), 2);
    assert!(capture_plan.contains(CaptureKind::Trace));
    assert!(capture_plan.contains(CaptureKind::Snapshot));

    assert_eq!(failure_artifacts.retained().len(), 3);
    assert!(failure_artifacts.contains(CaptureKind::Serial));
    assert!(failure_artifacts.contains(CaptureKind::Trace));
    assert!(failure_artifacts.contains(CaptureKind::Snapshot));
}

#[test]
fn rom_suite_can_be_built_incrementally_with_push_case() {
    let mut suite = RomSuite::new("boot", TestSubsystem::Boot);
    suite.push_case(RomTestCase::new(
        "skip-boot-handshake",
        PathBuf::from("boot.gb"),
        Timeout::Frames(60),
        PassCondition::TraceFixture(PathBuf::from("boot.trace")),
    ));

    assert_eq!(suite.cases.len(), 1);
    assert_eq!(suite.validate(), Ok(()));
}

#[test]
fn phase_2_rom_automation_targets_validate_for_cpu_and_interrupt_timing() {
    let cpu_suite = phase_2_cpu_timing_suite();
    let interrupt_suite = phase_2_interrupt_timing_suite();

    assert_eq!(cpu_suite.subsystem, TestSubsystem::Cpu);
    assert_eq!(interrupt_suite.subsystem, TestSubsystem::Interrupts);
    assert_eq!(cpu_suite.validate(), Ok(()));
    assert_eq!(interrupt_suite.validate(), Ok(()));
    assert!(
        cpu_suite
            .cases
            .iter()
            .all(|case| case.execution_mode == ExecutionMode::Strict)
    );
    assert!(
        interrupt_suite
            .cases
            .iter()
            .all(|case| case.execution_mode == ExecutionMode::Strict)
    );
    assert!(
        cpu_suite
            .cases
            .iter()
            .chain(interrupt_suite.cases.iter())
            .all(|case| case.capture_plan.contains(CaptureKind::Trace))
    );
    assert!(
        cpu_suite
            .cases
            .iter()
            .chain(interrupt_suite.cases.iter())
            .all(|case| case.failure_artifacts.contains(CaptureKind::Snapshot))
    );
}
