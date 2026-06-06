use super::*;

#[test]
fn render_frame_blends_the_base_frame_before_menu_and_hud_overlays() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("frame-blending-render", true, false, false);
    let texture_creator = harness.canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(
            sdl3::pixels::PixelFormat::RGB24,
            super::super::FRAMEBUFFER_WIDTH,
            super::super::FRAMEBUFFER_HEIGHT,
        )
        .expect("runtime texture should be creatable");
    let frame_len =
        super::super::FRAMEBUFFER_HEIGHT as usize * super::super::FRAMEBUFFER_PITCH_BYTES;
    let panel_len = (super::super::FRAMEBUFFER_WIDTH * super::super::FRAMEBUFFER_HEIGHT) as usize;
    let previous_panel = vec![0_u8; panel_len];
    let current_panel = vec![3_u8; panel_len];
    let layer_sources = vec![PpuFramebufferLayerSource::Background; panel_len];
    let mut video_options = harness.runtime.video_options.clone();
    video_options.frame_blending = DesktopFrameBlendingMode::On;
    video_options.presentation_filter = true;
    harness.runtime.video_options.frame_blending = DesktopFrameBlendingMode::On;

    let mut base_state = super::super::FrameBlendingState::default();
    let mut first_frame = vec![0_u8; frame_len];
    super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut first_frame,
        single_panel_render_input(&previous_panel, &layer_sources),
        &video_options,
        super::super::RenderPresentationInput {
            frame_blending_state: Some(&mut base_state),
            ..super::super::RenderPresentationInput::default()
        },
    )
    .expect("first frame should render");
    let mut blended_base_frame = vec![0_u8; frame_len];
    super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut blended_base_frame,
        single_panel_render_input(&current_panel, &layer_sources),
        &video_options,
        super::super::RenderPresentationInput {
            frame_blending_state: Some(&mut base_state),
            ..super::super::RenderPresentationInput::default()
        },
    )
    .expect("blended frame should render");
    let mut raw_base_frame = vec![0_u8; frame_len];
    super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut raw_base_frame,
        single_panel_render_input(&current_panel, &layer_sources),
        &video_options,
        super::super::RenderPresentationInput::default(),
    )
    .expect("raw frame should render");

    assert_ne!(
        &blended_base_frame[..3],
        &raw_base_frame[..3],
        "the base framebuffer should be blended before presentation"
    );
    assert_eq!(texture.scale_mode(), sdl3::render::ScaleMode::Linear);

    let menu_presentation = super::super::current_menu_presentation(
        harness.canvas.window(),
        &harness.runtime,
        &harness.machine,
        &harness.session,
    );
    harness.runtime.menu_state.open(menu_presentation);
    let open_menu_presentation = super::super::current_menu_presentation(
        harness.canvas.window(),
        &harness.runtime,
        &harness.machine,
        &harness.session,
    );
    let mut menu_state = super::super::FrameBlendingState::default();
    let mut discarded_first_frame = vec![0_u8; frame_len];
    super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut discarded_first_frame,
        single_panel_render_input(&previous_panel, &layer_sources),
        &video_options,
        super::super::RenderPresentationInput {
            frame_blending_state: Some(&mut menu_state),
            ..super::super::RenderPresentationInput::default()
        },
    )
    .expect("menu blend history frame should render");
    let mut blended_menu_frame = vec![0_u8; frame_len];
    super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut blended_menu_frame,
        single_panel_render_input(&current_panel, &layer_sources),
        &video_options,
        super::super::RenderPresentationInput {
            frame_blending_state: Some(&mut menu_state),
            menu_state: Some((&harness.runtime.menu_state, open_menu_presentation)),
            hud: super::super::RenderHudInput::default(),
        },
    )
    .expect("blended menu frame should render");
    let mut raw_menu_frame = vec![0_u8; frame_len];
    super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut raw_menu_frame,
        single_panel_render_input(&current_panel, &layer_sources),
        &video_options,
        super::super::RenderPresentationInput {
            frame_blending_state: None,
            menu_state: Some((&harness.runtime.menu_state, open_menu_presentation)),
            hud: super::super::RenderHudInput::default(),
        },
    )
    .expect("raw menu frame should render");
    let menu_panel_pixel = 25 * super::super::FRAMEBUFFER_PITCH_BYTES + 25 * 3;
    assert_eq!(
        &blended_menu_frame[menu_panel_pixel..menu_panel_pixel + 3],
        &raw_menu_frame[menu_panel_pixel..menu_panel_pixel + 3],
        "opaque menu panel pixels should be drawn after the frame blend"
    );
    assert_ne!(
        &raw_menu_frame[menu_panel_pixel..menu_panel_pixel + 3],
        &raw_base_frame[menu_panel_pixel..menu_panel_pixel + 3],
        "the selected sample should be inside the menu overlay"
    );
    harness.runtime.menu_state.close();

    let hud = super::super::RenderHudInput {
        performance: Some(PerformanceHudSnapshot {
            fps: 60.0,
            speed_percent: 100.0,
            frame_time_ms: 16.7,
            emulation_time_ms: 10.0,
            render_time_ms: 2.0,
            pacing_time_ms: 4.0,
            audio_queue_ms: Some(12.0),
            rewind: RewindHudSnapshot::default(),
        }),
        cgb_ir: None,
        rewind_indicator: false,
        fast_forward_indicator: false,
    };
    video_options.show_performance_hud = true;
    let mut hud_state = super::super::FrameBlendingState::default();
    let mut discarded_hud_first_frame = vec![0_u8; frame_len];
    super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut discarded_hud_first_frame,
        single_panel_render_input(&previous_panel, &layer_sources),
        &video_options,
        super::super::RenderPresentationInput {
            frame_blending_state: Some(&mut hud_state),
            ..super::super::RenderPresentationInput::default()
        },
    )
    .expect("HUD blend history frame should render");
    let mut blended_hud_frame = vec![0_u8; frame_len];
    super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut blended_hud_frame,
        single_panel_render_input(&current_panel, &layer_sources),
        &video_options,
        super::super::RenderPresentationInput {
            frame_blending_state: Some(&mut hud_state),
            menu_state: None,
            hud,
        },
    )
    .expect("blended HUD frame should render");
    let mut raw_hud_frame = vec![0_u8; frame_len];
    super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut raw_hud_frame,
        single_panel_render_input(&current_panel, &layer_sources),
        &video_options,
        super::super::RenderPresentationInput {
            frame_blending_state: None,
            menu_state: None,
            hud,
        },
    )
    .expect("raw HUD frame should render");
    let hud_panel_pixel = 5 * super::super::FRAMEBUFFER_PITCH_BYTES + 5 * 3;
    assert_eq!(
        &blended_hud_frame[hud_panel_pixel..hud_panel_pixel + 3],
        &raw_hud_frame[hud_panel_pixel..hud_panel_pixel + 3],
        "opaque HUD panel pixels should be drawn after the frame blend"
    );
    assert_ne!(
        &raw_hud_frame[hud_panel_pixel..hud_panel_pixel + 3],
        &raw_base_frame[hud_panel_pixel..hud_panel_pixel + 3],
        "the selected sample should be inside the HUD overlay"
    );
}

