use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::oracle::{FramebufferArtifactDescriptor, FramebufferArtifactSource};

use super::model::{LinkRunArtifacts, LinkSuiteCase, Report};
use super::status::runtime_root_for_report;

const FRAMEBUFFER_WIDTH: usize = 160;
const FRAMEBUFFER_HEIGHT: usize = 144;
const DMG_GRAYSCALE_SHADES: [u8; 4] = [255, 170, 85, 0];

pub(super) fn clean_case_artifacts(
    workspace_root: &Path,
    report: &Report,
    suite_name: &str,
    case_id: &str,
) -> Result<(), String> {
    let artifact_dir = case_artifact_dir(workspace_root, report, suite_name, case_id);
    if artifact_dir.exists() {
        fs::remove_dir_all(&artifact_dir).map_err(|error| {
            format!(
                "failed to clean linked suite artifact directory {}: {error}",
                artifact_dir.display()
            )
        })?;
    }
    Ok(())
}

pub(super) struct LinkFailureArtifactRequest<'a> {
    pub(super) workspace_root: &'a Path,
    pub(super) report: &'a Report,
    pub(super) suite_name: &'a str,
    pub(super) case: &'a LinkSuiteCase,
    pub(super) failure: &'a str,
    pub(super) executed_tcycles: u64,
    pub(super) artifacts: &'a LinkRunArtifacts,
    pub(super) framebuffer: Option<FramebufferArtifactDescriptor>,
}

pub(super) fn persist_failure_artifacts(
    request: LinkFailureArtifactRequest<'_>,
) -> Result<PathBuf, String> {
    let artifact_dir = case_artifact_dir(
        request.workspace_root,
        request.report,
        request.suite_name,
        &request.case.id,
    );
    fs::create_dir_all(&artifact_dir).map_err(|error| {
        format!(
            "failed to create linked suite artifact directory {}: {error}",
            artifact_dir.display()
        )
    })?;

    let mut written = Vec::new();
    let mut artifact_errors = Vec::new();

    if let Some(snapshot) = &request.artifacts.session_snapshot {
        write_text_artifact(
            &artifact_dir,
            "snapshot.txt",
            snapshot,
            "linked snapshot artifact",
            &mut written,
            &mut artifact_errors,
        );
    }
    if let Some(trace) = &request.artifacts.session_trace {
        write_text_artifact(
            &artifact_dir,
            "trace.txt",
            trace,
            "linked trace artifact",
            &mut written,
            &mut artifact_errors,
        );
    }
    if let Some(trace) = &request.artifacts.topology_trace {
        write_text_artifact(
            &artifact_dir,
            "topology_trace.txt",
            trace,
            "linked topology trace artifact",
            &mut written,
            &mut artifact_errors,
        );
    }

    for participant in &request.artifacts.participants {
        let participant_dir = artifact_dir.join(&participant.id);
        if let Err(error) = fs::create_dir_all(&participant_dir) {
            artifact_errors.push(format!(
                "failed to create participant artifact directory {}: {error}",
                participant_dir.display()
            ));
            continue;
        }
        write_text_artifact(
            &participant_dir,
            "serial.txt",
            &String::from_utf8_lossy(&participant.serial),
            "participant serial artifact",
            &mut written,
            &mut artifact_errors,
        );
        write_text_artifact(
            &participant_dir,
            "serial.hex.txt",
            &participant.serial_hex,
            "participant serial hex artifact",
            &mut written,
            &mut artifact_errors,
        );
        if let Some(snapshot) = &participant.snapshot {
            write_text_artifact(
                &participant_dir,
                "snapshot.txt",
                snapshot,
                "participant snapshot artifact",
                &mut written,
                &mut artifact_errors,
            );
        }
        if let Some(trace) = &participant.trace {
            write_text_artifact(
                &participant_dir,
                "trace.txt",
                trace,
                "participant trace artifact",
                &mut written,
                &mut artifact_errors,
            );
        }
    }

    if let Some(framebuffer) = &request.framebuffer {
        persist_framebuffer_artifacts(
            &artifact_dir,
            framebuffer,
            request.artifacts,
            &mut written,
            &mut artifact_errors,
        );
    }

    let metadata = LinkFailureArtifactMetadata {
        report: request.report.id.clone(),
        suite: request.suite_name.to_string(),
        case: request.case.id.clone(),
        topology: request.case.topology.as_str(),
        executed_tcycles: request.executed_tcycles,
        failure: request.failure.to_string(),
        artifact_dir: artifact_dir.to_string_lossy().into_owned(),
        artifacts: written,
        artifact_errors,
        framebuffer: request.framebuffer.map(FramebufferFailureMetadata::from),
    };
    let metadata_text = toml::to_string(&metadata).map_err(|error| {
        format!(
            "failed to serialize linked suite artifact metadata for case {:?}: {error}",
            request.case.id
        )
    })?;
    let metadata_path = artifact_dir.join("failure.toml");
    fs::write(&metadata_path, metadata_text).map_err(|error| {
        format!(
            "failed to write linked suite artifact metadata {}: {error}",
            metadata_path.display()
        )
    })?;

    Ok(artifact_dir)
}

