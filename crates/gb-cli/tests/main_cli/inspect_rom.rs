use super::*;

#[test]
fn binary_inspect_rom_surfaces_header_parse_failures() {
    let temp_dir = unique_temp_dir("inspect");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("small.gb");
    fs::write(&rom_path, [0x00; 4]).expect("tiny ROM should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_gb-cli"))
        .args([
            "inspect-rom",
            rom_path.to_str().expect("path should be valid UTF-8"),
        ])
        .output()
        .expect("gb-cli binary should run");

    assert!(!output.status.success());
    assert_eq!(output.stdout, b"");
    assert!(
        String::from_utf8(output.stderr)
            .expect("stderr should be UTF-8")
            .contains("error: ROM image is too small to contain a cartridge header")
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}

#[test]
fn binary_inspect_rom_reports_supported_header_details() {
    let temp_dir = unique_temp_dir("inspect-supported");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("warn.gb");
    let mut rom = build_test_rom_with_header(&[0x00], 0x08, 0x00, 0x02);
    set_header_flags(&mut rom, 0x80, 0x03);
    fs::write(&rom_path, rom).expect("supported ROM should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_gb-cli"))
        .args([
            "inspect-rom",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--mode",
            "experimental",
        ])
        .output()
        .expect("gb-cli binary should run");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert!(stdout.contains("selection=supported"));
    assert!(stdout.contains("cgb_flag=supported"));
    assert!(stdout.contains("sgb_flag=supported"));
    assert!(stdout.contains("diagnostic_count=1"));

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}
