use gb_core::PrintedPage;
use png::{BitDepth, ColorType};
use sdl3::VideoSubsystem;
use sdl3::event::{Event, WindowEvent};
use sdl3::pixels::{Color, PixelFormat};
use sdl3::render::Canvas;
use sdl3::sys;
use sdl3::video::Window;
use std::fs::{self, File};
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};

const PRINTER_OUTPUT_SUBDIRECTORY: &str = "printer";
const PRINTER_WINDOW_TITLE: &str = "GB Printer";
const PRINTER_WINDOW_SCALE: u32 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderedPrinterPage {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) grayscale_pixels: Vec<u8>,
    rgb_pixels: Vec<u8>,
}

#[derive(Default)]
pub(crate) struct PrinterOutputState {
    window: Option<PrinterWindow>,
    pending_document: Option<PendingPrinterDocument>,
    latest_page: Option<RenderedPrinterPage>,
    last_saved_path: Option<PathBuf>,
}

struct PendingPrinterDocument {
    rendered: RenderedPrinterPage,
    trailing_margin_after: u8,
}

struct PrinterWindow {
    canvas: Canvas<Window>,
}

impl PrinterOutputState {
    pub(crate) fn handle_printed_page(
        &mut self,
        main_window: &Window,
        rom_path: Option<&Path>,
        current_dir: &Path,
        page: &PrintedPage,
    ) -> Result<(), String> {
        let mut failures = Vec::new();

        let rendered = render_printed_page(page);
        let starts_continuation = page.print_args.margins.before == 0;
        let continues_after = page.print_args.margins.after == 0;

        let should_append = self.pending_document.as_ref().is_some_and(|document| {
            document.trailing_margin_after == 0
                && starts_continuation
                && document.rendered.width == rendered.width
        });

        if !should_append
            && let Err(error) = self.flush_pending_document(main_window, rom_path, current_dir)
        {
            failures.push(error);
        }

        if let Some(document) = self.pending_document.as_mut() {
            document.append(rendered, page.print_args.margins.after);
        } else {
            self.pending_document = Some(PendingPrinterDocument::new(
                rendered,
                page.print_args.margins.after,
            ));
        }
        self.latest_page = self
            .pending_document
            .as_ref()
            .map(|document| document.rendered.clone());

        if let Some(rendered) = self.latest_page.clone()
            && let Err(error) = self.present_page(main_window.subsystem(), &rendered)
        {
            failures.push(error);
        }

        if !continues_after
            && let Err(error) = self.flush_pending_document(main_window, rom_path, current_dir)
        {
            failures.push(error);
        }

        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("; "))
        }
    }

    pub(crate) fn flush_pending_document(
        &mut self,
        main_window: &Window,
        rom_path: Option<&Path>,
        current_dir: &Path,
    ) -> Result<(), String> {
        let Some(document) = self.pending_document.take() else {
            return Ok(());
        };
        let rendered = document.rendered;
        self.latest_page = Some(rendered.clone());

        let mut failures = Vec::new();
        if let Err(error) = self.present_page(main_window.subsystem(), &rendered) {
            failures.push(error);
        }

        match resolve_next_printer_output_path(rom_path, current_dir).and_then(|path| {
            match save_rendered_printer_page_png(&rendered, &path) {
                Ok(()) => Ok(path),
                Err(error) => Err(error),
            }
        }) {
            Ok(path) => {
                self.last_saved_path = Some(path);
            }
            Err(error) => failures.push(error),
        }

        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("; "))
        }
    }

    pub(crate) fn handle_event(&mut self, event: &Event) -> Result<bool, String> {
        let Some(window_id) = event.get_window_id() else {
            return Ok(false);
        };
        if self.window_id() != Some(window_id) {
            return Ok(false);
        }

        match event {
            Event::Window {
                win_event: WindowEvent::CloseRequested,
                ..
            } => {
                self.window = None;
                Ok(true)
            }
            Event::Window {
                win_event:
                    WindowEvent::Shown
                    | WindowEvent::Exposed
                    | WindowEvent::Restored
                    | WindowEvent::Resized(_, _)
                    | WindowEvent::PixelSizeChanged(_, _),
                ..
            } => {
                self.represent_latest_page()?;
                Ok(true)
            }
            _ => Ok(true),
        }
    }

    #[cfg(test)]
    pub(crate) fn has_window(&self) -> bool {
        self.window.is_some()
    }

    #[cfg(test)]
    pub(crate) fn latest_page_dimensions(&self) -> Option<(u32, u32)> {
        self.latest_page
            .as_ref()
            .map(|page| (page.width, page.height))
    }

    #[cfg(test)]
    pub(crate) fn last_saved_path(&self) -> Option<&Path> {
        self.last_saved_path.as_deref()
    }

    fn window_id(&self) -> Option<u32> {
        self.window.as_ref().map(PrinterWindow::id)
    }

    fn present_page(
        &mut self,
        video: &VideoSubsystem,
        rendered: &RenderedPrinterPage,
    ) -> Result<(), String> {
        if self.window.is_none() {
            self.window = Some(PrinterWindow::new(video, rendered.width, rendered.height)?);
        }

        let window = self
            .window
            .as_mut()
            .expect("printer window should exist after lazy creation");
        window.present(rendered)
    }

    fn represent_latest_page(&mut self) -> Result<(), String> {
        let Some(rendered) = self.latest_page.as_ref() else {
            return Ok(());
        };
        let Some(window) = self.window.as_mut() else {
            return Ok(());
        };
        window.present(rendered)
    }
}

