use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should be two levels above gb-test-runner")
        .to_path_buf()
}

fn executable_name(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

fn binary_candidate(dir: &Path, name: &str) -> Option<PathBuf> {
    let candidate = dir.join(executable_name(name));
    candidate.is_file().then_some(candidate)
}

fn active_target_debug_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().and_then(Path::parent).map(Path::to_path_buf))
}

fn binary_path(name: &str) -> PathBuf {
    std::env::var_os(format!("CARGO_BIN_EXE_{name}"))
        .map(PathBuf::from)
        // Coverage runs can build sibling binaries under a target-dir-specific
        // debug root without exporting runtime CARGO_BIN_EXE_* variables.
        .or_else(|| active_target_debug_dir().and_then(|dir| binary_candidate(&dir, name)))
        .or_else(|| binary_candidate(&workspace_root().join("target/debug"), name))
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

fn unique_temp_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "gb-cycle-binaries-{}-{}-{}",
        label,
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos()
    ))
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
fn run_determinism_binary_handles_help_and_parse_errors() {
    assert_help_and_parse_error(
        "run_determinism",
        &["--suite", "phase-2-cpu-timing", "--save-at-tcycles", "nope"],
        "invalid --save-at-tcycles",
    );
}

#[test]
fn run_first_divergence_binary_handles_help_and_parse_errors() {
    assert_help_and_parse_error(
        "run_first_divergence",
        &[
            "--oracle",
            "sameboy",
            "--suite",
            "hacktix-dmg-curated",
            "--probe-interval-tcycles",
            "nope",
        ],
        "invalid --probe-interval-tcycles",
    );
}

#[test]
fn run_linked_session_binary_handles_help_and_parse_errors() {
    assert_help_and_parse_error(
        "run_linked_session",
        &["--session"],
        "--session requires a value",
    );
}

#[test]
fn run_linked_session_binary_executes_manifest_backed_suites() {
    let binary = binary_path("run_linked_session");
    let manifest_path = workspace_root().join("crates/gb-test-runner/data/linked-dmg04-smoke.toml");

    let output = Command::new(&binary)
        .current_dir(workspace_root())
        .args([
            "--manifest",
            manifest_path
                .to_str()
                .expect("manifest path should be utf-8"),
        ])
        .output()
        .unwrap_or_else(|error| {
            panic!("failed to spawn run_linked_session manifest case: {error}")
        });

    assert!(
        output.status.success(),
        "manifest-backed linked session should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(stdout.contains("suite=linked-dmg04-smoke"));
    assert!(stdout.contains("session=dmg04-basic-exchange outcome=PASS"));
}

#[test]
fn run_linked_session_binary_returns_non_zero_and_writes_failure_artifacts() {
    let binary = binary_path("run_linked_session");
    let temp_dir = unique_temp_dir("linked-session-failure");
    let artifact_root = temp_dir.join("artifacts");
    let manifest_path = temp_dir.join("fixture-mismatch.toml");
    let wrong_fixture_path = temp_dir.join("wrong.snapshot");
    std::fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    std::fs::write(&wrong_fixture_path, "definitely wrong\n")
        .expect("wrong fixture should be writable");

    let left_rom =
        workspace_root().join("crates/gb-test-runner/data/fixtures/linked/dmg04/basic-left.gb");
    let right_rom =
        workspace_root().join("crates/gb-test-runner/data/fixtures/linked/dmg04/basic-right.gb");
    let manifest = format!(
        concat!(
            "version = 1\n",
            "suite_name = \"linked-fixture-mismatch\"\n",
            "family = \"serial-ext\"\n",
            "subsystem = \"serial\"\n\n",
            "[[session]]\n",
            "id = \"dmg04-basic-exchange\"\n",
            "topology = \"dmg04\"\n",
            "timeout_tcycles = 5000\n",
            "oracle = \"linked-snapshot-fixture\"\n",
            "fixture = \"{}\"\n\n",
            "  [[session.participant]]\n",
            "  id = \"left\"\n",
            "  rom = \"{}\"\n\n",
            "  [[session.participant]]\n",
            "  id = \"right\"\n",
            "  rom = \"{}\"\n"
        ),
        wrong_fixture_path.display(),
        left_rom.display(),
        right_rom.display(),
    );
    std::fs::write(&manifest_path, manifest).expect("manifest should be writable");

    let output = Command::new(&binary)
        .current_dir(workspace_root())
        .args([
            "--manifest",
            manifest_path
                .to_str()
                .expect("manifest path should be utf-8"),
            "--failure-artifact-root",
            artifact_root
                .to_str()
                .expect("artifact root should be utf-8"),
        ])
        .output()
        .unwrap_or_else(|error| panic!("failed to spawn run_linked_session failure case: {error}"));

    assert!(
        !output.status.success(),
        "fixture mismatch should fail with non-zero exit status"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(stderr.contains("one or more linked sessions failed"));

    let session_dir = artifact_root.join("dmg04-basic-exchange");
    assert!(session_dir.join("linked_snapshot.txt").is_file());
    assert!(session_dir.join("left_snapshot.txt").is_file());
    assert!(session_dir.join("right_snapshot.txt").is_file());

    std::fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
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
