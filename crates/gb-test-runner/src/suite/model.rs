use std::path::PathBuf;

use gb_core::{
    ConsoleModel, DMG_T_CYCLES_PER_FRAME, ExecutionMode, HardwareRevision, HostPlatform,
    JoypadButton, StartupMode,
};
use serde::Serialize;

use crate::oracle::Oracle;

pub(super) const DATA_DIR: &str = "crates/gb-test-runner/data";
pub(super) const REPORTS_MANIFEST_PATH: &str = "crates/gb-test-runner/data/reports.toml";
pub(super) const TEST_ROM_STORE_DIR: &str = "test";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Report {
    pub(super) id: String,
    pub(super) local: bool,
    pub(super) store_dir: PathBuf,
    pub(super) sources: Option<PathBuf>,
    pub(super) status_dir: PathBuf,
    pub(super) artifact_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SuiteManifest {
    pub(super) suite_name: String,
    pub(super) family: String,
    pub(super) cases: Vec<SuiteCase>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SuiteCase {
    pub(super) id: String,
    pub(super) family: String,
    pub(super) rom: PathBuf,
    pub(super) target_root: PathBuf,
    pub(super) report_console: ReportConsole,
    pub(super) report_console_suffix: bool,
    pub(super) console_model: ConsoleModel,
    pub(super) hardware_revision: HardwareRevision,
    pub(super) host_platform: HostPlatform,
    pub(super) execution_mode: ExecutionMode,
    pub(super) startup_mode: StartupMode,
    pub(super) timeout_frames: u32,
    pub(super) stimuli: Vec<SuiteStimulus>,
    pub(super) oracle: Oracle,
}

impl SuiteCase {
    pub(super) fn report_rom(&self) -> String {
        let rom = self.rom.to_string_lossy();
        if self.report_console_suffix {
            format!("{rom} {}", self.report_console.report_suffix())
        } else {
            rom.into_owned()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReportConsole {
    Dmg,
    Cgb,
    Agb,
    Sgb,
    Sgb2,
}

impl ReportConsole {
    pub(super) const fn report_suffix(self) -> &'static str {
        match self {
            Self::Dmg => "(DMG)",
            Self::Cgb => "(GBC)",
            Self::Agb => "(AGB)",
            Self::Sgb => "(SGB)",
            Self::Sgb2 => "(SGB2)",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SuiteStimulus {
    pub(super) when: SuiteStimulusTime,
    pub(super) button: JoypadButton,
    pub(super) pressed: bool,
}

impl SuiteStimulus {
    pub(super) fn tcycle(&self) -> u64 {
        self.when.tcycle()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SuiteStimulusTime {
    TCycle(u64),
    Frame(u32),
}

impl SuiteStimulusTime {
    fn tcycle(self) -> u64 {
        match self {
            Self::TCycle(tcycle) => tcycle,
            Self::Frame(frame) => u64::from(frame).saturating_mul(DMG_T_CYCLES_PER_FRAME),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SuiteRunReport {
    pub(super) suite_name: String,
    pub(super) family: String,
    pub(super) cases: Vec<CaseRunReport>,
}

impl SuiteRunReport {
    pub(super) fn passed_count(&self) -> usize {
        self.cases.iter().filter(|case| case.passed).count()
    }

    pub(super) fn failed_count(&self) -> usize {
        self.cases.len().saturating_sub(self.passed_count())
    }

    pub(super) fn all_passed(&self) -> bool {
        self.failed_count() == 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CaseRunReport {
    pub(super) id: String,
    pub(super) rom: String,
    pub(super) passed: bool,
    pub(super) informational: bool,
    pub(super) failure: Option<String>,
    pub(super) executed_tcycles: u64,
    pub(super) failure_artifact_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct PersistedSuiteStatus {
    pub(super) suite_name: String,
    pub(super) family: String,
    pub(super) cases: Vec<PersistedCaseStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct PersistedCaseStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) family: Option<String>,
    pub(super) rom: String,
    pub(super) status: String,
}
