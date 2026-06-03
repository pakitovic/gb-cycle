use askama::Template;
use serde::Deserialize;
use std::ffi::OsStr;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    BenchmarkModel, BenchmarkSuite, GB_CLI_FRONTEND, GB_DESKTOP_FRONTEND, load_benchmark_suite,
};

const BENCH_OUTPUT_DIR: &str = "test/bench";
const DEFAULT_SAMPLE_NAME: &str = "game.toml";
const DEFAULT_TEMPLATE: &str = r#"version = 1
id = "game"
rom = "/roms/game.gb"
model = "DMG"
startup = "custom-boot"
mode = "permissive"
palette = "grey"
screenshot = true
stats = true

[[run]]
id = "idle-40"
duration_seconds = 40

[[run]]
id = "start-a-120"
duration_seconds = 120

[[run.input]]
frame = 30
button = "start"
hold_frames = 8
repeat_every_frames = 60

[[run.input]]
frame = 60
button = "a"
hold_frames = 8
repeat_every_frames = 60
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
enum BenchAction {
    ShowHelp,
    Sample,
    NormalizeCase {
        case_dir: PathBuf,
    },
    RewriteRomDir {
        case_dir: PathBuf,
        rom_dir: PathBuf,
    },
    GenerateCases {
        case_dir: PathBuf,
        rom_dir: PathBuf,
        template_path: Option<PathBuf>,
    },
    Run(BenchRunOptions),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BenchRunOptions {
    case_dir: Option<PathBuf>,
    single_test: Option<PathBuf>,
    include_cli: bool,
}

#[derive(Debug, Clone)]
struct ResolvedRunOptions {
    case_dir: PathBuf,
    cases: Vec<PathBuf>,
    include_cli: bool,
}

#[derive(Debug, Clone, Template)]
#[template(path = "bench/index.html")]
struct BenchIndexTemplate {
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

pub fn bench_help_text() -> &'static str {
    concat!(
        "Usage:\n",
        "  cargo rom-bench --sample\n",
        "  cargo rom-bench <case-dir> [--rom-dir <rom-dir>]\n",
        "  cargo rom-bench <case-dir> --normalize-case\n",
        "  cargo rom-bench <case-dir> --rom-dir <rom-dir> --generate-cases [--template <case.toml>]\n",
        "  cargo rom-bench [<case-dir>] [--gb-cli] --test <case.toml>\n",
        "\n",
        "Arguments:\n",
        "  <case-dir>        Directory containing benchmark case *.toml files; optional with --test.\n",
        "\n",
        "Options:\n",
        "  --sample          Create test/bench/game.toml if missing.\n",
        "  --rom-dir <dir>   Rewrite rom = \"...\" in <case-dir>/*.toml preserving each ROM basename.\n",
        "  --normalize-case  Rename <case-dir>/*.toml from each case's ROM filename stem.\n",
        "  --generate-cases  Generate normalized cases for every *.gb and *.gbc ROM under --rom-dir.\n",
        "  --template <path>  Use a benchmark case TOML template with --generate-cases.\n",
        "  --gb-cli          Run gb-cli in addition to the default gb-desktop benchmark.\n",
        "  --test <path>     Run one benchmark case; without <case-dir>, infer it from this file.\n",
        "  -h, --help        Show this help.\n",
        "\n",
        "Outputs are written to test/bench/. By default only gb-desktop runs.\n",
    )
}

pub fn run_bench_command<I, S, W>(arguments: I, output: &mut W) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    let current_dir = std::env::current_dir()
        .map_err(|error| format!("failed to resolve current directory: {error}"))?;
    run_bench_command_with_workspace(arguments, &default_workspace_root(), &current_dir, output)
}

fn run_bench_command_with_workspace<I, S, W>(
    arguments: I,
    workspace_root: &Path,
    current_dir: &Path,
    output: &mut W,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    match parse_bench_arguments(arguments)? {
        BenchAction::ShowHelp => write_all(output, bench_help_text()),
        BenchAction::Sample => write_sample_case(workspace_root, output),
        BenchAction::NormalizeCase { case_dir } => {
            let case_dir = resolve_existing_dir(&case_dir, current_dir, "<case-dir>")?;
            normalize_cases(&case_dir, output)
        }
        BenchAction::RewriteRomDir { case_dir, rom_dir } => {
            let case_dir = resolve_existing_dir(&case_dir, current_dir, "<case-dir>")?;
            let rom_dir = resolve_existing_dir(&rom_dir, current_dir, "--rom-dir")?;
            rewrite_rom_dir(&case_dir, &rom_dir, output)
        }
        BenchAction::GenerateCases {
            case_dir,
            rom_dir,
            template_path,
        } => {
            let case_dir = resolve_or_create_dir(&case_dir, current_dir, "<case-dir>")?;
            let rom_dir = resolve_existing_dir(&rom_dir, current_dir, "--rom-dir")?;
            let template_path = template_path
                .map(|path| resolve_existing_file(&path, current_dir, "--template"))
                .transpose()?;
            generate_benchmark_cases(&case_dir, &rom_dir, template_path.as_deref(), output)
        }
        BenchAction::Run(options) => {
            let resolved = resolve_run_options(options, current_dir)?;
            run_benchmark_cases(workspace_root, &resolved, output)
        }
    }
}

