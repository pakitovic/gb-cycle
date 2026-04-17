use super::parse::{capture_plan_for_pass_condition, failure_artifacts_for_pass_condition};
use super::{
    LinkedSessionCaptureKind, LinkedSessionPassCondition, LinkedSessionSuiteManifestError,
    LinkedSessionTopology, load_linked_session_suite_manifest,
};
use crate::{ExternalStimulusAction, StimulusTime, TestSubsystem};
use gb_core::{ConsoleModel, ExecutionMode, JoypadButton, StartupMode};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "gb-cycle-linked-session-suite-manifest-{}-{}-{}",
        label,
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}

fn write_manifest(dir: &Path, name: &str, body: &str) -> PathBuf {
    let manifest_path = dir.join(name);
    fs::create_dir_all(dir).expect("manifest parent should be creatable");
    fs::write(&manifest_path, body).expect("manifest should be writable");
    manifest_path
}

#[test]
fn linked_session_manifest_defaults_to_trace_info_and_resolves_relative_rom_paths() {
    let workspace = unique_temp_dir("defaults");
    let left_rom = workspace.join("roms").join("left.gb");
    let right_rom = workspace.join("roms").join("right.gb");
    fs::create_dir_all(
        left_rom
            .parent()
            .expect("left ROM path should have a parent"),
    )
    .expect("temporary ROM parent should be creatable");
    fs::write(&left_rom, [0x00_u8]).expect("left rom should be writable");
    fs::write(&right_rom, [0x00_u8]).expect("right rom should be writable");

    let manifest_path = write_manifest(
        &workspace,
        "dmg04-defaults.toml",
        r#"
version = 1

[[session]]
id = "basic-exchange"
timeout_tcycles = 8192

  [[session.participant]]
  id = "left"
  rom = "roms/left.gb"

    [[session.participant.stimulus]]
    tcycle = 128
    button = "a"
    pressed = true

  [[session.participant]]
  id = "right"
  rom = "roms/right.gb"

    [[session.participant.stimulus]]
    frame = 2
    button = "b"
    pressed = false
"#,
    );

    let suite = load_linked_session_suite_manifest(&manifest_path)
        .expect("linked manifest should load cleanly");

    assert_eq!(suite.name, "dmg04-defaults");
    assert_eq!(suite.subsystem, TestSubsystem::CrossSubsystem);
    assert_eq!(suite.sessions.len(), 1);

    let session = &suite.sessions[0];
    assert_eq!(session.id, "basic-exchange");
    assert_eq!(session.topology, LinkedSessionTopology::Dmg04);
    assert_eq!(session.timeout, crate::Timeout::TCycles(8192));
    assert_eq!(
        session.pass_condition,
        LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Trace)
    );
    assert!(
        session
            .capture_plan
            .contains(LinkedSessionCaptureKind::Trace)
    );
    assert!(
        session
            .capture_plan
            .contains(LinkedSessionCaptureKind::Snapshot)
    );
    assert_eq!(session.participants.len(), 2);

    let left = &session.participants[0];
    assert_eq!(left.id, "left");
    assert_eq!(left.rom_path, left_rom);
    assert_eq!(left.console_model, ConsoleModel::Dmg);
    assert_eq!(left.startup_mode, StartupMode::SkipBoot);
    assert_eq!(left.execution_mode, ExecutionMode::Strict);
    assert_eq!(left.external_stimuli.stimuli().len(), 1);
    assert_eq!(
        left.external_stimuli.stimuli()[0].when,
        StimulusTime::TCycle(128)
    );
    assert_eq!(
        left.external_stimuli.stimuli()[0].action,
        ExternalStimulusAction::JoypadSetButton {
            button: JoypadButton::A,
            pressed: true,
        }
    );

    let right = &session.participants[1];
    assert_eq!(right.id, "right");
    assert_eq!(right.rom_path, right_rom);
    assert_eq!(
        right.external_stimuli.stimuli()[0].when,
        StimulusTime::Frame(2)
    );
    assert_eq!(
        right.external_stimuli.stimuli()[0].action,
        ExternalStimulusAction::JoypadSetButton {
            button: JoypadButton::B,
            pressed: false,
        }
    );
}

