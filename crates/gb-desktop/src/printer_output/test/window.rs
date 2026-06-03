use super::*;

#[test]
fn printer_output_state_recreates_the_window_after_close() {
    let _guard = crate::lock_sdl_test();
    crate::configure_headless_sdl();
    let sdl = sdl3::init().expect("SDL should initialize for printer output");
    let video = sdl.video().expect("video subsystem should initialize");
    let main_window = video
        .window("printer-main", 160, 144)
        .build()
        .expect("main test window should build");
    let page = sample_printed_page(0xE4);
    let root = temp_printer_root("window");
    let rom_path = root.join("printer.gb");
    let mut output = PrinterOutputState::default();

    output
        .handle_printed_page(&main_window, Some(&rom_path), root.as_path(), &page)
        .expect("first printer page should create a window");
    assert!(output.has_window());

    let window_id = output
        .window_id()
        .expect("printer window should expose an SDL id");
    assert!(
        output
            .handle_event(&Event::Window {
                timestamp: 0,
                window_id,
                win_event: WindowEvent::CloseRequested,
            })
            .expect("close event should succeed")
    );
    assert!(!output.has_window());

    output
        .handle_printed_page(&main_window, Some(&rom_path), root.as_path(), &page)
        .expect("second printer page should recreate the window");
    assert!(output.has_window());
    assert_eq!(output.latest_page_dimensions(), Some((4, 1)));
    assert!(
        Path::new(
            output
                .last_saved_path()
                .expect("printer output should remember the last save path")
        )
        .exists()
    );

    fs::remove_dir_all(root).expect("printer root should be removable");
}

#[test]
fn multipart_documents_with_zero_margins_are_stitched_into_one_png() {
    let _guard = crate::lock_sdl_test();
    crate::configure_headless_sdl();
    let sdl = sdl3::init().expect("SDL should initialize for multipart printer output");
    let video = sdl.video().expect("video subsystem should initialize");
    let main_window = video
        .window("printer-main", 160, 144)
        .build()
        .expect("main test window should build");
    let root = temp_printer_root("multipart");
    let rom_path = root.join("pokemon.gb");
    let mut output = PrinterOutputState::default();
    let first = sample_printed_page_with_margins(0xE4, 1, 0);
    let second = sample_printed_page_with_margins(0xE4, 0, 3);

    output
        .handle_printed_page(&main_window, Some(&rom_path), root.as_path(), &first)
        .expect("first segment should start a multipart document");
    assert!(output.has_window());
    assert_eq!(output.latest_page_dimensions(), Some((4, 1)));
    assert!(output.last_saved_path().is_none());
    assert!(!root.join("printer").exists());

    output
        .handle_printed_page(&main_window, Some(&rom_path), root.as_path(), &second)
        .expect("second segment should complete the multipart document");

    assert_eq!(output.latest_page_dimensions(), Some((4, 2)));
    let saved_path = output
        .last_saved_path()
        .expect("completed multipart document should save a PNG");
    assert!(saved_path.exists());

    let encoded = fs::read(saved_path).expect("multipart printer PNG should exist");
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
    assert_eq!(info.height, 2);

    fs::remove_dir_all(root).expect("printer root should be removable");
}
