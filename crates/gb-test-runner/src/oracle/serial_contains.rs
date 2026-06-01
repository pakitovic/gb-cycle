use super::catalog::{OracleConfig, OracleObservations};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SerialContainsOracle {
    expected: String,
}

impl SerialContainsOracle {
    pub(super) fn from_manifest(config: &OracleConfig) -> Result<Self, String> {
        config.reject_unknown_parameters(&["expected"])?;
        Ok(Self::new(config.required_string("expected")?))
    }

    pub(crate) fn new(expected: impl Into<String>) -> Self {
        Self {
            expected: expected.into(),
        }
    }

    pub(crate) fn matched(&self, observations: OracleObservations<'_>) -> bool {
        String::from_utf8_lossy(observations.serial).contains(&self.expected)
    }

    pub(crate) fn failure_message(&self, observations: OracleObservations<'_>) -> String {
        format!(
            "serial output did not contain {:?}; actual {:?}",
            self.expected,
            String::from_utf8_lossy(observations.serial)
        )
    }
}
