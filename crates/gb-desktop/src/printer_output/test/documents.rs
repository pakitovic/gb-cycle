use super::*;

#[test]
fn flush_pending_document_saves_a_partial_multipart_strip() {
    let _guard = crate::lock_sdl_test();
    crate::configure_headless_sdl();
    let sdl = sdl3::init().expect("SDL should initialize for printer flushing");
    let video = sdl.video().expect("video subsystem should initialize");
    let main_window = video
        .window("printer-main", 160, 144)
        .build()
        .expect("main test window should build");
    let root = temp_printer_root("flush-pending");
    let rom_path = root.join("pokemon.gb");
    let mut output = PrinterOutputState::default();
    let first = sample_printed_page_with_margins(0xE4, 2, 0);

    output
        .handle_printed_page(&main_window, Some(&rom_path), root.as_path(), &first)
        .expect("first segment should start a multipart document");
    assert!(output.last_saved_path().is_none());

    output
        .flush_pending_document(&main_window, Some(&rom_path), root.as_path())
        .expect("explicit flush should save the partial strip");

    assert_eq!(output.latest_page_dimensions(), Some((4, 1)));
    assert!(
        output
            .last_saved_path()
            .expect("explicit flush should remember the saved strip")
            .exists()
    );

    fs::remove_dir_all(root).expect("printer root should be removable");
}

#[test]
fn resolve_next_printer_output_path_uses_current_dir_when_no_rom_is_loaded() {
    let root = temp_printer_root("fallback-current-dir");

    let first = resolve_next_printer_output_path(None, root.as_path())
        .expect("current directory fallback should resolve");
    assert_eq!(
        first.file_name().and_then(|name| name.to_str()),
        Some("gb-cycle-printer-0001.png")
    );
    assert_eq!(
        first.parent().expect("printer path should have a parent"),
        root.join("printer")
    );

    fs::write(&first, b"placeholder").expect("first printer output should be writable");
    let second = resolve_next_printer_output_path(None, root.as_path())
        .expect("second current directory fallback should resolve");
    assert_eq!(
        second.file_name().and_then(|name| name.to_str()),
        Some("gb-cycle-printer-0002.png")
    );

    fs::remove_dir_all(root).expect("printer root should be removable");
}
