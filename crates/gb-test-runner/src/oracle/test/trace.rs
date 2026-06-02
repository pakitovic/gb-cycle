use serde::Deserialize;

use super::super::{Oracle, OracleConfig, OracleObservations, OracleOutcome, OracleStep};

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
fn trace_oracle_is_successful_and_ci_friendly() {
    let mut oracle = Oracle::from_manifest(&parse_oracle_config("oracle = { type = \"trace\" }"))
        .expect("trace oracle should parse");
    assert!(matches!(oracle, Oracle::Trace(_)));
    assert_eq!(
        oracle
            .observe(OracleObservations::serial(b""))
            .expect("trace oracle should observe"),
        OracleStep::Continue
    );
    assert_eq!(
        oracle
            .finish(OracleObservations::serial(b""))
            .expect("trace oracle should finish"),
        OracleOutcome::Passed
    );
}

#[test]
fn trace_oracle_rejects_unknown_parameters() {
    assert!(
        Oracle::from_manifest(&parse_oracle_config(
            "oracle = { type = \"trace\", fixture = \"trace.txt\" }"
        ))
        .expect_err("unknown parameter should fail")
        .contains("does not support parameter")
    );
}
