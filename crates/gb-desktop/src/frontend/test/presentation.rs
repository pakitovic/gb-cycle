use super::*;

#[test]
fn display_palette_selection_covers_visible_console_models() {
    assert_eq!(
        DesktopDisplayPalette::default_for_console_model(DesktopConsoleModel::GameBoy),
        DesktopDisplayPalette::GameBoy
    );
    assert_eq!(
        DesktopDisplayPalette::default_for_console_model(DesktopConsoleModel::GameBoyPocket),
        DesktopDisplayPalette::Pocket
    );
    assert_eq!(
        DesktopDisplayPalette::default_for_console_model(DesktopConsoleModel::GameBoyLight),
        DesktopDisplayPalette::Light
    );
    assert_eq!(
        DesktopDisplayPalette::default_for_console_model(DesktopConsoleModel::GameBoyColor),
        DesktopDisplayPalette::Grey
    );
    assert_eq!(
        DesktopDisplayPalette::default_for_console_model(DesktopConsoleModel::GameBoyAdvance),
        DesktopDisplayPalette::Grey
    );
    assert_eq!(
        super::super::display_palette_for_desktop_palette(DesktopDisplayPalette::Grey).shade_rgb(0),
        [super::super::DMG_GRAYSCALE_SHADES[0]; 3]
    );
    assert_eq!(
        super::super::display_palette_for_desktop_palette(DesktopDisplayPalette::GameBoy)
            .shade_rgb(0),
        super::super::DMG_DISPLAY_PALETTE.shade_rgb(0)
    );
    assert_eq!(
        super::super::display_palette_for_desktop_palette(DesktopDisplayPalette::Pocket)
            .shade_rgb(1),
        super::super::MGB_DISPLAY_PALETTE.shade_rgb(1)
    );
    assert_eq!(
        super::super::display_palette_for_desktop_palette(DesktopDisplayPalette::Light)
            .shade_rgb(2),
        super::super::GBL_DISPLAY_PALETTE.shade_rgb(2)
    );
}

#[test]
fn framebuffer_render_input_uses_the_active_desktop_display_palette() {
    let machine = super::super::DesktopEmulationSession::new_single(Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    ));
    let video_options = gb_desktop::VideoOptions {
        display_palette: DesktopDisplayPalette::Light,
        ..gb_desktop::VideoOptions::default()
    };

    let render_input = super::super::framebuffer_render_input_for_session(
        &machine,
        super::super::framebuffer_dimensions_for_session(&machine, &video_options, true),
        &video_options,
        true,
    );

    let panel = render_input.panels[0]
        .as_ref()
        .expect("primary panel should be populated");
    assert_eq!(panel.display_palette, super::super::GBL_DISPLAY_PALETTE);
    assert!(render_input.panels[1..].iter().all(Option::is_none));
}

#[test]
fn framebuffer_render_input_uses_sgb_host_frame_dimensions_and_rgb555_output() {
    let machine = super::super::DesktopEmulationSession::new_single(Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy)
            .with_startup_mode(StartupMode::SkipBoot)
            .with_sgb_profile(SgbHostProfile::SgbNtsc),
    ));
    let video_options = gb_desktop::VideoOptions::default();
    let dimensions =
        super::super::framebuffer_dimensions_for_session(&machine, &video_options, true);

    let render_input = super::super::framebuffer_render_input_for_session(
        &machine,
        dimensions,
        &video_options,
        true,
    );

    assert_eq!(
        dimensions,
        super::super::FramebufferDimensions {
            width: super::super::SGB_HOST_FRAMEBUFFER_WIDTH,
            height: super::super::SGB_HOST_FRAMEBUFFER_HEIGHT,
        }
    );
    let panel = render_input.panels[0]
        .as_ref()
        .expect("SGB panel should be populated");
    assert_eq!(panel.dimensions, dimensions);
    assert_eq!(
        panel
            .sgb_framebuffer_rgb555
            .as_ref()
            .expect("SGB panel should carry host RGB555 output")
            .len(),
        (super::super::SGB_HOST_FRAMEBUFFER_WIDTH * super::super::SGB_HOST_FRAMEBUFFER_HEIGHT)
            as usize
    );

    let hidden_border_options = gb_desktop::VideoOptions {
        sgb_border: SgbBorderPresentationMode::Off,
        ..gb_desktop::VideoOptions::default()
    };
    let hidden_dimensions =
        super::super::framebuffer_dimensions_for_session(&machine, &hidden_border_options, true);
    let hidden_render_input = super::super::framebuffer_render_input_for_session(
        &machine,
        hidden_dimensions,
        &hidden_border_options,
        true,
    );
    assert_eq!(
        hidden_dimensions,
        super::super::FramebufferDimensions {
            width: super::super::FRAMEBUFFER_WIDTH,
            height: super::super::FRAMEBUFFER_HEIGHT,
        }
    );
    let hidden_panel = hidden_render_input.panels[0]
        .as_ref()
        .expect("hidden-border SGB panel should be populated");
    assert_eq!(hidden_panel.dimensions, hidden_dimensions);
    assert_eq!(
        hidden_panel
            .sgb_framebuffer_rgb555
            .as_ref()
            .expect("hidden-border SGB panel should carry LCD RGB555 output")
            .len(),
        (super::super::FRAMEBUFFER_WIDTH * super::super::FRAMEBUFFER_HEIGHT) as usize
    );
}

