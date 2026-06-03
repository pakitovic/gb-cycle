pub(crate) mod help;
pub(crate) mod parse;

use crate::command::help::{INSPECT_HELP_TEXT, RUN_HELP_TEXT, SAVES_HELP_TEXT, general_help_text};
use crate::command::parse::parse_cli_arguments;
use crate::host_io::write_text;
use crate::inspect_rom::inspect_rom_command;
use crate::options::{BenchmarkRunOptions, InspectRomOptions, RunOptions, SavesOptions};
use crate::run::{run_benchmark_command, run_command};
use crate::saves::saves_command;
use std::io::Write;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CliAction {
    ShowGeneralHelp,
    ShowRunHelp,
    ShowInspectHelp,
    ShowSavesHelp,
    Run(Box<RunOptions>),
    RunBenchmark(BenchmarkRunOptions),
    InspectRom(InspectRomOptions),
    Saves(SavesOptions),
}

pub(crate) fn run_cli_command<I, S>(
    arguments: I,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    match parse_cli_arguments(arguments)? {
        CliAction::ShowGeneralHelp => write_text(stdout, general_help_text()),
        CliAction::ShowRunHelp => write_text(stdout, RUN_HELP_TEXT),
        CliAction::ShowInspectHelp => write_text(stdout, INSPECT_HELP_TEXT),
        CliAction::ShowSavesHelp => write_text(stdout, SAVES_HELP_TEXT),
        CliAction::Run(options) => run_command(*options, stdout, stderr),
        CliAction::RunBenchmark(options) => run_benchmark_command(options, stdout, stderr),
        CliAction::InspectRom(options) => inspect_rom_command(options, stdout),
        CliAction::Saves(options) => saves_command(options, stdout, stderr),
    }
}
