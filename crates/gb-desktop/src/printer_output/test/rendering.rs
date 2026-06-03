use super::*;

#[test]
fn render_printed_page_applies_the_requested_printer_palette() {
    let rendered = render_printed_page(&sample_printed_page(0x1B));

    assert_eq!(rendered.width, 4);
    assert_eq!(rendered.height, 1);
    assert_eq!(
        rendered.grayscale_pixels,
        vec![
            printer_grayscale_shade(0x1B, 0),
            printer_grayscale_shade(0x1B, 1),
            printer_grayscale_shade(0x1B, 2),
            printer_grayscale_shade(0x1B, 3),
        ]
    );
}

#[test]
fn resolve_next_printer_output_path_uses_printer_subdirectory_and_unique_names() {
    let root = temp_printer_root("paths");
    let rom_path = root.join("pokemon.gb");

    let first = resolve_next_printer_output_path(Some(&rom_path), root.as_path())
        .expect("first printer output path should resolve");
    assert_eq!(
        first.file_name().and_then(|name| name.to_str()),
        Some("pokemon-printer-0001.png")
    );
    fs::create_dir_all(first.parent().expect("first path should have a parent"))
        .expect("printer output directory should be creatable");
    fs::write(&first, b"placeholder").expect("first printer output should be writable");

    let second = resolve_next_printer_output_path(Some(&rom_path), root.as_path())
        .expect("second printer output path should resolve");
    assert_eq!(
        second.file_name().and_then(|name| name.to_str()),
        Some("pokemon-printer-0002.png")
    );

    fs::remove_dir_all(root).expect("printer root should be removable");
}

#[test]
fn save_rendered_printer_page_png_writes_an_8bit_grayscale_png() {
    let root = temp_printer_root("png");
    let output_path = root.join("printer.png");
    let rendered = render_printed_page(&sample_printed_page(0xE4));

    save_rendered_printer_page_png(&rendered, &output_path)
        .expect("printer output PNG should save");

    let encoded = fs::read(&output_path).expect("printer output PNG should exist");
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

    assert_eq!(info.width, 4);
    assert_eq!(info.height, 1);
    assert_eq!(info.color_type, ColorType::Grayscale);

    fs::remove_dir_all(root).expect("printer root should be removable");
}