fn parse_bench_arguments<I, S>(arguments: I) -> Result<BenchAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut action_sample = false;
    let mut rom_dir = None;
    let mut include_cli = false;
    let mut single_test = None;
    let mut case_dir = None;
    let mut normalize_case = false;
    let mut generate_cases = false;
    let mut template_path = None;
    let mut arguments = arguments.into_iter();

    while let Some(argument) = arguments.next() {
        match argument.as_ref() {
            "--sample" => action_sample = true,
            "--rom-dir" => {
                let Some(value) = arguments.next() else {
                    return Err("--rom-dir requires a value".to_string());
                };
                rom_dir = Some(PathBuf::from(value.as_ref()));
            }
            "--normalize-case" => normalize_case = true,
            "--generate-cases" => generate_cases = true,
            "--template" => {
                let Some(value) = arguments.next() else {
                    return Err("--template requires a value".to_string());
                };
                template_path = Some(PathBuf::from(value.as_ref()));
            }
            "--gb-cli" => include_cli = true,
            "--test" => {
                let Some(value) = arguments.next() else {
                    return Err("--test requires a value".to_string());
                };
                single_test = Some(PathBuf::from(value.as_ref()));
            }
            "-h" | "--help" => return Ok(BenchAction::ShowHelp),
            value if value.starts_with('-') => {
                return Err(format!("unknown option {value}"));
            }
            value => {
                if case_dir.is_some() {
                    return Err(format!("unexpected argument {value}"));
                }
                case_dir = Some(PathBuf::from(value));
            }
        }
    }

    if action_sample {
        if case_dir.is_some()
            || rom_dir.is_some()
            || single_test.is_some()
            || include_cli
            || normalize_case
            || generate_cases
            || template_path.is_some()
        {
            return Err("--sample cannot be combined with benchmark run options".to_string());
        }
        return Ok(BenchAction::Sample);
    }

    if normalize_case
        && (rom_dir.is_some()
            || single_test.is_some()
            || include_cli
            || generate_cases
            || template_path.is_some())
    {
        return Err("--normalize-case cannot be combined with other benchmark actions".to_string());
    }

    if template_path.is_some() && !generate_cases {
        return Err("--template requires --generate-cases".to_string());
    }

    if generate_cases && rom_dir.is_none() {
        return Err("--generate-cases requires --rom-dir".to_string());
    }

    if generate_cases && (single_test.is_some() || include_cli) {
        return Err("--generate-cases cannot be combined with benchmark run options".to_string());
    }

    if rom_dir.is_some() && !generate_cases && (single_test.is_some() || include_cli) {
        return Err("--rom-dir cannot be combined with benchmark run options".to_string());
    }

    if normalize_case {
        let Some(case_dir) = case_dir else {
            return Err("<case-dir> is required".to_string());
        };
        return Ok(BenchAction::NormalizeCase { case_dir });
    }

    if generate_cases {
        let Some(case_dir) = case_dir else {
            return Err("<case-dir> is required".to_string());
        };
        return Ok(BenchAction::GenerateCases {
            case_dir,
            rom_dir: rom_dir.expect("rom dir was validated above"),
            template_path,
        });
    }

    if let Some(rom_dir) = rom_dir {
        let Some(case_dir) = case_dir else {
            return Err("<case-dir> is required".to_string());
        };
        return Ok(BenchAction::RewriteRomDir { case_dir, rom_dir });
    }

    if case_dir.is_none() && single_test.is_none() {
        return Err("<case-dir> is required".to_string());
    }

    Ok(BenchAction::Run(BenchRunOptions {
        case_dir,
        single_test,
        include_cli,
    }))
}

fn write_sample_case<W>(workspace_root: &Path, output: &mut W) -> Result<(), String>
where
    W: Write,
{
    let output_dir = bench_output_dir(workspace_root);
    fs::create_dir_all(&output_dir).map_err(|error| {
        format!(
            "failed to create benchmark output directory {}: {error}",
            output_dir.display()
        )
    })?;
    let sample = output_dir.join(DEFAULT_SAMPLE_NAME);
    if sample.exists() {
        writeln!(output, "sample already exists: {}", sample.display()).map_err(io_error)
    } else {
        fs::write(&sample, DEFAULT_TEMPLATE)
            .map_err(|error| format!("failed to write sample {}: {error}", sample.display()))?;
        writeln!(output, "wrote {}", sample.display()).map_err(io_error)
    }
}