fn case_artifact_dir(
    workspace_root: &Path,
    report: &Report,
    suite_name: &str,
    case_id: &str,
) -> PathBuf {
    runtime_root_for_report(workspace_root, report)
        .join(&report.artifact_dir)
        .join(suite_name)
        .join(case_id)
}

fn write_text_artifact(
    artifact_dir: &Path,
    file_name: &str,
    contents: &str,
    label: &'static str,
    artifacts: &mut Vec<String>,
    artifact_errors: &mut Vec<String>,
) {
    let path = artifact_dir.join(file_name);
    match fs::write(&path, contents) {
        Ok(()) => artifacts.push(relative_artifact_name(artifact_dir, file_name)),
        Err(error) => artifact_errors.push(format!(
            "failed to write {label} {}: {error}",
            path.display()
        )),
    }
}

fn write_binary_artifact(
    artifact_dir: &Path,
    file_name: &str,
    contents: &[u8],
    label: &'static str,
    artifacts: &mut Vec<String>,
    artifact_errors: &mut Vec<String>,
) {
    let path = artifact_dir.join(file_name);
    match fs::write(&path, contents) {
        Ok(()) => artifacts.push(relative_artifact_name(artifact_dir, file_name)),
        Err(error) => artifact_errors.push(format!(
            "failed to write {label} {}: {error}",
            path.display()
        )),
    }
}

fn relative_artifact_name(_artifact_dir: &Path, file_name: &str) -> String {
    file_name.to_string()
}

fn persist_framebuffer_artifacts(
    artifact_dir: &Path,
    descriptor: &FramebufferArtifactDescriptor,
    artifacts: &LinkRunArtifacts,
    written: &mut Vec<String>,
    artifact_errors: &mut Vec<String>,
) {
    match actual_framebuffer_png(descriptor, artifacts) {
        Ok(png) => write_binary_artifact(
            artifact_dir,
            "actual.png",
            &png,
            "actual framebuffer PNG",
            written,
            artifact_errors,
        ),
        Err(error) => artifact_errors.push(error),
    }

    for (index, fixture) in descriptor.fixtures.iter().enumerate() {
        let file_name = expected_fixture_file_name(index, fixture);
        match fs::read(fixture) {
            Ok(bytes) => write_binary_artifact(
                artifact_dir,
                &file_name,
                &bytes,
                "expected framebuffer fixture",
                written,
                artifact_errors,
            ),
            Err(error) => artifact_errors.push(format!(
                "failed to read expected framebuffer fixture {}: {error}",
                fixture.display()
            )),
        }
    }
}

fn actual_framebuffer_png(
    descriptor: &FramebufferArtifactDescriptor,
    artifacts: &LinkRunArtifacts,
) -> Result<Vec<u8>, String> {
    let participant_id = descriptor.target_participant.as_deref().ok_or_else(|| {
        "linked framebuffer artifacts require target_participant in the oracle".to_string()
    })?;
    let participant = artifacts
        .participants
        .iter()
        .find(|participant| participant.id == participant_id)
        .ok_or_else(|| format!("participant {participant_id:?} artifacts are not available"))?;
    match descriptor.source {
        FramebufferArtifactSource::Dmg => encode_dmg_framebuffer_png(&participant.dmg_framebuffer),
        FramebufferArtifactSource::Cgb => encode_rgb555_framebuffer_png(
            participant
                .cgb_framebuffer
                .as_deref()
                .ok_or_else(|| "CGB RGB555 framebuffer is not available".to_string())?,
        ),
    }
}