#[test]
fn linked_session_manifest_supports_explicit_metadata_and_trace_fixture_oracles() {
    let workspace = unique_temp_dir("explicit-contract");
    let absolute_fixture = workspace.join("fixtures").join("linked.trace");
    let absolute_snapshot_fixture = workspace.join("fixtures").join("snapshot.txt");
    fs::create_dir_all(
        absolute_fixture
            .parent()
            .expect("fixture path should have a parent"),
    )
    .expect("fixture parent should be creatable");
    fs::write(&absolute_fixture, []).expect("absolute fixture should be writable");
    fs::write(&absolute_snapshot_fixture, [])
        .expect("absolute snapshot fixture should be writable");

    let manifest_path = write_manifest(
        &workspace,
        "linked-commercial-smoke.toml",
        &r#"
version = 1
suite_name = "linked-commercial-smoke"
family = "serial-ext"
subsystem = "serial"

[[session]]
id = "pokemon-trade"
topology = "dmg04"
timeout_frames = 90
oracle = "linked-trace-fixture"
fixture = "fixtures/pokemon.trace"

  [[session.participant]]
  id = "left"
  rom = "commercial/pokemon-red.gb"
  external_rom_root_key = "GB_CYCLE_LOCAL_COMMERCIAL_ROOT"
  console = "mgb"
  startup = "real-boot"
  mode = "permissive"

    [[session.participant.stimulus]]
    tcycle = 512
    button = "start"
    pressed = true

  [[session.participant]]
  id = "right"
  rom = "commercial/pokemon-blue.gb"
  console = "cgb"
  mode = "experimental"

[[session]]
id = "info-snapshot"
topology = "dmg04"
timeout_tcycles = 1024
oracle = "linked-snapshot-fixture"
fixture = "fixtures/snapshot.txt"

  [[session.participant]]
  id = "left2"
  rom = "commercial/left.gb"

  [[session.participant]]
  id = "right2"
  rom = "commercial/right.gb"
"#
        .replace(
            "fixtures/pokemon.trace",
            &absolute_fixture.display().to_string(),
        )
        .replace(
            "fixtures/snapshot.txt",
            &absolute_snapshot_fixture.display().to_string(),
        ),
    );

    let suite = load_linked_session_suite_manifest(&manifest_path)
        .expect("linked manifest should load cleanly");

    assert_eq!(suite.name, "linked-commercial-smoke");
    assert_eq!(suite.family.as_deref(), Some("serial-ext"));
    assert_eq!(suite.subsystem, TestSubsystem::Serial);
    assert_eq!(suite.sessions.len(), 2);

    let trace_session = &suite.sessions[0];
    assert_eq!(trace_session.timeout, crate::Timeout::Frames(90));
    assert_eq!(
        trace_session.pass_condition,
        LinkedSessionPassCondition::TraceFixture(absolute_fixture.clone())
    );
    assert!(
        trace_session
            .capture_plan
            .contains(LinkedSessionCaptureKind::Trace)
    );

    let left = &trace_session.participants[0];
    assert_eq!(left.console_model, ConsoleModel::Mgb);
    assert_eq!(left.startup_mode, StartupMode::RealBoot);
    assert_eq!(left.execution_mode, ExecutionMode::Permissive);
    assert_eq!(
        left.external_rom_root_key.as_deref(),
        Some("GB_CYCLE_LOCAL_COMMERCIAL_ROOT")
    );
    assert_eq!(
        left.external_stimuli.stimuli()[0].action,
        ExternalStimulusAction::JoypadSetButton {
            button: JoypadButton::Start,
            pressed: true,
        }
    );

    let right = &trace_session.participants[1];
    assert_eq!(right.console_model, ConsoleModel::Cgb);
    assert_eq!(right.execution_mode, ExecutionMode::Experimental);

    let info_session = &suite.sessions[1];
    assert_eq!(info_session.timeout, crate::Timeout::TCycles(1024));
    assert_eq!(
        info_session.pass_condition,
        LinkedSessionPassCondition::SnapshotFixture(absolute_snapshot_fixture)
    );
    assert!(
        info_session
            .capture_plan
            .contains(LinkedSessionCaptureKind::Snapshot)
    );
}

