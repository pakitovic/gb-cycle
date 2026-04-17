use std::fs;
use std::path::{Path, PathBuf};

use gb_test_runner::{
    LinkedSessionCaptureKind, LinkedSessionPassCondition, LinkedSessionRunner,
    load_linked_session_suite_manifest,
};

const FIXTURE_ACCEPT_ENV: &str = "GB_CYCLE_ACCEPT_GB_TEST_RUNNER_LINKED_FIXTURES";

fn data_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative)
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
    let manifest_path = data_path("data/linked-dmg04-smoke.toml");
    let fixture_path = data_path("data/fixtures/linked/dmg04/basic-exchange.snapshot");

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
    let manifest_path = data_path("data/linked-dmg04-contracts.toml");
    let left_snapshot_fixture = data_path("data/fixtures/linked/dmg04/basic-left.snapshot");
    let double_master_left_snapshot_fixture =
        data_path("data/fixtures/linked/dmg04/double-master-left.snapshot");
    let double_master_right_snapshot_fixture =
        data_path("data/fixtures/linked/dmg04/double-master-right.snapshot");
    let open_line_left_snapshot_fixture =
        data_path("data/fixtures/linked/dmg04/open-line-left.snapshot");

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
