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

fn participant<'a>(id: &'a str, serial_hex: &'a str) -> LinkedParticipantObservation<'a> {
    LinkedParticipantObservation {
        id,
        serial: b"",
        serial_hex,
        snapshot: None,
        trace: None,
        framebuffer: FramebufferObservation::empty(),
    }
}

fn observations<'a>(
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
            snapshot: None,
            trace: None,
            topology_trace: None,
            participants,
        }),
    }
}

fn serial_hex_oracle() -> Oracle {
    Oracle::from_manifest(&parse_oracle_config(
        "oracle = { type = \"serial-hex-exact\", target_participant = \"receiver\", expected = \"B2\" }",
    ))
    .expect("serial-hex-exact oracle should parse")
}

#[test]
fn serial_hex_exact_compares_target_participant_serial_hex() {
    let mut oracle = serial_hex_oracle();
    assert!(matches!(oracle, Oracle::SerialHexExact(_)));
    let participants = [participant("emitter", "00"), participant("receiver", "B2")];
    assert_eq!(
        oracle
            .finish(observations(&participants))
            .expect("serial-hex-exact oracle should finish"),
        OracleOutcome::Passed
    );

    let mut oracle = serial_hex_oracle();
    let participants = [participant("receiver", "B3")];
    assert!(matches!(
        oracle
            .finish(observations(&participants))
            .expect("serial-hex-exact oracle should finish"),
        OracleOutcome::Failed(message) if message.contains("expected") && message.contains("actual")
    ));
}

#[test]
fn serial_hex_exact_rejects_missing_and_unknown_parameters() {
    assert!(
        Oracle::from_manifest(&parse_oracle_config(
            "oracle = { type = \"serial-hex-exact\", expected = \"B2\" }"
        ))
        .expect_err("missing target participant should fail")
        .contains("requires target_participant")
    );
    assert!(
        Oracle::from_manifest(&parse_oracle_config(
            "oracle = { type = \"serial-hex-exact\", target_participant = \"receiver\" }"
        ))
        .expect_err("missing expected should fail")
        .contains("requires expected")
    );
    assert!(
        Oracle::from_manifest(&parse_oracle_config(
            "oracle = { type = \"serial-hex-exact\", target_participant = \"receiver\", expected = \"B2\", fixture = \"x\" }"
        ))
        .expect_err("unknown parameter should fail")
        .contains("does not support parameter")
    );
}

#[test]
fn serial_hex_exact_fails_when_target_participant_is_missing() {
    let mut oracle = serial_hex_oracle();
    let participants = [participant("emitter", "B2")];
    assert!(
        oracle
            .finish(observations(&participants))
            .expect_err("missing participant observation should fail")
            .contains("linked participant")
    );
}
