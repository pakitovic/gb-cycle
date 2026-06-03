use askama::Template;
use serde::Deserialize;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    BenchmarkSuite, GB_CLI_FRONTEND, GB_DESKTOP_FRONTEND, frontend_screenshot_path,
    frontend_stats_path, load_benchmark_suite,
};

use super::cases::portable_file_name;
use super::paths::{case_files, io_error, relative_display};

#[derive(Debug, Clone, Template)]
#[template(path = "bench/index.html")]
pub(super) struct BenchIndexTemplate {
    generated_at: String,
    case_dir: String,
    include_cli: bool,
    rows: Vec<BenchIndexRow>,
    column_count: usize,
}

#[derive(Debug, Clone)]
struct BenchIndexRow {
    include_case_cells: bool,
    case_rowspan: usize,
    rom: String,
    case_path: String,
    model: String,
    seconds: String,
    artifacts: Vec<BenchIndexArtifact>,
}

#[derive(Debug, Clone)]
struct BenchIndexArtifact {
    has_stats: bool,
    fps: String,
    speed_percent: String,
    has_image: bool,
    image_href: String,
    image_alt: String,
}

#[derive(Debug, Deserialize)]
struct BenchmarkStatsSummary {
    fps: Option<f64>,
    speed_percent: Option<f64>,
}

pub(super) fn generate_index<W>(
    benchmark_dir: &Path,
    case_dir: &Path,
    include_cli: bool,
    output: &mut W,
) -> Result<(), String>
where
    W: Write,
{
    let report = build_index_report(benchmark_dir, case_dir, include_cli)?;
    let index = report
        .render()
        .map_err(|error| format!("failed to render benchmark index: {error}"))?;
    fs::create_dir_all(benchmark_dir).map_err(|error| {
        format!(
            "failed to create benchmark directory {}: {error}",
            benchmark_dir.display()
        )
    })?;
    let index_path = benchmark_dir.join("index.html");
    fs::write(&index_path, index)
        .map_err(|error| format!("failed to write {}: {error}", index_path.display()))?;
    writeln!(output, "wrote {}", index_path.display()).map_err(io_error)
}

pub(super) fn build_index_report(
    benchmark_dir: &Path,
    case_dir: &Path,
    include_cli: bool,
) -> Result<BenchIndexTemplate, String> {
    let frontends = if include_cli {
        vec![GB_CLI_FRONTEND, GB_DESKTOP_FRONTEND]
    } else {
        vec![GB_DESKTOP_FRONTEND]
    };
    let mut rows = Vec::new();
    for case_path in case_files(case_dir)? {
        let Ok(suite) = load_benchmark_suite(&case_path) else {
            continue;
        };
        let run_rows = expanded_index_runs(benchmark_dir, &frontends, &suite);
        let run_rows: Vec<_> = run_rows
            .into_iter()
            .filter(|run| {
                run.artifacts
                    .iter()
                    .any(|artifact| artifact.has_stats && artifact.has_image)
            })
            .collect();
        if run_rows.is_empty() {
            continue;
        }
        let rowspan = run_rows.len();
        for (index, mut row) in run_rows.into_iter().enumerate() {
            row.include_case_cells = index == 0;
            row.case_rowspan = rowspan;
            row.rom = rom_name(&suite.rom);
            row.case_path = relative_display(&case_path, case_dir);
            row.model = suite.model.as_str().to_string();
            rows.push(row);
        }
    }

    Ok(BenchIndexTemplate {
        generated_at: generated_at_text(),
        case_dir: case_dir.display().to_string(),
        include_cli,
        rows,
        column_count: 4 + frontends.len() * 2,
    })
}

fn expanded_index_runs(
    benchmark_dir: &Path,
    frontends: &[&str],
    suite: &BenchmarkSuite,
) -> Vec<BenchIndexRow> {
    suite
        .cases
        .iter()
        .map(|case| BenchIndexRow {
            include_case_cells: false,
            case_rowspan: 1,
            rom: String::new(),
            case_path: String::new(),
            model: String::new(),
            seconds: case.duration_seconds.to_string(),
            artifacts: frontends
                .iter()
                .map(|frontend| frontend_index_artifact(benchmark_dir, frontend, &case.artifact_id))
                .collect(),
        })
        .collect()
}

fn frontend_index_artifact(
    benchmark_dir: &Path,
    frontend: &str,
    artifact_id: &str,
) -> BenchIndexArtifact {
    let stats_path = benchmark_dir.join(frontend_stats_path(frontend, artifact_id));
    let image_path = benchmark_dir.join(frontend_screenshot_path(frontend, artifact_id));
    let stats = load_stats_summary(&stats_path);
    let has_complete_artifacts = stats.is_some() && image_path.exists();
    let (fps, speed_percent) = if has_complete_artifacts {
        let stats = stats.expect("complete benchmark artifacts include stats");
        (fmt_number(stats.fps, 2), fmt_number(stats.speed_percent, 1))
    } else {
        (String::new(), String::new())
    };
    let image_href = if has_complete_artifacts {
        relative_display(&image_path, benchmark_dir)
    } else {
        String::new()
    };
    BenchIndexArtifact {
        has_stats: has_complete_artifacts,
        fps,
        speed_percent,
        has_image: has_complete_artifacts,
        image_href,
        image_alt: format!("{frontend} {artifact_id}"),
    }
}

fn load_stats_summary(path: &Path) -> Option<BenchmarkStatsSummary> {
    let text = fs::read_to_string(path).ok()?;
    toml::from_str(&text).ok()
}

fn rom_name(path: &Path) -> String {
    let text = path.display().to_string();
    let name = portable_file_name(&text);
    if name.is_empty() {
        "—".to_string()
    } else {
        name.to_string()
    }
}

fn fmt_number(value: Option<f64>, digits: usize) -> String {
    match value {
        Some(value) => format!("{value:.digits$}"),
        None => "—".to_string(),
    }
}

fn generated_at_text() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    format!("{seconds}s since UNIX epoch")
}
