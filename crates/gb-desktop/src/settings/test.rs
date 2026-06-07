use super::{
    DESKTOP_SETTINGS_PATH_ENV_VAR, DESKTOP_SETTINGS_VERSION, DesktopSettingsStore, MAX_RECENT_ROMS,
    PersistedAudioSettings, PersistedBootRomVerificationMode, PersistedDesktopConsoleModel,
    PersistedDesktopSettings, PersistedExecutionMode, PersistedHardwareRevision,
    PersistedSaveDirectoryPolicy, PersistedSgbVideoStandard, PersistedStartupMode,
    resolve_desktop_settings_path_from_locations,
};
use gb_core::{ExecutionMode, HardwareRevision, SgbVideoStandard, StartupMode};
use gb_desktop::{
    DesktopConfig, DesktopConsoleModel, DesktopDisplayPalette, DesktopFrameBlendingMode,
    DesktopKey, DesktopSaveFlushPolicy, GamepadButtonBinding, GamepadDirectionalSource,
    GamepadGyroMode, GamepadMenuBindings, GamepadRumbleMode, HotkeyBindings, InputOptions,
    JoypadKeyboardBindings, MenuKeyboardBindings, PreferredGamepadIdentity, RewindOptions,
    SaveDirectoryPolicy, SgbBorderPresentationMode, VideoOptions,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn unique_test_path(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time should be after the Unix epoch")
        .as_nanos();
    std::env::temp_dir()
        .join(format!("gb-cycle-{label}-{unique}-{}", std::process::id()))
        .join("desktop-settings.toml")
}

#[path = "test/base_config.rs"]
mod base_config;
#[path = "test/conversion.rs"]
mod conversion;
#[path = "test/io.rs"]
mod io;
#[path = "test/paths_loading.rs"]
mod paths_loading;
#[path = "test/recent_roms.rs"]
mod recent_roms;
#[path = "test/runtime.rs"]
mod runtime;
#[path = "test/store_and_migrations.rs"]
mod store_and_migrations;
#[path = "test/video_rewind.rs"]
mod video_rewind;
