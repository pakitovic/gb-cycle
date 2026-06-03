use super::*;

#[test]
fn framebuffer_artifact_defaults_to_pgm_when_path_is_not_png() {
    let encoded = encode_framebuffer_artifact(
        Path::new("framebuffer.pgm"),
        &[0, 1, 2, 3],
        None,
        None,
        None,
    )
    .expect("PGM encoding should succeed");

    assert!(encoded.starts_with(b"P5\n160 144\n3\n"));
}

#[test]
fn run_cli_command_sgb_border_off_captures_lcd_sized_png() {
    let temp_dir = unique_temp_dir("sgb-border-off");
    fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");
    let rom_path = temp_dir.join("sgb.gb");
    let bordered_path = temp_dir.join("bordered.png");
    let borderless_path = temp_dir.join("borderless.png");
    fs::write(&rom_path, build_nop_loop_rom()).expect("test ROM should be writable");

    run_cli_command(
        [
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--model",
            "SGB",
            "--tcycles",
            "1",
            "--framebuffer-out",
            bordered_path.to_str().expect("path should be valid UTF-8"),
        ],
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .expect("default SGB framebuffer output should include host border");
    let bordered_info = decode_png_info(&fs::read(&bordered_path).expect("PNG should exist"));
    assert_eq!(bordered_info.width, SGB_HOST_FRAMEBUFFER_WIDTH as u32);
    assert_eq!(bordered_info.height, SGB_HOST_FRAMEBUFFER_HEIGHT as u32);
    assert_eq!(bordered_info.color_type, png::ColorType::Rgb);

    run_cli_command(
        [
            "run",
            rom_path.to_str().expect("path should be valid UTF-8"),
            "--model",
            "SGB2",
            "--border-off",
            "--tcycles",
            "1",
            "--framebuffer-out",
            borderless_path
                .to_str()
                .expect("path should be valid UTF-8"),
        ],
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .expect("SGB2 framebuffer output should support hidden host border");
    let borderless_info = decode_png_info(&fs::read(&borderless_path).expect("PNG should exist"));
    assert_eq!(borderless_info.width, FRAMEBUFFER_WIDTH as u32);
    assert_eq!(borderless_info.height, FRAMEBUFFER_HEIGHT as u32);
    assert_eq!(borderless_info.color_type, png::ColorType::Rgb);

    fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
}