#[test]
fn render_frame_draws_corner_indicators_without_stats_hud() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("rewind-indicator-render", true, false, false);
    let texture_creator = harness.canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(
            sdl3::pixels::PixelFormat::RGB24,
            super::super::FRAMEBUFFER_WIDTH,
            super::super::FRAMEBUFFER_HEIGHT,
        )
        .expect("runtime texture should be creatable");
    let mut baseline_frame = vec![
        0_u8;
        super::super::FRAMEBUFFER_HEIGHT as usize
            * super::super::FRAMEBUFFER_PITCH_BYTES
    ];
    let mut indicator_frame = baseline_frame.clone();
    let mut fast_forward_indicator_frame = baseline_frame.clone();
    let mut cgb_ir_helper_off_frame = baseline_frame.clone();
    let mut cgb_ir_helper_on_frame = baseline_frame.clone();
    let panel_len = (super::super::FRAMEBUFFER_WIDTH * super::super::FRAMEBUFFER_HEIGHT) as usize;
    let framebuffer = vec![0_u8; panel_len];
    let layer_sources = vec![PpuFramebufferLayerSource::Background; panel_len];
    let render_input = || super::super::FramebufferRenderInput {
        dimensions: super::super::FramebufferDimensions {
            width: super::super::FRAMEBUFFER_WIDTH,
            height: super::super::FRAMEBUFFER_HEIGHT,
        },
        panels: [
            Some(super::super::FramebufferPanelInput {
                dimensions: super::super::FramebufferDimensions {
                    width: super::super::FRAMEBUFFER_WIDTH,
                    height: super::super::FRAMEBUFFER_HEIGHT,
                },
                framebuffer: &framebuffer,
                framebuffer_layer_sources: &layer_sources,
                bgwin_framebuffer: &framebuffer,
                backdrop_framebuffer: &framebuffer,
                bgwin_framebuffer_layer_sources: &layer_sources,
                display_palette: super::super::DMG_DISPLAY_PALETTE,
                cgb_framebuffer_rgb555: None,
                sgb_framebuffer_rgb555: None,
                borrowed_sgb_border: None,
            }),
            None,
            None,
            None,
        ],
    };
    let mut video_options = harness.runtime.video_options.clone();
    video_options.show_performance_hud = false;

    super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut baseline_frame,
        render_input(),
        &video_options,
        super::super::RenderPresentationInput::default(),
    )
    .expect("baseline frame should render");
    super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut indicator_frame,
        render_input(),
        &video_options,
        super::super::RenderPresentationInput {
            frame_blending_state: None,
            menu_state: None,
            hud: super::super::RenderHudInput {
                performance: None,
                cgb_ir: None,
                rewind_indicator: true,
                fast_forward_indicator: false,
            },
        },
    )
    .expect("rewind indicator frame should render");

    assert_ne!(
        indicator_frame, baseline_frame,
        "rewind indicator should render independently from the stats HUD"
    );

    super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut fast_forward_indicator_frame,
        render_input(),
        &video_options,
        super::super::RenderPresentationInput {
            frame_blending_state: None,
            menu_state: None,
            hud: super::super::RenderHudInput {
                performance: None,
                cgb_ir: None,
                rewind_indicator: false,
                fast_forward_indicator: true,
            },
        },
    )
    .expect("fast-forward indicator frame should render");
    assert_ne!(
        fast_forward_indicator_frame, baseline_frame,
        "fast-forward indicator should render independently from the stats HUD"
    );

    let cgb_ir_snapshot = super::super::CgbInfraredHudSnapshot {
        p1: super::super::CgbInfraredParticipantHudSnapshot {
            read_enabled: true,
            sensor_warmed: true,
            ..super::super::CgbInfraredParticipantHudSnapshot::default()
        },
        p2: super::super::CgbInfraredParticipantHudSnapshot {
            read_enabled: true,
            sensor_warmed: true,
            ..super::super::CgbInfraredParticipantHudSnapshot::default()
        },
    };
    super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut cgb_ir_helper_off_frame,
        render_input(),
        &video_options,
        super::super::RenderPresentationInput {
            frame_blending_state: None,
            menu_state: None,
            hud: super::super::RenderHudInput {
                performance: None,
                cgb_ir: Some(cgb_ir_snapshot),
                rewind_indicator: false,
                fast_forward_indicator: false,
            },
        },
    )
    .expect("CGB IR helper-off frame should render");
    assert_eq!(
        cgb_ir_helper_off_frame, baseline_frame,
        "CGB IR helper should not render while the video option is disabled"
    );

    video_options.show_cgb_infrared_helper = true;
    super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut cgb_ir_helper_on_frame,
        render_input(),
        &video_options,
        super::super::RenderPresentationInput {
            frame_blending_state: None,
            menu_state: None,
            hud: super::super::RenderHudInput {
                performance: None,
                cgb_ir: Some(cgb_ir_snapshot),
                rewind_indicator: false,
                fast_forward_indicator: false,
            },
        },
    )
    .expect("CGB IR helper-on frame should render");
    assert_ne!(
        cgb_ir_helper_on_frame, baseline_frame,
        "CGB IR helper should render when the video option is enabled"
    );
}

