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
mod test;
