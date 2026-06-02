use super::catalog::{OracleConfig, OracleObservations, OracleOutcome, OracleStep};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SerialContainsOracle {
    expected: String,
    matched: bool,
}

impl SerialContainsOracle {
    pub(super) fn from_manifest(config: &OracleConfig) -> Result<Self, String> {
        config.reject_unknown_parameters(&["expected"])?;
        Ok(Self::new(config.required_string("expected")?))
    }

    pub(crate) fn new(expected: impl Into<String>) -> Self {
        Self {
            expected: expected.into(),
            matched: false,
        }
    }

    pub(crate) fn matched(&self, observations: OracleObservations<'_>) -> bool {
        String::from_utf8_lossy(observations.serial).contains(&self.expected)
    }

    pub(crate) fn observe(&mut self, observations: OracleObservations<'_>) -> OracleStep {
        if self.matched(observations) {
            self.matched = true;
            OracleStep::Stop
        } else {
            OracleStep::Continue
        }
    }

    pub(crate) fn finish(&self, observations: OracleObservations<'_>) -> OracleOutcome {
        if self.matched || self.matched(observations) {
            OracleOutcome::Passed
        } else {
            OracleOutcome::Failed(self.failure_message(observations))
        }
    }

    pub(crate) fn failure_message(&self, observations: OracleObservations<'_>) -> String {
        format!(
            "serial output did not contain {:?}; actual {:?}",
            self.expected,
            String::from_utf8_lossy(observations.serial)
        )
    }
}