#[test]
fn linked_runtime_routes_p1_and_p2_keyboard_input_independently() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("linked-keyboard-routing", true, false, false);
    let secondary_machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    harness
        .machine
        .attach_secondary_dmg04(secondary_machine)
        .expect("secondary machine should attach");

    harness.push_key(Keycode::Up, true);
    harness.push_key_with_scancode(Keycode::W, Scancode::W, true);
    harness
        .process_events()
        .expect("linked keyboard press should process");
    harness.machine.step_t_cycle();

    assert_eq!(
        harness
            .machine
            .machine_for_player_slot(super::super::PlayerSlot::P1)
            .expect("P1 should map to the linked runtime machine")
            .joypad()
            .snapshot()
            .pressed_mask,
        0x04
    );
    assert_eq!(
        harness
            .machine
            .machine_for_player_slot(super::super::PlayerSlot::P2)
            .expect("P2 should map to the linked runtime secondary machine")
            .joypad()
            .snapshot()
            .pressed_mask,
        0x04
    );

    harness.push_key(Keycode::Up, false);
    harness.push_key_with_scancode(Keycode::W, Scancode::W, false);
    harness
        .process_events()
        .expect("linked keyboard release should process");
    harness.machine.step_t_cycle();

    assert_eq!(
        harness
            .machine
            .machine_for_player_slot(super::super::PlayerSlot::P1)
            .expect("P1 should map to the linked runtime machine")
            .joypad()
            .snapshot()
            .pressed_mask,
        0
    );
    assert_eq!(
        harness
            .machine
            .machine_for_player_slot(super::super::PlayerSlot::P2)
            .expect("P2 should map to the linked runtime secondary machine")
            .joypad()
            .snapshot()
            .pressed_mask,
        0
    );
}

#[test]
fn sgb_single_runtime_routes_multiplayer_keyboard_slots_to_host_controllers() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("sgb-keyboard-routing", true, false, false);
    harness.machine = super::super::DesktopEmulationSession::new_single(Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy)
            .with_startup_mode(StartupMode::SkipBoot)
            .with_sgb_profile(SgbHostProfile::SgbNtsc),
    ));

    harness.push_key_with_scancode(Keycode::E, Scancode::E, true);
    harness.push_key_with_scancode(Keycode::B, Scancode::B, true);
    harness.push_key_with_scancode(Keycode::M, Scancode::M, true);
    harness
        .process_events()
        .expect("SGB keyboard press should process");
    harness.machine.step_t_cycle();

    assert_eq!(
        harness
            .machine
            .primary_machine()
            .sgb_host()
            .snapshot()
            .multiplayer
            .player_pressed_masks,
        [0x00, 0x80, 0x10, 0x20],
        "P2 START, P3 A, and P4 B should route into SGB host controller slots instead of Game Link machines"
    );
    assert_eq!(
        harness
            .machine
            .primary_machine()
            .joypad()
            .snapshot()
            .pressed_mask,
        0,
        "SGB P2/P3/P4 desktop keys must not leak into the local P1 joypad"
    );

    harness.push_key_with_scancode(Keycode::E, Scancode::E, false);
    harness.push_key_with_scancode(Keycode::B, Scancode::B, false);
    harness.push_key_with_scancode(Keycode::M, Scancode::M, false);
    harness
        .process_events()
        .expect("SGB keyboard release should process");
    harness.machine.step_t_cycle();

    assert_eq!(
        harness
            .machine
            .primary_machine()
            .sgb_host()
            .snapshot()
            .multiplayer
            .player_pressed_masks,
        [0x00; 4]
    );
}

