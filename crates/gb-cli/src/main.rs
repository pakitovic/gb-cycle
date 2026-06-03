mod boot_rom;
mod command;
mod framebuffer;
mod host_io;
mod inspect_rom;
mod options;
mod report;
mod run;
mod save_key;
mod saves;

use std::env;
use std::io;
use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    match command::run_cli_command(env::args().skip(1), &mut stdout, &mut stderr) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(stderr, "error: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests;