#[test]
fn linked_session_manifest_supports_participant_scoped_serial_hex_oracles() {
    let workspace = unique_temp_dir("participant-serial-hex");
    let manifest_path = write_manifest(
        &workspace,
        "participant-serial-hex.toml",
        r#"
version = 1
suite_name = "participant-serial-hex"
subsystem = "serial"

[[session]]
id = "dmg04-byte-expectation"
topology = "dmg04"
timeout_tcycles = 2048
oracle = "linked-participant-serial-hex-exact"
target_participant = "left"
expected = "A5"

  [[session.participant]]
  id = "left"
  rom = "left.gb"

  [[session.participant]]
  id = "right"
  rom = "right.gb"
"#,
    );

    let suite = load_linked_session_suite_manifest(&manifest_path)
        .expect("participant serial-hex manifest should load cleanly");
    let session = &suite.sessions[0];
    assert_eq!(
        session.pass_condition,
        LinkedSessionPassCondition::ParticipantSerialHexExact {
            participant_id: "left".to_string(),
            expected: "A5".to_string(),
        }
    );
    assert!(
        session
            .capture_plan
            .contains(LinkedSessionCaptureKind::ParticipantSerialHex)
    );
    assert!(
        session
            .failure_artifacts
            .contains(LinkedSessionCaptureKind::ParticipantSerialHex)
    );
}

#[test]
fn linked_session_manifest_supports_participant_scoped_snapshot_fixture_oracles() {
    let workspace = unique_temp_dir("participant-snapshot-fixture");
    fs::create_dir_all(&workspace).expect("workspace should be creatable");
    let fixture_path = workspace.join("left.snapshot");
    fs::write(&fixture_path, "fixture snapshot\n").expect("fixture should be writable");
    let manifest_path = write_manifest(
        &workspace,
        "participant-snapshot-fixture.toml",
        &format!(
            r#"
version = 1
suite_name = "participant-snapshot-fixture"
subsystem = "serial"

[[session]]
id = "dmg04-snapshot-expectation"
topology = "dmg04"
timeout_tcycles = 2048
oracle = "linked-participant-snapshot-fixture"
target_participant = "right"
fixture = "{}"

  [[session.participant]]
  id = "left"
  rom = "left.gb"

  [[session.participant]]
  id = "right"
  rom = "right.gb"
"#,
            fixture_path.display()
        ),
    );

    let suite = load_linked_session_suite_manifest(&manifest_path)
        .expect("participant snapshot fixture manifest should load cleanly");
    let session = &suite.sessions[0];
    assert_eq!(
        session.pass_condition,
        LinkedSessionPassCondition::ParticipantSnapshotFixture {
            participant_id: "right".to_string(),
            fixture_path,
        }
    );
    assert!(
        session
            .capture_plan
            .contains(LinkedSessionCaptureKind::Snapshot)
    );
    assert!(
        session
            .failure_artifacts
            .contains(LinkedSessionCaptureKind::Snapshot)
    );
}

