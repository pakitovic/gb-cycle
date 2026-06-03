use super::super::*;

#[test]
fn run_command_saves_and_restores_machine_save_states() {
    let temp_dir = unique_temp_dir("run-machine-state");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom = build_nop_loop_rom();
    let rom_path = temp_dir.join("state.gb");
    let first_state_path = temp_dir.join("states/first.gbstate");
    let restored_state_path = temp_dir.join("states/restored.gbstate");
    fs::write(&rom_path, &rom).expect("test ROM should be writable");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_cli_command(
        [
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--tcycles",
            "64",
            "--state-out",
            first_state_path
                .to_str()
                .expect("path should be valid UTF-8"),
        ],
        &mut stdout,
        &mut stderr,
    )
    .expect("state-out run should succeed");
    assert!(stdout.is_empty());
    let first_state_bytes = fs::read(&first_state_path).expect(".gbstate should be created");
    decode_machine_save_state_envelope(&first_state_bytes).expect(".gbstate should decode");
    let first_stderr = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(first_stderr.contains("state_out="));

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_cli_command(
        [
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--tcycles",
            "64",
            "--state-in",
            first_state_path
                .to_str()
                .expect("path should be valid UTF-8"),
            "--state-out",
            restored_state_path
                .to_str()
                .expect("path should be valid UTF-8"),
        ],
        &mut stdout,
        &mut stderr,
    )
    .expect("state-in continuation run should succeed");
    assert!(stdout.is_empty());
    let restored = decode_machine_save_state_envelope(
        &fs::read(&restored_state_path).expect("restored .gbstate should exist"),
    )
    .expect("restored .gbstate should decode");
    let stderr_output = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(stderr_output.contains("state_in="));
    assert!(stderr_output.contains("state_out="));

    let mut uninterrupted = build_loaded_machine(rom, false);
    for _ in 0..128 {
        uninterrupted.step_t_cycle();
    }
    assert_eq!(restored.state, uninterrupted.capture_save_state());

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn run_command_rejects_incompatible_machine_save_states() {
    let temp_dir = unique_temp_dir("run-machine-state-mismatch");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("state.gb");
    let state_path = temp_dir.join("state.gbstate");
    fs::write(&rom_path, build_nop_loop_rom()).expect("test ROM should be writable");
    run_cli_command(
        [
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--tcycles",
            "16",
            "--state-out",
            state_path.to_str().expect("path should be valid UTF-8"),
        ],
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .expect("state-out seed run should succeed");

    let error = run_cli_command(
        [
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--model",
            "MGB",
            "--tcycles",
            "1",
            "--state-in",
            state_path.to_str().expect("path should be valid UTF-8"),
        ],
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .expect_err("model-incompatible state should fail restore");
    assert!(error.contains("failed to restore state"));
    assert!(error.to_ascii_lowercase().contains("model"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn run_command_state_in_uses_restored_cartridge_state_as_save_baseline() {
    let temp_dir = unique_temp_dir("run-machine-state-save-baseline");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom = build_battery_backed_serial_and_ram_rom(b'B', 0x11);
    let rom_path = temp_dir.join("battery.gb");
    let state_path = temp_dir.join("battery.gbstate");
    let save_root = temp_dir.join("saves");
    fs::write(&rom_path, &rom).expect("battery ROM should be writable");

    run_cli_command(
        [
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--tcycles",
            "256",
            "--state-out",
            state_path.to_str().expect("path should be valid UTF-8"),
        ],
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .expect("state-out seed run should succeed");

    let mut seeded_ram = vec![0xEE; 8 * 1024];
    seeded_ram[0] = 0xEE;
    let seed_machine = build_loaded_machine(rom, false);
    let save_key = derive_save_key(&rom_path).expect("save key should derive");
    let mut backend = FilesystemCartridgeSaveBackend::new(&save_root);
    backend
        .save(
            &save_key,
            seed_machine.cartridge().persistence_metadata(),
            &PersistentCartState::NoMbcRam { ram: seeded_ram },
        )
        .expect("pre-existing .gbsav should persist");

    let mut stderr = Vec::new();
    run_cli_command(
        [
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--tcycles",
            "1",
            "--state-in",
            state_path.to_str().expect("path should be valid UTF-8"),
            "--save-dir",
            save_root.to_str().expect("path should be valid UTF-8"),
            "--save-policy",
            "on-close",
        ],
        &mut Vec::new(),
        &mut stderr,
    )
    .expect("state-in run should skip pre-existing .gbsav restore");
    let stderr_output = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(
        !stderr_output.contains("save_loaded path="),
        "{stderr_output}"
    );
    assert!(stderr_output.contains("save_loaded_existing=false"));
    assert!(stderr_output.contains("save_writes=0"));
    let envelope = backend
        .load(&save_key)
        .expect("seed .gbsav should remain readable")
        .expect("seed .gbsav should still exist");
    match envelope.persistent_state {
        PersistentCartState::NoMbcRam { ram } => assert_eq!(ram[0], 0xEE),
        other => panic!("expected NoMbcRam persistence, got {other:?}"),
    }

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}
