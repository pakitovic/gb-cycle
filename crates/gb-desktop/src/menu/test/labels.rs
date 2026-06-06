use super::*;

#[test]
fn recent_rom_titles_scroll_when_selected_for_long_enough() {
    assert_eq!(
        rendered_recent_rom_item_label("ABCDEFGHIJKLMNOP", false, Duration::from_millis(2_000)),
        "ABCDEFGHIJKLMNO"
    );
    assert_eq!(
        rendered_recent_rom_item_label("ABCDEFGHIJKLMNOP", true, Duration::from_millis(900)),
        "ABCDEFGHIJKLMNO"
    );
    assert_eq!(
        rendered_recent_rom_item_label("ABCDEFGHIJKLMNOP", true, Duration::from_millis(1_050)),
        "BCDEFGHIJKLMNOP"
    );
}

#[test]
fn pokemon_pikachu_color_gift_labels_cover_documented_rewards() {
    assert_eq!(
        PokemonPikachuColorGift::ALL.map(pokemon_pikachu_color_gift_menu_label),
        [
            "1W EON MAIL",
            "100W BERRY",
            "200W BITTER BERRY",
            "300W GREAT BALL",
            "400W MAX REPEL",
            "500W ETHER",
            "600W MIRACLEBERRY",
            "700W GOLD BERRY",
            "800W ELIXIR",
            "900W REVIVE",
            "999W RARE CANDY",
        ]
    );
}

#[test]
fn pokemon_pikachu_color_gift_label_scrolls_when_too_long() {
    let mut presentation = test_presentation();
    presentation.console_model = DesktopConsoleModel::GameBoyColor;
    presentation.pokemon_pikachu_color_gift = PokemonPikachuColorGift::Watts200;

    assert_eq!(
        rendered_item_label(
            MenuItem::CgbInfraredPikachuGift,
            false,
            presentation,
            Duration::from_millis(2_000)
        ),
        "200W BITTER BER"
    );
    assert_eq!(
        rendered_item_label(
            MenuItem::CgbInfraredPikachuGift,
            true,
            presentation,
            Duration::from_millis(1_050)
        ),
        "00W BITTER BERR"
    );
}

#[test]
fn pokemon_mystery_gift_labels_use_names_without_codes_and_scroll() {
    let mut presentation = test_presentation();
    presentation.console_model = DesktopConsoleModel::GameBoyColor;
    presentation.pokemon_mystery_gift_active = true;
    presentation.pokemon_mystery_gift_kind = PokemonMysteryGiftKind::Decoration;
    presentation.pokemon_mystery_gift_code = PokemonMysteryGiftCode::new(0x21).unwrap();

    assert_eq!(
        presentation.item_label(MenuItem::CgbInfraredMysteryGiftKind),
        "GIFT DECORATION"
    );
    assert_eq!(
        presentation.item_label(MenuItem::CgbInfraredMysteryGiftSelection),
        "SURF PIKACHU DOLL"
    );
    assert_eq!(
        rendered_item_label(
            MenuItem::CgbInfraredMysteryGiftSelection,
            false,
            presentation,
            Duration::from_millis(2_000)
        ),
        "SURF PIKACHU DO"
    );
    assert_eq!(
        rendered_item_label(
            MenuItem::CgbInfraredMysteryGiftSelection,
            true,
            presentation,
            Duration::from_millis(1_050)
        ),
        "URF PIKACHU DOL"
    );
}

