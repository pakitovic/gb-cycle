use std::process;

fn run<I, S, W>(arguments: I, output: &mut W) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: std::io::Write,
{
    gb_test_runner::run_first_divergence_command(arguments, output)
}

fn exit_code<I, S, W, E>(arguments: I, output: &mut W, error_output: &mut E) -> i32
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    W: std::io::Write,
    E: std::io::Write,
{
    match run(arguments, output) {
        Ok(()) => 0,
        Err(message) => {
            let _ = writeln!(error_output, "{message}");
            1
        }
    }
}

fn main() {
    let code = exit_code(
        std::env::args().skip(1),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    );
    if code != 0 {
        process::exit(code);
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

    #[test]
    fn exit_code_returns_one_and_writes_parse_errors() {
        let mut output = Vec::new();
        let mut error = Vec::new();
        let code = super::exit_code(
            [
                "--oracle",
                "sameboy",
                "--suite",
                "ashiepaws-dmg-curated",
                "--probe-interval-tcycles",
                "nope",
            ],
            &mut output,
            &mut error,
        );

        assert_eq!(code, 1);
        assert!(output.is_empty());
        assert!(
            String::from_utf8(error)
                .expect("stderr should be utf-8")
                .contains("invalid --probe-interval-tcycles")
        );
    }
}
