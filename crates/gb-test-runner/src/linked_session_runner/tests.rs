use super::*;
use crate::{
    BootRomVerificationMode, ExternalStimulus, ExternalStimulusAction, LinkedSessionCapturePlan,
    LinkedSessionCase, LinkedSessionFailureArtifactPolicy, LinkedSessionParticipant,
    LinkedSessionPassCondition, LinkedSessionSuite, LinkedSessionTopology,
    external_rom_source_manifest_path,
};
use gb_core::{Dmg07Port, JoypadButton};
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

fn data_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
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

fn build_repeating_serial_table_rom(sc_value: u8, response_table: &[u8]) -> Vec<u8> {
    let mut program = vec![
        0x21, 0x14, 0x01, 0x7E, 0xE0, 0x01, 0x3E, sc_value, 0xE0, 0x02, 0xF0, 0x02, 0xCB, 0x7F,
        0x20, 0xFA, 0x23, 0xC3, 0x03, 0x01,
    ];
    program.extend_from_slice(response_table);
    build_test_rom(&program)
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
fn linked_session_runner_executes_a_sparse_dmg07_session_and_captures_adapter_trace() {
    let session = LinkedSessionCase::new(
        "dmg07-sparse",
        LinkedSessionTopology::Dmg07,
        Timeout::TCycles(500_000),
        LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Trace),
    )
    .with_participant(
        LinkedSessionParticipant::new("p1", data_path("data/fixtures/linked/dmg07/p1-basic.gb"))
            .with_adapter_port(Dmg07Port::P1),
    )
    .with_participant(
        LinkedSessionParticipant::new("p4", data_path("data/fixtures/linked/dmg07/p4-basic.gb"))
            .with_adapter_port(Dmg07Port::P4),
    );

    let report = LinkedSessionRunner::new()
        .run_session(&session)
        .expect("dmg07 linked session should execute");

    assert_eq!(report.outcome, LinkedSessionCaseOutcome::Informational);
    assert_eq!(report.participants.len(), 2);
    assert!(
        report.participants[0]
            .artifacts
            .serial_hex
            .starts_with("88880001AAAAAA00")
    );
    assert!(
        report.participants[1]
            .artifacts
            .serial_hex
            .starts_with("8888000100000000")
    );
    let topology_trace = report
        .artifacts
        .topology_trace_text
        .as_deref()
        .expect("dmg07 adapter should emit topology trace");
    assert!(topology_trace.contains("transition=transmission_indicator"));
    assert!(topology_trace.contains("transition=transmission"));
    assert!(topology_trace.contains("transition=ping_restart_indicator"));
    let combined_trace = report
        .artifacts
        .trace
        .as_deref()
        .expect("combined trace should be captured");
    assert!(combined_trace.contains("== link topology trace =="));
}

