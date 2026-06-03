use super::super::*;

#[test]
fn binary_run_executes_a_rom_and_streams_serial_stdout() {
    let temp_dir = unique_temp_dir("run");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

    let rom_path = temp_dir.join("serial.gb");
    fs::write(&rom_path, build_single_byte_serial_rom(b'Z')).expect("test ROM should be writable");

    let output = Command::new(env!("CARGO_BIN_EXE_gb-cli"))
        .args([
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--tcycles",
            "10000",
            "--serial-stdout",
        ])
        .output()
        .expect("gb-cli binary should run");

    assert!(output.status.success());
    assert_eq!(output.stdout, b"Z");
    assert!(
        String::from_utf8(output.stderr)
            .expect("stderr should be UTF-8")
            .contains("serial_bytes=1")
    );

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}
