use std::path::PathBuf;

use gb_core::{
    ConsoleModel, DMG_T_CYCLES_PER_FRAME, ExecutionMode, HostPlatform, JoypadButton, StartupMode,
};
use serde::Serialize;

use crate::oracle::Oracle;

pub(super) const DATA_DIR: &str = "crates/gb-test-runner/data";
pub(super) const REPORTS_MANIFEST_PATH: &str = "crates/gb-test-runner/data/reports.toml";
pub(super) const TEST_ROM_STORE_DIR: &str = "test";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Report {
    pub(super) id: String,
    pub(super) store_dir: PathBuf,
    pub(super) sources: PathBuf,
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
    pub(super) console_model: ConsoleModel,
    pub(super) host_platform: HostPlatform,
    pub(super) execution_mode: ExecutionMode,
    pub(super) startup_mode: StartupMode,
    pub(super) timeout_frames: u32,
    pub(super) stimuli: Vec<SuiteStimulus>,
    pub(super) oracle: Oracle,
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
    pub(super) rom: PathBuf,
    pub(super) passed: bool,
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