#[test]
fn linked_session_runner_does_not_treat_dmg07_internal_clock_as_valid_adapter_input() {
    let temp_dir = unique_temp_dir("dmg07-internal-clock");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let p1_rom = temp_dir.join("p1-internal.gb");
    fs::write(
        &p1_rom,
        build_repeating_serial_table_rom(
            0x81,
            &[
                0x88, 0x88, 0x00, 0x01, 0xAA, 0xAA, 0xAA, 0x00, 0x00, 0x00, 0x00, 0x00,
            ],
        ),
    )
    .expect("p1 ROM should be writable");

    let session = LinkedSessionCase::new(
        "dmg07-internal-clock",
        LinkedSessionTopology::Dmg07,
        Timeout::TCycles(260_000),
        LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Trace),
    )
    .with_participant(LinkedSessionParticipant::new("p1", &p1_rom).with_adapter_port(Dmg07Port::P1))
    .with_participant(
        LinkedSessionParticipant::new("p4", data_path("data/fixtures/linked/dmg07/p4-basic.gb"))
            .with_adapter_port(Dmg07Port::P4),
    );

    let report = LinkedSessionRunner::new()
        .run_session(&session)
        .expect("dmg07 linked session should execute");

    assert_eq!(report.outcome, LinkedSessionCaseOutcome::Informational);
    assert!(
        report.artifacts.topology_trace_text.is_none(),
        "internal-clock P1 bytes must not drive the active adapter protocol"
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn linked_session_runner_executes_dmg07_three_and_four_player_sessions_deterministically() {
    let build_session = |id: &str, ports: &[(&str, &str, Dmg07Port)]| {
        let mut session = LinkedSessionCase::new(
            id,
            LinkedSessionTopology::Dmg07,
            Timeout::TCycles(180_000),
            LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Trace),
        );
        for (participant_id, fixture, port) in ports {
            session = session.with_participant(
                LinkedSessionParticipant::new(
                    *participant_id,
                    data_path(&format!("data/fixtures/linked/dmg07/{fixture}")),
                )
                .with_adapter_port(*port),
            );
        }
        session
    };

    let three_player = build_session(
        "dmg07-three-player",
        &[
            ("p1", "p1-basic.gb", Dmg07Port::P1),
            ("p2", "p2-basic.gb", Dmg07Port::P2),
            ("p4", "p4-basic.gb", Dmg07Port::P4),
        ],
    );
    let four_player = build_session(
        "dmg07-four-player",
        &[
            ("p1", "p1-basic.gb", Dmg07Port::P1),
            ("p2", "p2-basic.gb", Dmg07Port::P2),
            ("p3", "p3-basic.gb", Dmg07Port::P3),
            ("p4", "p4-basic.gb", Dmg07Port::P4),
        ],
    );

    for session in [three_player, four_player] {
        let first = LinkedSessionRunner::new()
            .run_session(&session)
            .expect("first dmg07 run should execute");
        let second = LinkedSessionRunner::new()
            .run_session(&session)
            .expect("second dmg07 run should execute");

        assert_eq!(first.outcome, LinkedSessionCaseOutcome::Informational);
        assert_eq!(first.artifacts.trace, second.artifacts.trace);
        assert_eq!(
            first
                .participants
                .iter()
                .map(|participant| participant.artifacts.serial_hex.as_str())
                .collect::<Vec<_>>(),
            second
                .participants
                .iter()
                .map(|participant| participant.artifacts.serial_hex.as_str())
                .collect::<Vec<_>>()
        );
    }
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

#[test]
fn linked_session_runner_surfaces_missing_fixture_reads_as_typed_file_operations() {
    let temp_dir = unique_temp_dir("missing-fixtures");
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

    let missing_participant_snapshot = temp_dir.join("missing-left.snapshot");
    let participant_snapshot_session = LinkedSessionCase::new(
        "missing-participant-snapshot",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(5_000),
        LinkedSessionPassCondition::ParticipantSnapshotFixture {
            participant_id: "left".to_string(),
            fixture_path: missing_participant_snapshot.clone(),
        },
    )
    .with_participant(LinkedSessionParticipant::new("left", &left_rom))
    .with_participant(LinkedSessionParticipant::new("right", &right_rom));
    let snapshot_error = LinkedSessionRunner::new()
        .run_session(&participant_snapshot_session)
        .expect_err("missing participant snapshot fixture should fail");
    assert!(matches!(
        snapshot_error,
        LinkedSessionExecutionError::FileOperation {
            ref path,
            operation: "read participant snapshot fixture",
            ..
        } if path == &missing_participant_snapshot
    ));

    let missing_participant_trace = temp_dir.join("missing-left.trace");
    let participant_trace_session = LinkedSessionCase::new(
        "missing-participant-trace",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(5_000),
        LinkedSessionPassCondition::ParticipantTraceFixture {
            participant_id: "left".to_string(),
            fixture_path: missing_participant_trace.clone(),
        },
    )
    .with_participant(LinkedSessionParticipant::new("left", &left_rom))
    .with_participant(LinkedSessionParticipant::new("right", &right_rom));
    let trace_error = LinkedSessionRunner::new()
        .run_session(&participant_trace_session)
        .expect_err("missing participant trace fixture should fail");
    assert!(matches!(
        trace_error,
        LinkedSessionExecutionError::FileOperation {
            ref path,
            operation: "read participant trace fixture",
            ..
        } if path == &missing_participant_trace
    ));

    let missing_linked_trace = temp_dir.join("missing-linked.trace");
    let linked_trace_session = LinkedSessionCase::new(
        "missing-linked-trace",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(5_000),
        LinkedSessionPassCondition::TraceFixture(missing_linked_trace.clone()),
    )
    .with_participant(LinkedSessionParticipant::new("left", &left_rom))
    .with_participant(LinkedSessionParticipant::new("right", &right_rom));
    let linked_trace_error = LinkedSessionRunner::new()
        .run_session(&linked_trace_session)
        .expect_err("missing linked trace fixture should fail");
    assert!(matches!(
        linked_trace_error,
        LinkedSessionExecutionError::FileOperation {
            ref path,
            operation: "read linked trace fixture",
            ..
        } if path == &missing_linked_trace
    ));

    let missing_linked_snapshot = temp_dir.join("missing-linked.snapshot");
    let linked_snapshot_session = LinkedSessionCase::new(
        "missing-linked-snapshot",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(5_000),
        LinkedSessionPassCondition::SnapshotFixture(missing_linked_snapshot.clone()),
    )
    .with_participant(LinkedSessionParticipant::new("left", &left_rom))
    .with_participant(LinkedSessionParticipant::new("right", &right_rom));
    let linked_snapshot_error = LinkedSessionRunner::new()
        .run_session(&linked_snapshot_session)
        .expect_err("missing linked snapshot fixture should fail");
    assert!(matches!(
        linked_snapshot_error,
        LinkedSessionExecutionError::FileOperation {
            ref path,
            operation: "read linked snapshot fixture",
            ..
        } if path == &missing_linked_snapshot
    ));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn linked_session_runner_surfaces_loader_and_manifest_resolution_failures() {
    let temp_dir = unique_temp_dir("loader-errors");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let manifest_path = external_rom_source_manifest_path(&temp_dir);
    fs::create_dir_all(
        manifest_path
            .parent()
            .expect("external manifest path should have a parent"),
    )
    .expect("external manifest parent should be creatable");
    fs::write(&manifest_path, "version = 1\n[[source]]\nid = \"broken\"\n")
        .expect("broken external ROM manifest should be writable");

    let manifest_error_session = LinkedSessionCase::new(
        "manifest-error",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(64),
        LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Snapshot),
    )
    .with_participant(
        LinkedSessionParticipant::new("left", "retrio/case.gb")
            .with_external_rom_root_key("GB_CYCLE_LIB_TEST_EXTERNAL_ROOT"),
    )
    .with_participant(LinkedSessionParticipant::new("right", "right.gb"));
    let manifest_error = LinkedSessionRunner::new()
        .with_workspace_root(&temp_dir)
        .run_session(&manifest_error_session)
        .expect_err("broken external ROM manifest should surface as a typed error");
    assert!(matches!(
        manifest_error,
        LinkedSessionExecutionError::ExternalRomSourceManifest { .. }
    ));

    let left_rom = temp_dir.join("left.gb");
    fs::write(
        &left_rom,
        build_single_shot_serial_from_address_rom(0xC000, 0x81),
    )
    .expect("left ROM should be writable");
    let missing_rom_session = LinkedSessionCase::new(
        "missing-rom",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(64),
        LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Snapshot),
    )
    .with_participant(LinkedSessionParticipant::new("left", &left_rom))
    .with_participant(LinkedSessionParticipant::new(
        "right",
        temp_dir.join("missing.gb"),
    ));
    let missing_rom_error = LinkedSessionRunner::new()
        .run_session(&missing_rom_session)
        .expect_err("missing ROM should fail during participant loading");
    assert!(matches!(
        missing_rom_error,
        LinkedSessionExecutionError::FileOperation {
            ref path,
            operation: "read ROM",
            ..
        } if path.ends_with("missing.gb")
    ));

    let truncated_rom = temp_dir.join("truncated.gb");
    fs::write(&truncated_rom, [0x00_u8]).expect("truncated ROM should be writable");
    let cartridge_error_session = LinkedSessionCase::new(
        "cartridge-error",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(64),
        LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Snapshot),
    )
    .with_participant(LinkedSessionParticipant::new("left", &left_rom))
    .with_participant(LinkedSessionParticipant::new("right", &truncated_rom));
    let cartridge_error = LinkedSessionRunner::new()
        .run_session(&cartridge_error_session)
        .expect_err("invalid ROM images should surface as cartridge load failures");
    assert!(matches!(
        cartridge_error,
        LinkedSessionExecutionError::CartridgeLoad {
            ref participant_id,
            ref path,
            ..
        } if participant_id == "right" && path == &truncated_rom
    ));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn linked_session_runner_real_boot_helpers_cover_missing_roots_and_stimulus_timing() {
    let temp_dir = unique_temp_dir("real-boot-and-stimuli");
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
        "real-boot-and-stimuli",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(5_000),
        LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Snapshot),
    )
    .with_participant(
        LinkedSessionParticipant::new("left", &left_rom)
            .with_startup_mode(gb_core::StartupMode::RealBoot)
            .with_external_stimulus(ExternalStimulus::at_t_cycle(
                0,
                ExternalStimulusAction::WriteMemory {
                    address: 0xC000,
                    value: 0xA5,
                },
            ))
            .with_external_stimulus(ExternalStimulus::at_frame(
                1,
                ExternalStimulusAction::JoypadSetButton {
                    button: JoypadButton::Start,
                    pressed: true,
                },
            )),
    )
    .with_participant(LinkedSessionParticipant::new("right", &right_rom));

    let runner = LinkedSessionRunner::new()
        .with_boot_rom_root(temp_dir.join("missing-bootrom"))
        .with_boot_rom_verification_mode(BootRomVerificationMode::Off);
    let (mut linked, _, _) = runner
        .build_summary_linked_machines(&session)
        .expect("missing boot ROM roots should fall back to no assets when verification is off");

    let mut applied_stimuli = session
        .participants
        .iter()
        .map(|participant| vec![false; participant.external_stimuli.stimuli().len()])
        .collect::<Vec<_>>();

    runner.apply_scheduled_stimuli(&session, &mut linked, &[0, 0], &mut applied_stimuli);
    match &mut linked {
        RunnerLinkedMachines::Summary(linked) => {
            let left = linked
                .machine_mut(0)
                .expect("left participant should exist");
            assert_eq!(left.read_bus(0xC000), 0xA5);
            assert_eq!(left.joypad().snapshot().pressed_mask, 0x00);
        }
        RunnerLinkedMachines::Buffered(_) => panic!("summary build should use summary traces"),
    }
    assert_eq!(applied_stimuli[0], vec![true, false]);

    runner.apply_scheduled_stimuli(&session, &mut linked, &[1, 0], &mut applied_stimuli);
    linked.step_t_cycle();
    match &linked {
        RunnerLinkedMachines::Summary(linked) => {
            let left = linked.machine(0).expect("left participant should exist");
            assert_eq!(left.joypad().snapshot().pressed_mask, 0x80);
        }
        RunnerLinkedMachines::Buffered(_) => panic!("summary build should use summary traces"),
    }
    assert_eq!(applied_stimuli[0], vec![true, true]);

    let strict_runner = LinkedSessionRunner::new()
        .with_boot_rom_root(temp_dir.join("still-missing-bootrom"))
        .with_boot_rom_verification_mode(BootRomVerificationMode::Strict);
    let strict_error = match strict_runner.build_summary_linked_machines(&session) {
        Ok(_) => panic!("strict verification should reject missing real-boot assets"),
        Err(error) => error,
    };
    assert!(matches!(
        strict_error,
        LinkedSessionExecutionError::BootRomVerification { .. }
    ));

    let cgb_real_boot_session = LinkedSessionCase::new(
        "cgb-real-boot",
        LinkedSessionTopology::Dmg04,
        Timeout::TCycles(32),
        LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Snapshot),
    )
    .with_participant(
        LinkedSessionParticipant::new("left", &left_rom)
            .with_console_model(gb_core::ConsoleModel::Cgb)
            .with_startup_mode(gb_core::StartupMode::RealBoot),
    )
    .with_participant(LinkedSessionParticipant::new("right", &right_rom));
    LinkedSessionRunner::new()
        .build_summary_linked_machines(&cgb_real_boot_session)
        .expect("CGB real-boot participants should keep the DMG-only linked harness permissive");

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn linked_session_report_helpers_distinguish_passed_and_non_failing_states() {
    let passed = LinkedSessionCaseOutcome::Passed;
    let informational = LinkedSessionCaseOutcome::Informational;
    let failed = LinkedSessionCaseOutcome::Failed(LinkedSessionCaseFailure::FixtureMismatch {
        fixture_path: PathBuf::from("wrong.trace"),
    });

    assert!(!passed.failed());
    assert!(passed.non_failing());
    assert!(!informational.failed());
    assert!(informational.non_failing());
    assert!(failed.failed());
    assert!(!failed.non_failing());

    let passing_report = LinkedSessionCaseReport {
        session_id: "passing".to_string(),
        outcome: passed.clone(),
        executed_t_cycles: 0,
        participants: Vec::new(),
        artifacts: LinkedSessionCapturedArtifacts::default(),
        retained_failure_artifacts: Vec::new(),
    };
    let informational_report = LinkedSessionCaseReport {
        session_id: "info".to_string(),
        outcome: informational,
        executed_t_cycles: 0,
        participants: Vec::new(),
        artifacts: LinkedSessionCapturedArtifacts::default(),
        retained_failure_artifacts: Vec::new(),
    };
    let failing_report = LinkedSessionCaseReport {
        session_id: "failing".to_string(),
        outcome: failed,
        executed_t_cycles: 0,
        participants: Vec::new(),
        artifacts: LinkedSessionCapturedArtifacts::default(),
        retained_failure_artifacts: Vec::new(),
    };

    assert!(passing_report.passed());
    assert!(passing_report.non_failing());
    assert!(!informational_report.passed());
    assert!(informational_report.non_failing());
    assert!(!failing_report.passed());
    assert!(!failing_report.non_failing());

    let passing_suite = LinkedSessionSuiteReport {
        suite_name: "passing".to_string(),
        family: None,
        subsystem: crate::TestSubsystem::Serial,
        sessions: vec![passing_report],
    };
    assert!(passing_suite.all_passed());
    assert!(passing_suite.all_non_failing());

    let mixed_suite = LinkedSessionSuiteReport {
        suite_name: "mixed".to_string(),
        family: Some("serial-ext".to_string()),
        subsystem: crate::TestSubsystem::Serial,
        sessions: vec![informational_report, failing_report],
    };
    assert!(!mixed_suite.all_passed());
    assert!(!mixed_suite.all_non_failing());
}
