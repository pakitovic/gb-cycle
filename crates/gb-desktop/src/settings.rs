use gb_core::{ExecutionMode, HardwareRevision, SgbVideoStandard, StartupMode};
use gb_desktop::{
    AudioOptions, DesktopConfig, DesktopConsoleModel, DesktopDisplayPalette,
    DesktopFrameBlendingMode, DesktopKey, DesktopSaveFlushPolicy, FastForwardOptions,
    GamepadActionBindings, GamepadButtonBindings, GamepadDirectionalSource, GamepadGyroMode,
    GamepadMenuBindings, GamepadRumbleMode, HotkeyBindings, InputOptions, JoypadKeyboardBindings,
    MachineStateOptions, MenuKeyboardBindings, PreferredGamepadIdentity, RewindOptions,
    SaveDirectoryPolicy, VideoOptions,
};
use serde::{Deserialize, Serialize};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

pub const DESKTOP_SETTINGS_PATH_ENV_VAR: &str = "GB_CYCLE_DESKTOP_SETTINGS_PATH";

const DESKTOP_SETTINGS_VERSION: u32 = 1;
const DESKTOP_SETTINGS_DIRECTORY_NAME: &str = "gb-cycle";
const DESKTOP_SETTINGS_FILE_NAME: &str = "desktop-settings.toml";
const MAX_RECENT_ROMS: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopSettingsStore {
    path: Option<PathBuf>,
    settings: PersistedDesktopSettings,
}

impl DesktopSettingsStore {
    pub fn load() -> Result<Self, String> {
        let path = resolve_desktop_settings_path();
        let settings = match path.as_deref() {
            Some(path) => PersistedDesktopSettings::load(path)?,
            None => PersistedDesktopSettings::default(),
        };

        Ok(Self { path, settings })
    }

    pub fn base_config(&self) -> DesktopConfig {
        let mut config = DesktopConfig::default();
        self.settings.apply_to_config(&mut config);
        config
    }

    pub fn audio_muted(&self) -> bool {
        self.settings.audio.muted
    }

    pub fn persist_machine_preferences(&mut self, config: &DesktopConfig) -> Result<(), String> {
        let launch = PersistedLaunchSettings::from_config(config);
        let boot_rom = PersistedBootRomSettings::from_config(config);
        let saves = PersistedSaveSettings::from_config(config);
        let machine_state = config.machine_state;
        if self.settings.launch == launch
            && self.settings.boot_rom == boot_rom
            && self.settings.saves == saves
            && self.settings.machine_state == machine_state
        {
            return Ok(());
        }

        self.settings.launch = launch;
        self.settings.boot_rom = boot_rom;
        self.settings.saves = saves;
        self.settings.machine_state = machine_state;
        self.save()
    }

    pub fn last_open_directory(&self) -> Option<&Path> {
        self.settings.last_open_directory.as_deref()
    }

    pub fn recent_roms(&self) -> &[PathBuf] {
        &self.settings.recent_roms
    }

    pub fn set_fullscreen(&mut self, fullscreen: bool) -> Result<(), String> {
        if self.settings.video.fullscreen == fullscreen {
            return Ok(());
        }

        self.settings.video.fullscreen = fullscreen;
        self.save()
    }

    pub fn set_window_scale(&mut self, window_scale: u8) -> Result<(), String> {
        if self.settings.video.window_scale == window_scale {
            return Ok(());
        }

        self.settings.video.window_scale = window_scale;
        self.save()
    }

    pub fn set_integer_scale(&mut self, integer_scale: bool) -> Result<(), String> {
        if self.settings.video.integer_scale == integer_scale {
            return Ok(());
        }

        self.settings.video.integer_scale = integer_scale;
        self.save()
    }

    pub fn set_presentation_filter(&mut self, presentation_filter: bool) -> Result<(), String> {
        if self.settings.video.presentation_filter == presentation_filter {
            return Ok(());
        }

        self.settings.video.presentation_filter = presentation_filter;
        self.save()
    }

