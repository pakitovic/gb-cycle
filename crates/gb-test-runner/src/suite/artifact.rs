use std::fs;
use std::path::{Path, PathBuf};

use gb_core::{Machine, TraceSummaryBuffer};
use serde::Serialize;

use crate::oracle::{FramebufferArtifactDescriptor, FramebufferArtifactSource};

use super::model::{Report, SuiteCase};
use super::status::store_root_for_report;

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
                "failed to clean suite artifact directory {}: {error}",
                artifact_dir.display()
            )
        })?;
    }
    Ok(())
}

pub(super) struct FailureArtifactRequest<'a> {
    pub(super) workspace_root: &'a Path,
    pub(super) report: &'a Report,
    pub(super) suite_name: &'a str,
    pub(super) case: &'a SuiteCase,
    pub(super) failure: &'a str,
    pub(super) executed_tcycles: u64,
    pub(super) serial_bytes: &'a [u8],
    pub(super) machine: Option<&'a Machine<TraceSummaryBuffer>>,
}

pub(super) fn persist_failure_artifacts(
    request: FailureArtifactRequest<'_>,
) -> Result<PathBuf, String> {
    let artifact_dir = case_artifact_dir(
        request.workspace_root,
        request.report,
        request.suite_name,
        &request.case.id,
    );
    fs::create_dir_all(&artifact_dir).map_err(|error| {
        format!(
            "failed to create suite artifact directory {}: {error}",
            artifact_dir.display()
        )
    })?;

    let mut artifacts = Vec::new();
    let mut artifact_errors = Vec::new();

    if !request.serial_bytes.is_empty() {
        let serial = String::from_utf8_lossy(request.serial_bytes);
        write_text_artifact(
            &artifact_dir,
            "serial.txt",
            serial.as_ref(),
            "serial artifact",
            &mut artifacts,
            &mut artifact_errors,
        );
    }

    if let Some(machine) = request.machine {
        write_text_artifact(
            &artifact_dir,
            "snapshot.txt",
            &machine.snapshot().render_text(),
            "snapshot artifact",
            &mut artifacts,
            &mut artifact_errors,
        );
    }

    let framebuffer = request.case.oracle.framebuffer_artifact_descriptor();
    if let Some(descriptor) = &framebuffer {
        persist_framebuffer_artifacts(
            &artifact_dir,
            descriptor,
            request.machine,
            &mut artifacts,
            &mut artifact_errors,
        );
    }

    let metadata = FailureArtifactMetadata {
        report: request.report.id.clone(),
        suite: request.suite_name.to_string(),
        case: request.case.id.clone(),
        family: request.case.family.clone(),
        rom: request.case.rom.to_string_lossy().into_owned(),
        target_root: request.case.target_root.to_string_lossy().into_owned(),
        executed_tcycles: request.executed_tcycles,
        failure: request.failure.to_string(),
        artifact_dir: artifact_dir.to_string_lossy().into_owned(),
        artifacts,
        artifact_errors,
        framebuffer: framebuffer.map(FramebufferFailureMetadata::from),
    };
    let metadata_text = toml::to_string(&metadata).map_err(|error| {
        format!(
            "failed to serialize suite artifact metadata for case {:?}: {error}",
            request.case.id
        )
    })?;
    let metadata_path = artifact_dir.join("failure.toml");
    fs::write(&metadata_path, metadata_text).map_err(|error| {
        format!(
            "failed to write suite artifact metadata {}: {error}",
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
    store_root_for_report(workspace_root, report)
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
        Ok(()) => artifacts.push(file_name.to_string()),
        Err(error) => artifact_errors.push(format!(
            "failed to write {label} {}: {error}",
            path.display()
        )),
    }
}

fn persist_framebuffer_artifacts(
    artifact_dir: &Path,
    descriptor: &FramebufferArtifactDescriptor,
    machine: Option<&Machine<TraceSummaryBuffer>>,
    artifacts: &mut Vec<String>,
    artifact_errors: &mut Vec<String>,
) {
    match machine {
        Some(machine) => match encode_actual_framebuffer_png(descriptor.source, machine) {
            Ok(png) => write_binary_artifact(
                artifact_dir,
                "actual.png",
                &png,
                "actual framebuffer PNG",
                artifacts,
                artifact_errors,
            ),
            Err(error) => artifact_errors.push(error),
        },
        None => artifact_errors
            .push("actual framebuffer is not available before machine setup".to_string()),
    }

    for (index, fixture) in descriptor.fixtures.iter().enumerate() {
        let file_name = expected_fixture_file_name(index, fixture);
        match fs::read(fixture) {
            Ok(bytes) => write_binary_artifact(
                artifact_dir,
                &file_name,
                &bytes,
                "expected framebuffer fixture",
                artifacts,
                artifact_errors,
            ),
            Err(error) => artifact_errors.push(format!(
                "failed to read expected framebuffer fixture {}: {error}",
                fixture.display()
            )),
        }
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
        Ok(()) => artifacts.push(file_name.to_string()),
        Err(error) => artifact_errors.push(format!(
            "failed to write {label} {}: {error}",
            path.display()
        )),
    }
}

fn expected_fixture_file_name(index: usize, fixture: &Path) -> String {
    let extension = fixture
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("bin");
    format!("expected-{index}.{extension}")
}

fn encode_actual_framebuffer_png(
    source: FramebufferArtifactSource,
    machine: &Machine<TraceSummaryBuffer>,
) -> Result<Vec<u8>, String> {
    match source {
        FramebufferArtifactSource::Dmg => encode_dmg_framebuffer_png(machine.ppu().framebuffer()),
        FramebufferArtifactSource::Cgb => encode_rgb555_framebuffer_png(
            machine
                .ppu()
                .cgb_framebuffer_rgb555()
                .ok_or_else(|| "CGB RGB555 framebuffer is not available".to_string())?,
        ),
    }
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
            "CGB RGB555 framebuffer length {} does not match expected {expected_len}",
            framebuffer.len()
        ));
    }
    let mut pixels = Vec::with_capacity(expected_len * 3);
    for pixel in framebuffer.iter().copied() {
        pixels.extend_from_slice(&rgb555_to_rgb888(pixel));
    }
    encode_rgb_png(&pixels)
}

