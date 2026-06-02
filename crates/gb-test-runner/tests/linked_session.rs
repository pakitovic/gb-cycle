use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use gb_test_runner::{
    LinkedSessionCaptureKind, LinkedSessionCaseOutcome, LinkedSessionPassCondition,
    LinkedSessionRunner, load_linked_session_suite_manifest,
};

const FIXTURE_ACCEPT_ENV: &str = "GB_CYCLE_ACCEPT_GB_TEST_RUNNER_LINKED_FIXTURES";

fn data_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn linked_fixture_path(relative: &str) -> PathBuf {
    data_path("data/linked/fixtures").join(relative)
}

fn toml_path(path: &Path) -> String {
    format!("{:?}", path.to_string_lossy())
}

fn unique_temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "gb-cycle-linked-session-contracts-{}-{}-{}",
        label,
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}

fn write_temp_manifest(label: &str, body: &str) -> PathBuf {
    let dir = unique_temp_dir(label);
    fs::create_dir_all(&dir).expect("temporary manifest dir should be creatable");
    let path = dir.join("linked-session.toml");
    fs::write(&path, body).expect("temporary manifest should be writable");
    path
}

fn ensure_text_fixture(path: &Path, actual: &str) {
    match fs::read_to_string(path) {
        Ok(expected) => assert_eq!(expected, actual, "fixture mismatch: {}", path.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if std::env::var_os(FIXTURE_ACCEPT_ENV).is_some() {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).expect("fixture parent should be creatable");
                }
                fs::write(path, actual).expect("fixture should be writable");
            } else {
                panic!(
                    "missing fixture {} (set {}=1 to accept)",
                    path.display(),
                    FIXTURE_ACCEPT_ENV
                );
            }
        }
        Err(error) => panic!("failed to read fixture {}: {error}", path.display()),
    }
}

fn load_fixture_backed_suite() -> gb_test_runner::LinkedSessionSuite {
    let fixture_path = linked_fixture_path("dmg04/basic-exchange.snapshot");
    let left_rom = linked_fixture_path("dmg04/basic-left.gb");
    let right_rom = linked_fixture_path("dmg04/basic-right.gb");
    let manifest_path = write_temp_manifest(
        "dmg04-smoke",
        &format!(
            concat!(
                "suite_name = \"dmg04\"\n",
                "family = \"serial-ext\"\n",
                "\n",
                "[[session]]\n",
                "id = \"dmg04-basic-exchange\"\n",
                "topology = \"dmg04\"\n",
                "timeout_tcycles = 5000\n",
                "oracle = \"linked-snapshot-fixture\"\n",
                "fixture = {}\n",
                "\n",
                "  [[session.participant]]\n",
                "  id = \"left\"\n",
                "  rom = {}\n",
                "  console = \"dmg\"\n",
                "\n",
                "  [[session.participant]]\n",
                "  id = \"right\"\n",
                "  rom = {}\n",
                "  console = \"dmg\"\n",
            ),
            toml_path(&fixture_path),
            toml_path(&left_rom),
            toml_path(&right_rom)
        ),
    );

    let suite = load_linked_session_suite_manifest(&manifest_path)
        .expect("repo linked-session manifest should load");
    assert_eq!(suite.sessions.len(), 1);

    let mut fixture_suite = suite.clone();
    fixture_suite.sessions[0].pass_condition =
        LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Snapshot);

    let fixture_report = LinkedSessionRunner::new()
        .run_suite(&fixture_suite)
        .expect("informational linked-session suite should execute");
    let actual_snapshot = fixture_report.sessions[0]
        .artifacts
        .snapshot_text
        .as_deref()
        .expect("informational linked-session suite should capture a snapshot");
    ensure_text_fixture(&fixture_path, actual_snapshot);

    suite
}

