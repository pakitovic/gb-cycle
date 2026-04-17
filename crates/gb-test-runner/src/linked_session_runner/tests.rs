use super::*;
use crate::{
    ExternalStimulus, ExternalStimulusAction, LinkedSessionCapturePlan, LinkedSessionCase,
    LinkedSessionFailureArtifactPolicy, LinkedSessionParticipant, LinkedSessionPassCondition,
    LinkedSessionSuite, LinkedSessionTopology,
};
use std::env;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;

fn unique_temp_dir(label: &str) -> PathBuf {
    env::temp_dir().join(format!(
        "gb-cycle-linked-session-runner-{}-{}-{}",
        label,
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}

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

fn build_single_shot_serial_from_address_rom(address: u16, sc_value: u8) -> Vec<u8> {
    build_test_rom(&[
        0xFA,
        address as u8,
        (address >> 8) as u8, // LD A,(a16)
        0xE0,
        0x01, // LDH (SB),A
        0x3E,
        sc_value, // LD A,SC
        0xE0,
        0x02, // LDH (SC),A
        0xC3,
        0x08,
        0x01, // JP 0108 (self-loop after arming transfer)
    ])
}

#[test]
fn linked_session_runner_executes_a_dmg04_exchange_and_captures_session_trace() {
    let temp_dir = unique_temp_dir("dmg04-exchange");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let left_rom = temp_dir.join("left.gb");
    let right_rom = temp_dir.join("right.gb");
    fs::write(
        &left_rom,
        build_single_shot_serial_from_address_rom(0xC000, 0x81),
    )
    .expect("left ROM should be writable");
    fs::write(
        &right_rom,
        build_single_shot_serial_from_address_rom(0xC100, 0x80),
    )
    .expect("right ROM should be writable");

    let session = LinkedSessionCase::new(
        "dmg04-exchange",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(5_000),
        LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Trace),
    )
    .with_participant(
        LinkedSessionParticipant::new("left", &left_rom).with_external_stimulus(
            ExternalStimulus::at_t_cycle(
                0,
                ExternalStimulusAction::WriteMemory {
                    address: 0xC000,
                    value: 0xA5,
                },
            ),
        ),
    )
    .with_participant(
        LinkedSessionParticipant::new("right", &right_rom).with_external_stimulus(
            ExternalStimulus::at_t_cycle(
                0,
                ExternalStimulusAction::WriteMemory {
                    address: 0xC100,
                    value: 0x3C,
                },
            ),
        ),
    );

    let report = LinkedSessionRunner::new()
        .run_session(&session)
        .expect("linked session should execute");

    assert_eq!(report.outcome, LinkedSessionCaseOutcome::Informational);
    assert_eq!(report.participants.len(), 2);
    assert_eq!(report.participants[0].artifacts.serial_hex, "A5");
    assert_eq!(report.participants[1].artifacts.serial_hex, "3C");
    let trace = report
        .artifacts
        .trace
        .as_deref()
        .expect("trace artifact should be captured");
    assert!(trace.contains("== participant left trace =="));
    assert!(trace.contains("== participant right trace =="));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn linked_session_runner_only_captures_participant_snapshots_when_requested() {
    let temp_dir = unique_temp_dir("serial-hex-no-snapshot");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let left_rom = temp_dir.join("left.gb");
    let right_rom = temp_dir.join("right.gb");
    fs::write(
        &left_rom,
        build_single_shot_serial_from_address_rom(0xC000, 0x81),
    )
    .expect("left ROM should be writable");
    fs::write(
        &right_rom,
        build_single_shot_serial_from_address_rom(0xC100, 0x80),
    )
    .expect("right ROM should be writable");

    let session = LinkedSessionCase::new(
        "serial-hex-no-snapshot",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(5_000),
        LinkedSessionPassCondition::ParticipantSerialHexExact {
            participant_id: "left".to_string(),
            expected: "A5".to_string(),
        },
    )
    .with_capture_plan(
        LinkedSessionCapturePlan::new()
            .with_capture(LinkedSessionCaptureKind::ParticipantSerialHex),
    )
    .with_failure_artifacts(
        LinkedSessionFailureArtifactPolicy::new()
            .with_artifact(LinkedSessionCaptureKind::ParticipantSerialHex),
    )
    .with_participant(
        LinkedSessionParticipant::new("left", &left_rom).with_external_stimulus(
            ExternalStimulus::at_t_cycle(
                0,
                ExternalStimulusAction::WriteMemory {
                    address: 0xC000,
                    value: 0xA5,
                },
            ),
        ),
    )
    .with_participant(
        LinkedSessionParticipant::new("right", &right_rom).with_external_stimulus(
            ExternalStimulus::at_t_cycle(
                0,
                ExternalStimulusAction::WriteMemory {
                    address: 0xC100,
                    value: 0x3C,
                },
            ),
        ),
    );

    let report = LinkedSessionRunner::new()
        .run_session(&session)
        .expect("serial-hex-only linked session should execute");

    assert_eq!(report.outcome, LinkedSessionCaseOutcome::Passed);
    assert!(report.artifacts.snapshot_text.is_none());
    assert!(report.participants[0].artifacts.snapshot_text.is_none());
    assert!(report.participants[1].artifacts.snapshot_text.is_none());

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn linked_session_runner_replays_trace_fixtures_deterministically() {
    let temp_dir = unique_temp_dir("trace-fixture");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let left_rom = temp_dir.join("left.gb");
    let right_rom = temp_dir.join("right.gb");
    fs::write(
        &left_rom,
        build_single_shot_serial_from_address_rom(0xC000, 0x81),
    )
    .expect("left ROM should be writable");
    fs::write(
        &right_rom,
        build_single_shot_serial_from_address_rom(0xC100, 0x80),
    )
    .expect("right ROM should be writable");

    let info_session = LinkedSessionCase::new(
        "fixture-source",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(5_000),
        LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Trace),
    )
    .with_participant(
        LinkedSessionParticipant::new("left", &left_rom).with_external_stimulus(
            ExternalStimulus::at_t_cycle(
                0,
                ExternalStimulusAction::WriteMemory {
                    address: 0xC000,
                    value: 0x11,
                },
            ),
        ),
    )
    .with_participant(
        LinkedSessionParticipant::new("right", &right_rom).with_external_stimulus(
            ExternalStimulus::at_t_cycle(
                0,
                ExternalStimulusAction::WriteMemory {
                    address: 0xC100,
                    value: 0x22,
                },
            ),
        ),
    );

    let info_report = LinkedSessionRunner::new()
        .run_session(&info_session)
        .expect("informational linked session should execute");
    let fixture_path = temp_dir.join("linked.trace");
    fs::write(
        &fixture_path,
        info_report
            .artifacts
            .trace
            .as_deref()
            .expect("trace artifact should exist"),
    )
    .expect("fixture should be writable");

    let fixture_session = LinkedSessionCase::new(
        "fixture-match",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(5_000),
        LinkedSessionPassCondition::TraceFixture(fixture_path.clone()),
    )
    .with_participant(
        LinkedSessionParticipant::new("left", &left_rom).with_external_stimulus(
            ExternalStimulus::at_t_cycle(
                0,
                ExternalStimulusAction::WriteMemory {
                    address: 0xC000,
                    value: 0x11,
                },
            ),
        ),
    )
    .with_participant(
        LinkedSessionParticipant::new("right", &right_rom).with_external_stimulus(
            ExternalStimulus::at_t_cycle(
                0,
                ExternalStimulusAction::WriteMemory {
                    address: 0xC100,
                    value: 0x22,
                },
            ),
        ),
    );

    let runner = LinkedSessionRunner::new();
    let first = runner
        .run_session(&fixture_session)
        .expect("fixture session should pass");
    let second = runner
        .run_session(&fixture_session)
        .expect("fixture session should rerun deterministically");

    assert_eq!(first.outcome, LinkedSessionCaseOutcome::Passed);
    assert_eq!(second.outcome, LinkedSessionCaseOutcome::Passed);
    assert_eq!(first.artifacts, second.artifacts);
    assert_eq!(first.participants, second.participants);

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn linked_session_runner_reports_missing_external_rom_roots_as_typed_errors() {
    let session = LinkedSessionCase::new(
        "missing-external-root",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(32),
        LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Snapshot),
    )
    .with_participant(
        LinkedSessionParticipant::new("left", Path::new("left.gb"))
            .with_external_rom_root_key("GB_CYCLE_TEST_MISSING_ROOT"),
    )
    .with_participant(LinkedSessionParticipant::new(
        "right",
        Path::new("right.gb"),
    ));

    let error = LinkedSessionRunner::new()
        .run_session(&session)
        .expect_err("missing external ROM root should surface as a typed error");

    assert!(matches!(
        error,
        LinkedSessionExecutionError::MissingExternalRomRoot {
            ref key,
            ref relative_path,
        } if key == "GB_CYCLE_TEST_MISSING_ROOT" && relative_path == Path::new("left.gb")
    ));
}

#[test]
fn linked_session_runner_supports_participant_serial_hex_expectations() {
    let temp_dir = unique_temp_dir("participant-serial-hex-pass");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let left_rom = temp_dir.join("left.gb");
    let right_rom = temp_dir.join("right.gb");
    fs::write(
        &left_rom,
        build_test_rom(&[
            0x3E, 0xA5, 0xE0, 0x01, 0x3E, 0x81, 0xE0, 0x02, 0xC3, 0x08, 0x01,
        ]),
    )
    .expect("left ROM should be writable");
    fs::write(
        &right_rom,
        build_test_rom(&[
            0x3E, 0x3C, 0xE0, 0x01, 0x3E, 0x80, 0xE0, 0x02, 0xC3, 0x08, 0x01,
        ]),
    )
    .expect("right ROM should be writable");

    let session = LinkedSessionCase::new(
        "participant-serial-hex-pass",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(5_000),
        LinkedSessionPassCondition::ParticipantSerialHexExact {
            participant_id: "left".to_string(),
            expected: "A5".to_string(),
        },
    )
    .with_participant(LinkedSessionParticipant::new("left", &left_rom))
    .with_participant(LinkedSessionParticipant::new("right", &right_rom));

    let report = LinkedSessionRunner::new()
        .run_session(&session)
        .expect("participant serial hex session should execute");

    assert_eq!(report.outcome, LinkedSessionCaseOutcome::Passed);
    assert_eq!(
        report.participants[0].outcome,
        LinkedSessionCaseOutcome::Passed
    );
    assert_eq!(
        report.participants[1].outcome,
        LinkedSessionCaseOutcome::Passed
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn linked_session_runner_supports_participant_snapshot_fixtures() {
    let temp_dir = unique_temp_dir("participant-snapshot-pass");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let left_rom = temp_dir.join("left.gb");
    let right_rom = temp_dir.join("right.gb");
    let fixture_path = temp_dir.join("left.snapshot");
    fs::write(
        &left_rom,
        build_test_rom(&[
            0x3E, 0xA5, 0xE0, 0x01, 0x3E, 0x81, 0xE0, 0x02, 0xC3, 0x08, 0x01,
        ]),
    )
    .expect("left ROM should be writable");
    fs::write(
        &right_rom,
        build_test_rom(&[
            0x3E, 0x3C, 0xE0, 0x01, 0x3E, 0x80, 0xE0, 0x02, 0xC3, 0x08, 0x01,
        ]),
    )
    .expect("right ROM should be writable");

    let baseline = LinkedSessionCase::new(
        "participant-snapshot-baseline",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(5_000),
        LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Snapshot),
    )
    .with_participant(LinkedSessionParticipant::new("left", &left_rom))
    .with_participant(LinkedSessionParticipant::new("right", &right_rom));

    let baseline_report = LinkedSessionRunner::new()
        .run_session(&baseline)
        .expect("baseline linked snapshot session should execute");
    fs::write(
        &fixture_path,
        baseline_report.participants[0]
            .artifacts
            .snapshot_text
            .as_deref()
            .expect("baseline left snapshot should be captured"),
    )
    .expect("participant snapshot fixture should be writable");

    let session = LinkedSessionCase::new(
        "participant-snapshot-pass",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(5_000),
        LinkedSessionPassCondition::ParticipantSnapshotFixture {
            participant_id: "left".to_string(),
            fixture_path: fixture_path.clone(),
        },
    )
    .with_participant(LinkedSessionParticipant::new("left", &left_rom))
    .with_participant(LinkedSessionParticipant::new("right", &right_rom));

    let report = LinkedSessionRunner::new()
        .run_session(&session)
        .expect("participant snapshot fixture session should execute");

    assert_eq!(report.outcome, LinkedSessionCaseOutcome::Passed);
    assert_eq!(
        report.participants[0].outcome,
        LinkedSessionCaseOutcome::Passed
    );
    assert_eq!(
        report.participants[1].outcome,
        LinkedSessionCaseOutcome::Passed
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn linked_session_runner_supports_participant_trace_fixtures() {
    let temp_dir = unique_temp_dir("participant-trace-pass");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let left_rom = temp_dir.join("left.gb");
    let right_rom = temp_dir.join("right.gb");
    let fixture_path = temp_dir.join("left.trace");
    fs::write(
        &left_rom,
        build_test_rom(&[
            0x3E, 0xA5, 0xE0, 0x01, 0x3E, 0x81, 0xE0, 0x02, 0xC3, 0x08, 0x01,
        ]),
    )
    .expect("left ROM should be writable");
    fs::write(
        &right_rom,
        build_test_rom(&[
            0x3E, 0x3C, 0xE0, 0x01, 0x3E, 0x80, 0xE0, 0x02, 0xC3, 0x08, 0x01,
        ]),
    )
    .expect("right ROM should be writable");

    let baseline = LinkedSessionCase::new(
        "participant-trace-baseline",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(5_000),
        LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Trace),
    )
    .with_participant(LinkedSessionParticipant::new("left", &left_rom))
    .with_participant(LinkedSessionParticipant::new("right", &right_rom));

    let baseline_report = LinkedSessionRunner::new()
        .run_session(&baseline)
        .expect("baseline linked trace session should execute");
    fs::write(
        &fixture_path,
        baseline_report.participants[0]
            .artifacts
            .trace_text
            .as_deref()
            .expect("baseline left trace should be captured"),
    )
    .expect("participant trace fixture should be writable");

    let session = LinkedSessionCase::new(
        "participant-trace-pass",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(5_000),
        LinkedSessionPassCondition::ParticipantTraceFixture {
            participant_id: "left".to_string(),
            fixture_path: fixture_path.clone(),
        },
    )
    .with_participant(LinkedSessionParticipant::new("left", &left_rom))
    .with_participant(LinkedSessionParticipant::new("right", &right_rom));

    let report = LinkedSessionRunner::new()
        .run_session(&session)
        .expect("participant trace fixture session should execute");

    assert_eq!(report.outcome, LinkedSessionCaseOutcome::Passed);
    assert_eq!(
        report.participants[0].outcome,
        LinkedSessionCaseOutcome::Passed
    );
    assert_eq!(
        report.participants[1].outcome,
        LinkedSessionCaseOutcome::Passed
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn linked_session_runner_persists_failure_artifacts_for_trace_mismatches() {
    let temp_dir = unique_temp_dir("failure-artifacts");
    let artifact_root = temp_dir.join("artifacts");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let left_rom = temp_dir.join("left.gb");
    let right_rom = temp_dir.join("right.gb");
    fs::write(
        &left_rom,
        build_single_shot_serial_from_address_rom(0xC000, 0x81),
    )
    .expect("left ROM should be writable");
    fs::write(
        &right_rom,
        build_single_shot_serial_from_address_rom(0xC100, 0x80),
    )
    .expect("right ROM should be writable");
    let fixture_path = temp_dir.join("wrong.trace");
    fs::write(&fixture_path, "definitely wrong\n").expect("fixture should be writable");

    let session = LinkedSessionCase::new(
        "trace-mismatch",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(5_000),
        LinkedSessionPassCondition::TraceFixture(fixture_path.clone()),
    )
    .with_failure_artifacts(
        LinkedSessionFailureArtifactPolicy::new()
            .with_artifact(LinkedSessionCaptureKind::Trace)
            .with_artifact(LinkedSessionCaptureKind::Snapshot),
    )
    .with_participant(
        LinkedSessionParticipant::new("left", &left_rom)
            .with_external_stimulus(ExternalStimulus::at_frame(
                0,
                ExternalStimulusAction::WriteMemory {
                    address: 0xC000,
                    value: 0x41,
                },
            ))
            .with_external_stimulus(ExternalStimulus::at_t_cycle(
                0,
                ExternalStimulusAction::WriteMemory {
                    address: 0xC100,
                    value: 0x5A,
                },
            )),
    )
    .with_participant(LinkedSessionParticipant::new("right", &right_rom));

    let report = LinkedSessionRunner::new()
        .with_failure_artifact_root(&artifact_root)
        .run_session(&session)
        .expect("mismatch session should execute");

    assert!(matches!(
        report.outcome,
        LinkedSessionCaseOutcome::Failed(LinkedSessionCaseFailure::FixtureMismatch { .. })
    ));
    let left_serial_hex = &report.participants[0].artifacts.serial_hex;
    assert_eq!(left_serial_hex, "41");
    assert!(
        artifact_root
            .join("trace-mismatch")
            .join("linked_trace.txt")
            .is_file()
    );
    assert!(
        artifact_root
            .join("trace-mismatch")
            .join("linked_snapshot.txt")
            .is_file()
    );
    assert!(
        artifact_root
            .join("trace-mismatch")
            .join("left_serial.txt")
            .is_file()
    );
    assert!(
        artifact_root
            .join("trace-mismatch")
            .join("left_serial_hex.txt")
            .is_file()
    );
    assert!(
        artifact_root
            .join("trace-mismatch")
            .join("left_snapshot.txt")
            .is_file()
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn linked_session_runner_reports_participant_snapshot_fixture_mismatches_per_participant() {
    let temp_dir = unique_temp_dir("participant-snapshot-mismatch");
    let artifact_root = temp_dir.join("artifacts");
    let expected_fixture_path = temp_dir.join("wrong.snapshot");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let left_rom = temp_dir.join("left.gb");
    let right_rom = temp_dir.join("right.gb");
    fs::write(
        &left_rom,
        build_test_rom(&[
            0x3E, 0xA5, 0xE0, 0x01, 0x3E, 0x81, 0xE0, 0x02, 0xC3, 0x08, 0x01,
        ]),
    )
    .expect("left ROM should be writable");
    fs::write(
        &right_rom,
        build_test_rom(&[
            0x3E, 0x3C, 0xE0, 0x01, 0x3E, 0x80, 0xE0, 0x02, 0xC3, 0x08, 0x01,
        ]),
    )
    .expect("right ROM should be writable");
    fs::write(&expected_fixture_path, "definitely wrong\n")
        .expect("wrong participant snapshot fixture should be writable");

    let session = LinkedSessionCase::new(
        "participant-snapshot-mismatch",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(5_000),
        LinkedSessionPassCondition::ParticipantSnapshotFixture {
            participant_id: "left".to_string(),
            fixture_path: expected_fixture_path.clone(),
        },
    )
    .with_failure_artifacts(
        LinkedSessionFailureArtifactPolicy::new().with_artifact(LinkedSessionCaptureKind::Snapshot),
    )
    .with_participant(LinkedSessionParticipant::new("left", &left_rom))
    .with_participant(LinkedSessionParticipant::new("right", &right_rom));

    let report = LinkedSessionRunner::new()
        .with_failure_artifact_root(&artifact_root)
        .run_session(&session)
        .expect("participant snapshot mismatch session should execute");

    assert!(matches!(
        report.outcome,
        LinkedSessionCaseOutcome::Failed(
            LinkedSessionCaseFailure::ParticipantFixtureMismatch {
                ref participant_id,
                capture: LinkedSessionCaptureKind::Snapshot,
                ref fixture_path,
            }
        ) if participant_id == "left" && fixture_path == &expected_fixture_path
    ));
    assert!(matches!(
        report.participants[0].outcome,
        LinkedSessionCaseOutcome::Failed(
            LinkedSessionCaseFailure::ParticipantFixtureMismatch {
                ref participant_id,
                capture: LinkedSessionCaptureKind::Snapshot,
                ..
            }
        ) if participant_id == "left"
    ));
    assert_eq!(
        report.participants[1].outcome,
        LinkedSessionCaseOutcome::Passed
    );
    assert!(
        artifact_root
            .join("participant-snapshot-mismatch")
            .join("linked_snapshot.txt")
            .is_file()
    );
    assert!(
        artifact_root
            .join("participant-snapshot-mismatch")
            .join("left_snapshot.txt")
            .is_file()
    );
    assert!(
        artifact_root
            .join("participant-snapshot-mismatch")
            .join("right_snapshot.txt")
            .is_file()
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn linked_session_runner_reports_participant_trace_fixture_mismatches_per_participant() {
    let temp_dir = unique_temp_dir("participant-trace-mismatch");
    let artifact_root = temp_dir.join("artifacts");
    let expected_fixture_path = temp_dir.join("wrong.trace");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let left_rom = temp_dir.join("left.gb");
    let right_rom = temp_dir.join("right.gb");
    fs::write(
        &left_rom,
        build_test_rom(&[
            0x3E, 0xA5, 0xE0, 0x01, 0x3E, 0x81, 0xE0, 0x02, 0xC3, 0x08, 0x01,
        ]),
    )
    .expect("left ROM should be writable");
    fs::write(
        &right_rom,
        build_test_rom(&[
            0x3E, 0x3C, 0xE0, 0x01, 0x3E, 0x80, 0xE0, 0x02, 0xC3, 0x08, 0x01,
        ]),
    )
    .expect("right ROM should be writable");
    fs::write(&expected_fixture_path, "definitely wrong\n")
        .expect("wrong participant trace fixture should be writable");

    let session = LinkedSessionCase::new(
        "participant-trace-mismatch",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(5_000),
        LinkedSessionPassCondition::ParticipantTraceFixture {
            participant_id: "left".to_string(),
            fixture_path: expected_fixture_path.clone(),
        },
    )
    .with_failure_artifacts(
        LinkedSessionFailureArtifactPolicy::new()
            .with_artifact(LinkedSessionCaptureKind::Trace)
            .with_artifact(LinkedSessionCaptureKind::Snapshot),
    )
    .with_participant(LinkedSessionParticipant::new("left", &left_rom))
    .with_participant(LinkedSessionParticipant::new("right", &right_rom));

    let report = LinkedSessionRunner::new()
        .with_failure_artifact_root(&artifact_root)
        .run_session(&session)
        .expect("participant trace mismatch session should execute");

    assert!(matches!(
        report.outcome,
        LinkedSessionCaseOutcome::Failed(
            LinkedSessionCaseFailure::ParticipantFixtureMismatch {
                ref participant_id,
                capture: LinkedSessionCaptureKind::Trace,
                ref fixture_path,
            }
        ) if participant_id == "left" && fixture_path == &expected_fixture_path
    ));
    assert!(matches!(
        report.participants[0].outcome,
        LinkedSessionCaseOutcome::Failed(
            LinkedSessionCaseFailure::ParticipantFixtureMismatch {
                ref participant_id,
                capture: LinkedSessionCaptureKind::Trace,
                ..
            }
        ) if participant_id == "left"
    ));
    assert_eq!(
        report.participants[1].outcome,
        LinkedSessionCaseOutcome::Passed
    );
    assert!(
        artifact_root
            .join("participant-trace-mismatch")
            .join("linked_trace.txt")
            .is_file()
    );
    assert!(
        artifact_root
            .join("participant-trace-mismatch")
            .join("left_trace.txt")
            .is_file()
    );
    assert!(
        artifact_root
            .join("participant-trace-mismatch")
            .join("right_trace.txt")
            .is_file()
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn linked_session_runner_reports_participant_serial_hex_mismatches_per_participant() {
    let temp_dir = unique_temp_dir("participant-serial-hex-mismatch");
    let artifact_root = temp_dir.join("artifacts");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let left_rom = temp_dir.join("left.gb");
    let right_rom = temp_dir.join("right.gb");
    fs::write(
        &left_rom,
        build_test_rom(&[
            0x3E, 0xA5, 0xE0, 0x01, 0x3E, 0x81, 0xE0, 0x02, 0xC3, 0x08, 0x01,
        ]),
    )
    .expect("left ROM should be writable");
    fs::write(
        &right_rom,
        build_test_rom(&[
            0x3E, 0x3C, 0xE0, 0x01, 0x3E, 0x80, 0xE0, 0x02, 0xC3, 0x08, 0x01,
        ]),
    )
    .expect("right ROM should be writable");

    let session = LinkedSessionCase::new(
        "participant-serial-hex-mismatch",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(5_000),
        LinkedSessionPassCondition::ParticipantSerialHexExact {
            participant_id: "left".to_string(),
            expected: "FF".to_string(),
        },
    )
    .with_failure_artifacts(
        LinkedSessionFailureArtifactPolicy::new()
            .with_artifact(LinkedSessionCaptureKind::ParticipantSerialHex)
            .with_artifact(LinkedSessionCaptureKind::Snapshot),
    )
    .with_participant(LinkedSessionParticipant::new("left", &left_rom))
    .with_participant(LinkedSessionParticipant::new("right", &right_rom));

    let report = LinkedSessionRunner::new()
        .with_failure_artifact_root(&artifact_root)
        .run_session(&session)
        .expect("participant serial hex mismatch session should execute");

    assert!(matches!(
        report.outcome,
        LinkedSessionCaseOutcome::Failed(
            LinkedSessionCaseFailure::ParticipantSerialHexMismatch {
                ref participant_id,
                ref expected,
                ref actual,
            }
        ) if participant_id == "left" && expected == "FF" && actual == "A5"
    ));
    assert!(matches!(
        report.participants[0].outcome,
        LinkedSessionCaseOutcome::Failed(
            LinkedSessionCaseFailure::ParticipantSerialHexMismatch {
                ref participant_id,
                ..
            }
        ) if participant_id == "left"
    ));
    assert_eq!(
        report.participants[1].outcome,
        LinkedSessionCaseOutcome::Passed
    );
    assert!(
        artifact_root
            .join("participant-serial-hex-mismatch")
            .join("left_serial_hex.txt")
            .is_file()
    );
    assert!(
        artifact_root
            .join("participant-serial-hex-mismatch")
            .join("right_serial_hex.txt")
            .is_file()
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn linked_session_runner_reports_cpu_diagnostic_traps_per_participant() {
    let temp_dir = unique_temp_dir("diagnostic-trap");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let left_rom = temp_dir.join("left.gb");
    let right_rom = temp_dir.join("right.gb");
    fs::write(&left_rom, build_test_rom(&[0xD3, 0x00, 0x01])).expect("left ROM should be writable");
    fs::write(
        &right_rom,
        build_single_shot_serial_from_address_rom(0xC100, 0x80),
    )
    .expect("right ROM should be writable");

    let session = LinkedSessionCase::new(
        "diagnostic-trap",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(64),
        LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Snapshot),
    )
    .with_participant(LinkedSessionParticipant::new("left", &left_rom))
    .with_participant(LinkedSessionParticipant::new("right", &right_rom));

    let report = LinkedSessionRunner::new()
        .run_session(&session)
        .expect("diagnostic session should execute");

    assert!(matches!(
        report.outcome,
        LinkedSessionCaseOutcome::Failed(LinkedSessionCaseFailure::CpuDiagnosticTrap {
            participant_id,
            ..
        }) if participant_id == "left"
    ));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn linked_session_runner_executes_suites() {
    let temp_dir = unique_temp_dir("suite");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let left_rom = temp_dir.join("left.gb");
    let right_rom = temp_dir.join("right.gb");
    fs::write(
        &left_rom,
        build_single_shot_serial_from_address_rom(0xC000, 0x81),
    )
    .expect("left ROM should be writable");
    fs::write(
        &right_rom,
        build_single_shot_serial_from_address_rom(0xC100, 0x80),
    )
    .expect("right ROM should be writable");

    let session = LinkedSessionCase::new(
        "suite-session",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(5_000),
        LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Snapshot),
    )
    .with_participant(
        LinkedSessionParticipant::new("left", &left_rom).with_external_stimulus(
            ExternalStimulus::at_t_cycle(
                0,
                ExternalStimulusAction::WriteMemory {
                    address: 0xC000,
                    value: 0xAA,
                },
            ),
        ),
    )
    .with_participant(
        LinkedSessionParticipant::new("right", &right_rom).with_external_stimulus(
            ExternalStimulus::at_t_cycle(
                0,
                ExternalStimulusAction::WriteMemory {
                    address: 0xC100,
                    value: 0x55,
                },
            ),
        ),
    );

    let suite =
        LinkedSessionSuite::new("linked-suite", crate::TestSubsystem::Serial).with_session(session);
    let report = LinkedSessionRunner::new()
        .run_suite(&suite)
        .expect("linked suite should execute");

    assert!(report.all_non_failing());
    assert_eq!(report.sessions.len(), 1);

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}