    pub fn set_frame_blending(
        &mut self,
        frame_blending: DesktopFrameBlendingMode,
    ) -> Result<(), String> {
        if self.settings.video.frame_blending == frame_blending {
            return Ok(());
        }

        self.settings.video.frame_blending = frame_blending;
        self.save()
    }

    pub fn set_display_palette(
        &mut self,
        display_palette: DesktopDisplayPalette,
    ) -> Result<(), String> {
        if self.settings.video.display_palette == display_palette {
            return Ok(());
        }

        self.settings.video.display_palette = display_palette;
        self.save()
    }

    pub fn set_show_background(&mut self, show_background: bool) -> Result<(), String> {
        if self.settings.video.show_background == show_background {
            return Ok(());
        }

        self.settings.video.show_background = show_background;
        self.save()
    }

    pub fn set_show_window(&mut self, show_window: bool) -> Result<(), String> {
        if self.settings.video.show_window == show_window {
            return Ok(());
        }

        self.settings.video.show_window = show_window;
        self.save()
    }

    pub fn set_show_objects(&mut self, show_objects: bool) -> Result<(), String> {
        if self.settings.video.show_objects == show_objects {
            return Ok(());
        }

        self.settings.video.show_objects = show_objects;
        self.save()
    }

    pub fn set_show_performance_hud(&mut self, show_performance_hud: bool) -> Result<(), String> {
        if self.settings.video.show_performance_hud == show_performance_hud {
            return Ok(());
        }

        self.settings.video.show_performance_hud = show_performance_hud;
        self.save()
    }

    pub fn set_show_sgb_border(&mut self, show_sgb_border: bool) -> Result<(), String> {
        if self.settings.video.show_sgb_border == show_sgb_border {
            return Ok(());
        }

        self.settings.video.show_sgb_border = show_sgb_border;
        self.save()
    }

    pub fn set_show_cgb_infrared_helper(
        &mut self,
        show_cgb_infrared_helper: bool,
    ) -> Result<(), String> {
        if self.settings.video.show_cgb_infrared_helper == show_cgb_infrared_helper {
            return Ok(());
        }

        self.settings.video.show_cgb_infrared_helper = show_cgb_infrared_helper;
        self.save()
    }

    pub fn set_vsync(&mut self, vsync: bool) -> Result<(), String> {
        if self.settings.video.vsync == vsync {
            return Ok(());
        }

        self.settings.video.vsync = vsync;
        self.save()
    }

    pub fn reset_video_defaults(
        &mut self,
        console_model: DesktopConsoleModel,
    ) -> Result<(), String> {
        let defaults = VideoOptions::default_for_console_model(console_model);
        if self.settings.video == defaults {
            return Ok(());
        }

        self.settings.video = defaults;
        self.save()
    }

    pub fn set_rewind_options(&mut self, rewind: RewindOptions) -> Result<(), String> {
        if self.settings.rewind == rewind {
            return Ok(());
        }

        self.settings.rewind = rewind;
        self.save()
    }

    pub fn set_fast_forward_options(
        &mut self,
        fast_forward: FastForwardOptions,
    ) -> Result<(), String> {
        if self.settings.fast_forward == fast_forward {
            return Ok(());
        }

        self.settings.fast_forward = fast_forward;
        self.save()
    }

    pub fn set_machine_state_options(
        &mut self,
        machine_state: MachineStateOptions,
    ) -> Result<(), String> {
        if self.settings.machine_state == machine_state {
            return Ok(());
        }

        self.settings.machine_state = machine_state;
        self.save()
    }

    pub fn set_audio_muted(&mut self, muted: bool) -> Result<(), String> {
        if self.settings.audio.muted == muted {
            return Ok(());
        }

        self.settings.audio.muted = muted;
        self.save()
    }

