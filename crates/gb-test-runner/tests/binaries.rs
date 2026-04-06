use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should be two levels above gb-test-runner")
        .to_path_buf()
}

fn binary_path(name: &str) -> PathBuf {
    std::env::var_os(format!("CARGO_BIN_EXE_{name}"))
        .map(PathBuf::from)
        .or_else(|| {
            let candidate = workspace_root().join("target/debug").join(name);
            candidate.is_file().then_some(candidate)
        })
        .unwrap_or_else(|| panic!("Cargo did not expose or build binary path for {name}"))
}

fn assert_help_and_parse_error(binary_name: &str, error_args: &[&str], error_fragment: &str) {
    let binary = binary_path(binary_name);

    let help = Command::new(&binary)
        .current_dir(workspace_root())
        .arg("--help")
        .output()
        .unwrap_or_else(|error| panic!("failed to spawn {binary_name} --help: {error}"));
    assert!(
        help.status.success(),
        "{binary_name} --help should succeed: stderr={}",
        String::from_utf8_lossy(&help.stderr)
    );
    assert!(
        String::from_utf8(help.stdout)
            .expect("help output should be utf-8")
            .contains("Usage"),
        "{binary_name} help output should contain Usage"
    );

    let error = Command::new(&binary)
        .current_dir(workspace_root())
        .args(error_args)
        .output()
        .unwrap_or_else(|error| panic!("failed to spawn {binary_name} parse error case: {error}"));
    assert!(
        !error.status.success(),
        "{binary_name} invalid invocation should fail"
    );
    assert!(
        String::from_utf8(error.stderr)
            .expect("stderr should be utf-8")
            .contains(error_fragment),
        "{binary_name} stderr should mention {error_fragment:?}"
    );
}

#[test]
fn fetch_test_roms_binary_handles_help_and_parse_errors() {
    assert_help_and_parse_error("fetch_test_roms", &["all", "blargg"], "cannot be combined");
}

#[test]
fn run_differential_binary_handles_help_and_parse_errors() {
    assert_help_and_parse_error(
        "run_differential",
        &["--oracle", "unknown", "--suite", "phase-2-cpu-timing"],
        "unknown oracle",
    );
}

#[test]
fn run_rom_suite_binary_handles_help_and_parse_errors() {
    assert_help_and_parse_error(
        "run_rom_suite",
        &["--timeout-frames", "nope"],
        "invalid --timeout-frames value",
    );
}

#[test]
fn run_sameboy_case_bundle_binary_handles_help_and_parse_errors() {
    assert_help_and_parse_error(
        "run_sameboy_case_bundle",
        &["--timeout-tcycles", "nope"],
        "invalid --timeout-tcycles value",
    );
}

#[test]
fn run_sameboy_tester_binary_handles_help_and_parse_errors() {
    assert_help_and_parse_error(
        "run_sameboy_tester",
        &["--image-format", "unknown", "--suite", "acid-dmg-curated"],
        "unknown image format",
    );
}