fn load_contract_suite_with_accepted_participant_fixtures() -> gb_test_runner::LinkedSessionSuite {
    let basic_left = linked_fixture_path("dmg04/basic-left.gb");
    let basic_right = linked_fixture_path("dmg04/basic-right.gb");
    let stale_left = linked_fixture_path("dmg04/stale-left.gb");
    let stale_right = linked_fixture_path("dmg04/stale-right.gb");
    let double_master_left = linked_fixture_path("dmg04/double-master-left.gb");
    let double_master_right = linked_fixture_path("dmg04/double-master-right.gb");
    let open_line_right = linked_fixture_path("dmg04/open-line-right.gb");
    let left_snapshot_fixture = linked_fixture_path("dmg04/basic-left.snapshot");
    let double_master_left_snapshot_fixture =
        linked_fixture_path("dmg04/double-master-left.snapshot");
    let double_master_right_snapshot_fixture =
        linked_fixture_path("dmg04/double-master-right.snapshot");
    let open_line_left_snapshot_fixture = linked_fixture_path("dmg04/open-line-left.snapshot");
    let manifest_path = write_temp_manifest(
        "dmg04-contracts",
        &format!(
            concat!(
                "suite_name = \"dmg04-contracts\"\n",
                "family = \"serial-ext\"\n",
                "\n",
                "[[session]]\n",
                "id = \"dmg04-left-serial-hex\"\n",
                "topology = \"dmg04\"\n",
                "timeout_tcycles = 5000\n",
                "oracle = \"linked-participant-serial-hex-exact\"\n",
                "target_participant = \"left\"\n",
                "expected = \"A5\"\n",
                "  [[session.participant]]\n",
                "  id = \"left\"\n",
                "  rom = {}\n",
                "  console = \"dmg\"\n",
                "  [[session.participant]]\n",
                "  id = \"right\"\n",
                "  rom = {}\n",
                "  console = \"dmg\"\n",
                "\n",
                "[[session]]\n",
                "id = \"dmg04-right-serial-hex\"\n",
                "topology = \"dmg04\"\n",
                "timeout_tcycles = 5000\n",
                "oracle = \"linked-participant-serial-hex-exact\"\n",
                "target_participant = \"right\"\n",
                "expected = \"3C\"\n",
                "  [[session.participant]]\n",
                "  id = \"left\"\n",
                "  rom = {}\n",
                "  console = \"dmg\"\n",
                "  [[session.participant]]\n",
                "  id = \"right\"\n",
                "  rom = {}\n",
                "  console = \"dmg\"\n",
                "\n",
                "[[session]]\n",
                "id = \"dmg04-left-snapshot\"\n",
                "topology = \"dmg04\"\n",
                "timeout_tcycles = 5000\n",
                "oracle = \"linked-participant-snapshot-fixture\"\n",
                "target_participant = \"left\"\n",
                "fixture = {}\n",
                "  [[session.participant]]\n",
                "  id = \"left\"\n",
                "  rom = {}\n",
                "  console = \"dmg\"\n",
                "  [[session.participant]]\n",
                "  id = \"right\"\n",
                "  rom = {}\n",
                "  console = \"dmg\"\n",
                "\n",
                "[[session]]\n",
                "id = \"dmg04-stale-left-serial-hex\"\n",
                "topology = \"dmg04\"\n",
                "timeout_tcycles = 10000\n",
                "oracle = \"linked-participant-serial-hex-exact\"\n",
                "target_participant = \"left\"\n",
                "expected = \"A5A5\"\n",
                "  [[session.participant]]\n",
                "  id = \"left\"\n",
                "  rom = {}\n",
                "  console = \"dmg\"\n",
                "  [[session.participant]]\n",
                "  id = \"right\"\n",
                "  rom = {}\n",
                "  console = \"dmg\"\n",
                "\n",
                "[[session]]\n",
                "id = \"dmg04-stale-right-serial-hex\"\n",
                "topology = \"dmg04\"\n",
                "timeout_tcycles = 10000\n",
                "oracle = \"linked-participant-serial-hex-exact\"\n",
                "target_participant = \"right\"\n",
                "expected = \"3CF0\"\n",
                "  [[session.participant]]\n",
                "  id = \"left\"\n",
                "  rom = {}\n",
                "  console = \"dmg\"\n",
                "  [[session.participant]]\n",
                "  id = \"right\"\n",
                "  rom = {}\n",
                "  console = \"dmg\"\n",
                "\n",
                "[[session]]\n",
                "id = \"dmg04-double-master-left-snapshot\"\n",
                "topology = \"dmg04\"\n",
                "timeout_tcycles = 5000\n",
                "oracle = \"linked-participant-snapshot-fixture\"\n",
                "target_participant = \"left\"\n",
                "fixture = {}\n",
                "  [[session.participant]]\n",
                "  id = \"left\"\n",
                "  rom = {}\n",
                "  console = \"dmg\"\n",
                "  [[session.participant]]\n",
                "  id = \"right\"\n",
                "  rom = {}\n",
                "  console = \"dmg\"\n",
                "\n",
                "[[session]]\n",
                "id = \"dmg04-double-master-right-snapshot\"\n",
                "topology = \"dmg04\"\n",
                "timeout_tcycles = 5000\n",
                "oracle = \"linked-participant-snapshot-fixture\"\n",
                "target_participant = \"right\"\n",
                "fixture = {}\n",
                "  [[session.participant]]\n",
                "  id = \"left\"\n",
                "  rom = {}\n",
                "  console = \"dmg\"\n",
                "  [[session.participant]]\n",
                "  id = \"right\"\n",
                "  rom = {}\n",
                "  console = \"dmg\"\n",
                "\n",
                "[[session]]\n",
                "id = \"dmg04-open-line-left-snapshot\"\n",
                "topology = \"dmg04\"\n",
                "timeout_tcycles = 5000\n",
                "oracle = \"linked-participant-snapshot-fixture\"\n",
                "target_participant = \"left\"\n",
                "fixture = {}\n",
                "  [[session.participant]]\n",
                "  id = \"left\"\n",
                "  rom = {}\n",
                "  console = \"dmg\"\n",
                "  [[session.participant]]\n",
                "  id = \"right\"\n",
                "  rom = {}\n",
                "  console = \"dmg\"\n",
            ),
            toml_path(&basic_left),
            toml_path(&basic_right),
            toml_path(&basic_left),
            toml_path(&basic_right),
            toml_path(&left_snapshot_fixture),
            toml_path(&basic_left),
            toml_path(&basic_right),
            toml_path(&stale_left),
            toml_path(&stale_right),
            toml_path(&stale_left),
            toml_path(&stale_right),
            toml_path(&double_master_left_snapshot_fixture),
            toml_path(&double_master_left),
            toml_path(&double_master_right),
            toml_path(&double_master_right_snapshot_fixture),
            toml_path(&double_master_left),
            toml_path(&double_master_right),
            toml_path(&open_line_left_snapshot_fixture),
            toml_path(&basic_left),
            toml_path(&open_line_right),
        ),
    );

    let suite = load_linked_session_suite_manifest(&manifest_path)
        .expect("repo linked contract manifest should load");
    assert_eq!(suite.sessions.len(), 8);

    let accept_participant_snapshot =
        |session_index: usize, participant_index: usize, fixture_path: &Path| {
            let mut snapshot_suite = suite.clone();
            snapshot_suite.sessions = vec![snapshot_suite.sessions[session_index].clone()];
            snapshot_suite.sessions[0].pass_condition =
                LinkedSessionPassCondition::Informational(LinkedSessionCaptureKind::Snapshot);
            let snapshot_report = LinkedSessionRunner::new()
                .run_suite(&snapshot_suite)
                .expect("informational participant snapshot suite should execute");
            let actual_snapshot = snapshot_report.sessions[0].participants[participant_index]
                .artifacts
                .snapshot_text
                .as_deref()
                .expect("participant snapshot should be captured");
            ensure_text_fixture(fixture_path, actual_snapshot);
        };

    accept_participant_snapshot(2, 0, &left_snapshot_fixture);
    accept_participant_snapshot(5, 0, &double_master_left_snapshot_fixture);
    accept_participant_snapshot(6, 1, &double_master_right_snapshot_fixture);
    accept_participant_snapshot(7, 0, &open_line_left_snapshot_fixture);

    suite
}

