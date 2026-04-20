use gb_core::{ExecutionMode, StartupMode};
use gb_desktop::{
    BootRomVerificationMode, DesktopConsoleModel, DesktopExternalPortSelection, DesktopKey,
    DesktopSaveFlushPolicy, GamepadButtonBinding, GamepadButtonBindings, GamepadDirectionalSource,
    GamepadMenuBindings, GamepadRumbleMode, HotkeyBindings, JoypadKeyboardBindings,
    MenuKeyboardBindings,
};
use std::time::{Duration, Instant};

const GLYPH_WIDTH: usize = 5;
const GLYPH_HEIGHT: usize = 7;
const GLYPH_SPACING: usize = 1;
const MENU_PANEL_X: usize = 20;
const MENU_PANEL_Y: usize = 16;
const MENU_PANEL_WIDTH: usize = 120;
const MENU_PANEL_HEIGHT: usize = 112;
const MENU_ITEM_HEIGHT: usize = 14;
const MENU_ITEM_AREA_TOP_OFFSET: usize = 30;
const MENU_ITEM_AREA_BOTTOM_PADDING: usize = 12;
const MENU_ITEM_CURSOR_X: usize = MENU_PANEL_X + 10;
const MENU_ITEM_TEXT_X: usize = MENU_PANEL_X + 18;
const MENU_ITEM_TEXT_Y: usize = MENU_PANEL_Y + MENU_ITEM_AREA_TOP_OFFSET;
const MENU_ITEM_TEXT_AREA_WIDTH: usize = MENU_PANEL_WIDTH - (MENU_ITEM_TEXT_X - MENU_PANEL_X) - 8;
const MENU_ITEM_TEXT_CAPACITY: usize =
    (MENU_ITEM_TEXT_AREA_WIDTH + GLYPH_SPACING) / (GLYPH_WIDTH + GLYPH_SPACING);
const MENU_VISIBLE_ITEM_CAPACITY: usize =
    (MENU_PANEL_HEIGHT - MENU_ITEM_AREA_TOP_OFFSET - MENU_ITEM_AREA_BOTTOM_PADDING)
        / MENU_ITEM_HEIGHT;
const MENU_SCROLL_INDICATOR_X: usize = MENU_PANEL_X + MENU_PANEL_WIDTH - 11;
const MENU_SCROLL_INDICATOR_TOP_Y: usize = MENU_ITEM_TEXT_Y - 9;
const MENU_SCROLL_INDICATOR_BOTTOM_Y: usize =
    MENU_PANEL_Y + MENU_PANEL_HEIGHT - MENU_ITEM_AREA_BOTTOM_PADDING + 1;
const HUD_PANEL_X: usize = 4;
const HUD_PANEL_Y: usize = 4;
const HUD_PANEL_WIDTH: usize = 82;
const HUD_PANEL_HEIGHT: usize = 44;
const HUD_TEXT_X: usize = HUD_PANEL_X + 5;
const HUD_TEXT_Y: usize = HUD_PANEL_Y + 5;
const HUD_LINE_HEIGHT: usize = GLYPH_HEIGHT + 2;

const OVERLAY_DIM_FACTOR_NUMERATOR: u16 = 1;
const OVERLAY_DIM_FACTOR_DENOMINATOR: u16 = 3;

const PANEL_COLOR: [u8; 3] = [42, 56, 46];
const PANEL_BORDER_COLOR: [u8; 3] = [132, 156, 111];
const PANEL_INNER_BORDER_COLOR: [u8; 3] = [20, 28, 22];
const TITLE_COLOR: [u8; 3] = [230, 240, 214];
const TEXT_COLOR: [u8; 3] = [214, 224, 199];
const HUD_PANEL_COLOR: [u8; 3] = [30, 39, 32];
const SELECTED_TEXT_COLOR: [u8; 3] = [22, 31, 24];
const DISABLED_TEXT_COLOR: [u8; 3] = [118, 128, 112];
const SELECTION_COLOR: [u8; 3] = [181, 199, 122];
const CURSOR_COLOR: [u8; 3] = [26, 35, 27];
const COMPACT_MENU_LABEL_MAX_BYTES: usize = 9;
const RECENT_ROM_LABEL_MAX_BYTES: usize = 64;
const RECENT_ROM_SCROLL_DELAY: Duration = Duration::from_millis(900);
const RECENT_ROM_SCROLL_STEP: Duration = Duration::from_millis(150);
const RECENT_ROM_SCROLL_GAP_CHARS: usize = 3;
pub const RECENT_ROM_MENU_CAPACITY: usize = 8;

