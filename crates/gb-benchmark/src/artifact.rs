use std::path::PathBuf;

pub const GB_CLI_FRONTEND: &str = "gb-cli";
pub const GB_DESKTOP_FRONTEND: &str = "gb-desktop";

pub fn frontend_stats_path(frontend: &str, artifact_id: &str) -> PathBuf {
    PathBuf::from(frontend).join(format!("{artifact_id}-stats.toml"))
}

pub fn frontend_screenshot_path(frontend: &str, artifact_id: &str) -> PathBuf {
    PathBuf::from(frontend).join(format!("{artifact_id}.png"))
}
