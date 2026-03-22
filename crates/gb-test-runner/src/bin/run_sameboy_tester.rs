use std::process;

fn main() {
    if let Err(message) =
        gb_test_runner::run_sameboy_tester_command(std::env::args().skip(1), &mut std::io::stdout())
    {
        eprintln!("{message}");
        process::exit(1);
    }
}
