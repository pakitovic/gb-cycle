use gb_core::{
    BootRomAssetError, BootRomAssets, BootRomKind, CompatibilityPolicy, ConsoleModel,
    ExecutionMode, MachineConfig, StartupMode,
};
use gb_persistence::{CartridgeSaveKey, CartridgeSaveKeyError};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const DEFAULT_BOOT_ROM_DIR: &str = ".roms/bootrom";
pub const DEFAULT_WINDOW_SCALE: u8 = 4;
pub const DEFAULT_AUDIO_SAMPLE_RATE_HZ: u32 = 48_000;
pub const DEFAULT_AUDIO_BUFFER_FRAMES: u16 = 512;
pub const DEFAULT_SAVE_FLUSH_DEBOUNCE: Duration = Duration::from_secs(2);
const DEFAULT_SAVE_SUBDIRECTORY: &str = "saves";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DesktopConfig {
    pub launch: LaunchOptions,
    pub boot_rom: BootRomOptions,
    pub saves: SaveOptions,
    pub video: VideoOptions,
    pub audio: AudioOptions,
    pub input: InputOptions,
}

impl DesktopConfig {
    pub fn machine_config(&self) -> Result<MachineConfig, DesktopConfigError> {
        let boot_rom_assets = self.boot_rom.load_assets(
            self.launch.startup_mode,
            self.launch.console_model.boot_rom_kind(),
        )?;

        Ok(
            MachineConfig::new(self.launch.console_model.console_model())
                .with_startup_mode(self.launch.startup_mode)
                .with_boot_rom_assets(boot_rom_assets)
                .with_compatibility(self.launch.compatibility_policy()),
        )
    }
}

#[derive(Debug)]
pub enum DesktopConfigError {
    BootRomAssets(BootRomAssetError),
    SaveKey(CartridgeSaveKeyError),
    SaveKeyDerivationEmpty { rom_path: PathBuf },
}

impl fmt::Display for DesktopConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BootRomAssets(error) => error.fmt(f),
            Self::SaveKey(error) => error.fmt(f),
            Self::SaveKeyDerivationEmpty { rom_path } => write!(
                f,
                "could not derive a save key from ROM path {}; use an explicit save key",
                rom_path.display()
            ),
        }
    }
}

impl std::error::Error for DesktopConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BootRomAssets(error) => Some(error),
            Self::SaveKey(error) => Some(error),
            Self::SaveKeyDerivationEmpty { .. } => None,
        }
    }
}

impl From<BootRomAssetError> for DesktopConfigError {
    fn from(value: BootRomAssetError) -> Self {
        Self::BootRomAssets(value)
    }
}

