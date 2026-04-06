use gb_core::{ExecutionMode, StartupMode};
use gb_desktop::{
    AudioOptions, DesktopConfig, DesktopSaveFlushPolicy, GamepadButtonBindings,
    GamepadDirectionalSource, GamepadMenuBindings, GamepadRumbleMode, HotkeyBindings, InputOptions,
    JoypadKeyboardBindings, MenuKeyboardBindings, PreferredGamepadIdentity, SaveDirectoryPolicy,
    VideoOptions,
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
const MAX_RECENT_ROMS: usize = 8;

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
        if self.settings.launch == launch
            && self.settings.boot_rom == boot_rom
            && self.settings.saves == saves
        {
            return Ok(());
        }

        self.settings.launch = launch;
        self.settings.boot_rom = boot_rom;
        self.settings.saves = saves;
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

    pub fn set_show_performance_hud(&mut self, show_performance_hud: bool) -> Result<(), String> {
        if self.settings.video.show_performance_hud == show_performance_hud {
            return Ok(());
        }

        self.settings.video.show_performance_hud = show_performance_hud;
        self.save()
    }

    pub fn set_vsync(&mut self, vsync: bool) -> Result<(), String> {
        if self.settings.video.vsync == vsync {
            return Ok(());
        }

        self.settings.video.vsync = vsync;
        self.save()
    }

    pub fn reset_video_defaults(&mut self) -> Result<(), String> {
        let defaults = VideoOptions::default();
        if self.settings.video == defaults {
            return Ok(());
        }

        self.settings.video = defaults;
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

    pub fn set_gamepad_bindings(&mut self, bindings: GamepadButtonBindings) -> Result<(), String> {
        if self.settings.input.gamepad.bindings == bindings {
            return Ok(());
        }

        self.settings.input.gamepad.bindings = bindings;
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
        let settings = toml::from_str::<Self>(&text).map_err(|error| {
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

        Ok(settings)
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
    startup_mode: PersistedStartupMode,
    execution_mode: PersistedExecutionMode,
}

impl PersistedLaunchSettings {
    fn from_config(config: &DesktopConfig) -> Self {
        Self {
            console_model: PersistedDesktopConsoleModel::from_external(config.launch.console_model),
            startup_mode: PersistedStartupMode::from_external(config.launch.startup_mode),
            execution_mode: PersistedExecutionMode::from_external(config.launch.execution_mode),
        }
    }

    fn apply_to_config(&self, config: &mut DesktopConfig) {
        config.launch.console_model = self.console_model.to_external();
        config.launch.startup_mode = self.startup_mode.to_external();
        config.launch.execution_mode = self.execution_mode.to_external();
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
    #[serde(rename = "dmg0")]
    Dmg0,
    #[default]
    #[serde(rename = "dmg")]
    Dmg,
    #[serde(rename = "mgb")]
    Mgb,
}

impl PersistedDesktopConsoleModel {
    fn from_external(value: gb_desktop::DesktopConsoleModel) -> Self {
        match value {
            gb_desktop::DesktopConsoleModel::Dmg0 => Self::Dmg0,
            gb_desktop::DesktopConsoleModel::Dmg => Self::Dmg,
            gb_desktop::DesktopConsoleModel::Mgb => Self::Mgb,
        }
    }

    fn to_external(self) -> gb_desktop::DesktopConsoleModel {
        match self {
            Self::Dmg0 => gb_desktop::DesktopConsoleModel::Dmg0,
            Self::Dmg => gb_desktop::DesktopConsoleModel::Dmg,
            Self::Mgb => gb_desktop::DesktopConsoleModel::Mgb,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
enum PersistedStartupMode {
    #[default]
    #[serde(rename = "skip-boot")]
    SkipBoot,
    #[serde(rename = "real-boot")]
    RealBoot,
}

impl PersistedStartupMode {
    fn from_external(value: StartupMode) -> Self {
        match value {
            StartupMode::SkipBoot => Self::SkipBoot,
            StartupMode::RealBoot => Self::RealBoot,
        }
    }

    fn to_external(self) -> StartupMode {
        match self {
            Self::SkipBoot => StartupMode::SkipBoot,
            Self::RealBoot => StartupMode::RealBoot,
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
    home: Option<OsString>,
    _xdg_config_home: Option<OsString>,
    _app_data: Option<OsString>,
) -> Option<PathBuf> {
    if let Some(path) = explicit_override {
        return Some(PathBuf::from(path));
    }

    #[cfg(target_os = "macos")]
    {
        home.map(PathBuf::from).map(|home| {
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
            .or_else(|| home.map(PathBuf::from).map(|home| home.join(".config")))
            .map(|config_root| {
                config_root
                    .join(DESKTOP_SETTINGS_DIRECTORY_NAME)
                    .join(DESKTOP_SETTINGS_FILE_NAME)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DESKTOP_SETTINGS_PATH_ENV_VAR, DESKTOP_SETTINGS_VERSION, DesktopSettingsStore,
        PersistedAudioSettings, PersistedBootRomVerificationMode, PersistedDesktopConsoleModel,
        PersistedDesktopSettings, PersistedExecutionMode, PersistedSaveDirectoryPolicy,
        PersistedStartupMode, resolve_desktop_settings_path_from_locations,
    };
    use gb_core::{ExecutionMode, StartupMode};
    use gb_desktop::{
        DesktopConfig, DesktopKey, DesktopSaveFlushPolicy, GamepadButtonBinding,
        GamepadDirectionalSource, GamepadMenuBindings, GamepadRumbleMode, HotkeyBindings,
        InputOptions, JoypadKeyboardBindings, MenuKeyboardBindings, PreferredGamepadIdentity,
        SaveDirectoryPolicy, VideoOptions,
    };
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn explicit_settings_path_override_wins_over_platform_defaults() {
        assert_eq!(
            resolve_desktop_settings_path_from_locations(
                Some(PathBuf::from("/tmp/custom-desktop-settings.toml").into_os_string()),
                Some(PathBuf::from("/Users/pakitovic").into_os_string()),
                None,
                Some(PathBuf::from("C:/Users/pakitovic/AppData/Roaming").into_os_string()),
            ),
            Some(PathBuf::from("/tmp/custom-desktop-settings.toml"))
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn platform_default_settings_path_matches_macos_conventions() {
        assert_eq!(
            resolve_desktop_settings_path_from_locations(
                None,
                Some(PathBuf::from("/Users/pakitovic").into_os_string()),
                None,
                None,
            ),
            Some(PathBuf::from(
                "/Users/pakitovic/Library/Application Support/gb-cycle/desktop-settings.toml"
            ))
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn platform_default_settings_path_matches_windows_conventions() {
        assert_eq!(
            resolve_desktop_settings_path_from_locations(
                None,
                None,
                None,
                Some(PathBuf::from("C:/Users/pakitovic/AppData/Roaming").into_os_string()),
            ),
            Some(PathBuf::from(
                "C:/Users/pakitovic/AppData/Roaming/gb-cycle/desktop-settings.toml"
            ))
        );
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    #[test]
    fn platform_default_settings_path_matches_xdg_conventions() {
        assert_eq!(
            resolve_desktop_settings_path_from_locations(
                None,
                Some(PathBuf::from("/home/pakitovic").into_os_string()),
                Some(PathBuf::from("/tmp/xdg-config").into_os_string()),
                None,
            ),
            Some(PathBuf::from(
                "/tmp/xdg-config/gb-cycle/desktop-settings.toml"
            ))
        );
    }

    #[test]
    fn missing_settings_file_falls_back_to_defaults() {
        let path = unique_test_path("missing-settings");
        let settings =
            PersistedDesktopSettings::load(&path).expect("missing settings should default");

        assert_eq!(settings.version, DESKTOP_SETTINGS_VERSION);
        assert_eq!(settings.video, DesktopConfig::default().video);
        assert_eq!(settings.input, DesktopConfig::default().input);
        assert_eq!(settings.last_open_directory, None);
        assert!(settings.recent_roms.is_empty());
    }

    #[test]
    fn settings_store_base_config_applies_persisted_host_preferences() {
        let path = unique_test_path("applies-settings");
        let mut settings = PersistedDesktopSettings::default();
        settings.launch.console_model = PersistedDesktopConsoleModel::Mgb;
        settings.launch.startup_mode = PersistedStartupMode::RealBoot;
        settings.launch.execution_mode = PersistedExecutionMode::Permissive;
        settings.boot_rom.search_path = Some(PathBuf::from("/tmp/firmware/mgb_boot.bin"));
        settings.boot_rom.verification = PersistedBootRomVerificationMode::Warn;
        settings.saves.enabled = false;
        settings.saves.directory_policy =
            PersistedSaveDirectoryPolicy::Custom(PathBuf::from("/tmp/saves"));
        settings.saves.flush_policy = DesktopSaveFlushPolicy::OnClose;
        settings.video.window_scale = 6;
        settings.video.integer_scale = false;
        settings.video.fullscreen = true;
        settings.video.show_performance_hud = false;
        settings.video.vsync = false;
        settings.audio.volume_percent = 75;
        settings.audio.muted = true;
        settings.input.keyboard.joypad.a = DesktopKey::Space;
        settings.input.keyboard.menu.confirm = DesktopKey::X;
        settings.input.keyboard.hotkeys.pause = DesktopKey::X;
        settings.input.gamepad.directional_source = GamepadDirectionalSource::LeftStickOnly;
        settings.input.gamepad.rumble_mode = GamepadRumbleMode::Weak;
        settings.input.gamepad.bindings.a = GamepadButtonBinding::North;
        settings.input.gamepad.menu.cancel = GamepadButtonBinding::West;
        settings.input.gamepad.preferred_device = PreferredGamepadIdentity {
            name: Some("Nintendo Switch Pro Controller".to_string()),
            path: Some("bluetooth:vendor=057e,product=2009".to_string()),
        };
        settings
            .save(&path)
            .expect("settings file should be writable");

        let store = DesktopSettingsStore {
            path: Some(path.clone()),
            settings: PersistedDesktopSettings::load(&path).expect("saved settings should reload"),
        };
        let config = store.base_config();

        assert_eq!(
            config.launch.console_model,
            gb_desktop::DesktopConsoleModel::Mgb
        );
        assert_eq!(config.launch.startup_mode, StartupMode::RealBoot);
        assert_eq!(config.launch.execution_mode, ExecutionMode::Permissive);
        assert_eq!(
            config.boot_rom.search_path,
            Some(PathBuf::from("/tmp/firmware/mgb_boot.bin"))
        );
        assert_eq!(
            config.boot_rom.verification,
            gb_desktop::BootRomVerificationMode::Warn
        );
        assert!(!config.saves.enabled);
        assert_eq!(
            config.saves.directory_policy,
            SaveDirectoryPolicy::Custom(PathBuf::from("/tmp/saves"))
        );
        assert_eq!(config.saves.flush_policy, DesktopSaveFlushPolicy::OnClose);
        assert_eq!(config.video.window_scale, 6);
        assert!(!config.video.integer_scale);
        assert!(config.video.fullscreen);
        assert!(!config.video.show_performance_hud);
        assert!(!config.video.vsync);
        assert_eq!(config.audio.volume_percent, 75);
        assert!(store.audio_muted());
        assert_eq!(config.input.keyboard.joypad.a, DesktopKey::Space);
        assert_eq!(config.input.keyboard.menu.confirm, DesktopKey::X);
        assert_eq!(config.input.keyboard.hotkeys.pause, DesktopKey::X);
        assert_eq!(
            config.input.gamepad.directional_source,
            GamepadDirectionalSource::LeftStickOnly
        );
        assert_eq!(config.input.gamepad.rumble_mode, GamepadRumbleMode::Weak);
        assert_eq!(config.input.gamepad.bindings.a, GamepadButtonBinding::North);
        assert_eq!(config.input.gamepad.menu.cancel, GamepadButtonBinding::West);
        assert_eq!(
            config.input.gamepad.preferred_device,
            PreferredGamepadIdentity {
                name: Some("Nintendo Switch Pro Controller".to_string()),
                path: Some("bluetooth:vendor=057e,product=2009".to_string()),
            }
        );
        assert!(store.recent_roms().is_empty());
    }

    #[test]
    fn settings_store_load_uses_the_env_override_and_defaults_missing_files() {
        let _lock = ENV_LOCK.lock().expect("env lock should not be poisoned");
        let path = unique_test_path("load-env-override");
        let settings = PersistedDesktopSettings {
            recent_roms: vec![PathBuf::from("/tmp/roms/Kirby.gb")],
            last_open_directory: Some(PathBuf::from("/tmp/roms")),
            ..PersistedDesktopSettings::default()
        };
        settings
            .save(&path)
            .expect("settings file should be writable through the env override");

        unsafe {
            std::env::set_var(DESKTOP_SETTINGS_PATH_ENV_VAR, &path);
        }
        let loaded = DesktopSettingsStore::load().expect("settings store should load from env");
        assert_eq!(loaded.last_open_directory(), Some(Path::new("/tmp/roms")));
        assert_eq!(loaded.recent_roms(), &[PathBuf::from("/tmp/roms/Kirby.gb")]);

        let missing_path = unique_test_path("load-missing-env-override");
        unsafe {
            std::env::set_var(DESKTOP_SETTINGS_PATH_ENV_VAR, &missing_path);
        }
        let missing =
            DesktopSettingsStore::load().expect("missing env-backed settings should default");
        assert_eq!(missing.last_open_directory(), None);
        assert!(missing.recent_roms().is_empty());

        unsafe {
            std::env::remove_var(DESKTOP_SETTINGS_PATH_ENV_VAR);
        }
    }

    #[test]
    fn persisting_machine_preferences_updates_the_saved_settings() {
        let path = unique_test_path("persist-launch-boot");
        let mut store = DesktopSettingsStore {
            path: Some(path.clone()),
            settings: PersistedDesktopSettings::default(),
        };
        let mut config = DesktopConfig::default();
        config.launch.console_model = gb_desktop::DesktopConsoleModel::Dmg0;
        config.launch.startup_mode = StartupMode::RealBoot;
        config.launch.execution_mode = ExecutionMode::Experimental;
        config.boot_rom.search_path = Some(PathBuf::from("/tmp/firmware/dmg0_boot.bin"));
        config.boot_rom.verification = gb_desktop::BootRomVerificationMode::Off;
        config.saves.enabled = false;
        config.saves.directory_policy = SaveDirectoryPolicy::Custom(PathBuf::from("/tmp/gb-saves"));
        config.saves.flush_policy = DesktopSaveFlushPolicy::OnWrite;

        store
            .persist_machine_preferences(&config)
            .expect("machine settings should persist");

        let reloaded =
            PersistedDesktopSettings::load(&path).expect("persisted settings should reload");
        assert_eq!(
            reloaded.launch.console_model,
            PersistedDesktopConsoleModel::Dmg0
        );
        assert_eq!(reloaded.launch.startup_mode, PersistedStartupMode::RealBoot);
        assert_eq!(
            reloaded.launch.execution_mode,
            PersistedExecutionMode::Experimental
        );
        assert_eq!(
            reloaded.boot_rom.search_path,
            Some(PathBuf::from("/tmp/firmware/dmg0_boot.bin"))
        );
        assert_eq!(
            reloaded.boot_rom.verification,
            PersistedBootRomVerificationMode::Off
        );
        assert!(!reloaded.saves.enabled);
        assert_eq!(
            reloaded.saves.directory_policy,
            PersistedSaveDirectoryPolicy::Custom(PathBuf::from("/tmp/gb-saves"))
        );
        assert_eq!(reloaded.saves.flush_policy, DesktopSaveFlushPolicy::OnWrite);
    }

    #[test]
    fn runtime_updates_persist_muted_fullscreen_and_last_open_directory() {
        let path = unique_test_path("runtime-updates");
        let mut store = DesktopSettingsStore {
            path: Some(path.clone()),
            settings: PersistedDesktopSettings::default(),
        };

        store
            .set_fullscreen(true)
            .expect("fullscreen toggle should persist");
        store
            .set_window_scale(6)
            .expect("window scale should persist");
        store
            .set_integer_scale(false)
            .expect("integer scale should persist");
        store
            .set_show_performance_hud(false)
            .expect("performance HUD visibility should persist");
        store.set_vsync(false).expect("vsync toggle should persist");
        store
            .set_audio_muted(true)
            .expect("audio mute toggle should persist");
        store
            .set_audio_volume_percent(75)
            .expect("audio volume should persist");
        store
            .remember_loaded_rom(Path::new("/tmp/roms/Tetris.gb"))
            .expect("loaded ROM directory should persist");
        store
            .set_gamepad_directional_source(GamepadDirectionalSource::LeftStickOnly)
            .expect("gamepad direction should persist");
        store
            .set_keyboard_joypad_bindings(JoypadKeyboardBindings {
                a: DesktopKey::Space,
                ..JoypadKeyboardBindings::default()
            })
            .expect("keyboard joypad bindings should persist");
        store
            .set_keyboard_menu_bindings(MenuKeyboardBindings {
                confirm: DesktopKey::X,
                ..MenuKeyboardBindings::default()
            })
            .expect("keyboard menu bindings should persist");
        store
            .set_preferred_gamepad_device(PreferredGamepadIdentity {
                name: Some("Nintendo Switch Pro Controller".to_string()),
                path: Some("bluetooth:vendor=057e,product=2009".to_string()),
            })
            .expect("preferred gamepad identity should persist");
        store
            .set_gamepad_menu_bindings(GamepadMenuBindings {
                cancel: GamepadButtonBinding::West,
                ..GamepadMenuBindings::default()
            })
            .expect("gamepad menu bindings should persist");
        store
            .set_keyboard_hotkey_bindings(HotkeyBindings {
                pause: DesktopKey::X,
                ..HotkeyBindings::default()
            })
            .expect("keyboard hotkey bindings should persist");

        let reloaded =
            PersistedDesktopSettings::load(&path).expect("persisted settings should reload");
        assert!(reloaded.video.fullscreen);
        assert_eq!(reloaded.video.window_scale, 6);
        assert!(!reloaded.video.integer_scale);
        assert!(!reloaded.video.show_performance_hud);
        assert!(!reloaded.video.vsync);
        assert_eq!(reloaded.audio.volume_percent, 75);
        assert!(reloaded.audio.muted);
        assert_eq!(
            reloaded.input.gamepad.directional_source,
            GamepadDirectionalSource::LeftStickOnly
        );
        assert_eq!(
            reloaded.input.gamepad.menu.cancel,
            GamepadButtonBinding::West
        );
        assert_eq!(
            reloaded.input.gamepad.preferred_device,
            PreferredGamepadIdentity {
                name: Some("Nintendo Switch Pro Controller".to_string()),
                path: Some("bluetooth:vendor=057e,product=2009".to_string()),
            }
        );
        assert_eq!(reloaded.input.keyboard.joypad.a, DesktopKey::Space);
        assert_eq!(reloaded.input.keyboard.menu.confirm, DesktopKey::X);
        assert_eq!(reloaded.input.keyboard.hotkeys.pause, DesktopKey::X);
        assert_eq!(
            reloaded.last_open_directory,
            Some(PathBuf::from("/tmp/roms"))
        );
        assert_eq!(
            reloaded.recent_roms,
            vec![PathBuf::from("/tmp/roms/Tetris.gb")]
        );
    }

    #[test]
    fn reset_helpers_restore_default_video_audio_and_input_preferences() {
        let path = unique_test_path("reset-defaults");
        let mut store = DesktopSettingsStore {
            path: Some(path.clone()),
            settings: PersistedDesktopSettings::default(),
        };

        store
            .set_fullscreen(true)
            .expect("fullscreen toggle should persist");
        store
            .set_window_scale(6)
            .expect("window scale should persist");
        store
            .set_integer_scale(false)
            .expect("integer scale should persist");
        store
            .set_show_performance_hud(false)
            .expect("HUD visibility should persist");
        store.set_vsync(false).expect("vsync should persist");
        store
            .set_audio_muted(true)
            .expect("audio mute toggle should persist");
        store
            .set_audio_volume_percent(75)
            .expect("audio volume should persist");
        store
            .set_keyboard_joypad_bindings(JoypadKeyboardBindings {
                a: DesktopKey::Space,
                ..JoypadKeyboardBindings::default()
            })
            .expect("keyboard joypad bindings should persist");
        store
            .set_keyboard_menu_bindings(MenuKeyboardBindings {
                confirm: DesktopKey::X,
                ..MenuKeyboardBindings::default()
            })
            .expect("keyboard menu bindings should persist");
        store
            .set_keyboard_hotkey_bindings(HotkeyBindings {
                pause: DesktopKey::X,
                ..HotkeyBindings::default()
            })
            .expect("keyboard hotkey bindings should persist");

        store
            .reset_video_defaults()
            .expect("video defaults should persist");
        store
            .reset_audio_defaults()
            .expect("audio defaults should persist");
        store
            .reset_input_defaults()
            .expect("input defaults should persist");

        let reloaded =
            PersistedDesktopSettings::load(&path).expect("persisted settings should reload");
        assert_eq!(reloaded.video, VideoOptions::default());
        assert_eq!(reloaded.audio, PersistedAudioSettings::default());
        assert_eq!(reloaded.input, InputOptions::default());
    }

    #[test]
    fn remembered_roms_are_deduplicated_and_kept_in_most_recent_order() {
        let path = unique_test_path("recent-roms");
        let mut store = DesktopSettingsStore {
            path: Some(path.clone()),
            settings: PersistedDesktopSettings::default(),
        };

        store
            .remember_loaded_rom(Path::new("/tmp/roms/Tetris.gb"))
            .expect("first ROM should persist");
        store
            .remember_loaded_rom(Path::new("/tmp/roms/DrMario.gb"))
            .expect("second ROM should persist");
        store
            .remember_loaded_rom(Path::new("/tmp/roms/Tetris.gb"))
            .expect("reloading a recent ROM should move it to the front");

        let reloaded =
            PersistedDesktopSettings::load(&path).expect("persisted settings should reload");
        assert_eq!(
            reloaded.recent_roms,
            vec![
                PathBuf::from("/tmp/roms/Tetris.gb"),
                PathBuf::from("/tmp/roms/DrMario.gb"),
            ]
        );
    }

    #[test]
    fn removing_a_recent_rom_updates_the_persisted_history() {
        let path = unique_test_path("remove-recent-rom");
        let mut store = DesktopSettingsStore {
            path: Some(path.clone()),
            settings: PersistedDesktopSettings::default(),
        };

        store
            .remember_loaded_rom(Path::new("/tmp/roms/Tetris.gb"))
            .expect("first ROM should persist");
        store
            .remember_loaded_rom(Path::new("/tmp/roms/DrMario.gb"))
            .expect("second ROM should persist");
        store
            .remove_recent_rom(Path::new("/tmp/roms/Tetris.gb"))
            .expect("stale ROM should be removable");

        let reloaded =
            PersistedDesktopSettings::load(&path).expect("persisted settings should reload");
        assert_eq!(
            reloaded.recent_roms,
            vec![PathBuf::from("/tmp/roms/DrMario.gb")]
        );
    }

    #[test]
    fn settings_path_env_var_name_stays_stable() {
        assert_eq!(
            DESKTOP_SETTINGS_PATH_ENV_VAR,
            "GB_CYCLE_DESKTOP_SETTINGS_PATH"
        );
    }

    #[test]
    fn persisted_conversion_helpers_round_trip_external_values() {
        assert_eq!(
            PersistedSaveDirectoryPolicy::from_external(&SaveDirectoryPolicy::Custom(
                PathBuf::from("/tmp/saves")
            ))
            .to_external(),
            SaveDirectoryPolicy::Custom(PathBuf::from("/tmp/saves"))
        );
        assert_eq!(
            PersistedDesktopConsoleModel::from_external(gb_desktop::DesktopConsoleModel::Mgb)
                .to_external(),
            gb_desktop::DesktopConsoleModel::Mgb
        );
        assert_eq!(
            PersistedStartupMode::from_external(StartupMode::RealBoot).to_external(),
            StartupMode::RealBoot
        );
        assert_eq!(
            PersistedExecutionMode::from_external(ExecutionMode::Experimental).to_external(),
            ExecutionMode::Experimental
        );
        assert_eq!(
            PersistedBootRomVerificationMode::from_external(
                gb_desktop::BootRomVerificationMode::Warn
            )
            .to_external(),
            gb_desktop::BootRomVerificationMode::Warn
        );
    }

    #[test]
    fn persisted_audio_settings_rebuild_audio_options() {
        let audio = PersistedAudioSettings {
            enabled: false,
            volume_percent: 75,
            output_sample_rate_hz: 44_100,
            buffer_frames: 256,
            muted: true,
        };

        assert_eq!(
            audio.audio_options(),
            gb_desktop::AudioOptions {
                enabled: false,
                volume_percent: 75,
                output_sample_rate_hz: 44_100,
                buffer_frames: 256,
            }
        );
    }

    #[test]
    fn store_exposes_last_open_directory_and_gamepad_binding_updates() {
        let path = unique_test_path("gamepad-bindings");
        let mut store = DesktopSettingsStore {
            path: Some(path.clone()),
            settings: PersistedDesktopSettings::default(),
        };
        let bindings = gb_desktop::GamepadButtonBindings {
            a: GamepadButtonBinding::North,
            ..gb_desktop::GamepadButtonBindings::default()
        };

        store
            .set_gamepad_bindings(bindings)
            .expect("gamepad bindings should persist");
        store
            .remember_loaded_rom(Path::new("/tmp/roms/Alleyway.gb"))
            .expect("loaded ROM should update the last open directory");

        assert_eq!(store.last_open_directory(), Some(Path::new("/tmp/roms")));

        let reloaded =
            PersistedDesktopSettings::load(&path).expect("persisted settings should reload");
        assert_eq!(reloaded.input.gamepad.bindings, bindings);
        assert_eq!(
            reloaded.last_open_directory,
            Some(PathBuf::from("/tmp/roms"))
        );
    }

    fn unique_test_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time should be after the Unix epoch")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("gb-cycle-{label}-{unique}-{}", std::process::id()))
            .join("desktop-settings.toml")
    }
}
