mod audio;
mod audio_recording;
mod bootrom;
mod cli;
mod frontend;
mod input;
mod linked_session;
mod menu;
mod player_slots;
mod pocket_camera_live;
mod printer_output;
mod save_session;
mod screenshot_output;
mod settings;

fn main() -> std::process::ExitCode {
    frontend::main()
}

#[cfg(test)]
pub(crate) use frontend::{configure_headless_sdl, lock_sdl_test};

pub(crate) use frontend::{
    DMG_GRAYSCALE_SHADES, FramebufferRenderInput, format_path_error,
    framebuffer_cell_dimensions_for_panels, framebuffer_pitch_bytes_for_dimensions,
    map_display_result, overflow_error, write_framebuffer_region,
};
pub(crate) use gb_desktop::VideoOptions;

#[cfg(test)]
pub(crate) use frontend::{
    DMG_DISPLAY_PALETTE, FRAMEBUFFER_HEIGHT, FRAMEBUFFER_WIDTH, FramebufferDimensions,
    FramebufferPanelInput, SGB_HOST_FRAMEBUFFER_HEIGHT, SGB_HOST_FRAMEBUFFER_WIDTH,
};
#[cfg(test)]
pub(crate) use gb_desktop::DesktopDisplayPalette;
#[cfg(test)]
pub(crate) use player_slots::PLAYER_SLOT_COUNT;
