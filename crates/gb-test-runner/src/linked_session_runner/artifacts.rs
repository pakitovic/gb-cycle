use std::fs;

use super::{
    LinkedSessionCaptureKind, LinkedSessionCapturedArtifacts, LinkedSessionCase,
    LinkedSessionExecutionError, LinkedSessionParticipantArtifacts, LinkedSessionRunner,
    RunnerLinkedMachines,
};
use crate::framebuffer_oracle::{convert_pgm_to_png, encode_framebuffer_pgm};

pub(super) struct LinkedSessionRunArtifacts {
    pub(super) session: LinkedSessionCapturedArtifacts,
    pub(super) participants: Vec<LinkedSessionParticipantArtifacts>,
}

impl LinkedSessionRunner {
    pub(super) fn capture_artifacts(
        &self,
        session: &LinkedSessionCase,
        linked: &RunnerLinkedMachines,
        serial_bytes: &[Vec<u8>],
    ) -> LinkedSessionRunArtifacts {
        let capture_trace = session
            .capture_plan
            .contains(LinkedSessionCaptureKind::Trace);
        let capture_snapshot = session
            .capture_plan
            .contains(LinkedSessionCaptureKind::Snapshot);
        let capture_framebuffer = session
            .capture_plan
            .contains(LinkedSessionCaptureKind::Framebuffer);

        let mut participants = Vec::with_capacity(session.participants.len());
        for (participant_index, bytes) in serial_bytes.iter().enumerate() {
            participants.push(LinkedSessionParticipantArtifacts {
                serial: String::from_utf8_lossy(bytes).into_owned(),
                serial_hex: crate::encode_bytes_as_upper_hex(bytes),
                framebuffer_pgm: capture_framebuffer.then(|| {
                    encode_framebuffer_pgm(linked.participant_framebuffer(participant_index))
                }),
                trace_text: capture_trace
                    .then(|| linked.participant_trace_text(participant_index))
                    .flatten(),
                snapshot_text: capture_snapshot
                    .then(|| linked.participant_snapshot_text(participant_index)),
            });
        }

        let topology_trace_text = capture_trace
            .then(|| linked.topology_trace_text())
            .flatten();
        let session_trace = if capture_trace {
            Some(render_combined_trace(
                session,
                topology_trace_text.as_deref(),
                &participants,
            ))
        } else {
            None
        };
        let session_snapshot = if capture_snapshot {
            Some(render_combined_snapshot(session, &participants))
        } else {
            None
        };

        LinkedSessionRunArtifacts {
            session: LinkedSessionCapturedArtifacts {
                trace: session_trace,
                snapshot_text: session_snapshot,
                topology_trace_text,
            },
            participants,
        }
    }