    pub fn set_audio_volume_percent(&mut self, volume_percent: u8) -> Result<(), String> {
        if self.settings.audio.volume_percent == volume_percent {
            return Ok(());
        }

        self.settings.audio.volume_percent = volume_percent;
        self.save()
    }

    pub fn reset_audio_defaults(&mut self) -> Result<(), String> {
        let defaults = PersistedAudioSettings::default();
        if self.settings.audio == defaults {
            return Ok(());
        }

        self.settings.audio = defaults;
        self.save()
    }

    pub fn remember_loaded_rom(&mut self, rom_path: &Path) -> Result<(), String> {
        let rom_directory = rom_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let mut changed = false;
        if self.settings.last_open_directory.as_ref() != Some(&rom_directory) {
            self.settings.last_open_directory = Some(rom_directory);
            changed = true;
        }

        let mut recent_roms = self
            .settings
            .recent_roms
            .iter()
            .filter(|path| path.as_path() != rom_path)
            .cloned()
            .collect::<Vec<_>>();
        recent_roms.insert(0, rom_path.to_path_buf());
        recent_roms.truncate(MAX_RECENT_ROMS);
        if self.settings.recent_roms != recent_roms {
            self.settings.recent_roms = recent_roms;
            changed = true;
        }

        if changed { self.save() } else { Ok(()) }
    }

    pub fn remove_recent_rom(&mut self, rom_path: &Path) -> Result<(), String> {
        let original_len = self.settings.recent_roms.len();
        self.settings
            .recent_roms
            .retain(|path| path.as_path() != rom_path);
        if self.settings.recent_roms.len() == original_len {
            return Ok(());
        }

        self.save()
    }

    pub fn clear_recent_roms(&mut self) -> Result<(), String> {
        if self.settings.recent_roms.is_empty() {
            return Ok(());
        }

        self.settings.recent_roms.clear();
        self.save()
    }

    pub fn set_gamepad_directional_source(
        &mut self,
        directional_source: GamepadDirectionalSource,
    ) -> Result<(), String> {
        if self.settings.input.gamepad.directional_source == directional_source {
            return Ok(());
        }

        self.settings.input.gamepad.directional_source = directional_source;
        self.save()
    }

    pub fn set_gamepad_rumble_mode(
        &mut self,
        rumble_mode: GamepadRumbleMode,
    ) -> Result<(), String> {
        if self.settings.input.gamepad.rumble_mode == rumble_mode {
            return Ok(());
        }

        self.settings.input.gamepad.rumble_mode = rumble_mode;
        self.save()
    }

    pub fn set_gamepad_gyro_mode(&mut self, gyro_mode: GamepadGyroMode) -> Result<(), String> {
        if self.settings.input.gamepad.gyro_mode == gyro_mode {
            return Ok(());
        }

        self.settings.input.gamepad.gyro_mode = gyro_mode;
        self.save()
    }

    pub fn set_gamepad_bindings(&mut self, bindings: GamepadButtonBindings) -> Result<(), String> {
        if self.settings.input.gamepad.bindings == bindings {
            return Ok(());
        }

        self.settings.input.gamepad.bindings = bindings;
        self.save()
    }

    pub fn set_gamepad_action_bindings(
        &mut self,
        bindings: GamepadActionBindings,
    ) -> Result<(), String> {
        if self.settings.input.gamepad.actions == bindings {
            return Ok(());
        }

        self.settings.input.gamepad.actions = bindings;
        self.save()
    }

    pub fn set_gamepad_menu_bindings(
        &mut self,
        bindings: GamepadMenuBindings,
    ) -> Result<(), String> {
        if self.settings.input.gamepad.menu == bindings {
            return Ok(());
        }

        self.settings.input.gamepad.menu = bindings;
        self.save()
    }

    pub fn set_preferred_gamepad_device(
        &mut self,
        preferred_device: PreferredGamepadIdentity,
    ) -> Result<(), String> {
        if self.settings.input.gamepad.preferred_device == preferred_device {
            return Ok(());
        }

        self.settings.input.gamepad.preferred_device = preferred_device;
        self.save()
    }

