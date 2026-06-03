use super::*;
use crate::{
    BenchmarkMode, BenchmarkModel, BenchmarkPalette, BenchmarkStartup, GB_DESKTOP_FRONTEND,
};
use gb_core::DMG_T_CYCLES_PER_SECOND;
use std::path::{Path, PathBuf};

#[test]
fn speed_percent_matches_presented_frame_stats() {
    let case = crate::BenchmarkCase {
        source_path: PathBuf::from("case.toml"),
        id: "bench".to_string(),
        run_id: Some("run".to_string()),
        run_label: None,
        artifact_id: "bench-run".to_string(),
        rom: PathBuf::from("bench.gb"),
        model: BenchmarkModel::Dmg,
        startup: BenchmarkStartup::CustomBoot,
        mode: BenchmarkMode::Permissive,
        palette: Some(BenchmarkPalette::Grey),
        duration_seconds: 1,
        screenshot: true,
        stats: true,
        stimuli: Vec::new(),
    };
    let stats = BenchmarkStats::new(
        GB_DESKTOP_FRONTEND,
        &case,
        true,
        120,
        2.0,
        Some(DMG_T_CYCLES_PER_SECOND * 8),
        Some(Path::new("gb-desktop/bench-run.png")),
    );
    let expected = stats.fps / crate::target_frame_rate_hz() * 100.0;

    assert!((stats.speed_percent - expected).abs() < f64::EPSILON);
    assert_eq!(stats.executed_tcycles, Some(DMG_T_CYCLES_PER_SECOND * 8));
}