#[test]
fn framebuffer_render_input_uses_borrowed_sgb_border_without_changing_handheld_host() {
    let mut handheld = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    handheld.set_borrowed_sgb_border(Some(gb_core::SgbBorrowedBorder::new(
        gb_core::SgbBorderState::default(),
    )));
    let machine = super::super::DesktopEmulationSession::new_single(handheld);
    let video_options = gb_desktop::VideoOptions::default();

    let dimensions =
        super::super::framebuffer_dimensions_for_session(&machine, &video_options, true);
    let render_input = super::super::framebuffer_render_input_for_session(
        &machine,
        dimensions,
        &video_options,
        true,
    );

    assert_eq!(
        machine.primary_machine().config().host_platform,
        gb_core::HostPlatform::Handheld
    );
    assert_eq!(
        dimensions,
        super::super::FramebufferDimensions {
            width: super::super::SGB_HOST_FRAMEBUFFER_WIDTH,
            height: super::super::SGB_HOST_FRAMEBUFFER_HEIGHT,
        }
    );
    assert!(
        render_input.panels[0]
            .as_ref()
            .expect("borrowed-border panel should be populated")
            .borrowed_sgb_border
            .is_some()
    );

    let border_off_options = gb_desktop::VideoOptions {
        sgb_border: SgbBorderPresentationMode::Off,
        ..gb_desktop::VideoOptions::default()
    };
    assert_eq!(
        super::super::framebuffer_dimensions_for_session(&machine, &border_off_options, true),
        super::super::FramebufferDimensions {
            width: super::super::FRAMEBUFFER_WIDTH,
            height: super::super::FRAMEBUFFER_HEIGHT,
        }
    );
}

#[test]
fn launcher_sgb_profile_uses_handheld_dimensions_until_a_rom_is_loaded() {
    let machine = super::super::DesktopEmulationSession::new_single(Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy)
            .with_startup_mode(StartupMode::SkipBoot)
            .with_sgb_profile(SgbHostProfile::Sgb2Ntsc),
    ));
    for sgb_border in [
        SgbBorderPresentationMode::Auto,
        SgbBorderPresentationMode::Off,
    ] {
        let video_options = gb_desktop::VideoOptions {
            sgb_border,
            ..gb_desktop::VideoOptions::default()
        };
        let dimensions =
            super::super::framebuffer_dimensions_for_session(&machine, &video_options, false);
        let render_input = super::super::framebuffer_render_input_for_session(
            &machine,
            dimensions,
            &video_options,
            false,
        );

        assert_eq!(
            dimensions,
            super::super::FramebufferDimensions {
                width: super::super::FRAMEBUFFER_WIDTH,
                height: super::super::FRAMEBUFFER_HEIGHT,
            }
        );
        let panel = render_input.panels[0]
            .as_ref()
            .expect("launcher panel should be populated");
        assert_eq!(panel.dimensions, dimensions);
        assert!(
            panel.sgb_framebuffer_rgb555.is_none(),
            "launcher presentation must not allocate an SGB host frame before a ROM is loaded"
        );
    }
}