#[test]
fn root_config_and_video_menu_order_matches_the_overlay_contract() {
    assert_eq!(ROOT_MENU_ITEMS[0], MenuItem::CameraLive);
    assert_eq!(ROOT_MENU_ITEMS[1], MenuItem::CameraImage);
    assert_eq!(ROOT_MENU_ITEMS[2], MenuItem::CameraReset);
    assert_eq!(ROOT_MENU_ITEMS[3], MenuItem::OpenRom);
    assert_eq!(ROOT_MENU_ITEMS[4], MenuItem::RecentMenu);
    assert_eq!(ROOT_MENU_ITEMS[5], MenuItem::SaveState);
    assert_eq!(ROOT_MENU_ITEMS[6], MenuItem::LoadState);
    assert_eq!(ROOT_MENU_ITEMS[7], MenuItem::StateSlot);
    assert_eq!(ROOT_MENU_ITEMS[8], MenuItem::StateAutoloadSlot);
    assert_eq!(ROOT_MENU_ITEMS[9], MenuItem::ExtPortMenu);
    assert_eq!(ROOT_MENU_ITEMS[10], MenuItem::CgbInfrared);
    assert_eq!(ROOT_MENU_ITEMS[11], MenuItem::ConfigMenu);
    assert_eq!(ROOT_MENU_ITEMS[12], MenuItem::Quit);
    assert!(!ROOT_MENU_ITEMS.contains(&MenuItem::SaveBattery));
    assert!(!ROOT_MENU_ITEMS.contains(&MenuItem::AudioMenu));
    assert!(!ROOT_MENU_ITEMS.contains(&MenuItem::VideoMenu));
    assert!(!ROOT_MENU_ITEMS.contains(&MenuItem::InputMenu));
    assert!(!ROOT_MENU_ITEMS.contains(&MenuItem::SystemMenu));

    assert_eq!(CONFIG_MENU_ITEMS[0], MenuItem::AudioMenu);
    assert_eq!(CONFIG_MENU_ITEMS[1], MenuItem::VideoMenu);
    assert_eq!(CONFIG_MENU_ITEMS[2], MenuItem::InputMenu);
    assert_eq!(CONFIG_MENU_ITEMS[3], MenuItem::SystemMenu);
    assert_eq!(CONFIG_MENU_ITEMS[4], MenuItem::Return);

    assert_eq!(RECENT_MENU_ITEMS[0], MenuItem::RecentRom1);
    assert_eq!(RECENT_MENU_ITEMS[7], MenuItem::RecentRom8);
    assert_eq!(RECENT_MENU_ITEMS[8], MenuItem::RecentRom9);
    assert_eq!(RECENT_MENU_ITEMS[11], MenuItem::RecentRom12);
    assert_eq!(RECENT_MENU_ITEMS[12], MenuItem::ClearRecentList);
    assert_eq!(RECENT_MENU_ITEMS[13], MenuItem::Return);

    assert_eq!(VIDEO_MENU_ITEMS[0], MenuItem::PerformanceHud);
    assert_eq!(VIDEO_MENU_ITEMS[1], MenuItem::PresentationFilter);
    assert_eq!(VIDEO_MENU_ITEMS[2], MenuItem::FrameBlending);
    assert_eq!(VIDEO_MENU_ITEMS[3], MenuItem::DisplayPalette);
    assert_eq!(VIDEO_MENU_ITEMS[8], MenuItem::Screenshot);
    assert_eq!(VIDEO_MENU_ITEMS[9], MenuItem::ShowBackground);

    assert_eq!(AUDIO_MENU_ITEMS[0], MenuItem::ToggleMute);
    assert_eq!(AUDIO_MENU_ITEMS[1], MenuItem::AudioVolume);
    assert_eq!(AUDIO_MENU_ITEMS[2], MenuItem::AudioRecord);
    assert_eq!(AUDIO_MENU_ITEMS[7], MenuItem::AudioDefaults);
    assert_eq!(AUDIO_MENU_ITEMS[8], MenuItem::Return);

    assert_eq!(INPUT_MENU_ITEMS[0], MenuItem::KeyboardMenu);
    assert_eq!(INPUT_MENU_ITEMS[1], MenuItem::KeyboardMenuControls);
    assert_eq!(INPUT_MENU_ITEMS[2], MenuItem::HotkeysMenu);
    assert_eq!(INPUT_MENU_ITEMS[3], MenuItem::GamepadMenu);
    assert_eq!(INPUT_MENU_ITEMS[4], MenuItem::GamepadMenuControls);
    assert_eq!(INPUT_MENU_ITEMS[5], MenuItem::GamepadDirection);
    assert_eq!(INPUT_MENU_ITEMS[6], MenuItem::GamepadGyro);
    assert_eq!(INPUT_MENU_ITEMS[7], MenuItem::GamepadRumble);
    assert_eq!(INPUT_MENU_ITEMS[9], MenuItem::Return);

    assert_eq!(EXT_PORT_MENU_ITEMS[0], MenuItem::ExternalPortNone);
    assert_eq!(EXT_PORT_MENU_ITEMS[1], MenuItem::ExternalPortPrinter);
    assert_eq!(EXT_PORT_MENU_ITEMS[2], MenuItem::ExternalPortGameLink);
    assert_eq!(
        EXT_PORT_MENU_ITEMS[3],
        MenuItem::ExternalPortFourPlayerAdapter
    );
    assert_eq!(EXT_PORT_MENU_ITEMS[4], MenuItem::Return);

    assert_eq!(GAME_LINK_MENU_ITEMS[0], MenuItem::GameLinkSameGame);
    assert_eq!(GAME_LINK_MENU_ITEMS[1], MenuItem::GameLinkSelectGame);
    assert_eq!(GAME_LINK_MENU_ITEMS[2], MenuItem::Return);

    assert_eq!(CGB_INFRARED_MENU_ITEMS[0], MenuItem::CgbInfraredNone);
    assert_eq!(CGB_INFRARED_MENU_ITEMS[1], MenuItem::CgbInfraredSameGame);
    assert_eq!(CGB_INFRARED_MENU_ITEMS[2], MenuItem::CgbInfraredSelectGame);
    assert_eq!(
        CGB_INFRARED_MENU_ITEMS[3],
        MenuItem::CgbInfraredPikachuColor
    );
    assert_eq!(CGB_INFRARED_MENU_ITEMS[4], MenuItem::CgbInfraredPikachuGift);
    assert_eq!(CGB_INFRARED_MENU_ITEMS[5], MenuItem::CgbInfraredMysteryGift);
    assert_eq!(
        CGB_INFRARED_MENU_ITEMS[6],
        MenuItem::CgbInfraredMysteryGiftKind
    );
    assert_eq!(
        CGB_INFRARED_MENU_ITEMS[7],
        MenuItem::CgbInfraredMysteryGiftSelection
    );
    assert_eq!(CGB_INFRARED_MENU_ITEMS[8], MenuItem::CgbInfraredHelper);
    assert_eq!(CGB_INFRARED_MENU_ITEMS[9], MenuItem::Return);

    assert_eq!(KEYBOARD_MENU_ITEMS[0], MenuItem::KeyboardUp);
    assert_eq!(KEYBOARD_MENU_ITEMS[8], MenuItem::Return);
    assert_eq!(KEYBOARD_MENU_CONTROL_ITEMS[0], MenuItem::KeyboardMenuUp);
    assert_eq!(KEYBOARD_MENU_CONTROL_ITEMS[4], MenuItem::Return);
    assert_eq!(HOTKEYS_MENU_ITEMS[0], MenuItem::HotkeyPause);
    assert_eq!(HOTKEYS_MENU_ITEMS[7], MenuItem::HotkeyRewind);
    assert_eq!(HOTKEYS_MENU_ITEMS[8], MenuItem::HotkeyFastForward);
    assert_eq!(HOTKEYS_MENU_ITEMS[13], MenuItem::Return);
    assert_eq!(GAMEPAD_MENU_ITEMS[0], MenuItem::GamepadActive);
    assert_eq!(GAMEPAD_MENU_ITEMS[10], MenuItem::GamepadSaveState);
    assert_eq!(GAMEPAD_MENU_ITEMS[11], MenuItem::GamepadLoadState);
    assert_eq!(GAMEPAD_MENU_ITEMS[12], MenuItem::GamepadRewind);
    assert_eq!(GAMEPAD_MENU_ITEMS[13], MenuItem::GamepadFastForward);
    assert_eq!(GAMEPAD_MENU_ITEMS[14], MenuItem::Return);
    assert_eq!(GAMEPAD_MENU_CONTROL_ITEMS[0], MenuItem::GamepadMenuUp);
    assert_eq!(GAMEPAD_MENU_CONTROL_ITEMS[4], MenuItem::Return);

    assert_eq!(SYSTEM_MENU_ITEMS[0], MenuItem::ConsoleModel);
    assert_eq!(SYSTEM_MENU_ITEMS[1], MenuItem::HardwareRevision);
    assert_eq!(SYSTEM_MENU_ITEMS[2], MenuItem::SgbVideoStandard);
    assert_eq!(SYSTEM_MENU_ITEMS[3], MenuItem::SgbBorder);
    assert_eq!(SYSTEM_MENU_ITEMS[4], MenuItem::StartupMode);
    assert_eq!(SYSTEM_MENU_ITEMS[5], MenuItem::ExecutionMode);
    assert_eq!(SYSTEM_MENU_ITEMS[6], MenuItem::BootRomMenu);
    assert_eq!(SYSTEM_MENU_ITEMS[7], MenuItem::SaveMenu);
    assert_eq!(SYSTEM_MENU_ITEMS[8], MenuItem::RewindMenu);
    assert_eq!(SYSTEM_MENU_ITEMS[9], MenuItem::FastForwardMenu);
    assert_eq!(SYSTEM_MENU_ITEMS[10], MenuItem::Reset);
    assert_eq!(SYSTEM_MENU_ITEMS[11], MenuItem::Return);
    assert!(!SYSTEM_MENU_ITEMS.contains(&MenuItem::SaveBattery));

    assert_eq!(REWIND_MENU_ITEMS[0], MenuItem::RewindEnabled);
    assert_eq!(REWIND_MENU_ITEMS[1], MenuItem::RewindHistory);
    assert_eq!(REWIND_MENU_ITEMS[2], MenuItem::RewindSubframes);
    assert_eq!(REWIND_MENU_ITEMS[3], MenuItem::RewindSpeed);
    assert_eq!(REWIND_MENU_ITEMS[4], MenuItem::RewindMemory);
    assert_eq!(REWIND_MENU_ITEMS[5], MenuItem::RewindDefaults);
    assert_eq!(REWIND_MENU_ITEMS[6], MenuItem::Return);

    assert_eq!(FAST_FORWARD_MENU_ITEMS[0], MenuItem::FastForwardEnabled);
    assert_eq!(FAST_FORWARD_MENU_ITEMS[1], MenuItem::FastForwardSpeed);
    assert_eq!(FAST_FORWARD_MENU_ITEMS[2], MenuItem::FastForwardDefaults);
    assert_eq!(FAST_FORWARD_MENU_ITEMS[3], MenuItem::Return);

    assert_eq!(BOOT_ROM_MENU_ITEMS[0], MenuItem::BootRomDirectoryPath);
    assert_eq!(BOOT_ROM_MENU_ITEMS[1], MenuItem::BootRomVerify);
    assert_eq!(BOOT_ROM_MENU_ITEMS[2], MenuItem::Return);
    assert!(!BOOT_ROM_MENU_ITEMS.contains(&MenuItem::ConsoleModel));
    assert!(!BOOT_ROM_MENU_ITEMS.contains(&MenuItem::StartupMode));

    assert_eq!(SAVE_MENU_ITEMS[0], MenuItem::ExportSave);
    assert_eq!(SAVE_MENU_ITEMS[1], MenuItem::ImportSave);
    assert_eq!(SAVE_MENU_ITEMS[2], MenuItem::SaveBattery);
    assert_eq!(SAVE_MENU_ITEMS[3], MenuItem::SavesEnabled);
    assert_eq!(SAVE_MENU_ITEMS[4], MenuItem::SavePolicy);
    assert_eq!(SAVE_MENU_ITEMS[5], MenuItem::SaveDefaultPath);
    assert_eq!(SAVE_MENU_ITEMS[6], MenuItem::SaveDirectoryPath);
    assert_eq!(SAVE_MENU_ITEMS[7], MenuItem::Return);
}