#[test]
fn dmg07_keyboard_profiles_route_p3_and_p4_to_their_own_slots() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("dmg07-keyboard-routing", true, false, false);
    assert!(
        harness
            .execute_action(super::super::MenuAction::SetFourPlayerAdapter(
                super::super::DesktopDmg07PlayerCount::Four,
            ))
            .expect("4 PLAYER ADAPTER action should activate")
            .is_none()
    );

    harness.push_key_with_scancode(Keycode::B, Scancode::B, true);
    harness.push_key_with_scancode(Keycode::M, Scancode::M, true);
    harness
        .process_events()
        .expect("DMG-07 keyboard press should process");
    harness.machine.step_t_cycle();

    assert_eq!(
        harness
            .machine
            .machine_for_player_slot(super::super::PlayerSlot::P3)
            .expect("P3 should map to the third DMG-07 machine")
            .joypad()
            .snapshot()
            .pressed_mask,
        0x10
    );
    assert_eq!(
        harness
            .machine
            .machine_for_player_slot(super::super::PlayerSlot::P4)
            .expect("P4 should map to the fourth DMG-07 machine")
            .joypad()
            .snapshot()
            .pressed_mask,
        0x20
    );
    assert_eq!(
        harness
            .machine
            .machine_for_player_slot(super::super::PlayerSlot::P1)
            .expect("P1 should map to the primary DMG-07 machine")
            .joypad()
            .snapshot()
            .pressed_mask,
        0
    );
    assert_eq!(
        harness
            .machine
            .machine_for_player_slot(super::super::PlayerSlot::P2)
            .expect("P2 should map to the second DMG-07 machine")
            .joypad()
            .snapshot()
            .pressed_mask,
        0
    );

    harness.push_key_with_scancode(Keycode::B, Scancode::B, false);
    harness.push_key_with_scancode(Keycode::M, Scancode::M, false);
    harness
        .process_events()
        .expect("DMG-07 keyboard release should process");
    harness.machine.step_t_cycle();

    assert_eq!(
        harness
            .machine
            .machine_for_player_slot(super::super::PlayerSlot::P3)
            .expect("P3 should map to the third DMG-07 machine")
            .joypad()
            .snapshot()
            .pressed_mask,
        0
    );
    assert_eq!(
        harness
            .machine
            .machine_for_player_slot(super::super::PlayerSlot::P4)
            .expect("P4 should map to the fourth DMG-07 machine")
            .joypad()
            .snapshot()
            .pressed_mask,
        0
    );
}

#[test]
fn audio_source_machine_follows_the_p1_host_policy_for_single_and_linked_sessions() {
    let _guard = crate::lock_sdl_test();
    let single = FrontendHarness::new("audio-source-single", true, false, false).machine;
    assert_eq!(
        super::super::emulation_profile_session_kind(&single),
        super::super::EmulationProfileSessionKind::Single
    );
    assert!(std::ptr::eq(
        super::super::audio_source_machine(&single),
        single
            .machine_for_player_slot(super::super::PlayerSlot::P1)
            .expect("P1 should map to the single runtime machine")
    ));

    let primary = dmg_skip_boot_summary_machine();
    let secondary = dmg_skip_boot_summary_machine();
    let linked =
        super::super::linked_session::DesktopEmulationSession::new_linked_dmg04_two_player(
            primary, secondary,
        )
        .expect("linked desktop session should build");

    assert_eq!(
        super::super::emulation_profile_session_kind(&linked),
        super::super::EmulationProfileSessionKind::LinkedDmg04TwoPlayer
    );
    assert!(std::ptr::eq(
        super::super::audio_source_machine(&linked),
        linked
            .machine_for_player_slot(super::super::PlayerSlot::P1)
            .expect("P1 should map to the linked runtime machine")
    ));

    let dmg07 = super::super::linked_session::DesktopEmulationSession::new_linked_dmg07(
        vec![
            dmg_skip_boot_summary_machine(),
            dmg_skip_boot_summary_machine(),
            dmg_skip_boot_summary_machine(),
        ],
        super::super::DesktopDmg07PlayerCount::Three,
    )
    .expect("DMG-07 desktop session should build");
    assert_eq!(
        super::super::emulation_profile_session_kind(&dmg07),
        super::super::EmulationProfileSessionKind::LinkedDmg07
    );
    assert!(std::ptr::eq(
        super::super::audio_source_machine(&dmg07),
        dmg07
            .machine_for_player_slot(super::super::PlayerSlot::P1)
            .expect("P1 should map to the DMG-07 runtime machine")
    ));
}

