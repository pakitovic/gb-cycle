use super::{
    PrinterOutputState, WindowEvent, png_encoding_io_error, printer_grayscale_shade,
    render_printed_page, resolve_next_printer_output_path, save_rendered_printer_page_png,
    scale_printer_dimension,
};
use gb_core::{PrintedPage, PrinterMargins, PrinterPrintArgs};
use png::ColorType;
use sdl3::event::Event;
use std::fs;
use std::io::Cursor;
use std::path::Path;

fn temp_printer_root(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "gb-cycle-printer-output-{name}-{}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("stale printer test root should be removable");
    }
    fs::create_dir_all(&root).expect("printer test root should create");
    root
}

fn sample_printed_page_with_margins(palette: u8, before: u8, after: u8) -> PrintedPage {
    PrintedPage {
        width: 4,
        height: 1,
        pixels: vec![0, 1, 2, 3],
        print_args: PrinterPrintArgs {
            sheets: 1,
            margins: PrinterMargins { before, after },
            palette,
            exposure: 0x40,
        },
    }
}

fn sample_printed_page(palette: u8) -> PrintedPage {
    sample_printed_page_with_margins(palette, 1, 1)
}

#[path = "test/documents.rs"]
mod documents;
#[path = "test/errors.rs"]
mod errors;
#[path = "test/paths_events.rs"]
mod paths_events;
#[path = "test/rendering.rs"]
mod rendering;
#[path = "test/window.rs"]
mod window;