#[test]
fn cgb_framebuffer_rendering_uses_rgb555_without_desktop_display_palette() {
    let panel_len = (super::super::FRAMEBUFFER_WIDTH * super::super::FRAMEBUFFER_HEIGHT) as usize;
    let framebuffer = vec![0_u8; panel_len];
    let layer_sources = vec![PpuFramebufferLayerSource::Background; panel_len];
    let mut cgb_framebuffer_rgb555 = vec![0x7FFF_u16; panel_len];
    cgb_framebuffer_rgb555[0] = 0x001F;
    cgb_framebuffer_rgb555[1] = 0x03E0;
    cgb_framebuffer_rgb555[2] = 0x7C00;
    let mut rgb_frame = vec![
        0_u8;
        super::super::FRAMEBUFFER_HEIGHT as usize
            * super::super::FRAMEBUFFER_PITCH_BYTES
    ];

    super::super::write_framebuffer_region(
        &mut rgb_frame,
        super::super::FramebufferDimensions {
            width: super::super::FRAMEBUFFER_WIDTH,
            height: super::super::FRAMEBUFFER_HEIGHT,
        },
        0,
        0,
        super::super::FramebufferPanelInput {
            dimensions: super::super::FramebufferDimensions {
                width: super::super::FRAMEBUFFER_WIDTH,
                height: super::super::FRAMEBUFFER_HEIGHT,
            },
            framebuffer: &framebuffer,
            framebuffer_layer_sources: &layer_sources,
            bgwin_framebuffer: &framebuffer,
            backdrop_framebuffer: &framebuffer,
            bgwin_framebuffer_layer_sources: &layer_sources,
            display_palette: super::super::GBL_DISPLAY_PALETTE,
            cgb_framebuffer_rgb555: Some(&cgb_framebuffer_rgb555),
            sgb_framebuffer_rgb555: None,
            borrowed_sgb_border: None,
        },
        &gb_desktop::VideoOptions {
            display_palette: DesktopDisplayPalette::Light,
            ..gb_desktop::VideoOptions::default()
        },
    );

    assert_eq!(
        &rgb_frame[..9],
        &[0xFF, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00, 0xFF]
    );
}

#[test]
fn frame_blending_state_keeps_the_first_frame_raw_and_simple_blends_gamma_correctly() {
    let dimensions = super::super::FramebufferDimensions {
        width: 1,
        height: 1,
    };
    let mut state = super::super::FrameBlendingState::default();
    let mut first_frame = vec![255_u8; 3];

    state.apply(&mut first_frame, dimensions, DesktopFrameBlendingMode::On);

    assert_eq!(first_frame, vec![255_u8; 3]);
    assert_eq!(state.previous_rgb_frame, vec![255_u8; 3]);
    assert!(state.has_previous_frame);

    let mut second_frame = vec![0_u8; 3];
    state.apply(&mut second_frame, dimensions, DesktopFrameBlendingMode::On);

    assert_eq!(second_frame, vec![186_u8; 3]);
    assert_eq!(state.previous_rgb_frame, vec![0_u8; 3]);
}

#[test]
fn frame_blending_uses_the_same_half_weight_for_every_line() {
    let dimensions = super::super::FramebufferDimensions {
        width: 1,
        height: 2,
    };
    let current_frame = vec![0_u8; 6];
    let previous_frame = vec![255_u8; 6];
    let mut target = current_frame.clone();

    super::super::blend_rgb24_frames(
        &mut target,
        &current_frame,
        &previous_frame,
        dimensions,
        DesktopFrameBlendingMode::On,
    );

    assert_eq!(&target[0..3], &[186_u8; 3]);
    assert_eq!(&target[3..6], &[186_u8; 3]);
}

#[test]
fn frame_blending_state_clears_history_for_mode_and_dimension_changes() {
    let one_pixel = super::super::FramebufferDimensions {
        width: 1,
        height: 1,
    };
    let two_pixels = super::super::FramebufferDimensions {
        width: 2,
        height: 1,
    };
    let mut state = super::super::FrameBlendingState::default();
    let mut first_frame = vec![255_u8; 3];
    let mut second_frame = vec![0_u8; 3];

    state.apply(&mut first_frame, one_pixel, DesktopFrameBlendingMode::On);
    state.apply(&mut second_frame, one_pixel, DesktopFrameBlendingMode::On);
    assert_eq!(second_frame, vec![186_u8; 3]);

    let mut resized_frame = vec![64_u8; 6];
    state.apply(&mut resized_frame, two_pixels, DesktopFrameBlendingMode::On);
    assert_eq!(resized_frame, vec![64_u8; 6]);
    assert_eq!(state.dimensions, Some(two_pixels));
    assert_eq!(state.previous_rgb_frame, vec![64_u8; 6]);
    assert!(state.has_previous_frame);

    let mut disabled_frame = vec![32_u8; 6];
    state.apply(
        &mut disabled_frame,
        two_pixels,
        DesktopFrameBlendingMode::Off,
    );
    assert_eq!(disabled_frame, vec![32_u8; 6]);
    assert_eq!(state.mode, DesktopFrameBlendingMode::Off);
    assert!(state.previous_rgb_frame.is_empty());
    assert!(!state.has_previous_frame);
}
