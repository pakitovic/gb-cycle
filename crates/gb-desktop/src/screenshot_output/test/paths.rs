use super::*;

#[test]
fn save_rendered_screenshot_png_writes_an_8bit_rgb_png() {
    let root = temp_screenshot_root("png");
    let output_path = root.join("shot.png");
    let rendered = RenderedScreenshot {
        width: 2,
        height: 1,
        rgb_pixels: vec![255, 255, 255, 0, 0, 0],
    };

    save_rendered_screenshot_png(&rendered, &output_path).expect("screenshot PNG should save");

    let encoded = fs::read(&output_path).expect("screenshot PNG should exist");
    let decoder = png::Decoder::new(Cursor::new(encoded));
    let mut reader = decoder.read_info().expect("PNG header should decode");
    let mut buffer = vec![
        0;
        reader
            .output_buffer_size()
            .expect("PNG output buffer size should fit in memory")
    ];
    let info = reader
        .next_frame(&mut buffer)
        .expect("PNG payload should decode");

    assert_eq!(info.width, 2);
    assert_eq!(info.height, 1);
    assert_eq!(info.color_type, ColorType::Rgb);
    assert_eq!(&buffer[..info.buffer_size()], &[255, 255, 255, 0, 0, 0]);

    fs::remove_dir_all(root).expect("temporary screenshot root should be removable");
}

#[test]
fn resolve_next_screenshot_output_path_falls_back_to_current_dir_without_a_rom() {
    let root = temp_screenshot_root("fallback-path");
    let expected_relative = PathBuf::from("screenshots").join("gb-cycle-0.png");

    let first = resolve_next_screenshot_output_path(None, root.as_path())
        .expect("launcher screenshot path should resolve");
    assert_eq!(
        first
            .strip_prefix(&root)
            .expect("path should live under the temp root"),
        expected_relative.as_path()
    );

    fs::write(&first, b"placeholder").expect("first launcher screenshot should be writable");
    let second = resolve_next_screenshot_output_path(None, root.as_path())
        .expect("second launcher screenshot path should resolve");
    assert_eq!(
        second.file_name().and_then(|name| name.to_str()),
        Some("gb-cycle-1.png")
    );

    fs::remove_dir_all(root).expect("temporary screenshot root should be removable");
}

#[test]
fn resolve_next_screenshot_output_path_reports_directory_creation_failures() {
    let root = temp_screenshot_root("path-error");
    let blocking_path = root.join("blocking");
    fs::write(&blocking_path, b"not-a-directory").expect("blocking file should be writable");

    let error = resolve_next_screenshot_output_path(None, blocking_path.as_path())
        .expect_err("non-directory screenshot root should fail");
    assert!(error.contains("failed to create screenshot output directory"));

    fs::remove_dir_all(root).expect("temporary screenshot root should be removable");
}
