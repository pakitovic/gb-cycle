use std::collections::BTreeMap;

use serde::Deserialize;

use super::serial_contains::SerialContainsOracle;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct OracleConfig {
    #[serde(rename = "type")]
    kind: String,
    #[serde(flatten)]
    parameters: BTreeMap<String, toml::Value>,
}

impl OracleConfig {
    pub(super) fn kind(&self) -> &str {
        &self.kind
    }

    pub(super) fn required_string(&self, field: &str) -> Result<String, String> {
        match self.parameters.get(field) {
            Some(toml::Value::String(value)) => Ok(value.clone()),
            Some(_) => Err(format!(
                "oracle {:?} field {field} must be a string",
                self.kind
            )),
            None => Err(format!("oracle {:?} requires {field}", self.kind)),
        }
    }

    pub(super) fn reject_unknown_parameters(&self, allowed: &[&str]) -> Result<(), String> {
        for parameter in self.parameters.keys() {
            if !allowed.contains(&parameter.as_str()) {
                return Err(format!(
                    "oracle {:?} does not support parameter {parameter:?}",
                    self.kind
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OracleObservations<'a> {
    pub(crate) serial: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Oracle {
    SerialContains(SerialContainsOracle),
}

impl Oracle {
    pub(crate) fn from_manifest(config: &OracleConfig) -> Result<Self, String> {
        match config.kind() {
            "serial-contains" => Ok(Self::SerialContains(SerialContainsOracle::from_manifest(
                config,
            )?)),
            other => Err(format!("unsupported suite oracle {other:?}")),
        }
    }

    pub(crate) fn matched(&self, observations: OracleObservations<'_>) -> bool {
        match self {
            Self::SerialContains(oracle) => oracle.matched(observations),
        }
    }

    pub(crate) fn failure_message(&self, observations: OracleObservations<'_>) -> String {
        match self {
            Self::SerialContains(oracle) => oracle.failure_message(observations),
        }
    }
}
