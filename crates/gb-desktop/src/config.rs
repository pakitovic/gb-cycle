use gb_core::{
    BootRomAssetError, BootRomAssets, CompatibilityPolicy, ConsoleModel, ExecutionMode,
    HardwareRevision, MachineConfig, MachineRewindConfig, MachineRewindSubframeCadence,
    StartupMode,
};
use gb_persistence::{CartridgeSaveKey, CartridgeSaveKeyError};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const DEFAULT_WINDOW_SCALE: u8 = 4;
pub const DEFAULT_AUDIO_SAMPLE_RATE_HZ: u32 = 48_000;
pub const DEFAULT_AUDIO_BUFFER_FRAMES: u16 = 512;
pub const DEFAULT_SAVE_FLUSH_DEBOUNCE: Duration = Duration::from_secs(2);
pub const DEFAULT_REWIND_HISTORY_SECONDS: u16 = 10;
pub const DEFAULT_REWIND_SUBFRAMES_PER_FRAME: u8 = 1;
pub const DEFAULT_REWIND_MAX_MEMORY_MIB: u16 = 256;
pub const DEFAULT_REWIND_SPEED_MULTIPLIER: u8 = 2;
pub const DEFAULT_FAST_FORWARD_SPEED_MULTIPLIER: u8 = 4;
pub const FAST_FORWARD_SPEED_MULTIPLIER_OPTIONS: [u8; 3] = [4, 8, 16];
const DEFAULT_SAVE_SUBDIRECTORY: &str = "saves";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DesktopConfig {
    pub launch: LaunchOptions,
    pub boot_rom: BootRomOptions,
    pub saves: SaveOptions,
    pub machine_state: MachineStateOptions,
    pub rewind: RewindOptions,
    pub fast_forward: FastForwardOptions,
    pub video: VideoOptions,
    pub audio: AudioOptions,
    pub input: InputOptions,
}

