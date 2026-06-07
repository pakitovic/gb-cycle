use super::{
    AUDIO_MENU_ITEMS, BOOT_ROM_MENU_ITEMS, CGB_INFRARED_MENU_ITEMS, CONFIG_MENU_ITEMS,
    CgbInfraredHudSnapshot, CgbInfraredParticipantHudSnapshot, CompactMenuLabel,
    CompactRecentRomLabel, EXT_PORT_MENU_ITEMS, FAST_FORWARD_MENU_ITEMS, GAME_LINK_MENU_ITEMS,
    GAMEPAD_MENU_CONTROL_ITEMS, GAMEPAD_MENU_ITEMS, GamepadActionBindingTarget,
    GamepadBindingTarget, GamepadMenuBindingTarget, HOTKEYS_MENU_ITEMS, INPUT_MENU_ITEMS,
    KEYBOARD_MENU_CONTROL_ITEMS, KEYBOARD_MENU_ITEMS, KeyboardBindingTarget,
    KeyboardMenuBindingTarget, MENU_ITEM_AREA_TOP_OFFSET, MENU_ITEM_TEXT_OFFSET_X,
    MENU_VISIBLE_ITEM_CAPACITY, MenuAction, MenuInput, MenuItem, MenuPanelLayout, MenuPresentation,
    MenuScreen, OverlayMenuState, PerformanceHudSnapshot, RECENT_MENU_ITEMS,
    RECENT_ROM_MENU_CAPACITY, REWIND_MENU_ITEMS, ROOT_MENU_ITEMS, RewindHudSnapshot,
    SAVE_MENU_ITEMS, SYSTEM_MENU_ITEMS, ScrollIndicatorDirection, VIDEO_MENU_ITEMS,
    cgb_ir_indicator_lines, desktop_key_label, gamepad_binding_label, glyph_rows,
    normalized_selected_index, performance_hud_lines, pokemon_pikachu_color_gift_menu_label,
    previous_enabled_index, render_cgb_ir_indicator, render_fast_forward_indicator,
    render_performance_hud, render_rewind_indicator, rendered_item_label,
    rendered_recent_rom_item_label, scroll_indicator_rows, viewport_start_index, visible_item_at,
    visible_item_count,
};
use crate::player_slots::DesktopDmg07PlayerCount;
use gb_core::{
    ExecutionMode, HardwareRevision, PokemonMysteryGiftCode, PokemonMysteryGiftKind,
    PokemonPikachuColorGift, SgbVideoStandard, StartupMode,
};
use gb_desktop::{
    BootRomVerificationMode, DesktopConsoleModel, DesktopDisplayPalette,
    DesktopExternalPortSelection, DesktopFrameBlendingMode, DesktopKey, DesktopSaveFlushPolicy,
    FastForwardOptions, GamepadActionBindings, GamepadButtonBinding, GamepadButtonBindings,
    GamepadDirectionalSource, GamepadGyroMode, GamepadMenuBindings, GamepadRumbleMode,
    HotkeyBindings, JoypadKeyboardBindings, MenuKeyboardBindings, RewindOptions,
    SgbBorderPresentationMode,
};
use std::time::Duration;

