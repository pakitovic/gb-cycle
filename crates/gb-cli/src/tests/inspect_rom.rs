use super::*;

#[test]
fn inspect_rom_reports_the_supported_nomcb_header_shape() {
    let temp_dir = unique_temp_dir("inspect-rom");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("inspect.gb");
    fs::write(&rom_path, build_single_byte_serial_rom(b'O')).expect("test ROM should be writable");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_cli_command(
        [
            "inspect-rom",
            rom_path.to_str().expect("path should be valid UTF-8"),
        ],
        &mut stdout,
        &mut stderr,
    )
    .expect("inspect-rom should succeed");

    let output = String::from_utf8(stdout).expect("inspect output should be UTF-8");
    assert!(output.contains("load_status=ok"));
    assert!(output.contains("mapper_name=ROM ONLY"));
    assert!(output.contains("selection=supported"));
    assert!(output.contains("effective_rom_size_bytes=32768"));
    assert!(output.contains("effective_rom_bank_count=2"));
    assert!(output.contains("rom_size_source=declared-exact"));
    assert!(stderr.is_empty());

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn inspect_rom_reports_effective_layout_for_permissive_mbc5_size_metadata() {
    let temp_dir = unique_temp_dir("inspect-rom-mbc5-effective");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("permissive-mbc5.gbc");
    let mut rom = build_test_rom_with_header(&[0x00], 0x19, 0x00, 0x00);
    rom.resize(64 * 1024, 0xFF);
    fs::write(&rom_path, rom).expect("test ROM should be writable");

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_cli_command(
        [
            "inspect-rom",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--mode",
            "permissive",
        ],
        &mut stdout,
        &mut stderr,
    )
    .expect("inspect-rom should succeed");

    let output = String::from_utf8(stdout).expect("inspect output should be UTF-8");
    assert!(output.contains("load_status=ok"));
    assert!(output.contains("mapper_name=MBC5"));
    assert!(output.contains("rom_size_bytes=32768"));
    assert!(output.contains("rom_bank_count=2"));
    assert!(output.contains("effective_rom_size_bytes=65536"));
    assert!(output.contains("effective_rom_bank_count=4"));
    assert!(output.contains("rom_size_source=permissive-rounded-actual"));
    assert!(output.contains("diagnostic=warning MBC5 declared a 32768-byte ROM"));
    assert!(stderr.is_empty());

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn inspect_rom_command_covers_rejected_and_header_error_paths() {
    let temp_dir = unique_temp_dir("inspect-rejected");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("unsupported.gb");
    let mut rom = build_test_rom_with_header(&[0x00], 0xFD, 0x55, 0x06);
    rom[0x0143] = 0xAA;
    rom[0x0146] = 0x7F;
    fs::write(&rom_path, rom).expect("unsupported ROM should be writable");

    let mut output = Vec::new();
    inspect_rom_command(
        InspectRomOptions {
            rom_path: rom_path.clone(),
            execution_mode: ExecutionMode::Strict,
        },
        &mut output,
    )
    .expect("unsupported headers should still inspect successfully");
    let text = String::from_utf8(output).expect("inspect output should be UTF-8");
    assert!(text.contains("load_status=rejected"));
    assert!(text.contains("selection=unsupported-accessory"));
    assert!(text.contains("rejection_reason="));
    assert!(text.contains("cgb_flag=supported-noncanonical(0xAA)"));
    assert!(text.contains("sgb_flag=unknown(0x7F)"));
    assert!(text.contains("rom_size_bytes=unknown"));
    assert!(text.contains("effective_rom_size_bytes=unknown"));
    assert!(text.contains("effective_rom_bank_count=unknown"));
    assert!(text.contains("rom_size_source=unknown"));
    assert!(text.contains("ram_size_bytes=unknown"));

    let tiny_path = temp_dir.join("tiny.gb");
    fs::write(&tiny_path, [0x00; 4]).expect("tiny ROM should be writable");
    let error = inspect_rom_command(
        InspectRomOptions {
            rom_path: tiny_path,
            execution_mode: ExecutionMode::Strict,
        },
        &mut Vec::new(),
    )
    .expect_err("tiny ROMs should fail header parsing");
    assert!(error.contains("ROM image is too small to contain a cartridge header"));

    let missing_error = inspect_rom_command(
        InspectRomOptions {
            rom_path: temp_dir.join("missing.gb"),
            execution_mode: ExecutionMode::Strict,
        },
        &mut Vec::new(),
    )
    .expect_err("missing ROMs should surface read errors");
    assert!(missing_error.contains("failed to read ROM"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn inspect_rom_command_surfaces_output_writer_failures() {
    let temp_dir = unique_temp_dir("inspect-writer-failures");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("inspect.gb");
    fs::write(&rom_path, build_single_byte_serial_rom(b'I')).expect("test ROM should be writable");

    for fail_on_write in [5, 9, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31] {
        let mut output = FailOnWrite {
            fail_on_write: Some(fail_on_write),
            ..FailOnWrite::default()
        };
        let error = inspect_rom_command(
            InspectRomOptions {
                rom_path: rom_path.clone(),
                execution_mode: ExecutionMode::Strict,
            },
            &mut output,
        )
        .expect_err("inspect output write failures should surface");
        assert!(error.contains("failed to write output"));
    }

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}
