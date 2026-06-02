use super::catalog::{
    LinkedParticipantObservation, LinkedSessionObservation, OracleConfig, OracleObservations,
    OracleOutcome, OracleStep,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SerialHexExactOracle {
    target_participant: String,
    expected: String,
}

impl SerialHexExactOracle {
    pub(super) fn from_manifest(config: &OracleConfig) -> Result<Self, String> {
        config.reject_unknown_parameters(&["target_participant", "expected"])?;
        Ok(Self {
            target_participant: config.required_string("target_participant")?,
            expected: config.required_string("expected")?,
        })
    }

    pub(crate) const fn observe(&self, _observations: OracleObservations<'_>) -> OracleStep {
        OracleStep::Continue
    }

    pub(crate) fn finish(
        &self,
        observations: OracleObservations<'_>,
    ) -> Result<OracleOutcome, String> {
        let actual = self
            .target_participant_observation(observations)?
            .serial_hex;
        Ok(if actual == self.expected {
            OracleOutcome::Passed
        } else {
            OracleOutcome::Failed(format!(
                "serial hex for participant {:?} did not match: expected {:?}, actual {:?}",
                self.target_participant, self.expected, actual
            ))
        })
    }

    fn target_participant_observation<'a>(
        &self,
        observations: OracleObservations<'a>,
    ) -> Result<LinkedParticipantObservation<'a>, String> {
        let linked = observations.linked.ok_or_else(|| {
            "serial-hex-exact oracle requires linked session observation".to_string()
        })?;
        participant_observation(linked, &self.target_participant)
    }
}

fn participant_observation<'a>(
    linked: LinkedSessionObservation<'a>,
    target_participant: &str,
) -> Result<LinkedParticipantObservation<'a>, String> {
    linked
        .participants
        .iter()
        .copied()
        .find(|participant| participant.id == target_participant)
        .ok_or_else(|| {
            format!("linked participant {target_participant:?} observation is not available")
        })
}