fn expected_fixture_file_name(index: usize, fixture: &Path) -> String {
    let extension = fixture
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("bin");
    format!("expected-{index}.{extension}")
}

fn encode_dmg_framebuffer_png(framebuffer: &[u8]) -> Result<Vec<u8>, String> {
    let expected_len = FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT;
    if framebuffer.len() != expected_len {
        return Err(format!(
            "DMG framebuffer length {} does not match expected {expected_len}",
            framebuffer.len()
        ));
    }
    let pixels = framebuffer
        .iter()
        .copied()
        .map(|pixel| match pixel {
            0..=3 => DMG_GRAYSCALE_SHADES[usize::from(pixel)],
            _ => DMG_GRAYSCALE_SHADES[3],
        })
        .collect::<Vec<_>>();
    encode_grayscale_png(&pixels)
}

fn encode_rgb555_framebuffer_png(framebuffer: &[u16]) -> Result<Vec<u8>, String> {
    let expected_len = FRAMEBUFFER_WIDTH * FRAMEBUFFER_HEIGHT;
    if framebuffer.len() != expected_len {
        return Err(format!(
            "RGB555 framebuffer length {} does not match expected {expected_len}",
            framebuffer.len()
        ));
    }
    let mut pixels = Vec::with_capacity(expected_len * 3);
    for rgb555 in framebuffer.iter().copied() {
        let red = ((rgb555 & 0x001F) as u8) << 3;
        let green = (((rgb555 >> 5) & 0x001F) as u8) << 3;
        let blue = (((rgb555 >> 10) & 0x001F) as u8) << 3;
        pixels.extend_from_slice(&[red, green, blue]);
    }
    encode_rgb_png(&pixels)
}

fn encode_grayscale_png(pixels: &[u8]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(
        &mut bytes,
        FRAMEBUFFER_WIDTH as u32,
        FRAMEBUFFER_HEIGHT as u32,
    );
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|error| format!("failed to encode grayscale framebuffer PNG header: {error}"))?;
    writer
        .write_image_data(pixels)
        .map_err(|error| format!("failed to encode grayscale framebuffer PNG data: {error}"))?;
    drop(writer);
    Ok(bytes)
}

fn encode_rgb_png(pixels: &[u8]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(
        &mut bytes,
        FRAMEBUFFER_WIDTH as u32,
        FRAMEBUFFER_HEIGHT as u32,
    );
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|error| format!("failed to encode RGB framebuffer PNG header: {error}"))?;
    writer
        .write_image_data(pixels)
        .map_err(|error| format!("failed to encode RGB framebuffer PNG data: {error}"))?;
    drop(writer);
    Ok(bytes)
}

#[derive(Debug, Serialize)]
struct LinkFailureArtifactMetadata {
    report: String,
    suite: String,
    case: String,
    topology: &'static str,
    executed_tcycles: u64,
    failure: String,
    artifact_dir: String,
    artifacts: Vec<String>,
    artifact_errors: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    framebuffer: Option<FramebufferFailureMetadata>,
}

#[derive(Debug, Serialize)]
struct FramebufferFailureMetadata {
    source: &'static str,
    mode: &'static str,
    projection: &'static str,
    compare: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_participant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tolerance: Option<u8>,
    fixtures: Vec<String>,
}

impl From<FramebufferArtifactDescriptor> for FramebufferFailureMetadata {
    fn from(descriptor: FramebufferArtifactDescriptor) -> Self {
        Self {
            source: descriptor.source.as_str(),
            mode: descriptor.mode,
            projection: descriptor.projection,
            compare: descriptor.compare,
            target_participant: descriptor.target_participant,
            tolerance: descriptor.tolerance,
            fixtures: descriptor
                .fixtures
                .into_iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
        }
    }
}