fn rewrite_rom_dir<W>(case_dir: &Path, rom_dir: &Path, output: &mut W) -> Result<(), String>
where
    W: Write,
{
    let cases = case_files(case_dir)?;
    if cases.is_empty() {
        return Err(format!(
            "no benchmark cases found in {}",
            case_dir.display()
        ));
    }

    let mut updated = 0;
    for case_path in cases {
        let text = fs::read_to_string(&case_path)
            .map_err(|error| format!("failed to read {}: {error}", case_path.display()))?;
        let Some(rom) = top_level_string_value(&text, "rom") else {
            writeln!(
                output,
                "warning: no rom = entry found in {}",
                relative_display(&case_path, case_dir)
            )
            .map_err(io_error)?;
            continue;
        };
        let basename = portable_file_name(&rom);
        if basename.is_empty() {
            writeln!(
                output,
                "warning: {} has an empty ROM basename; skipped",
                case_path.display()
            )
            .map_err(io_error)?;
            continue;
        }
        let next_rom = rom_dir.join(basename);
        let (next_text, changed) = replace_top_level_string_value(
            &text,
            "rom",
            &next_rom.display().to_string(),
            InsertMissing::No,
        );
        if changed {
            fs::write(&case_path, next_text)
                .map_err(|error| format!("failed to write {}: {error}", case_path.display()))?;
            updated += 1;
            writeln!(output, "updated {}", relative_display(&case_path, case_dir))
                .map_err(io_error)?;
        }
    }

    writeln!(output, "updated {updated} benchmark case(s)").map_err(io_error)
}

fn normalize_cases<W>(case_dir: &Path, output: &mut W) -> Result<(), String>
where
    W: Write,
{
    let cases = case_files(case_dir)?;
    if cases.is_empty() {
        return Err(format!(
            "no benchmark cases found in {}",
            case_dir.display()
        ));
    }

    let mut renamed = 0;
    let mut unchanged = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();

    for case_path in cases {
        let text = fs::read_to_string(&case_path)
            .map_err(|error| format!("failed to read {}: {error}", case_path.display()))?;
        let Some(rom) = top_level_string_value(&text, "rom") else {
            writeln!(
                output,
                "warning: no top-level rom = entry found in {}; skipped",
                case_path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .unwrap_or("<unknown>")
            )
            .map_err(io_error)?;
            skipped += 1;
            continue;
        };
        let target_name = normalized_case_name(&rom);
        let target_path = case_dir.join(&target_name);
        if case_path.file_name() == Some(OsStr::new(&target_name)) {
            unchanged += 1;
            writeln!(output, "unchanged {target_name}").map_err(io_error)?;
            continue;
        }
        if target_path.exists() {
            errors.push(format!(
                "cannot rename {} to {}; target already exists",
                case_path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .unwrap_or("<unknown>"),
                target_name
            ));
            continue;
        }
        let source_name = case_path
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("<unknown>")
            .to_string();
        fs::rename(&case_path, &target_path).map_err(|error| {
            format!(
                "failed to rename {} to {}: {error}",
                case_path.display(),
                target_path.display()
            )
        })?;
        renamed += 1;
        writeln!(output, "renamed {source_name} -> {target_name}").map_err(io_error)?;
    }

    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    writeln!(
        output,
        "renamed {renamed}, unchanged {unchanged}, skipped {skipped} benchmark case(s)"
    )
    .map_err(io_error)
}

fn generate_benchmark_cases<W>(
    case_dir: &Path,
    rom_dir: &Path,
    template_path: Option<&Path>,
    output: &mut W,
) -> Result<(), String>
where
    W: Write,
{
    let template_text = if let Some(template_path) = template_path {
        fs::read_to_string(template_path).map_err(|error| {
            format!(
                "failed to read template {}: {error}",
                template_path.display()
            )
        })?
    } else {
        DEFAULT_TEMPLATE.to_string()
    };

    let roms = rom_files(rom_dir)?;
    if roms.is_empty() {
        return Err(format!(
            "no .gb or .gbc ROMs found in {}",
            rom_dir.display()
        ));
    }

    let mut targets: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut errors = Vec::new();
    for rom_path in roms {
        let stem = rom_path
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("game");
        let target_path = case_dir.join(format!("{stem}.toml"));
        if let Some((_, previous)) = targets.iter().find(|(target, _)| target == &target_path) {
            errors.push(format!(
                "ROMs {} and {} both normalize to {}",
                previous.display(),
                rom_path.display(),
                target_path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .unwrap_or("<unknown>")
            ));
        }
        targets.push((target_path, rom_path));
    }
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    targets.sort_by(|left, right| {
        left.0
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .cmp(
                &right
                    .0
                    .file_name()
                    .and_then(OsStr::to_str)
                    .unwrap_or_default()
                    .to_ascii_lowercase(),
            )
    });

    let mut created = 0;
    let mut updated = 0;
    let mut unchanged = 0;
    for (target_path, rom_path) in targets {
        let rendered = render_case_from_template(&template_text, &rom_path)?;
        if target_path.exists() {
            let current = fs::read_to_string(&target_path)
                .map_err(|error| format!("failed to read {}: {error}", target_path.display()))?;
            if current == rendered {
                unchanged += 1;
                writeln!(
                    output,
                    "unchanged {}",
                    target_path
                        .file_name()
                        .and_then(OsStr::to_str)
                        .unwrap_or("<unknown>")
                )
                .map_err(io_error)?;
            } else {
                fs::write(&target_path, rendered).map_err(|error| {
                    format!("failed to write {}: {error}", target_path.display())
                })?;
                updated += 1;
                writeln!(
                    output,
                    "updated {}",
                    target_path
                        .file_name()
                        .and_then(OsStr::to_str)
                        .unwrap_or("<unknown>")
                )
                .map_err(io_error)?;
            }
        } else {
            fs::write(&target_path, rendered)
                .map_err(|error| format!("failed to write {}: {error}", target_path.display()))?;
            created += 1;
            writeln!(
                output,
                "wrote {}",
                target_path
                    .file_name()
                    .and_then(OsStr::to_str)
                    .unwrap_or("<unknown>")
            )
            .map_err(io_error)?;
        }
    }

    writeln!(
        output,
        "created {created}, updated {updated}, unchanged {unchanged} benchmark case(s)"
    )
    .map_err(io_error)
}