#[test]
fn camera_root_actions_appear_before_general_rom_menus_when_supported() {
    let presentation = MenuPresentation {
        cartridge_pocket_camera_supported: true,
        recent_rom_count: 1,
        manual_save_available: true,
        ..test_presentation()
    };

    assert_eq!(
        visible_item_at(MenuScreen::Root, 0, presentation),
        Some(MenuItem::CameraLive)
    );
    assert_eq!(
        visible_item_at(MenuScreen::Root, 1, presentation),
        Some(MenuItem::CameraImage)
    );
    assert_eq!(
        visible_item_at(MenuScreen::Root, 2, presentation),
        Some(MenuItem::CameraReset)
    );
    assert_eq!(
        visible_item_at(MenuScreen::Root, 3, presentation),
        Some(MenuItem::OpenRom)
    );
}

#[test]
fn menu_item_labels_cover_runtime_variants_and_binding_summaries() {
    let mut presentation = test_presentation();
    presentation.recent_rom_count = 12;
    for (index, label) in [
        "TETRIS",
        "MARIO",
        "DRMARIO",
        "KIRBY",
        "ZELDA",
        "WARIO",
        "METROID",
        "TENNIS",
        "ALLEYWAY",
        "FZERO",
        "DONKEY",
        "MOLEMANIA",
    ]
    .into_iter()
    .enumerate()
    {
        presentation.recent_rom_labels[index] = CompactRecentRomLabel::from_text(label);
    }
    assert_eq!(presentation.item_label(MenuItem::RecentRom1), "TETRIS");
    assert_eq!(presentation.item_label(MenuItem::RecentRom3), "DRMARIO");
    assert_eq!(presentation.item_label(MenuItem::RecentRom4), "KIRBY");
    assert_eq!(presentation.item_label(MenuItem::RecentRom5), "ZELDA");
    assert_eq!(presentation.item_label(MenuItem::RecentRom6), "WARIO");
    assert_eq!(presentation.item_label(MenuItem::RecentRom7), "METROID");
    assert_eq!(presentation.item_label(MenuItem::RecentRom8), "TENNIS");
    assert_eq!(presentation.item_label(MenuItem::RecentRom9), "ALLEYWAY");
    assert_eq!(presentation.item_label(MenuItem::RecentRom10), "FZERO");
    assert_eq!(presentation.item_label(MenuItem::RecentRom11), "DONKEY");
    assert_eq!(presentation.item_label(MenuItem::RecentRom12), "MOLEMANIA");
    assert_eq!(
        presentation.item_label(MenuItem::ClearRecentList),
        "CLEAR LIST"
    );

    presentation.console_model = DesktopConsoleModel::GameBoy;
    assert_eq!(
        presentation.item_label(MenuItem::ConsoleModel),
        "MODEL GAME BOY"
    );
    presentation.console_model = DesktopConsoleModel::GameBoyPocket;
    assert_eq!(
        presentation.item_label(MenuItem::ConsoleModel),
        "MODEL GB POCKET"
    );
    presentation.console_model = DesktopConsoleModel::GameBoyLight;
    assert_eq!(
        presentation.item_label(MenuItem::ConsoleModel),
        "MODEL GB LIGHT"
    );
    presentation.console_model = DesktopConsoleModel::GameBoyColor;
    assert_eq!(
        presentation.item_label(MenuItem::ConsoleModel),
        "MODEL GB COLOR"
    );
    presentation.console_model = DesktopConsoleModel::GameBoyAdvance;
    presentation.revision = HardwareRevision::CpuAgbA;
    assert_eq!(
        presentation.item_label(MenuItem::ConsoleModel),
        "MODEL GB ADVANCE"
    );
    assert_eq!(
        presentation.item_label(MenuItem::HardwareRevision),
        "REV CPU AGB A"
    );
    presentation.console_model = DesktopConsoleModel::SuperGameBoy;
    assert_eq!(
        presentation.item_label(MenuItem::ConsoleModel),
        "MODEL SUPER GB"
    );
    assert_eq!(
        presentation.item_label(MenuItem::HardwareRevision),
        "REV SGB-CPU 01"
    );
    presentation.console_model = DesktopConsoleModel::SuperGameBoy2;
    assert_eq!(
        presentation.item_label(MenuItem::ConsoleModel),
        "MODEL SUPER GB2"
    );
    assert_eq!(
        presentation.item_label(MenuItem::HardwareRevision),
        "REV CPU SGB2"
    );
    presentation.console_model = DesktopConsoleModel::GameBoyColor;
    presentation.revision = HardwareRevision::CpuCgbC;
    assert_eq!(
        presentation.item_label(MenuItem::HardwareRevision),
        "REV CPU CGB C"
    );
    presentation.revision = HardwareRevision::CpuCgbD;
    assert_eq!(
        presentation.item_label(MenuItem::HardwareRevision),
        "REV CPU CGB D"
    );
    presentation.revision = HardwareRevision::CpuCgbE;
    assert_eq!(
        presentation.item_label(MenuItem::HardwareRevision),
        "REV CPU CGB E"
    );

    presentation.startup_mode = StartupMode::RealBoot;
    assert_eq!(presentation.item_label(MenuItem::StartupMode), "START REAL");
    presentation.startup_mode = StartupMode::CustomBoot;
    assert_eq!(
        presentation.item_label(MenuItem::StartupMode),
        "START CUSTOM"
    );
    presentation.execution_mode = ExecutionMode::Permissive;
    assert_eq!(
        presentation.item_label(MenuItem::ExecutionMode),
        "MODE PERM"
    );
    presentation.execution_mode = ExecutionMode::Experimental;
    assert_eq!(presentation.item_label(MenuItem::ExecutionMode), "MODE EXP");

    presentation.boot_rom_verification = BootRomVerificationMode::Warn;
    assert_eq!(
        presentation.item_label(MenuItem::BootRomVerify),
        "VERIFY WARN"
    );
    presentation.boot_rom_verification = BootRomVerificationMode::Off;
    assert_eq!(
        presentation.item_label(MenuItem::BootRomVerify),
        "VERIFY OFF"
    );

    assert_eq!(presentation.item_label(MenuItem::BootRomMenu), "BOOT ROM");
    assert_eq!(presentation.item_label(MenuItem::SaveMenu), "SAVE");
    assert_eq!(presentation.item_label(MenuItem::RewindMenu), "REWIND");
    assert_eq!(
        presentation.item_label(MenuItem::FastForwardMenu),
        "F-FORWARD"
    );
    assert_eq!(
        presentation.item_label(MenuItem::RewindEnabled),
        "REWIND ON"
    );
    assert_eq!(
        presentation.item_label(MenuItem::RewindHistory),
        "HISTORY 10S"
    );
    assert_eq!(
        presentation.item_label(MenuItem::RewindSubframes),
        "SUBFR 1"
    );
    assert_eq!(presentation.item_label(MenuItem::RewindSpeed), "SPEED 2X");
    assert_eq!(
        presentation.item_label(MenuItem::RewindMemory),
        "MEMORY 256M"
    );
    assert_eq!(
        presentation.item_label(MenuItem::RewindDefaults),
        "DEFAULTS"
    );
    assert_eq!(
        presentation.item_label(MenuItem::FastForwardEnabled),
        "F-FORWARD ON"
    );
    assert_eq!(
        presentation.item_label(MenuItem::FastForwardSpeed),
        "SPEED 2X"
    );
    assert_eq!(
        presentation.item_label(MenuItem::FastForwardDefaults),
        "DEFAULTS"
    );
    presentation.rewind_options.enabled = false;
    presentation.rewind_options.history_seconds = 20;
    presentation.rewind_options.subframes_per_frame = 0;
    presentation.rewind_options.speed_multiplier = 4;
    presentation.rewind_options.max_memory_mib = 128;
    assert_eq!(
        presentation.item_label(MenuItem::RewindEnabled),
        "REWIND OFF"
    );
    assert_eq!(
        presentation.item_label(MenuItem::RewindHistory),
        "HISTORY 20S"
    );
    assert_eq!(
        presentation.item_label(MenuItem::RewindSubframes),
        "SUBFR OFF"
    );
    assert_eq!(presentation.item_label(MenuItem::RewindSpeed), "SPEED 4X");
    assert_eq!(
        presentation.item_label(MenuItem::RewindMemory),
        "MEMORY 128M"
    );
    presentation.fast_forward_options.enabled = false;
    presentation.fast_forward_options.speed_multiplier = 8;
    assert_eq!(
        presentation.item_label(MenuItem::FastForwardEnabled),
        "F-FORWARD OFF"
    );
    assert_eq!(
        presentation.item_label(MenuItem::FastForwardSpeed),
        "SPEED 4X"
    );
    assert_eq!(presentation.item_label(MenuItem::ExportSave), "EXPORT SAVE");
    assert_eq!(presentation.item_label(MenuItem::ImportSave), "IMPORT SAVE");
    assert!(!presentation.item_enabled(MenuItem::ExportSave));
    assert!(!presentation.item_enabled(MenuItem::ImportSave));
    presentation.external_save_available = true;
    assert!(presentation.item_enabled(MenuItem::ExportSave));
    presentation.external_save_import_available = true;
    assert!(presentation.item_enabled(MenuItem::ImportSave));
    presentation.any_dialog_pending = true;
    assert!(!presentation.item_enabled(MenuItem::ExportSave));
    assert!(!presentation.item_enabled(MenuItem::ImportSave));
    presentation.any_dialog_pending = false;
    presentation.saves_enabled = false;
    assert_eq!(presentation.item_label(MenuItem::SavesEnabled), "SAVES OFF");
    presentation.save_flush_policy = DesktopSaveFlushPolicy::Manual;
    assert_eq!(presentation.item_label(MenuItem::SavePolicy), "SAVE MANUAL");
    presentation.save_flush_policy = DesktopSaveFlushPolicy::OnClose;
    assert_eq!(presentation.item_label(MenuItem::SavePolicy), "SAVE CLOSE");
    presentation.save_flush_policy = DesktopSaveFlushPolicy::OnWrite;
    assert_eq!(presentation.item_label(MenuItem::SavePolicy), "SAVE WRITE");
    presentation.save_directory_uses_default_path = false;
    assert_eq!(
        presentation.item_label(MenuItem::SaveDefaultPath),
        "DIR AUTO OFF"
    );

    presentation.fullscreen = true;
    assert_eq!(
        presentation.item_label(MenuItem::Fullscreen),
        "FULLSCREEN ON"
    );
    presentation.vsync = false;
    assert_eq!(presentation.item_label(MenuItem::Vsync), "VSYNC OFF");
    presentation.integer_scale = false;
    assert_eq!(
        presentation.item_label(MenuItem::IntegerScale),
        "INTEGER OFF"
    );
    presentation.presentation_filter = true;
    assert_eq!(
        presentation.item_label(MenuItem::PresentationFilter),
        "FILTER ON"
    );
    for (frame_blending, expected_label) in [
        (DesktopFrameBlendingMode::Off, "BLEND OFF"),
        (DesktopFrameBlendingMode::On, "BLEND ON"),
    ] {
        presentation.frame_blending = frame_blending;
        assert_eq!(
            presentation.item_label(MenuItem::FrameBlending),
            expected_label
        );
    }
    presentation.console_model = DesktopConsoleModel::GameBoyColor;
    assert_eq!(
        presentation.item_label(MenuItem::DisplayPalette),
        "PALETTE RGB555"
    );
    presentation.console_model = DesktopConsoleModel::GameBoyAdvance;
    assert_eq!(
        presentation.item_label(MenuItem::DisplayPalette),
        "PALETTE RGB555"
    );
    presentation.console_model = DesktopConsoleModel::SuperGameBoy;
    assert_eq!(
        presentation.item_label(MenuItem::DisplayPalette),
        "PALETTE SGB"
    );
    presentation.console_model = DesktopConsoleModel::SuperGameBoy2;
    assert_eq!(
        presentation.item_label(MenuItem::DisplayPalette),
        "PALETTE SGB2"
    );
    presentation.console_model = DesktopConsoleModel::GameBoy;
    for (display_palette, expected_label) in [
        (DesktopDisplayPalette::Grey, "PALETTE GREY"),
        (DesktopDisplayPalette::GameBoy, "PALETTE GB"),
        (DesktopDisplayPalette::Pocket, "PALETTE POCKET"),
        (DesktopDisplayPalette::Light, "PALETTE LIGHT"),
    ] {
        presentation.display_palette = display_palette;
        let label = presentation.item_label(MenuItem::DisplayPalette);
        assert_eq!(label, expected_label);
        assert!(label.len() <= super::super::MENU_ITEM_TEXT_CAPACITY);
    }
    presentation.show_background = false;
    assert_eq!(
        presentation.item_label(MenuItem::ShowBackground),
        "BACKGROUND OFF"
    );
    presentation.show_window = false;
    assert_eq!(presentation.item_label(MenuItem::ShowWindow), "WINDOW OFF");
    presentation.show_objects = false;
    assert_eq!(
        presentation.item_label(MenuItem::ShowObjects),
        "OBJECTS OFF"
    );
    assert_eq!(presentation.item_label(MenuItem::Screenshot), "SCREENSHOT");
    presentation.show_performance_hud = false;
    assert_eq!(
        presentation.item_label(MenuItem::PerformanceHud),
        "STATS OFF"
    );
    assert_eq!(presentation.item_label(MenuItem::ConfigMenu), "CONFIG");
    presentation.muted = true;
    assert_eq!(presentation.item_label(MenuItem::ToggleMute), "MUTE ON");
    presentation.audio_volume_percent = 250;
    assert_eq!(presentation.item_label(MenuItem::AudioVolume), "VOL 100%");

    assert_eq!(presentation.item_label(MenuItem::ExtPortMenu), "EXT: NONE");
    assert!(presentation.item_enabled(MenuItem::ExtPortMenu));
    let no_rom_presentation = MenuPresentation {
        rom_loaded: false,
        ..presentation
    };
    assert_eq!(
        no_rom_presentation.item_label(MenuItem::ExtPortMenu),
        "EXT: NONE"
    );
    assert!(!no_rom_presentation.item_enabled(MenuItem::ExtPortMenu));
    assert_eq!(
        presentation.item_label(MenuItem::ExternalPortNone),
        "NONE ✓"
    );
    presentation.external_port_selection = DesktopExternalPortSelection::Printer;
    assert_eq!(
        presentation.item_label(MenuItem::ExtPortMenu),
        "EXT: PRINTER"
    );
    assert_eq!(presentation.item_label(MenuItem::ExternalPortNone), "NONE");
    assert_eq!(
        presentation.item_label(MenuItem::ExternalPortPrinter),
        "PRINTER ✓"
    );
    assert_eq!(
        presentation.item_label(MenuItem::ExternalPortGameLink),
        "GAME LINK"
    );
    assert_eq!(
        presentation.item_label(MenuItem::GameLinkSameGame),
        "SAME GAME"
    );
    assert_eq!(
        presentation.item_label(MenuItem::GameLinkSelectGame),
        "SELECT GAME"
    );
    assert_eq!(
        presentation.item_label(MenuItem::ExternalPortFourPlayerAdapter),
        "4P ADAPTER"
    );
    assert!(presentation.item_enabled(MenuItem::ExternalPortGameLink));
    assert!(presentation.item_enabled(MenuItem::GameLinkSameGame));
    assert!(presentation.item_enabled(MenuItem::GameLinkSelectGame));
    assert!(presentation.item_enabled(MenuItem::ExternalPortFourPlayerAdapter));
    assert_eq!(
        presentation.item_label(MenuItem::FourPlayerAdapterTwoPlayers),
        "2 PLAYERS"
    );
    assert_eq!(
        presentation.item_label(MenuItem::FourPlayerAdapterThreePlayers),
        "3 PLAYERS"
    );
    assert_eq!(
        presentation.item_label(MenuItem::FourPlayerAdapterFourPlayers),
        "4 PLAYERS"
    );
    presentation.any_dialog_pending = true;
    assert!(!presentation.item_enabled(MenuItem::ExternalPortGameLink));
    assert!(!presentation.item_enabled(MenuItem::GameLinkSameGame));
    assert!(!presentation.item_enabled(MenuItem::GameLinkSelectGame));
    assert!(!presentation.item_enabled(MenuItem::ExternalPortFourPlayerAdapter));
    presentation.any_dialog_pending = false;
    presentation.external_port_selection = DesktopExternalPortSelection::GameLink;
    assert_eq!(
        presentation.item_label(MenuItem::ExtPortMenu),
        "EXT: GAME LINK"
    );
    assert_eq!(
        presentation.item_label(MenuItem::ExternalPortGameLink),
        "GAME LINK ✓"
    );
    assert_eq!(
        presentation.item_label(MenuItem::ExternalPortFourPlayerAdapter),
        "4P ADAPTER"
    );
    presentation.external_port_selection = DesktopExternalPortSelection::FourPlayerAdapter;
    assert_eq!(
        presentation.item_label(MenuItem::ExtPortMenu),
        "EXT: 4P ADAPTER"
    );
    assert_eq!(
        presentation.item_label(MenuItem::ExternalPortGameLink),
        "GAME LINK"
    );
    assert_eq!(
        presentation.item_label(MenuItem::ExternalPortFourPlayerAdapter),
        "4P ADAPTER ✓"
    );
    presentation.console_model = DesktopConsoleModel::SuperGameBoy;
    assert!(!presentation.item_enabled(MenuItem::ExtPortMenu));
    assert!(!presentation.item_enabled(MenuItem::ExternalPortNone));
    assert!(!presentation.item_enabled(MenuItem::ExternalPortPrinter));
    assert!(!presentation.item_enabled(MenuItem::ExternalPortGameLink));
    presentation.console_model = DesktopConsoleModel::SuperGameBoy2;
    assert!(presentation.item_enabled(MenuItem::ExtPortMenu));
    assert!(presentation.item_enabled(MenuItem::ExternalPortNone));
    assert!(presentation.item_enabled(MenuItem::ExternalPortPrinter));
    assert!(presentation.item_enabled(MenuItem::ExternalPortGameLink));
    presentation.console_model = DesktopConsoleModel::GameBoyColor;
    assert!(presentation.item_enabled(MenuItem::ExtPortMenu));
    assert!(presentation.item_enabled(MenuItem::ExternalPortNone));
    assert!(presentation.item_enabled(MenuItem::ExternalPortPrinter));
    assert!(presentation.item_enabled(MenuItem::ExternalPortGameLink));

    presentation.console_model = DesktopConsoleModel::GameBoy;
    assert_eq!(presentation.item_label(MenuItem::CgbInfrared), "IR: NONE");
    assert_eq!(presentation.item_label(MenuItem::CgbInfraredNone), "NONE ✓");
    assert_eq!(
        presentation.item_label(MenuItem::CgbInfraredSameGame),
        "SAME GAME"
    );
    assert_eq!(
        presentation.item_label(MenuItem::CgbInfraredSelectGame),
        "SELECT GAME"
    );
    assert_eq!(
        presentation.item_label(MenuItem::CgbInfraredHelper),
        "HELPER OFF"
    );
    assert_eq!(
        presentation.item_label(MenuItem::CgbInfraredPikachuColor),
        "PIKACHU 2"
    );
    assert_eq!(
        presentation.item_label(MenuItem::CgbInfraredPikachuGift),
        "1W EON MAIL"
    );
    assert_eq!(
        presentation.item_label(MenuItem::CgbInfraredMysteryGift),
        "MYSTERY GIFT"
    );
    assert_eq!(
        presentation.item_label(MenuItem::CgbInfraredMysteryGiftKind),
        "GIFT ITEM"
    );
    assert_eq!(
        presentation.item_label(MenuItem::CgbInfraredMysteryGiftSelection),
        "BERRY"
    );
    assert!(!presentation.item_visible(MenuItem::CgbInfrared));
    assert!(!presentation.item_enabled(MenuItem::CgbInfrared));
    let cgb_no_rom_presentation = MenuPresentation {
        rom_loaded: false,
        console_model: DesktopConsoleModel::GameBoyColor,
        ..presentation
    };
    assert_eq!(
        cgb_no_rom_presentation.item_label(MenuItem::CgbInfrared),
        "IR: NONE"
    );
    assert!(cgb_no_rom_presentation.item_visible(MenuItem::CgbInfrared));
    assert!(!cgb_no_rom_presentation.item_enabled(MenuItem::CgbInfrared));
    presentation.console_model = DesktopConsoleModel::GameBoyColor;
    assert!(presentation.item_visible(MenuItem::CgbInfrared));
    assert!(presentation.item_enabled(MenuItem::CgbInfrared));
    assert!(presentation.item_visible(MenuItem::CgbInfraredNone));
    assert!(presentation.item_visible(MenuItem::CgbInfraredSameGame));
    assert!(presentation.item_visible(MenuItem::CgbInfraredSelectGame));
    assert!(presentation.item_visible(MenuItem::CgbInfraredPikachuColor));
    assert!(presentation.item_visible(MenuItem::CgbInfraredPikachuGift));
    assert!(presentation.item_visible(MenuItem::CgbInfraredMysteryGift));
    assert!(presentation.item_visible(MenuItem::CgbInfraredMysteryGiftKind));
    assert!(presentation.item_visible(MenuItem::CgbInfraredMysteryGiftSelection));
    assert!(presentation.item_visible(MenuItem::CgbInfraredHelper));
    presentation.any_dialog_pending = true;
    assert!(!presentation.item_enabled(MenuItem::CgbInfrared));
    assert!(!presentation.item_enabled(MenuItem::CgbInfraredSameGame));
    assert!(!presentation.item_enabled(MenuItem::CgbInfraredSelectGame));
    assert!(!presentation.item_enabled(MenuItem::CgbInfraredPikachuColor));
    assert!(!presentation.item_enabled(MenuItem::CgbInfraredMysteryGift));
    presentation.any_dialog_pending = false;
    assert!(!presentation.item_enabled(MenuItem::CgbInfraredPikachuGift));
    assert!(!presentation.item_enabled(MenuItem::CgbInfraredMysteryGiftKind));
    assert!(!presentation.item_enabled(MenuItem::CgbInfraredMysteryGiftSelection));
    assert!(presentation.item_enabled(MenuItem::CgbInfraredHelper));
    presentation.show_cgb_infrared_helper = true;
    assert_eq!(
        presentation.item_label(MenuItem::CgbInfraredHelper),
        "HELPER ON"
    );
    presentation.show_cgb_infrared_helper = false;
    presentation.cgb_infrared_link_active = true;
    presentation.cgb_infrared_same_game_active = true;
    assert_eq!(
        presentation.item_label(MenuItem::CgbInfrared),
        "IR: SAME GAME"
    );
    assert_eq!(presentation.item_label(MenuItem::CgbInfraredNone), "NONE");
    assert_eq!(
        presentation.item_label(MenuItem::CgbInfraredSameGame),
        "SAME GAME ✓"
    );
    assert_eq!(
        presentation.item_label(MenuItem::CgbInfraredSelectGame),
        "SELECT GAME"
    );
    presentation.cgb_infrared_same_game_active = false;
    assert_eq!(
        presentation.item_label(MenuItem::CgbInfrared),
        "IR: SELECT GAME"
    );
    assert_eq!(
        presentation.item_label(MenuItem::CgbInfraredSelectGame),
        "SELECT GAME ✓"
    );
    presentation.cgb_infrared_link_active = false;
    presentation.pokemon_pikachu_color_active = true;
    presentation.pokemon_pikachu_color_gift = PokemonPikachuColorGift::Watts999;
    assert_eq!(
        presentation.item_label(MenuItem::CgbInfrared),
        "IR: PIKACHU 2"
    );
    assert_eq!(
        presentation.item_label(MenuItem::CgbInfraredPikachuColor),
        "PIKACHU 2 ✓"
    );
    assert!(presentation.item_enabled(MenuItem::CgbInfraredPikachuGift));
    assert_eq!(
        presentation.item_label(MenuItem::CgbInfraredPikachuGift),
        "999W RARE CANDY"
    );
    presentation.pokemon_pikachu_color_active = false;
    presentation.pokemon_mystery_gift_active = true;
    presentation.pokemon_mystery_gift_kind = PokemonMysteryGiftKind::Decoration;
    presentation.pokemon_mystery_gift_code = PokemonMysteryGiftCode::new(0x0D).unwrap();
    assert_eq!(
        presentation.item_label(MenuItem::CgbInfrared),
        "IR: MYSTERY GIFT"
    );
    assert_eq!(
        presentation.item_label(MenuItem::CgbInfraredMysteryGift),
        "MYSTERY GIFT ✓"
    );
    assert!(presentation.item_enabled(MenuItem::CgbInfraredMysteryGiftKind));
    assert!(presentation.item_enabled(MenuItem::CgbInfraredMysteryGiftSelection));
    assert_eq!(
        presentation.item_label(MenuItem::CgbInfraredMysteryGiftSelection),
        "WEEDLE DOLL"
    );

    presentation.gamepad_directional_source = GamepadDirectionalSource::DpadOnly;
    assert_eq!(
        presentation.item_label(MenuItem::GamepadDirection),
        "DIR DPAD"
    );
    presentation.gamepad_directional_source = GamepadDirectionalSource::LeftStickOnly;
    assert_eq!(
        presentation.item_label(MenuItem::GamepadDirection),
        "DIR LEFT"
    );
    assert!(!presentation.item_enabled(MenuItem::GamepadGyro));
    assert_eq!(presentation.item_label(MenuItem::GamepadGyro), "GYRO N/A");
    presentation.active_gamepad_connected = true;
    assert!(!presentation.item_enabled(MenuItem::GamepadGyro));
    presentation.cartridge_mbc7_accelerometer_supported = true;
    assert!(presentation.item_enabled(MenuItem::GamepadGyro));
    assert_eq!(presentation.item_label(MenuItem::GamepadGyro), "GYRO OFF");
    presentation.gamepad_gyro_mode = GamepadGyroMode::PadInput;
    assert_eq!(
        presentation.item_label(MenuItem::GamepadGyro),
        "GYRO PAD INPUT"
    );
    presentation.gamepad_gyro_mode = GamepadGyroMode::PadGyro;
    assert_eq!(presentation.item_label(MenuItem::GamepadGyro), "GYRO N/A");
    presentation.active_gamepad_accelerometer_supported = true;
    assert_eq!(
        presentation.item_label(MenuItem::GamepadGyro),
        "GYRO PAD GYRO"
    );
    assert!(!presentation.item_enabled(MenuItem::GamepadRumble));
    assert_eq!(
        presentation.item_label(MenuItem::GamepadRumble),
        "RUMBLE N/A"
    );
    presentation.cartridge_rumble_supported = true;
    presentation.active_gamepad_rumble_supported = true;
    assert!(presentation.item_enabled(MenuItem::GamepadRumble));
    assert_eq!(
        presentation.item_label(MenuItem::GamepadRumble),
        "RUMBLE HIGH"
    );
    presentation.gamepad_rumble_mode = GamepadRumbleMode::Weak;
    assert_eq!(
        presentation.item_label(MenuItem::GamepadRumble),
        "RUMBLE LOW"
    );
    presentation.gamepad_rumble_mode = GamepadRumbleMode::Off;
    assert_eq!(
        presentation.item_label(MenuItem::GamepadRumble),
        "RUMBLE OFF"
    );
    assert!(!presentation.item_visible(MenuItem::CameraImage));
    assert!(!presentation.item_visible(MenuItem::CameraLive));
    assert!(!presentation.item_visible(MenuItem::CameraReset));
    presentation.cartridge_pocket_camera_supported = true;
    assert!(presentation.item_visible(MenuItem::CameraImage));
    assert!(presentation.item_visible(MenuItem::CameraLive));
    assert!(presentation.item_visible(MenuItem::CameraReset));
    assert_eq!(presentation.item_label(MenuItem::CameraImage), "CAM IMAGE");
    assert_eq!(
        presentation.item_label(MenuItem::CameraLive),
        "CAM LIVE OFF"
    );
    presentation.pocket_camera_live_enabled = true;
    assert_eq!(presentation.item_label(MenuItem::CameraLive), "CAM LIVE ON");
    presentation.pocket_camera_live_enabled = false;
    assert_eq!(presentation.item_label(MenuItem::CameraReset), "CAM RESET");
    assert!(presentation.item_enabled(MenuItem::CameraImage));
    assert!(presentation.item_enabled(MenuItem::CameraLive));
    assert!(presentation.item_enabled(MenuItem::CameraReset));
    presentation.any_dialog_pending = true;
    assert!(!presentation.item_enabled(MenuItem::CameraImage));
    assert!(!presentation.item_enabled(MenuItem::CameraLive));
    assert!(presentation.item_enabled(MenuItem::CameraReset));
    presentation.any_dialog_pending = false;

    presentation.active_gamepad_connected = true;
    presentation.active_gamepad_label = CompactMenuLabel::from_text("SWITCH");
    assert_eq!(
        presentation.item_label(MenuItem::GamepadActive),
        "ACTIVE SWITCH"
    );
    presentation.active_gamepad_connected = false;
    presentation.active_gamepad_label = CompactMenuLabel::default();
    assert_eq!(
        presentation.item_label(MenuItem::GamepadActive),
        "ACTIVE NONE"
    );
    assert_eq!(
        presentation.item_label(MenuItem::GamepadPreferred),
        "PREF AUTO"
    );
    presentation.preferred_gamepad_configured = true;
    assert_eq!(
        presentation.item_label(MenuItem::GamepadPreferred),
        "PREF SAVED"
    );
    presentation.preferred_gamepad_label = CompactMenuLabel::from_text("ARCADE");
    assert_eq!(
        presentation.item_label(MenuItem::GamepadPreferred),
        "PREF ARCADE"
    );

    presentation.gamepad_bindings = GamepadButtonBindings {
        up: GamepadButtonBinding::DPadUp,
        down: GamepadButtonBinding::DPadDown,
        left: GamepadButtonBinding::DPadLeft,
        right: GamepadButtonBinding::DPadRight,
        a: GamepadButtonBinding::North,
        b: GamepadButtonBinding::West,
        select: GamepadButtonBinding::Back,
        start: GamepadButtonBinding::Guide,
    };
    assert_eq!(presentation.item_label(MenuItem::GamepadUp), "UP D UP");
    assert_eq!(
        presentation.item_label(MenuItem::GamepadDown),
        "DOWN D DOWN"
    );
    assert_eq!(
        presentation.item_label(MenuItem::GamepadLeft),
        "LEFT D LEFT"
    );
    assert_eq!(
        presentation.item_label(MenuItem::GamepadRight),
        "RIGHT D RIGHT"
    );
    assert_eq!(presentation.item_label(MenuItem::GamepadA), "A NORTH");
    assert_eq!(presentation.item_label(MenuItem::GamepadB), "B WEST");
    assert_eq!(
        presentation.item_label(MenuItem::GamepadSelect),
        "SELECT BACK"
    );
    assert_eq!(
        presentation.item_label(MenuItem::GamepadStart),
        "START GUIDE"
    );
    assert_eq!(
        presentation.item_label(MenuItem::GamepadSaveState),
        "SAVE STATE NONE"
    );
    assert_eq!(
        presentation.item_label(MenuItem::GamepadLoadState),
        "LOAD STATE NONE"
    );
    assert_eq!(
        presentation.item_label(MenuItem::GamepadRewind),
        "REWIND NONE"
    );
    assert_eq!(
        presentation.item_label(MenuItem::GamepadFastForward),
        "F-FORWARD NONE"
    );
    presentation.gamepad_action_bindings = GamepadActionBindings {
        save_state: Some(GamepadButtonBinding::LeftShoulder),
        load_state: Some(GamepadButtonBinding::RightShoulder),
        rewind: Some(GamepadButtonBinding::LeftStickClick),
        fast_forward: Some(GamepadButtonBinding::RightStickClick),
    };
    assert_eq!(
        presentation.item_label(MenuItem::GamepadSaveState),
        "SAVE STATE L1"
    );
    assert_eq!(
        presentation.item_label(MenuItem::GamepadLoadState),
        "LOAD STATE R1"
    );
    assert_eq!(
        presentation.item_label(MenuItem::GamepadRewind),
        "REWIND LSTICK"
    );
    assert_eq!(
        presentation.item_label(MenuItem::GamepadFastForward),
        "F-FORWARD RSTICK"
    );

    presentation.gamepad_menu_bindings = GamepadMenuBindings {
        up: GamepadButtonBinding::LeftShoulder,
        down: GamepadButtonBinding::RightShoulder,
        confirm: GamepadButtonBinding::LeftStickClick,
        cancel: GamepadButtonBinding::RightStickClick,
    };
    assert_eq!(presentation.item_label(MenuItem::GamepadMenuUp), "UP L1");
    assert_eq!(
        presentation.item_label(MenuItem::GamepadMenuDown),
        "DOWN R1"
    );
    assert_eq!(
        presentation.item_label(MenuItem::GamepadMenuConfirm),
        "OK LSTICK"
    );
    assert_eq!(
        presentation.item_label(MenuItem::GamepadMenuCancel),
        "BACK RSTICK"
    );

    presentation.keyboard_bindings = JoypadKeyboardBindings {
        up: DesktopKey::ArrowUp,
        down: DesktopKey::ArrowDown,
        left: DesktopKey::ArrowLeft,
        right: DesktopKey::ArrowRight,
        a: DesktopKey::Z,
        b: DesktopKey::X,
        select: DesktopKey::Backspace,
        start: DesktopKey::Space,
    };
    assert_eq!(presentation.item_label(MenuItem::KeyboardUp), "UP UP");
    assert_eq!(presentation.item_label(MenuItem::KeyboardDown), "DOWN DOWN");
    assert_eq!(presentation.item_label(MenuItem::KeyboardLeft), "LEFT LEFT");
    assert_eq!(
        presentation.item_label(MenuItem::KeyboardRight),
        "RIGHT RIGHT"
    );
    assert_eq!(presentation.item_label(MenuItem::KeyboardA), "A Z");
    assert_eq!(presentation.item_label(MenuItem::KeyboardB), "B X");
    assert_eq!(
        presentation.item_label(MenuItem::KeyboardSelect),
        "SELECT BACK"
    );
    assert_eq!(
        presentation.item_label(MenuItem::KeyboardStart),
        "START SPACE"
    );

    presentation.keyboard_menu_bindings = MenuKeyboardBindings {
        up: DesktopKey::ArrowUp,
        down: DesktopKey::ArrowDown,
        confirm: DesktopKey::Return,
        cancel: DesktopKey::Escape,
    };
    assert_eq!(presentation.item_label(MenuItem::KeyboardMenuUp), "UP UP");
    assert_eq!(
        presentation.item_label(MenuItem::KeyboardMenuDown),
        "DOWN DOWN"
    );
    assert_eq!(
        presentation.item_label(MenuItem::KeyboardMenuConfirm),
        "OK ENTER"
    );
    assert_eq!(
        presentation.item_label(MenuItem::KeyboardMenuCancel),
        "BACK ESC"
    );

    presentation.hotkey_bindings = HotkeyBindings {
        pause: DesktopKey::R,
        save_state: DesktopKey::F1,
        load_state: DesktopKey::F2,
        state_slot_1: DesktopKey::Digit1,
        state_slot_2: DesktopKey::Digit2,
        state_slot_3: DesktopKey::Digit3,
        state_slot_4: DesktopKey::Digit4,
        reset: DesktopKey::Space,
        rewind: DesktopKey::LeftShift,
        fast_forward: DesktopKey::RightShift,
        toggle_fullscreen: DesktopKey::F11,
        toggle_performance_hud: DesktopKey::F10,
        save_battery: DesktopKey::F9,
    };
    assert_eq!(presentation.item_label(MenuItem::HotkeyPause), "PAUSE R");
    assert_eq!(
        presentation.item_label(MenuItem::HotkeySaveState),
        "SAVE STATE F1"
    );
    assert_eq!(
        presentation.item_label(MenuItem::HotkeyLoadState),
        "LOAD STATE F2"
    );
    assert_eq!(
        presentation.item_label(MenuItem::HotkeyStateSlot1),
        "STATE SLOT 1 1"
    );
    assert_eq!(
        presentation.item_label(MenuItem::HotkeyStateSlot4),
        "STATE SLOT 4 4"
    );
    assert_eq!(
        presentation.item_label(MenuItem::HotkeyReset),
        "RESET SPACE"
    );
    assert_eq!(
        presentation.item_label(MenuItem::HotkeyRewind),
        "REWIND L SHIFT"
    );
    assert_eq!(
        presentation.item_label(MenuItem::HotkeyFastForward),
        "F-FORWARD R SHIFT"
    );
    assert_eq!(
        presentation.item_label(MenuItem::HotkeyFullscreen),
        "FULLSCREEN F11"
    );
    assert_eq!(
        presentation.item_label(MenuItem::HotkeyPerformanceHud),
        "STATS F10"
    );
    assert_eq!(
        presentation.item_label(MenuItem::HotkeySaveBattery),
        "SAVE BATTERY F9"
    );
    assert_eq!(presentation.item_label(MenuItem::Quit), "QUIT");
}