    pub fn set_keyboard_joypad_bindings(
        &mut self,
        bindings: JoypadKeyboardBindings,
    ) -> Result<(), String> {
        if self.settings.input.keyboard.joypad == bindings {
            return Ok(());
        }

        self.settings.input.keyboard.joypad = bindings;
        self.save()
    }

    pub fn set_keyboard_menu_bindings(
        &mut self,
        bindings: MenuKeyboardBindings,
    ) -> Result<(), String> {
        if self.settings.input.keyboard.menu == bindings {
            return Ok(());
        }

        self.settings.input.keyboard.menu = bindings;
        self.save()
    }

    pub fn set_keyboard_hotkey_bindings(&mut self, bindings: HotkeyBindings) -> Result<(), String> {
        if self.settings.input.keyboard.hotkeys == bindings {
            return Ok(());
        }

        self.settings.input.keyboard.hotkeys = bindings;
        self.save()
    }

    pub fn reset_input_defaults(&mut self) -> Result<(), String> {
        let defaults = InputOptions::default();
        if self.settings.input == defaults {
            return Ok(());
        }

        self.settings.input = defaults;
        self.save()
    }

    fn save(&self) -> Result<(), String> {
        let Some(path) = self.path.as_deref() else {
            return Ok(());
        };

        self.settings.save(path)
    }
}

#[cfg(test)]
impl DesktopSettingsStore {
    pub(crate) fn new_for_tests(path: PathBuf) -> Self {
        Self {
            path: Some(path),
            settings: PersistedDesktopSettings::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct PersistedDesktopSettings {
    version: u32,
    launch: PersistedLaunchSettings,
    boot_rom: PersistedBootRomSettings,
    saves: PersistedSaveSettings,
    machine_state: MachineStateOptions,
    rewind: RewindOptions,
    fast_forward: FastForwardOptions,
    video: VideoOptions,
    audio: PersistedAudioSettings,
    input: InputOptions,
    last_open_directory: Option<PathBuf>,
    recent_roms: Vec<PathBuf>,
}

impl PersistedDesktopSettings {
    fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let text = fs::read_to_string(path).map_err(|error| {
            format!(
                "failed to read desktop settings {}: {error}",
                path.display()
            )
        })?;
        let mut settings = toml::from_str::<Self>(&text).map_err(|error| {
            format!(
                "failed to parse desktop settings {}: {error}",
                path.display()
            )
        })?;
        if settings.version != DESKTOP_SETTINGS_VERSION {
            return Err(format!(
                "unsupported desktop settings version {} in {}; expected {}",
                settings.version,
                path.display(),
                DESKTOP_SETTINGS_VERSION
            ));
        }

        settings.migrate_defaults();
        Ok(settings)
    }

    fn migrate_defaults(&mut self) {
        if matches!(
            self.input.keyboard.hotkeys.reset,
            DesktopKey::R | DesktopKey::F1
        ) {
            self.input.keyboard.hotkeys.reset = DesktopKey::F12;
        }
        if self.input.keyboard.hotkeys.rewind == DesktopKey::F6 {
            self.input.keyboard.hotkeys.rewind = DesktopKey::LeftShift;
        }
        if self.input.keyboard.hotkeys.save_battery == DesktopKey::F5 {
            self.input.keyboard.hotkeys.save_battery = DesktopKey::F9;
        }
        if !matches!(self.machine_state.autoload_slot, None | Some(1..=4)) {
            self.machine_state.autoload_slot = None;
        }
    }

    fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create desktop settings directory {}: {error}",
                    parent.display()
                )
            })?;
        }

        let text = toml::to_string_pretty(self).map_err(|error| {
            format!(
                "failed to encode desktop settings {}: {error}",
                path.display()
            )
        })?;
        fs::write(path, text).map_err(|error| {
            format!(
                "failed to write desktop settings {}: {error}",
                path.display()
            )
        })
    }

    fn apply_to_config(&self, config: &mut DesktopConfig) {
        self.launch.apply_to_config(config);
        self.boot_rom.apply_to_config(config);
        self.saves.apply_to_config(config);
        config.machine_state = self.machine_state;
        config.rewind = self.rewind;
        config.fast_forward = self.fast_forward;
        config.video = self.video.clone();
        config.audio = self.audio.audio_options();
        config.input = self.input.clone();
    }
}