#[test]
fn linked_session_manifest_supports_participant_scoped_trace_fixture_oracles() {
    let workspace = unique_temp_dir("participant-trace-fixture");
    fs::create_dir_all(&workspace).expect("workspace should be creatable");
    let fixture_path = workspace.join("left.trace");
    fs::write(&fixture_path, "fixture trace\n").expect("fixture should be writable");
    let manifest_path = write_manifest(
        &workspace,
        "participant-trace-fixture.toml",
        &format!(
            r#"
version = 1
suite_name = "participant-trace-fixture"
subsystem = "serial"

[[session]]
id = "dmg04-trace-expectation"
topology = "dmg04"
timeout_tcycles = 2048
oracle = "linked-participant-trace-fixture"
target_participant = "left"
fixture = "{}"

  [[session.participant]]
  id = "left"
  rom = "left.gb"

  [[session.participant]]
  id = "right"
  rom = "right.gb"
"#,
            fixture_path.display()
        ),
    );

    let suite = load_linked_session_suite_manifest(&manifest_path)
        .expect("participant trace fixture manifest should load cleanly");
    let session = &suite.sessions[0];
    assert_eq!(
        session.pass_condition,
        LinkedSessionPassCondition::ParticipantTraceFixture {
            participant_id: "left".to_string(),
            fixture_path,
        }
    );
    assert!(
        session
            .capture_plan
            .contains(LinkedSessionCaptureKind::Trace)
    );
    assert!(
        session
            .failure_artifacts
            .contains(LinkedSessionCaptureKind::Trace)
    );
    assert!(
        session
            .capture_plan
            .contains(LinkedSessionCaptureKind::Snapshot)
    );
}

#[test]
fn linked_session_manifest_rejects_invalid_timeout_topology_and_participant_count() {
    let workspace = unique_temp_dir("invalid-timeout-topology");

    let bad_timeout = write_manifest(
        &workspace,
        "bad-timeout.toml",
        r#"
version = 1

[[session]]
id = "broken"
timeout_frames = 1
timeout_tcycles = 2

  [[session.participant]]
  id = "left"
  rom = "left.gb"

  [[session.participant]]
  id = "right"
  rom = "right.gb"
"#,
    );
    let bad_timeout_error = load_linked_session_suite_manifest(&bad_timeout)
        .expect_err("invalid linked timeout should fail");
    match bad_timeout_error {
        LinkedSessionSuiteManifestError::Build { message, .. } => {
            assert!(message.contains("cannot specify both timeout_frames and timeout_tcycles"));
        }
        other => panic!("unexpected linked manifest error: {other:?}"),
    }

    let unsupported_topology = write_manifest(
        &workspace,
        "unsupported-topology.toml",
        r#"
version = 1

[[session]]
id = "broken"
topology = "dmg07"
timeout_frames = 1
oracle = "info-linked-trace"

  [[session.participant]]
  id = "left"
  rom = "left.gb"

  [[session.participant]]
  id = "right"
  rom = "right.gb"
"#,
    );
    let unsupported_topology_error = load_linked_session_suite_manifest(&unsupported_topology)
        .expect_err("unsupported topology should fail");
    match unsupported_topology_error {
        LinkedSessionSuiteManifestError::Build { message, .. } => {
            assert!(message.contains("unsupported topology"));
        }
        other => panic!("unexpected linked manifest error: {other:?}"),
    }

    let bad_participant_count = write_manifest(
        &workspace,
        "bad-participant-count.toml",
        r#"
version = 1

[[session]]
id = "broken"
timeout_frames = 1
oracle = "info-linked-trace"

  [[session.participant]]
  id = "solo"
  rom = "solo.gb"
"#,
    );
    let bad_participant_count_error = load_linked_session_suite_manifest(&bad_participant_count)
        .expect_err("dmg04 should require exactly two participants");
    match bad_participant_count_error {
        LinkedSessionSuiteManifestError::Build { message, .. } => {
            assert!(message.contains("UnsupportedTopologyParticipantCount"));
        }
        other => panic!("unexpected linked manifest error: {other:?}"),
    }
}