#[test]
fn automatic_audio_recording_helpers_build_restart_and_finish_recorders() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("automatic-recorder", true, false, false);
    let channel_mask = super::super::ApuRecordedChannelMask::NONE
        .with_channel(super::super::ApuRecordedChannel::Ch4, true);
    let recorder = super::super::create_audio_recorder(
        &super::super::DesktopAudioRecordingMode::Automatic,
        channel_mask,
        &harness.session,
        &harness.machine,
    )
    .expect("automatic recorder creation should succeed")
    .expect("automatic mode should create a recorder");
    let first_path = harness.root.join("audios").join("automatic-recorder-0.wav");
    assert!(first_path.exists());
    assert_eq!(recorder.channel_mask(), channel_mask);

    let mut recorder_slot = Some(recorder);
    super::super::finish_audio_recorder(&mut recorder_slot).expect("finishing a live recorder");
    assert!(recorder_slot.is_none());

    harness.runtime.audio_recording_mode = super::super::DesktopAudioRecordingMode::Automatic;
    harness.runtime.audio_channel_mask = channel_mask;
    harness.runtime.audio_recorder = super::super::create_audio_recorder(
        &harness.runtime.audio_recording_mode,
        harness.runtime.audio_channel_mask,
        &harness.session,
        &harness.machine,
    )
    .expect("initial automatic recorder should build");
    super::super::restart_automatic_audio_recorder(
        &mut harness.runtime,
        &harness.session,
        &harness.machine,
    )
    .expect("restarting automatic recording should rotate to a new file");
    let second_path = harness.root.join("audios").join("automatic-recorder-1.wav");
    assert!(second_path.exists());
    assert!(harness.runtime.audio_recorder.is_some());

    super::super::finish_audio_recorder(&mut harness.runtime.audio_recorder)
        .expect("final recorder cleanup should succeed");
    assert!(harness.runtime.audio_recorder.is_none());
}

#[test]
fn automatic_audio_recording_without_a_rom_falls_back_to_the_session_directory() {
    let _guard = crate::lock_sdl_test();
    let harness = FrontendHarness::new("automatic-recorder-no-rom", false, false, false);
    let recorder = super::super::create_audio_recorder(
        &super::super::DesktopAudioRecordingMode::Automatic,
        super::super::ApuRecordedChannelMask::ALL,
        &harness.session,
        &harness.machine,
    )
    .expect("automatic recorder creation without a rom should succeed");
    assert!(recorder.is_some());
    let fallback_path = harness.root.join("audios").join("gb-cycle-0.wav");
    assert!(fallback_path.exists());
}

#[test]
fn render_frame_places_p2_output_in_the_right_panel() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("linked-render", true, false, false);
    let texture_creator = harness.canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(
            sdl3::pixels::PixelFormat::RGB24,
            super::super::FRAMEBUFFER_WIDTH * 2,
            super::super::FRAMEBUFFER_HEIGHT,
        )
        .expect("linked runtime texture should be creatable");
    let linked_dimensions = super::super::FramebufferDimensions {
        width: super::super::FRAMEBUFFER_WIDTH * 2,
        height: super::super::FRAMEBUFFER_HEIGHT,
    };
    let mut rgb_frame =
        vec![
            0_u8;
            linked_dimensions.height as usize
                * super::super::framebuffer_pitch_bytes_for_dimensions(linked_dimensions)
        ];
    let left_framebuffer =
        vec![0_u8; (super::super::FRAMEBUFFER_WIDTH * super::super::FRAMEBUFFER_HEIGHT) as usize];
    let right_framebuffer =
        vec![3_u8; (super::super::FRAMEBUFFER_WIDTH * super::super::FRAMEBUFFER_HEIGHT) as usize];
    let left_sources = vec![PpuFramebufferLayerSource::Background; left_framebuffer.len()];
    let right_sources = vec![PpuFramebufferLayerSource::Background; right_framebuffer.len()];

    let _ = super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut rgb_frame,
        super::super::FramebufferRenderInput {
            dimensions: linked_dimensions,
            panels: [
                Some(super::super::FramebufferPanelInput {
                    dimensions: super::super::FramebufferDimensions {
                        width: super::super::FRAMEBUFFER_WIDTH,
                        height: super::super::FRAMEBUFFER_HEIGHT,
                    },
                    framebuffer: &left_framebuffer,
                    framebuffer_layer_sources: &left_sources,
                    bgwin_framebuffer: &left_framebuffer,
                    backdrop_framebuffer: &left_framebuffer,
                    bgwin_framebuffer_layer_sources: &left_sources,
                    display_palette: super::super::DMG_DISPLAY_PALETTE,
                    cgb_framebuffer_rgb555: None,
                    sgb_framebuffer_rgb555: None,
                    borrowed_sgb_border: None,
                }),
                Some(super::super::FramebufferPanelInput {
                    dimensions: super::super::FramebufferDimensions {
                        width: super::super::FRAMEBUFFER_WIDTH,
                        height: super::super::FRAMEBUFFER_HEIGHT,
                    },
                    framebuffer: &right_framebuffer,
                    framebuffer_layer_sources: &right_sources,
                    bgwin_framebuffer: &right_framebuffer,
                    backdrop_framebuffer: &right_framebuffer,
                    bgwin_framebuffer_layer_sources: &right_sources,
                    display_palette: super::super::DMG_DISPLAY_PALETTE,
                    cgb_framebuffer_rgb555: None,
                    sgb_framebuffer_rgb555: None,
                    borrowed_sgb_border: None,
                }),
                None,
                None,
            ],
        },
        &harness.runtime.video_options,
        super::super::RenderPresentationInput::default(),
    )
    .expect("linked frame should render");

    let pitch = super::super::framebuffer_pitch_bytes_for_dimensions(linked_dimensions);
    let left_pixel = &rgb_frame[0..3];
    let right_pixel_index = super::super::FRAMEBUFFER_WIDTH as usize * 3;
    let right_pixel = &rgb_frame[right_pixel_index..right_pixel_index + 3];
    assert_eq!(left_pixel, &super::super::DMG_DISPLAY_PALETTE.shade_rgb(0));
    assert_eq!(right_pixel, &super::super::DMG_DISPLAY_PALETTE.shade_rgb(3));
    assert_eq!(rgb_frame.len(), linked_dimensions.height as usize * pitch);
}

