use std::collections::BTreeSet;

use super::model::{
    LinkedSessionCase, LinkedSessionCaseValidationError, LinkedSessionParticipant,
    LinkedSessionParticipantValidationError, LinkedSessionPassCondition, LinkedSessionSuite,
    LinkedSessionSuiteValidationError, LinkedSessionTopology,
};

impl LinkedSessionParticipant {
    pub fn validate(&self) -> Result<(), LinkedSessionParticipantValidationError> {
        if self.id.trim().is_empty() {
            return Err(LinkedSessionParticipantValidationError::EmptyParticipantId);
        }

        if self.rom_path.as_os_str().is_empty() {
            return Err(LinkedSessionParticipantValidationError::MissingRomPath);
        }

        if self
            .external_rom_root_key
            .as_deref()
            .is_some_and(|key| key.trim().is_empty())
        {
            return Err(LinkedSessionParticipantValidationError::EmptyExternalRomRootKey);
        }

        for (index, stimulus) in self.external_stimuli.stimuli().iter().enumerate() {
            if self.external_stimuli.stimuli()[index + 1..].contains(stimulus) {
                return Err(
                    LinkedSessionParticipantValidationError::DuplicateExternalStimulus(*stimulus),
                );
            }
        }

        Ok(())
    }
}

impl LinkedSessionCase {
    pub fn validate(&self) -> Result<(), LinkedSessionCaseValidationError> {
        if self.id.trim().is_empty() {
            return Err(LinkedSessionCaseValidationError::EmptySessionId);
        }

        if !self.timeout.is_valid() {
            return Err(LinkedSessionCaseValidationError::InvalidTimeout);
        }

        let required_capture = self.pass_condition.required_capture();
        if !self.capture_plan.contains(required_capture) {
            return Err(LinkedSessionCaseValidationError::MissingRequiredCapture(
                required_capture,
            ));
        }

        if self.failure_artifacts.retained().is_empty() {
            return Err(LinkedSessionCaseValidationError::MissingFailureArtifacts);
        }

        if !self.failure_artifacts.contains(required_capture) {
            return Err(
                LinkedSessionCaseValidationError::MissingRequiredFailureArtifact(required_capture),
            );
        }

        for artifact in self.failure_artifacts.retained() {
            if !self.capture_plan.contains(*artifact) {
                return Err(LinkedSessionCaseValidationError::ArtifactNotCaptured(
                    *artifact,
                ));
            }
        }

        let participant_count = self.participants.len();
        match self.topology {
            LinkedSessionTopology::Dmg04 if participant_count != 2 => {
                return Err(
                    LinkedSessionCaseValidationError::UnsupportedTopologyParticipantCount {
                        topology: self.topology,
                        count: participant_count,
                    },
                );
            }
            LinkedSessionTopology::Dmg04 => {}
            LinkedSessionTopology::Dmg07 if !(2..=4).contains(&participant_count) => {
                return Err(
                    LinkedSessionCaseValidationError::UnsupportedTopologyParticipantCount {
                        topology: self.topology,
                        count: participant_count,
                    },
                );
            }
            LinkedSessionTopology::Dmg07 => {}
        }

        let mut seen_participant_ids = BTreeSet::new();
        let mut seen_dmg07_ports = BTreeSet::new();
        let mut has_dmg07_p1 = false;
        for participant in &self.participants {
            if !seen_participant_ids.insert(participant.id.clone()) {
                return Err(LinkedSessionCaseValidationError::DuplicateParticipantId(
                    participant.id.clone(),
                ));
            }

            match (self.topology, participant.adapter_port) {
                (LinkedSessionTopology::Dmg04, Some(port)) => {
                    return Err(
                        LinkedSessionCaseValidationError::UnexpectedDmg04ParticipantPort {
                            participant_id: participant.id.clone(),
                            port,
                        },
                    );
                }
                (LinkedSessionTopology::Dmg04, None) => {}
                (LinkedSessionTopology::Dmg07, Some(port)) => {
                    if !seen_dmg07_ports.insert(port) {
                        return Err(
                            LinkedSessionCaseValidationError::DuplicateDmg07ParticipantPort {
                                port,
                            },
                        );
                    }
                    has_dmg07_p1 |= port == gb_core::Dmg07Port::P1;
                }
                (LinkedSessionTopology::Dmg07, None) => {
                    return Err(
                        LinkedSessionCaseValidationError::MissingDmg07ParticipantPort {
                            participant_id: participant.id.clone(),
                        },
                    );
                }
            }

            if let Err(error) = participant.validate() {
                return Err(LinkedSessionCaseValidationError::InvalidParticipant {
                    participant_id: participant.id.clone(),
                    error,
                });
            }
        }

        if self.topology == LinkedSessionTopology::Dmg07 && !has_dmg07_p1 {
            return Err(LinkedSessionCaseValidationError::MissingDmg07PlayerOne);
        }

        match &self.pass_condition {
            LinkedSessionPassCondition::ParticipantSerialHexExact { participant_id, .. }
            | LinkedSessionPassCondition::ParticipantTraceFixture { participant_id, .. }
            | LinkedSessionPassCondition::ParticipantSnapshotFixture { participant_id, .. }
                if !self
                    .participants
                    .iter()
                    .any(|participant| participant.id == *participant_id) =>
            {
                return Err(
                    LinkedSessionCaseValidationError::UnknownPassConditionParticipant(
                        participant_id.clone(),
                    ),
                );
            }
            _ => {}
        }

        Ok(())
    }
}

impl LinkedSessionSuite {
    pub fn validate(&self) -> Result<(), LinkedSessionSuiteValidationError> {
        if self.name.trim().is_empty() {
            return Err(LinkedSessionSuiteValidationError::EmptySuiteName);
        }

        let mut seen_session_ids = BTreeSet::new();
        for session in &self.sessions {
            if !seen_session_ids.insert(session.id.clone()) {
                return Err(LinkedSessionSuiteValidationError::DuplicateSessionId(
                    session.id.clone(),
                ));
            }

            if let Err(error) = session.validate() {
                return Err(LinkedSessionSuiteValidationError::InvalidSession {
                    session_id: session.id.clone(),
                    error,
                });
            }
        }

        Ok(())
    }
}
