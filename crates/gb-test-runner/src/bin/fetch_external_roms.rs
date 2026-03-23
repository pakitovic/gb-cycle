use std::process;

fn run<I, S, W>(
    workspace_root: &std::path::Path,
    arguments: I,
    output: &mut W,
) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: std::io::Write,
{
    gb_test_runner::run_fetch_external_roms_command(arguments, workspace_root, output)
}

fn main() {
    let workspace_root = gb_test_runner::default_workspace_root();
    if let Err(message) = run(
        &workspace_root,
        std::env::args().skip(1),
        &mut std::io::stdout(),
    ) {
        eprintln!("{message}");
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn run_help_succeeds_without_manifest() {
        let mut output = Vec::new();
        super::run(std::path::Path::new("."), ["--help"], &mut output)
            .expect("help should succeed");
        let output = String::from_utf8(output).expect("help output should be utf-8");
        assert!(output.contains("Usage:"));
    }
}