fn run_benchmark_cases<W>(
    workspace_root: &Path,
    options: &ResolvedRunOptions,
    output: &mut W,
) -> Result<(), String>
where
    W: Write,
{
    let benchmark_dir = bench_output_dir(workspace_root);
    fs::create_dir_all(&benchmark_dir).map_err(|error| {
        format!(
            "failed to create benchmark output directory {}: {error}",
            benchmark_dir.display()
        )
    })?;

    let cases = filter_valid_cases(&options.cases, output)?;
    if cases.is_empty() {
        writeln!(
            output,
            "warning: no benchmark cases with readable ROMs found; nothing to run"
        )
        .map_err(io_error)?;
        generate_index(
            &benchmark_dir,
            &options.case_dir,
            options.include_cli,
            output,
        )?;
        return Ok(());
    }

    build_frontends(workspace_root, options.include_cli, output)?;

    fs::create_dir_all(benchmark_dir.join(GB_DESKTOP_FRONTEND)).map_err(|error| {
        format!(
            "failed to create {}: {error}",
            benchmark_dir.join(GB_DESKTOP_FRONTEND).display()
        )
    })?;
    if options.include_cli {
        fs::create_dir_all(benchmark_dir.join(GB_CLI_FRONTEND)).map_err(|error| {
            format!(
                "failed to create {}: {error}",
                benchmark_dir.join(GB_CLI_FRONTEND).display()
            )
        })?;
    }

    let gb_cli_bin = workspace_binary(workspace_root, GB_CLI_FRONTEND);
    let gb_desktop_bin = workspace_binary(workspace_root, GB_DESKTOP_FRONTEND);
    for case_path in cases {
        writeln!(output, "==> {}", case_label(&options.case_dir, &case_path)).map_err(io_error)?;
        if options.include_cli {
            writeln!(output, "--> gb-cli").map_err(io_error)?;
            run_frontend(
                &gb_cli_bin,
                &["run", "--test-runner", "--benchmark"],
                &case_path,
                &benchmark_dir,
            )?;
        }
        writeln!(output, "--> gb-desktop").map_err(io_error)?;
        run_frontend(
            &gb_desktop_bin,
            &["--test-runner", "--benchmark"],
            &case_path,
            &benchmark_dir,
        )?;
    }

    generate_index(
        &benchmark_dir,
        &options.case_dir,
        options.include_cli,
        output,
    )
}

fn resolve_run_options(
    options: BenchRunOptions,
    current_dir: &Path,
) -> Result<ResolvedRunOptions, String> {
    if let Some(case_dir) = options.case_dir {
        let case_dir = resolve_existing_dir(&case_dir, current_dir, "<case-dir>")?;
        let cases = if let Some(single_test) = options.single_test {
            vec![resolve_single_test(&single_test, &case_dir, current_dir)?]
        } else {
            let cases = case_files(&case_dir)?;
            if cases.is_empty() {
                return Err(format!(
                    "no benchmark cases found in {}",
                    case_dir.display()
                ));
            }
            cases
        };
        Ok(ResolvedRunOptions {
            case_dir,
            cases,
            include_cli: options.include_cli,
        })
    } else {
        let single_test = options
            .single_test
            .expect("parse requires --test when case_dir is absent");
        let single_test = resolve_existing_file(&single_test, current_dir, "benchmark test")?;
        let case_dir = single_test
            .parent()
            .ok_or_else(|| {
                format!(
                    "benchmark test has no parent directory: {}",
                    single_test.display()
                )
            })?
            .to_path_buf();
        Ok(ResolvedRunOptions {
            case_dir,
            cases: vec![single_test],
            include_cli: options.include_cli,
        })
    }
}

fn resolve_single_test(
    requested: &Path,
    case_dir: &Path,
    current_dir: &Path,
) -> Result<PathBuf, String> {
    if requested.is_absolute() && requested.is_file() {
        return canonicalize_lossy(requested);
    }

    let case_relative = case_dir.join(requested);
    if case_relative.is_file() {
        return canonicalize_lossy(&case_relative);
    }

    let cwd_relative = current_dir.join(requested);
    if cwd_relative.is_file() {
        return canonicalize_lossy(&cwd_relative);
    }

    Err(format!("benchmark test not found: {}", requested.display()))
}

