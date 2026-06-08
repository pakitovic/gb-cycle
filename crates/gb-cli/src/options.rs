use crate::framebuffer::{DisplayPalette, RunDisplayPalette};
use gb_benchmark::BenchmarkCase;
use gb_core::{
    ConsoleModel, ExecutionMode, HardwareRevision, HostPlatform, SgbHostProfile, SgbVideoStandard,
    StartupMode,
};
use std::path::PathBuf;

pub(crate) const DEFAULT_SKIP_BOOT_FRAME_LIMIT: u32 = 120;

pub(crate) const DEFAULT_REAL_BOOT_POST_HANDOFF_FRAME_LIMIT: u32 = 120;

pub(crate) const DEFAULT_REAL_BOOT_SAFETY_FRAME_LIMIT: u32 = 480;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum BootRomVerificationMode {
    Off,
    Warn,
    #[default]
    Strict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum RunModel {
    #[default]
    GameBoy,
    Pocket,
    Light,
    Color,
    Advance,
    SuperGameBoy,
    SuperGameBoy2,
}

impl RunModel {
    pub(crate) fn console_model(self) -> ConsoleModel {
        match self {
            Self::GameBoy => ConsoleModel::GameBoy,
            Self::Pocket => ConsoleModel::GameBoyPocket,
            Self::Light => ConsoleModel::GameBoyLight,
            Self::Color => ConsoleModel::GameBoyColor,
            Self::Advance => ConsoleModel::GameBoyAdvance,
            Self::SuperGameBoy | Self::SuperGameBoy2 => ConsoleModel::GameBoy,
        }
    }

    pub(crate) const fn host_platform(self) -> HostPlatform {
        match self {
            Self::SuperGameBoy => HostPlatform::Sgb,
            Self::SuperGameBoy2 => HostPlatform::Sgb2,
            Self::GameBoy | Self::Pocket | Self::Light | Self::Color | Self::Advance => {
                HostPlatform::Handheld
            }
        }
    }

    #[cfg(test)]
    pub(crate) const fn sgb_profile(self) -> Option<SgbHostProfile> {
        match self {
            Self::SuperGameBoy => Some(SgbHostProfile::SgbNtsc),
            Self::SuperGameBoy2 => Some(SgbHostProfile::Sgb2Ntsc),
            Self::GameBoy | Self::Pocket | Self::Light | Self::Color | Self::Advance => None,
        }
    }

    pub(crate) const fn sgb_profile_for_standard(
        self,
        video_standard: SgbVideoStandard,
    ) -> Option<SgbHostProfile> {
        match self {
            Self::SuperGameBoy => match video_standard {
                SgbVideoStandard::Ntsc => Some(SgbHostProfile::SgbNtsc),
                SgbVideoStandard::Pal => Some(SgbHostProfile::SgbPal),
            },
            Self::SuperGameBoy2 => Some(SgbHostProfile::Sgb2Ntsc),
            Self::GameBoy | Self::Pocket | Self::Light | Self::Color | Self::Advance => None,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::GameBoy => "DMG",
            Self::Pocket => "MGB",
            Self::Light => "LGB",
            Self::Color => "CGB",
            Self::Advance => "AGB",
            Self::SuperGameBoy => "SGB",
            Self::SuperGameBoy2 => "SGB2",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SavePolicy {
    Manual,
    #[default]
    OnClose,
    OnWrite,
}

impl SavePolicy {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::OnClose => "on-close",
            Self::OnWrite => "on-write",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DefaultRunBudget {
    SkipBootFrames {
        frame_limit: u32,
    },
    RealBootPostHandoff {
        post_handoff_frame_limit: u32,
        safety_frame_limit: u32,
    },
}

impl DefaultRunBudget {
    pub(crate) fn for_startup_mode(startup_mode: StartupMode) -> Self {
        match startup_mode {
            StartupMode::SkipBoot | StartupMode::CustomBoot => Self::SkipBootFrames {
                frame_limit: DEFAULT_SKIP_BOOT_FRAME_LIMIT,
            },
            StartupMode::RealBoot => Self::RealBootPostHandoff {
                post_handoff_frame_limit: DEFAULT_REAL_BOOT_POST_HANDOFF_FRAME_LIMIT,
                safety_frame_limit: DEFAULT_REAL_BOOT_SAFETY_FRAME_LIMIT,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SgbBorderPresentationMode {
    Auto,
    Off,
}

impl SgbBorderPresentationMode {
    pub(crate) const fn is_auto(self) -> bool {
        matches!(self, Self::Auto)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RunOptions {
    pub(crate) rom_path: PathBuf,
    pub(crate) model: RunModel,
    pub(crate) revision: HardwareRevision,
    pub(crate) sgb_video_standard: SgbVideoStandard,
    pub(crate) startup_mode: StartupMode,
    pub(crate) execution_mode: ExecutionMode,
    pub(crate) boot_rom_dir: Option<PathBuf>,
    pub(crate) boot_rom_verify: BootRomVerificationMode,
    pub(crate) frame_limit: Option<u32>,
    pub(crate) tcycle_limit: Option<u64>,
    pub(crate) default_run_budget: Option<DefaultRunBudget>,
    pub(crate) serial_stdout: bool,
    pub(crate) serial_out: Option<PathBuf>,
    pub(crate) framebuffer_out: Option<PathBuf>,
    pub(crate) sgb_border: SgbBorderPresentationMode,
    pub(crate) display_palette: Option<RunDisplayPalette>,
    pub(crate) trace_out: Option<PathBuf>,
    pub(crate) state_in: Option<PathBuf>,
    pub(crate) state_out: Option<PathBuf>,
    pub(crate) save_dir: Option<PathBuf>,
    pub(crate) save_key: Option<String>,
    pub(crate) save_policy: SavePolicy,
    pub(crate) test_runner: bool,
    pub(crate) benchmark_case: Option<BenchmarkCase>,
}

impl RunOptions {
    pub(crate) fn default_with_rom(rom_path: PathBuf) -> Self {
        Self {
            rom_path,
            model: RunModel::default(),
            revision: RunModel::default().console_model().default_revision(),
            sgb_video_standard: SgbVideoStandard::default(),
            startup_mode: StartupMode::SkipBoot,
            execution_mode: ExecutionMode::Strict,
            boot_rom_dir: None,
            boot_rom_verify: BootRomVerificationMode::Strict,
            frame_limit: None,
            tcycle_limit: None,
            default_run_budget: None,
            serial_stdout: false,
            serial_out: None,
            framebuffer_out: None,
            sgb_border: SgbBorderPresentationMode::Auto,
            display_palette: None,
            trace_out: None,
            state_in: None,
            state_out: None,
            save_dir: None,
            save_key: None,
            save_policy: SavePolicy::default(),
            test_runner: false,
            benchmark_case: None,
        }
    }

    pub(crate) fn effective_display_palette(&self) -> Option<DisplayPalette> {
        if self.model == RunModel::GameBoy {
            self.display_palette.map(RunDisplayPalette::display_palette)
        } else {
            None
        }
    }

    pub(crate) fn effective_revision(&self) -> HardwareRevision {
        self.revision
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BenchmarkRunOptions {
    pub(crate) benchmark_path: PathBuf,
    pub(crate) test_runner: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InspectRomOptions {
    pub(crate) rom_path: PathBuf,
    pub(crate) execution_mode: ExecutionMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SavesDirection {
    Export,
    Import,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SavesOptions {
    pub(crate) direction: SavesDirection,
    pub(crate) rom_path: PathBuf,
    pub(crate) external_save_path: PathBuf,
    pub(crate) save_dir: PathBuf,
    pub(crate) save_key: Option<String>,
}
