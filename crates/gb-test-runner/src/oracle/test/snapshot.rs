use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use super::super::{
    FramebufferObservation, LinkedParticipantObservation, LinkedSessionObservation, Oracle,
    OracleConfig, OracleObservations, OracleOutcome,
};

#[derive(Debug, Deserialize)]
struct OracleWrapper {
    oracle: OracleConfig,
}

fn parse_oracle_config(text: &str) -> OracleConfig {
    toml::from_str::<OracleWrapper>(text)
        .expect("oracle config should parse")
        .oracle
}

fn temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "gb-cycle-snapshot-oracle-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
}

fn write_fixture(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("fixture should have parent"))
        .expect("fixture parent should be creatable");
    fs::write(path, text).expect("fixture should be writable");
}

fn participant<'a>(
    id: &'a str,
    serial_hex: &'a str,
    snapshot: Option<&'a str>,
) -> LinkedParticipantObservation<'a> {
    LinkedParticipantObservation {
        id,
        serial: b"",
        serial_hex,
        snapshot,
        trace: None,
        framebuffer: FramebufferObservation::empty(),
    }
}

fn observations<'a>(
    snapshot: Option<&'a str>,
    participants: &'a [LinkedParticipantObservation<'a>],
) -> OracleObservations<'a> {
    OracleObservations {
        serial: b"",
        cpu: None,
        memory: &[],
        executed_tcycles: 0,
        framebuffer: FramebufferObservation::empty(),
        participants: &[],
        linked: Some(LinkedSessionObservation {
            snapshot,
            trace: None,
            topology_trace: None,
            participants,
        }),
    }
}

fn no_linked_observations() -> OracleObservations<'static> {
    OracleObservations {
        serial: b"",
        cpu: None,
        memory: &[],
        executed_tcycles: 0,
        framebuffer: FramebufferObservation::empty(),
        participants: &[],
        linked: None,
    }
}

#[test]
fn snapshot_global_compares_session_snapshot_fixture() {
    let temp_dir = temp_dir("global");
    write_fixture(&temp_dir, "fixtures/session.snapshot", "session snapshot\n");
    let mut oracle = Oracle::from_manifest_with_fixture_root(
        &parse_oracle_config(
            "oracle = { type = \"snapshot\", fixture = \"fixtures/session.snapshot\" }",
        ),
        &temp_dir,
    )
    .expect("snapshot oracle should parse");

    assert!(matches!(oracle, Oracle::Snapshot(_)));
    assert_eq!(
        oracle
            .finish(observations(Some("session snapshot\n"), &[]))
            .expect("snapshot oracle should finish"),
        OracleOutcome::Passed
    );

    let mut oracle = Oracle::from_manifest_with_fixture_root(
        &parse_oracle_config(
            "oracle = { type = \"snapshot\", fixture = \"fixtures/session.snapshot\" }",
        ),
        &temp_dir,
    )
    .expect("snapshot oracle should parse");
    assert!(matches!(
        oracle
            .finish(observations(Some("different\n"), &[]))
            .expect("snapshot oracle should finish"),
        OracleOutcome::Failed(message) if message.contains("linked session snapshot did not match fixture")
    ));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn snapshot_target_participant_compares_participant_snapshot_fixture() {
    let temp_dir = temp_dir("participant");
    write_fixture(&temp_dir, "fixtures/left.snapshot", "left snapshot\n");
    let mut oracle = Oracle::from_manifest_with_fixture_root(
        &parse_oracle_config(
            "oracle = { type = \"snapshot\", target_participant = \"left\", fixture = \"fixtures/left.snapshot\" }",
        ),
        &temp_dir,
    )
    .expect("participant snapshot oracle should parse");
    let participants = [
        participant("left", "A5", Some("left snapshot\n")),
        participant("right", "3C", Some("right snapshot\n")),
    ];

    assert_eq!(
        oracle
            .finish(observations(None, &participants))
            .expect("participant snapshot oracle should finish"),
        OracleOutcome::Passed
    );

    let mut oracle = Oracle::from_manifest_with_fixture_root(
        &parse_oracle_config(
            "oracle = { type = \"snapshot\", target_participant = \"left\", fixture = \"fixtures/left.snapshot\" }",
        ),
        &temp_dir,
    )
    .expect("participant snapshot oracle should parse");
    let participants = [participant("left", "A5", Some("different\n"))];
    assert!(matches!(
        oracle
            .finish(observations(None, &participants))
            .expect("participant snapshot oracle should finish"),
        OracleOutcome::Failed(message) if message.contains("snapshot for participant")
    ));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn snapshot_rejects_missing_fixture_and_unknown_parameters() {
    assert!(
        Oracle::from_manifest(&parse_oracle_config("oracle = { type = \"snapshot\" }"))
            .expect_err("missing fixture should fail")
            .contains("requires fixture")
    );
    assert!(
        Oracle::from_manifest(&parse_oracle_config(
            "oracle = { type = \"snapshot\", fixture = \"snapshot.txt\", expected = \"x\" }"
        ))
        .expect_err("unknown parameter should fail")
        .contains("does not support parameter")
    );
}

#[test]
fn snapshot_requires_matching_linked_observations() {
    let temp_dir = temp_dir("missing-observation");
    write_fixture(&temp_dir, "fixtures/session.snapshot", "session snapshot\n");
    let mut oracle = Oracle::from_manifest_with_fixture_root(
        &parse_oracle_config(
            "oracle = { type = \"snapshot\", fixture = \"fixtures/session.snapshot\" }",
        ),
        &temp_dir,
    )
    .expect("snapshot oracle should parse");
    assert!(
        oracle
            .finish(no_linked_observations())
            .expect_err("missing linked observation should fail")
            .contains("requires linked session observation")
    );

    let mut oracle = Oracle::from_manifest_with_fixture_root(
        &parse_oracle_config(
            "oracle = { type = \"snapshot\", target_participant = \"left\", fixture = \"fixtures/session.snapshot\" }",
        ),
        &temp_dir,
    )
    .expect("participant snapshot oracle should parse");
    let participants = [participant("right", "3C", Some("session snapshot\n"))];
    assert!(
        oracle
            .finish(observations(None, &participants))
            .expect_err("missing participant observation should fail")
            .contains("linked participant")
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}
