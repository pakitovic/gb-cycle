#[cfg(test)]
mod test;

use serde::Serialize;
use std::path::Path;

use crate::{BenchmarkCase, target_frame_rate_hz, target_frames_for_duration};

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkStats {
    pub version: u32,
    pub frontend: String,
    pub id: String,
    pub artifact_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_label: Option<String>,
    pub rom: String,
    pub model: String,
    pub startup: String,
    pub mode: String,
    pub test_runner: bool,
    pub duration_seconds: u32,
    pub target_frames: u32,
    pub completed_frames: u64,
    pub elapsed_seconds: f64,
    pub fps: f64,
    pub speed_percent: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executed_tcycles: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot: Option<String>,
}

impl BenchmarkStats {
    pub fn new(
        frontend: &str,
        case: &BenchmarkCase,
        test_runner: bool,
        completed_frames: u64,
        elapsed_seconds: f64,
        executed_tcycles: Option<u64>,
        screenshot: Option<&Path>,
    ) -> Self {
        let elapsed_seconds = elapsed_seconds.max(f64::EPSILON);
        let fps = completed_frames as f64 / elapsed_seconds;
        let speed_percent = fps / target_frame_rate_hz() * 100.0;
        Self {
            version: 1,
            frontend: frontend.to_string(),
            id: case.id.clone(),
            artifact_id: case.artifact_id.clone(),
            run_id: case.run_id.clone(),
            run_label: case.run_label.clone(),
            rom: case.rom.display().to_string(),
            model: case.model.as_str().to_string(),
            startup: case.startup.as_str().to_string(),
            mode: case.mode.as_str().to_string(),
            test_runner,
            duration_seconds: case.duration_seconds,
            target_frames: target_frames_for_duration(case.duration_seconds),
            completed_frames,
            elapsed_seconds,
            fps,
            speed_percent,
            executed_tcycles,
            screenshot: screenshot.map(|path| path.display().to_string()),
        }
    }
}

pub fn encode_stats_toml(stats: &BenchmarkStats) -> Result<String, toml::ser::Error> {
    toml::to_string_pretty(stats)
}