impl PrinterWindow {
    fn new(video: &VideoSubsystem, width: u32, height: u32) -> Result<Self, String> {
        let scaled_width = scale_printer_dimension(width, "printer window width overflowed")?;
        let scaled_height = scale_printer_dimension(height, "printer window height overflowed")?;
        let mut window_builder = video.window(PRINTER_WINDOW_TITLE, scaled_width, scaled_height);
        window_builder.position_centered();
        let window: Window = crate::map_display_result(
            window_builder.build(),
            "failed to create printer output window",
        )?;
        let mut canvas = window.into_canvas();
        apply_printer_canvas_presentation(&mut canvas, width, height)?;
        Ok(Self { canvas })
    }

    fn id(&self) -> u32 {
        self.canvas.window().id()
    }

    fn present(&mut self, rendered: &RenderedPrinterPage) -> Result<(), String> {
        apply_printer_canvas_geometry(&mut self.canvas, rendered.width, rendered.height)?;
        let texture_creator = self.canvas.texture_creator();
        let mut texture = crate::map_display_result(
            texture_creator.create_texture_streaming(
                PixelFormat::RGB24,
                rendered.width,
                rendered.height,
            ),
            "failed to create printer output texture",
        )?;
        crate::map_display_result(
            texture.update(None, &rendered.rgb_pixels, rendered.pitch_bytes()),
            "failed to update printer output texture",
        )?;
        self.canvas.set_draw_color(Color::RGB(255, 255, 255));
        self.canvas.clear();
        crate::map_display_result(
            self.canvas.copy(&texture, None, None),
            "failed to present printer output texture",
        )?;
        self.canvas.present();
        Ok(())
    }
}

impl PendingPrinterDocument {
    fn new(rendered: RenderedPrinterPage, trailing_margin_after: u8) -> Self {
        Self {
            rendered,
            trailing_margin_after,
        }
    }

    fn append(&mut self, segment: RenderedPrinterPage, trailing_margin_after: u8) {
        debug_assert_eq!(self.rendered.width, segment.width);
        self.rendered.height = self.rendered.height.saturating_add(segment.height);
        self.rendered
            .grayscale_pixels
            .extend_from_slice(&segment.grayscale_pixels);
        self.rendered
            .rgb_pixels
            .extend_from_slice(&segment.rgb_pixels);
        self.trailing_margin_after = trailing_margin_after;
    }
}

pub(crate) fn render_printed_page(page: &PrintedPage) -> RenderedPrinterPage {
    let width = usize::from(page.width);
    let height = usize::from(page.height);
    let mut grayscale_pixels = Vec::with_capacity(width * height);
    let mut rgb_pixels = Vec::with_capacity(width * height * 3);

    for &pixel in &page.pixels {
        let shade = printer_grayscale_shade(page.print_args.palette, pixel);
        grayscale_pixels.push(shade);
        rgb_pixels.extend_from_slice(&[shade, shade, shade]);
    }

    RenderedPrinterPage {
        width: u32::from(page.width),
        height: u32::from(page.height),
        grayscale_pixels,
        rgb_pixels,
    }
}