    pub(super) fn persist_failure_artifacts(
        &self,
        session: &LinkedSessionCase,
        artifacts: &LinkedSessionRunArtifacts,
    ) -> Result<Vec<std::path::PathBuf>, LinkedSessionExecutionError> {
        let Some(root) = &self.runner.failure_artifact_root else {
            return Ok(Vec::new());
        };

        let session_dir = root.join(&session.id);
        fs::create_dir_all(&session_dir).map_err(|source| {
            LinkedSessionExecutionError::FileOperation {
                path: session_dir.clone(),
                operation: "create linked-session artifact directory",
                source: Box::new(source),
            }
        })?;

        let mut written_paths = Vec::new();
        for artifact in session.failure_artifacts.retained() {
            match artifact {
                LinkedSessionCaptureKind::ParticipantSerialHex => {
                    for (participant_index, participant) in session.participants.iter().enumerate()
                    {
                        write_failure_artifact(
                            session_dir.join(format!("{}_serial_hex.txt", participant.id)),
                            artifacts.participants[participant_index]
                                .serial_hex
                                .as_str(),
                            "write participant serial hex artifact",
                            &mut written_paths,
                        )?;
                    }
                }
                LinkedSessionCaptureKind::Framebuffer => {
                    for (participant_index, participant) in session.participants.iter().enumerate()
                    {
                        let Some(framebuffer_pgm) =
                            &artifacts.participants[participant_index].framebuffer_pgm
                        else {
                            continue;
                        };
                        let png_path =
                            session_dir.join(format!("{}_framebuffer.png", participant.id));
                        let framebuffer_png =
                            convert_pgm_to_png(framebuffer_pgm).map_err(|error| {
                                let path = error.path.clone();
                                LinkedSessionExecutionError::FileOperation {
                                    path,
                                    operation: "decode participant framebuffer artifact",
                                    source: Box::new(error.into_invalid_data_error()),
                                }
                            })?;
                        fs::write(&png_path, framebuffer_png).map_err(|source| {
                            LinkedSessionExecutionError::FileOperation {
                                path: png_path.clone(),
                                operation: "write participant framebuffer artifact",
                                source: Box::new(source),
                            }
                        })?;
                        written_paths.push(png_path);

                        let pgm_path =
                            session_dir.join(format!("{}_framebuffer.pgm", participant.id));
                        fs::write(&pgm_path, framebuffer_pgm).map_err(|source| {
                            LinkedSessionExecutionError::FileOperation {
                                path: pgm_path.clone(),
                                operation: "write participant framebuffer PGM artifact",
                                source: Box::new(source),
                            }
                        })?;
                        written_paths.push(pgm_path);
                    }
                }
                LinkedSessionCaptureKind::Trace => {
                    write_optional_failure_artifact(
                        session_dir.join("linked_trace.txt"),
                        artifacts.session.trace.as_deref(),
                        "write linked trace artifact",
                        &mut written_paths,
                    )?;
                    for (participant_index, participant) in session.participants.iter().enumerate()
                    {
                        write_failure_artifact(
                            session_dir.join(format!("{}_serial.txt", participant.id)),
                            artifacts.participants[participant_index].serial.as_str(),
                            "write participant serial artifact",
                            &mut written_paths,
                        )?;
                        write_failure_artifact(
                            session_dir.join(format!("{}_serial_hex.txt", participant.id)),
                            artifacts.participants[participant_index]
                                .serial_hex
                                .as_str(),
                            "write participant serial hex artifact",
                            &mut written_paths,
                        )?;
                        write_optional_failure_artifact(
                            session_dir.join(format!("{}_trace.txt", participant.id)),
                            artifacts.participants[participant_index]
                                .trace_text
                                .as_deref(),
                            "write participant trace artifact",
                            &mut written_paths,
                        )?;
                    }
                }
                LinkedSessionCaptureKind::Snapshot => {
                    write_optional_failure_artifact(
                        session_dir.join("linked_snapshot.txt"),
                        artifacts.session.snapshot_text.as_deref(),
                        "write linked snapshot artifact",
                        &mut written_paths,
                    )?;
                    for (participant_index, participant) in session.participants.iter().enumerate()
                    {
                        write_optional_failure_artifact(
                            session_dir.join(format!("{}_snapshot.txt", participant.id)),
                            artifacts.participants[participant_index]
                                .snapshot_text
                                .as_deref(),
                            "write participant snapshot artifact",
                            &mut written_paths,
                        )?;
                    }
                }
            }
        }

        Ok(written_paths)
    }
}

fn write_failure_artifact(
    path: std::path::PathBuf,
    contents: &str,
    operation: &'static str,
    written_paths: &mut Vec<std::path::PathBuf>,
) -> Result<(), LinkedSessionExecutionError> {
    fs::write(&path, contents).map_err(|source| LinkedSessionExecutionError::FileOperation {
        path: path.clone(),
        operation,
        source: Box::new(source),
    })?;
    written_paths.push(path);
    Ok(())
}

fn write_optional_failure_artifact(
    path: std::path::PathBuf,
    contents: Option<&str>,
    operation: &'static str,
    written_paths: &mut Vec<std::path::PathBuf>,
) -> Result<(), LinkedSessionExecutionError> {
    let Some(contents) = contents else {
        return Ok(());
    };

    write_failure_artifact(path, contents, operation, written_paths)
}

fn render_combined_trace(
    session: &LinkedSessionCase,
    topology_trace_text: Option<&str>,
    participants: &[LinkedSessionParticipantArtifacts],
) -> String {
    let mut rendered = String::new();
    if let Some(topology_trace_text) = topology_trace_text {
        rendered.push_str("== link topology trace ==\n");
        rendered.push_str(topology_trace_text);
        if !topology_trace_text.ends_with('\n') {
            rendered.push('\n');
        }
    }
    for (participant, artifacts) in session.participants.iter().zip(participants.iter()) {
        rendered.push_str("== participant ");
        rendered.push_str(&participant.id);
        rendered.push_str(" trace ==\n");
        if let Some(trace_text) = &artifacts.trace_text {
            rendered.push_str(trace_text);
            if !trace_text.ends_with('\n') {
                rendered.push('\n');
            }
        }
    }
    rendered
}

fn render_combined_snapshot(
    session: &LinkedSessionCase,
    participants: &[LinkedSessionParticipantArtifacts],
) -> String {
    let mut rendered = String::new();
    for (participant, artifacts) in session.participants.iter().zip(participants.iter()) {
        rendered.push_str("== participant ");
        rendered.push_str(&participant.id);
        rendered.push_str(" snapshot ==\n");
        if let Some(snapshot_text) = &artifacts.snapshot_text {
            rendered.push_str(snapshot_text);
            if !snapshot_text.ends_with('\n') {
                rendered.push('\n');
            }
        }
    }
    rendered
}
