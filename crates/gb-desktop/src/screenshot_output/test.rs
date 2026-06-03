use super::{
    RenderedScreenshot, render_screenshot, resolve_next_screenshot_output_path,
    save_rendered_screenshot_png,
};
use gb_core::PpuFramebufferLayerSource;
use png::ColorType;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_screenshot_root(name: &str) -> PathBuf {
    let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "gb-cycle-desktop-screenshot-tests-{name}-{counter}"
    ));
    fs::create_dir_all(&root).expect("temporary screenshot root should be creatable");
    root
}

fn render_input(
    dimensions: crate::FramebufferDimensions,
    panels: [Option<crate::FramebufferPanelInput<'_>>; crate::PLAYER_SLOT_COUNT],
) -> crate::FramebufferRenderInput<'_> {
    crate::FramebufferRenderInput { dimensions, panels }
}

fn single_panel_input(
    panel: crate::FramebufferPanelInput<'_>,
) -> crate::FramebufferRenderInput<'_> {
    render_input(
        crate::FramebufferDimensions {
            width: crate::FRAMEBUFFER_WIDTH,
            height: crate::FRAMEBUFFER_HEIGHT,
        },
        [Some(panel), None, None, None],
    )
}

#[path = "test/errors.rs"]
mod errors;
#[path = "test/layers.rs"]
mod layers;
#[path = "test/paths.rs"]
mod paths;
#[path = "test/rendering.rs"]
mod rendering;
