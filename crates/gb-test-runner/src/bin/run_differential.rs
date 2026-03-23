use std::process;

fn run<I, S, W>(arguments: I, output: &mut W) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: std::io::Write,
{
    gb_test_runner::run_differential_command(arguments, output)
}

fn main() {
    if let Err(message) = run(std::env::args().skip(1), &mut std::io::stdout()) {
        eprintln!("{message}");
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn run_help_succeeds() {
        let mut output = Vec::new();
        super::run(["--help"], &mut output).expect("help should succeed");
        let output = String::from_utf8(output).expect("help output should be utf-8");
        assert!(output.contains("Usage:"));
    }
}