#[test]
fn linked_session_manifest_rejects_duplicate_ids_and_invalid_participant_metadata() {
    let workspace = unique_temp_dir("duplicate-ids");

    let duplicate_session_ids = write_manifest(
        &workspace,
        "duplicate-sessions.toml",
        r#"
version = 1

[[session]]
id = "duplicate"
timeout_frames = 1

  [[session.participant]]
  id = "left"
  rom = "left.gb"

  [[session.participant]]
  id = "right"
  rom = "right.gb"

[[session]]
id = "duplicate"
timeout_frames = 1

  [[session.participant]]
  id = "left2"
  rom = "left2.gb"

  [[session.participant]]
  id = "right2"
  rom = "right2.gb"
"#,
    );
    let duplicate_session_ids_error = load_linked_session_suite_manifest(&duplicate_session_ids)
        .expect_err("duplicate linked session ids should fail");
    match duplicate_session_ids_error {
        LinkedSessionSuiteManifestError::Build { message, .. } => {
            assert!(message.contains("DuplicateSessionId"));
        }
        other => panic!("unexpected linked manifest error: {other:?}"),
    }

    let duplicate_participant_ids = write_manifest(
        &workspace,
        "duplicate-participants.toml",
        r#"
version = 1

[[session]]
id = "broken"
timeout_frames = 1

  [[session.participant]]
  id = "duplicate"
  rom = "left.gb"

  [[session.participant]]
  id = "duplicate"
  rom = "right.gb"
"#,
    );
    let duplicate_participant_ids_error =
        load_linked_session_suite_manifest(&duplicate_participant_ids)
            .expect_err("duplicate participant ids should fail");
    match duplicate_participant_ids_error {
        LinkedSessionSuiteManifestError::Build { message, .. } => {
            assert!(message.contains("DuplicateParticipantId"));
        }
        other => panic!("unexpected linked manifest error: {other:?}"),
    }

    let invalid_participant_metadata = write_manifest(
        &workspace,
        "invalid-participant-metadata.toml",
        r#"
version = 1

[[session]]
id = "broken"
timeout_frames = 1
oracle = "linked-trace-fixture"
fixture = "fixtures/trace.txt"

  [[session.participant]]
  id = "left"
  rom = "left.gb"
  console = "sgb2"

  [[session.participant]]
  id = "right"
  rom = "right.gb"
"#,
    );
    let invalid_participant_metadata_error =
        load_linked_session_suite_manifest(&invalid_participant_metadata)
            .expect_err("bad console should fail");
    match invalid_participant_metadata_error {
        LinkedSessionSuiteManifestError::Build { message, .. } => {
            assert!(message.contains("unsupported console"));
        }
        other => panic!("unexpected linked manifest error: {other:?}"),
    }
}

