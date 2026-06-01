use serde::Deserialize;

use super::super::{
    Oracle, OracleConfig, OracleObservations, serial_contains::SerialContainsOracle,
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

#[test]
fn serial_contains_matches_lossy_serial_text() {
    let oracle = SerialContainsOracle::new("Passed");

    assert!(oracle.matched(OracleObservations::serial(b"noise Passed\n")));
    assert!(!oracle.matched(OracleObservations::serial(b"noise Failed\n")));
}

#[test]
fn catalog_builds_serial_contains_oracle_from_manifest_config() {
    assert!(matches!(
        Oracle::from_manifest(&parse_oracle_config(
            "oracle = { type = \"serial-contains\", expected = \"Passed\" }"
        ))
        .expect("oracle should parse"),
        Oracle::SerialContains(_)
    ));
    assert!(
        Oracle::from_manifest(&parse_oracle_config(
            "oracle = { type = \"serial-contains\" }"
        ))
        .expect_err("missing expected should fail")
        .contains("requires expected")
    );
    assert!(
        Oracle::from_manifest(&parse_oracle_config(
            "oracle = { type = \"legacy-framebuffer-fixture\", expected = \"Passed\" }"
        ))
        .expect_err("unsupported oracle should fail")
        .contains("unsupported suite oracle")
    );
}