#[test]
fn render_frame_places_dmg07_outputs_in_a_two_by_two_grid() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("dmg07-grid-render", true, false, false);
    let linked = super::super::linked_session::DesktopEmulationSession::new_linked_dmg07(
        vec![
            dmg_skip_boot_summary_machine(),
            dmg_skip_boot_summary_machine(),
            dmg_skip_boot_summary_machine(),
            dmg_skip_boot_summary_machine(),
        ],
        super::super::DesktopDmg07PlayerCount::Four,
    )
    .expect("DMG-07 desktop session should build");
    let dimensions = super::super::framebuffer_dimensions_for_session(
        &linked,
        &gb_desktop::VideoOptions::default(),
        true,
    );
    assert_eq!(
        dimensions,
        super::super::FramebufferDimensions {
            width: super::super::FRAMEBUFFER_WIDTH * 2,
            height: super::super::FRAMEBUFFER_HEIGHT * 2,
        }
    );

    let texture_creator = harness.canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(
            sdl3::pixels::PixelFormat::RGB24,
            dimensions.width,
            dimensions.height,
        )
        .expect("DMG-07 runtime texture should be creatable");
    let mut rgb_frame = vec![
        0_u8;
        dimensions.height as usize
            * super::super::framebuffer_pitch_bytes_for_dimensions(dimensions)
    ];
    let panel_len = (super::super::FRAMEBUFFER_WIDTH * super::super::FRAMEBUFFER_HEIGHT) as usize;
    let panel_0 = vec![0_u8; panel_len];
    let panel_1 = vec![1_u8; panel_len];
    let panel_2 = vec![2_u8; panel_len];
    let panel_3 = vec![3_u8; panel_len];
    let sources = vec![PpuFramebufferLayerSource::Background; panel_len];

    super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut rgb_frame,
        super::super::FramebufferRenderInput {
            dimensions,
            panels: [
                Some(super::super::FramebufferPanelInput {
                    dimensions: super::super::FramebufferDimensions {
                        width: super::super::FRAMEBUFFER_WIDTH,
                        height: super::super::FRAMEBUFFER_HEIGHT,
                    },
                    framebuffer: &panel_0,
                    framebuffer_layer_sources: &sources,
                    bgwin_framebuffer: &panel_0,
                    backdrop_framebuffer: &panel_0,
                    bgwin_framebuffer_layer_sources: &sources,
                    display_palette: super::super::DMG_DISPLAY_PALETTE,
                    cgb_framebuffer_rgb555: None,
                    sgb_framebuffer_rgb555: None,
                    borrowed_sgb_border: None,
                }),
                Some(super::super::FramebufferPanelInput {
                    dimensions: super::super::FramebufferDimensions {
                        width: super::super::FRAMEBUFFER_WIDTH,
                        height: super::super::FRAMEBUFFER_HEIGHT,
                    },
                    framebuffer: &panel_1,
                    framebuffer_layer_sources: &sources,
                    bgwin_framebuffer: &panel_1,
                    backdrop_framebuffer: &panel_1,
                    bgwin_framebuffer_layer_sources: &sources,
                    display_palette: super::super::DMG_DISPLAY_PALETTE,
                    cgb_framebuffer_rgb555: None,
                    sgb_framebuffer_rgb555: None,
                    borrowed_sgb_border: None,
                }),
                Some(super::super::FramebufferPanelInput {
                    dimensions: super::super::FramebufferDimensions {
                        width: super::super::FRAMEBUFFER_WIDTH,
                        height: super::super::FRAMEBUFFER_HEIGHT,
                    },
                    framebuffer: &panel_2,
                    framebuffer_layer_sources: &sources,
                    bgwin_framebuffer: &panel_2,
                    backdrop_framebuffer: &panel_2,
                    bgwin_framebuffer_layer_sources: &sources,
                    display_palette: super::super::DMG_DISPLAY_PALETTE,
                    cgb_framebuffer_rgb555: None,
                    sgb_framebuffer_rgb555: None,
                    borrowed_sgb_border: None,
                }),
                Some(super::super::FramebufferPanelInput {
                    dimensions: super::super::FramebufferDimensions {
                        width: super::super::FRAMEBUFFER_WIDTH,
                        height: super::super::FRAMEBUFFER_HEIGHT,
                    },
                    framebuffer: &panel_3,
                    framebuffer_layer_sources: &sources,
                    bgwin_framebuffer: &panel_3,
                    backdrop_framebuffer: &panel_3,
                    bgwin_framebuffer_layer_sources: &sources,
                    display_palette: super::super::DMG_DISPLAY_PALETTE,
                    cgb_framebuffer_rgb555: None,
                    sgb_framebuffer_rgb555: None,
                    borrowed_sgb_border: None,
                }),
            ],
        },
        &harness.runtime.video_options,
        super::super::RenderPresentationInput::default(),
    )
    .expect("DMG-07 grid frame should render");

    let pitch = super::super::framebuffer_pitch_bytes_for_dimensions(dimensions);
    let top_left = &rgb_frame[0..3];
    let top_right_index = super::super::FRAMEBUFFER_WIDTH as usize * 3;
    let bottom_left_index = super::super::FRAMEBUFFER_HEIGHT as usize * pitch;
    let bottom_right_index = bottom_left_index + top_right_index;
    assert_eq!(top_left, &super::super::DMG_DISPLAY_PALETTE.shade_rgb(0));
    assert_eq!(
        &rgb_frame[top_right_index..top_right_index + 3],
        &super::super::DMG_DISPLAY_PALETTE.shade_rgb(1)
    );
    assert_eq!(
        &rgb_frame[bottom_left_index..bottom_left_index + 3],
        &super::super::DMG_DISPLAY_PALETTE.shade_rgb(2)
    );
    assert_eq!(
        &rgb_frame[bottom_right_index..bottom_right_index + 3],
        &super::super::DMG_DISPLAY_PALETTE.shade_rgb(3)
    );
}

