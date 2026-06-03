use super::*;

#[test]
fn printer_output_helpers_cover_error_and_overflow_paths() {
    let root = temp_printer_root("helper-errors");
    let blocking_dir = root.join("existing-dir");
    fs::create_dir_all(&blocking_dir).expect("blocking directory should exist");
    let blocking_file = root.join("not-a-directory");
    fs::write(&blocking_file, b"occupied").expect("blocking file should exist");
    let rendered = render_printed_page(&sample_printed_page(0xE4));

    let save_error = save_rendered_printer_page_png(&rendered, &blocking_dir)
        .expect_err("writing a PNG into a directory should fail");
    assert!(save_error.contains("failed to create printer output file"));

    let create_dir_error = resolve_next_printer_output_path(None, blocking_file.as_path())
        .expect_err("treating a file-like current dir parent as a directory should fail");
    assert!(create_dir_error.contains("failed to create printer output directory"));

    let overflow_error = scale_printer_dimension(u32::MAX, "printer dimension overflow")
        .expect_err("overflowing printer dimensions should be rejected");
    assert!(overflow_error.contains("printer dimension overflow"));

    let io_passthrough = png_encoding_io_error(png::EncodingError::IoError(std::io::Error::other(
        "io passthrough",
    )));
    assert_eq!(io_passthrough.kind(), std::io::ErrorKind::Other);
    assert_eq!(io_passthrough.to_string(), "io passthrough");

    fs::remove_dir_all(root).expect("printer root should be removable");
}
