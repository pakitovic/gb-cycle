use super::super::*;

#[test]
fn run_benchmark_command_expands_multi_run_case_artifacts() {
    let _guard = ENV_LOCK.lock().expect("env lock should not be poisoned");
    let temp_dir = unique_temp_dir("benchmark-multirun");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let _cwd = CurrentDirGuard::enter(&temp_dir);

    let rom_path = temp_dir.join("bench.gb");
    fs::write(&rom_path, build_nop_loop_rom()).expect("test ROM should be writable");
    let case_path = temp_dir.join("test/bench.toml");
    fs::create_dir_all(case_path.parent().expect("case path should have a parent"))
        .expect("case directory should be creatable");
    fs::write(
        &case_path,
        format!(
            r#"
version = 1
id = "bench"
rom = "{}"
model = "DMG"
startup = "custom-boot"
mode = "permissive"
screenshot = true
stats = true

[[run]]
id = "idle"
label = "Idle"
duration_seconds = 1

[[run]]
id = "tap"
label = "Tap A"
duration_seconds = 1

[[run.input]]
frame = 2
button = "a"
hold_frames = 3
"#,
            rom_path.display()
        ),
    )
    .expect("benchmark case should be writable");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_cli_command(
        vec![
            "run".to_string(),
            "--test-runner".to_string(),
            "--benchmark".to_string(),
            case_path.display().to_string(),
        ],
        &mut stdout,
        &mut stderr,
    )
    .expect("multi-run benchmark command should succeed through the CLI router");

    assert!(stdout.is_empty());
    assert!(temp_dir.join("gb-cli/bench-idle.png").exists());
    assert!(temp_dir.join("gb-cli/bench-tap.png").exists());
    let idle_stats = fs::read_to_string(temp_dir.join("gb-cli/bench-idle-stats.toml"))
        .expect("idle stats should exist");
    let tap_stats = fs::read_to_string(temp_dir.join("gb-cli/bench-tap-stats.toml"))
        .expect("tap stats should exist");
    assert!(idle_stats.contains("artifact_id = \"bench-idle\""));
    assert!(idle_stats.contains("run_id = \"idle\""));
    assert!(tap_stats.contains("artifact_id = \"bench-tap\""));
    assert!(tap_stats.contains("run_label = \"Tap A\""));
    assert!(
        String::from_utf8(stderr)
            .expect("stderr should be UTF-8")
            .contains("benchmark_stats_out=gb-cli/bench-tap-stats.toml")
    );

    drop(_cwd);
    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn run_benchmark_command_rejects_real_boot_without_explicit_boot_rom_dir() {
    let _guard = ENV_LOCK.lock().expect("env lock should not be poisoned");
    let temp_dir = unique_temp_dir("benchmark-real-boot-without-assets");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let _cwd = CurrentDirGuard::enter(&temp_dir);

    let rom_path = temp_dir.join("bench.gb");
    fs::write(&rom_path, build_nop_loop_rom()).expect("test ROM should be writable");
    let case_path = temp_dir.join("test/bench.toml");
    fs::create_dir_all(case_path.parent().expect("case path should have a parent"))
        .expect("case directory should be creatable");
    fs::write(
        &case_path,
        format!(
            r#"
version = 1
id = "bench"
rom = "{}"
model = "DMG"
startup = "real-boot"
mode = "strict"
screenshot = false
stats = false

[[run]]
id = "idle"
duration_seconds = 1
"#,
            rom_path.display()
        ),
    )
    .expect("benchmark case should be writable");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let error = run_cli_command(
        vec![
            "run".to_string(),
            "--benchmark".to_string(),
            case_path.display().to_string(),
        ],
        &mut stdout,
        &mut stderr,
    )
    .expect_err("real-boot benchmark runs should require explicit boot ROM assets");

    assert_eq!(
        error,
        "boot ROM root is not configured; use --boot-rom-dir <dir>"
    );
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());

    drop(_cwd);
    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn run_benchmark_command_stops_at_tcycle_budget_when_frames_freeze() {
    let _guard = ENV_LOCK.lock().expect("env lock should not be poisoned");
    let temp_dir = unique_temp_dir("benchmark-lcd-off");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let _cwd = CurrentDirGuard::enter(&temp_dir);

    let rom_path = temp_dir.join("lcd-off.gb");
    fs::write(&rom_path, build_lcd_off_loop_rom()).expect("test ROM should be writable");
    let case_path = temp_dir.join("test/lcd-off.toml");
    fs::create_dir_all(case_path.parent().expect("case path should have a parent"))
        .expect("case directory should be creatable");
    fs::write(
        &case_path,
        format!(
            r#"
version = 1
id = "lcd-off"
rom = "{}"
model = "DMG"
startup = "custom-boot"
mode = "permissive"
screenshot = false
stats = true

[[run]]
id = "budget"
duration_seconds = 1
"#,
            rom_path.display()
        ),
    )
    .expect("benchmark case should be writable");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_cli_command(
        vec![
            "run".to_string(),
            "--test-runner".to_string(),
            "--benchmark".to_string(),
            case_path.display().to_string(),
        ],
        &mut stdout,
        &mut stderr,
    )
    .expect("LCD-off benchmark should stop at the tcycle duration budget");

    assert!(stdout.is_empty());
    let stats = fs::read_to_string(temp_dir.join("gb-cli/lcd-off-budget-stats.toml"))
        .expect("stats should exist");
    assert!(stats.contains("artifact_id = \"lcd-off-budget\""));
    assert!(stats.contains("executed_tcycles = 4213440"));

    drop(_cwd);
    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}
