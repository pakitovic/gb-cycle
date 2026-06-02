use std::path::PathBuf;

use gb_core::{
    BootRomAssets, ConsoleModel, Dmg07Port, HardwareRevision, HostPlatform, StartupMode,
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
pub(super) struct LinkSuiteManifest {
    pub(super) suite_name: String,
    pub(super) family: String,
    pub(super) topology: LinkTopology,
    pub(super) cases: Vec<LinkSuiteCase>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LinkTopology {
    Dmg04,
    Dmg07,
    CgbIr,
}

impl LinkTopology {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Dmg04 => "dmg04",
            Self::Dmg07 => "dmg07",
            Self::CgbIr => "cgb-ir",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LinkSuiteCase {
    pub(super) id: String,
    pub(super) topology: LinkTopology,
    pub(super) timeout_tcycles: u64,
    pub(super) target_root: PathBuf,
    pub(super) participants: Vec<LinkParticipant>,
    pub(super) oracle: Oracle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LinkParticipant {
    pub(super) id: String,
    pub(super) rom: PathBuf,
    pub(super) console_model: ConsoleModel,
    pub(super) hardware_revision: HardwareRevision,
    pub(super) host_platform: HostPlatform,
    pub(super) startup_mode: StartupMode,
    pub(super) adapter_port: Option<Dmg07Port>,
}

#[derive(Clone, Default)]
pub(super) struct LinkRunConfig {
    pub(super) boot_rom_assets: Option<BootRomAssets>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LinkSuiteRunReport {
    pub(super) suite_name: String,
    pub(super) family: String,
    pub(super) cases: Vec<LinkCaseRunReport>,
}

impl LinkSuiteRunReport {
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
pub(super) struct LinkCaseRunReport {
    pub(super) id: String,
    pub(super) passed: bool,
    pub(super) informational: bool,
    pub(super) failure: Option<String>,
    pub(super) executed_tcycles: u64,
    pub(super) participants: Vec<LinkParticipantRunReport>,
    pub(super) failure_artifact_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LinkParticipantRunReport {
    pub(super) id: String,
    pub(super) rom: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LinkRunArtifacts {
    pub(super) session_snapshot: Option<String>,
    pub(super) session_trace: Option<String>,
    pub(super) topology_trace: Option<String>,
    pub(super) participants: Vec<LinkParticipantArtifacts>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LinkParticipantArtifacts {
    pub(super) id: String,
    pub(super) serial: Vec<u8>,
    pub(super) serial_hex: String,
    pub(super) snapshot: Option<String>,
    pub(super) trace: Option<String>,
    pub(super) dmg_framebuffer: Vec<u8>,
    pub(super) cgb_framebuffer: Option<Vec<u16>>,
    pub(super) in_vblank: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct PersistedLinkSuiteStatus {
    pub(super) suite_name: String,
    pub(super) family: String,
    pub(super) cases: Vec<PersistedLinkCaseStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct PersistedLinkCaseStatus {
    pub(super) id: String,
    pub(super) status: String,
    pub(super) participants: Vec<PersistedLinkParticipantStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct PersistedLinkParticipantStatus {
    pub(super) id: String,
    pub(super) rom: String,
}