fn encode_grayscale_png(pixels: &[u8]) -> Result<Vec<u8>, String> {
    encode_png(png::ColorType::Grayscale, pixels)
}

fn encode_rgb_png(pixels: &[u8]) -> Result<Vec<u8>, String> {
    encode_png(png::ColorType::Rgb, pixels)
}

fn encode_png(color_type: png::ColorType, pixels: &[u8]) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(
            &mut bytes,
            FRAMEBUFFER_WIDTH as u32,
            FRAMEBUFFER_HEIGHT as u32,
        );
        encoder.set_color(color_type);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| format!("failed to write PNG header: {error}"))?;
        writer
            .write_image_data(pixels)
            .map_err(|error| format!("failed to write PNG data: {error}"))?;
    }
    Ok(bytes)
}

fn rgb555_to_rgb888(color: u16) -> [u8; 3] {
    let red = (color & 0x001F) as u8;
    let green = ((color >> 5) & 0x001F) as u8;
    let blue = ((color >> 10) & 0x001F) as u8;
    [
        scale_5_bit_to_8_bit(red),
        scale_5_bit_to_8_bit(green),
        scale_5_bit_to_8_bit(blue),
    ]
}

fn scale_5_bit_to_8_bit(component: u8) -> u8 {
    (component << 3) | (component >> 2)
}

#[derive(Debug, Serialize)]
struct FailureArtifactMetadata {
    report: String,
    suite: String,
    case: String,
    family: String,
    rom: String,
    target_root: String,
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
                .map(|fixture| fixture.to_string_lossy().into_owned())
                .collect(),
        }
    }
}