impl Default for PersistedDesktopSettings {
    fn default() -> Self {
        Self {
            version: DESKTOP_SETTINGS_VERSION,
            launch: PersistedLaunchSettings::default(),
            boot_rom: PersistedBootRomSettings::default(),
            saves: PersistedSaveSettings::default(),
            machine_state: MachineStateOptions::default(),
            rewind: RewindOptions::default(),
            fast_forward: FastForwardOptions::default(),
            video: VideoOptions::default(),
            audio: PersistedAudioSettings::default(),
            input: InputOptions::default(),
            last_open_directory: None,
            recent_roms: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
struct PersistedLaunchSettings {
    console_model: PersistedDesktopConsoleModel,
    revision: PersistedHardwareRevision,
    sgb_video_standard: PersistedSgbVideoStandard,
    startup_mode: PersistedStartupMode,
    execution_mode: PersistedExecutionMode,
}

impl PersistedLaunchSettings {
    fn from_config(config: &DesktopConfig) -> Self {
        Self {
            console_model: PersistedDesktopConsoleModel::from_external(config.launch.console_model),
            revision: PersistedHardwareRevision::from_external(config.launch.revision),
            sgb_video_standard: PersistedSgbVideoStandard::from_external(
                config.launch.sgb_video_standard,
            ),
            startup_mode: PersistedStartupMode::from_external(config.launch.startup_mode),
            execution_mode: PersistedExecutionMode::from_external(config.launch.execution_mode),
        }
    }

    fn apply_to_config(&self, config: &mut DesktopConfig) {
        config.launch.console_model = self.console_model.to_external();
        config.launch.revision = self.revision.to_external();
        config.launch.normalize_revision_for_model();
        config.launch.sgb_video_standard = self.sgb_video_standard.to_external();
        config.launch.startup_mode = self.startup_mode.to_external();
        config.launch.execution_mode = self.execution_mode.to_external();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
enum PersistedSgbVideoStandard {
    #[default]
    #[serde(rename = "ntsc")]
    Ntsc,
    #[serde(rename = "pal")]
    Pal,
}

impl PersistedSgbVideoStandard {
    fn from_external(value: SgbVideoStandard) -> Self {
        match value {
            SgbVideoStandard::Ntsc => Self::Ntsc,
            SgbVideoStandard::Pal => Self::Pal,
        }
    }

    fn to_external(self) -> SgbVideoStandard {
        match self {
            Self::Ntsc => SgbVideoStandard::Ntsc,
            Self::Pal => SgbVideoStandard::Pal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
enum PersistedHardwareRevision {
    #[serde(rename = "dmg-cpu")]
    DmgCpu,
    #[serde(rename = "dmg-cpu-a")]
    DmgCpuA,
    #[serde(rename = "dmg-cpu-b")]
    DmgCpuB,
    #[default]
    #[serde(rename = "dmg-cpu-c")]
    DmgCpuC,
    #[serde(rename = "cpu-mgb")]
    CpuMgb,
    #[serde(rename = "cpu-cgb")]
    CpuCgb,
    #[serde(rename = "cpu-cgb-a")]
    CpuCgbA,
    #[serde(rename = "cpu-cgb-b")]
    CpuCgbB,
    #[serde(rename = "cpu-cgb-c")]
    CpuCgbC,
    #[serde(rename = "cpu-cgb-d")]
    CpuCgbD,
    #[serde(rename = "cpu-cgb-e")]
    CpuCgbE,
    #[serde(rename = "cpu-agb-a")]
    CpuAgbA,
}

impl PersistedHardwareRevision {
    fn from_external(value: HardwareRevision) -> Self {
        match value {
            HardwareRevision::DmgCpu => Self::DmgCpu,
            HardwareRevision::DmgCpuA => Self::DmgCpuA,
            HardwareRevision::DmgCpuB => Self::DmgCpuB,
            HardwareRevision::DmgCpuC => Self::DmgCpuC,
            HardwareRevision::CpuMgb => Self::CpuMgb,
            HardwareRevision::CpuCgb => Self::CpuCgb,
            HardwareRevision::CpuCgbA => Self::CpuCgbA,
            HardwareRevision::CpuCgbB => Self::CpuCgbB,
            HardwareRevision::CpuCgbC => Self::CpuCgbC,
            HardwareRevision::CpuCgbD => Self::CpuCgbD,
            HardwareRevision::CpuCgbE => Self::CpuCgbE,
            HardwareRevision::CpuAgbA => Self::CpuAgbA,
        }
    }

    fn to_external(self) -> HardwareRevision {
        match self {
            Self::DmgCpu => HardwareRevision::DmgCpu,
            Self::DmgCpuA => HardwareRevision::DmgCpuA,
            Self::DmgCpuB => HardwareRevision::DmgCpuB,
            Self::DmgCpuC => HardwareRevision::DmgCpuC,
            Self::CpuMgb => HardwareRevision::CpuMgb,
            Self::CpuCgb => HardwareRevision::CpuCgb,
            Self::CpuCgbA => HardwareRevision::CpuCgbA,
            Self::CpuCgbB => HardwareRevision::CpuCgbB,
            Self::CpuCgbC => HardwareRevision::CpuCgbC,
            Self::CpuCgbD => HardwareRevision::CpuCgbD,
            Self::CpuCgbE => HardwareRevision::CpuCgbE,
            Self::CpuAgbA => HardwareRevision::CpuAgbA,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
struct PersistedBootRomSettings {
    search_path: Option<PathBuf>,
    verification: PersistedBootRomVerificationMode,
}

impl PersistedBootRomSettings {
    fn from_config(config: &DesktopConfig) -> Self {
        Self {
            search_path: config.boot_rom.search_path.clone(),
            verification: PersistedBootRomVerificationMode::from_external(
                config.boot_rom.verification,
            ),
        }
    }

    fn apply_to_config(&self, config: &mut DesktopConfig) {
        config.boot_rom.search_path = self.search_path.clone();
        config.boot_rom.verification = self.verification.to_external();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct PersistedSaveSettings {
    enabled: bool,
    directory_policy: PersistedSaveDirectoryPolicy,
    flush_policy: DesktopSaveFlushPolicy,
}

impl PersistedSaveSettings {
    fn from_config(config: &DesktopConfig) -> Self {
        Self {
            enabled: config.saves.enabled,
            directory_policy: PersistedSaveDirectoryPolicy::from_external(
                &config.saves.directory_policy,
            ),
            flush_policy: config.saves.flush_policy,
        }
    }

    fn apply_to_config(&self, config: &mut DesktopConfig) {
        config.saves.enabled = self.enabled;
        config.saves.directory_policy = self.directory_policy.to_external();
        config.saves.flush_policy = self.flush_policy;
    }
}

impl Default for PersistedSaveSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            directory_policy: PersistedSaveDirectoryPolicy::default(),
            flush_policy: DesktopSaveFlushPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", content = "path")]
enum PersistedSaveDirectoryPolicy {
    #[default]
    #[serde(rename = "rom-folder-saves-subdir")]
    RomFolderSavesSubdir,
    #[serde(rename = "custom")]
    Custom(PathBuf),
}

impl PersistedSaveDirectoryPolicy {
    fn from_external(value: &SaveDirectoryPolicy) -> Self {
        match value {
            SaveDirectoryPolicy::RomFolderSavesSubdir => Self::RomFolderSavesSubdir,
            SaveDirectoryPolicy::Custom(path) => Self::Custom(path.clone()),
        }
    }

    fn to_external(&self) -> SaveDirectoryPolicy {
        match self {
            Self::RomFolderSavesSubdir => SaveDirectoryPolicy::RomFolderSavesSubdir,
            Self::Custom(path) => SaveDirectoryPolicy::Custom(path.clone()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
enum PersistedDesktopConsoleModel {
    #[default]
    #[serde(rename = "game-boy")]
    GameBoy,
    #[serde(rename = "pocket")]
    GameBoyPocket,
    #[serde(rename = "light")]
    GameBoyLight,
    #[serde(rename = "color")]
    GameBoyColor,
    #[serde(rename = "advance", alias = "agb")]
    GameBoyAdvance,
    #[serde(rename = "super-gb")]
    SuperGameBoy,
    #[serde(rename = "super-gb-2")]
    SuperGameBoy2,
    #[serde(rename = "dmg0")]
    LegacyDmg0,
    #[serde(rename = "dmg")]
    LegacyDmg,
    #[serde(rename = "mgb")]
    LegacyMgb,
    #[serde(rename = "cgb")]
    LegacyCgb,
}

impl PersistedDesktopConsoleModel {
    fn from_external(value: gb_desktop::DesktopConsoleModel) -> Self {
        match value {
            gb_desktop::DesktopConsoleModel::GameBoy => Self::GameBoy,
            gb_desktop::DesktopConsoleModel::GameBoyPocket => Self::GameBoyPocket,
            gb_desktop::DesktopConsoleModel::GameBoyLight => Self::GameBoyLight,
            gb_desktop::DesktopConsoleModel::GameBoyColor => Self::GameBoyColor,
            gb_desktop::DesktopConsoleModel::GameBoyAdvance => Self::GameBoyAdvance,
            gb_desktop::DesktopConsoleModel::SuperGameBoy => Self::SuperGameBoy,
            gb_desktop::DesktopConsoleModel::SuperGameBoy2 => Self::SuperGameBoy2,
        }
    }

    fn to_external(self) -> gb_desktop::DesktopConsoleModel {
        match self {
            Self::GameBoy | Self::LegacyDmg0 | Self::LegacyDmg => {
                gb_desktop::DesktopConsoleModel::GameBoy
            }
            Self::GameBoyPocket | Self::LegacyMgb => gb_desktop::DesktopConsoleModel::GameBoyPocket,
            Self::GameBoyLight => gb_desktop::DesktopConsoleModel::GameBoyLight,
            Self::GameBoyColor | Self::LegacyCgb => gb_desktop::DesktopConsoleModel::GameBoyColor,
            Self::GameBoyAdvance => gb_desktop::DesktopConsoleModel::GameBoyAdvance,
            Self::SuperGameBoy => gb_desktop::DesktopConsoleModel::SuperGameBoy,
            Self::SuperGameBoy2 => gb_desktop::DesktopConsoleModel::SuperGameBoy2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
enum PersistedStartupMode {
    #[default]
    #[serde(rename = "skip-boot")]
    Skip,
    #[serde(rename = "custom-boot")]
    Custom,
    #[serde(rename = "real-boot")]
    Real,
}

impl PersistedStartupMode {
    fn from_external(value: StartupMode) -> Self {
        match value {
            StartupMode::SkipBoot => Self::Skip,
            StartupMode::CustomBoot => Self::Custom,
            StartupMode::RealBoot => Self::Real,
        }
    }

    fn to_external(self) -> StartupMode {
        match self {
            Self::Skip => StartupMode::SkipBoot,
            Self::Custom => StartupMode::CustomBoot,
            Self::Real => StartupMode::RealBoot,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
enum PersistedExecutionMode {
    #[default]
    #[serde(rename = "strict")]
    Strict,
    #[serde(rename = "permissive")]
    Permissive,
    #[serde(rename = "experimental")]
    Experimental,
}

impl PersistedExecutionMode {
    fn from_external(value: ExecutionMode) -> Self {
        match value {
            ExecutionMode::Strict => Self::Strict,
            ExecutionMode::Permissive => Self::Permissive,
            ExecutionMode::Experimental => Self::Experimental,
        }
    }

    fn to_external(self) -> ExecutionMode {
        match self {
            Self::Strict => ExecutionMode::Strict,
            Self::Permissive => ExecutionMode::Permissive,
            Self::Experimental => ExecutionMode::Experimental,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
enum PersistedBootRomVerificationMode {
    #[serde(rename = "off")]
    Off,
    #[serde(rename = "warn")]
    Warn,
    #[default]
    #[serde(rename = "strict")]
    Strict,
}

impl PersistedBootRomVerificationMode {
    fn from_external(value: gb_desktop::BootRomVerificationMode) -> Self {
        match value {
            gb_desktop::BootRomVerificationMode::Off => Self::Off,
            gb_desktop::BootRomVerificationMode::Warn => Self::Warn,
            gb_desktop::BootRomVerificationMode::Strict => Self::Strict,
        }
    }

    fn to_external(self) -> gb_desktop::BootRomVerificationMode {
        match self {
            Self::Off => gb_desktop::BootRomVerificationMode::Off,
            Self::Warn => gb_desktop::BootRomVerificationMode::Warn,
            Self::Strict => gb_desktop::BootRomVerificationMode::Strict,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct PersistedAudioSettings {
    enabled: bool,
    volume_percent: u8,
    output_sample_rate_hz: u32,
    buffer_frames: u16,
    muted: bool,
}

impl PersistedAudioSettings {
    fn audio_options(&self) -> AudioOptions {
        AudioOptions {
            enabled: self.enabled,
            volume_percent: self.volume_percent,
            output_sample_rate_hz: self.output_sample_rate_hz,
            buffer_frames: self.buffer_frames,
        }
    }
}

impl Default for PersistedAudioSettings {
    fn default() -> Self {
        let options = AudioOptions::default();
        Self {
            enabled: options.enabled,
            volume_percent: options.volume_percent,
            output_sample_rate_hz: options.output_sample_rate_hz,
            buffer_frames: options.buffer_frames,
            muted: false,
        }
    }
}

fn resolve_desktop_settings_path() -> Option<PathBuf> {
    resolve_desktop_settings_path_from_locations(
        env::var_os(DESKTOP_SETTINGS_PATH_ENV_VAR),
        env::var_os("HOME"),
        env::var_os("XDG_CONFIG_HOME"),
        env::var_os("APPDATA"),
    )
}

fn resolve_desktop_settings_path_from_locations(
    explicit_override: Option<OsString>,
    _home: Option<OsString>,
    _xdg_config_home: Option<OsString>,
    _app_data: Option<OsString>,
) -> Option<PathBuf> {
    if let Some(path) = explicit_override {
        return Some(PathBuf::from(path));
    }

    #[cfg(target_os = "macos")]
    {
        _home.map(PathBuf::from).map(|home| {
            home.join("Library")
                .join("Application Support")
                .join(DESKTOP_SETTINGS_DIRECTORY_NAME)
                .join(DESKTOP_SETTINGS_FILE_NAME)
        })
    }

    #[cfg(target_os = "windows")]
    {
        _app_data.map(PathBuf::from).map(|app_data| {
            app_data
                .join(DESKTOP_SETTINGS_DIRECTORY_NAME)
                .join(DESKTOP_SETTINGS_FILE_NAME)
        })
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        _xdg_config_home
            .map(PathBuf::from)
            .or_else(|| _home.map(PathBuf::from).map(|home| home.join(".config")))
            .map(|config_root| {
                config_root
                    .join(DESKTOP_SETTINGS_DIRECTORY_NAME)
                    .join(DESKTOP_SETTINGS_FILE_NAME)
            })
    }
}

#[cfg(test)]
mod test;