#[test]
fn render_frame_reveals_bgwin_pixels_when_objects_are_hidden() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("layer-mask-render", true, false, false);
    let texture_creator = harness.canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(
            sdl3::pixels::PixelFormat::RGB24,
            super::super::FRAMEBUFFER_WIDTH,
            super::super::FRAMEBUFFER_HEIGHT,
        )
        .expect("runtime texture should be creatable");
    let mut rgb_frame = vec![
        0_u8;
        super::super::FRAMEBUFFER_HEIGHT as usize
            * super::super::FRAMEBUFFER_PITCH_BYTES
    ];
    let framebuffer =
        vec![3_u8; (super::super::FRAMEBUFFER_WIDTH * super::super::FRAMEBUFFER_HEIGHT) as usize];
    let layer_sources = vec![PpuFramebufferLayerSource::Object; framebuffer.len()];
    let bgwin_framebuffer =
        vec![1_u8; (super::super::FRAMEBUFFER_WIDTH * super::super::FRAMEBUFFER_HEIGHT) as usize];
    let bgwin_layer_sources = vec![PpuFramebufferLayerSource::Window; framebuffer.len()];
    let mut video_options = harness.runtime.video_options.clone();
    video_options.show_objects = false;

    super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut rgb_frame,
        super::super::FramebufferRenderInput {
            dimensions: super::super::FramebufferDimensions {
                width: super::super::FRAMEBUFFER_WIDTH,
                height: super::super::FRAMEBUFFER_HEIGHT,
            },
            panels: [
                Some(super::super::FramebufferPanelInput {
                    dimensions: super::super::FramebufferDimensions {
                        width: super::super::FRAMEBUFFER_WIDTH,
                        height: super::super::FRAMEBUFFER_HEIGHT,
                    },
                    framebuffer: &framebuffer,
                    framebuffer_layer_sources: &layer_sources,
                    bgwin_framebuffer: &bgwin_framebuffer,
                    backdrop_framebuffer: &bgwin_framebuffer,
                    bgwin_framebuffer_layer_sources: &bgwin_layer_sources,
                    display_palette: super::super::DMG_DISPLAY_PALETTE,
                    cgb_framebuffer_rgb555: None,
                    sgb_framebuffer_rgb555: None,
                    borrowed_sgb_border: None,
                }),
                None,
                None,
                None,
            ],
        },
        &video_options,
        super::super::RenderPresentationInput::default(),
    )
    .expect("layer-masked frame should render");

    assert_eq!(
        &rgb_frame[..3],
        &super::super::DMG_DISPLAY_PALETTE.shade_rgb(1)
    );
}

#[test]
fn composite_uses_final_bgwin_shade_when_visible_layers_are_enabled() {
    let video_options = super::super::VideoOptions::default();

    assert_eq!(
        super::super::composite_framebuffer_panel_shade(
            3,
            PpuFramebufferLayerSource::Background,
            1,
            PpuFramebufferLayerSource::Background,
            2,
            &video_options,
        ),
        3
    );
    assert_eq!(
        super::super::composite_framebuffer_panel_shade(
            3,
            PpuFramebufferLayerSource::Window,
            1,
            PpuFramebufferLayerSource::Window,
            2,
            &video_options,
        ),
        3
    );
}

