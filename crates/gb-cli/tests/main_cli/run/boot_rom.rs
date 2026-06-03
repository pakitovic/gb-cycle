use super::super::*;

#[test]
fn binary_real_boot_warns_for_mismatched_boot_rom_assets() {
    let temp_dir = unique_temp_dir("real-boot");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("serial.gb");
    let boot_dir = temp_dir.join("bootroms");
    fs::write(&rom_path, build_single_byte_serial_rom(b'B')).expect("plain ROM should be writable");
    write_fake_boot_rom(&boot_dir, HardwareRevision::DmgCpuC, 0x00);

    let output = Command::new(env!("CARGO_BIN_EXE_gb-cli"))
        .args([
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--startup",
            "real-boot",
            "--boot-rom-dir",
            boot_dir.to_str().expect("path should be valid UTF-8"),
            "--boot-rom-verify",
            "warn",
            "--tcycles",
            "1",
        ])
        .output()
        .expect("gb-cli binary should run");

    assert!(output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .expect("stderr should be UTF-8")
            .contains("warning: boot ROM asset for Dmg")
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}