fn filter_valid_cases<W>(cases: &[PathBuf], output: &mut W) -> Result<Vec<PathBuf>, String>
where
    W: Write,
{
    let mut valid_cases = Vec::new();
    let mut skipped = 0;
    for case_path in cases {
        match validate_case_rom(case_path) {
            Ok(()) => valid_cases.push(case_path.clone()),
            Err((rom_path, error)) => {
                let rom_display = rom_path
                    .map(|path| format!(" ({})", path.display()))
                    .unwrap_or_default();
                writeln!(
                    output,
                    "warning: skipping {}{}: {error}",
                    case_path
                        .file_name()
                        .and_then(OsStr::to_str)
                        .unwrap_or("<unknown>"),
                    rom_display
                )
                .map_err(io_error)?;
                skipped += 1;
            }
        }
    }
    writeln!(
        output,
        "validated {}/{} benchmark case ROM(s); skipped {skipped}",
        valid_cases.len(),
        cases.len()
    )
    .map_err(io_error)?;
    Ok(valid_cases)
}

fn validate_case_rom(case_path: &Path) -> Result<(), (Option<PathBuf>, String)> {
    let suite = load_benchmark_suite(case_path).map_err(|error| (None, error.to_string()))?;
    let rom_path = suite.rom;
    if !rom_path.is_file() {
        return Err((
            Some(rom_path),
            "ROM does not exist or is not a file".to_string(),
        ));
    }
    let metadata = rom_path
        .metadata()
        .map_err(|error| (Some(rom_path.clone()), format!("cannot stat ROM: {error}")))?;
    if metadata.len() == 0 {
        return Err((Some(rom_path), "ROM is empty".to_string()));
    }
    let mut file = fs::File::open(&rom_path)
        .map_err(|error| (Some(rom_path.clone()), format!("cannot read ROM: {error}")))?;
    let mut byte = [0_u8; 1];
    file.read_exact(&mut byte)
        .map_err(|error| (Some(rom_path), format!("cannot read ROM: {error}")))?;
    Ok(())
}

fn generate_index<W>(
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

fn build_index_report(
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
    let stats_path = benchmark_dir
        .join(frontend)
        .join(format!("{artifact_id}-stats.toml"));
    let image_path = benchmark_dir
        .join(frontend)
        .join(format!("{artifact_id}.png"));
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

fn build_frontends<W>(
    workspace_root: &Path,
    include_cli: bool,
    output: &mut W,
) -> Result<(), String>
where
    W: Write,
{
    let mut command = Command::new("cargo");
    command
        .current_dir(workspace_root)
        .arg("build")
        .arg("--profile")
        .arg("release-max");
    if include_cli {
        command.arg("-p").arg("gb-cli");
    }
    command.arg("-p").arg("gb-desktop");
    writeln!(output, "building benchmark frontend(s)").map_err(io_error)?;
    let status = command
        .status()
        .map_err(|error| format!("failed to run cargo build: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("cargo build failed with status {status}"))
    }
}

fn run_frontend(
    binary: &Path,
    arguments: &[&str],
    case_path: &Path,
    benchmark_dir: &Path,
) -> Result<(), String> {
    let mut command = Command::new(binary);
    command
        .current_dir(benchmark_dir)
        .args(arguments)
        .arg(case_path);
    let status = command.status().map_err(|error| {
        format!(
            "failed to run benchmark frontend {}: {error}",
            binary.display()
        )
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "benchmark frontend {} failed with status {status}",
            binary.display()
        ))
    }
}

fn render_case_from_template(template: &str, rom_path: &Path) -> Result<String, String> {
    let absolute_rom = canonicalize_lossy(rom_path)?;
    let id = safe_id(
        rom_path
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("game"),
    );
    let model = model_for_rom(rom_path)?;
    let (text, _) = replace_top_level_string_value(template, "id", &id, InsertMissing::Yes);
    let (text, _) = replace_top_level_string_value(
        &text,
        "rom",
        &absolute_rom.display().to_string(),
        InsertMissing::Yes,
    );
    let (mut text, _) =
        replace_top_level_string_value(&text, "model", model.as_str(), InsertMissing::Yes);
    if !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}

fn replace_top_level_string_value(
    text: &str,
    key: &str,
    value: &str,
    insert_missing: InsertMissing,
) -> (String, bool) {
    let mut output = String::new();
    let mut changed = false;
    let mut found = false;
    let mut inserted = false;
    for line in text.split_inclusive('\n') {
        let (body, newline) = split_line_newline(line);
        let trimmed = body.trim_start();
        if !inserted && trimmed.starts_with('[') && insert_missing == InsertMissing::Yes {
            output.push_str(&format!("{key} = {}\n", toml_string(value)));
            inserted = true;
            changed = true;
        }
        if !found && !trimmed.starts_with('[') && top_level_assignment_key(body) == Some(key) {
            let comment = line_comment(body).unwrap_or_default();
            output.push_str(&format!("{key} = {}{comment}{newline}", toml_string(value)));
            found = true;
            changed = true;
        } else {
            output.push_str(line);
        }
    }
    if !found && !inserted && insert_missing == InsertMissing::Yes {
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(&format!("{key} = {}\n", toml_string(value)));
        changed = true;
    }
    (output, changed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InsertMissing {
    No,
    Yes,
}

fn top_level_string_value(text: &str, key: &str) -> Option<String> {
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            break;
        }
        if top_level_assignment_key(raw_line) == Some(key) {
            let value = raw_line.split_once('=')?.1;
            return parse_toml_string(value);
        }
    }
    None
}

fn top_level_assignment_key(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let (key, _) = trimmed.split_once('=')?;
    let key = key.trim();
    if !key.is_empty()
        && key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        Some(key)
    } else {
        None
    }
}

fn parse_toml_string(value: &str) -> Option<String> {
    let value = value.split('#').next().unwrap_or_default().trim();
    if value.is_empty() {
        return None;
    }
    #[derive(Deserialize)]
    struct InlineString {
        value: String,
    }
    toml::from_str::<InlineString>(&format!("value = {value}"))
        .ok()
        .map(|parsed| parsed.value)
}

fn line_comment(body: &str) -> Option<&str> {
    body.find('#').map(|index| &body[index..])
}

fn split_line_newline(line: &str) -> (&str, &str) {
    if let Some(body) = line.strip_suffix('\n') {
        if let Some(body) = body.strip_suffix('\r') {
            (body, "\r\n")
        } else {
            (body, "\n")
        }
    } else {
        (line, "")
    }
}

fn toml_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn model_for_rom(path: &Path) -> Result<BenchmarkModel, String> {
    match path
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase)
    {
        Some(extension) if extension == "gb" => Ok(BenchmarkModel::Dmg),
        Some(extension) if extension == "gbc" => Ok(BenchmarkModel::Cgb),
        _ => Err(format!("unsupported ROM suffix in {}", path.display())),
    }
}