#[test]
fn linked_session_manifest_reports_read_parse_and_remaining_stimulus_errors() {
    let missing = load_linked_session_suite_manifest(Path::new(
        "/definitely/missing/linked-session-suite.toml",
    ))
    .expect_err("missing linked manifest should fail");
    assert!(matches!(
        missing,
        LinkedSessionSuiteManifestError::Read { .. }
    ));

    let workspace = unique_temp_dir("invalid-parse");
    let invalid_toml = write_manifest(&workspace, "invalid.toml", "version = 1\n[[session]\n");
    let parse_error = load_linked_session_suite_manifest(&invalid_toml)
        .expect_err("invalid linked TOML should fail");
    assert!(matches!(
        parse_error,
        LinkedSessionSuiteManifestError::Parse { .. }
    ));

    let bad_stimulus = write_manifest(
        &workspace,
        "bad-stimulus.toml",
        r#"
version = 1

[[session]]
id = "broken"
timeout_frames = 1
oracle = "info-linked-trace"

  [[session.participant]]
  id = "left"
  rom = "left.gb"

    [[session.participant.stimulus]]
    frame = 1
    tcycle = 2
    button = "a"
    pressed = true

  [[session.participant]]
  id = "right"
  rom = "right.gb"
"#,
    );
    let bad_stimulus_error = load_linked_session_suite_manifest(&bad_stimulus)
        .expect_err("bad linked stimulus should fail");
    match bad_stimulus_error {
        LinkedSessionSuiteManifestError::Build { message, .. } => {
            assert!(message.contains("cannot specify both frame and tcycle"));
        }
        other => panic!("unexpected linked manifest error: {other:?}"),
    }

    let missing_stimulus_time = write_manifest(
        &workspace,
        "missing-stimulus-time.toml",
        r#"
version = 1

[[session]]
id = "broken"
timeout_frames = 1
oracle = "info-linked-trace"

  [[session.participant]]
  id = "left"
  rom = "left.gb"

    [[session.participant.stimulus]]
    button = "a"
    pressed = true

  [[session.participant]]
  id = "right"
  rom = "right.gb"
"#,
    );
    let missing_stimulus_time_error = load_linked_session_suite_manifest(&missing_stimulus_time)
        .expect_err("stimulus without frame or tcycle should fail");
    match missing_stimulus_time_error {
        LinkedSessionSuiteManifestError::Build { message, .. } => {
            assert!(message.contains("must specify either frame or tcycle"));
        }
        other => panic!("unexpected linked manifest error: {other:?}"),
    }

    let bad_button = write_manifest(
        &workspace,
        "bad-button.toml",
        r#"
version = 1

[[session]]
id = "broken"
timeout_frames = 1
oracle = "info-linked-trace"

  [[session.participant]]
  id = "left"
  rom = "left.gb"

    [[session.participant.stimulus]]
    frame = 1
    button = "turbo"
    pressed = true

  [[session.participant]]
  id = "right"
  rom = "right.gb"
"#,
    );
    let bad_button_error = load_linked_session_suite_manifest(&bad_button)
        .expect_err("unsupported linked button should fail");
    match bad_button_error {
        LinkedSessionSuiteManifestError::Build { message, .. } => {
            assert!(message.contains("unsupported joypad button"));
        }
        other => panic!("unexpected linked manifest error: {other:?}"),
    }
}

#[test]
fn linked_session_manifest_policy_helpers_keep_trace_and_snapshot_debugging_minimum() {
    let trace_fixture = LinkedSessionPassCondition::TraceFixture(PathBuf::from("expected.trace"));
    let snapshot_info =
        LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Snapshot);
    let participant_serial_hex = LinkedSessionPassCondition::ParticipantSerialHexExact {
        participant_id: "left".to_string(),
        expected: "A5".to_string(),
    };
    let participant_snapshot_fixture = LinkedSessionPassCondition::ParticipantSnapshotFixture {
        participant_id: "left".to_string(),
        fixture_path: PathBuf::from("left.snapshot"),
    };
    let participant_trace_fixture = LinkedSessionPassCondition::ParticipantTraceFixture {
        participant_id: "left".to_string(),
        fixture_path: PathBuf::from("left.trace"),
    };

    let trace_plan = capture_plan_for_pass_condition(&trace_fixture);
    assert!(trace_plan.contains(LinkedSessionCaptureKind::Trace));
    assert!(trace_plan.contains(LinkedSessionCaptureKind::Snapshot));

    let snapshot_plan = capture_plan_for_pass_condition(&snapshot_info);
    assert!(snapshot_plan.contains(LinkedSessionCaptureKind::Snapshot));
    let participant_plan = capture_plan_for_pass_condition(&participant_serial_hex);
    assert!(participant_plan.contains(LinkedSessionCaptureKind::ParticipantSerialHex));
    assert!(participant_plan.contains(LinkedSessionCaptureKind::Snapshot));
    let participant_snapshot_plan = capture_plan_for_pass_condition(&participant_snapshot_fixture);
    assert!(participant_snapshot_plan.contains(LinkedSessionCaptureKind::Snapshot));
    let participant_trace_plan = capture_plan_for_pass_condition(&participant_trace_fixture);
    assert!(participant_trace_plan.contains(LinkedSessionCaptureKind::Trace));
    assert!(participant_trace_plan.contains(LinkedSessionCaptureKind::Snapshot));

    let trace_failures = failure_artifacts_for_pass_condition(&trace_fixture);
    assert!(trace_failures.contains(LinkedSessionCaptureKind::Trace));
    assert!(trace_failures.contains(LinkedSessionCaptureKind::Snapshot));
    let participant_failures = failure_artifacts_for_pass_condition(&participant_serial_hex);
    assert!(participant_failures.contains(LinkedSessionCaptureKind::ParticipantSerialHex));
    assert!(participant_failures.contains(LinkedSessionCaptureKind::Snapshot));
    let participant_snapshot_failures =
        failure_artifacts_for_pass_condition(&participant_snapshot_fixture);
    assert!(participant_snapshot_failures.contains(LinkedSessionCaptureKind::Snapshot));
    let participant_trace_failures =
        failure_artifacts_for_pass_condition(&participant_trace_fixture);
    assert!(participant_trace_failures.contains(LinkedSessionCaptureKind::Trace));
    assert!(participant_trace_failures.contains(LinkedSessionCaptureKind::Snapshot));
}

