use super::*;

#[test]
fn binary_help_prints_usage_and_exits_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_gb-cli"))
        .arg("--help")
        .output()
        .expect("gb-cli binary should run");

    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .expect("stdout should be UTF-8")
            .contains("Usage:\n  gb-cli run <rom> [options]")
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn binary_unknown_subcommands_fail_with_a_formatted_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_gb-cli"))
        .arg("unknown")
        .output()
        .expect("gb-cli binary should run");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");
    assert!(
        String::from_utf8(output.stderr)
            .expect("stderr should be UTF-8")
            .contains("error: unknown subcommand \"unknown\"; run `gb-cli --help` for usage")
    );
}