fn case_files(case_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut cases = fs::read_dir(case_dir)
        .map_err(|error| {
            format!(
                "failed to read case directory {}: {error}",
                case_dir.display()
            )
        })?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            format!(
                "failed to read case directory {}: {error}",
                case_dir.display()
            )
        })?;
    cases.retain(|path| path.is_file() && path.extension().and_then(OsStr::to_str) == Some("toml"));
    cases.sort_by(|left, right| {
        left.file_name()
            .and_then(OsStr::to_str)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .cmp(
                &right
                    .file_name()
                    .and_then(OsStr::to_str)
                    .unwrap_or_default()
                    .to_ascii_lowercase(),
            )
    });
    Ok(cases)
}

fn rom_files(rom_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut roms = Vec::new();
    collect_rom_files(rom_dir, &mut roms)?;
    roms.sort_by(|left, right| {
        left.display()
            .to_string()
            .to_ascii_lowercase()
            .cmp(&right.display().to_string().to_ascii_lowercase())
    });
    Ok(roms)
}

fn collect_rom_files(dir: &Path, roms: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir)
        .map_err(|error| format!("failed to read ROM directory {}: {error}", dir.display()))?
    {
        let path = entry
            .map_err(|error| format!("failed to read ROM directory {}: {error}", dir.display()))?
            .path();
        if path.is_dir() {
            collect_rom_files(&path, roms)?;
        } else if path.is_file() && is_rom_path(&path) {
            roms.push(path);
        }
    }
    Ok(())
}

fn is_rom_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(OsStr::to_str)
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("gb" | "gbc")
    )
}

fn resolve_existing_dir(path: &Path, current_dir: &Path, label: &str) -> Result<PathBuf, String> {
    let path = absolutize(path, current_dir);
    if !path.is_dir() {
        return Err(format!("{label} is not a directory: {}", path.display()));
    }
    canonicalize_lossy(&path)
}

fn resolve_or_create_dir(path: &Path, current_dir: &Path, label: &str) -> Result<PathBuf, String> {
    let path = absolutize(path, current_dir);
    fs::create_dir_all(&path)
        .map_err(|error| format!("failed to create {label} {}: {error}", path.display()))?;
    resolve_existing_dir(&path, current_dir, label)
}

fn resolve_existing_file(path: &Path, current_dir: &Path, label: &str) -> Result<PathBuf, String> {
    let path = absolutize(path, current_dir);
    if !path.is_file() {
        return Err(format!("{label} not found: {}", path.display()));
    }
    canonicalize_lossy(&path)
}

fn absolutize(path: &Path, current_dir: &Path) -> PathBuf {
    let path = expand_tilde(path);
    if path.is_absolute() {
        path
    } else {
        current_dir.join(path)
    }
}

fn expand_tilde(path: &Path) -> PathBuf {
    let text = path.display().to_string();
    if text == "~" {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| path.to_path_buf());
    }
    if let Some(rest) = text.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home).join(rest);
    }
    path.to_path_buf()
}