impl From<CartridgeSaveKeyError> for DesktopConfigError {
    fn from(value: CartridgeSaveKeyError) -> Self {
        Self::SaveKey(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DesktopConsoleModel {
    Dmg0,
    #[default]
    Dmg,
    Mgb,
}

impl DesktopConsoleModel {
    pub fn console_model(self) -> ConsoleModel {
        match self {
            Self::Dmg0 => ConsoleModel::Dmg0,
            Self::Dmg => ConsoleModel::Dmg,
            Self::Mgb => ConsoleModel::Mgb,
        }
    }

    pub fn boot_rom_kind(self) -> BootRomKind {
        match self {
            Self::Dmg0 => BootRomKind::Dmg0,
            Self::Dmg => BootRomKind::Dmg,
            Self::Mgb => BootRomKind::Mgb,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Dmg0 => "dmg0",
            Self::Dmg => "dmg",
            Self::Mgb => "mgb",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchOptions {
    pub console_model: DesktopConsoleModel,
    pub startup_mode: StartupMode,
    pub execution_mode: ExecutionMode,
}

impl LaunchOptions {
    pub fn compatibility_policy(&self) -> CompatibilityPolicy {
        match self.execution_mode {
            ExecutionMode::Strict => CompatibilityPolicy::strict(),
            ExecutionMode::Permissive => CompatibilityPolicy::permissive(),
            ExecutionMode::Experimental => CompatibilityPolicy::experimental(),
        }
    }
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            console_model: DesktopConsoleModel::Dmg,
            startup_mode: StartupMode::SkipBoot,
            execution_mode: ExecutionMode::Strict,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BootRomVerificationMode {
    Off,
    Warn,
    #[default]
    Strict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootRomOptions {
    pub search_path: Option<PathBuf>,
    pub verification: BootRomVerificationMode,
}

impl BootRomOptions {
    pub fn resolved_search_path(&self) -> PathBuf {
        self.search_path
            .clone()
            .unwrap_or_else(|| PathBuf::from(DEFAULT_BOOT_ROM_DIR))
    }

    pub fn load_assets(
        &self,
        startup_mode: StartupMode,
        boot_rom_kind: BootRomKind,
    ) -> Result<BootRomAssets, BootRomAssetError> {
        if !startup_mode.requires_boot_rom() {
            return Ok(BootRomAssets::none());
        }

        let path = self.resolved_search_path();
        if path.is_file() {
            let bytes = fs::read(&path).map_err(|source| BootRomAssetError::ReadFailed {
                path: path.clone(),
                source,
            })?;
            return BootRomAssets::none().with_bytes(boot_rom_kind, bytes);
        }

        BootRomAssets::from_directory(path)
    }
}

impl Default for BootRomOptions {
    fn default() -> Self {
        Self {
            search_path: None,
            verification: BootRomVerificationMode::Strict,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveOptions {
    pub enabled: bool,
    pub directory_policy: SaveDirectoryPolicy,
    pub key_policy: SaveKeyPolicy,
    pub flush_policy: DesktopSaveFlushPolicy,
}

impl SaveOptions {
    pub fn resolve_directory(&self, rom_path: &Path) -> Option<PathBuf> {
        if !self.enabled {
            return None;
        }

        Some(self.directory_policy.resolve(rom_path))
    }

    pub fn resolve_key(
        &self,
        rom_path: &Path,
    ) -> Result<Option<CartridgeSaveKey>, DesktopConfigError> {
        if !self.enabled {
            return Ok(None);
        }

        Ok(Some(self.key_policy.resolve(rom_path)?))
    }
}

impl Default for SaveOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            directory_policy: SaveDirectoryPolicy::default(),
            key_policy: SaveKeyPolicy::default(),
            flush_policy: DesktopSaveFlushPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DesktopSaveFlushPolicy {
    #[serde(rename = "manual")]
    Manual,
    #[serde(rename = "on-close")]
    OnClose,
    #[serde(rename = "on-write")]
    OnWrite,
    #[default]
    #[serde(rename = "debounced")]
    Debounced,
}

impl DesktopSaveFlushPolicy {
    pub fn flush_on_close(self) -> bool {
        !matches!(self, Self::Manual)
    }

    pub fn flush_each_frame_boundary(self) -> bool {
        matches!(self, Self::OnWrite | Self::Debounced)
    }

    pub fn debounce_window(self) -> Option<Duration> {
        matches!(self, Self::Debounced).then_some(DEFAULT_SAVE_FLUSH_DEBOUNCE)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SaveDirectoryPolicy {
    #[default]
    RomFolderSavesSubdir,
    Custom(PathBuf),
}

impl SaveDirectoryPolicy {
    pub fn resolve(&self, rom_path: &Path) -> PathBuf {
        match self {
            Self::RomFolderSavesSubdir => rom_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(DEFAULT_SAVE_SUBDIRECTORY),
            Self::Custom(path) => path.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SaveKeyPolicy {
    #[default]
    DerivedFromRomStem,
    Explicit(CartridgeSaveKey),
}

impl SaveKeyPolicy {
    pub fn resolve(&self, rom_path: &Path) -> Result<CartridgeSaveKey, DesktopConfigError> {
        match self {
            Self::DerivedFromRomStem => derive_save_key(rom_path),
            Self::Explicit(key) => Ok(key.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoOptions {
    pub window_scale: u8,
    pub integer_scale: bool,
    pub presentation_filter: bool,
    pub show_background: bool,
    pub show_window: bool,
    pub show_objects: bool,
    pub vsync: bool,
    pub fullscreen: bool,
    pub show_performance_hud: bool,
}

impl Default for VideoOptions {
    fn default() -> Self {
        Self {
            window_scale: DEFAULT_WINDOW_SCALE,
            integer_scale: true,
            presentation_filter: false,
            show_background: true,
            show_window: true,
            show_objects: true,
            vsync: true,
            fullscreen: false,
            show_performance_hud: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioOptions {
    pub enabled: bool,
    pub volume_percent: u8,
    pub output_sample_rate_hz: u32,
    pub buffer_frames: u16,
}

impl Default for AudioOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            volume_percent: 100,
            output_sample_rate_hz: DEFAULT_AUDIO_SAMPLE_RATE_HZ,
            buffer_frames: DEFAULT_AUDIO_BUFFER_FRAMES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct InputOptions {
    pub keyboard: KeyboardBindings,
    pub gamepad: GamepadOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyboardBindings {
    pub joypad: JoypadKeyboardBindings,
    pub menu: MenuKeyboardBindings,
    pub hotkeys: HotkeyBindings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct JoypadKeyboardBindings {
    pub up: DesktopKey,
    pub down: DesktopKey,
    pub left: DesktopKey,
    pub right: DesktopKey,
    pub a: DesktopKey,
    pub b: DesktopKey,
    pub select: DesktopKey,
    pub start: DesktopKey,
}

impl Default for JoypadKeyboardBindings {
    fn default() -> Self {
        Self {
            up: DesktopKey::ArrowUp,
            down: DesktopKey::ArrowDown,
            left: DesktopKey::ArrowLeft,
            right: DesktopKey::ArrowRight,
            a: DesktopKey::X,
            b: DesktopKey::Z,
            select: DesktopKey::Backspace,
            start: DesktopKey::Return,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MenuKeyboardBindings {
    pub up: DesktopKey,
    pub down: DesktopKey,
    pub confirm: DesktopKey,
    pub cancel: DesktopKey,
}

impl Default for MenuKeyboardBindings {
    fn default() -> Self {
        Self {
            up: DesktopKey::ArrowUp,
            down: DesktopKey::ArrowDown,
            confirm: DesktopKey::Return,
            cancel: DesktopKey::Escape,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeyBindings {
    pub pause: DesktopKey,
    pub reset: DesktopKey,
    pub toggle_fullscreen: DesktopKey,
    pub toggle_performance_hud: DesktopKey,
    pub save_battery: DesktopKey,
}

impl Default for HotkeyBindings {
    fn default() -> Self {
        Self {
            pause: DesktopKey::Space,
            reset: DesktopKey::R,
            toggle_fullscreen: DesktopKey::F11,
            toggle_performance_hud: DesktopKey::F10,
            save_battery: DesktopKey::F5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GamepadOptions {
    pub enabled: bool,
    pub directional_source: GamepadDirectionalSource,
    pub rumble_mode: GamepadRumbleMode,
    pub bindings: GamepadButtonBindings,
    pub menu: GamepadMenuBindings,
    pub preferred_device: PreferredGamepadIdentity,
}

impl Default for GamepadOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            directional_source: GamepadDirectionalSource::default(),
            rumble_mode: GamepadRumbleMode::default(),
            bindings: GamepadButtonBindings::default(),
            menu: GamepadMenuBindings::default(),
            preferred_device: PreferredGamepadIdentity::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GamepadButtonBindings {
    pub up: GamepadButtonBinding,
    pub down: GamepadButtonBinding,
    pub left: GamepadButtonBinding,
    pub right: GamepadButtonBinding,
    pub a: GamepadButtonBinding,
    pub b: GamepadButtonBinding,
    pub select: GamepadButtonBinding,
    pub start: GamepadButtonBinding,
}

impl GamepadButtonBindings {
    pub fn apply_face_layout(&mut self, layout: GamepadFaceLayout) {
        let (a, b) = layout.face_buttons();
        self.a = a;
        self.b = b;
    }
}

impl Default for GamepadButtonBindings {
    fn default() -> Self {
        Self {
            up: GamepadButtonBinding::DPadUp,
            down: GamepadButtonBinding::DPadDown,
            left: GamepadButtonBinding::DPadLeft,
            right: GamepadButtonBinding::DPadRight,
            a: GamepadButtonBinding::East,
            b: GamepadButtonBinding::South,
            select: GamepadButtonBinding::Back,
            start: GamepadButtonBinding::Start,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GamepadMenuBindings {
    pub up: GamepadButtonBinding,
    pub down: GamepadButtonBinding,
    pub confirm: GamepadButtonBinding,
    pub cancel: GamepadButtonBinding,
}

impl Default for GamepadMenuBindings {
    fn default() -> Self {
        Self {
            up: GamepadButtonBinding::DPadUp,
            down: GamepadButtonBinding::DPadDown,
            confirm: GamepadButtonBinding::South,
            cancel: GamepadButtonBinding::East,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GamepadButtonBinding {
    #[serde(rename = "south")]
    South,
    #[serde(rename = "east")]
    East,
    #[serde(rename = "west")]
    West,
    #[serde(rename = "north")]
    North,
    #[serde(rename = "back")]
    Back,
    #[serde(rename = "start")]
    Start,
    #[serde(rename = "guide")]
    Guide,
    #[serde(rename = "left-shoulder")]
    LeftShoulder,
    #[serde(rename = "right-shoulder")]
    RightShoulder,
    #[serde(rename = "left-stick-click")]
    LeftStickClick,
    #[serde(rename = "right-stick-click")]
    RightStickClick,
    #[serde(rename = "dpad-up")]
    DPadUp,
    #[serde(rename = "dpad-down")]
    DPadDown,
    #[serde(rename = "dpad-left")]
    DPadLeft,
    #[serde(rename = "dpad-right")]
    DPadRight,
    #[serde(rename = "misc1")]
    Misc1,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PreferredGamepadIdentity {
    pub path: Option<String>,
    pub name: Option<String>,
}

impl PreferredGamepadIdentity {
    pub fn is_configured(&self) -> bool {
        self.path.is_some() || self.name.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GamepadDirectionalSource {
    #[serde(rename = "dpad-only")]
    DpadOnly,
    #[serde(rename = "left-stick")]
    LeftStickOnly,
    #[default]
    #[serde(rename = "both")]
    DpadAndLeftStick,
}

impl GamepadDirectionalSource {
    pub fn uses_dpad(self) -> bool {
        matches!(self, Self::DpadOnly | Self::DpadAndLeftStick)
    }

    pub fn uses_left_stick(self) -> bool {
        matches!(self, Self::LeftStickOnly | Self::DpadAndLeftStick)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GamepadRumbleMode {
    #[serde(rename = "off")]
    Off,
    #[default]
    #[serde(rename = "strong")]
    Strong,
    #[serde(rename = "weak")]
    Weak,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GamepadFaceLayout {
    #[default]
    EastASouthB,
    SouthAEastB,
}

impl GamepadFaceLayout {
    pub fn face_buttons(self) -> (GamepadButtonBinding, GamepadButtonBinding) {
        match self {
            Self::EastASouthB => (GamepadButtonBinding::East, GamepadButtonBinding::South),
            Self::SouthAEastB => (GamepadButtonBinding::South, GamepadButtonBinding::East),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DesktopKey {
    Escape,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Backspace,
    Return,
    Space,
    R,
    X,
    Z,
    F5,
    F10,
    F11,
}

fn derive_save_key(rom_path: &Path) -> Result<CartridgeSaveKey, DesktopConfigError> {
    let stem = rom_path
        .file_stem()
        .or_else(|| rom_path.file_name())
        .ok_or_else(|| DesktopConfigError::SaveKeyDerivationEmpty {
            rom_path: rom_path.to_path_buf(),
        })?
        .to_string_lossy();

    let mut sanitized = String::new();
    let mut inserted_separator = false;
    for character in stem.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
            sanitized.push(character);
            inserted_separator = false;
        } else if !inserted_separator {
            sanitized.push('_');
            inserted_separator = true;
        }
    }

    let sanitized = sanitized.trim_matches('_').to_string();
    if sanitized.is_empty() {
        return Err(DesktopConfigError::SaveKeyDerivationEmpty {
            rom_path: rom_path.to_path_buf(),
        });
    }

    Ok(CartridgeSaveKey::new(sanitized)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::error::Error as _;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn default_desktop_config_matches_the_dmg_interactive_baseline() {
        let config = DesktopConfig::default();

        assert_eq!(config.launch.console_model, DesktopConsoleModel::Dmg);
        assert_eq!(config.launch.startup_mode, StartupMode::SkipBoot);
        assert_eq!(config.launch.execution_mode, ExecutionMode::Strict);
        assert!(config.saves.enabled);
        assert_eq!(config.saves.flush_policy, DesktopSaveFlushPolicy::Debounced);
        assert_eq!(config.video.window_scale, DEFAULT_WINDOW_SCALE);
        assert!(config.video.integer_scale);
        assert!(!config.video.presentation_filter);
        assert!(config.video.show_background);
        assert!(config.video.show_window);
        assert!(config.video.show_objects);
        assert!(config.video.vsync);
        assert!(config.video.show_performance_hud);
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
        assert_eq!(config.input.gamepad.rumble_mode, GamepadRumbleMode::Strong);
        assert_eq!(
            config.input.gamepad.bindings,
            GamepadButtonBindings::default()
        );
        assert_eq!(config.input.gamepad.menu, GamepadMenuBindings::default());
        assert_eq!(
            config.input.gamepad.preferred_device,
            PreferredGamepadIdentity::default()
        );
    }

    #[test]
    fn machine_config_uses_the_same_execution_mode_presets_as_other_frontends() {
        let mut config = DesktopConfig::default();
        config.launch.execution_mode = ExecutionMode::Permissive;

        let machine_config = config
            .machine_config()
            .expect("skip-boot should not load firmware");

        assert_eq!(machine_config.console_model, ConsoleModel::Dmg);
        assert_eq!(machine_config.startup_mode, StartupMode::SkipBoot);
        assert_eq!(
            machine_config.compatibility.execution_mode,
            ExecutionMode::Permissive
        );
        assert_eq!(
            machine_config.compatibility.validation_policy,
            gb_core::ValidationPolicy::Warn
        );
    }

    #[test]
    fn default_save_directory_lives_under_a_saves_subdirectory_next_to_the_rom() {
        let rom_path = Path::new("/tmp/roms/Tetris.gb");
        let save_options = SaveOptions::default();

        assert_eq!(
            save_options.resolve_directory(rom_path),
            Some(PathBuf::from("/tmp/roms/saves"))
        );
    }

    #[test]
    fn derived_save_key_sanitizes_the_rom_stem() {
        let rom_path = Path::new("/tmp/roms/Pokemon Red (World).gb");
        let save_key = SaveOptions::default()
            .resolve_key(rom_path)
            .expect("save-key derivation should succeed")
            .expect("saves are enabled by default");

        assert_eq!(save_key.as_str(), "Pokemon_Red_World");
    }

    #[test]
    fn keyboard_defaults_match_the_expected_handheld_layout() {
        let keyboard = KeyboardBindings::default();

        assert_eq!(keyboard.joypad.up, DesktopKey::ArrowUp);
        assert_eq!(keyboard.joypad.down, DesktopKey::ArrowDown);
        assert_eq!(keyboard.joypad.left, DesktopKey::ArrowLeft);
        assert_eq!(keyboard.joypad.right, DesktopKey::ArrowRight);
        assert_eq!(keyboard.joypad.a, DesktopKey::X);
        assert_eq!(keyboard.joypad.b, DesktopKey::Z);
        assert_eq!(keyboard.joypad.select, DesktopKey::Backspace);
        assert_eq!(keyboard.joypad.start, DesktopKey::Return);
        assert_eq!(keyboard.menu.up, DesktopKey::ArrowUp);
        assert_eq!(keyboard.menu.down, DesktopKey::ArrowDown);
        assert_eq!(keyboard.menu.confirm, DesktopKey::Return);
        assert_eq!(keyboard.menu.cancel, DesktopKey::Escape);
        assert_eq!(keyboard.hotkeys.pause, DesktopKey::Space);
        assert_eq!(keyboard.hotkeys.reset, DesktopKey::R);
        assert_eq!(keyboard.hotkeys.toggle_fullscreen, DesktopKey::F11);
        assert_eq!(keyboard.hotkeys.toggle_performance_hud, DesktopKey::F10);
        assert_eq!(keyboard.hotkeys.save_battery, DesktopKey::F5);
    }

    #[test]
    fn console_model_helpers_cover_all_supported_dmg_family_variants() {
        assert_eq!(
            DesktopConsoleModel::Dmg0.console_model(),
            ConsoleModel::Dmg0
        );
        assert_eq!(DesktopConsoleModel::Dmg.console_model(), ConsoleModel::Dmg);
        assert_eq!(DesktopConsoleModel::Mgb.console_model(), ConsoleModel::Mgb);
        assert_eq!(DesktopConsoleModel::Dmg0.boot_rom_kind(), BootRomKind::Dmg0);
        assert_eq!(DesktopConsoleModel::Dmg.boot_rom_kind(), BootRomKind::Dmg);
        assert_eq!(DesktopConsoleModel::Mgb.boot_rom_kind(), BootRomKind::Mgb);
        assert_eq!(DesktopConsoleModel::Dmg0.name(), "dmg0");
        assert_eq!(DesktopConsoleModel::Dmg.name(), "dmg");
        assert_eq!(DesktopConsoleModel::Mgb.name(), "mgb");
    }

    #[test]
    fn save_flush_policy_helpers_match_runtime_expectations() {
        assert!(!DesktopSaveFlushPolicy::Manual.flush_on_close());
        assert!(!DesktopSaveFlushPolicy::Manual.flush_each_frame_boundary());
        assert_eq!(DesktopSaveFlushPolicy::Manual.debounce_window(), None);

        assert!(DesktopSaveFlushPolicy::OnClose.flush_on_close());
        assert!(!DesktopSaveFlushPolicy::OnClose.flush_each_frame_boundary());

        assert!(DesktopSaveFlushPolicy::OnWrite.flush_on_close());
        assert!(DesktopSaveFlushPolicy::OnWrite.flush_each_frame_boundary());
        assert_eq!(DesktopSaveFlushPolicy::OnWrite.debounce_window(), None);

        assert!(DesktopSaveFlushPolicy::Debounced.flush_on_close());
        assert!(DesktopSaveFlushPolicy::Debounced.flush_each_frame_boundary());
        assert_eq!(
            DesktopSaveFlushPolicy::Debounced.debounce_window(),
            Some(DEFAULT_SAVE_FLUSH_DEBOUNCE)
        );
    }

    #[test]
    fn save_options_cover_disabled_custom_and_explicit_key_paths() {
        let rom_path = Path::new("/tmp/roms/Tetris.gb");
        let disabled = SaveOptions {
            enabled: false,
            ..SaveOptions::default()
        };
        assert_eq!(disabled.resolve_directory(rom_path), None);
        assert_eq!(
            disabled
                .resolve_key(rom_path)
                .expect("disabled saves should not error"),
            None
        );

        let explicit_key = CartridgeSaveKey::new("tetris".to_string())
            .expect("explicit save keys in tests should be valid");
        let custom = SaveOptions {
            directory_policy: SaveDirectoryPolicy::Custom(PathBuf::from("/tmp/custom-saves")),
            key_policy: SaveKeyPolicy::Explicit(explicit_key.clone()),
            ..SaveOptions::default()
        };
        assert_eq!(
            custom.resolve_directory(rom_path),
            Some(PathBuf::from("/tmp/custom-saves"))
        );
        assert_eq!(
            custom
                .resolve_key(rom_path)
                .expect("explicit save keys should resolve")
                .expect("saves are enabled"),
            explicit_key
        );
    }

    #[test]
    fn input_helpers_cover_face_layout_preferred_gamepads_and_direction_sources() {
        let mut bindings = GamepadButtonBindings::default();
        bindings.apply_face_layout(GamepadFaceLayout::SouthAEastB);
        assert_eq!(bindings.a, GamepadButtonBinding::South);
        assert_eq!(bindings.b, GamepadButtonBinding::East);

        assert!(!PreferredGamepadIdentity::default().is_configured());
        assert!(
            PreferredGamepadIdentity {
                path: Some("/dev/input/js0".to_string()),
                name: None,
            }
            .is_configured()
        );

        assert!(GamepadDirectionalSource::DpadOnly.uses_dpad());
        assert!(!GamepadDirectionalSource::DpadOnly.uses_left_stick());
        assert!(!GamepadDirectionalSource::LeftStickOnly.uses_dpad());
        assert!(GamepadDirectionalSource::LeftStickOnly.uses_left_stick());
        assert!(GamepadDirectionalSource::DpadAndLeftStick.uses_dpad());
        assert!(GamepadDirectionalSource::DpadAndLeftStick.uses_left_stick());
        assert_eq!(GamepadRumbleMode::default(), GamepadRumbleMode::Strong);
    }

    #[test]
    fn boot_rom_options_cover_default_custom_and_skip_boot_loading() {
        let mut options = BootRomOptions::default();
        assert_eq!(
            options.resolved_search_path(),
            PathBuf::from(DEFAULT_BOOT_ROM_DIR)
        );

        options.search_path = Some(PathBuf::from("/tmp/firmware"));
        assert_eq!(
            options.resolved_search_path(),
            PathBuf::from("/tmp/firmware")
        );

        assert!(
            options
                .load_assets(StartupMode::SkipBoot, BootRomKind::Dmg)
                .expect("skip-boot should not attempt to read firmware")
                .is_empty()
        );
    }

    #[test]
    fn boot_rom_options_can_load_a_single_exact_image_file() {
        let root = temp_root("boot-options");
        let image_path = root.join("dmg_boot.bin");
        fs::write(&image_path, vec![0x11; 0x100]).expect("test boot ROM image should be writable");

        let options = BootRomOptions {
            search_path: Some(image_path.clone()),
            verification: BootRomVerificationMode::Off,
        };
        let assets = options
            .load_assets(StartupMode::RealBoot, BootRomKind::Dmg)
            .expect("exact boot ROM file should load");
        assert_eq!(assets.read_byte(BootRomKind::Dmg, 0), Some(0x11));

        fs::remove_dir_all(root).expect("temp boot ROM root should be removable");
    }

    #[test]
    fn derive_save_key_rejects_paths_without_any_usable_key_material() {
        let error = derive_save_key(Path::new("/tmp/roms/..."))
            .expect_err("all-punctuation stems should be rejected");
        assert!(matches!(
            error,
            DesktopConfigError::SaveKeyDerivationEmpty { .. }
        ));
    }

    #[test]
    fn desktop_config_error_helpers_cover_conversion_display_and_sources() {
        let boot_error = DesktopConfigError::from(BootRomAssetError::DirectoryNotFound {
            path: PathBuf::from("/tmp/missing-bootrom"),
        });
        assert!(boot_error.to_string().contains("/tmp/missing-bootrom"));
        assert!(boot_error.source().is_some());

        let save_key_error = CartridgeSaveKey::new("contains spaces".to_string())
            .expect_err("invalid save keys should fail validation");
        let save_error = DesktopConfigError::from(save_key_error);
        assert!(save_error.source().is_some());
        assert!(!save_error.to_string().is_empty());

        let derived = DesktopConfigError::SaveKeyDerivationEmpty {
            rom_path: PathBuf::from("/tmp/roms/..."),
        };
        assert!(derived.to_string().contains("explicit save key"));
        assert!(derived.source().is_none());
    }

    #[test]
    fn launch_option_helpers_cover_strict_and_experimental_compatibility_modes() {
        let strict = LaunchOptions {
            execution_mode: ExecutionMode::Strict,
            ..LaunchOptions::default()
        };
        assert_eq!(strict.compatibility_policy(), CompatibilityPolicy::strict());

        let experimental = LaunchOptions {
            execution_mode: ExecutionMode::Experimental,
            ..LaunchOptions::default()
        };
        assert_eq!(
            experimental.compatibility_policy(),
            CompatibilityPolicy::experimental()
        );
    }

    fn temp_root(label: &str) -> PathBuf {
        let id = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!(
            "gb-cycle-config-tests-{label}-{}-{id}",
            std::process::id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("stale config temp root should be removable");
        }
        fs::create_dir_all(&root).expect("config temp root should be creatable");
        root
    }
}
