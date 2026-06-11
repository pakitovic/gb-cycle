use super::*;

#[test]
fn help_text_lists_host_boot_audio_and_input_overrides() {
    let text = help_text();

    assert!(text.contains("Usage:"));
    assert!(text.contains("--boot-rom-dir <dir>"));
    assert!(text.contains(
        "--revision <dmg-cpu-0|dmg-cpu-c|cpu-mgb|cpu-cgb-0|cpu-cgb-c|cpu-cgb-d|cpu-cgb-e|cpu-agb-0|cpu-agb-a>"
    ));
    assert!(!text.contains("--cgb-revision"));
    assert!(!text.contains("--boot-rom <"));
    assert!(text.contains("--test-runner"));
    assert!(text.contains("--benchmark <path>"));
    assert!(text.contains("--save-key <key>"));
    assert!(text.contains("--fullscreen"));
    assert!(text.contains("--no-rewind"));
    assert!(text.contains("--palette <grey>"));
    assert!(text.contains("--frame-blend <off|on>"));
    assert!(text.contains("--mute"));
    assert!(text.contains("--audio-record <path.wav|path.aifc>"));
    assert!(text.contains("--audio-record-rate <hz>"));
    assert!(text.contains("--audio-record-stems <all|ch1,ch2,ch3,ch4>"));
    assert!(text.contains("--link-rom <path>"));
    assert!(text.contains("--exit-after-frames <n>"));
    assert!(text.contains("--gamepad-preferred-path <path>"));
    assert!(text.contains("GB_CYCLE_DESKTOP_SETTINGS_PATH"));
    assert!(text.contains("GB_CYCLE_DESKTOP_AUDIO_LOG"));
    assert!(text.contains("GB_CYCLE_DESKTOP_AUDIO_DISABLE_AUTO_CLEAR"));
    assert!(text.contains("GB_CYCLE_DESKTOP_EMU_PROFILE"));
    assert!(text.contains("GB_CYCLE_DESKTOP_TRACE_PATH"));
    assert!(text.contains("GB_CYCLE_DESKTOP_TRACE_T_CYCLES"));
    assert!(text.contains("GB_CYCLE_DESKTOP_CH4_NR43_TRACE_PATH"));
}

#[test]
fn parse_supports_model_boot_save_and_video_overrides() {
    let action = parse_cli_arguments([
        "demo.gb",
        "--model",
        "MGB",
        "--mode",
        "experimental",
        "--boot-rom-dir",
        "firmware",
        "--boot-rom-verify",
        "warn",
        "--save-dir",
        "saves",
        "--save-key",
        "slot_1",
        "--no-rewind",
        "--fullscreen",
        "--no-vsync",
        "--mute",
        "--link-rom",
        "linked.gb",
        "--exit-after-frames",
        "120",
    ])
    .expect("host overrides should parse");

    let CliAction::Run(options) = action else {
        panic!("expected a run action");
    };

    assert_eq!(options.rom_path, Some(PathBuf::from("demo.gb")));
    assert_eq!(
        options.linked_peer_rom_path,
        Some(PathBuf::from("linked.gb"))
    );
    assert_eq!(options.exit_after_frames, Some(120));
    assert_eq!(options.audio_recording, None);
    assert_eq!(
        options.config.launch.console_model,
        DesktopConsoleModel::GameBoyPocket
    );
    assert_eq!(options.config.launch.revision, HardwareRevision::CpuMgb);
    assert_eq!(
        options.config.launch.execution_mode,
        ExecutionMode::Experimental
    );
    assert_eq!(
        options.config.boot_rom.search_path,
        Some(PathBuf::from("firmware"))
    );
    assert_eq!(
        options.config.boot_rom.verification,
        BootRomVerificationMode::Warn
    );
    assert_eq!(
        options.config.saves.directory_policy,
        SaveDirectoryPolicy::Custom(PathBuf::from("saves"))
    );
    assert!(!options.config.rewind.enabled);
    let SaveKeyPolicy::Explicit(save_key) = &options.config.saves.key_policy else {
        panic!("expected an explicit save key override");
    };
    assert_eq!(save_key.as_str(), "slot_1");
    assert!(options.config.video.fullscreen);
    assert!(!options.config.video.vsync);
    assert_eq!(
        options.config.audio,
        AudioOptions {
            enabled: false,
            ..DesktopConfig::default().audio
        }
    );
}
