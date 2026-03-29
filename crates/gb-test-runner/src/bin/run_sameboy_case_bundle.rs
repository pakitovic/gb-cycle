use std::env;
use std::io;

fn main() {
    let arguments = env::args().skip(1);
    let output = &mut io::stdout();

    if let Err(message) = gb_test_runner::run_sameboy_case_bundle_command(arguments, output) {
        eprintln!("{message}");
        std::process::exit(1);
    }
}