#[test]
fn render_frame_uses_dynamic_backdrop_when_bgwin_layers_are_hidden() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("layer-mask-dynamic-backdrop", true, false, false);
    let texture_creator = harness.canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(
            sdl3::pixels::PixelFormat::RGB24,
            super::super::FRAMEBUFFER_WIDTH,
            super::super::FRAMEBUFFER_HEIGHT,
        )
        .expect("runtime texture should be creatable");
    let mut rgb_frame = vec![
        0_u8;
        super::super::FRAMEBUFFER_HEIGHT as usize
            * super::super::FRAMEBUFFER_PITCH_BYTES
    ];
    let mut framebuffer =
        vec![0_u8; (super::super::FRAMEBUFFER_WIDTH * super::super::FRAMEBUFFER_HEIGHT) as usize];
    let mut layer_sources = vec![PpuFramebufferLayerSource::Background; framebuffer.len()];
    let bgwin_framebuffer =
        vec![1_u8; (super::super::FRAMEBUFFER_WIDTH * super::super::FRAMEBUFFER_HEIGHT) as usize];
    let bgwin_layer_sources = vec![PpuFramebufferLayerSource::Window; framebuffer.len()];
    let mut backdrop_framebuffer =
        vec![2_u8; (super::super::FRAMEBUFFER_WIDTH * super::super::FRAMEBUFFER_HEIGHT) as usize];
    let mut video_options = harness.runtime.video_options.clone();
    video_options.show_background = false;
    video_options.show_window = false;
    backdrop_framebuffer[0] = 1;
    framebuffer[1] = 3;
    layer_sources[1] = PpuFramebufferLayerSource::Object;

    super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut rgb_frame,
        super::super::FramebufferRenderInput {
            dimensions: super::super::FramebufferDimensions {
                width: super::super::FRAMEBUFFER_WIDTH,
                height: super::super::FRAMEBUFFER_HEIGHT,
            },
            panels: [
                Some(super::super::FramebufferPanelInput {
                    dimensions: super::super::FramebufferDimensions {
                        width: super::super::FRAMEBUFFER_WIDTH,
                        height: super::super::FRAMEBUFFER_HEIGHT,
                    },
                    framebuffer: &framebuffer,
                    framebuffer_layer_sources: &layer_sources,
                    bgwin_framebuffer: &bgwin_framebuffer,
                    backdrop_framebuffer: &backdrop_framebuffer,
                    bgwin_framebuffer_layer_sources: &bgwin_layer_sources,
                    display_palette: super::super::DMG_DISPLAY_PALETTE,
                    cgb_framebuffer_rgb555: None,
                    sgb_framebuffer_rgb555: None,
                    borrowed_sgb_border: None,
                }),
                None,
                None,
                None,
            ],
        },
        &video_options,
        super::super::RenderPresentationInput::default(),
    )
    .expect("OBJ-only frame should render with a dynamic backdrop");

    assert_eq!(
        &rgb_frame[..3],
        &super::super::DMG_DISPLAY_PALETTE.shade_rgb(1)
    );
    assert_eq!(
        &rgb_frame[3..6],
        &super::super::DMG_DISPLAY_PALETTE.shade_rgb(3)
    );
    assert_eq!(
        &rgb_frame[6..9],
        &super::super::DMG_DISPLAY_PALETTE.shade_rgb(2)
    );
}

#[test]
fn render_frame_applies_the_selected_presentation_filter_to_the_texture() {
    let _guard = crate::lock_sdl_test();
    let mut harness = FrontendHarness::new("render-scale-mode", true, false, false);
    let texture_creator = harness.canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(
            sdl3::pixels::PixelFormat::RGB24,
            super::super::FRAMEBUFFER_WIDTH,
            super::super::FRAMEBUFFER_HEIGHT,
        )
        .expect("runtime texture should be creatable");
    let mut rgb_frame = vec![
        0_u8;
        super::super::FRAMEBUFFER_HEIGHT as usize
            * super::super::FRAMEBUFFER_PITCH_BYTES
    ];
    let framebuffer = super::super::FramebufferRenderInput {
        dimensions: super::super::FramebufferDimensions {
            width: super::super::FRAMEBUFFER_WIDTH,
            height: super::super::FRAMEBUFFER_HEIGHT,
        },
        panels: [
            Some(super::super::FramebufferPanelInput {
                dimensions: super::super::FramebufferDimensions {
                    width: super::super::FRAMEBUFFER_WIDTH,
                    height: super::super::FRAMEBUFFER_HEIGHT,
                },
                framebuffer: harness.machine.ppu().framebuffer(),
                framebuffer_layer_sources: harness.machine.ppu().framebuffer_layer_sources(),
                bgwin_framebuffer: harness.machine.ppu().framebuffer_bgwin_panel_shades(),
                backdrop_framebuffer: harness.machine.ppu().framebuffer_backdrop_panel_shades(),
                bgwin_framebuffer_layer_sources: harness
                    .machine
                    .ppu()
                    .framebuffer_bgwin_layer_sources(),
                display_palette: super::super::DMG_DISPLAY_PALETTE,
                cgb_framebuffer_rgb555: None,
                sgb_framebuffer_rgb555: None,
                borrowed_sgb_border: None,
            }),
            None,
            None,
            None,
        ],
    };
    let mut video_options = harness.runtime.video_options.clone();

    video_options.presentation_filter = false;
    super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut rgb_frame,
        framebuffer.clone(),
        &video_options,
        super::super::RenderPresentationInput::default(),
    )
    .expect("nearest-neighbor frame should render");
    assert_eq!(texture.scale_mode(), sdl3::render::ScaleMode::Nearest);

    video_options.presentation_filter = true;
    super::super::render_frame(
        &mut harness.canvas,
        &mut texture,
        &mut rgb_frame,
        framebuffer,
        &video_options,
        super::super::RenderPresentationInput::default(),
    )
    .expect("filtered frame should render");
    assert_eq!(texture.scale_mode(), sdl3::render::ScaleMode::Linear);
}