const ROOT_MENU_ITEMS: [MenuItem; 10] = [
    MenuItem::Resume,
    MenuItem::OpenRom,
    MenuItem::RecentMenu,
    MenuItem::SaveBattery,
    MenuItem::VideoMenu,
    MenuItem::AudioMenu,
    MenuItem::InputMenu,
    MenuItem::ExtPortMenu,
    MenuItem::SystemMenu,
    MenuItem::Quit,
];
const RECENT_MENU_ITEMS: [MenuItem; RECENT_ROM_MENU_CAPACITY + 2] = [
    MenuItem::RecentRom1,
    MenuItem::RecentRom2,
    MenuItem::RecentRom3,
    MenuItem::RecentRom4,
    MenuItem::RecentRom5,
    MenuItem::RecentRom6,
    MenuItem::RecentRom7,
    MenuItem::RecentRom8,
    MenuItem::ClearRecentList,
    MenuItem::Return,
];
const VIDEO_MENU_ITEMS: [MenuItem; 12] = [
    MenuItem::Fullscreen,
    MenuItem::Vsync,
    MenuItem::WindowScale,
    MenuItem::IntegerScale,
    MenuItem::PresentationFilter,
    MenuItem::ShowBackground,
    MenuItem::ShowWindow,
    MenuItem::ShowObjects,
    MenuItem::Screenshot,
    MenuItem::PerformanceHud,
    MenuItem::VideoDefaults,
    MenuItem::Return,
];
const AUDIO_MENU_ITEMS: [MenuItem; 4] = [
    MenuItem::ToggleMute,
    MenuItem::AudioVolume,
    MenuItem::AudioDefaults,
    MenuItem::Return,
];
const INPUT_MENU_ITEMS: [MenuItem; 9] = [
    MenuItem::KeyboardMenu,
    MenuItem::KeyboardMenuControls,
    MenuItem::HotkeysMenu,
    MenuItem::GamepadMenu,
    MenuItem::GamepadMenuControls,
    MenuItem::GamepadDirection,
    MenuItem::GamepadRumble,
    MenuItem::InputDefaults,
    MenuItem::Return,
];
const EXT_PORT_MENU_ITEMS: [MenuItem; 5] = [
    MenuItem::ExternalPortNone,
    MenuItem::ExternalPortPrinter,
    MenuItem::ExternalPortGameLink,
    MenuItem::ExternalPortFourPlayerAdapter,
    MenuItem::Return,
];
const KEYBOARD_MENU_ITEMS: [MenuItem; 9] = [
    MenuItem::KeyboardUp,
    MenuItem::KeyboardDown,
    MenuItem::KeyboardLeft,
    MenuItem::KeyboardRight,
    MenuItem::KeyboardA,
    MenuItem::KeyboardB,
    MenuItem::KeyboardSelect,
    MenuItem::KeyboardStart,
    MenuItem::Return,
];
const KEYBOARD_MENU_CONTROL_ITEMS: [MenuItem; 5] = [
    MenuItem::KeyboardMenuUp,
    MenuItem::KeyboardMenuDown,
    MenuItem::KeyboardMenuConfirm,
    MenuItem::KeyboardMenuCancel,
    MenuItem::Return,
];
const HOTKEYS_MENU_ITEMS: [MenuItem; 6] = [
    MenuItem::HotkeyPause,
    MenuItem::HotkeyReset,
    MenuItem::HotkeyFullscreen,
    MenuItem::HotkeyPerformanceHud,
    MenuItem::HotkeySaveBattery,
    MenuItem::Return,
];
const GAMEPAD_MENU_ITEMS: [MenuItem; 11] = [
    MenuItem::GamepadActive,
    MenuItem::GamepadPreferred,
    MenuItem::GamepadUp,
    MenuItem::GamepadDown,
    MenuItem::GamepadLeft,
    MenuItem::GamepadRight,
    MenuItem::GamepadA,
    MenuItem::GamepadB,
    MenuItem::GamepadSelect,
    MenuItem::GamepadStart,
    MenuItem::Return,
];
const GAMEPAD_MENU_CONTROL_ITEMS: [MenuItem; 5] = [
    MenuItem::GamepadMenuUp,
    MenuItem::GamepadMenuDown,
    MenuItem::GamepadMenuConfirm,
    MenuItem::GamepadMenuCancel,
    MenuItem::Return,
];
const SYSTEM_MENU_ITEMS: [MenuItem; 13] = [
    MenuItem::ConsoleModel,
    MenuItem::StartupMode,
    MenuItem::ExecutionMode,
    MenuItem::BootRomDefaultPath,
    MenuItem::BootRomFilePath,
    MenuItem::BootRomDirectoryPath,
    MenuItem::BootRomVerify,
    MenuItem::SavesEnabled,
    MenuItem::SavePolicy,
    MenuItem::SaveDefaultPath,
    MenuItem::SaveDirectoryPath,
    MenuItem::Reset,
    MenuItem::Return,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuInput {
    Up,
    Down,
    Confirm,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Close,
    OpenRom,
    OpenRecentRom(usize),
    ClearRecentList,
    SaveBattery,
    SaveScreenshot,
    CycleConsoleModel,
    CycleStartupMode,
    CycleExecutionMode,
    ClearBootRomPath,
    SelectBootRomFilePath,
    SelectBootRomDirectoryPath,
    CycleBootRomVerify,
    ToggleSavesEnabled,
    CycleSavePolicy,
    ClearSaveDirectoryPath,
    SelectSaveDirectoryPath,
    ToggleFullscreen,
    ToggleVsync,
    CycleWindowScale,
    ToggleIntegerScale,
    TogglePresentationFilter,
    ToggleBackgroundLayer,
    ToggleWindowLayer,
    ToggleObjectLayer,
    TogglePerformanceHud,
    ToggleMute,
    CycleAudioVolume,
    CycleGamepadDirectionalSource,
    CycleGamepadRumbleMode,
    TogglePreferredGamepad,
    ResetVideoDefaults,
    ResetAudioDefaults,
    SetExternalPort(DesktopExternalPortSelection),
    ResetInputDefaults,
    SetKeyboardBinding(KeyboardBindingTarget, DesktopKey),
    SetKeyboardMenuBinding(KeyboardMenuBindingTarget, DesktopKey),
    SetGamepadBinding(GamepadBindingTarget, GamepadButtonBinding),
    SetGamepadMenuBinding(GamepadMenuBindingTarget, GamepadButtonBinding),
    Reset,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CompactMenuLabel {
    bytes: [u8; COMPACT_MENU_LABEL_MAX_BYTES],
    len: u8,
}

impl CompactMenuLabel {
    pub fn from_text(text: &str) -> Self {
        let mut label = Self::default();
        for byte in text.bytes() {
            if usize::from(label.len) == COMPACT_MENU_LABEL_MAX_BYTES {
                break;
            }
            if !matches!(byte, b' ' | b'0'..=b'9' | b'A'..=b'Z') {
                continue;
            }

            label.bytes[usize::from(label.len)] = byte;
            label.len += 1;
        }
        label
    }

    pub fn from_gamepad_name(name: &str) -> Self {
        let compact = compact_gamepad_name(name);
        Self::from_text(&compact)
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..usize::from(self.len)])
            .expect("compact menu labels only contain ASCII bytes")
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompactRecentRomLabel {
    bytes: [u8; RECENT_ROM_LABEL_MAX_BYTES],
    len: u8,
}

impl Default for CompactRecentRomLabel {
    fn default() -> Self {
        Self {
            bytes: [0; RECENT_ROM_LABEL_MAX_BYTES],
            len: 0,
        }
    }
}

impl CompactRecentRomLabel {
    pub fn from_text(text: &str) -> Self {
        let mut label = Self::default();
        for byte in text.bytes() {
            if usize::from(label.len) == RECENT_ROM_LABEL_MAX_BYTES {
                break;
            }
            if !matches!(byte, b' ' | b'0'..=b'9' | b'A'..=b'Z') {
                continue;
            }

            label.bytes[usize::from(label.len)] = byte;
            label.len += 1;
        }
        label
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..usize::from(self.len)])
            .expect("compact recent ROM labels only contain ASCII bytes")
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardBindingTarget {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    Select,
    Start,
    Pause,
    Reset,
    ToggleFullscreen,
    TogglePerformanceHud,
    SaveBattery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardMenuBindingTarget {
    Up,
    Down,
    Confirm,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadBindingTarget {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    Select,
    Start,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadMenuBindingTarget {
    Up,
    Down,
    Confirm,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingBindingCapture {
    Keyboard(KeyboardBindingTarget),
    KeyboardMenu(KeyboardMenuBindingTarget),
    Gamepad(GamepadBindingTarget),
    GamepadMenu(GamepadMenuBindingTarget),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerformanceHudSnapshot {
    pub fps: f64,
    pub speed_percent: f64,
    pub frame_time_ms: f64,
    pub emulation_time_ms: f64,
    pub render_time_ms: f64,
    pub pacing_time_ms: f64,
    pub audio_queue_ms: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuPresentation {
    pub rom_loaded: bool,
    pub recent_rom_count: u8,
    pub recent_rom_labels: [CompactRecentRomLabel; RECENT_ROM_MENU_CAPACITY],
    pub console_model: DesktopConsoleModel,
    pub startup_mode: StartupMode,
    pub execution_mode: ExecutionMode,
    pub external_port_selection: DesktopExternalPortSelection,
    pub boot_rom_uses_default_path: bool,
    pub boot_rom_verification: BootRomVerificationMode,
    pub saves_enabled: bool,
    pub save_flush_policy: DesktopSaveFlushPolicy,
    pub save_directory_uses_default_path: bool,
    pub fullscreen: bool,
    pub vsync: bool,
    pub window_scale: u8,
    pub integer_scale: bool,
    pub presentation_filter: bool,
    pub show_background: bool,
    pub show_window: bool,
    pub show_objects: bool,
    pub show_performance_hud: bool,
    pub muted: bool,
    pub audio_available: bool,
    pub audio_volume_percent: u8,
    pub manual_save_available: bool,
    pub any_dialog_pending: bool,
    pub gamepad_available: bool,
    pub gamepad_directional_source: GamepadDirectionalSource,
    pub gamepad_rumble_mode: GamepadRumbleMode,
    pub gamepad_bindings: GamepadButtonBindings,
    pub gamepad_menu_bindings: GamepadMenuBindings,
    pub active_gamepad_connected: bool,
    pub cartridge_rumble_supported: bool,
    pub active_gamepad_rumble_supported: bool,
    pub active_gamepad_label: CompactMenuLabel,
    pub preferred_gamepad_configured: bool,
    pub preferred_gamepad_label: CompactMenuLabel,
    pub keyboard_bindings: JoypadKeyboardBindings,
    pub keyboard_menu_bindings: MenuKeyboardBindings,
    pub hotkey_bindings: HotkeyBindings,
}

impl MenuPresentation {
    fn item_visible(self, item: MenuItem) -> bool {
        match item {
            MenuItem::SaveBattery => self.manual_save_available,
            MenuItem::RecentMenu => self.recent_rom_count > 0,
            MenuItem::RecentRom1 => self.recent_rom_count >= 1,
            MenuItem::RecentRom2 => self.recent_rom_count >= 2,
            MenuItem::RecentRom3 => self.recent_rom_count >= 3,
            MenuItem::RecentRom4 => self.recent_rom_count >= 4,
            MenuItem::RecentRom5 => self.recent_rom_count >= 5,
            MenuItem::RecentRom6 => self.recent_rom_count >= 6,
            MenuItem::RecentRom7 => self.recent_rom_count >= 7,
            MenuItem::RecentRom8 => self.recent_rom_count >= 8,
            MenuItem::ClearRecentList => self.recent_rom_count > 0,
            _ => true,
        }
    }

    fn item_enabled(self, item: MenuItem) -> bool {
        match item {
            MenuItem::Resume | MenuItem::Reset | MenuItem::Screenshot => self.rom_loaded,
            MenuItem::OpenRom
            | MenuItem::RecentMenu
            | MenuItem::RecentRom1
            | MenuItem::RecentRom2
            | MenuItem::RecentRom3
            | MenuItem::RecentRom4
            | MenuItem::RecentRom5
            | MenuItem::RecentRom6
            | MenuItem::RecentRom7
            | MenuItem::RecentRom8
            | MenuItem::BootRomFilePath
            | MenuItem::BootRomDirectoryPath
            | MenuItem::SaveDirectoryPath => !self.any_dialog_pending,
            MenuItem::SaveBattery => self.manual_save_available,
            MenuItem::AudioMenu | MenuItem::ToggleMute | MenuItem::AudioVolume => {
                self.audio_available
            }
            MenuItem::GamepadActive => false,
            MenuItem::GamepadPreferred => {
                self.gamepad_available
                    && (self.active_gamepad_connected || self.preferred_gamepad_configured)
            }
            MenuItem::GamepadRumble => {
                self.cartridge_rumble_supported && self.active_gamepad_rumble_supported
            }
            MenuItem::GamepadMenu
            | MenuItem::GamepadMenuControls
            | MenuItem::GamepadDirection
            | MenuItem::GamepadUp
            | MenuItem::GamepadDown
            | MenuItem::GamepadLeft
            | MenuItem::GamepadRight
            | MenuItem::GamepadA
            | MenuItem::GamepadB
            | MenuItem::GamepadSelect
            | MenuItem::GamepadStart
            | MenuItem::GamepadMenuUp
            | MenuItem::GamepadMenuDown
            | MenuItem::GamepadMenuConfirm
            | MenuItem::GamepadMenuCancel => self.gamepad_available,
            MenuItem::KeyboardMenu
            | MenuItem::KeyboardMenuControls
            | MenuItem::HotkeysMenu
            | MenuItem::KeyboardUp
            | MenuItem::KeyboardDown
            | MenuItem::KeyboardLeft
            | MenuItem::KeyboardRight
            | MenuItem::KeyboardA
            | MenuItem::KeyboardB
            | MenuItem::KeyboardSelect
            | MenuItem::KeyboardStart
            | MenuItem::KeyboardMenuUp
            | MenuItem::KeyboardMenuDown
            | MenuItem::KeyboardMenuConfirm
            | MenuItem::KeyboardMenuCancel
            | MenuItem::HotkeyPause
            | MenuItem::HotkeyReset
            | MenuItem::HotkeyFullscreen
            | MenuItem::HotkeyPerformanceHud
            | MenuItem::HotkeySaveBattery
            | MenuItem::InputMenu
            | MenuItem::ExtPortMenu
            | MenuItem::VideoMenu
            | MenuItem::SystemMenu
            | MenuItem::ConsoleModel
            | MenuItem::StartupMode
            | MenuItem::ExecutionMode
            | MenuItem::BootRomDefaultPath
            | MenuItem::BootRomVerify
            | MenuItem::SavesEnabled
            | MenuItem::SavePolicy
            | MenuItem::SaveDefaultPath
            | MenuItem::ExternalPortNone
            | MenuItem::ExternalPortPrinter
            | MenuItem::Fullscreen
            | MenuItem::Vsync
            | MenuItem::WindowScale
            | MenuItem::IntegerScale
            | MenuItem::PresentationFilter
            | MenuItem::ShowBackground
            | MenuItem::ShowWindow
            | MenuItem::ShowObjects
            | MenuItem::PerformanceHud
            | MenuItem::VideoDefaults
            | MenuItem::AudioDefaults
            | MenuItem::InputDefaults
            | MenuItem::ClearRecentList
            | MenuItem::Quit
            | MenuItem::Return => true,
            MenuItem::ExternalPortGameLink => self.rom_loaded && !self.any_dialog_pending,
            MenuItem::ExternalPortFourPlayerAdapter => false,
        }
    }

    fn item_label(self, item: MenuItem) -> String {
        match item {
            MenuItem::Resume => "RESUME".to_string(),
            MenuItem::OpenRom => "OPEN ROM".to_string(),
            MenuItem::RecentMenu => "OPEN RECENT".to_string(),
            MenuItem::RecentRom1 => recent_rom_item_label(self.recent_rom_labels[0]),
            MenuItem::RecentRom2 => recent_rom_item_label(self.recent_rom_labels[1]),
            MenuItem::RecentRom3 => recent_rom_item_label(self.recent_rom_labels[2]),
            MenuItem::RecentRom4 => recent_rom_item_label(self.recent_rom_labels[3]),
            MenuItem::RecentRom5 => recent_rom_item_label(self.recent_rom_labels[4]),
            MenuItem::RecentRom6 => recent_rom_item_label(self.recent_rom_labels[5]),
            MenuItem::RecentRom7 => recent_rom_item_label(self.recent_rom_labels[6]),
            MenuItem::RecentRom8 => recent_rom_item_label(self.recent_rom_labels[7]),
            MenuItem::ClearRecentList => "CLEAR LIST".to_string(),
            MenuItem::SaveBattery => "SAVE BATTERY".to_string(),
            MenuItem::VideoMenu => "VIDEO".to_string(),
            MenuItem::AudioMenu => "AUDIO".to_string(),
            MenuItem::InputMenu => "INPUT".to_string(),
            MenuItem::ExtPortMenu => match self.external_port_selection {
                DesktopExternalPortSelection::None => "EXT NONE".to_string(),
                DesktopExternalPortSelection::Printer => "EXT PRINTER".to_string(),
                DesktopExternalPortSelection::GameLink => "EXT LINK".to_string(),
                DesktopExternalPortSelection::FourPlayerAdapter => "EXT 4P".to_string(),
            },
            MenuItem::KeyboardMenu => "KEYBOARD".to_string(),
            MenuItem::KeyboardMenuControls => "KB MENU".to_string(),
            MenuItem::HotkeysMenu => "HOTKEYS".to_string(),
            MenuItem::GamepadMenu => "GAMEPAD".to_string(),
            MenuItem::GamepadMenuControls => "PAD MENU".to_string(),
            MenuItem::SystemMenu => "SYSTEM".to_string(),
            MenuItem::ConsoleModel => match self.console_model {
                DesktopConsoleModel::Dmg0 => "MODEL DMG0".to_string(),
                DesktopConsoleModel::Dmg => "MODEL DMG".to_string(),
                DesktopConsoleModel::Mgb => "MODEL MGB".to_string(),
            },
            MenuItem::StartupMode => match self.startup_mode {
                StartupMode::SkipBoot => "START SKIP".to_string(),
                StartupMode::RealBoot => "START REAL".to_string(),
            },
            MenuItem::ExecutionMode => match self.execution_mode {
                ExecutionMode::Strict => "MODE STRICT".to_string(),
                ExecutionMode::Permissive => "MODE PERM".to_string(),
                ExecutionMode::Experimental => "MODE EXP".to_string(),
            },
            MenuItem::BootRomDefaultPath => {
                if self.boot_rom_uses_default_path {
                    "BOOT AUTO ON".to_string()
                } else {
                    "BOOT AUTO OFF".to_string()
                }
            }
            MenuItem::BootRomFilePath => "BOOT FILE".to_string(),
            MenuItem::BootRomDirectoryPath => "BOOT DIR".to_string(),
            MenuItem::BootRomVerify => match self.boot_rom_verification {
                BootRomVerificationMode::Off => "VERIFY OFF".to_string(),
                BootRomVerificationMode::Warn => "VERIFY WARN".to_string(),
                BootRomVerificationMode::Strict => "VERIFY STRICT".to_string(),
            },
            MenuItem::SavesEnabled => {
                if self.saves_enabled {
                    "SAVES ON".to_string()
                } else {
                    "SAVES OFF".to_string()
                }
            }
            MenuItem::SavePolicy => match self.save_flush_policy {
                DesktopSaveFlushPolicy::Manual => "SAVE MANUAL".to_string(),
                DesktopSaveFlushPolicy::OnClose => "SAVE CLOSE".to_string(),
                DesktopSaveFlushPolicy::OnWrite => "SAVE WRITE".to_string(),
                DesktopSaveFlushPolicy::Debounced => "SAVE DEBNC".to_string(),
            },
            MenuItem::SaveDefaultPath => {
                if self.save_directory_uses_default_path {
                    "DIR AUTO ON".to_string()
                } else {
                    "DIR AUTO OFF".to_string()
                }
            }
            MenuItem::SaveDirectoryPath => "SAVE DIR".to_string(),
            MenuItem::Fullscreen => {
                if self.fullscreen {
                    "FULLSCREEN ON".to_string()
                } else {
                    "FULLSCREEN OFF".to_string()
                }
            }
            MenuItem::Vsync => {
                if self.vsync {
                    "VSYNC ON".to_string()
                } else {
                    "VSYNC OFF".to_string()
                }
            }
            MenuItem::WindowScale => format!("SCALE {}X", self.window_scale.max(1)),
            MenuItem::IntegerScale => {
                if self.integer_scale {
                    "INTEGER ON".to_string()
                } else {
                    "INTEGER OFF".to_string()
                }
            }
            MenuItem::PresentationFilter => {
                if self.presentation_filter {
                    "FILTER ON".to_string()
                } else {
                    "FILTER OFF".to_string()
                }
            }
            MenuItem::ShowBackground => {
                if self.show_background {
                    "BACKGROUND ON".to_string()
                } else {
                    "BACKGROUND OFF".to_string()
                }
            }
            MenuItem::ShowWindow => {
                if self.show_window {
                    "WINDOW ON".to_string()
                } else {
                    "WINDOW OFF".to_string()
                }
            }
            MenuItem::ShowObjects => {
                if self.show_objects {
                    "OBJECTS ON".to_string()
                } else {
                    "OBJECTS OFF".to_string()
                }
            }
            MenuItem::Screenshot => "SCREENSHOT".to_string(),
            MenuItem::PerformanceHud => {
                if self.show_performance_hud {
                    "STATS ON".to_string()
                } else {
                    "STATS OFF".to_string()
                }
            }
            MenuItem::VideoDefaults => "DEFAULTS".to_string(),
            MenuItem::ToggleMute => {
                if self.muted {
                    "MUTE ON".to_string()
                } else {
                    "MUTE OFF".to_string()
                }
            }
            MenuItem::AudioVolume => format!("VOL {}%", self.audio_volume_percent.min(100)),
            MenuItem::AudioDefaults => "DEFAULTS".to_string(),
            MenuItem::GamepadDirection => match self.gamepad_directional_source {
                GamepadDirectionalSource::DpadOnly => "DIR DPAD".to_string(),
                GamepadDirectionalSource::LeftStickOnly => "DIR LEFT".to_string(),
                GamepadDirectionalSource::DpadAndLeftStick => "DIR ALL".to_string(),
            },
            MenuItem::GamepadRumble => {
                if !(self.cartridge_rumble_supported && self.active_gamepad_rumble_supported) {
                    "RUMBLE N/A".to_string()
                } else {
                    match self.gamepad_rumble_mode {
                        GamepadRumbleMode::Off => "RUMBLE OFF".to_string(),
                        GamepadRumbleMode::Strong => "RUMBLE HIGH".to_string(),
                        GamepadRumbleMode::Weak => "RUMBLE LOW".to_string(),
                    }
                }
            }
            MenuItem::InputDefaults => "DEFAULTS".to_string(),
            MenuItem::ExternalPortNone => {
                if self.external_port_selection == DesktopExternalPortSelection::None {
                    "NONE ON".to_string()
                } else {
                    "NONE".to_string()
                }
            }
            MenuItem::ExternalPortPrinter => {
                if self.external_port_selection == DesktopExternalPortSelection::Printer {
                    "PRINTER ON".to_string()
                } else {
                    "PRINTER".to_string()
                }
            }
            MenuItem::ExternalPortGameLink => "GAME LINK".to_string(),
            MenuItem::ExternalPortFourPlayerAdapter => "4P ADAPTER".to_string(),
            MenuItem::GamepadActive => {
                if self.active_gamepad_connected {
                    format!("ACTIVE {}", self.active_gamepad_label.as_str())
                } else {
                    "ACTIVE NONE".to_string()
                }
            }
            MenuItem::GamepadPreferred => {
                if self.preferred_gamepad_configured {
                    if self.preferred_gamepad_label.is_empty() {
                        "PREF SAVED".to_string()
                    } else {
                        format!("PREF {}", self.preferred_gamepad_label.as_str())
                    }
                } else {
                    "PREF AUTO".to_string()
                }
            }
            MenuItem::GamepadUp => {
                format!("UP {}", gamepad_binding_label(self.gamepad_bindings.up))
            }
            MenuItem::GamepadDown => {
                format!("DOWN {}", gamepad_binding_label(self.gamepad_bindings.down))
            }
            MenuItem::GamepadLeft => {
                format!("LEFT {}", gamepad_binding_label(self.gamepad_bindings.left))
            }
            MenuItem::GamepadRight => {
                format!(
                    "RIGHT {}",
                    gamepad_binding_label(self.gamepad_bindings.right)
                )
            }
            MenuItem::GamepadA => format!("A {}", gamepad_binding_label(self.gamepad_bindings.a)),
            MenuItem::GamepadB => format!("B {}", gamepad_binding_label(self.gamepad_bindings.b)),
            MenuItem::GamepadSelect => {
                format!(
                    "SELECT {}",
                    gamepad_binding_label(self.gamepad_bindings.select)
                )
            }
            MenuItem::GamepadStart => {
                format!(
                    "START {}",
                    gamepad_binding_label(self.gamepad_bindings.start)
                )
            }
            MenuItem::GamepadMenuUp => {
                format!(
                    "UP {}",
                    gamepad_binding_label(self.gamepad_menu_bindings.up)
                )
            }
            MenuItem::GamepadMenuDown => {
                format!(
                    "DOWN {}",
                    gamepad_binding_label(self.gamepad_menu_bindings.down)
                )
            }
            MenuItem::GamepadMenuConfirm => {
                format!(
                    "OK {}",
                    gamepad_binding_label(self.gamepad_menu_bindings.confirm)
                )
            }
            MenuItem::GamepadMenuCancel => {
                format!(
                    "BACK {}",
                    gamepad_binding_label(self.gamepad_menu_bindings.cancel)
                )
            }
            MenuItem::KeyboardUp => format!("UP {}", desktop_key_label(self.keyboard_bindings.up)),
            MenuItem::KeyboardDown => {
                format!("DOWN {}", desktop_key_label(self.keyboard_bindings.down))
            }
            MenuItem::KeyboardLeft => {
                format!("LEFT {}", desktop_key_label(self.keyboard_bindings.left))
            }
            MenuItem::KeyboardRight => {
                format!("RIGHT {}", desktop_key_label(self.keyboard_bindings.right))
            }
            MenuItem::KeyboardA => format!("A {}", desktop_key_label(self.keyboard_bindings.a)),
            MenuItem::KeyboardB => format!("B {}", desktop_key_label(self.keyboard_bindings.b)),
            MenuItem::KeyboardSelect => {
                format!(
                    "SELECT {}",
                    desktop_key_label(self.keyboard_bindings.select)
                )
            }
            MenuItem::KeyboardStart => {
                format!("START {}", desktop_key_label(self.keyboard_bindings.start))
            }
            MenuItem::KeyboardMenuUp => {
                format!("UP {}", desktop_key_label(self.keyboard_menu_bindings.up))
            }
            MenuItem::KeyboardMenuDown => {
                format!(
                    "DOWN {}",
                    desktop_key_label(self.keyboard_menu_bindings.down)
                )
            }
            MenuItem::KeyboardMenuConfirm => {
                format!(
                    "OK {}",
                    desktop_key_label(self.keyboard_menu_bindings.confirm)
                )
            }
            MenuItem::KeyboardMenuCancel => {
                format!(
                    "BACK {}",
                    desktop_key_label(self.keyboard_menu_bindings.cancel)
                )
            }
            MenuItem::HotkeyPause => {
                format!("PAUSE {}", desktop_key_label(self.hotkey_bindings.pause))
            }
            MenuItem::HotkeyReset => {
                format!("RESET {}", desktop_key_label(self.hotkey_bindings.reset))
            }
            MenuItem::HotkeyFullscreen => {
                format!(
                    "FULLSCREEN {}",
                    desktop_key_label(self.hotkey_bindings.toggle_fullscreen)
                )
            }
            MenuItem::HotkeyPerformanceHud => {
                format!(
                    "STATS {}",
                    desktop_key_label(self.hotkey_bindings.toggle_performance_hud)
                )
            }
            MenuItem::HotkeySaveBattery => {
                format!(
                    "SAVE BATTERY {}",
                    desktop_key_label(self.hotkey_bindings.save_battery)
                )
            }
            MenuItem::Reset => "RESET".to_string(),
            MenuItem::Quit => "QUIT".to_string(),
            MenuItem::Return => "RETURN".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuScreen {
    Root,
    Recent,
    Video,
    Audio,
    Input,
    ExtPort,
    Gamepad,
    GamepadMenuControls,
    Keyboard,
    KeyboardMenuControls,
    Hotkeys,
    System,
}

impl MenuScreen {
    fn title(self, presentation: MenuPresentation) -> &'static str {
        match self {
            Self::Root => {
                if presentation.rom_loaded {
                    "MENU"
                } else {
                    "NO ROM"
                }
            }
            Self::Recent => "RECENT",
            Self::Video => "VIDEO",
            Self::Audio => "AUDIO",
            Self::Input => "INPUT",
            Self::ExtPort => "EXT PORT",
            Self::Gamepad => "GAMEPAD",
            Self::GamepadMenuControls => "PAD MENU",
            Self::Keyboard => "KEYBOARD",
            Self::KeyboardMenuControls => "KB MENU",
            Self::Hotkeys => "HOTKEYS",
            Self::System => "SYSTEM",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuItem {
    Resume,
    OpenRom,
    RecentMenu,
    RecentRom1,
    RecentRom2,
    RecentRom3,
    RecentRom4,
    RecentRom5,
    RecentRom6,
    RecentRom7,
    RecentRom8,
    ClearRecentList,
    SaveBattery,
    VideoMenu,
    AudioMenu,
    InputMenu,
    ExtPortMenu,
    KeyboardMenu,
    KeyboardMenuControls,
    HotkeysMenu,
    GamepadMenu,
    GamepadMenuControls,
    SystemMenu,
    ConsoleModel,
    StartupMode,
    ExecutionMode,
    BootRomDefaultPath,
    BootRomFilePath,
    BootRomDirectoryPath,
    BootRomVerify,
    SavesEnabled,
    SavePolicy,
    SaveDefaultPath,
    SaveDirectoryPath,
    Fullscreen,
    Vsync,
    WindowScale,
    IntegerScale,
    PresentationFilter,
    ShowBackground,
    ShowWindow,
    ShowObjects,
    Screenshot,
    PerformanceHud,
    VideoDefaults,
    ToggleMute,
    AudioVolume,
    AudioDefaults,
    GamepadDirection,
    ExternalPortNone,
    ExternalPortPrinter,
    ExternalPortGameLink,
    ExternalPortFourPlayerAdapter,
    GamepadRumble,
    InputDefaults,
    GamepadActive,
    GamepadPreferred,
    GamepadUp,
    GamepadDown,
    GamepadLeft,
    GamepadRight,
    GamepadA,
    GamepadB,
    GamepadSelect,
    GamepadStart,
    KeyboardUp,
    KeyboardDown,
    KeyboardLeft,
    KeyboardRight,
    KeyboardA,
    KeyboardB,
    KeyboardSelect,
    KeyboardStart,
    KeyboardMenuUp,
    KeyboardMenuDown,
    KeyboardMenuConfirm,
    KeyboardMenuCancel,
    HotkeyPause,
    HotkeyReset,
    HotkeyFullscreen,
    HotkeyPerformanceHud,
    HotkeySaveBattery,
    GamepadMenuUp,
    GamepadMenuDown,
    GamepadMenuConfirm,
    GamepadMenuCancel,
    Reset,
    Quit,
    Return,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScreenState {
    screen: MenuScreen,
    selected_index: usize,
}

impl ScreenState {
    fn new(screen: MenuScreen, presentation: MenuPresentation) -> Self {
        Self {
            screen,
            selected_index: first_enabled_index(screen, presentation),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OverlayMenuState {
    open: bool,
    screen_stack: Vec<ScreenState>,
    pending_binding_capture: Option<PendingBindingCapture>,
    selection_started_at: Option<Instant>,
}

struct OverlayCanvas<'a> {
    rgb_frame: &'a mut [u8],
    frame_width: usize,
    frame_height: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScrollIndicatorDirection {
    Up,
    Down,
}

impl<'a> OverlayCanvas<'a> {
    fn new(rgb_frame: &'a mut [u8], frame_width: usize, frame_height: usize) -> Self {
        Self {
            rgb_frame,
            frame_width,
            frame_height,
        }
    }

    fn dim_frame(&mut self) {
        for component in self.rgb_frame.iter_mut() {
            *component = ((*component as u16 * OVERLAY_DIM_FACTOR_NUMERATOR)
                / OVERLAY_DIM_FACTOR_DENOMINATOR) as u8;
        }
    }

    fn fill_rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: [u8; 3]) {
        for row in y..y.saturating_add(height).min(self.frame_height) {
            for column in x..x.saturating_add(width).min(self.frame_width) {
                self.put_pixel(column, row, color);
            }
        }
    }

    fn draw_rect(&mut self, x: usize, y: usize, width: usize, height: usize, color: [u8; 3]) {
        if width == 0 || height == 0 {
            return;
        }

        self.fill_rect(x, y, width, 1, color);
        self.fill_rect(x, y.saturating_add(height - 1), width, 1, color);
        self.fill_rect(x, y, 1, height, color);
        self.fill_rect(x.saturating_add(width - 1), y, 1, height, color);
    }

    fn draw_text_centered(
        &mut self,
        area_x: usize,
        area_width: usize,
        y: usize,
        text: &str,
        color: [u8; 3],
        scale: usize,
    ) {
        let width = text_width(text, scale);
        let x = area_x + area_width.saturating_sub(width) / 2;
        self.draw_text(x, y, text, color, scale);
    }

    fn draw_text(&mut self, x: usize, y: usize, text: &str, color: [u8; 3], scale: usize) {
        let mut cursor_x = x;
        for character in text.chars() {
            self.draw_glyph(cursor_x, y, character, color, scale);
            cursor_x += (GLYPH_WIDTH + GLYPH_SPACING) * scale;
        }
    }

    fn draw_glyph(&mut self, x: usize, y: usize, character: char, color: [u8; 3], scale: usize) {
        let rows = glyph_rows(character);
        for (row_index, row) in rows.iter().copied().enumerate() {
            for column_index in 0..GLYPH_WIDTH {
                let mask = 1 << (GLYPH_WIDTH - 1 - column_index);
                if row & mask == 0 {
                    continue;
                }

                self.fill_rect(
                    x + column_index * scale,
                    y + row_index * scale,
                    scale,
                    scale,
                    color,
                );
            }
        }
    }

    fn put_pixel(&mut self, x: usize, y: usize, color: [u8; 3]) {
        if x >= self.frame_width || y >= self.frame_height {
            return;
        }

        let pixel_index = (y * self.frame_width + x) * 3;
        self.rgb_frame[pixel_index] = color[0];
        self.rgb_frame[pixel_index + 1] = color[1];
        self.rgb_frame[pixel_index + 2] = color[2];
    }

    fn draw_scroll_indicator(
        &mut self,
        x: usize,
        y: usize,
        direction: ScrollIndicatorDirection,
        color: [u8; 3],
    ) {
        for (row_offset, width) in scroll_indicator_rows(direction) {
            self.fill_rect(
                x + 2usize.saturating_sub(width / 2),
                y + row_offset,
                width,
                1,
                color,
            );
        }
    }
}

pub fn render_performance_hud(
    rgb_frame: &mut [u8],
    frame_width: usize,
    frame_height: usize,
    snapshot: PerformanceHudSnapshot,
) {
    let mut canvas = OverlayCanvas::new(rgb_frame, frame_width, frame_height);
    canvas.fill_rect(
        HUD_PANEL_X,
        HUD_PANEL_Y,
        HUD_PANEL_WIDTH,
        HUD_PANEL_HEIGHT,
        HUD_PANEL_COLOR,
    );
    canvas.draw_rect(
        HUD_PANEL_X,
        HUD_PANEL_Y,
        HUD_PANEL_WIDTH,
        HUD_PANEL_HEIGHT,
        PANEL_BORDER_COLOR,
    );
    canvas.draw_rect(
        HUD_PANEL_X + 1,
        HUD_PANEL_Y + 1,
        HUD_PANEL_WIDTH.saturating_sub(2),
        HUD_PANEL_HEIGHT.saturating_sub(2),
        PANEL_INNER_BORDER_COLOR,
    );

    for (line_index, line) in performance_hud_lines(snapshot).into_iter().enumerate() {
        canvas.draw_text(
            HUD_TEXT_X,
            HUD_TEXT_Y + line_index * HUD_LINE_HEIGHT,
            &line,
            TEXT_COLOR,
            1,
        );
    }
}

fn scroll_indicator_rows(direction: ScrollIndicatorDirection) -> [(usize, usize); 3] {
    match direction {
        ScrollIndicatorDirection::Up => [(0, 1), (1, 3), (2, 5)],
        ScrollIndicatorDirection::Down => [(0, 5), (1, 3), (2, 1)],
    }
}

impl OverlayMenuState {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_capturing_binding(&self) -> bool {
        self.pending_binding_capture.is_some()
    }

    pub fn pending_keyboard_binding_target(&self) -> Option<KeyboardBindingTarget> {
        match self.pending_binding_capture {
            Some(PendingBindingCapture::Keyboard(target)) => Some(target),
            Some(PendingBindingCapture::KeyboardMenu(_))
            | Some(PendingBindingCapture::Gamepad(_))
            | Some(PendingBindingCapture::GamepadMenu(_))
            | None => None,
        }
    }

    pub fn pending_keyboard_menu_binding_target(&self) -> Option<KeyboardMenuBindingTarget> {
        match self.pending_binding_capture {
            Some(PendingBindingCapture::KeyboardMenu(target)) => Some(target),
            Some(PendingBindingCapture::Keyboard(_))
            | Some(PendingBindingCapture::Gamepad(_))
            | Some(PendingBindingCapture::GamepadMenu(_))
            | None => None,
        }
    }

    pub fn pending_gamepad_binding_target(&self) -> Option<GamepadBindingTarget> {
        match self.pending_binding_capture {
            Some(PendingBindingCapture::Gamepad(target)) => Some(target),
            Some(PendingBindingCapture::Keyboard(_))
            | Some(PendingBindingCapture::KeyboardMenu(_))
            | Some(PendingBindingCapture::GamepadMenu(_))
            | None => None,
        }
    }

    pub fn pending_gamepad_menu_binding_target(&self) -> Option<GamepadMenuBindingTarget> {
        match self.pending_binding_capture {
            Some(PendingBindingCapture::GamepadMenu(target)) => Some(target),
            Some(PendingBindingCapture::Keyboard(_))
            | Some(PendingBindingCapture::KeyboardMenu(_))
            | Some(PendingBindingCapture::Gamepad(_))
            | None => None,
        }
    }

    pub fn open(&mut self, presentation: MenuPresentation) {
        self.open = true;
        self.screen_stack.clear();
        self.pending_binding_capture = None;
        self.screen_stack
            .push(ScreenState::new(MenuScreen::Root, presentation));
        self.selection_started_at = Some(Instant::now());
    }

    pub fn close(&mut self) {
        self.open = false;
        self.screen_stack.clear();
        self.pending_binding_capture = None;
        self.selection_started_at = None;
    }

    pub fn cancel_binding_capture(&mut self) {
        self.pending_binding_capture = None;
    }

    pub fn handle_keyboard_binding_capture(&mut self, key: DesktopKey) -> Option<MenuAction> {
        match self.pending_binding_capture.take()? {
            PendingBindingCapture::Keyboard(target) => {
                Some(MenuAction::SetKeyboardBinding(target, key))
            }
            PendingBindingCapture::KeyboardMenu(target) => {
                Some(MenuAction::SetKeyboardMenuBinding(target, key))
            }
            PendingBindingCapture::Gamepad(_) | PendingBindingCapture::GamepadMenu(_) => None,
        }
    }

    pub fn handle_gamepad_binding_capture(
        &mut self,
        binding: GamepadButtonBinding,
    ) -> Option<MenuAction> {
        match self.pending_binding_capture.take()? {
            PendingBindingCapture::Gamepad(target) => {
                Some(MenuAction::SetGamepadBinding(target, binding))
            }
            PendingBindingCapture::GamepadMenu(target) => {
                Some(MenuAction::SetGamepadMenuBinding(target, binding))
            }
            PendingBindingCapture::Keyboard(_) | PendingBindingCapture::KeyboardMenu(_) => None,
        }
    }

    pub fn handle_input(
        &mut self,
        input: MenuInput,
        presentation: MenuPresentation,
    ) -> Option<MenuAction> {
        if !self.open || self.pending_binding_capture.is_some() {
            return None;
        }
        self.normalize_current_selection(presentation);

        match input {
            MenuInput::Up => {
                let screen_state = self.current_screen_state();
                let selected_index = previous_enabled_index(
                    screen_state.screen,
                    screen_state.selected_index,
                    presentation,
                );
                self.set_selected_index(selected_index);
                None
            }
            MenuInput::Down => {
                let screen_state = self.current_screen_state();
                let selected_index = next_enabled_index(
                    screen_state.screen,
                    screen_state.selected_index,
                    presentation,
                );
                self.set_selected_index(selected_index);
                None
            }
            MenuInput::Confirm => {
                let screen_state = self.current_screen_state();
                let item = visible_item_at(
                    screen_state.screen,
                    screen_state.selected_index,
                    presentation,
                )
                .expect("normalized menu selection should point to a visible item");
                if !presentation.item_enabled(item) {
                    return None;
                }
                self.apply_item_action(item, presentation)
            }
            MenuInput::Cancel => self.pop_screen_or_close(presentation),
        }
    }

    pub fn render_overlay(
        &self,
        rgb_frame: &mut [u8],
        frame_width: usize,
        frame_height: usize,
        presentation: MenuPresentation,
    ) {
        if !self.open {
            return;
        }
        let screen_state = self.current_screen_state();
        let screen = screen_state.screen;
        let selected_index =
            normalized_selected_index(screen, screen_state.selected_index, presentation);
        let selection_elapsed = self.selection_elapsed();
        let item_count = visible_item_count(screen, presentation);

        let mut canvas = OverlayCanvas::new(rgb_frame, frame_width, frame_height);
        canvas.dim_frame();
        canvas.fill_rect(
            MENU_PANEL_X,
            MENU_PANEL_Y,
            MENU_PANEL_WIDTH,
            MENU_PANEL_HEIGHT,
            PANEL_COLOR,
        );
        canvas.draw_rect(
            MENU_PANEL_X,
            MENU_PANEL_Y,
            MENU_PANEL_WIDTH,
            MENU_PANEL_HEIGHT,
            PANEL_BORDER_COLOR,
        );
        canvas.draw_rect(
            MENU_PANEL_X + 2,
            MENU_PANEL_Y + 2,
            MENU_PANEL_WIDTH.saturating_sub(4),
            MENU_PANEL_HEIGHT.saturating_sub(4),
            PANEL_INNER_BORDER_COLOR,
        );

        let title = if self.pending_keyboard_binding_target().is_some()
            || self.pending_keyboard_menu_binding_target().is_some()
        {
            "PRESS KEY"
        } else if self.pending_gamepad_binding_target().is_some()
            || self.pending_gamepad_menu_binding_target().is_some()
        {
            "PRESS BTN"
        } else {
            screen.title(presentation)
        };
        canvas.draw_text_centered(
            MENU_PANEL_X,
            MENU_PANEL_WIDTH,
            MENU_PANEL_Y + 10,
            title,
            TITLE_COLOR,
            2,
        );

        let viewport_start = viewport_start_index(selected_index, item_count);
        let viewport_end = (viewport_start + MENU_VISIBLE_ITEM_CAPACITY).min(item_count);
        if viewport_start > 0 {
            canvas.draw_scroll_indicator(
                MENU_SCROLL_INDICATOR_X,
                MENU_SCROLL_INDICATOR_TOP_Y,
                ScrollIndicatorDirection::Up,
                PANEL_BORDER_COLOR,
            );
        }
        if viewport_end < item_count {
            canvas.draw_scroll_indicator(
                MENU_SCROLL_INDICATOR_X,
                MENU_SCROLL_INDICATOR_BOTTOM_Y,
                ScrollIndicatorDirection::Down,
                PANEL_BORDER_COLOR,
            );
        }

        for (visible_index, index) in (viewport_start..viewport_end).enumerate() {
            let item = visible_item_at(screen, index, presentation)
                .expect("render viewport index should map to a visible item");
            let item_y = MENU_ITEM_TEXT_Y + visible_index * MENU_ITEM_HEIGHT;
            let enabled = if self.pending_binding_capture.is_some() {
                self.pending_binding_item() == Some(item)
            } else {
                presentation.item_enabled(item)
            };
            let selected = selected_index == index;
            if selected {
                canvas.fill_rect(
                    MENU_PANEL_X + 8,
                    item_y.saturating_sub(3),
                    MENU_PANEL_WIDTH.saturating_sub(16),
                    11,
                    SELECTION_COLOR,
                );
                canvas.fill_rect(MENU_ITEM_CURSOR_X, item_y, 4, 4, CURSOR_COLOR);
            }

            let color = if !enabled {
                DISABLED_TEXT_COLOR
            } else if selected {
                SELECTED_TEXT_COLOR
            } else {
                TEXT_COLOR
            };
            let label = if self.pending_binding_item() == Some(item) {
                if self.pending_keyboard_binding_target().is_some()
                    || self.pending_keyboard_menu_binding_target().is_some()
                {
                    "PRESS KEY".to_string()
                } else {
                    "PRESS BTN".to_string()
                }
            } else {
                rendered_item_label(item, selected, presentation, selection_elapsed)
            };
            canvas.draw_text(MENU_ITEM_TEXT_X, item_y, &label, color, 1);
        }
    }

    fn apply_item_action(
        &mut self,
        item: MenuItem,
        presentation: MenuPresentation,
    ) -> Option<MenuAction> {
        match item {
            MenuItem::Resume => {
                self.close();
                Some(MenuAction::Close)
            }
            MenuItem::OpenRom => Some(MenuAction::OpenRom),
            MenuItem::RecentMenu => {
                self.push_screen(MenuScreen::Recent, presentation);
                None
            }
            MenuItem::RecentRom1 => Some(MenuAction::OpenRecentRom(0)),
            MenuItem::RecentRom2 => Some(MenuAction::OpenRecentRom(1)),
            MenuItem::RecentRom3 => Some(MenuAction::OpenRecentRom(2)),
            MenuItem::RecentRom4 => Some(MenuAction::OpenRecentRom(3)),
            MenuItem::RecentRom5 => Some(MenuAction::OpenRecentRom(4)),
            MenuItem::RecentRom6 => Some(MenuAction::OpenRecentRom(5)),
            MenuItem::RecentRom7 => Some(MenuAction::OpenRecentRom(6)),
            MenuItem::RecentRom8 => Some(MenuAction::OpenRecentRom(7)),
            MenuItem::ClearRecentList => Some(MenuAction::ClearRecentList),
            MenuItem::SaveBattery => Some(MenuAction::SaveBattery),
            MenuItem::VideoMenu => {
                self.push_screen(MenuScreen::Video, presentation);
                None
            }
            MenuItem::AudioMenu => {
                self.push_screen(MenuScreen::Audio, presentation);
                None
            }
            MenuItem::InputMenu => {
                self.push_screen(MenuScreen::Input, presentation);
                None
            }
            MenuItem::ExtPortMenu => {
                self.push_screen(MenuScreen::ExtPort, presentation);
                None
            }
            MenuItem::KeyboardMenu => {
                self.push_screen(MenuScreen::Keyboard, presentation);
                None
            }
            MenuItem::KeyboardMenuControls => {
                self.push_screen(MenuScreen::KeyboardMenuControls, presentation);
                None
            }
            MenuItem::HotkeysMenu => {
                self.push_screen(MenuScreen::Hotkeys, presentation);
                None
            }
            MenuItem::GamepadMenu => {
                self.push_screen(MenuScreen::Gamepad, presentation);
                None
            }
            MenuItem::GamepadMenuControls => {
                self.push_screen(MenuScreen::GamepadMenuControls, presentation);
                None
            }
            MenuItem::SystemMenu => {
                self.push_screen(MenuScreen::System, presentation);
                None
            }
            MenuItem::ConsoleModel => Some(MenuAction::CycleConsoleModel),
            MenuItem::StartupMode => Some(MenuAction::CycleStartupMode),
            MenuItem::ExecutionMode => Some(MenuAction::CycleExecutionMode),
            MenuItem::BootRomDefaultPath => Some(MenuAction::ClearBootRomPath),
            MenuItem::BootRomFilePath => Some(MenuAction::SelectBootRomFilePath),
            MenuItem::BootRomDirectoryPath => Some(MenuAction::SelectBootRomDirectoryPath),
            MenuItem::BootRomVerify => Some(MenuAction::CycleBootRomVerify),
            MenuItem::SavesEnabled => Some(MenuAction::ToggleSavesEnabled),
            MenuItem::SavePolicy => Some(MenuAction::CycleSavePolicy),
            MenuItem::SaveDefaultPath => Some(MenuAction::ClearSaveDirectoryPath),
            MenuItem::SaveDirectoryPath => Some(MenuAction::SelectSaveDirectoryPath),
            MenuItem::Fullscreen => Some(MenuAction::ToggleFullscreen),
            MenuItem::Vsync => Some(MenuAction::ToggleVsync),
            MenuItem::WindowScale => Some(MenuAction::CycleWindowScale),
            MenuItem::IntegerScale => Some(MenuAction::ToggleIntegerScale),
            MenuItem::PresentationFilter => Some(MenuAction::TogglePresentationFilter),
            MenuItem::ShowBackground => Some(MenuAction::ToggleBackgroundLayer),
            MenuItem::ShowWindow => Some(MenuAction::ToggleWindowLayer),
            MenuItem::ShowObjects => Some(MenuAction::ToggleObjectLayer),
            MenuItem::Screenshot => Some(MenuAction::SaveScreenshot),
            MenuItem::PerformanceHud => Some(MenuAction::TogglePerformanceHud),
            MenuItem::VideoDefaults => Some(MenuAction::ResetVideoDefaults),
            MenuItem::ToggleMute => Some(MenuAction::ToggleMute),
            MenuItem::AudioVolume => Some(MenuAction::CycleAudioVolume),
            MenuItem::AudioDefaults => Some(MenuAction::ResetAudioDefaults),
            MenuItem::GamepadDirection => Some(MenuAction::CycleGamepadDirectionalSource),
            MenuItem::ExternalPortNone => Some(MenuAction::SetExternalPort(
                DesktopExternalPortSelection::None,
            )),
            MenuItem::ExternalPortPrinter => Some(MenuAction::SetExternalPort(
                DesktopExternalPortSelection::Printer,
            )),
            MenuItem::ExternalPortGameLink => Some(MenuAction::SetExternalPort(
                DesktopExternalPortSelection::GameLink,
            )),
            MenuItem::ExternalPortFourPlayerAdapter => None,
            MenuItem::GamepadRumble => Some(MenuAction::CycleGamepadRumbleMode),
            MenuItem::InputDefaults => Some(MenuAction::ResetInputDefaults),
            MenuItem::GamepadActive => None,
            MenuItem::GamepadPreferred => Some(MenuAction::TogglePreferredGamepad),
            MenuItem::KeyboardUp => {
                self.pending_binding_capture =
                    Some(PendingBindingCapture::Keyboard(KeyboardBindingTarget::Up));
                None
            }
            MenuItem::KeyboardDown => {
                self.pending_binding_capture =
                    Some(PendingBindingCapture::Keyboard(KeyboardBindingTarget::Down));
                None
            }
            MenuItem::KeyboardLeft => {
                self.pending_binding_capture =
                    Some(PendingBindingCapture::Keyboard(KeyboardBindingTarget::Left));
                None
            }
            MenuItem::KeyboardRight => {
                self.pending_binding_capture = Some(PendingBindingCapture::Keyboard(
                    KeyboardBindingTarget::Right,
                ));
                None
            }
            MenuItem::KeyboardA => {
                self.pending_binding_capture =
                    Some(PendingBindingCapture::Keyboard(KeyboardBindingTarget::A));
                None
            }
            MenuItem::KeyboardB => {
                self.pending_binding_capture =
                    Some(PendingBindingCapture::Keyboard(KeyboardBindingTarget::B));
                None
            }
            MenuItem::KeyboardSelect => {
                self.pending_binding_capture = Some(PendingBindingCapture::Keyboard(
                    KeyboardBindingTarget::Select,
                ));
                None
            }
            MenuItem::KeyboardStart => {
                self.pending_binding_capture = Some(PendingBindingCapture::Keyboard(
                    KeyboardBindingTarget::Start,
                ));
                None
            }
            MenuItem::KeyboardMenuUp => {
                self.pending_binding_capture = Some(PendingBindingCapture::KeyboardMenu(
                    KeyboardMenuBindingTarget::Up,
                ));
                None
            }
            MenuItem::KeyboardMenuDown => {
                self.pending_binding_capture = Some(PendingBindingCapture::KeyboardMenu(
                    KeyboardMenuBindingTarget::Down,
                ));
                None
            }
            MenuItem::KeyboardMenuConfirm => {
                self.pending_binding_capture = Some(PendingBindingCapture::KeyboardMenu(
                    KeyboardMenuBindingTarget::Confirm,
                ));
                None
            }
            MenuItem::KeyboardMenuCancel => {
                self.pending_binding_capture = Some(PendingBindingCapture::KeyboardMenu(
                    KeyboardMenuBindingTarget::Cancel,
                ));
                None
            }
            MenuItem::HotkeyPause => {
                self.pending_binding_capture = Some(PendingBindingCapture::Keyboard(
                    KeyboardBindingTarget::Pause,
                ));
                None
            }
            MenuItem::HotkeyReset => {
                self.pending_binding_capture = Some(PendingBindingCapture::Keyboard(
                    KeyboardBindingTarget::Reset,
                ));
                None
            }
            MenuItem::HotkeyFullscreen => {
                self.pending_binding_capture = Some(PendingBindingCapture::Keyboard(
                    KeyboardBindingTarget::ToggleFullscreen,
                ));
                None
            }
            MenuItem::HotkeyPerformanceHud => {
                self.pending_binding_capture = Some(PendingBindingCapture::Keyboard(
                    KeyboardBindingTarget::TogglePerformanceHud,
                ));
                None
            }
            MenuItem::HotkeySaveBattery => {
                self.pending_binding_capture = Some(PendingBindingCapture::Keyboard(
                    KeyboardBindingTarget::SaveBattery,
                ));
                None
            }
            MenuItem::GamepadUp => {
                self.pending_binding_capture =
                    Some(PendingBindingCapture::Gamepad(GamepadBindingTarget::Up));
                None
            }
            MenuItem::GamepadDown => {
                self.pending_binding_capture =
                    Some(PendingBindingCapture::Gamepad(GamepadBindingTarget::Down));
                None
            }
            MenuItem::GamepadLeft => {
                self.pending_binding_capture =
                    Some(PendingBindingCapture::Gamepad(GamepadBindingTarget::Left));
                None
            }
            MenuItem::GamepadRight => {
                self.pending_binding_capture =
                    Some(PendingBindingCapture::Gamepad(GamepadBindingTarget::Right));
                None
            }
            MenuItem::GamepadA => {
                self.pending_binding_capture =
                    Some(PendingBindingCapture::Gamepad(GamepadBindingTarget::A));
                None
            }
            MenuItem::GamepadB => {
                self.pending_binding_capture =
                    Some(PendingBindingCapture::Gamepad(GamepadBindingTarget::B));
                None
            }
            MenuItem::GamepadSelect => {
                self.pending_binding_capture =
                    Some(PendingBindingCapture::Gamepad(GamepadBindingTarget::Select));
                None
            }
            MenuItem::GamepadStart => {
                self.pending_binding_capture =
                    Some(PendingBindingCapture::Gamepad(GamepadBindingTarget::Start));
                None
            }
            MenuItem::GamepadMenuUp => {
                self.pending_binding_capture = Some(PendingBindingCapture::GamepadMenu(
                    GamepadMenuBindingTarget::Up,
                ));
                None
            }
            MenuItem::GamepadMenuDown => {
                self.pending_binding_capture = Some(PendingBindingCapture::GamepadMenu(
                    GamepadMenuBindingTarget::Down,
                ));
                None
            }
            MenuItem::GamepadMenuConfirm => {
                self.pending_binding_capture = Some(PendingBindingCapture::GamepadMenu(
                    GamepadMenuBindingTarget::Confirm,
                ));
                None
            }
            MenuItem::GamepadMenuCancel => {
                self.pending_binding_capture = Some(PendingBindingCapture::GamepadMenu(
                    GamepadMenuBindingTarget::Cancel,
                ));
                None
            }
            MenuItem::Reset => Some(MenuAction::Reset),
            MenuItem::Quit => Some(MenuAction::Quit),
            MenuItem::Return => {
                self.pop_screen_or_close(presentation);
                None
            }
        }
    }

    fn pending_binding_item(&self) -> Option<MenuItem> {
        match self.pending_binding_capture {
            Some(PendingBindingCapture::Keyboard(KeyboardBindingTarget::Up)) => {
                Some(MenuItem::KeyboardUp)
            }
            Some(PendingBindingCapture::Keyboard(KeyboardBindingTarget::Down)) => {
                Some(MenuItem::KeyboardDown)
            }
            Some(PendingBindingCapture::Keyboard(KeyboardBindingTarget::Left)) => {
                Some(MenuItem::KeyboardLeft)
            }
            Some(PendingBindingCapture::Keyboard(KeyboardBindingTarget::Right)) => {
                Some(MenuItem::KeyboardRight)
            }
            Some(PendingBindingCapture::Keyboard(KeyboardBindingTarget::A)) => {
                Some(MenuItem::KeyboardA)
            }
            Some(PendingBindingCapture::Keyboard(KeyboardBindingTarget::B)) => {
                Some(MenuItem::KeyboardB)
            }
            Some(PendingBindingCapture::Keyboard(KeyboardBindingTarget::Select)) => {
                Some(MenuItem::KeyboardSelect)
            }
            Some(PendingBindingCapture::Keyboard(KeyboardBindingTarget::Start)) => {
                Some(MenuItem::KeyboardStart)
            }
            Some(PendingBindingCapture::KeyboardMenu(KeyboardMenuBindingTarget::Up)) => {
                Some(MenuItem::KeyboardMenuUp)
            }
            Some(PendingBindingCapture::KeyboardMenu(KeyboardMenuBindingTarget::Down)) => {
                Some(MenuItem::KeyboardMenuDown)
            }
            Some(PendingBindingCapture::KeyboardMenu(KeyboardMenuBindingTarget::Confirm)) => {
                Some(MenuItem::KeyboardMenuConfirm)
            }
            Some(PendingBindingCapture::KeyboardMenu(KeyboardMenuBindingTarget::Cancel)) => {
                Some(MenuItem::KeyboardMenuCancel)
            }
            Some(PendingBindingCapture::Keyboard(KeyboardBindingTarget::Pause)) => {
                Some(MenuItem::HotkeyPause)
            }
            Some(PendingBindingCapture::Keyboard(KeyboardBindingTarget::Reset)) => {
                Some(MenuItem::HotkeyReset)
            }
            Some(PendingBindingCapture::Keyboard(KeyboardBindingTarget::ToggleFullscreen)) => {
                Some(MenuItem::HotkeyFullscreen)
            }
            Some(PendingBindingCapture::Keyboard(KeyboardBindingTarget::TogglePerformanceHud)) => {
                Some(MenuItem::HotkeyPerformanceHud)
            }
            Some(PendingBindingCapture::Keyboard(KeyboardBindingTarget::SaveBattery)) => {
                Some(MenuItem::HotkeySaveBattery)
            }
            Some(PendingBindingCapture::Gamepad(GamepadBindingTarget::Up)) => {
                Some(MenuItem::GamepadUp)
            }
            Some(PendingBindingCapture::Gamepad(GamepadBindingTarget::Down)) => {
                Some(MenuItem::GamepadDown)
            }
            Some(PendingBindingCapture::Gamepad(GamepadBindingTarget::Left)) => {
                Some(MenuItem::GamepadLeft)
            }
            Some(PendingBindingCapture::Gamepad(GamepadBindingTarget::Right)) => {
                Some(MenuItem::GamepadRight)
            }
            Some(PendingBindingCapture::Gamepad(GamepadBindingTarget::A)) => {
                Some(MenuItem::GamepadA)
            }
            Some(PendingBindingCapture::Gamepad(GamepadBindingTarget::B)) => {
                Some(MenuItem::GamepadB)
            }
            Some(PendingBindingCapture::Gamepad(GamepadBindingTarget::Select)) => {
                Some(MenuItem::GamepadSelect)
            }
            Some(PendingBindingCapture::Gamepad(GamepadBindingTarget::Start)) => {
                Some(MenuItem::GamepadStart)
            }
            Some(PendingBindingCapture::GamepadMenu(GamepadMenuBindingTarget::Up)) => {
                Some(MenuItem::GamepadMenuUp)
            }
            Some(PendingBindingCapture::GamepadMenu(GamepadMenuBindingTarget::Down)) => {
                Some(MenuItem::GamepadMenuDown)
            }
            Some(PendingBindingCapture::GamepadMenu(GamepadMenuBindingTarget::Confirm)) => {
                Some(MenuItem::GamepadMenuConfirm)
            }
            Some(PendingBindingCapture::GamepadMenu(GamepadMenuBindingTarget::Cancel)) => {
                Some(MenuItem::GamepadMenuCancel)
            }
            None => None,
        }
    }

    fn push_screen(&mut self, screen: MenuScreen, presentation: MenuPresentation) {
        self.screen_stack
            .push(ScreenState::new(screen, presentation));
        self.selection_started_at = Some(Instant::now());
    }

    fn pop_screen_or_close(&mut self, presentation: MenuPresentation) -> Option<MenuAction> {
        self.pending_binding_capture = None;
        if self.screen_stack.len() > 1 {
            self.screen_stack.pop();
            self.selection_started_at = Some(Instant::now());
            None
        } else if !presentation.rom_loaded {
            None
        } else {
            self.close();
            Some(MenuAction::Close)
        }
    }

    fn normalize_current_selection(&mut self, presentation: MenuPresentation) {
        let screen_state = self.current_screen_state();
        let selected_index = normalized_selected_index(
            screen_state.screen,
            screen_state.selected_index,
            presentation,
        );
        self.set_selected_index(selected_index);
    }

    fn current_screen_state(&self) -> &ScreenState {
        self.screen_stack
            .last()
            .expect("open overlay menu should always have one active screen")
    }

    fn current_screen_state_mut(&mut self) -> &mut ScreenState {
        self.screen_stack
            .last_mut()
            .expect("open overlay menu should always have one active screen")
    }

    fn set_selected_index(&mut self, selected_index: usize) {
        let selection_changed = {
            let screen_state = self.current_screen_state_mut();
            if screen_state.selected_index == selected_index {
                false
            } else {
                screen_state.selected_index = selected_index;
                true
            }
        };

        if selection_changed {
            self.selection_started_at = Some(Instant::now());
        }
    }

    fn selection_elapsed(&self) -> Duration {
        self.selection_started_at
            .map_or(Duration::default(), |started_at| started_at.elapsed())
    }
}

#[cfg(test)]
impl OverlayMenuState {
    pub(crate) fn begin_keyboard_binding_capture_for_tests(
        &mut self,
        target: KeyboardBindingTarget,
    ) {
        self.pending_binding_capture = Some(PendingBindingCapture::Keyboard(target));
    }

    pub(crate) fn begin_keyboard_menu_binding_capture_for_tests(
        &mut self,
        target: KeyboardMenuBindingTarget,
    ) {
        self.pending_binding_capture = Some(PendingBindingCapture::KeyboardMenu(target));
    }

    pub(crate) fn begin_gamepad_binding_capture_for_tests(&mut self, target: GamepadBindingTarget) {
        self.pending_binding_capture = Some(PendingBindingCapture::Gamepad(target));
    }
}

fn items_for_screen(screen: MenuScreen) -> &'static [MenuItem] {
    match screen {
        MenuScreen::Root => &ROOT_MENU_ITEMS,
        MenuScreen::Recent => &RECENT_MENU_ITEMS,
        MenuScreen::Video => &VIDEO_MENU_ITEMS,
        MenuScreen::Audio => &AUDIO_MENU_ITEMS,
        MenuScreen::Input => &INPUT_MENU_ITEMS,
        MenuScreen::ExtPort => &EXT_PORT_MENU_ITEMS,
        MenuScreen::Gamepad => &GAMEPAD_MENU_ITEMS,
        MenuScreen::GamepadMenuControls => &GAMEPAD_MENU_CONTROL_ITEMS,
        MenuScreen::Keyboard => &KEYBOARD_MENU_ITEMS,
        MenuScreen::KeyboardMenuControls => &KEYBOARD_MENU_CONTROL_ITEMS,
        MenuScreen::Hotkeys => &HOTKEYS_MENU_ITEMS,
        MenuScreen::System => &SYSTEM_MENU_ITEMS,
    }
}

fn desktop_key_label(key: DesktopKey) -> &'static str {
    match key {
        DesktopKey::Escape => "ESC",
        DesktopKey::ArrowUp => "UP",
        DesktopKey::ArrowDown => "DOWN",
        DesktopKey::ArrowLeft => "LEFT",
        DesktopKey::ArrowRight => "RIGHT",
        DesktopKey::Backspace => "BACK",
        DesktopKey::Return => "ENTER",
        DesktopKey::Space => "SPACE",
        DesktopKey::R => "R",
        DesktopKey::X => "X",
        DesktopKey::Z => "Z",
        DesktopKey::F5 => "F5",
        DesktopKey::F10 => "F10",
        DesktopKey::F11 => "F11",
    }
}

fn gamepad_binding_label(binding: GamepadButtonBinding) -> &'static str {
    match binding {
        GamepadButtonBinding::South => "SOUTH",
        GamepadButtonBinding::East => "EAST",
        GamepadButtonBinding::West => "WEST",
        GamepadButtonBinding::North => "NORTH",
        GamepadButtonBinding::Back => "BACK",
        GamepadButtonBinding::Start => "START",
        GamepadButtonBinding::Guide => "GUIDE",
        GamepadButtonBinding::LeftShoulder => "L1",
        GamepadButtonBinding::RightShoulder => "R1",
        GamepadButtonBinding::LeftStickClick => "LSTICK",
        GamepadButtonBinding::RightStickClick => "RSTICK",
        GamepadButtonBinding::DPadUp => "D UP",
        GamepadButtonBinding::DPadDown => "D DOWN",
        GamepadButtonBinding::DPadLeft => "D LEFT",
        GamepadButtonBinding::DPadRight => "D RIGHT",
        GamepadButtonBinding::Misc1 => "MISC1",
    }
}

fn recent_rom_item_label(label: CompactRecentRomLabel) -> String {
    if label.is_empty() {
        "ROM".to_string()
    } else {
        label.as_str().to_string()
    }
}

fn rendered_item_label(
    item: MenuItem,
    selected: bool,
    presentation: MenuPresentation,
    selection_elapsed: Duration,
) -> String {
    let label = presentation.item_label(item);
    if !matches!(
        item,
        MenuItem::RecentRom1
            | MenuItem::RecentRom2
            | MenuItem::RecentRom3
            | MenuItem::RecentRom4
            | MenuItem::RecentRom5
            | MenuItem::RecentRom6
            | MenuItem::RecentRom7
            | MenuItem::RecentRom8
    ) {
        return label;
    }

    rendered_recent_rom_item_label(&label, selected, selection_elapsed)
}

fn rendered_recent_rom_item_label(
    label: &str,
    selected: bool,
    selection_elapsed: Duration,
) -> String {
    if label.len() <= MENU_ITEM_TEXT_CAPACITY {
        return label.to_string();
    }

    if !selected || selection_elapsed < RECENT_ROM_SCROLL_DELAY {
        return label[..MENU_ITEM_TEXT_CAPACITY].to_string();
    }

    let step = (selection_elapsed - RECENT_ROM_SCROLL_DELAY).as_millis()
        / RECENT_ROM_SCROLL_STEP.as_millis();
    let cycle_offset = (step as usize) % (label.len() + RECENT_ROM_SCROLL_GAP_CHARS);
    marquee_label_window(label, cycle_offset, MENU_ITEM_TEXT_CAPACITY)
}

fn marquee_label_window(label: &str, start_offset: usize, visible_capacity: usize) -> String {
    let cycle_len = label.len() + RECENT_ROM_SCROLL_GAP_CHARS;
    let bytes = label.as_bytes();
    let mut window = String::with_capacity(visible_capacity);
    for offset in 0..visible_capacity {
        let index = (start_offset + offset) % cycle_len;
        if index < bytes.len() {
            window.push(bytes[index] as char);
        } else {
            window.push(' ');
        }
    }

    window
}

fn compact_gamepad_name(name: &str) -> String {
    let mut all_tokens = Vec::new();
    let mut filtered_tokens = Vec::new();
    for token in name.split(|character: char| !character.is_ascii_alphanumeric()) {
        if token.is_empty() {
            continue;
        }

        let token = token.to_ascii_uppercase();
        if !matches!(token.as_str(), "CONTROLLER" | "WIRELESS" | "GAMEPAD") {
            filtered_tokens.push(token.clone());
        }
        all_tokens.push(token);
    }

    let mut tokens = if filtered_tokens.is_empty() {
        all_tokens
    } else {
        filtered_tokens
    };

    let Some(last_token) = tokens.pop() else {
        return "PAD".to_string();
    };

    if let Some(previous_token) = tokens.pop() {
        let remaining_len = COMPACT_MENU_LABEL_MAX_BYTES.saturating_sub(last_token.len() + 1);
        if remaining_len > 0 {
            let prefix = &previous_token[..previous_token.len().min(remaining_len)];
            return format!("{prefix} {last_token}");
        }
    }

    last_token[..last_token.len().min(COMPACT_MENU_LABEL_MAX_BYTES)].to_string()
}

fn performance_hud_lines(snapshot: PerformanceHudSnapshot) -> [String; 4] {
    [
        format!(
            "FPS {} {}%",
            hud_number(snapshot.fps),
            hud_number(snapshot.speed_percent)
        ),
        format!(
            "FRM {} EMU {}",
            hud_number(snapshot.frame_time_ms),
            hud_number(snapshot.emulation_time_ms)
        ),
        format!(
            "REN {} PAC {}",
            hud_number(snapshot.render_time_ms),
            hud_number(snapshot.pacing_time_ms)
        ),
        snapshot
            .audio_queue_ms
            .map(|audio_queue_ms| format!("AUD {}", hud_number(audio_queue_ms)))
            .unwrap_or_else(|| "AUD OFF".to_string()),
    ]
}

fn hud_number(value: f64) -> u32 {
    value.max(0.0).round() as u32
}

fn visible_item_count(screen: MenuScreen, presentation: MenuPresentation) -> usize {
    items_for_screen(screen)
        .iter()
        .copied()
        .filter(|item| presentation.item_visible(*item))
        .count()
}

fn visible_item_at(
    screen: MenuScreen,
    visible_index: usize,
    presentation: MenuPresentation,
) -> Option<MenuItem> {
    items_for_screen(screen)
        .iter()
        .copied()
        .filter(|item| presentation.item_visible(*item))
        .nth(visible_index)
}

fn first_enabled_index(screen: MenuScreen, presentation: MenuPresentation) -> usize {
    items_for_screen(screen)
        .iter()
        .copied()
        .filter(|item| presentation.item_visible(*item))
        .position(|item| presentation.item_enabled(item))
        .unwrap_or(0)
}

fn normalized_selected_index(
    screen: MenuScreen,
    current_index: usize,
    presentation: MenuPresentation,
) -> usize {
    if visible_item_at(screen, current_index, presentation).is_some() {
        current_index
    } else {
        first_enabled_index(screen, presentation)
    }
}

fn viewport_start_index(selected_index: usize, item_count: usize) -> usize {
    let max_start = item_count.saturating_sub(MENU_VISIBLE_ITEM_CAPACITY);
    selected_index
        .saturating_sub(MENU_VISIBLE_ITEM_CAPACITY.saturating_sub(1))
        .min(max_start)
}

fn next_enabled_index(
    screen: MenuScreen,
    current_index: usize,
    presentation: MenuPresentation,
) -> usize {
    let item_count = visible_item_count(screen, presentation);
    if item_count == 0 {
        return current_index;
    }

    for step in 1..=item_count {
        let index = (current_index + step) % item_count;
        if visible_item_at(screen, index, presentation)
            .is_some_and(|item| presentation.item_enabled(item))
        {
            return index;
        }
    }

    current_index
}

fn previous_enabled_index(
    screen: MenuScreen,
    current_index: usize,
    presentation: MenuPresentation,
) -> usize {
    let item_count = visible_item_count(screen, presentation);
    if item_count == 0 {
        return current_index;
    }

    for step in 1..=item_count {
        let index = (current_index + item_count - step) % item_count;
        if visible_item_at(screen, index, presentation)
            .is_some_and(|item| presentation.item_enabled(item))
        {
            return index;
        }
    }

    current_index
}

fn text_width(text: &str, scale: usize) -> usize {
    text.chars()
        .count()
        .saturating_mul((GLYPH_WIDTH + GLYPH_SPACING) * scale)
        .saturating_sub(GLYPH_SPACING * scale)
}

fn glyph_rows(character: char) -> [u8; GLYPH_HEIGHT] {
    match character {
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'G' => [
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'Q' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ],
        'W' => [
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b11100,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '%' => [
            0b11001, 0b11010, 0b00100, 0b01000, 0b10110, 0b00110, 0b00000,
        ],
        ' ' => [0, 0, 0, 0, 0, 0, 0],
        _ => [0, 0, 0, 0, 0, 0, 0],
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CompactMenuLabel, CompactRecentRomLabel, GamepadBindingTarget, GamepadMenuBindingTarget,
        KeyboardBindingTarget, KeyboardMenuBindingTarget, MENU_VISIBLE_ITEM_CAPACITY, MenuAction,
        MenuInput, MenuItem, MenuPresentation, MenuScreen, OverlayMenuState,
        PerformanceHudSnapshot, RECENT_ROM_MENU_CAPACITY, ScrollIndicatorDirection,
        gamepad_binding_label, normalized_selected_index, performance_hud_lines,
        previous_enabled_index, render_performance_hud, rendered_recent_rom_item_label,
        scroll_indicator_rows, viewport_start_index,
    };
    use gb_core::{ExecutionMode, StartupMode};
    use gb_desktop::{
        BootRomVerificationMode, DesktopConsoleModel, DesktopExternalPortSelection, DesktopKey,
        DesktopSaveFlushPolicy, GamepadButtonBinding, GamepadButtonBindings,
        GamepadDirectionalSource, GamepadMenuBindings, GamepadRumbleMode, HotkeyBindings,
        JoypadKeyboardBindings, MenuKeyboardBindings,
    };
    use std::time::Duration;

    fn test_presentation() -> MenuPresentation {
        MenuPresentation {
            rom_loaded: true,
            recent_rom_count: 0,
            recent_rom_labels: [CompactRecentRomLabel::default(); RECENT_ROM_MENU_CAPACITY],
            console_model: DesktopConsoleModel::Dmg,
            startup_mode: StartupMode::SkipBoot,
            execution_mode: ExecutionMode::Strict,
            external_port_selection: DesktopExternalPortSelection::None,
            boot_rom_uses_default_path: true,
            boot_rom_verification: BootRomVerificationMode::Strict,
            saves_enabled: true,
            save_flush_policy: DesktopSaveFlushPolicy::Debounced,
            save_directory_uses_default_path: true,
            fullscreen: false,
            vsync: true,
            window_scale: 4,
            integer_scale: true,
            presentation_filter: false,
            show_background: true,
            show_window: true,
            show_objects: true,
            show_performance_hud: true,
            muted: false,
            audio_available: false,
            audio_volume_percent: 100,
            manual_save_available: false,
            any_dialog_pending: false,
            gamepad_available: false,
            gamepad_directional_source: GamepadDirectionalSource::DpadAndLeftStick,
            gamepad_rumble_mode: GamepadRumbleMode::Strong,
            gamepad_bindings: GamepadButtonBindings::default(),
            gamepad_menu_bindings: GamepadMenuBindings::default(),
            active_gamepad_connected: false,
            cartridge_rumble_supported: false,
            active_gamepad_rumble_supported: false,
            active_gamepad_label: CompactMenuLabel::default(),
            preferred_gamepad_configured: false,
            preferred_gamepad_label: CompactMenuLabel::default(),
            keyboard_bindings: JoypadKeyboardBindings::default(),
            keyboard_menu_bindings: MenuKeyboardBindings::default(),
            hotkey_bindings: HotkeyBindings::default(),
        }
    }

    #[test]
    fn opening_the_menu_selects_the_first_enabled_root_item() {
        let presentation = test_presentation();
        let mut menu = OverlayMenuState::default();

        menu.open(presentation);

        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::Close)
        );
    }

    #[test]
    fn navigation_skips_disabled_root_items() {
        let presentation = MenuPresentation {
            rom_loaded: false,
            ..test_presentation()
        };
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::OpenRom)
        );
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::ToggleFullscreen)
        );
    }

    #[test]
    fn audio_item_toggles_mute_inside_the_audio_submenu() {
        let presentation = MenuPresentation {
            audio_available: true,
            ..test_presentation()
        };
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::ToggleMute)
        );
    }

    #[test]
    fn audio_submenu_cycles_volume_after_mute() {
        let presentation = MenuPresentation {
            audio_available: true,
            ..test_presentation()
        };
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::CycleAudioVolume)
        );
    }

    #[test]
    fn video_submenu_cycles_scale_and_toggles_integer_presentation() {
        let presentation = test_presentation();
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::CycleWindowScale)
        );
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::ToggleIntegerScale)
        );
    }

    #[test]
    fn video_submenu_toggles_the_performance_hud() {
        let presentation = test_presentation();
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::TogglePerformanceHud)
        );
    }

    #[test]
    fn video_submenu_toggles_the_presentation_filter() {
        let presentation = test_presentation();
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::TogglePresentationFilter)
        );
    }

    #[test]
    fn video_submenu_saves_a_screenshot_after_filter() {
        let presentation = test_presentation();
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::SaveScreenshot)
        );
    }

    #[test]
    fn video_submenu_exposes_layer_toggles_after_filter() {
        let presentation = test_presentation();
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::ToggleBackgroundLayer)
        );
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::ToggleWindowLayer)
        );
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::ToggleObjectLayer)
        );
    }

    #[test]
    fn video_submenu_toggles_vsync_before_scale() {
        let presentation = test_presentation();
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::ToggleVsync)
        );
    }

    #[test]
    fn video_submenu_resets_defaults_after_the_host_toggles() {
        let presentation = test_presentation();
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::ResetVideoDefaults)
        );
    }

    #[test]
    fn audio_submenu_resets_defaults_after_volume() {
        let presentation = MenuPresentation {
            audio_available: true,
            ..test_presentation()
        };
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::ResetAudioDefaults)
        );
    }

    #[test]
    fn input_submenu_cycles_the_gamepad_directional_source() {
        let presentation = MenuPresentation {
            audio_available: true,
            gamepad_available: true,
            ..test_presentation()
        };
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::CycleGamepadDirectionalSource)
        );
    }

    #[test]
    fn input_submenu_cycles_the_gamepad_rumble_mode_when_supported() {
        let presentation = MenuPresentation {
            audio_available: true,
            gamepad_available: true,
            cartridge_rumble_supported: true,
            active_gamepad_rumble_supported: true,
            ..test_presentation()
        };
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::CycleGamepadRumbleMode)
        );
    }

    #[test]
    fn input_submenu_resets_defaults_after_directional_source() {
        let presentation = MenuPresentation {
            audio_available: true,
            gamepad_available: true,
            ..test_presentation()
        };
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::ResetInputDefaults)
        );
    }

    #[test]
    fn system_submenu_cycles_model_startup_and_execution_mode() {
        let presentation = test_presentation();
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::CycleConsoleModel)
        );
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::CycleStartupMode)
        );
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::CycleExecutionMode)
        );
    }

    #[test]
    fn system_submenu_exposes_boot_path_and_verify_actions() {
        let presentation = test_presentation();
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::SelectBootRomFilePath)
        );
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::SelectBootRomDirectoryPath)
        );
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::CycleBootRomVerify)
        );
    }

    #[test]
    fn system_submenu_exposes_save_actions() {
        let presentation = test_presentation();
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::ToggleSavesEnabled)
        );
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::CycleSavePolicy)
        );
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::ClearSaveDirectoryPath)
        );
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::SelectSaveDirectoryPath)
        );
    }

    #[test]
    fn opening_rom_is_skipped_while_dialog_is_pending() {
        let presentation = MenuPresentation {
            rom_loaded: false,
            ..test_presentation()
        };
        let blocked_presentation = MenuPresentation {
            any_dialog_pending: true,
            ..presentation
        };
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(
            menu.handle_input(MenuInput::Confirm, blocked_presentation),
            None
        );
        assert_eq!(
            menu.handle_input(MenuInput::Down, blocked_presentation),
            None
        );
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, blocked_presentation),
            None
        );
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, blocked_presentation),
            Some(MenuAction::ToggleFullscreen)
        );
    }

    #[test]
    fn open_rom_stays_selected_while_the_dialog_is_pending() {
        let presentation = MenuPresentation {
            rom_loaded: false,
            ..test_presentation()
        };
        let blocked_presentation = MenuPresentation {
            any_dialog_pending: true,
            ..presentation
        };
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::OpenRom)
        );
        assert_eq!(
            normalized_selected_index(MenuScreen::Root, 1, blocked_presentation),
            1
        );
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, blocked_presentation),
            None
        );
    }

    #[test]
    fn recent_roms_root_entry_opens_the_recent_submenu() {
        let mut recent_rom_labels = [CompactRecentRomLabel::default(); RECENT_ROM_MENU_CAPACITY];
        recent_rom_labels[0] = CompactRecentRomLabel::from_text("TETRIS");
        let presentation = MenuPresentation {
            recent_rom_count: 1,
            recent_rom_labels,
            ..test_presentation()
        };
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::OpenRecentRom(0))
        );
    }

    #[test]
    fn recent_submenu_exposes_clear_list_before_return() {
        let mut recent_rom_labels = [CompactRecentRomLabel::default(); RECENT_ROM_MENU_CAPACITY];
        recent_rom_labels[0] = CompactRecentRomLabel::from_text("TETRIS");
        let presentation = MenuPresentation {
            recent_rom_count: 1,
            recent_rom_labels,
            ..test_presentation()
        };
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::ClearRecentList)
        );
    }

    #[test]
    fn cancel_in_a_submenu_returns_to_the_previous_screen() {
        let presentation = MenuPresentation {
            audio_available: true,
            ..test_presentation()
        };
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Cancel, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    }

    #[test]
    fn cancel_on_the_root_screen_stays_in_the_launcher_until_a_rom_is_loaded() {
        let presentation = MenuPresentation {
            rom_loaded: false,
            ..test_presentation()
        };
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Cancel, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::OpenRom)
        );
    }

    #[test]
    fn root_title_reports_no_rom_when_the_menu_is_acting_as_a_launcher() {
        let launcher_presentation = MenuPresentation {
            rom_loaded: false,
            ..test_presentation()
        };
        let loaded_presentation = MenuPresentation {
            rom_loaded: true,
            ..launcher_presentation
        };

        assert_eq!(MenuScreen::Root.title(launcher_presentation), "NO ROM");
        assert_eq!(MenuScreen::Root.title(loaded_presentation), "MENU");
    }

    #[test]
    fn save_battery_is_hidden_when_auto_flush_policy_is_active() {
        let presentation = MenuPresentation {
            audio_available: true,
            ..test_presentation()
        };
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::ToggleFullscreen)
        );
    }

    #[test]
    fn save_battery_remains_available_when_manual_save_policy_is_active() {
        let presentation = MenuPresentation {
            audio_available: true,
            manual_save_available: true,
            ..test_presentation()
        };
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::SaveBattery)
        );
    }

    #[test]
    fn keyboard_submenu_starts_a_capture_and_emits_the_selected_binding() {
        let presentation = test_presentation();
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert!(menu.is_capturing_binding());
        assert_eq!(
            menu.handle_keyboard_binding_capture(DesktopKey::Space),
            Some(MenuAction::SetKeyboardBinding(
                KeyboardBindingTarget::Up,
                DesktopKey::Space
            ))
        );
        assert!(!menu.is_capturing_binding());
    }

    #[test]
    fn keyboard_binding_capture_can_be_canceled_without_closing_the_menu() {
        let presentation = test_presentation();
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert!(menu.is_capturing_binding());

        menu.cancel_binding_capture();

        assert!(!menu.is_capturing_binding());
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
    }

    #[test]
    fn hotkeys_submenu_starts_a_capture_and_emits_the_selected_binding() {
        let presentation = test_presentation();
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert!(menu.is_capturing_binding());
        assert_eq!(
            menu.handle_keyboard_binding_capture(DesktopKey::F11),
            Some(MenuAction::SetKeyboardBinding(
                KeyboardBindingTarget::Pause,
                DesktopKey::F11
            ))
        );
        assert!(!menu.is_capturing_binding());
    }

    #[test]
    fn hotkeys_submenu_can_capture_the_stats_hud_hotkey() {
        let presentation = test_presentation();
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert!(menu.is_capturing_binding());
        assert_eq!(
            menu.handle_keyboard_binding_capture(DesktopKey::F10),
            Some(MenuAction::SetKeyboardBinding(
                KeyboardBindingTarget::TogglePerformanceHud,
                DesktopKey::F10
            ))
        );
        assert!(!menu.is_capturing_binding());
    }

    #[test]
    fn keyboard_menu_controls_submenu_starts_a_capture_and_emits_the_selected_binding() {
        let presentation = test_presentation();
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert!(menu.is_capturing_binding());
        assert_eq!(
            menu.handle_keyboard_binding_capture(DesktopKey::Space),
            Some(MenuAction::SetKeyboardMenuBinding(
                KeyboardMenuBindingTarget::Up,
                DesktopKey::Space
            ))
        );
        assert!(!menu.is_capturing_binding());
    }

    #[test]
    fn gamepad_submenu_starts_a_capture_and_emits_the_selected_binding() {
        let presentation = MenuPresentation {
            gamepad_available: true,
            ..test_presentation()
        };
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert!(menu.is_capturing_binding());
        assert_eq!(
            menu.handle_gamepad_binding_capture(GamepadButtonBinding::North),
            Some(MenuAction::SetGamepadBinding(
                GamepadBindingTarget::Up,
                GamepadButtonBinding::North
            ))
        );
        assert!(!menu.is_capturing_binding());
    }

    #[test]
    fn gamepad_menu_controls_submenu_starts_a_capture_and_emits_the_selected_binding() {
        let presentation = MenuPresentation {
            gamepad_available: true,
            ..test_presentation()
        };
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert!(menu.is_capturing_binding());
        assert_eq!(
            menu.handle_gamepad_binding_capture(GamepadButtonBinding::North),
            Some(MenuAction::SetGamepadMenuBinding(
                GamepadMenuBindingTarget::Up,
                GamepadButtonBinding::North
            ))
        );
        assert!(!menu.is_capturing_binding());
    }

    #[test]
    fn gamepad_submenu_exposes_the_preferred_device_toggle_before_bindings() {
        let presentation = MenuPresentation {
            gamepad_available: true,
            active_gamepad_connected: true,
            active_gamepad_label: CompactMenuLabel::from_text("SWITCH"),
            ..test_presentation()
        };
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Down, presentation), None);
        assert_eq!(menu.handle_input(MenuInput::Confirm, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::TogglePreferredGamepad)
        );
    }

    #[test]
    fn active_gamepad_labels_are_compacted_for_the_overlay_width() {
        assert_eq!(
            CompactMenuLabel::from_gamepad_name("Nintendo Switch Pro Controller").as_str(),
            "SWITC PRO"
        );
        assert_eq!(
            CompactMenuLabel::from_gamepad_name("Xbox Wireless Controller").as_str(),
            "XBOX"
        );
    }

    #[test]
    fn performance_hud_lines_round_metrics_and_report_audio_state() {
        let snapshot = PerformanceHudSnapshot {
            fps: 59.6,
            speed_percent: 99.7,
            frame_time_ms: 16.7,
            emulation_time_ms: 11.8,
            render_time_ms: 1.4,
            pacing_time_ms: 3.1,
            audio_queue_ms: Some(18.2),
        };

        assert_eq!(
            performance_hud_lines(snapshot),
            [
                "FPS 60 100%".to_string(),
                "FRM 17 EMU 12".to_string(),
                "REN 1 PAC 3".to_string(),
                "AUD 18".to_string(),
            ]
        );

        let without_audio = PerformanceHudSnapshot {
            audio_queue_ms: None,
            ..snapshot
        };
        assert_eq!(performance_hud_lines(without_audio)[3], "AUD OFF");
    }

    #[test]
    fn viewport_scrolls_to_keep_the_last_visible_items_in_view() {
        assert_eq!(MENU_VISIBLE_ITEM_CAPACITY, 5);
        assert_eq!(viewport_start_index(0, 6), 0);
        assert_eq!(viewport_start_index(4, 6), 0);
        assert_eq!(viewport_start_index(5, 6), 1);
        assert_eq!(viewport_start_index(0, 7), 0);
        assert_eq!(viewport_start_index(4, 7), 0);
        assert_eq!(viewport_start_index(5, 7), 1);
        assert_eq!(viewport_start_index(6, 7), 2);
    }

    #[test]
    fn scroll_indicators_point_toward_the_hidden_items() {
        assert_eq!(
            scroll_indicator_rows(ScrollIndicatorDirection::Up),
            [(0, 1), (1, 3), (2, 5)]
        );
        assert_eq!(
            scroll_indicator_rows(ScrollIndicatorDirection::Down),
            [(0, 5), (1, 3), (2, 1)]
        );
    }

    #[test]
    fn compact_labels_binding_labels_and_previous_navigation_cover_overlay_helpers() {
        assert!(CompactMenuLabel::default().is_empty());
        assert!(!CompactMenuLabel::from_text("PAD").is_empty());
        assert_eq!(CompactMenuLabel::from_text("PAD!? 12").as_str(), "PAD 12");
        assert_eq!(
            CompactRecentRomLabel::from_text("ROM!? 7").as_str(),
            "ROM 7"
        );
        assert_eq!(
            gamepad_binding_label(GamepadButtonBinding::RightShoulder),
            "R1"
        );
        assert_eq!(
            previous_enabled_index(MenuScreen::Root, 0, test_presentation()),
            7
        );

        let mut presentation = test_presentation();
        presentation.recent_rom_count = 2;
        presentation.recent_rom_labels[0] = CompactRecentRomLabel::from_text("TETRIS");
        presentation.recent_rom_labels[1] = CompactRecentRomLabel::from_text("MARIO");
        assert_eq!(presentation.item_label(MenuItem::RecentRom2), "MARIO");
    }

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
    fn menu_item_labels_cover_runtime_variants_and_binding_summaries() {
        let mut presentation = test_presentation();
        presentation.recent_rom_count = 8;
        for (index, label) in [
            "TETRIS", "MARIO", "DRMARIO", "KIRBY", "ZELDA", "WARIO", "METROID", "TENNIS",
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
        assert_eq!(
            presentation.item_label(MenuItem::ClearRecentList),
            "CLEAR LIST"
        );

        presentation.console_model = DesktopConsoleModel::Dmg0;
        assert_eq!(
            presentation.item_label(MenuItem::ConsoleModel),
            "MODEL DMG0"
        );
        presentation.console_model = DesktopConsoleModel::Mgb;
        assert_eq!(presentation.item_label(MenuItem::ConsoleModel), "MODEL MGB");

        presentation.startup_mode = StartupMode::RealBoot;
        assert_eq!(presentation.item_label(MenuItem::StartupMode), "START REAL");
        presentation.execution_mode = ExecutionMode::Permissive;
        assert_eq!(
            presentation.item_label(MenuItem::ExecutionMode),
            "MODE PERM"
        );
        presentation.execution_mode = ExecutionMode::Experimental;
        assert_eq!(presentation.item_label(MenuItem::ExecutionMode), "MODE EXP");

        presentation.boot_rom_uses_default_path = false;
        assert_eq!(
            presentation.item_label(MenuItem::BootRomDefaultPath),
            "BOOT AUTO OFF"
        );
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
        presentation.muted = true;
        assert_eq!(presentation.item_label(MenuItem::ToggleMute), "MUTE ON");
        presentation.audio_volume_percent = 250;
        assert_eq!(presentation.item_label(MenuItem::AudioVolume), "VOL 100%");

        assert_eq!(presentation.item_label(MenuItem::ExtPortMenu), "EXT NONE");
        assert_eq!(
            presentation.item_label(MenuItem::ExternalPortNone),
            "NONE ON"
        );
        presentation.external_port_selection = DesktopExternalPortSelection::Printer;
        assert_eq!(
            presentation.item_label(MenuItem::ExtPortMenu),
            "EXT PRINTER"
        );
        assert_eq!(presentation.item_label(MenuItem::ExternalPortNone), "NONE");
        assert_eq!(
            presentation.item_label(MenuItem::ExternalPortPrinter),
            "PRINTER ON"
        );
        assert_eq!(
            presentation.item_label(MenuItem::ExternalPortGameLink),
            "GAME LINK"
        );
        assert_eq!(
            presentation.item_label(MenuItem::ExternalPortFourPlayerAdapter),
            "4P ADAPTER"
        );
        assert!(presentation.item_enabled(MenuItem::ExternalPortGameLink));
        assert!(!presentation.item_enabled(MenuItem::ExternalPortFourPlayerAdapter));
        presentation.external_port_selection = DesktopExternalPortSelection::GameLink;
        assert_eq!(presentation.item_label(MenuItem::ExtPortMenu), "EXT LINK");
        presentation.external_port_selection = DesktopExternalPortSelection::FourPlayerAdapter;
        assert_eq!(presentation.item_label(MenuItem::ExtPortMenu), "EXT 4P");

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
            reset: DesktopKey::Space,
            toggle_fullscreen: DesktopKey::F11,
            toggle_performance_hud: DesktopKey::F10,
            save_battery: DesktopKey::F5,
        };
        assert_eq!(presentation.item_label(MenuItem::HotkeyPause), "PAUSE R");
        assert_eq!(
            presentation.item_label(MenuItem::HotkeyReset),
            "RESET SPACE"
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
            "SAVE BATTERY F5"
        );
        assert_eq!(presentation.item_label(MenuItem::Quit), "QUIT");
    }

    #[test]
    fn root_menu_exposes_quit_as_the_last_first_level_action() {
        let presentation = test_presentation();
        let mut menu = OverlayMenuState::default();
        menu.open(presentation);

        assert_eq!(menu.handle_input(MenuInput::Up, presentation), None);
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::Quit)
        );
    }

    #[test]
    fn overlay_actions_cover_binding_capture_targets_and_screen_titles() {
        let mut presentation = MenuPresentation {
            recent_rom_count: 1,
            audio_available: true,
            manual_save_available: true,
            gamepad_available: true,
            ..test_presentation()
        };
        presentation.recent_rom_labels[0] = CompactRecentRomLabel::from_text("TETRIS");

        assert_eq!(MenuScreen::Root.title(test_presentation()), "MENU");
        assert_eq!(
            MenuScreen::Root.title(MenuPresentation {
                rom_loaded: false,
                ..test_presentation()
            }),
            "NO ROM"
        );
        assert_eq!(MenuScreen::Recent.title(presentation), "RECENT");
        assert_eq!(MenuScreen::Video.title(presentation), "VIDEO");
        assert_eq!(MenuScreen::Audio.title(presentation), "AUDIO");
        assert_eq!(MenuScreen::Input.title(presentation), "INPUT");
        assert_eq!(MenuScreen::ExtPort.title(presentation), "EXT PORT");
        assert_eq!(MenuScreen::Gamepad.title(presentation), "GAMEPAD");
        assert_eq!(
            MenuScreen::GamepadMenuControls.title(presentation),
            "PAD MENU"
        );
        assert_eq!(MenuScreen::Keyboard.title(presentation), "KEYBOARD");
        assert_eq!(
            MenuScreen::KeyboardMenuControls.title(presentation),
            "KB MENU"
        );
        assert_eq!(MenuScreen::Hotkeys.title(presentation), "HOTKEYS");
        assert_eq!(MenuScreen::System.title(presentation), "SYSTEM");

        let mut menu = OverlayMenuState::default();
        menu.open(presentation);
        assert_eq!(
            menu.apply_item_action(MenuItem::OpenRom, presentation),
            Some(MenuAction::OpenRom)
        );
        assert_eq!(
            menu.apply_item_action(MenuItem::RecentMenu, presentation),
            None
        );
        assert_eq!(
            menu.handle_input(MenuInput::Confirm, presentation),
            Some(MenuAction::OpenRecentRom(0))
        );
        assert_eq!(
            menu.apply_item_action(MenuItem::ClearRecentList, presentation),
            Some(MenuAction::ClearRecentList)
        );
        menu.open(presentation);
        assert_eq!(
            menu.apply_item_action(MenuItem::VideoMenu, presentation),
            None
        );
        assert_eq!(
            menu.apply_item_action(MenuItem::AudioMenu, presentation),
            None
        );
        assert_eq!(
            menu.apply_item_action(MenuItem::InputMenu, presentation),
            None
        );
        assert_eq!(
            menu.apply_item_action(MenuItem::ExtPortMenu, presentation),
            None
        );
        assert_eq!(menu.current_screen_state().screen, MenuScreen::ExtPort);
        menu.open(presentation);
        assert_eq!(
            menu.apply_item_action(MenuItem::SystemMenu, presentation),
            None
        );
        assert_eq!(
            menu.apply_item_action(MenuItem::SaveBattery, presentation),
            Some(MenuAction::SaveBattery)
        );
        assert_eq!(
            menu.apply_item_action(MenuItem::GamepadActive, presentation),
            None
        );
        assert_eq!(
            menu.apply_item_action(MenuItem::GamepadPreferred, presentation),
            Some(MenuAction::TogglePreferredGamepad)
        );
        assert_eq!(
            menu.apply_item_action(MenuItem::GamepadRumble, presentation),
            Some(MenuAction::CycleGamepadRumbleMode)
        );
        assert_eq!(
            menu.apply_item_action(MenuItem::Reset, presentation),
            Some(MenuAction::Reset)
        );
        assert_eq!(
            menu.apply_item_action(MenuItem::Quit, presentation),
            Some(MenuAction::Quit)
        );

        for (item, expected) in [
            (MenuItem::KeyboardUp, MenuItem::KeyboardUp),
            (MenuItem::KeyboardDown, MenuItem::KeyboardDown),
            (MenuItem::KeyboardLeft, MenuItem::KeyboardLeft),
            (MenuItem::KeyboardRight, MenuItem::KeyboardRight),
            (MenuItem::KeyboardA, MenuItem::KeyboardA),
            (MenuItem::KeyboardB, MenuItem::KeyboardB),
            (MenuItem::KeyboardSelect, MenuItem::KeyboardSelect),
            (MenuItem::KeyboardStart, MenuItem::KeyboardStart),
            (MenuItem::KeyboardMenuUp, MenuItem::KeyboardMenuUp),
            (MenuItem::KeyboardMenuDown, MenuItem::KeyboardMenuDown),
            (MenuItem::KeyboardMenuConfirm, MenuItem::KeyboardMenuConfirm),
            (MenuItem::KeyboardMenuCancel, MenuItem::KeyboardMenuCancel),
            (MenuItem::HotkeyPause, MenuItem::HotkeyPause),
            (MenuItem::HotkeyReset, MenuItem::HotkeyReset),
            (MenuItem::HotkeyFullscreen, MenuItem::HotkeyFullscreen),
            (
                MenuItem::HotkeyPerformanceHud,
                MenuItem::HotkeyPerformanceHud,
            ),
            (MenuItem::HotkeySaveBattery, MenuItem::HotkeySaveBattery),
            (MenuItem::GamepadUp, MenuItem::GamepadUp),
            (MenuItem::GamepadDown, MenuItem::GamepadDown),
            (MenuItem::GamepadLeft, MenuItem::GamepadLeft),
            (MenuItem::GamepadRight, MenuItem::GamepadRight),
            (MenuItem::GamepadA, MenuItem::GamepadA),
            (MenuItem::GamepadB, MenuItem::GamepadB),
            (MenuItem::GamepadSelect, MenuItem::GamepadSelect),
            (MenuItem::GamepadStart, MenuItem::GamepadStart),
            (MenuItem::GamepadMenuUp, MenuItem::GamepadMenuUp),
            (MenuItem::GamepadMenuDown, MenuItem::GamepadMenuDown),
            (MenuItem::GamepadMenuConfirm, MenuItem::GamepadMenuConfirm),
            (MenuItem::GamepadMenuCancel, MenuItem::GamepadMenuCancel),
        ] {
            menu.pending_binding_capture = None;
            assert_eq!(menu.apply_item_action(item, presentation), None);
            assert_eq!(menu.pending_binding_item(), Some(expected));
        }
    }

    #[test]
    fn performance_hud_renderer_draws_into_the_framebuffer() {
        let mut frame = vec![255_u8; 160 * 144 * 3];
        render_performance_hud(
            &mut frame,
            160,
            144,
            PerformanceHudSnapshot {
                fps: 59.8,
                speed_percent: 100.0,
                frame_time_ms: 16.7,
                emulation_time_ms: 11.0,
                render_time_ms: 2.0,
                pacing_time_ms: 3.0,
                audio_queue_ms: Some(18.0),
            },
        );

        assert!(
            frame.iter().any(|component| *component != 255),
            "HUD rendering should modify the destination framebuffer"
        );
    }
}
