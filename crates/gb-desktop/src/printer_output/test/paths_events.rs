use super::*;

#[test]
fn printer_output_state_ignores_events_for_other_windows() {
    let _guard = crate::lock_sdl_test();
    crate::configure_headless_sdl();
    let sdl = sdl3::init().expect("SDL should initialize for printer event filtering");
    let video = sdl.video().expect("video subsystem should initialize");
    let main_window = video
        .window("printer-main", 160, 144)
        .build()
        .expect("main test window should build");
    let root = temp_printer_root("event-filter");
    let rom_path = root.join("printer.gb");
    let page = sample_printed_page(0xE4);
    let mut output = PrinterOutputState::default();

    output
        .handle_printed_page(&main_window, Some(&rom_path), root.as_path(), &page)
        .expect("printer page should create a window");
    let printer_window_id = output
        .window_id()
        .expect("printer window should expose an SDL id");

    assert!(
        !output
            .handle_event(&Event::Window {
                timestamp: 0,
                window_id: printer_window_id + 1,
                win_event: WindowEvent::Exposed,
            })
            .expect("other windows should be ignored")
    );
    assert!(output.has_window());

    fs::remove_dir_all(root).expect("printer root should be removable");
}

#[test]
fn flush_pending_document_is_a_noop_without_any_pending_page() {
    let _guard = crate::lock_sdl_test();
    crate::configure_headless_sdl();
    let sdl = sdl3::init().expect("SDL should initialize for empty printer flushing");
    let video = sdl.video().expect("video subsystem should initialize");
    let main_window = video
        .window("printer-main", 160, 144)
        .build()
        .expect("main test window should build");
    let root = temp_printer_root("empty-flush");

    let mut output = PrinterOutputState::default();
    output
        .flush_pending_document(&main_window, None, root.as_path())
        .expect("empty flush should be a no-op");
    assert!(output.last_saved_path().is_none());
    assert_eq!(output.latest_page_dimensions(), None);

    fs::remove_dir_all(root).expect("printer root should be removable");
}

#[test]
fn handle_printed_page_surfaces_output_directory_failures() {
    let _guard = crate::lock_sdl_test();
    crate::configure_headless_sdl();
    let sdl = sdl3::init().expect("SDL should initialize for printer save errors");
    let video = sdl.video().expect("video subsystem should initialize");
    let main_window = video
        .window("printer-main", 160, 144)
        .build()
        .expect("main test window should build");
    let root = temp_printer_root("output-errors");
    let blocking_file = root.join("not-a-directory");
    fs::write(&blocking_file, b"occupied").expect("blocking file should exist");
    let page = sample_printed_page(0xE4);

    let mut output = PrinterOutputState::default();
    let error = output
        .handle_printed_page(&main_window, None, blocking_file.as_path(), &page)
        .expect_err("printer save should fail when the output directory cannot be created");
    assert!(error.contains("failed to create printer output directory"));
    assert_eq!(output.latest_page_dimensions(), Some((4, 1)));
    assert!(output.has_window());

    fs::remove_dir_all(root).expect("printer root should be removable");
}

#[test]
fn handle_event_marks_matching_window_exposure_and_ignores_non_window_events() {
    let _guard = crate::lock_sdl_test();
    crate::configure_headless_sdl();
    let sdl = sdl3::init().expect("SDL should initialize for printer event handling");
    let video = sdl.video().expect("video subsystem should initialize");
    let main_window = video
        .window("printer-main", 160, 144)
        .build()
        .expect("main test window should build");
    let root = temp_printer_root("expose-event");
    let rom_path = root.join("printer.gb");
    let page = sample_printed_page(0xE4);
    let mut output = PrinterOutputState::default();

    assert!(
        !output
            .handle_event(&Event::Quit { timestamp: 0 })
            .expect("non-window events should be ignored")
    );

    output
        .handle_printed_page(&main_window, Some(&rom_path), root.as_path(), &page)
        .expect("printer page should create a window");
    let printer_window_id = output
        .window_id()
        .expect("printer window should expose an SDL id");

    assert!(
        output
            .handle_event(&Event::Window {
                timestamp: 0,
                window_id: printer_window_id,
                win_event: WindowEvent::Exposed,
            })
            .expect("exposed events should be handled for the printer window")
    );

    fs::remove_dir_all(root).expect("printer root should be removable");
}