impl DesktopConfig {
    pub fn machine_config(&self) -> Result<MachineConfig, DesktopConfigError> {
        let revision = self.launch.effective_revision();
        let boot_rom_assets = self
            .boot_rom
            .load_assets(self.launch.startup_mode, revision)?;

        Ok(
            MachineConfig::new(self.launch.console_model.console_model())
                .with_startup_mode(self.launch.startup_mode)
                .with_revision(revision)
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
    #[default]
    GameBoy,
    GameBoyPocket,
    GameBoyLight,
    GameBoyColor,
}

impl DesktopConsoleModel {
    pub fn console_model(self) -> ConsoleModel {
        match self {
            Self::GameBoy => ConsoleModel::GameBoy,
            Self::GameBoyPocket => ConsoleModel::GameBoyPocket,
            Self::GameBoyLight => ConsoleModel::GameBoyLight,
            Self::GameBoyColor => ConsoleModel::GameBoyColor,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::GameBoy => "DMG",
            Self::GameBoyPocket => "MGB",
            Self::GameBoyLight => "LGB",
            Self::GameBoyColor => "CGB",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DesktopDisplayPalette {
    Grey,
    #[default]
    GameBoy,
    Pocket,
    Light,
}

impl DesktopDisplayPalette {
    pub fn default_for_console_model(console_model: DesktopConsoleModel) -> Self {
        match console_model {
            DesktopConsoleModel::GameBoy => Self::GameBoy,
            DesktopConsoleModel::GameBoyPocket => Self::Pocket,
            DesktopConsoleModel::GameBoyLight => Self::Light,
            DesktopConsoleModel::GameBoyColor => Self::Grey,
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Grey => Self::GameBoy,
            Self::GameBoy => Self::Pocket,
            Self::Pocket => Self::Light,
            Self::Light => Self::Grey,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DesktopFrameBlendingMode {
    #[default]
    Off,
    On,
}

impl DesktopFrameBlendingMode {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::On,
            Self::On => Self::Off,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchOptions {
    pub console_model: DesktopConsoleModel,
    pub revision: HardwareRevision,
    pub startup_mode: StartupMode,
    pub execution_mode: ExecutionMode,
}

impl LaunchOptions {
    pub fn effective_revision(&self) -> HardwareRevision {
        if self
            .console_model
            .console_model()
            .supports_revision(self.revision)
        {
            self.revision
        } else {
            self.console_model.console_model().default_revision()
        }
    }

    pub fn normalize_revision_for_model(&mut self) {
        self.revision = self.effective_revision();
    }

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
            console_model: DesktopConsoleModel::GameBoy,
            revision: DesktopConsoleModel::GameBoy
                .console_model()
                .default_revision(),
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
    pub fn resolved_search_path(&self) -> Option<PathBuf> {
        self.search_path.clone()
    }

    pub fn load_assets(
        &self,
        startup_mode: StartupMode,
        revision: HardwareRevision,
    ) -> Result<BootRomAssets, BootRomAssetError> {
        if !startup_mode.requires_boot_rom() {
            return Ok(BootRomAssets::none());
        }

        let Some(path) = self.resolved_search_path() else {
            return Ok(BootRomAssets::none());
        };
        if path.is_file() {
            let bytes = fs::read(&path).map_err(|source| BootRomAssetError::ReadFailed {
                path: path.clone(),
                source,
            })?;
            return BootRomAssets::none().with_bytes(revision, bytes);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MachineStateOptions {
    pub autoload_slot: Option<u8>,
}

impl MachineStateOptions {
    pub fn normalized_autoload_slot(self, max_slot: u8) -> Option<u8> {
        self.autoload_slot
            .filter(|slot| (1..=max_slot.max(1)).contains(slot))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RewindOptions {
    pub enabled: bool,
    pub history_seconds: u16,
    pub subframes_per_frame: u8,
    pub max_memory_mib: u16,
    pub speed_multiplier: u8,
}

impl RewindOptions {
    pub fn machine_rewind_config(self) -> MachineRewindConfig {
        MachineRewindConfig::default()
            .with_target_history_t_cycles(
                u64::from(self.history_seconds.max(1))
                    .saturating_mul(gb_core::DMG_T_CYCLES_PER_SECOND),
            )
            .with_max_estimated_bytes(
                usize::from(self.max_memory_mib.max(1)).saturating_mul(1024 * 1024),
            )
            .with_subframe_cadence(if self.subframes_per_frame == 0 {
                MachineRewindSubframeCadence::Disabled
            } else {
                MachineRewindSubframeCadence::FixedPerFrame {
                    captures_per_frame: u16::from(self.subframes_per_frame),
                }
            })
    }
}

impl Default for RewindOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            history_seconds: DEFAULT_REWIND_HISTORY_SECONDS,
            subframes_per_frame: DEFAULT_REWIND_SUBFRAMES_PER_FRAME,
            max_memory_mib: DEFAULT_REWIND_MAX_MEMORY_MIB,
            speed_multiplier: DEFAULT_REWIND_SPEED_MULTIPLIER,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FastForwardOptions {
    pub enabled: bool,
    pub speed_multiplier: u8,
}

impl FastForwardOptions {
    pub fn display_speed_multiplier(self) -> u8 {
        fast_forward_display_speed_multiplier(self.speed_multiplier)
    }
}

impl Default for FastForwardOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            speed_multiplier: DEFAULT_FAST_FORWARD_SPEED_MULTIPLIER,
        }
    }
}

pub fn fast_forward_display_speed_multiplier(speed_multiplier: u8) -> u8 {
    (speed_multiplier / 2).max(1)
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
    pub frame_blending: DesktopFrameBlendingMode,
    pub display_palette: DesktopDisplayPalette,
    pub show_background: bool,
    pub show_window: bool,
    pub show_objects: bool,
    pub vsync: bool,
    pub fullscreen: bool,
    pub show_performance_hud: bool,
    pub show_cgb_infrared_helper: bool,
}

impl VideoOptions {
    pub fn default_for_console_model(console_model: DesktopConsoleModel) -> Self {
        Self {
            display_palette: DesktopDisplayPalette::default_for_console_model(console_model),
            ..Self::default()
        }
    }
}

impl Default for VideoOptions {
    fn default() -> Self {
        Self {
            window_scale: DEFAULT_WINDOW_SCALE,
            integer_scale: true,
            presentation_filter: false,
            frame_blending: DesktopFrameBlendingMode::Off,
            display_palette: DesktopDisplayPalette::default(),
            show_background: true,
            show_window: true,
            show_objects: true,
            vsync: true,
            fullscreen: false,
            show_performance_hud: false,
            show_cgb_infrared_helper: false,
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
            a: DesktopKey::LeftGui,
            b: DesktopKey::LeftAlt,
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
            confirm: DesktopKey::LeftGui,
            cancel: DesktopKey::LeftAlt,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeyBindings {
    pub pause: DesktopKey,
    pub save_state: DesktopKey,
    pub load_state: DesktopKey,
    pub state_slot_1: DesktopKey,
    pub state_slot_2: DesktopKey,
    pub state_slot_3: DesktopKey,
    pub state_slot_4: DesktopKey,
    pub reset: DesktopKey,
    pub rewind: DesktopKey,
    pub fast_forward: DesktopKey,
    pub toggle_fullscreen: DesktopKey,
    pub toggle_performance_hud: DesktopKey,
    pub save_battery: DesktopKey,
}

impl Default for HotkeyBindings {
    fn default() -> Self {
        Self {
            pause: DesktopKey::Space,
            save_state: DesktopKey::F1,
            load_state: DesktopKey::F2,
            state_slot_1: DesktopKey::Digit1,
            state_slot_2: DesktopKey::Digit2,
            state_slot_3: DesktopKey::Digit3,
            state_slot_4: DesktopKey::Digit4,
            reset: DesktopKey::F12,
            rewind: DesktopKey::LeftShift,
            fast_forward: DesktopKey::RightShift,
            toggle_fullscreen: DesktopKey::F11,
            toggle_performance_hud: DesktopKey::F10,
            save_battery: DesktopKey::F9,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GamepadOptions {
    pub enabled: bool,
    pub directional_source: GamepadDirectionalSource,
    pub gyro_mode: GamepadGyroMode,
    pub rumble_mode: GamepadRumbleMode,
    pub bindings: GamepadButtonBindings,
    pub actions: GamepadActionBindings,
    pub menu: GamepadMenuBindings,
    pub preferred_device: PreferredGamepadIdentity,
}

impl Default for GamepadOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            directional_source: GamepadDirectionalSource::default(),
            gyro_mode: GamepadGyroMode::default(),
            rumble_mode: GamepadRumbleMode::default(),
            bindings: GamepadButtonBindings::default(),
            actions: GamepadActionBindings::default(),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GamepadActionBindings {
    pub save_state: Option<GamepadButtonBinding>,
    pub load_state: Option<GamepadButtonBinding>,
    pub rewind: Option<GamepadButtonBinding>,
    pub fast_forward: Option<GamepadButtonBinding>,
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
    #[serde(rename = "left-trigger")]
    LeftTrigger,
    #[serde(rename = "right-trigger")]
    RightTrigger,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GamepadGyroMode {
    #[default]
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "pad-gyro")]
    PadGyro,
    #[serde(rename = "pad-input")]
    PadInput,
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
    Tab,
    Backspace,
    Return,
    Space,
    R,
    X,
    Z,
    #[serde(rename = "1", alias = "digit-1", alias = "key-1")]
    Digit1,
    #[serde(rename = "2", alias = "digit-2", alias = "key-2")]
    Digit2,
    #[serde(rename = "3", alias = "digit-3", alias = "key-3")]
    Digit3,
    #[serde(rename = "4", alias = "digit-4", alias = "key-4")]
    Digit4,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    LeftShift,
    RightShift,
    LeftControl,
    RightControl,
    #[serde(alias = "left-option")]
    LeftAlt,
    #[serde(alias = "right-option")]
    RightAlt,
    #[serde(alias = "left-command", alias = "left-super", alias = "left-windows")]
    LeftGui,
    #[serde(
        alias = "right-command",
        alias = "right-super",
        alias = "right-windows"
    )]
    RightGui,
}

fn derive_save_key(rom_path: &Path) -> Result<CartridgeSaveKey, DesktopConfigError> {
    let stem = rom_path
        .file_stem()
        .or_else(|| rom_path.file_name())
        .ok_or_else(|| DesktopConfigError::SaveKeyDerivationEmpty {
            rom_path: rom_path.to_path_buf(),
        })?
        .to_string_lossy()
        .into_owned();

    Ok(CartridgeSaveKey::new(stem)?)
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

    #[test]
    fn machine_config_uses_the_same_execution_mode_presets_as_other_frontends() {
        let mut config = DesktopConfig::default();
        config.launch.execution_mode = ExecutionMode::Permissive;

        let machine_config = config
            .machine_config()
            .expect("skip-boot should not load firmware");

        assert_eq!(machine_config.console_model, ConsoleModel::GameBoy);
        assert_eq!(machine_config.revision, HardwareRevision::default());
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
    fn machine_config_applies_revision_for_selected_model() {
        let mut config = DesktopConfig::default();
        config.launch.console_model = DesktopConsoleModel::GameBoyColor;
        config.launch.revision = HardwareRevision::CpuCgbE;

        let cgb_config = config
            .machine_config()
            .expect("skip-boot should not load firmware");
        assert_eq!(cgb_config.console_model, ConsoleModel::GameBoyColor);
        assert_eq!(cgb_config.revision, HardwareRevision::CpuCgbE);

        config.launch.console_model = DesktopConsoleModel::GameBoy;
        let dmg_config = config
            .machine_config()
            .expect("skip-boot should not load firmware");
        assert_eq!(dmg_config.console_model, ConsoleModel::GameBoy);
        assert_eq!(dmg_config.revision, HardwareRevision::DmgCpuC);
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
    fn derived_save_key_preserves_the_rom_stem() {
        let rom_path =
            Path::new("/tmp/roms/Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).gb");
        let save_key = SaveOptions::default()
            .resolve_key(rom_path)
            .expect("save-key derivation should succeed")
            .expect("saves are enabled by default");

        assert_eq!(
            save_key.as_str(),
            "Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2)"
        );
    }

    #[test]
    fn keyboard_defaults_match_the_expected_handheld_layout() {
        let keyboard = KeyboardBindings::default();

        assert_eq!(keyboard.joypad.up, DesktopKey::ArrowUp);
        assert_eq!(keyboard.joypad.down, DesktopKey::ArrowDown);
        assert_eq!(keyboard.joypad.left, DesktopKey::ArrowLeft);
        assert_eq!(keyboard.joypad.right, DesktopKey::ArrowRight);
        assert_eq!(keyboard.joypad.a, DesktopKey::LeftGui);
        assert_eq!(keyboard.joypad.b, DesktopKey::LeftAlt);
        assert_eq!(keyboard.joypad.select, DesktopKey::Backspace);
        assert_eq!(keyboard.joypad.start, DesktopKey::Return);
        assert_eq!(keyboard.menu.up, DesktopKey::ArrowUp);
        assert_eq!(keyboard.menu.down, DesktopKey::ArrowDown);
        assert_eq!(keyboard.menu.confirm, DesktopKey::LeftGui);
        assert_eq!(keyboard.menu.cancel, DesktopKey::LeftAlt);
        assert_eq!(keyboard.hotkeys.pause, DesktopKey::Space);
        assert_eq!(keyboard.hotkeys.save_state, DesktopKey::F1);
        assert_eq!(keyboard.hotkeys.load_state, DesktopKey::F2);
        assert_eq!(keyboard.hotkeys.state_slot_1, DesktopKey::Digit1);
        assert_eq!(keyboard.hotkeys.state_slot_2, DesktopKey::Digit2);
        assert_eq!(keyboard.hotkeys.state_slot_3, DesktopKey::Digit3);
        assert_eq!(keyboard.hotkeys.state_slot_4, DesktopKey::Digit4);
        assert_eq!(keyboard.hotkeys.reset, DesktopKey::F12);
        assert_eq!(keyboard.hotkeys.rewind, DesktopKey::LeftShift);
        assert_eq!(keyboard.hotkeys.fast_forward, DesktopKey::RightShift);
        assert_eq!(keyboard.hotkeys.toggle_fullscreen, DesktopKey::F11);
        assert_eq!(keyboard.hotkeys.toggle_performance_hud, DesktopKey::F10);
        assert_eq!(keyboard.hotkeys.save_battery, DesktopKey::F9);
    }

    #[test]
    fn desktop_key_deserialization_accepts_platform_aliases() {
        #[derive(serde::Deserialize)]
        struct KeyWrapper {
            key: DesktopKey,
        }

        let cases = [
            ("1", DesktopKey::Digit1),
            ("digit-2", DesktopKey::Digit2),
            ("key-3", DesktopKey::Digit3),
            ("f1", DesktopKey::F1),
            ("f2", DesktopKey::F2),
            ("f3", DesktopKey::F3),
            ("f4", DesktopKey::F4),
            ("f5", DesktopKey::F5),
            ("f6", DesktopKey::F6),
            ("f7", DesktopKey::F7),
            ("f8", DesktopKey::F8),
            ("f9", DesktopKey::F9),
            ("f12", DesktopKey::F12),
            ("left-alt", DesktopKey::LeftAlt),
            ("left-option", DesktopKey::LeftAlt),
            ("right-option", DesktopKey::RightAlt),
            ("left-gui", DesktopKey::LeftGui),
            ("left-command", DesktopKey::LeftGui),
            ("right-command", DesktopKey::RightGui),
            ("left-super", DesktopKey::LeftGui),
            ("right-windows", DesktopKey::RightGui),
            ("left-shift", DesktopKey::LeftShift),
            ("right-control", DesktopKey::RightControl),
            ("tab", DesktopKey::Tab),
        ];

        for (serialized, expected) in cases {
            let decoded: KeyWrapper = toml::from_str(&format!("key = \"{serialized}\""))
                .expect("desktop key alias should deserialize");
            assert_eq!(decoded.key, expected);
        }
    }

    #[test]
    fn console_model_helpers_cover_all_visible_product_models() {
        assert_eq!(
            DesktopConsoleModel::GameBoy.console_model(),
            ConsoleModel::GameBoy
        );
        assert_eq!(
            DesktopConsoleModel::GameBoyPocket.console_model(),
            ConsoleModel::GameBoyPocket
        );
        assert_eq!(
            DesktopConsoleModel::GameBoyLight.console_model(),
            ConsoleModel::GameBoyLight
        );
        assert_eq!(
            DesktopConsoleModel::GameBoyColor.console_model(),
            ConsoleModel::GameBoyColor
        );
        assert_eq!(DesktopConsoleModel::GameBoy.name(), "DMG");
        assert_eq!(DesktopConsoleModel::GameBoyPocket.name(), "MGB");
        assert_eq!(DesktopConsoleModel::GameBoyLight.name(), "LGB");
        assert_eq!(DesktopConsoleModel::GameBoyColor.name(), "CGB");
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
            DesktopDisplayPalette::Grey.next(),
            DesktopDisplayPalette::GameBoy
        );
        assert_eq!(
            DesktopDisplayPalette::GameBoy.next(),
            DesktopDisplayPalette::Pocket
        );
        assert_eq!(
            DesktopDisplayPalette::Pocket.next(),
            DesktopDisplayPalette::Light
        );
        assert_eq!(
            DesktopDisplayPalette::Light.next(),
            DesktopDisplayPalette::Grey
        );
        assert_eq!(
            DesktopFrameBlendingMode::Off.next(),
            DesktopFrameBlendingMode::On
        );
        assert_eq!(
            DesktopFrameBlendingMode::On.next(),
            DesktopFrameBlendingMode::Off
        );
        assert_eq!(
            VideoOptions::default_for_console_model(DesktopConsoleModel::GameBoyColor)
                .display_palette,
            DesktopDisplayPalette::Grey
        );
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
        assert_eq!(GamepadGyroMode::default(), GamepadGyroMode::Off);
        assert_eq!(GamepadRumbleMode::default(), GamepadRumbleMode::Strong);
    }

    #[test]
    fn gamepad_gyro_mode_serializes_as_stable_kebab_case_values() {
        #[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
        struct GyroModeWrapper {
            gyro_mode: GamepadGyroMode,
        }

        for (mode, serialized) in [
            (GamepadGyroMode::Off, "off"),
            (GamepadGyroMode::PadGyro, "pad-gyro"),
            (GamepadGyroMode::PadInput, "pad-input"),
        ] {
            let wrapper = GyroModeWrapper { gyro_mode: mode };
            let encoded = toml::to_string(&wrapper).expect("gyro mode should serialize");
            assert_eq!(encoded.trim(), format!("gyro_mode = \"{serialized}\""));
            let decoded: GyroModeWrapper =
                toml::from_str(&encoded).expect("gyro mode should deserialize");
            assert_eq!(decoded, wrapper);
        }
    }

    #[test]
    fn boot_rom_options_cover_default_custom_and_skip_boot_loading() {
        let mut options = BootRomOptions::default();
        assert_eq!(options.resolved_search_path(), None);

        options.search_path = Some(PathBuf::from("/tmp/firmware"));
        assert_eq!(
            options.resolved_search_path(),
            Some(PathBuf::from("/tmp/firmware"))
        );

        assert!(
            options
                .load_assets(StartupMode::SkipBoot, HardwareRevision::DmgCpuC)
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
            .load_assets(StartupMode::RealBoot, HardwareRevision::DmgCpuC)
            .expect("exact boot ROM file should load");
        assert_eq!(assets.read_byte(HardwareRevision::DmgCpuC, 0), Some(0x11));

        fs::remove_dir_all(root).expect("temp boot ROM root should be removable");
    }

    #[test]
    fn derive_save_key_rejects_paths_without_file_names() {
        let error = derive_save_key(Path::new("/"))
            .expect_err("paths without a file name should be rejected");
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

        let save_key_error = CartridgeSaveKey::new("bad/key".to_string())
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
        let mut cgb = LaunchOptions {
            console_model: DesktopConsoleModel::GameBoyColor,
            revision: HardwareRevision::CpuCgbE,
            ..LaunchOptions::default()
        };
        assert_eq!(cgb.effective_revision(), HardwareRevision::CpuCgbE);
        cgb.console_model = DesktopConsoleModel::GameBoy;
        cgb.normalize_revision_for_model();
        assert_eq!(cgb.revision, HardwareRevision::default());

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