#[test]
fn linked_session_data_manifest_matches_the_retained_trace_fixture() {
    let suite = load_fixture_backed_suite();

    let report = LinkedSessionRunner::new()
        .run_suite(&suite)
        .expect("fixture-backed linked-session suite should execute");

    assert!(report.all_passed());
    assert_eq!(report.sessions.len(), 1);
    assert_eq!(
        report.sessions[0].participants[0].artifacts.serial_hex,
        "A5"
    );
    assert_eq!(
        report.sessions[0].participants[1].artifacts.serial_hex,
        "3C"
    );
}

#[test]
fn linked_session_data_manifest_is_deterministic_across_reruns() {
    let suite = load_fixture_backed_suite();

    let runner = LinkedSessionRunner::new();
    let first = runner
        .run_suite(&suite)
        .expect("first linked-session suite run should succeed");
    let second = runner
        .run_suite(&suite)
        .expect("second linked-session suite run should succeed");

    assert_eq!(first, second);
}

#[test]
fn linked_session_contract_manifest_matches_participant_scoped_oracles() {
    let suite = load_contract_suite_with_accepted_participant_fixtures();

    let report = LinkedSessionRunner::new()
        .run_suite(&suite)
        .expect("contract linked-session suite should execute");

    assert!(report.all_passed());
    assert_eq!(report.sessions.len(), 8);
    assert_eq!(
        report.sessions[0].participants[0].artifacts.serial_hex,
        "A5"
    );
    assert_eq!(
        report.sessions[1].participants[1].artifacts.serial_hex,
        "3C"
    );
    assert_eq!(report.sessions[2].participants[0].participant_id, "left");
    assert_eq!(
        report.sessions[3].participants[0].artifacts.serial_hex,
        "A5A5"
    );
    assert_eq!(
        report.sessions[4].participants[1].artifacts.serial_hex,
        "3CF0"
    );
    assert!(
        report.sessions[5].participants[0]
            .artifacts
            .snapshot_text
            .as_deref()
            .expect("left double-master snapshot should be captured")
            .contains("serial.sb=0xFF")
    );
    assert!(
        report.sessions[6].participants[1]
            .artifacts
            .snapshot_text
            .as_deref()
            .expect("right double-master snapshot should be captured")
            .contains("serial.sb=0xFF")
    );
    assert!(
        report.sessions[7].participants[0]
            .artifacts
            .snapshot_text
            .as_deref()
            .expect("left open-line snapshot should be captured")
            .contains("serial.sb=0xFF")
    );
}

