use std::path::{Path, PathBuf};

use super::catalog::{
    LinkedParticipantObservation, LinkedSessionObservation, OracleConfig, OracleFixtureRoots,
    OracleObservations, OracleOutcome, OracleStep,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SnapshotOracle {
    fixture_path: PathBuf,
    expected: String,
    target_participant: Option<String>,
}

impl SnapshotOracle {
    pub(super) fn from_manifest(
        config: &OracleConfig,
        fixture_roots: OracleFixtureRoots<'_>,
    ) -> Result<Self, String> {
        config.reject_unknown_parameters(&["fixture", "target_participant"])?;
        let fixture_path = resolve_fixture_path(fixture_roots, &config.required_string("fixture")?);
        let expected = std::fs::read_to_string(&fixture_path).map_err(|error| {
            format!(
                "failed to read snapshot fixture {}: {error}",
                fixture_path.display()
            )
        })?;
        Ok(Self {
            fixture_path,
            expected,
            target_participant: config.optional_string("target_participant")?,
        })
    }

    pub(crate) const fn observe(&self, _observations: OracleObservations<'_>) -> OracleStep {
        OracleStep::Continue
    }

    pub(crate) fn finish(
        &self,
        observations: OracleObservations<'_>,
    ) -> Result<OracleOutcome, String> {
        let actual = self.target_snapshot(observations)?;
        if actual == self.expected {
            Ok(OracleOutcome::Passed)
        } else {
            Ok(OracleOutcome::Failed(self.failure_message()))
        }
    }

    fn target_snapshot<'a>(&self, observations: OracleObservations<'a>) -> Result<&'a str, String> {
        let linked = observations
            .linked
            .ok_or_else(|| "snapshot oracle requires linked session observation".to_string())?;
        let Some(target_participant) = &self.target_participant else {
            return linked
                .snapshot
                .ok_or_else(|| "linked session snapshot observation is not available".to_string());
        };
        participant_observation(linked, target_participant)?
            .snapshot
            .ok_or_else(|| {
                format!(
                    "snapshot observation for participant {target_participant:?} is not available"
                )
            })
    }

    fn failure_message(&self) -> String {
        match &self.target_participant {
            Some(participant) => format!(
                "snapshot for participant {participant:?} did not match fixture {}",
                self.fixture_path.display()
            ),
            None => format!(
                "linked session snapshot did not match fixture {}",
                self.fixture_path.display()
            ),
        }
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

fn resolve_fixture_path(fixture_roots: OracleFixtureRoots<'_>, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        fixture_roots.store.join(path)
    }
}
