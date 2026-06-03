use super::*;

#[test]
fn default_desktop_config_matches_the_dmg_interactive_baseline() {
    let config = DesktopConfig::default();

    assert_eq!(config.launch.console_model, DesktopConsoleModel::GameBoy);
    assert_eq!(config.launch.revision, HardwareRevision::DmgCpuC);
    assert_eq!(config.launch.startup_mode, StartupMode::SkipBoot);
    assert_eq!(config.launch.execution_mode, ExecutionMode::Strict);
    assert!(config.saves.enabled);
    assert_eq!(config.saves.flush_policy, DesktopSaveFlushPolicy::Debounced);
    assert_eq!(config.video.window_scale, DEFAULT_WINDOW_SCALE);
    assert!(config.video.integer_scale);
    assert!(!config.video.presentation_filter);
    assert_eq!(config.video.frame_blending, DesktopFrameBlendingMode::Off);
    assert_eq!(config.video.display_palette, DesktopDisplayPalette::GameBoy);
    assert!(config.video.show_background);
    assert!(config.video.show_window);
    assert!(config.video.show_objects);
    assert!(config.video.show_sgb_border);
    assert!(config.video.vsync);
    assert!(!config.video.show_performance_hud);
    assert!(!config.video.show_cgb_infrared_helper);
    assert!(config.audio.enabled);
    assert_eq!(
        config.audio.output_sample_rate_hz,
        DEFAULT_AUDIO_SAMPLE_RATE_HZ
    );
    assert!(config.input.gamepad.enabled);
    assert_eq!(
        config.input.gamepad.directional_source,
        GamepadDirectionalSource::DpadAndLeftStick
    );
    assert_eq!(config.input.gamepad.gyro_mode, GamepadGyroMode::Off);
    assert_eq!(config.input.gamepad.rumble_mode, GamepadRumbleMode::Strong);
    assert_eq!(
        config.input.gamepad.bindings,
        GamepadButtonBindings::default()
    );
    assert_eq!(
        config.input.gamepad.actions,
        GamepadActionBindings::default()
    );
    assert_eq!(config.input.gamepad.menu, GamepadMenuBindings::default());
    assert_eq!(
        config.input.gamepad.preferred_device,
        PreferredGamepadIdentity::default()
    );
    assert_eq!(config.machine_state, MachineStateOptions::default());
    assert_eq!(config.machine_state.normalized_autoload_slot(4), None);
    assert_eq!(config.rewind, RewindOptions::default());
    assert!(config.rewind.enabled);
    assert_eq!(
        config.rewind.history_seconds,
        DEFAULT_REWIND_HISTORY_SECONDS
    );
    assert_eq!(
        config.rewind.subframes_per_frame,
        DEFAULT_REWIND_SUBFRAMES_PER_FRAME
    );
    assert_eq!(config.rewind.max_memory_mib, DEFAULT_REWIND_MAX_MEMORY_MIB);
    assert_eq!(
        config.rewind.speed_multiplier,
        DEFAULT_REWIND_SPEED_MULTIPLIER
    );
    assert_eq!(config.fast_forward, FastForwardOptions::default());
    assert!(config.fast_forward.enabled);
    assert_eq!(
        config.fast_forward.speed_multiplier,
        DEFAULT_FAST_FORWARD_SPEED_MULTIPLIER
    );
    assert_eq!(config.fast_forward.display_speed_multiplier(), 2);
}

#[test]
fn fast_forward_options_map_retuned_display_and_runtime_presets() {
    assert_eq!(FAST_FORWARD_SPEED_MULTIPLIER_OPTIONS, [4, 8, 16]);
    assert_eq!(fast_forward_display_speed_multiplier(4), 2);
    assert_eq!(fast_forward_display_speed_multiplier(8), 4);
    assert_eq!(fast_forward_display_speed_multiplier(16), 8);
}

#[test]
fn rewind_options_map_to_core_rewind_config() {
    let options = RewindOptions::default();
    let config = options.machine_rewind_config();

    assert_eq!(
        config.target_history_t_cycles,
        u64::from(DEFAULT_REWIND_HISTORY_SECONDS) * gb_core::DMG_T_CYCLES_PER_SECOND
    );
    assert_eq!(
        config.max_estimated_bytes,
        usize::from(DEFAULT_REWIND_MAX_MEMORY_MIB) * 1024 * 1024
    );
    assert_eq!(
        config.subframe_cadence,
        MachineRewindSubframeCadence::FixedPerFrame {
            captures_per_frame: u16::from(DEFAULT_REWIND_SUBFRAMES_PER_FRAME),
        }
    );

    let disabled_subframes = RewindOptions {
        subframes_per_frame: 0,
        ..RewindOptions::default()
    }
    .machine_rewind_config();
    assert_eq!(
        disabled_subframes.subframe_cadence,
        MachineRewindSubframeCadence::Disabled
    );
}

#[test]
fn machine_state_options_normalize_autoload_slots_against_the_desktop_slot_count() {
    assert_eq!(
        MachineStateOptions {
            autoload_slot: Some(3),
        }
        .normalized_autoload_slot(4),
        Some(3)
    );
    assert_eq!(
        MachineStateOptions {
            autoload_slot: Some(5),
        }
        .normalized_autoload_slot(4),
        None
    );
}