fn canonicalize_lossy(path: &Path) -> Result<PathBuf, String> {
    path.canonicalize()
        .map_err(|error| format!("failed to resolve {}: {error}", path.display()))
}

fn bench_output_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join(BENCH_OUTPUT_DIR)
}

fn workspace_binary(workspace_root: &Path, name: &str) -> PathBuf {
    workspace_root
        .join("target")
        .join("release-max")
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

fn default_workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should be two levels above gb-benchmark")
        .to_path_buf()
}

fn safe_id(stem: &str) -> String {
    let mut slug = String::new();
    let mut last_was_separator = false;
    for ch in stem.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            slug.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator && !slug.is_empty() {
            slug.push('-');
            last_was_separator = true;
        }
    }
    while slug.ends_with('-') || slug.ends_with('_') {
        slug.pop();
    }
    if slug.is_empty() {
        "game".to_string()
    } else {
        slug
    }
}

fn portable_file_name(value: &str) -> &str {
    value.rsplit(['/', '\\']).next().unwrap_or(value)
}

fn normalized_case_name(rom: &str) -> String {
    let basename = portable_file_name(rom);
    let stem = basename
        .rsplit_once('.')
        .map(|(stem, _)| stem)
        .unwrap_or(basename);
    format!("{stem}.toml")
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

fn case_label(case_dir: &Path, case_path: &Path) -> String {
    relative_display(case_path, case_dir)
}

fn relative_display(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
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

fn write_all<W>(output: &mut W, text: &str) -> Result<(), String>
where
    W: Write,
{
    output.write_all(text.as_bytes()).map_err(io_error)
}

fn io_error(error: std::io::Error) -> String {
    error.to_string()
}

#[cfg(test)]
pub(super) fn run_bench_command_with_workspace_for_test<I, S, W>(
    arguments: I,
    workspace_root: &Path,
    current_dir: &Path,
    output: &mut W,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: Write,
{
    run_bench_command_with_workspace(arguments, workspace_root, current_dir, output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parse_arguments_accepts_run_and_maintenance_modes() {
        assert_eq!(parse_bench_arguments(["--sample"]), Ok(BenchAction::Sample));
        assert_eq!(
            parse_bench_arguments(["cases", "--gb-cli", "--test", "one.toml"]),
            Ok(BenchAction::Run(BenchRunOptions {
                case_dir: Some(PathBuf::from("cases")),
                single_test: Some(PathBuf::from("one.toml")),
                include_cli: true,
            }))
        );
        assert_eq!(
            parse_bench_arguments(["cases", "--rom-dir", "roms", "--generate-cases"]),
            Ok(BenchAction::GenerateCases {
                case_dir: PathBuf::from("cases"),
                rom_dir: PathBuf::from("roms"),
                template_path: None,
            })
        );
    }

    #[test]
    fn parse_arguments_rejects_invalid_combinations() {
        assert_eq!(
            parse_bench_arguments(["--sample", "cases"]).expect_err("sample combinations fail"),
            "--sample cannot be combined with benchmark run options"
        );
        assert_eq!(
            parse_bench_arguments(["cases", "--template", "case.toml"])
                .expect_err("template without generation fails"),
            "--template requires --generate-cases"
        );
        assert_eq!(
            parse_bench_arguments(["cases", "--rom-dir", "roms", "--gb-cli"])
                .expect_err("rom dir cannot combine with runs"),
            "--rom-dir cannot be combined with benchmark run options"
        );
    }

    #[test]
    fn sample_is_written_under_test_bench() {
        let root = temp_root("sample");
        let mut output = Vec::new();
        write_sample_case(&root, &mut output).expect("sample should write");

        assert!(root.join("test/bench/game.toml").is_file());
        let output = String::from_utf8(output).expect("output should be utf-8");
        assert!(output.contains("test/bench/game.toml"));
    }

    #[test]
    fn generate_cases_uses_rom_suffix_for_model_and_safe_id() {
        let root = temp_root("generate");
        let case_dir = root.join("cases");
        let rom_dir = root.join("roms");
        fs::create_dir_all(&case_dir).expect("case dir should create");
        fs::create_dir_all(&rom_dir).expect("rom dir should create");
        fs::write(rom_dir.join("Dr Mario.gb"), [0_u8]).expect("rom should write");
        fs::write(rom_dir.join("Zelda.gbc"), [0_u8]).expect("rom should write");

        let mut output = Vec::new();
        generate_benchmark_cases(&case_dir, &rom_dir, None, &mut output)
            .expect("cases should generate");

        let dmg = fs::read_to_string(case_dir.join("Dr Mario.toml")).expect("DMG case exists");
        assert!(dmg.contains("id = \"dr-mario\""));
        assert!(dmg.contains("model = \"DMG\""));
        let cgb = fs::read_to_string(case_dir.join("Zelda.toml")).expect("CGB case exists");
        assert!(cgb.contains("model = \"CGB\""));
    }

    #[test]
    fn normalize_and_rewrite_cases_use_top_level_rom() {
        let root = temp_root("normalize");
        let case_dir = root.join("cases");
        let rom_dir = root.join("next-roms");
        fs::create_dir_all(&case_dir).expect("case dir should create");
        fs::create_dir_all(&rom_dir).expect("rom dir should create");
        fs::write(
            case_dir.join("old.toml"),
            "version = 1\nid = \"old\"\nrom = \"/old/Dr Mario.gb\" # keep\nmodel = \"DMG\"\n\n[[run]]\nid = \"idle\"\nduration_seconds = 1\n",
        )
        .expect("case should write");

        let mut output = Vec::new();
        normalize_cases(&case_dir, &mut output).expect("case should normalize");
        assert!(case_dir.join("Dr Mario.toml").is_file());

        rewrite_rom_dir(&case_dir, &rom_dir, &mut output).expect("rom dir should rewrite");
        let text = fs::read_to_string(case_dir.join("Dr Mario.toml")).expect("case should read");
        assert!(text.contains(&format!(
            "rom = \"{}\"# keep",
            rom_dir.join("Dr Mario.gb").display()
        )));
    }

    #[test]
    fn filter_valid_cases_skips_missing_and_empty_roms() {
        let root = temp_root("filter");
        let case_dir = root.join("cases");
        fs::create_dir_all(&case_dir).expect("case dir should create");
        let good_rom = root.join("good.gb");
        let empty_rom = root.join("empty.gb");
        fs::write(&good_rom, [0_u8]).expect("good rom should write");
        fs::write(&empty_rom, []).expect("empty rom should write");
        let good_case = case_dir.join("good.toml");
        let empty_case = case_dir.join("empty.toml");
        write_case(&good_case, "good", &good_rom);
        write_case(&empty_case, "empty", &empty_rom);

        let mut output = Vec::new();
        let valid = filter_valid_cases(&[good_case.clone(), empty_case], &mut output)
            .expect("filtering should succeed");

        assert_eq!(valid, vec![good_case]);
        let output = String::from_utf8(output).expect("output should be utf-8");
        assert!(output.contains("validated 1/2 benchmark case ROM(s); skipped 1"));
        assert!(output.contains("ROM is empty"));
    }

    #[test]
    fn askama_index_escapes_case_data_and_supports_cli_columns() {
        let root = temp_root("index");
        let benchmark_dir = root.join("test/bench");
        let case_dir = root.join("cases");
        fs::create_dir_all(benchmark_dir.join("gb-cli")).expect("cli dir should create");
        fs::create_dir_all(benchmark_dir.join("gb-desktop")).expect("desktop dir should create");
        fs::create_dir_all(&case_dir).expect("case dir should create");
        let rom = root.join("evil<&>.gb");
        fs::write(&rom, [0_u8]).expect("rom should write");
        let case = case_dir.join("case.toml");
        write_case(&case, "evil", &rom);
        for frontend in [GB_CLI_FRONTEND, GB_DESKTOP_FRONTEND] {
            fs::write(
                benchmark_dir
                    .join(frontend)
                    .join("evil-idle-stats.toml"),
                format!(
                    "version = 1\nfrontend = \"{frontend}\"\nid = \"evil\"\nartifact_id = \"evil-idle\"\nrom = \"{}\"\nmodel = \"DMG\"\nstartup = \"custom-boot\"\nmode = \"permissive\"\ntest_runner = true\nduration_seconds = 1\ntarget_frames = 60\ncompleted_frames = 60\nelapsed_seconds = 1.0\nfps = 60.0\nspeed_percent = 100.0\n",
                    rom.display()
                ),
            )
            .expect("stats should write");
            fs::write(benchmark_dir.join(frontend).join("evil-idle.png"), [0_u8])
                .expect("image should write");
        }

        let report = build_index_report(&benchmark_dir, &case_dir, true)
            .expect("report should build")
            .render()
            .expect("report should render");

        assert!(report.contains("evil&#60;&#38;&#62;.gb"));
        assert!(!report.contains("evil<&>.gb"));
        assert!(report.contains("<th>gb-cli</th>"));
        assert!(report.contains("<th>gb-desktop</th>"));
        assert!(report.contains("60.00 FPS<br>100.0%"));
    }

    #[test]
    fn command_helper_uses_workspace_context_for_sample() {
        let root = temp_root("command");
        let mut output = Vec::new();
        run_bench_command_with_workspace_for_test(["--sample"], &root, &root, &mut output)
            .expect("sample command should succeed");
        assert!(root.join("test/bench/game.toml").is_file());
    }

    fn write_case(path: &Path, id: &str, rom: &Path) {
        fs::write(
            path,
            format!(
                "version = 1\nid = \"{id}\"\nrom = \"{}\"\nmodel = \"DMG\"\nstartup = \"custom-boot\"\nmode = \"permissive\"\n\n[[run]]\nid = \"idle\"\nduration_seconds = 1\n",
                rom.display()
            ),
        )
        .expect("case should write");
    }

    fn temp_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "gb-benchmark-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("temp root should create");
        root
    }
}