pub(crate) fn resolve_next_printer_output_path(
    rom_path: Option<&Path>,
    current_dir: &Path,
) -> Result<PathBuf, String> {
    let output_dir = printer_output_directory(rom_path, current_dir);
    fs::create_dir_all(&output_dir).map_err(|error| {
        crate::format_path_error(
            "failed to create printer output directory",
            &output_dir,
            &error.to_string(),
        )
    })?;

    let stem = printer_output_stem(rom_path);
    for index in 1..=u16::MAX {
        let candidate = output_dir.join(format!("{stem}-printer-{index:04}.png"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(crate::format_path_error(
        "failed to allocate a free printer output path in",
        &output_dir,
        "directory is full",
    ))
}

pub(crate) fn save_rendered_printer_page_png(
    rendered: &RenderedPrinterPage,
    output_path: &Path,
) -> Result<(), String> {
    let file = File::create(output_path).map_err(|error| {
        crate::format_path_error(
            "failed to create printer output file",
            output_path,
            &error.to_string(),
        )
    })?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), rendered.width, rendered.height);
    encoder.set_color(ColorType::Grayscale);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| {
        crate::format_path_error(
            "failed to encode printer output PNG header",
            output_path,
            &png_encoding_io_error(error).to_string(),
        )
    })?;
    writer
        .write_image_data(&rendered.grayscale_pixels)
        .map_err(|error| {
            crate::format_path_error(
                "failed to write printer output PNG",
                output_path,
                &png_encoding_io_error(error).to_string(),
            )
        })?;
    Ok(())
}

fn apply_printer_canvas_geometry(
    canvas: &mut Canvas<Window>,
    width: u32,
    height: u32,
) -> Result<(), String> {
    let scaled_width = scale_printer_dimension(width, "printer window width overflowed")?;
    let scaled_height = scale_printer_dimension(height, "printer window height overflowed")?;
    crate::map_display_result(
        canvas.window_mut().set_size(scaled_width, scaled_height),
        "failed to resize printer output window",
    )?;
    apply_printer_canvas_presentation(canvas, width, height)
}

fn apply_printer_canvas_presentation(
    canvas: &mut Canvas<Window>,
    width: u32,
    height: u32,
) -> Result<(), String> {
    crate::map_display_result(
        canvas.set_logical_size(
            width,
            height,
            sys::render::SDL_LOGICAL_PRESENTATION_INTEGER_SCALE,
        ),
        "failed to configure printer output presentation",
    )
}

fn printer_grayscale_shade(palette: u8, pixel: u8) -> u8 {
    let palette_index = pixel.min(3) * 2;
    let shade_index = (palette >> palette_index) & 0x03;
    crate::DMG_GRAYSCALE_SHADES[usize::from(shade_index)]
}

fn printer_output_directory(rom_path: Option<&Path>, current_dir: &Path) -> PathBuf {
    let base_dir = match rom_path.and_then(Path::parent) {
        Some(parent) => parent.to_path_buf(),
        None => current_dir.to_path_buf(),
    };
    base_dir.join(PRINTER_OUTPUT_SUBDIRECTORY)
}

fn printer_output_stem(rom_path: Option<&Path>) -> String {
    rom_path
        .and_then(Path::file_stem)
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("gb-cycle")
        .to_string()
}

fn scale_printer_dimension(value: u32, overflow_context: &str) -> Result<u32, String> {
    value
        .checked_mul(PRINTER_WINDOW_SCALE)
        .ok_or_else(|| crate::overflow_error(overflow_context))
}

fn png_encoding_io_error(source: png::EncodingError) -> io::Error {
    match source {
        png::EncodingError::IoError(error) => error,
        other => io::Error::other(other.to_string()),
    }
}

impl RenderedPrinterPage {
    fn pitch_bytes(&self) -> usize {
        self.width as usize * 3
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PrinterOutputState, WindowEvent, printer_grayscale_shade, render_printed_page,
        resolve_next_printer_output_path, save_rendered_printer_page_png,
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
        let mut buffer = vec![0; reader.output_buffer_size()];
        let info = reader
            .next_frame(&mut buffer)
            .expect("PNG payload should decode");

        assert_eq!(info.width, 4);
        assert_eq!(info.height, 1);
        assert_eq!(info.color_type, ColorType::Grayscale);

        fs::remove_dir_all(root).expect("printer root should be removable");
    }

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
        let mut buffer = vec![0; reader.output_buffer_size()];
        let info = reader
            .next_frame(&mut buffer)
            .expect("PNG payload should decode");
        assert_eq!(info.width, 4);
        assert_eq!(info.height, 2);

        fs::remove_dir_all(root).expect("printer root should be removable");
    }
}