fn test_presentation() -> MenuPresentation {
    MenuPresentation {
        rom_loaded: true,
        recent_rom_count: 0,
        recent_rom_labels: [CompactRecentRomLabel::default(); RECENT_ROM_MENU_CAPACITY],
        console_model: DesktopConsoleModel::GameBoy,
        revision: HardwareRevision::DmgCpuC,
        sgb_video_standard: SgbVideoStandard::Ntsc,
        startup_mode: StartupMode::SkipBoot,
        execution_mode: ExecutionMode::Strict,
        external_port_selection: DesktopExternalPortSelection::None,
        cgb_infrared_link_active: false,
        cgb_infrared_same_game_active: false,
        pokemon_pikachu_color_active: false,
        pokemon_pikachu_color_gift: PokemonPikachuColorGift::default(),
        pokemon_mystery_gift_active: false,
        pokemon_mystery_gift_kind: PokemonMysteryGiftKind::default(),
        pokemon_mystery_gift_code: PokemonMysteryGiftCode::default(),
        show_cgb_infrared_helper: false,
        boot_rom_verification: BootRomVerificationMode::Strict,
        saves_enabled: true,
        save_flush_policy: DesktopSaveFlushPolicy::Debounced,
        save_directory_uses_default_path: true,
        fullscreen: false,
        vsync: true,
        window_scale: 4,
        integer_scale: true,
        presentation_filter: false,
        frame_blending: DesktopFrameBlendingMode::Off,
        display_palette: DesktopDisplayPalette::GameBoy,
        show_background: true,
        show_window: true,
        show_objects: true,
        sgb_border: SgbBorderPresentationMode::Auto,
        show_performance_hud: true,
        muted: false,
        audio_available: false,
        audio_volume_percent: 100,
        audio_recording_enabled: false,
        ch1_enabled: true,
        ch2_enabled: true,
        ch3_enabled: true,
        ch4_enabled: true,
        manual_save_available: false,
        external_save_available: false,
        external_save_import_available: false,
        machine_state_available: true,
        machine_state_load_available: true,
        machine_state_slot: 1,
        machine_state_autoload_slot: None,
        rewind_supported: true,
        rewind_options: RewindOptions::default(),
        fast_forward_options: FastForwardOptions::default(),
        rewind_available: true,
        any_dialog_pending: false,
        cartridge_pocket_camera_supported: false,
        pocket_camera_live_enabled: false,
        gamepad_available: false,
        gamepad_directional_source: GamepadDirectionalSource::DpadAndLeftStick,
        gamepad_gyro_mode: GamepadGyroMode::Off,
        gamepad_rumble_mode: GamepadRumbleMode::Strong,
        gamepad_bindings: GamepadButtonBindings::default(),
        gamepad_action_bindings: GamepadActionBindings::default(),
        gamepad_menu_bindings: GamepadMenuBindings::default(),
        active_gamepad_connected: false,
        cartridge_mbc7_accelerometer_supported: false,
        cartridge_rumble_supported: false,
        active_gamepad_accelerometer_supported: false,
        active_gamepad_rumble_supported: false,
        active_gamepad_label: CompactMenuLabel::default(),
        preferred_gamepad_configured: false,
        preferred_gamepad_label: CompactMenuLabel::default(),
        keyboard_bindings: JoypadKeyboardBindings::default(),
        keyboard_menu_bindings: MenuKeyboardBindings::default(),
        hotkey_bindings: HotkeyBindings::default(),
    }
}

fn select_visible_item(
    menu: &mut OverlayMenuState,
    presentation: MenuPresentation,
    target: MenuItem,
) {
    while visible_item_at(
        menu.current_screen_state().screen,
        menu.current_screen_state().selected_index,
        presentation,
    ) != Some(target)
    {
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
    }
}

fn open_video_menu(menu: &mut OverlayMenuState, presentation: MenuPresentation) {
    menu.open(presentation);
    select_visible_item(menu, presentation, MenuItem::ConfigMenu);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    select_visible_item(menu, presentation, MenuItem::VideoMenu);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
}

fn open_audio_menu(menu: &mut OverlayMenuState, presentation: MenuPresentation) {
    menu.open(presentation);
    select_visible_item(menu, presentation, MenuItem::ConfigMenu);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    select_visible_item(menu, presentation, MenuItem::AudioMenu);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
}

fn open_input_menu(menu: &mut OverlayMenuState, presentation: MenuPresentation) {
    menu.open(presentation);
    select_visible_item(menu, presentation, MenuItem::ConfigMenu);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    select_visible_item(menu, presentation, MenuItem::InputMenu);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
}

fn open_recent_menu(menu: &mut OverlayMenuState, presentation: MenuPresentation) {
    menu.open(presentation);
    select_visible_item(menu, presentation, MenuItem::RecentMenu);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
}

fn open_system_menu(menu: &mut OverlayMenuState, presentation: MenuPresentation) {
    menu.open(presentation);
    select_visible_item(menu, presentation, MenuItem::ConfigMenu);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    select_visible_item(menu, presentation, MenuItem::SystemMenu);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
}

fn open_boot_rom_menu(menu: &mut OverlayMenuState, presentation: MenuPresentation) {
    open_system_menu(menu, presentation);
    select_visible_item(menu, presentation, MenuItem::BootRomMenu);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
}

fn open_save_menu(menu: &mut OverlayMenuState, presentation: MenuPresentation) {
    open_system_menu(menu, presentation);
    select_visible_item(menu, presentation, MenuItem::SaveMenu);
    assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
}

#[path = "test/actions.rs"]
mod actions;
#[path = "test/audio_video.rs"]
mod audio_video;
#[path = "test/binding_capture.rs"]
mod binding_capture;
#[path = "test/dialogs_recent.rs"]
mod dialogs_recent;
#[path = "test/hud_helpers.rs"]
mod hud_helpers;
#[path = "test/input_system.rs"]
mod input_system;
#[path = "test/labels.rs"]
mod labels;
#[path = "test/layout_overlay.rs"]
mod layout_overlay;
#[path = "test/navigation.rs"]
mod navigation;
#[path = "test/renderers.rs"]
mod renderers;
