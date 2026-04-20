use std::fs;

use super::{
    LinkedSessionCaseFailure, LinkedSessionCaseOutcome, LinkedSessionExecutionError,
    LinkedSessionRunArtifacts, LinkedSessionRunner,
};
use crate::{LinkedSessionCaptureKind, LinkedSessionCase, LinkedSessionPassCondition};
use gb_core::CpuDiagnosticTrap;

impl LinkedSessionRunner {
    pub(super) fn evaluate_session(
        &self,
        session: &LinkedSessionCase,
        artifacts: &LinkedSessionRunArtifacts,
        diagnostic_trap: Option<(usize, CpuDiagnosticTrap)>,
    ) -> Result<LinkedSessionCaseOutcome, LinkedSessionExecutionError> {
        if let Some((participant_index, trap)) = diagnostic_trap {
            return Ok(LinkedSessionCaseOutcome::Failed(
                LinkedSessionCaseFailure::CpuDiagnosticTrap {
                    participant_id: session.participants[participant_index].id.clone(),
                    trap,
                },
            ));
        }

        Ok(match &session.pass_condition {
            LinkedSessionPassCondition::Informational(_) => LinkedSessionCaseOutcome::Informational,
            LinkedSessionPassCondition::ParticipantSerialHexExact {
                participant_id,
                expected,
            } => {
                let participant_index = session
                    .participants
                    .iter()
                    .position(|participant| participant.id == *participant_id)
                    .expect("linked session should validate target participant existence");
                let actual = artifacts.participants[participant_index].serial_hex.clone();
                if actual == *expected {
                    LinkedSessionCaseOutcome::Passed
                } else {
                    LinkedSessionCaseOutcome::Failed(
                        LinkedSessionCaseFailure::ParticipantSerialHexMismatch {
                            participant_id: participant_id.clone(),
                            expected: expected.clone(),
                            actual,
                        },
                    )
                }
            }
            LinkedSessionPassCondition::ParticipantSnapshotFixture {
                participant_id,
                fixture_path,
            } => {
                let participant_index = session
                    .participants
                    .iter()
                    .position(|participant| participant.id == *participant_id)
                    .expect("linked session should validate target participant existence");
                let resolved_fixture = self.runner.resolve_path(fixture_path);
                let expected = fs::read_to_string(&resolved_fixture).map_err(|source| {
                    LinkedSessionExecutionError::FileOperation {
                        path: resolved_fixture.clone(),
                        operation: "read participant snapshot fixture",
                        source: Box::new(source),
                    }
                })?;
                if artifacts.participants[participant_index]
                    .snapshot_text
                    .as_deref()
                    == Some(expected.as_str())
                {
                    LinkedSessionCaseOutcome::Passed
                } else {
                    LinkedSessionCaseOutcome::Failed(
                        LinkedSessionCaseFailure::ParticipantFixtureMismatch {
                            participant_id: participant_id.clone(),
                            capture: LinkedSessionCaptureKind::Snapshot,
                            fixture_path: resolved_fixture,
                        },
                    )
                }
            }
            LinkedSessionPassCondition::ParticipantTraceFixture {
                participant_id,
                fixture_path,
            } => {
                let participant_index = session
                    .participants
                    .iter()
                    .position(|participant| participant.id == *participant_id)
                    .expect("linked session should validate target participant existence");
                let resolved_fixture = self.runner.resolve_path(fixture_path);
                let expected = fs::read_to_string(&resolved_fixture).map_err(|source| {
                    LinkedSessionExecutionError::FileOperation {
                        path: resolved_fixture.clone(),
                        operation: "read participant trace fixture",
                        source: Box::new(source),
                    }
                })?;
                if artifacts.participants[participant_index]
                    .trace_text
                    .as_deref()
                    == Some(expected.as_str())
                {
                    LinkedSessionCaseOutcome::Passed
                } else {
                    LinkedSessionCaseOutcome::Failed(
                        LinkedSessionCaseFailure::ParticipantFixtureMismatch {
                            participant_id: participant_id.clone(),
                            capture: LinkedSessionCaptureKind::Trace,
                            fixture_path: resolved_fixture,
                        },
                    )
                }
            }
            LinkedSessionPassCondition::TraceFixture(fixture_path) => {
                let resolved_fixture = self.runner.resolve_path(fixture_path);
                let expected = fs::read_to_string(&resolved_fixture).map_err(|source| {
                    LinkedSessionExecutionError::FileOperation {
                        path: resolved_fixture.clone(),
                        operation: "read linked trace fixture",
                        source: Box::new(source),
                    }
                })?;
                if artifacts.session.trace.as_deref() == Some(expected.as_str()) {
                    LinkedSessionCaseOutcome::Passed
                } else {
                    LinkedSessionCaseOutcome::Failed(LinkedSessionCaseFailure::FixtureMismatch {
                        fixture_path: resolved_fixture,
                    })
                }
            }
            LinkedSessionPassCondition::SnapshotFixture(fixture_path) => {
                let resolved_fixture = self.runner.resolve_path(fixture_path);
                let expected = fs::read_to_string(&resolved_fixture).map_err(|source| {
                    LinkedSessionExecutionError::FileOperation {
                        path: resolved_fixture.clone(),
                        operation: "read linked snapshot fixture",
                        source: Box::new(source),
                    }
                })?;
                if artifacts.session.snapshot_text.as_deref() == Some(expected.as_str()) {
                    LinkedSessionCaseOutcome::Passed
                } else {
                    LinkedSessionCaseOutcome::Failed(LinkedSessionCaseFailure::FixtureMismatch {
                        fixture_path: resolved_fixture,
                    })
                }
            }
        })
    }
}

