use gb_core::{
    BootRomAssetError, BootRomAssetKind, BootRomAssets, CompatibilityPolicy, ConsoleModel,
    ExecutionMode, HardwareRevision, HostPlatform, MachineConfig, MachineRewindConfig,
    MachineRewindSubframeCadence, SgbHostProfile, SgbVideoStandard, StartupMode,
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
    pub fn machine_config_without_boot_rom_assets(&self) -> MachineConfig {
        self.launch.machine_config_without_boot_rom_assets()
    }

    pub fn machine_config(&self) -> Result<MachineConfig, DesktopConfigError> {
        let machine_config = self.machine_config_without_boot_rom_assets();
        let boot_rom_assets = self.boot_rom.load_assets(
            self.launch.startup_mode,
            machine_config.boot_rom_asset_kind(),
        )?;

        Ok(machine_config.with_boot_rom_assets(boot_rom_assets))
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
    GameBoyAdvance,
    SuperGameBoy,
    SuperGameBoy2,
}

impl DesktopConsoleModel {
    pub fn console_model(self) -> ConsoleModel {
        match self {
            Self::GameBoy => ConsoleModel::GameBoy,
            Self::GameBoyPocket => ConsoleModel::GameBoyPocket,
            Self::GameBoyLight => ConsoleModel::GameBoyLight,
            Self::GameBoyColor => ConsoleModel::GameBoyColor,
            Self::GameBoyAdvance => ConsoleModel::GameBoyAdvance,
            Self::SuperGameBoy | Self::SuperGameBoy2 => ConsoleModel::GameBoy,
        }
    }

    pub const fn host_platform(self) -> HostPlatform {
        match self {
            Self::SuperGameBoy => HostPlatform::Sgb,
            Self::SuperGameBoy2 => HostPlatform::Sgb2,
            Self::GameBoy
            | Self::GameBoyPocket
            | Self::GameBoyLight
            | Self::GameBoyColor
            | Self::GameBoyAdvance => HostPlatform::Handheld,
        }
    }

    pub fn active_revisions(self) -> &'static [HardwareRevision] {
        self.console_model()
            .active_revisions_on_host(self.host_platform())
    }

    pub const fn sgb_profile(self) -> Option<SgbHostProfile> {
        match self {
            Self::SuperGameBoy => Some(SgbHostProfile::SgbNtsc),
            Self::SuperGameBoy2 => Some(SgbHostProfile::Sgb2Ntsc),
            Self::GameBoy
            | Self::GameBoyPocket
            | Self::GameBoyLight
            | Self::GameBoyColor
            | Self::GameBoyAdvance => None,
        }
    }

    pub const fn sgb_profile_for_standard(
        self,
        video_standard: SgbVideoStandard,
    ) -> Option<SgbHostProfile> {
        match self {
            Self::SuperGameBoy => match video_standard {
                SgbVideoStandard::Ntsc => Some(SgbHostProfile::SgbNtsc),
                SgbVideoStandard::Pal => Some(SgbHostProfile::SgbPal),
            },
            Self::SuperGameBoy2 => Some(SgbHostProfile::Sgb2Ntsc),
            Self::GameBoy
            | Self::GameBoyPocket
            | Self::GameBoyLight
            | Self::GameBoyColor
            | Self::GameBoyAdvance => None,
        }
    }

    pub const fn allows_sgb_video_standard_selection(self) -> bool {
        matches!(self, Self::SuperGameBoy)
    }

    pub const fn uses_rgb555_output(self) -> bool {
        matches!(
            self,
            Self::GameBoyColor | Self::GameBoyAdvance | Self::SuperGameBoy | Self::SuperGameBoy2
        )
    }

    pub const fn allows_display_palette(self) -> bool {
        !self.uses_rgb555_output()
    }

    pub const fn allows_ext_port_menu(self) -> bool {
        !matches!(self, Self::SuperGameBoy)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::GameBoy => "DMG",
            Self::GameBoyPocket => "MGB",
            Self::GameBoyLight => "LGB",
            Self::GameBoyColor => "CGB",
            Self::GameBoyAdvance => "AGB",
            Self::SuperGameBoy => "SGB",
            Self::SuperGameBoy2 => "SGB2",
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
            DesktopConsoleModel::GameBoyColor
            | DesktopConsoleModel::GameBoyAdvance
            | DesktopConsoleModel::SuperGameBoy
            | DesktopConsoleModel::SuperGameBoy2 => Self::Grey,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SgbBorderPresentationMode {
    #[default]
    Auto,
    Off,
}

impl SgbBorderPresentationMode {
    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::Off,
            Self::Off => Self::Auto,
        }
    }

    pub const fn is_auto(self) -> bool {
        matches!(self, Self::Auto)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchOptions {
    pub console_model: DesktopConsoleModel,
    pub revision: HardwareRevision,
    pub sgb_video_standard: SgbVideoStandard,
    pub startup_mode: StartupMode,
    pub execution_mode: ExecutionMode,
}

impl LaunchOptions {
    pub fn effective_revision(&self) -> HardwareRevision {
        if self
            .console_model
            .console_model()
            .supports_revision_on_host(self.console_model.host_platform(), self.revision)
        {
            self.revision
        } else {
            self.console_model.console_model().default_revision()
        }
    }

    pub fn normalize_revision_for_model(&mut self) {
        self.revision = self.effective_revision();
    }

    pub fn effective_sgb_video_standard(&self) -> SgbVideoStandard {
        match self.console_model {
            DesktopConsoleModel::SuperGameBoy => self.sgb_video_standard,
            DesktopConsoleModel::SuperGameBoy2 => SgbVideoStandard::Ntsc,
            DesktopConsoleModel::GameBoy
            | DesktopConsoleModel::GameBoyPocket
            | DesktopConsoleModel::GameBoyLight
            | DesktopConsoleModel::GameBoyColor
            | DesktopConsoleModel::GameBoyAdvance => self.sgb_video_standard,
        }
    }

    pub fn compatibility_policy(&self) -> CompatibilityPolicy {
        match self.execution_mode {
            ExecutionMode::Strict => CompatibilityPolicy::strict(),
            ExecutionMode::Permissive => CompatibilityPolicy::permissive(),
            ExecutionMode::Experimental => CompatibilityPolicy::experimental(),
        }
    }

    pub fn machine_config_without_boot_rom_assets(&self) -> MachineConfig {
        let revision = self.effective_revision();
        let machine_config = MachineConfig::new(self.console_model.console_model())
            .with_startup_mode(self.startup_mode)
            .with_revision(revision)
            .with_compatibility(self.compatibility_policy());
        if let Some(profile) = self
            .console_model
            .sgb_profile_for_standard(self.effective_sgb_video_standard())
        {
            machine_config.with_sgb_profile(profile)
        } else {
            machine_config
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
            sgb_video_standard: SgbVideoStandard::default(),
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
        asset: impl Into<BootRomAssetKind>,
    ) -> Result<BootRomAssets, BootRomAssetError> {
        let asset = asset.into();
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
            return BootRomAssets::none().with_asset_bytes(asset, bytes);
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
    pub sgb_border: SgbBorderPresentationMode,
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
            sgb_border: SgbBorderPresentationMode::Auto,
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
mod test;
