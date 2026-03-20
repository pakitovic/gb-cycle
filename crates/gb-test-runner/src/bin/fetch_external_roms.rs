use std::process;

fn main() {
    let workspace_root = gb_test_runner::default_workspace_root();
    if let Err(message) = gb_test_runner::run_fetch_external_roms_command(
        std::env::args().skip(1),
        &workspace_root,
        &mut std::io::stdout(),
    ) {
        eprintln!("{message}");
        process::exit(1);
    }
}
