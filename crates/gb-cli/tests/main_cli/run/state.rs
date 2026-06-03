use super::super::*;

#[test]
fn binary_run_saves_and_loads_machine_save_state_artifacts() {
    let temp_dir = unique_temp_dir("state-artifacts");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("state.gb");
    let state_path = temp_dir.join("states/slot1.gbstate");
    let continued_state_path = temp_dir.join("states/slot2.gbstate");
    fs::write(&rom_path, build_nop_loop_rom()).expect("test ROM should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_gb-cli"))
        .args([
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--tcycles",
            "64",
            "--state-out",
            state_path.to_str().expect("path should be valid UTF-8"),
        ])
        .output()
        .expect("gb-cli binary should run");

    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .expect("stderr should be UTF-8")
            .contains("state_out=")
    );
    let first_state_bytes = fs::read(&state_path).expect(".gbstate should be written");
    let first_state =
        decode_machine_save_state_envelope(&first_state_bytes).expect(".gbstate should decode");

    let output = Command::new(env!("CARGO_BIN_EXE_gb-cli"))
        .args([
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--tcycles",
            "64",
            "--state-in",
            state_path.to_str().expect("path should be valid UTF-8"),
            "--state-out",
            continued_state_path
                .to_str()
                .expect("path should be valid UTF-8"),
        ])
        .output()
        .expect("gb-cli binary should run");

    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("state_in="));
    assert!(stderr.contains("state_out="));
    let continued_state_bytes =
        fs::read(&continued_state_path).expect("continued .gbstate should be written");
    let continued_state = decode_machine_save_state_envelope(&continued_state_bytes)
        .expect("continued .gbstate should decode");
    assert!(
        continued_state.state.metadata().next_t_cycle > first_state.state.metadata().next_t_cycle,
        "state-in run should continue from the restored machine state"
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn binary_run_rejects_corrupt_machine_save_state_artifacts() {
    let temp_dir = unique_temp_dir("state-corrupt");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("state.gb");
    let corrupt_state_path = temp_dir.join("corrupt.gbstate");
    fs::write(&rom_path, build_nop_loop_rom()).expect("test ROM should be writable");
    fs::write(&corrupt_state_path, b"not-a-gbstate").expect("corrupt state should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_gb-cli"))
        .args([
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--tcycles",
            "1",
            "--state-in",
            corrupt_state_path
                .to_str()
                .expect("path should be valid UTF-8"),
        ])
        .output()
        .expect("gb-cli binary should run");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");
    assert!(
        String::from_utf8(output.stderr)
            .expect("stderr should be UTF-8")
            .contains("failed to decode .gbstate state")
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}
