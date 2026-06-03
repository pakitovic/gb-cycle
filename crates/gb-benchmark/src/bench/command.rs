use std::io::Write;
use std::path::Path;

use super::args::{BenchAction, bench_help_text, parse_bench_arguments};
use super::cases::{generate_benchmark_cases, normalize_cases, rewrite_rom_dir, write_sample_case};
use super::paths::{
    default_workspace_root, resolve_existing_dir, resolve_existing_file, resolve_or_create_dir,
    write_all,
};
use super::run::{resolve_run_options, run_benchmark_cases};

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
