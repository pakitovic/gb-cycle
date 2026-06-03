use super::super::*;

#[test]
fn binary_run_with_artifacts_and_persistence_covers_headless_paths() {
    let temp_dir = unique_temp_dir("run-artifacts");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("battery.gb");
    let serial_path = temp_dir.join("artifacts/serial.bin");
    let framebuffer_path = temp_dir.join("artifacts/framebuffer.pgm");
    let trace_path = temp_dir.join("artifacts/trace.txt");
    let save_root = temp_dir.join("saves");
    fs::write(
        &rom_path,
        build_battery_backed_serial_and_ram_rom(b'Q', 0x5A),
    )
    .expect("battery-backed ROM should be writable");

    fs::create_dir_all(&save_root).expect("save root should be creatable");
    let save_key =
        gb_persistence::CartridgeSaveKey::new("battery").expect("save key should be valid");
    let mut store = FilesystemCartridgeSaveStore::new(&save_root);
    store
        .save(
            &save_key,
            gb_core::CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: false,
                profile: gb_core::CartridgePersistenceProfile::PersistentRam {
                    ram: gb_core::CartridgeRamPayloadKind::Linear { byte_len: 8 * 1024 },
                },
            },
            &PersistentCartState::NoMbcRam {
                ram: vec![0x22; 8 * 1024],
            },
        )
        .expect("seed save should persist");

    let output = Command::new(env!("CARGO_BIN_EXE_gb-cli"))
        .args([
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--model",
            "MGB",
            "--startup",
            "skip-boot",
            "--mode",
            "experimental",
            "--frames",
            "1",
            "--tcycles",
            "10000",
            "--serial-out",
            serial_path.to_str().expect("path should be valid UTF-8"),
            "--framebuffer-out",
            framebuffer_path
                .to_str()
                .expect("path should be valid UTF-8"),
            "--palette",
            "grey",
            "--trace-out",
            trace_path.to_str().expect("path should be valid UTF-8"),
            "--save-dir",
            save_root.to_str().expect("path should be valid UTF-8"),
            "--save-policy",
            "on-close",
        ])
        .output()
        .expect("gb-cli binary should run");

    assert!(output.status.success());
    assert_eq!(
        fs::read(&serial_path).expect("serial artifact should exist"),
        b"Q"
    );
    assert!(
        fs::read(&framebuffer_path)
            .expect("framebuffer artifact should exist")
            .starts_with(b"P5\n160 144\n3\n")
    );
    assert!(
        fs::read_to_string(&trace_path)
            .expect("trace artifact should exist")
            .contains("t_cycle=")
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be UTF-8");
    assert!(stderr.contains("model=MGB"));
    assert!(stderr.contains("startup=skip-boot"));
    assert!(stderr.contains("mode=experimental"));
    assert!(stderr.contains("save_loaded path="));
    assert!(stderr.contains("save_writes=1"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}
