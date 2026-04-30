mod config;
mod external_port;

pub use config::{
    AudioOptions, BootRomOptions, BootRomVerificationMode, DEFAULT_AUDIO_BUFFER_FRAMES,
    DEFAULT_AUDIO_SAMPLE_RATE_HZ, DEFAULT_FAST_FORWARD_SPEED_MULTIPLIER,
    DEFAULT_REWIND_HISTORY_SECONDS, DEFAULT_REWIND_MAX_MEMORY_MIB, DEFAULT_REWIND_SPEED_MULTIPLIER,
    DEFAULT_REWIND_SUBFRAMES_PER_FRAME, DEFAULT_SAVE_FLUSH_DEBOUNCE, DEFAULT_WINDOW_SCALE,
    DesktopConfig, DesktopConfigError, DesktopConsoleModel, DesktopDisplayPalette, DesktopKey,
    DesktopSaveFlushPolicy, FastForwardOptions, GamepadActionBindings, GamepadButtonBinding,
    GamepadButtonBindings, GamepadDirectionalSource, GamepadFaceLayout, GamepadMenuBindings,
    GamepadOptions, GamepadRumbleMode, HotkeyBindings, InputOptions, JoypadKeyboardBindings,
    KeyboardBindings, LaunchOptions, MachineStateOptions, MenuKeyboardBindings,
    PreferredGamepadIdentity, RewindOptions, SaveDirectoryPolicy, SaveKeyPolicy, SaveOptions,
    VideoOptions,
};

pub use external_port::DesktopExternalPortSelection;
