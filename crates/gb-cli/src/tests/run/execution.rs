use super::super::*;

#[test]
fn run_command_emits_requested_artifacts_and_persists_battery_backed_ram() {
    let temp_dir = unique_temp_dir("run-artifacts");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path =
        temp_dir.join("Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).gb");
    let serial_path = temp_dir.join("artifacts/serial.bin");
    let framebuffer_path = temp_dir.join("artifacts/framebuffer.png");
    let trace_path = temp_dir.join("artifacts/trace.txt");
    let save_root = temp_dir.join("saves");
    fs::write(
        &rom_path,
        build_battery_backed_serial_and_ram_rom(b'R', 0x5A),
    )
    .expect("test ROM should be writable");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_cli_command(
        [
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--tcycles",
            "10000",
            "--serial-out",
            serial_path.to_str().expect("path should be valid UTF-8"),
            "--framebuffer-out",
            framebuffer_path
                .to_str()
                .expect("path should be valid UTF-8"),
            "--trace-out",
            trace_path.to_str().expect("path should be valid UTF-8"),
            "--save-dir",
            save_root.to_str().expect("path should be valid UTF-8"),
        ],
        &mut stdout,
        &mut stderr,
    )
    .expect("run command should succeed");

    assert!(
        stdout.is_empty(),
        "serial was written to a file, not stdout"
    );
    assert_eq!(
        fs::read(&serial_path).expect("serial output should exist"),
        b"R"
    );
    let framebuffer = fs::read(&framebuffer_path).expect("framebuffer should exist");
    assert!(framebuffer.starts_with(b"\x89PNG\r\n\x1A\n"));
    let info = decode_png_info(&framebuffer);
    assert_eq!(info.width, FRAMEBUFFER_WIDTH as u32);
    assert_eq!(info.height, FRAMEBUFFER_HEIGHT as u32);
    assert_eq!(info.color_type, png::ColorType::Grayscale);
    let trace = fs::read_to_string(&trace_path).expect("trace should exist");
    assert!(trace.contains("t_cycle="));

    let save_key = derive_save_key(&rom_path).expect("save key should derive");
    assert_eq!(
        fs::read(save_root.join(format!("{}.sav", save_key.as_str())))
            .expect("external-primary save should exist")[0],
        0x5A
    );

    let stderr_output = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(stderr_output.contains("save_writes=1"));
    assert!(stderr_output.contains("serial_bytes=1"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn run_command_streams_serial_stdout_in_summary_mode_and_reports_missing_roms() {
    let temp_dir = unique_temp_dir("summary-run");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("serial.gb");
    let framebuffer_path = temp_dir.join("summary/framebuffer.pgm");
    fs::write(&rom_path, build_single_byte_serial_rom(b'Z')).expect("test ROM should be writable");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut options = RunOptions::default_with_rom(rom_path.clone());
    options.serial_stdout = true;
    options.framebuffer_out = Some(framebuffer_path.clone());
    options.display_palette = Some(RunDisplayPalette::Grey);
    options.tcycle_limit = Some(10_000);
    run_command(options, &mut stdout, &mut stderr).expect("run command should succeed");

    assert_eq!(stdout, b"Z");
    assert!(
        fs::read(&framebuffer_path)
            .expect("framebuffer should exist")
            .starts_with(b"P5\n160 144\n255\n")
    );
    assert!(
        String::from_utf8(stderr)
            .expect("stderr should be UTF-8")
            .contains("serial_bytes=1")
    );

    let missing = run_command(
        RunOptions::default_with_rom(PathBuf::from("this-rom-does-not-exist.gb")),
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .expect_err("missing ROMs should fail");
    assert!(missing.contains("failed to read ROM"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn run_command_covers_on_write_frame_flush_and_manual_save_policy() {
    let temp_dir = unique_temp_dir("run-save-policies");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("battery.gb");
    fs::write(
        &rom_path,
        build_battery_backed_serial_and_ram_rom(b'W', 0x5A),
    )
    .expect("battery-backed ROM should be writable");

    let on_write_root = temp_dir.join("on-write");
    let mut on_write = RunOptions::default_with_rom(rom_path.clone());
    on_write.save_dir = Some(on_write_root.clone());
    on_write.save_policy = SavePolicy::OnWrite;
    on_write.frame_limit = Some(1);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_command(on_write, &mut stdout, &mut stderr).expect("on-write runs should succeed");
    assert!(stdout.is_empty());
    let stderr_text = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(stderr_text.contains("completed_frames=1"));
    assert!(stderr_text.contains("save_writes=1"));

    let manual_root = temp_dir.join("manual");
    let mut manual = RunOptions::default_with_rom(rom_path);
    manual.save_dir = Some(manual_root);
    manual.save_policy = SavePolicy::Manual;
    manual.frame_limit = Some(1);
    manual.framebuffer_out = Some(temp_dir.join("manual/framebuffer.pgm"));
    manual.trace_out = Some(temp_dir.join("manual/trace.txt"));
    let mut stderr = Vec::new();
    run_command(manual, &mut Vec::new(), &mut stderr).expect("manual runs should succeed");
    let stderr_text = String::from_utf8(stderr).expect("stderr should be UTF-8");
    assert!(stderr_text.contains("save_policy=manual"));
    assert!(stderr_text.contains("save_writes=0"));
    assert!(stderr_text.contains("framebuffer_out="));
    assert!(stderr_text.contains("trace_out="));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn run_command_surfaces_summary_save_and_session_writer_failures() {
    let temp_dir = unique_temp_dir("run-writer-failures");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let plain_rom_path = temp_dir.join("plain.gb");
    fs::write(&plain_rom_path, build_single_byte_serial_rom(b'P'))
        .expect("plain ROM should be writable");

    let mut serial_write_options = RunOptions::default_with_rom(plain_rom_path.clone());
    serial_write_options.serial_stdout = true;
    serial_write_options.tcycle_limit = Some(10_000);
    let mut stdout = FailOnWrite {
        fail_on_write: Some(1),
        ..FailOnWrite::default()
    };
    let serial_write_error = run_command(serial_write_options, &mut stdout, &mut Vec::new())
        .expect_err("serial stdout write failures should surface");
    assert!(serial_write_error.contains("failed to write serial stdout"));

    let mut serial_flush_options = RunOptions::default_with_rom(plain_rom_path.clone());
    serial_flush_options.serial_stdout = true;
    serial_flush_options.tcycle_limit = Some(10_000);
    let mut stdout = FailOnWrite {
        fail_on_flush: true,
        ..FailOnWrite::default()
    };
    let serial_flush_error = run_command(serial_flush_options, &mut stdout, &mut Vec::new())
        .expect_err("serial stdout flush failures should surface");
    assert!(serial_flush_error.contains("failed to flush serial stdout"));

    for fail_on_write in [5, 7] {
        let mut options = RunOptions::default_with_rom(plain_rom_path.clone());
        options.tcycle_limit = Some(0);
        let mut stderr = FailOnWrite {
            fail_on_write: Some(fail_on_write),
            ..FailOnWrite::default()
        };
        let error = run_command(options, &mut Vec::new(), &mut stderr)
            .expect_err("summary write failures should surface");
        assert!(error.contains("failed to write output"));
    }

    let mut framebuffer_options = RunOptions::default_with_rom(plain_rom_path.clone());
    framebuffer_options.tcycle_limit = Some(0);
    framebuffer_options.framebuffer_out = Some(temp_dir.join("framebuffer/frame.pgm"));
    let mut stderr = FailOnWrite {
        fail_on_write: Some(15),
        ..FailOnWrite::default()
    };
    let framebuffer_error = run_command(framebuffer_options, &mut Vec::new(), &mut stderr)
        .expect_err("framebuffer summary write failures should surface");
    assert!(framebuffer_error.contains("failed to write output"));

    let mut skipped_save_options = RunOptions::default_with_rom(plain_rom_path);
    skipped_save_options.tcycle_limit = Some(0);
    skipped_save_options.save_dir = Some(temp_dir.join("plain-saves"));
    let mut stderr = FailOnWrite {
        fail_on_write: Some(1),
        ..FailOnWrite::default()
    };
    let skipped_save_error = run_command(skipped_save_options, &mut Vec::new(), &mut stderr)
        .expect_err("save-session status write failures should surface");
    assert!(skipped_save_error.contains("failed to write output"));

    let battery_rom_path = temp_dir.join("battery.gb");
    fs::write(
        &battery_rom_path,
        build_battery_backed_serial_and_ram_rom(b'B', 0x11),
    )
    .expect("battery ROM should be writable");
    for fail_on_write in [17, 19, 21, 22] {
        let mut options = RunOptions::default_with_rom(battery_rom_path.clone());
        options.tcycle_limit = Some(0);
        options.save_dir = Some(temp_dir.join(format!("battery-saves-{fail_on_write}")));
        let mut stderr = FailOnWrite {
            fail_on_write: Some(fail_on_write),
            ..FailOnWrite::default()
        };
        let error = run_command(options, &mut Vec::new(), &mut stderr)
            .expect_err("save summary write failures should surface");
        assert!(error.contains("failed to write output"));
    }

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}
