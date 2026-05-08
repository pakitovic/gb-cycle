use std::{fs, io};

use super::{
    LinkedSessionCaseFailure, LinkedSessionCaseOutcome, LinkedSessionExecutionError,
    LinkedSessionRunArtifacts, LinkedSessionRunner,
};
use crate::framebuffer_oracle::{decode_fixture_framebuffer_path, decode_local_pgm_framebuffer};
use crate::{LinkedSessionCaptureKind, LinkedSessionCase, LinkedSessionPassCondition};
use gb_core::CpuDiagnosticTrap;

impl LinkedSessionRunner {
    pub(super) fn evaluate_session(
        &self,
        session: &LinkedSessionCase,
        artifacts: &LinkedSessionRunArtifacts,
        diagnostic_trap: Option<(usize, CpuDiagnosticTrap)>,
        framebuffer_until_match_matched: bool,
        framebuffer_until_match_check_at_reached: bool,
        executed_t_cycles: u64,
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
            LinkedSessionPassCondition::ParticipantFramebufferFixtureUntilMatch {
                participant_id,
                fixture_path,
                check_at_tcycles,
                ..
            } => {
                if framebuffer_until_match_matched {
                    LinkedSessionCaseOutcome::Passed
                } else if let Some(check_at_tcycles) = check_at_tcycles
                    && !framebuffer_until_match_check_at_reached
                {
                    LinkedSessionCaseOutcome::Failed(
                        LinkedSessionCaseFailure::ParticipantFramebufferCheckAtNotReached {
                            participant_id: participant_id.clone(),
                            check_at_tcycles: *check_at_tcycles,
                            executed_t_cycles,
                        },
                    )
                } else {
                    let participant_index = session
                        .participants
                        .iter()
                        .position(|participant| participant.id == *participant_id)
                        .expect("linked session should validate target participant existence");
                    let resolved_fixture = self.runner.resolve_path(fixture_path);
                    let actual = decode_local_pgm_framebuffer(
                        session.id.as_str(),
                        artifacts.participants[participant_index]
                            .framebuffer_pgm
                            .as_deref()
                            .ok_or_else(|| LinkedSessionExecutionError::FileOperation {
                                path: std::path::PathBuf::from(format!(
                                    "<participant framebuffer for {}>",
                                    session.id
                                )),
                                operation: "decode participant framebuffer artifact",
                                source: Box::new(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    "missing participant framebuffer capture",
                                )),
                            })?,
                    )
                    .map_err(|error| {
                        let path = error.path.clone();
                        LinkedSessionExecutionError::FileOperation {
                            path,
                            operation: "decode participant framebuffer artifact",
                            source: Box::new(error.into_invalid_data_error()),
                        }
                    })?;
                    let expected =
                        decode_fixture_framebuffer_path(&resolved_fixture).map_err(|error| {
                            let path = error.path.clone();
                            LinkedSessionExecutionError::FileOperation {
                                path,
                                operation: "decode participant framebuffer fixture",
                                source: Box::new(error.into_invalid_data_error()),
                            }
                        })?;
                    if actual == expected {
                        LinkedSessionCaseOutcome::Passed
                    } else {
                        LinkedSessionCaseOutcome::Failed(
                            LinkedSessionCaseFailure::ParticipantFixtureMismatch {
                                participant_id: participant_id.clone(),
                                capture: LinkedSessionCaptureKind::Framebuffer,
                                fixture_path: resolved_fixture,
                            },
                        )
                    }
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
        LinkedSessionCaseOutcome::Failed(
            LinkedSessionCaseFailure::ParticipantFramebufferCheckAtNotReached {
                participant_id: failed_participant_id,
                check_at_tcycles,
                executed_t_cycles,
            },
        ) => {
            if failed_participant_id == participant_id {
                LinkedSessionCaseOutcome::Failed(
                    LinkedSessionCaseFailure::ParticipantFramebufferCheckAtNotReached {
                        participant_id: failed_participant_id.clone(),
                        check_at_tcycles: *check_at_tcycles,
                        executed_t_cycles: *executed_t_cycles,
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
