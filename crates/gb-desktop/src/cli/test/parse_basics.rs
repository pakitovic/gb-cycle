use super::*;

#[test]
fn parse_defaults_to_the_expected_dmg_desktop_baseline() {
    let action = parse_cli_arguments(["roms/tetris.gb"]).expect("default CLI should parse");

    let CliAction::Run(options) = action else {
        panic!("expected run action");
    };
    assert_eq!(options.rom_path, Some(PathBuf::from("roms/tetris.gb")));
    assert_eq!(options.linked_peer_rom_path, None);
    assert_eq!(options.exit_after_frames, None);
    assert_eq!(options.audio_recording, None);
    assert!(!options.test_runner);
    assert_eq!(
        options.config.launch.console_model,
        DesktopConsoleModel::GameBoy
    );
    assert_eq!(options.config.launch.revision, HardwareRevision::DmgCpuC);
    assert_eq!(options.config.launch.startup_mode, StartupMode::SkipBoot);
    assert_eq!(options.config.launch.execution_mode, ExecutionMode::Strict);
    assert!(options.config.saves.enabled);
    assert_eq!(
        options.config.saves.flush_policy,
        DesktopSaveFlushPolicy::Debounced
    );
    assert_eq!(
        options.config.video.window_scale,
        DesktopConfig::default().video.window_scale
    );
    assert_eq!(
        options.config.video.frame_blending,
        DesktopFrameBlendingMode::Off
    );
}

#[test]
fn parse_supports_running_without_a_rom_path() {
    let action = parse_cli_arguments(["--startup", "real-boot"]).expect("CLI should allow no ROM");

    let CliAction::Run(options) = action else {
        panic!("expected run action");
    };
    assert_eq!(options.rom_path, None);
    assert_eq!(options.linked_peer_rom_path, None);
    assert_eq!(options.exit_after_frames, None);
    assert_eq!(options.audio_recording, None);
    assert_eq!(options.config.launch.startup_mode, StartupMode::RealBoot);
}

#[test]
fn parse_supports_disabling_saves_and_overriding_the_scale() {
    let action = parse_cli_arguments(["demo.gb", "--no-saves", "--scale", "6"])
        .expect("CLI overrides should parse");

    let CliAction::Run(options) = action else {
        panic!("expected run action");
    };
    assert!(!options.config.saves.enabled);
    assert_eq!(options.config.video.window_scale, 6);
}

#[test]
fn parse_supports_debounced_save_policy_overrides() {
    let action = parse_cli_arguments(["demo.gb", "--save-policy", "on-close"])
        .expect("save policy CLI overrides should parse");

    let CliAction::Run(options) = action else {
        panic!("expected run action");
    };
    assert_eq!(
        options.config.saves.flush_policy,
        DesktopSaveFlushPolicy::OnClose
    );

    let action = parse_cli_arguments(["demo.gb", "--save-policy", "debounced"])
        .expect("debounced save policy should parse");
    let CliAction::Run(options) = action else {
        panic!("expected run action");
    };
    assert_eq!(
        options.config.saves.flush_policy,
        DesktopSaveFlushPolicy::Debounced
    );
}

#[test]
fn parse_supports_direct_audio_recording_overrides() {
    let action = parse_cli_arguments([
        "demo.gb",
        "--audio-record",
        "captures/zelda.wav",
        "--audio-record-rate",
        "48000",
    ])
    .expect("audio recording CLI overrides should parse");

    let CliAction::Run(options) = action else {
        panic!("expected a run action");
    };
    assert_eq!(
        options.audio_recording,
        Some(DesktopAudioRecordingOptions {
            output_path: PathBuf::from("captures/zelda.wav"),
            sample_rate_hz: 48_000,
            stem_channels: Vec::new(),
        })
    );
}

#[test]
fn parse_supports_audio_recording_stems_overrides() {
    let action = parse_cli_arguments([
        "demo.gb",
        "--audio-record",
        "captures/zelda.wav",
        "--audio-record-stems",
        "ch1,ch4",
    ])
    .expect("audio recording stems should parse");

    let CliAction::Run(options) = action else {
        panic!("expected a run action");
    };
    assert_eq!(
        options.audio_recording,
        Some(DesktopAudioRecordingOptions {
            output_path: PathBuf::from("captures/zelda.wav"),
            sample_rate_hz: DEFAULT_AUDIO_RECORDING_SAMPLE_RATE_HZ,
            stem_channels: vec![ApuRecordedChannel::Ch1, ApuRecordedChannel::Ch4],
        })
    );

    let action = parse_cli_arguments([
        "demo.gb",
        "--audio-record",
        "captures/zelda.wav",
        "--audio-record-stems",
        "all",
    ])
    .expect("all stems should parse");

    let CliAction::Run(options) = action else {
        panic!("expected a run action");
    };
    assert_eq!(
        options.audio_recording.expect("audio recording"),
        DesktopAudioRecordingOptions {
            output_path: PathBuf::from("captures/zelda.wav"),
            sample_rate_hz: DEFAULT_AUDIO_RECORDING_SAMPLE_RATE_HZ,
            stem_channels: ApuRecordedChannel::ALL.to_vec(),
        }
    );
}

#[test]
fn parse_supports_gamepad_overrides() {
    let action = parse_cli_arguments([
        "demo.gb",
        "--no-gamepad",
        "--gamepad-direction",
        "left-stick",
        "--gamepad-face-layout",
        "south-a",
    ])
    .expect("gamepad CLI overrides should parse");

    let CliAction::Run(options) = action else {
        panic!("expected run action");
    };
    assert!(!options.config.input.gamepad.enabled);
    assert_eq!(
        options.config.input.gamepad.directional_source,
        GamepadDirectionalSource::LeftStickOnly
    );
    assert_eq!(
        options.config.input.gamepad.bindings.a,
        GamepadButtonBinding::South
    );
    assert_eq!(
        options.config.input.gamepad.bindings.b,
        GamepadButtonBinding::East
    );
}
