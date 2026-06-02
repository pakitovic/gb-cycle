use serde::Deserialize;

use super::super::{
    FramebufferObservation, MemoryObservation, Oracle, OracleConfig, OracleObservations,
    OracleOutcome, OracleStep,
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

fn observations(memory: &[MemoryObservation]) -> OracleObservations<'_> {
    OracleObservations {
        serial: b"",
        cpu: None,
        memory,
        executed_tcycles: 0,
        framebuffer: FramebufferObservation::empty(),
        participants: &[],
        linked: None,
    }
}

fn memory_observation(address: u16, value: u8) -> [MemoryObservation; 1] {
    [MemoryObservation { address, value }]
}

fn empty_observations() -> OracleObservations<'static> {
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

fn memory_oracle() -> Oracle {
    Oracle::from_manifest(&parse_oracle_config(
        "oracle = { type = \"memory-byte-equals\", address = 65520, value = 1, fail_value = 2 }",
    ))
    .expect("memory-byte-equals oracle should parse")
}

#[test]
fn catalog_builds_memory_byte_equals_oracle_from_manifest_config() {
    assert!(matches!(memory_oracle(), Oracle::MemoryByteEquals(_)));
    assert!(matches!(
        Oracle::from_manifest(&parse_oracle_config(
            "oracle = { type = \"memory-byte-equals\", address = 65410, value = 1 }"
        ))
        .expect("memory-byte-equals without fail value should parse"),
        Oracle::MemoryByteEquals(_)
    ));
}

#[test]
fn memory_byte_equals_rejects_missing_out_of_range_and_unknown_parameters() {
    assert!(
        Oracle::from_manifest(&parse_oracle_config(
            "oracle = { type = \"memory-byte-equals\", value = 1 }"
        ))
        .expect_err("missing address should fail")
        .contains("requires address")
    );
    assert!(
        Oracle::from_manifest(&parse_oracle_config(
            "oracle = { type = \"memory-byte-equals\", address = 65520 }"
        ))
        .expect_err("missing value should fail")
        .contains("requires value")
    );
    assert!(
        Oracle::from_manifest(&parse_oracle_config(
            "oracle = { type = \"memory-byte-equals\", address = 65536, value = 1 }"
        ))
        .expect_err("address out of range should fail")
        .contains("between 0 and 65535")
    );
    assert!(
        Oracle::from_manifest(&parse_oracle_config(
            "oracle = { type = \"memory-byte-equals\", address = 65520, value = 256 }"
        ))
        .expect_err("value out of range should fail")
        .contains("between 0 and 255")
    );
    assert!(
        Oracle::from_manifest(&parse_oracle_config(
            "oracle = { type = \"memory-byte-equals\", address = 65520, value = 1, fail_value = 256 }"
        ))
        .expect_err("fail value out of range should fail")
        .contains("between 0 and 255")
    );
    assert!(
        Oracle::from_manifest(&parse_oracle_config(
            "oracle = { type = \"memory-byte-equals\", address = 65520, value = 1, expected = \"Passed\" }"
        ))
        .expect_err("unknown parameter should fail")
        .contains("does not support parameter")
    );
}

#[test]
fn memory_byte_equals_passes_when_expected_value_is_observed() {
    let mut oracle = memory_oracle();
    assert_eq!(
        oracle
            .observe(observations(&memory_observation(0xFFF0, 1)))
            .expect("oracle should observe"),
        OracleStep::Stop
    );
    assert_eq!(
        oracle
            .finish(observations(&memory_observation(0xFFF0, 1)))
            .expect("oracle should finish"),
        OracleOutcome::Passed
    );
}

#[test]
fn memory_byte_equals_fails_when_fail_value_is_observed() {
    let mut oracle = memory_oracle();
    assert_eq!(
        oracle
            .observe(observations(&memory_observation(0xFFF0, 2)))
            .expect("oracle should observe"),
        OracleStep::Stop
    );
    assert!(matches!(
        oracle
            .finish(observations(&memory_observation(0xFFF0, 2)))
            .expect("oracle should finish"),
        OracleOutcome::Failed(message)
            if message.contains("0xFFF0")
                && message.contains("expected 0x01")
                && message.contains("fail_value 0x02")
                && message.contains("actual 0x02")
    ));
}

#[test]
fn memory_byte_equals_fails_on_finish_when_expected_value_was_not_reached() {
    let mut oracle = memory_oracle();
    assert_eq!(
        oracle
            .observe(observations(&memory_observation(0xFFF0, 0)))
            .expect("oracle should observe"),
        OracleStep::Continue
    );
    assert!(matches!(
        oracle
            .finish(observations(&memory_observation(0xFFF0, 0)))
            .expect("oracle should finish"),
        OracleOutcome::Failed(message)
            if message.contains("0xFFF0")
                && message.contains("expected 0x01")
                && message.contains("actual 0x00")
    ));
}

#[test]
fn memory_byte_equals_requires_matching_memory_observation() {
    let mut oracle = memory_oracle();
    assert!(
        oracle
            .observe(empty_observations())
            .expect_err("missing memory observation should fail")
            .contains("requires memory observation")
    );
}
