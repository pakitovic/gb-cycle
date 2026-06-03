use std::ffi::OsStr;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{GB_CLI_FRONTEND, GB_DESKTOP_FRONTEND, load_benchmark_suite};

use super::args::BenchRunOptions;
use super::paths::{
    bench_output_dir, canonicalize_lossy, case_files, case_label, io_error, resolve_existing_dir,
    resolve_existing_file, workspace_binary,
};
use super::report::generate_index;

#[derive(Debug, Clone)]
pub(super) struct ResolvedRunOptions {
    pub(super) case_dir: PathBuf,
    pub(super) cases: Vec<PathBuf>,
    pub(super) include_cli: bool,
}

pub(super) fn run_benchmark_cases<W>(
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

pub(super) fn resolve_run_options(
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

pub(super) fn filter_valid_cases<W>(
    cases: &[PathBuf],
    output: &mut W,
) -> Result<Vec<PathBuf>, String>
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