#[test]
fn linked_session_contract_manifest_is_deterministic_across_reruns() {
    let suite = load_contract_suite_with_accepted_participant_fixtures();

    let runner = LinkedSessionRunner::new();
    let first = runner
        .run_suite(&suite)
        .expect("first linked contract suite run should succeed");
    let second = runner
        .run_suite(&suite)
        .expect("second linked contract suite run should succeed");

    assert_eq!(first, second);
}

#[test]
fn linked_session_cgb_ir_smoke_manifest_passes() {
    let emitter_rom = linked_fixture_path("cgb-ir/emitter.gb");
    let receiver_rom = linked_fixture_path("cgb-ir/receiver.gb");
    let manifest_path = write_temp_manifest(
        "cgb-ir",
        &format!(
            concat!(
                "suite_name = \"cgb-ir\"\n",
                "family = \"internal\"\n",
                "\n",
                "[[session]]\n",
                "id = \"cgb-ir-emitter-to-receiver\"\n",
                "topology = \"cgb-ir\"\n",
                "timeout_tcycles = 80000\n",
                "oracle = \"linked-participant-serial-hex-exact\"\n",
                "target_participant = \"receiver\"\n",
                "expected = \"B2\"\n",
                "\n",
                "  [[session.participant]]\n",
                "  id = \"emitter\"\n",
                "  rom = {}\n",
                "  console = \"cgb\"\n",
                "\n",
                "  [[session.participant]]\n",
                "  id = \"receiver\"\n",
                "  rom = {}\n",
                "  console = \"cgb\"\n",
            ),
            toml_path(&emitter_rom),
            toml_path(&receiver_rom)
        ),
    );
    let suite = load_linked_session_suite_manifest(&manifest_path)
        .expect("repo CGB IR linked-session manifest should load");

    let report = LinkedSessionRunner::new()
        .run_suite(&suite)
        .expect("CGB IR linked-session suite should execute");

    assert!(report.all_passed());
    assert_eq!(report.sessions.len(), 1);
    assert_eq!(
        report.sessions[0].participants[1].artifacts.serial_hex,
        "B2"
    );
}

#[test]
fn linked_session_contract_suite_persists_failure_artifacts_for_real_dmg04_cases() {
    let temp_dir = unique_temp_dir("failure-artifacts");
    let artifact_root = temp_dir.join("artifacts");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let mut suite = load_contract_suite_with_accepted_participant_fixtures();
    suite.sessions = vec![suite.sessions[0].clone()];
    suite.sessions[0].pass_condition = LinkedSessionPassCondition::ParticipantSerialHexExact {
        participant_id: "left".to_string(),
        expected: "FF".to_string(),
    };

    let report = LinkedSessionRunner::new()
        .with_failure_artifact_root(&artifact_root)
        .run_suite(&suite)
        .expect("failing contract suite should execute");

    assert!(!report.all_passed());
    assert!(matches!(
        report.sessions[0].outcome,
        LinkedSessionCaseOutcome::Failed(_)
    ));

    let session_dir = artifact_root.join("dmg04-left-serial-hex");
    assert!(session_dir.join("left_serial_hex.txt").is_file());
    assert!(session_dir.join("right_serial_hex.txt").is_file());
    assert!(session_dir.join("left_snapshot.txt").is_file());
    assert!(session_dir.join("right_snapshot.txt").is_file());

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}