pub(super) fn participant_outcome_for_session(
    session_outcome: &LinkedSessionCaseOutcome,
    participant_id: &str,
) -> LinkedSessionCaseOutcome {
    match session_outcome {
        LinkedSessionCaseOutcome::Passed => LinkedSessionCaseOutcome::Passed,
        LinkedSessionCaseOutcome::Informational => LinkedSessionCaseOutcome::Informational,
        LinkedSessionCaseOutcome::Failed(LinkedSessionCaseFailure::CpuDiagnosticTrap {
            participant_id: failed_participant_id,
            trap,
        }) => {
            if failed_participant_id == participant_id {
                LinkedSessionCaseOutcome::Failed(LinkedSessionCaseFailure::CpuDiagnosticTrap {
                    participant_id: failed_participant_id.clone(),
                    trap: *trap,
                })
            } else {
                LinkedSessionCaseOutcome::Passed
            }
        }
        LinkedSessionCaseOutcome::Failed(
            LinkedSessionCaseFailure::ParticipantSerialHexMismatch {
                participant_id: failed_participant_id,
                expected,
                actual,
            },
        ) => {
            if failed_participant_id == participant_id {
                LinkedSessionCaseOutcome::Failed(
                    LinkedSessionCaseFailure::ParticipantSerialHexMismatch {
                        participant_id: failed_participant_id.clone(),
                        expected: expected.clone(),
                        actual: actual.clone(),
                    },
                )
            } else {
                LinkedSessionCaseOutcome::Passed
            }
        }
        LinkedSessionCaseOutcome::Failed(
            LinkedSessionCaseFailure::ParticipantFixtureMismatch {
                participant_id: failed_participant_id,
                capture,
                fixture_path,
            },
        ) => {
            if failed_participant_id == participant_id {
                LinkedSessionCaseOutcome::Failed(
                    LinkedSessionCaseFailure::ParticipantFixtureMismatch {
                        participant_id: failed_participant_id.clone(),
                        capture: *capture,
                        fixture_path: fixture_path.clone(),
                    },
                )
            } else {
                LinkedSessionCaseOutcome::Passed
            }
        }
        LinkedSessionCaseOutcome::Failed(LinkedSessionCaseFailure::FixtureMismatch {
            fixture_path,
        }) => LinkedSessionCaseOutcome::Failed(LinkedSessionCaseFailure::FixtureMismatch {
            fixture_path: fixture_path.clone(),
        }),
    }
}
