use super::catalog::{OracleConfig, OracleObservations, OracleOutcome, OracleStep};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TraceOracle;

impl TraceOracle {
    pub(super) fn from_manifest(config: &OracleConfig) -> Result<Self, String> {
        config.reject_unknown_parameters(&[])?;
        Ok(Self)
    }

    pub(crate) const fn observe(&self, _observations: OracleObservations<'_>) -> OracleStep {
        OracleStep::Continue
    }

    pub(crate) const fn finish(&self, _observations: OracleObservations<'_>) -> OracleOutcome {
        OracleOutcome::Passed
    }
}
