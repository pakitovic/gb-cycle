use gb_desktop::{
    AudioOptions, DesktopConfig, GamepadButtonBindings, GamepadDirectionalSource,
    GamepadMenuBindings, HotkeyBindings, InputOptions, JoypadKeyboardBindings,
    MenuKeyboardBindings, PreferredGamepadIdentity, VideoOptions,
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

    fn save(&self) -> Result<(), String> {
        let Some(path) = self.path.as_deref() else {
            return Ok(());
        };

        self.settings.save(path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
struct PersistedDesktopSettings {
    version: u32,
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
        config.video = self.video.clone();
        config.audio = self.audio.audio_options();
        config.input = self.input.clone();
    }
}

impl Default for PersistedDesktopSettings {
    fn default() -> Self {
        Self {
            version: DESKTOP_SETTINGS_VERSION,
            video: VideoOptions::default(),
            audio: PersistedAudioSettings::default(),
            input: InputOptions::default(),
            last_open_directory: None,
            recent_roms: Vec::new(),
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
        PersistedDesktopSettings, resolve_desktop_settings_path_from_locations,
    };
    use gb_desktop::{
        DesktopConfig, DesktopKey, GamepadButtonBinding, GamepadDirectionalSource,
        GamepadMenuBindings, HotkeyBindings, JoypadKeyboardBindings, MenuKeyboardBindings,
        PreferredGamepadIdentity,
    };
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

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
        settings.video.window_scale = 6;
        settings.video.integer_scale = false;
        settings.video.fullscreen = true;
        settings.video.show_performance_hud = false;
        settings.audio.volume_percent = 75;
        settings.audio.muted = true;
        settings.input.keyboard.joypad.a = DesktopKey::Space;
        settings.input.keyboard.menu.confirm = DesktopKey::X;
        settings.input.keyboard.hotkeys.pause = DesktopKey::X;
        settings.input.gamepad.directional_source = GamepadDirectionalSource::LeftStickOnly;
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

        assert_eq!(config.video.window_scale, 6);
        assert!(!config.video.integer_scale);
        assert!(config.video.fullscreen);
        assert!(!config.video.show_performance_hud);
        assert_eq!(config.audio.volume_percent, 75);
        assert!(store.audio_muted());
        assert_eq!(config.input.keyboard.joypad.a, DesktopKey::Space);
        assert_eq!(config.input.keyboard.menu.confirm, DesktopKey::X);
        assert_eq!(config.input.keyboard.hotkeys.pause, DesktopKey::X);
        assert_eq!(
            config.input.gamepad.directional_source,
            GamepadDirectionalSource::LeftStickOnly
        );
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