#[test]
fn linked_session_manifest_rejects_unknown_participant_targets_for_participant_oracles() {
    let workspace = unique_temp_dir("unknown-pass-condition-participant");
    let manifest_path = write_manifest(
        &workspace,
        "unknown-participant.toml",
        r#"
version = 1

[[session]]
id = "broken"
topology = "dmg04"
timeout_tcycles = 256
oracle = "linked-participant-serial-hex-exact"
target_participant = "ghost"
expected = "A5"

  [[session.participant]]
  id = "left"
  rom = "left.gb"

  [[session.participant]]
  id = "right"
  rom = "right.gb"
"#,
    );

    let error = load_linked_session_suite_manifest(&manifest_path)
        .expect_err("unknown participant target should fail");
    match error {
        LinkedSessionSuiteManifestError::Build { message, .. } => {
            assert!(message.contains("UnknownPassConditionParticipant(\"ghost\")"));
        }
        other => panic!("unexpected linked manifest error: {other:?}"),
    }

    let snapshot_manifest_path = write_manifest(
        &workspace,
        "unknown-participant-snapshot.toml",
        r#"
version = 1

[[session]]
id = "broken-snapshot"
topology = "dmg04"
timeout_tcycles = 256
oracle = "linked-participant-snapshot-fixture"
target_participant = "ghost"
fixture = "ghost.snapshot"

  [[session.participant]]
  id = "left"
  rom = "left.gb"

  [[session.participant]]
  id = "right"
  rom = "right.gb"
"#,
    );

    let snapshot_error = load_linked_session_suite_manifest(&snapshot_manifest_path)
        .expect_err("unknown snapshot participant target should fail");
    match snapshot_error {
        LinkedSessionSuiteManifestError::Build { message, .. } => {
            assert!(message.contains("UnknownPassConditionParticipant(\"ghost\")"));
        }
        other => panic!("unexpected linked manifest error: {other:?}"),
    }

    let trace_manifest_path = write_manifest(
        &workspace,
        "unknown-participant-trace.toml",
        r#"
version = 1

[[session]]
id = "broken-trace"
topology = "dmg04"
timeout_tcycles = 256
oracle = "linked-participant-trace-fixture"
target_participant = "ghost"
fixture = "ghost.trace"

  [[session.participant]]
  id = "left"
  rom = "left.gb"

  [[session.participant]]
  id = "right"
  rom = "right.gb"
"#,
    );

    let trace_error = load_linked_session_suite_manifest(&trace_manifest_path)
        .expect_err("unknown trace participant target should fail");
    match trace_error {
        LinkedSessionSuiteManifestError::Build { message, .. } => {
            assert!(message.contains("UnknownPassConditionParticipant(\"ghost\")"));
        }
        other => panic!("unexpected linked manifest error: {other:?}"),
    }
}
