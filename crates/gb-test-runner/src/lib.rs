mod boot_rom;
mod boot_rom_verification;
#[cfg(test)]
mod determinism;
mod fetch;
mod framebuffer_oracle;
mod oracle;
mod rtc;
mod suite;
mod suite_link;
#[cfg(test)]
mod test_support;
mod workspace_paths;

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{fs, io};

use framebuffer_oracle::{
    GBEMU_SHOOTOUT_GRAYSCALE_TOLERANCE, NormalizedFramebuffer, convert_pgm_to_png,
    decode_fixture_framebuffer_path, decode_fixture_grayscale_framebuffer_path,
    decode_local_pgm_framebuffer, decode_local_pgm_grayscale_framebuffer,
    decode_local_rgb555_framebuffer, decode_local_rgb555_grayscale_framebuffer,
    encode_framebuffer_pgm, encode_rgb555_framebuffer_png,
    grayscale_framebuffers_match_with_tolerance, normalize_dmg_framebuffer,
};
use gb_core::{
    BootRomAssetError, BootRomAssetKind, BootRomAssets, CartridgeDiagnostic, CartridgeLoadError,
    CgbSpeedMode, CompatibilityPolicy, ConsoleModel, CpuBusAccessKind, CpuDiagnosticTrap,
    CpuExecutionState, CpuSnapshot, ExecutionMode, HardwareRevision, HostPlatform, JoypadButton,
    Machine, MachineConfig, StartupMode, TimerStartupState, TraceBuffer, TraceSummaryBuffer,
};
#[cfg(test)]
use gb_core::{MachineSaveState, MachineSaveStateRestoreError};
use rayon::prelude::*;

pub use boot_rom_verification::{
    BootRomVerificationIssue, BootRomVerificationMode, enforce_boot_rom_asset_verification,
    enforce_boot_rom_verification, expected_boot_rom_asset_sha256, expected_boot_rom_asset_size,
    expected_boot_rom_sha256, expected_boot_rom_size, verify_boot_rom_asset_file,
    verify_boot_rom_file,
};
pub use fetch::{fetch_help_text, run_fetch_command};
pub use suite::{run_suite_command, suite_help_text};
pub use suite_link::{run_suite_link_command, suite_link_help_text};
pub use workspace_paths::{
    BOOT_ROM_ROOT_ENV_VAR, boot_rom_asset_for_console_profile, boot_rom_image_path,
    boot_rom_revision_for_console_model, discover_boot_rom_root,
};

pub const TEST_ROM_STORE_DIR: &str = "test";
pub const TEST_ROM_ROOT_ENV_VAR: &str = "GB_CYCLE_TEST_ROM_ROOT";
pub const GB_EMULATOR_SHOOTOUT_REPORT_ID: &str = "gb-emulator-shootout";
pub const DOCBOY_REPORT_ID: &str = "docboy";
pub const GBMICROTEST_REPORT_ID: &str = "gbmicrotest";

fn test_rom_store_root(workspace_root: &Path) -> PathBuf {
    workspace_root.join(TEST_ROM_STORE_DIR)
}

fn discover_test_rom_store_root(workspace_root: &Path) -> Option<PathBuf> {
    if let Some(root) = std::env::var_os(TEST_ROM_ROOT_ENV_VAR) {
        return Some(PathBuf::from(root));
    }

    let default_root = test_rom_store_root(workspace_root);
    default_root.exists().then_some(default_root)
}

pub(crate) fn boot_rom_asset_is_required_for_runner_gate(asset: BootRomAssetKind) -> bool {
    matches!(
        asset,
        BootRomAssetKind::Dmg
            | BootRomAssetKind::Mgb
            | BootRomAssetKind::Sgb
            | BootRomAssetKind::Sgb2
            | BootRomAssetKind::Cgb
            | BootRomAssetKind::CgbE
    )
}

pub(crate) fn enforce_missing_boot_rom_root_verification(
    mode: BootRomVerificationMode,
    asset: BootRomAssetKind,
) -> Result<(), BootRomVerificationIssue> {
    match mode {
        BootRomVerificationMode::Off => Ok(()),
        BootRomVerificationMode::Warn => {
            eprintln!(
                "warning: {}",
                BootRomVerificationIssue::MissingRoot {
                    asset,
                    env_var: BOOT_ROM_ROOT_ENV_VAR,
                }
            );
            Ok(())
        }
        BootRomVerificationMode::Strict => Err(BootRomVerificationIssue::MissingRoot {
            asset,
            env_var: BOOT_ROM_ROOT_ENV_VAR,
        }),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CaptureKind {
    Serial,
    SerialHex,
    MemoryBytes,
    MemoryTextOutput,
    BlarggConsoleText,
    Framebuffer,
    Trace,
    Snapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InformationalCaptureKind {
    Serial,
    SerialHex,
    Framebuffer,
    Trace,
    Snapshot,
}

impl InformationalCaptureKind {
    pub const fn capture_kind(self) -> CaptureKind {
        match self {
            Self::Serial => CaptureKind::Serial,
            Self::SerialHex => CaptureKind::SerialHex,
            Self::Framebuffer => CaptureKind::Framebuffer,
            Self::Trace => CaptureKind::Trace,
            Self::Snapshot => CaptureKind::Snapshot,
        }
    }
}

const DMG_FAMILY_FRAME_T_CYCLES: u64 = 70_224;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Timeout {
    TCycles(u64),
    Frames(u32),
}

impl Timeout {
    pub fn is_valid(self) -> bool {
        match self {
            Self::TCycles(limit) => limit > 0,
            Self::Frames(limit) => limit > 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StimulusTime {
    TCycle(u64),
    Frame(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExternalStimulusAction {
    JoypadSetButton { button: JoypadButton, pressed: bool },
    WriteMemory { address: u16, value: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StartupMemoryWrite {
    pub address: u16,
    pub value: u8,
}

impl StartupMemoryWrite {
    pub const fn new(address: u16, value: u8) -> Self {
        Self { address, value }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExternalStimulus {
    pub when: StimulusTime,
    pub action: ExternalStimulusAction,
}

impl ExternalStimulus {
    pub const fn at_t_cycle(t_cycle: u64, action: ExternalStimulusAction) -> Self {
        Self {
            when: StimulusTime::TCycle(t_cycle),
            action,
        }
    }

    pub const fn at_frame(frame: u32, action: ExternalStimulusAction) -> Self {
        Self {
            when: StimulusTime::Frame(frame),
            action,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExternalStimulusPlan {
    stimuli: Vec<ExternalStimulus>,
}

impl ExternalStimulusPlan {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_stimulus(mut self, stimulus: ExternalStimulus) -> Self {
        self.stimuli.push(stimulus);
        self
    }

    pub fn contains(&self, stimulus: ExternalStimulus) -> bool {
        self.stimuli.contains(&stimulus)
    }

    pub fn stimuli(&self) -> &[ExternalStimulus] {
        &self.stimuli
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecutionStopCondition {
    MemoryEquals { address: u16, value: u8 },
    CurrentOpcodeEquals { opcode: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MemoryByteExpectation {
    pub address: u16,
    pub value: u8,
    pub fail_value: Option<u8>,
}

impl MemoryByteExpectation {
    pub const fn new(address: u16, value: u8) -> Self {
        Self {
            address,
            value,
            fail_value: None,
        }
    }

    pub const fn with_fail_value(address: u16, value: u8, fail_value: u8) -> Self {
        Self {
            address,
            value,
            fail_value: Some(fail_value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CapturedMemoryByte {
    pub address: u16,
    pub expected: u8,
    pub fail_value: Option<u8>,
    pub actual: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapturedMemoryBytes {
    pub bytes: Vec<CapturedMemoryByte>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MemoryTextOutputSpec {
    pub status_address: u16,
    pub running_status: u8,
    pub pass_status: u8,
    pub signature_address: u16,
    pub expected_signature: [u8; 3],
    pub text_address: u16,
    pub max_text_bytes: usize,
}

impl MemoryTextOutputSpec {
    pub const fn new(
        status_address: u16,
        running_status: u8,
        pass_status: u8,
        signature_address: u16,
        expected_signature: [u8; 3],
        text_address: u16,
        max_text_bytes: usize,
    ) -> Self {
        Self {
            status_address,
            running_status,
            pass_status,
            signature_address,
            expected_signature,
            text_address,
            max_text_bytes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PassCondition {
    SerialExact(String),
    SerialContains(String),
    SerialHexExact(String),
    MemoryBytesEqual(Vec<MemoryByteExpectation>),
    MemoryTextOutputContains {
        spec: MemoryTextOutputSpec,
        expected_substring: String,
    },
    BlarggConsoleTextContains(String),
    MooneyeResult,
    Informational(InformationalCaptureKind),
    FramebufferFixture(PathBuf),
    FramebufferFixtureUntilMatch {
        fixture_path: PathBuf,
        check_interval_tcycles: u64,
        check_at_tcycles: Option<u64>,
    },
    FramebufferGrayscaleFixture(PathBuf),
    FramebufferRgb555Fixture(PathBuf),
    FramebufferRgb555FixtureUntilMatch {
        fixture_path: PathBuf,
        check_interval_tcycles: u64,
        check_at_tcycles: Option<u64>,
    },
    FramebufferRgb555GrayscaleFixture(PathBuf),
    FramebufferRgb555GrayscaleToleranceFixture(PathBuf),
    FramebufferFixtureSet(Vec<PathBuf>),
    TraceFixture(PathBuf),
}

impl PassCondition {
    pub fn required_capture(&self) -> CaptureKind {
        match self {
            Self::SerialExact(_) | Self::SerialContains(_) => CaptureKind::Serial,
            Self::SerialHexExact(_) => CaptureKind::SerialHex,
            Self::MemoryBytesEqual(_) => CaptureKind::MemoryBytes,
            Self::MemoryTextOutputContains { .. } => CaptureKind::MemoryTextOutput,
            Self::BlarggConsoleTextContains(_) => CaptureKind::BlarggConsoleText,
            Self::MooneyeResult => CaptureKind::Snapshot,
            Self::Informational(capture) => capture.capture_kind(),
            Self::FramebufferFixture(_)
            | Self::FramebufferFixtureUntilMatch { .. }
            | Self::FramebufferGrayscaleFixture(_)
            | Self::FramebufferRgb555Fixture(_)
            | Self::FramebufferRgb555FixtureUntilMatch { .. }
            | Self::FramebufferRgb555GrayscaleFixture(_)
            | Self::FramebufferRgb555GrayscaleToleranceFixture(_)
            | Self::FramebufferFixtureSet(_) => CaptureKind::Framebuffer,
            Self::TraceFixture(_) => CaptureKind::Trace,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapturePlan {
    captures: BTreeSet<CaptureKind>,
}

impl CapturePlan {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn debugging_minimum_for(pass_condition: &PassCondition) -> Self {
        Self::new()
            .with_capture(pass_condition.required_capture())
            .with_capture(CaptureKind::Trace)
            .with_capture(CaptureKind::Snapshot)
    }

    pub fn with_capture(mut self, capture: CaptureKind) -> Self {
        self.captures.insert(capture);
        self
    }

    pub fn contains(&self, capture: CaptureKind) -> bool {
        self.captures.contains(&capture)
    }

    pub fn captures(&self) -> &BTreeSet<CaptureKind> {
        &self.captures
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FailureArtifactPolicy {
    retained: BTreeSet<CaptureKind>,
}

impl FailureArtifactPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn debugging_minimum_for(pass_condition: &PassCondition) -> Self {
        Self::new()
            .with_artifact(pass_condition.required_capture())
            .with_artifact(CaptureKind::Trace)
            .with_artifact(CaptureKind::Snapshot)
    }

    pub fn with_artifact(mut self, artifact: CaptureKind) -> Self {
        self.retained.insert(artifact);
        self
    }

    pub fn contains(&self, artifact: CaptureKind) -> bool {
        self.retained.contains(&artifact)
    }

    pub fn retained(&self) -> &BTreeSet<CaptureKind> {
        &self.retained
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RomCaseValidationError {
    EmptyCaseId,
    MissingRomPath,
    InvalidTimeout,
    MissingRequiredCapture(CaptureKind),
    MissingRequiredFailureArtifact(CaptureKind),
    ArtifactNotCaptured(CaptureKind),
    MissingFailureArtifacts,
    InvalidFramebufferCheckInterval,
    FramebufferCheckAtExceedsTimeout {
        check_at_tcycles: u64,
        timeout_tcycles: u64,
    },
    DuplicateExternalStimulus(ExternalStimulus),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomTestCase {
    pub id: String,
    pub rom_path: PathBuf,
    pub report_id: Option<String>,
    pub console_model: ConsoleModel,
    pub host_platform: HostPlatform,
    pub revision: HardwareRevision,
    pub startup_mode: StartupMode,
    pub execution_mode: ExecutionMode,
    pub startup_cartridge_rtc_seconds: Option<u64>,
    pub startup_timer_state: Option<TimerStartupState>,
    pub startup_memory_writes: Vec<StartupMemoryWrite>,
    pub external_stimuli: ExternalStimulusPlan,
    pub stop_condition: Option<ExecutionStopCondition>,
    pub timeout: Timeout,
    pub pass_condition: PassCondition,
    pub capture_plan: CapturePlan,
    pub failure_artifacts: FailureArtifactPolicy,
}

impl RomTestCase {
    pub fn new(
        id: impl Into<String>,
        rom_path: impl Into<PathBuf>,
        timeout: Timeout,
        pass_condition: PassCondition,
    ) -> Self {
        let capture_plan = CapturePlan::debugging_minimum_for(&pass_condition);
        let failure_artifacts = FailureArtifactPolicy::debugging_minimum_for(&pass_condition);

        Self {
            id: id.into(),
            rom_path: rom_path.into(),
            report_id: None,
            console_model: ConsoleModel::GameBoy,
            host_platform: HostPlatform::Handheld,
            revision: ConsoleModel::GameBoy.default_revision(),
            startup_mode: StartupMode::SkipBoot,
            execution_mode: ExecutionMode::Strict,
            startup_cartridge_rtc_seconds: None,
            startup_timer_state: None,
            startup_memory_writes: Vec::new(),
            external_stimuli: ExternalStimulusPlan::new(),
            stop_condition: None,
            timeout,
            pass_condition,
            capture_plan,
            failure_artifacts,
        }
    }

    pub fn with_console_model(mut self, console_model: ConsoleModel) -> Self {
        self.console_model = console_model;
        self.revision = console_model.default_revision();
        self
    }

    pub fn with_host_platform(mut self, host_platform: HostPlatform) -> Self {
        self.host_platform = host_platform;
        self
    }

    pub fn with_revision(mut self, revision: HardwareRevision) -> Self {
        self.revision = revision;
        self
    }

    pub fn with_startup_mode(mut self, startup_mode: StartupMode) -> Self {
        self.startup_mode = startup_mode;
        self
    }

    pub fn with_execution_mode(mut self, execution_mode: ExecutionMode) -> Self {
        self.execution_mode = execution_mode;
        self
    }

    pub fn with_startup_cartridge_rtc_seconds(mut self, seconds: u64) -> Self {
        self.startup_cartridge_rtc_seconds = Some(seconds);
        self
    }

    pub fn with_startup_timer_state(mut self, startup_timer_state: TimerStartupState) -> Self {
        self.startup_timer_state = Some(startup_timer_state);
        self
    }

    pub fn with_startup_memory_write(mut self, write: StartupMemoryWrite) -> Self {
        self.startup_memory_writes.push(write);
        self
    }

    pub fn with_startup_memory_writes(
        mut self,
        writes: impl IntoIterator<Item = StartupMemoryWrite>,
    ) -> Self {
        self.startup_memory_writes.extend(writes);
        self
    }

    pub fn with_report_id(mut self, report_id: impl Into<String>) -> Self {
        self.report_id = Some(report_id.into());
        self
    }

    pub fn with_external_stimuli(mut self, external_stimuli: ExternalStimulusPlan) -> Self {
        self.external_stimuli = external_stimuli;
        self
    }

    pub fn with_external_stimulus(mut self, stimulus: ExternalStimulus) -> Self {
        self.external_stimuli = self.external_stimuli.with_stimulus(stimulus);
        self
    }

    pub fn with_stop_condition(mut self, stop_condition: ExecutionStopCondition) -> Self {
        self.stop_condition = Some(stop_condition);
        self
    }

    pub fn with_capture_plan(mut self, capture_plan: CapturePlan) -> Self {
        self.capture_plan = capture_plan;
        self
    }

    pub fn with_failure_artifacts(mut self, failure_artifacts: FailureArtifactPolicy) -> Self {
        self.failure_artifacts = failure_artifacts;
        self
    }

    pub fn validate(&self) -> Result<(), RomCaseValidationError> {
        if self.id.trim().is_empty() {
            return Err(RomCaseValidationError::EmptyCaseId);
        }

        if self.rom_path.as_os_str().is_empty() {
            return Err(RomCaseValidationError::MissingRomPath);
        }

        if !self.timeout.is_valid() {
            return Err(RomCaseValidationError::InvalidTimeout);
        }

        if let PassCondition::FramebufferFixtureUntilMatch {
            check_interval_tcycles,
            check_at_tcycles,
            ..
        }
        | PassCondition::FramebufferRgb555FixtureUntilMatch {
            check_interval_tcycles,
            check_at_tcycles,
            ..
        } = &self.pass_condition
        {
            if *check_interval_tcycles == 0 {
                return Err(RomCaseValidationError::InvalidFramebufferCheckInterval);
            }

            if let Some(check_at_tcycles) = *check_at_tcycles
                && let Timeout::TCycles(timeout_tcycles) = self.timeout
                && timeout_tcycles < check_at_tcycles
            {
                return Err(RomCaseValidationError::FramebufferCheckAtExceedsTimeout {
                    check_at_tcycles,
                    timeout_tcycles,
                });
            }
        }

        let required_capture = self.pass_condition.required_capture();
        if !self.capture_plan.contains(required_capture) {
            return Err(RomCaseValidationError::MissingRequiredCapture(
                required_capture,
            ));
        }

        if self.failure_artifacts.retained().is_empty() {
            return Err(RomCaseValidationError::MissingFailureArtifacts);
        }

        if !self.failure_artifacts.contains(required_capture) {
            return Err(RomCaseValidationError::MissingRequiredFailureArtifact(
                required_capture,
            ));
        }

        for artifact in self.failure_artifacts.retained() {
            if !self.capture_plan.contains(*artifact) {
                return Err(RomCaseValidationError::ArtifactNotCaptured(*artifact));
            }
        }

        for (index, stimulus) in self.external_stimuli.stimuli().iter().enumerate() {
            if self.external_stimuli.stimuli()[index + 1..].contains(stimulus) {
                return Err(RomCaseValidationError::DuplicateExternalStimulus(*stimulus));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RomSuiteValidationError {
    EmptySuiteName,
    DuplicateCaseId(String),
    InvalidCase {
        case_id: String,
        error: RomCaseValidationError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomSuite {
    pub name: String,
    pub family: Option<String>,
    pub report_id: Option<String>,
    pub cases: Vec<RomTestCase>,
}

impl RomSuite {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            family: None,
            report_id: None,
            cases: Vec::new(),
        }
    }

    pub fn with_family(mut self, family: impl Into<String>) -> Self {
        self.family = Some(family.into());
        self
    }

    pub fn with_report_id(mut self, report_id: impl Into<String>) -> Self {
        self.report_id = Some(report_id.into());
        self
    }

    pub fn with_case(mut self, case: RomTestCase) -> Self {
        self.cases.push(case);
        self
    }

    pub fn push_case(&mut self, case: RomTestCase) {
        self.cases.push(case);
    }

    pub fn validate(&self) -> Result<(), RomSuiteValidationError> {
        if self.name.trim().is_empty() {
            return Err(RomSuiteValidationError::EmptySuiteName);
        }

        let mut seen_case_ids = BTreeSet::new();
        for case in &self.cases {
            if !seen_case_ids.insert(case.id.clone()) {
                return Err(RomSuiteValidationError::DuplicateCaseId(case.id.clone()));
            }

            if let Err(error) = case.validate() {
                return Err(RomSuiteValidationError::InvalidCase {
                    case_id: case.id.clone(),
                    error,
                });
            }
        }

        Ok(())
    }
}

fn phase_2_rom_path(name: &str) -> PathBuf {
    PathBuf::from("crates/gb-core/tests/fixtures/roms/phase2").join(name)
}

fn phase_2_trace_path(name: &str) -> PathBuf {
    PathBuf::from("crates/gb-core/tests/fixtures/traces/phase2").join(name)
}

fn phase_4_rom_path(name: &str) -> PathBuf {
    PathBuf::from("crates/gb-core/tests/fixtures/roms/phase4").join(name)
}

fn phase_4_trace_path(name: &str) -> PathBuf {
    PathBuf::from("crates/gb-core/tests/fixtures/traces/phase4").join(name)
}

fn phase_6_rom_path(name: &str) -> PathBuf {
    PathBuf::from("crates/gb-core/tests/fixtures/roms/phase6").join(name)
}

const PHASE_SENTINEL_ADDRESS: u16 = 0xC010;
const PHASE_SENTINEL_VALUE: u8 = 0xA5;
const PHASE_6_MBC3_STARTUP_RTC_SECONDS: u64 = 93_784;

pub fn phase_2_cpu_timing_suite() -> RomSuite {
    RomSuite::new("phase-2-cpu-timing")
        .with_case(
            RomTestCase::new(
                "phase2-fetch-immediate-order",
                phase_2_rom_path("phase2_fetch_immediate_order.gb"),
                Timeout::TCycles(256),
                PassCondition::TraceFixture(phase_2_trace_path(
                    "phase2_fetch_immediate_order.trace",
                )),
            )
            .with_stop_condition(ExecutionStopCondition::MemoryEquals {
                address: PHASE_SENTINEL_ADDRESS,
                value: PHASE_SENTINEL_VALUE,
            }),
        )
        .with_case(
            RomTestCase::new(
                "phase2-control-flow-stack-cb",
                phase_2_rom_path("phase2_control_flow_stack_cb.gb"),
                Timeout::TCycles(512),
                PassCondition::TraceFixture(phase_2_trace_path(
                    "phase2_control_flow_stack_cb.trace",
                )),
            )
            .with_stop_condition(ExecutionStopCondition::MemoryEquals {
                address: PHASE_SENTINEL_ADDRESS,
                value: PHASE_SENTINEL_VALUE,
            }),
        )
}

pub fn phase_2_interrupt_timing_suite() -> RomSuite {
    const PHASE_2_HALT_STOP_WAKE_T_CYCLE: u64 = 356;
    const PHASE_2_HALT_STOP_IF_INJECT_T_CYCLE: u64 = 357;

    RomSuite::new("phase-2-interrupt-timing")
        .with_case(
            RomTestCase::new(
                "phase2-ei-delay-priority",
                phase_2_rom_path("phase2_ei_delay_priority.gb"),
                Timeout::TCycles(256),
                PassCondition::TraceFixture(phase_2_trace_path("phase2_ei_delay_priority.trace")),
            )
            .with_stop_condition(ExecutionStopCondition::MemoryEquals {
                address: PHASE_SENTINEL_ADDRESS,
                value: PHASE_SENTINEL_VALUE,
            }),
        )
        .with_case(
            RomTestCase::new(
                "phase2-timer-if-visibility-and-service",
                phase_2_rom_path("phase2_timer_if_visibility_and_service.gb"),
                Timeout::TCycles(512),
                PassCondition::TraceFixture(phase_2_trace_path(
                    "phase2_timer_if_visibility_and_service.trace",
                )),
            )
            .with_stop_condition(ExecutionStopCondition::MemoryEquals {
                address: PHASE_SENTINEL_ADDRESS,
                value: PHASE_SENTINEL_VALUE,
            }),
        )
        .with_case(
            RomTestCase::new(
                "phase2-halt-stop-and-halt-bug",
                phase_2_rom_path("phase2_halt_stop_and_halt_bug.gb"),
                Timeout::TCycles(512),
                PassCondition::TraceFixture(phase_2_trace_path(
                    "phase2_halt_stop_and_halt_bug.trace",
                )),
            )
            .with_external_stimulus(ExternalStimulus::at_t_cycle(
                PHASE_2_HALT_STOP_WAKE_T_CYCLE,
                ExternalStimulusAction::JoypadSetButton {
                    button: JoypadButton::A,
                    pressed: true,
                },
            ))
            .with_external_stimulus(ExternalStimulus::at_t_cycle(
                PHASE_2_HALT_STOP_IF_INJECT_T_CYCLE,
                ExternalStimulusAction::WriteMemory {
                    address: 0xFF0F,
                    value: 0x01,
                },
            ))
            .with_stop_condition(ExecutionStopCondition::MemoryEquals {
                address: PHASE_SENTINEL_ADDRESS,
                value: PHASE_SENTINEL_VALUE,
            }),
        )
}

pub fn phase_4_ppu_oam_corruption_suite() -> RomSuite {
    RomSuite::new("phase-4-ppu-oam-corruption")
        .with_case(
            RomTestCase::new(
                "phase4-oam-direct-mode2-oam-access",
                phase_4_rom_path("phase4_oam_bug_direct_mode2_oam_access.gb"),
                Timeout::TCycles(1_024),
                PassCondition::TraceFixture(phase_4_trace_path(
                    "phase4_oam_bug_direct_mode2_oam_access.trace",
                )),
            )
            .with_stop_condition(ExecutionStopCondition::MemoryEquals {
                address: PHASE_SENTINEL_ADDRESS,
                value: PHASE_SENTINEL_VALUE,
            }),
        )
        .with_case(
            RomTestCase::new(
                "phase4-oam-fea0-mode2-read",
                phase_4_rom_path("phase4_oam_bug_fea0_mode2_read.gb"),
                Timeout::TCycles(1_024),
                PassCondition::TraceFixture(phase_4_trace_path(
                    "phase4_oam_bug_fea0_mode2_read.trace",
                )),
            )
            .with_stop_condition(ExecutionStopCondition::MemoryEquals {
                address: PHASE_SENTINEL_ADDRESS,
                value: PHASE_SENTINEL_VALUE,
            }),
        )
        .with_case(
            RomTestCase::new(
                "phase4-oam-inc-hl-dmg0",
                phase_4_rom_path("phase4_oam_bug_inc_hl.gb"),
                Timeout::TCycles(1_024),
                PassCondition::TraceFixture(phase_4_trace_path("phase4_oam_bug_inc_hl_dmg0.trace")),
            )
            .with_console_model(ConsoleModel::GameBoy)
            .with_stop_condition(ExecutionStopCondition::MemoryEquals {
                address: PHASE_SENTINEL_ADDRESS,
                value: PHASE_SENTINEL_VALUE,
            }),
        )
        .with_case(
            RomTestCase::new(
                "phase4-oam-inc-hl-dmg",
                phase_4_rom_path("phase4_oam_bug_inc_hl.gb"),
                Timeout::TCycles(1_024),
                PassCondition::TraceFixture(phase_4_trace_path("phase4_oam_bug_inc_hl_dmg.trace")),
            )
            .with_stop_condition(ExecutionStopCondition::MemoryEquals {
                address: PHASE_SENTINEL_ADDRESS,
                value: PHASE_SENTINEL_VALUE,
            }),
        )
        .with_case(
            RomTestCase::new(
                "phase4-oam-inc-hl-mgb",
                phase_4_rom_path("phase4_oam_bug_inc_hl.gb"),
                Timeout::TCycles(1_024),
                PassCondition::TraceFixture(phase_4_trace_path("phase4_oam_bug_inc_hl_mgb.trace")),
            )
            .with_console_model(ConsoleModel::GameBoyPocket)
            .with_stop_condition(ExecutionStopCondition::MemoryEquals {
                address: PHASE_SENTINEL_ADDRESS,
                value: PHASE_SENTINEL_VALUE,
            }),
        )
        .with_case(
            RomTestCase::new(
                "phase4-oam-hli-hld",
                phase_4_rom_path("phase4_oam_bug_hli_hld.gb"),
                Timeout::TCycles(1_536),
                PassCondition::TraceFixture(phase_4_trace_path("phase4_oam_bug_hli_hld.trace")),
            )
            .with_stop_condition(ExecutionStopCondition::MemoryEquals {
                address: PHASE_SENTINEL_ADDRESS,
                value: PHASE_SENTINEL_VALUE,
            }),
        )
        .with_case(
            RomTestCase::new(
                "phase4-oam-stack-and-interrupt-service",
                phase_4_rom_path("phase4_oam_bug_stack_and_interrupt_service.gb"),
                Timeout::TCycles(2_048),
                PassCondition::TraceFixture(phase_4_trace_path(
                    "phase4_oam_bug_stack_and_interrupt_service.trace",
                )),
            )
            .with_stop_condition(ExecutionStopCondition::MemoryEquals {
                address: PHASE_SENTINEL_ADDRESS,
                value: PHASE_SENTINEL_VALUE,
            }),
        )
        .with_case(
            RomTestCase::new(
                "phase4-oam-cgb-negative",
                phase_4_rom_path("phase4_oam_bug_inc_hl.gb"),
                Timeout::TCycles(1_024),
                PassCondition::TraceFixture(phase_4_trace_path("phase4_oam_bug_inc_hl_cgb.trace")),
            )
            .with_console_model(ConsoleModel::GameBoyColor)
            .with_stop_condition(ExecutionStopCondition::MemoryEquals {
                address: PHASE_SENTINEL_ADDRESS,
                value: PHASE_SENTINEL_VALUE,
            }),
        )
}

pub fn phase_6_cartridge_oracle_suite() -> RomSuite {
    RomSuite::new("phase-6-cartridge-oracle")
        .with_case(RomTestCase::new(
            "phase6-mbc1-standard-banking",
            phase_6_rom_path("phase6_mbc1_standard_banking.gb"),
            Timeout::TCycles(200_000),
            PassCondition::SerialHexExact("4D31533A011F1122".to_string()),
        ))
        .with_case(RomTestCase::new(
            "phase6-mbc1-small-rom-mask-and-ram",
            phase_6_rom_path("phase6_mbc1_small_rom_mask_and_ram.gb"),
            Timeout::TCycles(200_000),
            PassCondition::SerialHexExact("4D314D3A01003344".to_string()),
        ))
        .with_case(RomTestCase::new(
            "phase6-mbc2-control-decode-and-nibble-ram",
            phase_6_rom_path("phase6_mbc2_control_decode_and_nibble_ram.gb"),
            Timeout::TCycles(200_000),
            PassCondition::SerialHexExact("4D323A0103FBFBFB".to_string()),
        ))
        .with_case(
            RomTestCase::new(
                "phase6-mbc3-banking-ram-and-rtc",
                phase_6_rom_path("phase6_mbc3_banking_ram_and_rtc.gb"),
                Timeout::TCycles(200_000),
                PassCondition::SerialHexExact("4D333A01204060335504030201042A".to_string()),
            )
            .with_startup_cartridge_rtc_seconds(PHASE_6_MBC3_STARTUP_RTC_SECONDS),
        )
        .with_case(RomTestCase::new(
            "phase6-mbc5-rom-banking-rumble-and-ram",
            phase_6_rom_path("phase6_mbc5_rom_banking_rumble_and_ram.gb"),
            Timeout::TCycles(200_000),
            PassCondition::SerialHexExact("4D353A0100FF000001331133".to_string()),
        ))
}

pub fn phase_6_mbc6_oracle_suite() -> RomSuite {
    RomSuite::new("phase-6-mbc6-oracle").with_case(
        RomTestCase::new(
            "phase6-mbc6-split-window-flash",
            phase_6_rom_path("phase6_mbc6_split_window_flash.gb"),
            Timeout::TCycles(300_000),
            PassCondition::SerialHexExact("4D363A020304050011223344C281805A803C".to_string()),
        )
        .with_console_model(ConsoleModel::GameBoyColor),
    )
}

pub fn built_in_rom_suites() -> Vec<RomSuite> {
    vec![
        phase_2_cpu_timing_suite(),
        phase_2_interrupt_timing_suite(),
        phase_4_ppu_oam_corruption_suite(),
        phase_6_cartridge_oracle_suite(),
    ]
}

pub fn built_in_rom_suite_by_name(name: &str) -> Option<RomSuite> {
    built_in_rom_suites()
        .into_iter()
        .find(|suite| suite.name == name)
}

#[derive(Debug)]
pub enum RomExecutionError {
    InvalidCase(RomCaseValidationError),
    InvalidSuite(RomSuiteValidationError),
    BootRomAssets {
        path: PathBuf,
        source: BootRomAssetError,
    },
    BootRomVerification {
        issue: BootRomVerificationIssue,
    },
    ReadFile {
        path: PathBuf,
        operation: &'static str,
        source: io::Error,
    },
    CreateDirectory {
        path: PathBuf,
        source: io::Error,
    },
    CartridgeLoad {
        path: PathBuf,
        source: CartridgeLoadError,
    },
    MissingExternalRomRoot {
        key: String,
        relative_path: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RomCaseFailure {
    TimeoutExceeded,
    RealBootHandoffTimeout {
        t_cycle_limit: u64,
    },
    CpuDiagnosticTrap {
        trap: CpuDiagnosticTrap,
    },
    SerialExactMismatch {
        expected: String,
        actual: String,
    },
    SerialMissingSubstring {
        expected_substring: String,
        actual: String,
    },
    MemoryTextOutputMismatch {
        expected_substring: String,
        pass_status: u8,
        expected_signature: [u8; 3],
        actual_status: u8,
        actual_signature: [u8; 3],
        actual_text: String,
    },
    MemoryByteMismatch {
        bytes: Vec<CapturedMemoryByte>,
    },
    BlarggConsoleTextMissingSubstring {
        expected_substring: String,
        actual: String,
    },
    MooneyeFailureSignature,
    MooneyeResultNotReached,
    TraceFixtureMismatch {
        fixture_path: PathBuf,
    },
    FramebufferFixtureMismatch {
        fixture_path: PathBuf,
    },
    FramebufferCheckAtNotReached {
        check_at_tcycles: u64,
        executed_t_cycles: u64,
    },
    FramebufferFixtureSetMismatch {
        fixture_paths: Vec<PathBuf>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RomCaseOutcome {
    Passed,
    Informational,
    Failed(RomCaseFailure),
}

impl RomCaseOutcome {
    pub fn failed(&self) -> bool {
        matches!(self, Self::Failed(_))
    }

    pub fn non_failing(&self) -> bool {
        !self.failed()
    }

    pub fn report_status(&self) -> &'static str {
        match self {
            Self::Passed => "PASS",
            Self::Informational => "INFO",
            Self::Failed(_) => "FAIL",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapturedMemoryTextOutput {
    pub status: u8,
    pub signature: [u8; 3],
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapturedArtifacts {
    pub serial: Option<String>,
    pub serial_hex: Option<String>,
    pub memory_bytes: Option<CapturedMemoryBytes>,
    pub memory_text_output: Option<CapturedMemoryTextOutput>,
    pub blargg_console_text: Option<String>,
    pub framebuffer_pgm: Option<Vec<u8>>,
    pub framebuffer_rgb555: Option<Vec<u16>>,
    pub trace: Option<String>,
    pub snapshot_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomCaseReport {
    pub case_id: String,
    pub rom_path: PathBuf,
    pub outcome: RomCaseOutcome,
    pub executed_t_cycles: u64,
    pub completed_frames: u32,
    pub diagnostics: Vec<CartridgeDiagnostic>,
    pub artifacts: CapturedArtifacts,
    pub retained_failure_artifacts: Vec<PathBuf>,
}

impl RomCaseReport {
    pub fn passed(&self) -> bool {
        matches!(self.outcome, RomCaseOutcome::Passed)
    }

    pub fn non_failing(&self) -> bool {
        self.outcome.non_failing()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomSuiteReport {
    pub suite_name: String,
    pub family: Option<String>,
    pub cases: Vec<RomCaseReport>,
}

impl RomSuiteReport {
    pub fn all_passed(&self) -> bool {
        self.cases.iter().all(RomCaseReport::passed)
    }

    pub fn all_non_failing(&self) -> bool {
        self.cases.iter().all(RomCaseReport::non_failing)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomRunner {
    workspace_root: PathBuf,
    failure_artifact_root: Option<PathBuf>,
    boot_rom_root: Option<PathBuf>,
    boot_rom_verification_mode: BootRomVerificationMode,
}

enum RunnerMachine {
    Buffered(Machine<TraceBuffer>),
    Summary(Machine<TraceSummaryBuffer>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MooneyeTestResult {
    Passed,
    Failed,
}

struct CaseEvaluationInputs<'a> {
    artifacts: &'a CapturedArtifacts,
    serial_contains_matched: bool,
    diagnostic_trap: Option<CpuDiagnosticTrap>,
    mooneye_result: Option<MooneyeTestResult>,
    framebuffer_until_match_matched: bool,
    framebuffer_until_match_check_at_reached: bool,
    executed_t_cycles: u64,
    completed_frames: u32,
}

impl CaseEvaluationInputs<'_> {
    fn budget_exhausted(&self, timeout: Timeout) -> bool {
        budget_exhausted(timeout, self.executed_t_cycles, self.completed_frames)
    }
}

fn compatibility_for_execution_mode(execution_mode: ExecutionMode) -> CompatibilityPolicy {
    match execution_mode {
        ExecutionMode::Strict => CompatibilityPolicy::strict(),
        ExecutionMode::Permissive => CompatibilityPolicy::permissive(),
        ExecutionMode::Experimental => CompatibilityPolicy::experimental(),
    }
}

const MOONEYE_MAGIC_BREAKPOINT_OPCODE: u8 = 0x40;
const MOONEYE_PASS_SIGNATURE: [u8; 6] = [3, 5, 8, 13, 21, 34];
const MOONEYE_FAIL_SIGNATURE: [u8; 6] = [0x42; 6];
const MOONEYE_HALT_LOOP_BYTES: [u8; 4] = [0x40, 0x00, 0x18, 0xFD];
const REAL_BOOT_HANDOFF_T_CYCLE_LIMIT: u64 = 25_000_000;
const MBC3_RTC_CLOCK_HALF_NORMAL_T_CYCLES: u16 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct DeterministicMbc3RtcClock {
    half_normal_t_cycle_remainder: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FramebufferUntilMatchOracle {
    source: FramebufferUntilMatchSource,
    expected: NormalizedFramebuffer,
    check_interval_tcycles: u64,
    check_at_tcycles: Option<u64>,
    pending_periodic_check: bool,
    matched: bool,
    check_at_reached: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FramebufferUntilMatchSource {
    Dmg,
    Rgb555,
}

impl DeterministicMbc3RtcClock {
    fn tick_t_cycle(&mut self, machine: &mut RunnerMachine) {
        let rtc_ticks = self.tick_t_cycle_for_speed(machine.current_speed());
        if rtc_ticks != 0 {
            machine.advance_mbc3_cartridge_rtc_clock_ticks(rtc_ticks);
        }
    }

    fn tick_t_cycle_for_speed(&mut self, speed_mode: CgbSpeedMode) -> u64 {
        self.half_normal_t_cycle_remainder += match speed_mode {
            CgbSpeedMode::Normal => 2,
            CgbSpeedMode::Double => 1,
        };

        if self.half_normal_t_cycle_remainder >= MBC3_RTC_CLOCK_HALF_NORMAL_T_CYCLES {
            self.half_normal_t_cycle_remainder -= MBC3_RTC_CLOCK_HALF_NORMAL_T_CYCLES;
            1
        } else {
            0
        }
    }
}

impl RunnerMachine {
    fn new(case: &RomTestCase, boot_rom_assets: BootRomAssets) -> Self {
        let config = MachineConfig::new(case.console_model)
            .with_host_platform(case.host_platform)
            .with_revision(case.revision)
            .with_startup_mode(case.startup_mode)
            .with_compatibility(compatibility_for_execution_mode(case.execution_mode))
            .with_boot_rom_assets(boot_rom_assets);
        let needs_trace_buffer = case.capture_plan.contains(CaptureKind::Trace)
            || case.failure_artifacts.contains(CaptureKind::Trace);

        if needs_trace_buffer {
            Self::Buffered(Machine::new(config))
        } else {
            Self::Summary(Machine::new_summary(config))
        }
    }

    fn load_cartridge(
        &mut self,
        rom_bytes: Vec<u8>,
    ) -> Result<Vec<CartridgeDiagnostic>, CartridgeLoadError> {
        match self {
            Self::Buffered(machine) => machine.load_cartridge(rom_bytes),
            Self::Summary(machine) => machine.load_cartridge(rom_bytes),
        }
    }

    fn next_t_cycle(&self) -> u64 {
        match self {
            Self::Buffered(machine) => machine.next_t_cycle().get(),
            Self::Summary(machine) => machine.next_t_cycle().get(),
        }
    }

    fn step_t_cycle(&mut self) {
        match self {
            Self::Buffered(machine) => {
                machine.step_t_cycle();
            }
            Self::Summary(machine) => {
                machine.step_t_cycle();
            }
        }
    }

    fn read_bus(&mut self, address: u16) -> u8 {
        match self {
            Self::Buffered(machine) => machine.read_bus(address),
            Self::Summary(machine) => machine.read_bus(address),
        }
    }

    fn write_bus(&mut self, address: u16, value: u8) {
        match self {
            Self::Buffered(machine) => machine.write_bus(address, value),
            Self::Summary(machine) => machine.write_bus(address, value),
        }
    }

    fn set_joypad_button_pressed(&mut self, button: JoypadButton, pressed: bool) {
        match self {
            Self::Buffered(machine) => machine.set_joypad_button_pressed(button, pressed),
            Self::Summary(machine) => machine.set_joypad_button_pressed(button, pressed),
        }
    }

    fn take_serial_output_bytes(&mut self) -> Vec<u8> {
        match self {
            Self::Buffered(machine) => machine.take_serial_output_bytes(),
            Self::Summary(machine) => machine.take_serial_output_bytes(),
        }
    }

    fn current_speed(&self) -> CgbSpeedMode {
        match self {
            Self::Buffered(machine) => machine.speed().current_speed(),
            Self::Summary(machine) => machine.speed().current_speed(),
        }
    }

    fn boot_rom_mapped(&self) -> bool {
        match self {
            Self::Buffered(machine) => machine.boot().is_boot_rom_mapped(),
            Self::Summary(machine) => machine.boot().is_boot_rom_mapped(),
        }
    }

    fn at_frame_origin(&self) -> bool {
        match self {
            Self::Buffered(machine) => machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0,
            Self::Summary(machine) => machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0,
        }
    }

    fn in_vblank(&self) -> bool {
        match self {
            Self::Buffered(machine) => machine.ppu().ly() >= 144,
            Self::Summary(machine) => machine.ppu().ly() >= 144,
        }
    }

    fn framebuffer(&self) -> &[u8] {
        match self {
            Self::Buffered(machine) => machine.ppu().framebuffer(),
            Self::Summary(machine) => machine.ppu().framebuffer(),
        }
    }

    fn host_framebuffer_rgb555(&self) -> Option<Cow<'_, [u16]>> {
        match self {
            Self::Buffered(machine) => machine
                .ppu()
                .cgb_framebuffer_rgb555()
                .map(Cow::Borrowed)
                .or_else(|| machine.sgb_lcd_framebuffer_rgb555().map(Cow::Owned)),
            Self::Summary(machine) => machine
                .ppu()
                .cgb_framebuffer_rgb555()
                .map(Cow::Borrowed)
                .or_else(|| machine.sgb_lcd_framebuffer_rgb555().map(Cow::Owned)),
        }
    }

    fn cpu_execution_state(&self) -> CpuExecutionState {
        match self {
            Self::Buffered(machine) => machine.cpu().snapshot().execution_state,
            Self::Summary(machine) => machine.cpu().snapshot().execution_state,
        }
    }

    fn cpu_snapshot(&self) -> CpuSnapshot {
        match self {
            Self::Buffered(machine) => machine.cpu().snapshot(),
            Self::Summary(machine) => machine.cpu().snapshot(),
        }
    }

    fn trace_text(&self) -> Option<String> {
        match self {
            Self::Buffered(machine) => Some(machine.tracer().sink().render_text()),
            Self::Summary(_) => None,
        }
    }

    fn snapshot_text(&self) -> String {
        match self {
            Self::Buffered(machine) => machine.snapshot().render_text(),
            Self::Summary(machine) => machine.snapshot().render_text(),
        }
    }

    #[cfg(test)]
    fn capture_save_state(&self) -> MachineSaveState {
        match self {
            Self::Buffered(machine) => machine.capture_save_state(),
            Self::Summary(machine) => machine.capture_save_state(),
        }
    }

    #[cfg(test)]
    fn restore_save_state(
        &mut self,
        state: &MachineSaveState,
    ) -> Result<(), MachineSaveStateRestoreError> {
        match self {
            Self::Buffered(machine) => machine.restore_save_state(state),
            Self::Summary(machine) => machine.restore_save_state(state),
        }
    }

    fn discard_trace_events_if_needed(&mut self, executed_t_cycles: u64) {
        match self {
            Self::Buffered(machine) => {
                discard_trace_events_if_needed(machine.tracer_mut().sink_mut(), executed_t_cycles);
            }
            Self::Summary(_) => {}
        }
    }

    fn advance_cartridge_rtc_seconds(&mut self, seconds: u64) {
        match self {
            Self::Buffered(machine) => machine.advance_cartridge_rtc_seconds(seconds),
            Self::Summary(machine) => machine.advance_cartridge_rtc_seconds(seconds),
        }
    }

    fn advance_mbc3_cartridge_rtc_clock_ticks(&mut self, ticks: u64) {
        match self {
            Self::Buffered(machine) => machine.advance_mbc3_cartridge_rtc_clock_ticks(ticks),
            Self::Summary(machine) => machine.advance_mbc3_cartridge_rtc_clock_ticks(ticks),
        }
    }

    fn apply_timer_startup_state(&mut self, startup_timer_state: TimerStartupState) {
        match self {
            Self::Buffered(machine) => machine.apply_timer_startup_state(startup_timer_state),
            Self::Summary(machine) => machine.apply_timer_startup_state(startup_timer_state),
        }
    }
}

impl Default for RomRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl RomRunner {
    pub fn new() -> Self {
        Self {
            workspace_root: default_workspace_root(),
            failure_artifact_root: None,
            boot_rom_root: None,
            boot_rom_verification_mode: BootRomVerificationMode::Strict,
        }
    }

    pub fn with_workspace_root(mut self, workspace_root: impl Into<PathBuf>) -> Self {
        self.workspace_root = workspace_root.into();
        self
    }

    pub fn with_failure_artifact_root(mut self, failure_artifact_root: impl Into<PathBuf>) -> Self {
        self.failure_artifact_root = Some(failure_artifact_root.into());
        self
    }

    pub fn with_boot_rom_root(mut self, boot_rom_root: impl Into<PathBuf>) -> Self {
        self.boot_rom_root = Some(boot_rom_root.into());
        self
    }

    pub fn with_boot_rom_verification_mode(
        mut self,
        boot_rom_verification_mode: BootRomVerificationMode,
    ) -> Self {
        self.boot_rom_verification_mode = boot_rom_verification_mode;
        self
    }

    pub fn run_suite(&self, suite: &RomSuite) -> Result<RomSuiteReport, RomExecutionError> {
        suite.validate().map_err(RomExecutionError::InvalidSuite)?;

        let case_reports: Vec<RomCaseReport> = suite
            .cases
            .par_iter()
            .map(|case| self.run_case(case))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(RomSuiteReport {
            suite_name: suite.name.clone(),
            family: suite.family.clone(),
            cases: case_reports,
        })
    }

    pub fn run_case(&self, case: &RomTestCase) -> Result<RomCaseReport, RomExecutionError> {
        case.validate().map_err(RomExecutionError::InvalidCase)?;
        let mut framebuffer_until_match_oracle = self.framebuffer_until_match_oracle(case)?;

        let rom_path = self.resolve_case_rom_path(case)?;
        let rom_bytes = fs::read(&rom_path).map_err(|source| RomExecutionError::ReadFile {
            path: rom_path.clone(),
            operation: "read ROM",
            source,
        })?;

        let boot_rom_assets = self.load_boot_rom_assets(case)?;
        let mut machine = RunnerMachine::new(case, boot_rom_assets);
        let diagnostics = machine.load_cartridge(rom_bytes).map_err(|source| {
            RomExecutionError::CartridgeLoad {
                path: rom_path.clone(),
                source,
            }
        })?;
        let mut rtc_clock = DeterministicMbc3RtcClock::default();
        let startup_failure =
            self.advance_real_boot_to_handoff_if_needed(case, &mut machine, &mut rtc_clock);
        if startup_failure.is_none() {
            self.apply_startup_cartridge_state(case, &mut machine);
            self.apply_startup_timer_state(case, &mut machine);
            self.apply_startup_memory_writes(case, &mut machine);
        }

        let mut executed_t_cycles = 0_u64;
        let mut completed_frames = 0_u32;
        let mut at_frame_origin = machine.at_frame_origin();
        let mut serial_bytes = Vec::new();
        let mut applied_stimuli = vec![false; case.external_stimuli.stimuli().len()];
        let mut serial_contains_matched = false;
        let mut diagnostic_trap = None;
        let mut last_memory_text_output_completion_candidate = None;
        let mut mooneye_result = None;

        while startup_failure.is_none()
            && !budget_exhausted(case.timeout, executed_t_cycles, completed_frames)
        {
            if stop_condition_satisfied(case.stop_condition, &mut machine) {
                break;
            }

            self.apply_scheduled_stimuli(
                case,
                &mut machine,
                completed_frames,
                &mut applied_stimuli,
            );

            machine.step_t_cycle();
            rtc_clock.tick_t_cycle(&mut machine);
            executed_t_cycles += 1;

            if stop_condition_satisfied(case.stop_condition, &mut machine) {
                break;
            }

            serial_bytes.extend(machine.take_serial_output_bytes());

            let now_at_frame_origin = machine.at_frame_origin();
            if now_at_frame_origin && !at_frame_origin {
                completed_frames += 1;
            }
            at_frame_origin = now_at_frame_origin;

            if let Some(oracle) = &mut framebuffer_until_match_oracle
                && framebuffer_until_match_poll_due(
                    case.id.as_str(),
                    &mut machine,
                    executed_t_cycles,
                    oracle,
                )?
            {
                break;
            }

            if let PassCondition::SerialContains(expected) = &case.pass_condition
                && String::from_utf8_lossy(&serial_bytes).contains(expected)
            {
                serial_contains_matched = true;
                break;
            }

            if mooneye_result.is_none() {
                mooneye_result =
                    mooneye_result_completion_candidate(&case.pass_condition, &mut machine);
                if mooneye_result.is_some() {
                    break;
                }
            }

            if executed_t_cycles & 0x1FFF == 0
                && memory_bytes_terminal(&case.pass_condition, &mut machine)
            {
                break;
            }

            if executed_t_cycles.is_multiple_of(1_024) {
                let memory_completion_candidate =
                    memory_text_output_completion_candidate(&case.pass_condition, &mut machine);
                if memory_text_output_completion_reached(
                    &mut last_memory_text_output_completion_candidate,
                    memory_completion_candidate,
                ) {
                    break;
                }
            }

            if executed_t_cycles.is_multiple_of(1_024)
                && blargg_console_text_complete(&case.pass_condition, &mut machine)
            {
                break;
            }

            if let CpuExecutionState::DiagnosticTrap { trap } = machine.cpu_execution_state() {
                diagnostic_trap = Some(trap);
                break;
            }

            machine.discard_trace_events_if_needed(executed_t_cycles);
        }

        let artifacts = self.capture_artifacts(case, &mut machine, &serial_bytes);
        let evaluation = CaseEvaluationInputs {
            artifacts: &artifacts,
            serial_contains_matched,
            diagnostic_trap,
            mooneye_result,
            framebuffer_until_match_matched: framebuffer_until_match_oracle
                .as_ref()
                .is_some_and(|oracle| oracle.matched),
            framebuffer_until_match_check_at_reached: framebuffer_until_match_oracle
                .as_ref()
                .is_some_and(|oracle| oracle.check_at_reached),
            executed_t_cycles,
            completed_frames,
        };
        let outcome = if let Some(failure) = startup_failure {
            RomCaseOutcome::Failed(failure)
        } else {
            self.evaluate_case(case, &evaluation)?
        };
        let retained_failure_artifacts = if outcome.failed() {
            self.persist_failure_artifacts(case, &artifacts)?
        } else {
            Vec::new()
        };

        Ok(RomCaseReport {
            case_id: case.id.clone(),
            rom_path: case.rom_path.clone(),
            outcome,
            executed_t_cycles,
            completed_frames,
            diagnostics,
            artifacts,
            retained_failure_artifacts,
        })
    }

    pub fn resolve_case_rom_path(&self, case: &RomTestCase) -> Result<PathBuf, RomExecutionError> {
        self.resolve_case_path(&case.rom_path)
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    fn advance_real_boot_to_handoff_if_needed(
        &self,
        case: &RomTestCase,
        machine: &mut RunnerMachine,
        rtc_clock: &mut DeterministicMbc3RtcClock,
    ) -> Option<RomCaseFailure> {
        if case.startup_mode != StartupMode::RealBoot || !machine.boot_rom_mapped() {
            return None;
        }

        for _ in 0..REAL_BOOT_HANDOFF_T_CYCLE_LIMIT {
            machine.step_t_cycle();
            rtc_clock.tick_t_cycle(machine);

            if let CpuExecutionState::DiagnosticTrap { trap } = machine.cpu_execution_state() {
                return Some(RomCaseFailure::CpuDiagnosticTrap { trap });
            }

            if !machine.boot_rom_mapped() {
                let _ = machine.take_serial_output_bytes();
                return None;
            }
        }

        Some(RomCaseFailure::RealBootHandoffTimeout {
            t_cycle_limit: REAL_BOOT_HANDOFF_T_CYCLE_LIMIT,
        })
    }

    fn load_boot_rom_assets(&self, case: &RomTestCase) -> Result<BootRomAssets, RomExecutionError> {
        if case.startup_mode != StartupMode::RealBoot {
            return Ok(BootRomAssets::none());
        }

        let asset = boot_rom_asset_for_console_profile(case.console_model, case.host_platform);

        let Some(root) = self.boot_rom_root.clone().or_else(discover_boot_rom_root) else {
            if boot_rom_asset_is_required_for_runner_gate(asset) {
                enforce_missing_boot_rom_root_verification(self.boot_rom_verification_mode, asset)
                    .map_err(|issue| RomExecutionError::BootRomVerification { issue })?;
            }
            return Ok(BootRomAssets::none());
        };
        let image_path = boot_rom_image_path(&root, asset);
        if !boot_rom_asset_is_required_for_runner_gate(asset) && !image_path.is_file() {
            return Ok(BootRomAssets::none());
        }
        enforce_boot_rom_asset_verification(self.boot_rom_verification_mode, &image_path, asset)
            .map_err(|issue| RomExecutionError::BootRomVerification { issue })?;
        if !root.is_dir() {
            return Ok(BootRomAssets::none());
        }

        BootRomAssets::from_directory(&root)
            .map_err(|source| RomExecutionError::BootRomAssets { path: root, source })
    }

    fn resolve_path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else if let Ok(store_relative_path) = path.strip_prefix(TEST_ROM_STORE_DIR) {
            if let Some(root) = discover_test_rom_store_root(&self.workspace_root) {
                root.join(store_relative_path)
            } else {
                self.workspace_root.join(path)
            }
        } else {
            self.workspace_root.join(path)
        }
    }

    fn framebuffer_until_match_oracle(
        &self,
        case: &RomTestCase,
    ) -> Result<Option<FramebufferUntilMatchOracle>, RomExecutionError> {
        let (source, fixture_path, check_interval_tcycles, check_at_tcycles) =
            match &case.pass_condition {
                PassCondition::FramebufferFixtureUntilMatch {
                    fixture_path,
                    check_interval_tcycles,
                    check_at_tcycles,
                } => (
                    FramebufferUntilMatchSource::Dmg,
                    fixture_path,
                    *check_interval_tcycles,
                    *check_at_tcycles,
                ),
                PassCondition::FramebufferRgb555FixtureUntilMatch {
                    fixture_path,
                    check_interval_tcycles,
                    check_at_tcycles,
                } => (
                    FramebufferUntilMatchSource::Rgb555,
                    fixture_path,
                    *check_interval_tcycles,
                    *check_at_tcycles,
                ),
                _ => return Ok(None),
            };
        let resolved_fixture = self.resolve_path(fixture_path);
        let expected = decode_fixture_framebuffer_path(&resolved_fixture).map_err(|error| {
            let path = error.path.clone();
            RomExecutionError::ReadFile {
                path,
                operation: "decode framebuffer fixture",
                source: error.into_invalid_data_error(),
            }
        })?;

        Ok(Some(FramebufferUntilMatchOracle {
            source,
            expected,
            check_interval_tcycles,
            check_at_tcycles,
            pending_periodic_check: false,
            matched: false,
            check_at_reached: false,
        }))
    }

    fn resolve_case_path(&self, path: &Path) -> Result<PathBuf, RomExecutionError> {
        if path.is_absolute() {
            return Ok(path.to_path_buf());
        }

        if let Ok(store_relative_path) = path.strip_prefix(TEST_ROM_STORE_DIR) {
            if let Some(root) = discover_test_rom_store_root(&self.workspace_root) {
                return Ok(root.join(store_relative_path));
            }
            return Err(RomExecutionError::MissingExternalRomRoot {
                key: TEST_ROM_ROOT_ENV_VAR.to_string(),
                relative_path: store_relative_path.to_path_buf(),
            });
        }

        Ok(self.workspace_root.join(path))
    }

    fn apply_scheduled_stimuli(
        &self,
        case: &RomTestCase,
        machine: &mut RunnerMachine,
        completed_frames: u32,
        applied_stimuli: &mut [bool],
    ) {
        let current_t_cycle = machine.next_t_cycle();

        for (index, stimulus) in case.external_stimuli.stimuli().iter().enumerate() {
            if applied_stimuli[index] {
                continue;
            }

            let should_apply = match stimulus.when {
                StimulusTime::TCycle(t_cycle) => t_cycle == current_t_cycle,
                StimulusTime::Frame(frame) => frame == completed_frames,
            };

            if !should_apply {
                continue;
            }

            match stimulus.action {
                ExternalStimulusAction::JoypadSetButton { button, pressed } => {
                    machine.set_joypad_button_pressed(button, pressed);
                }
                ExternalStimulusAction::WriteMemory { address, value } => {
                    machine.write_bus(address, value);
                }
            }

            applied_stimuli[index] = true;
        }
    }

    fn apply_startup_cartridge_state(&self, case: &RomTestCase, machine: &mut RunnerMachine) {
        if let Some(seconds) = case.startup_cartridge_rtc_seconds {
            machine.advance_cartridge_rtc_seconds(seconds);
        }
    }

    fn apply_startup_timer_state(&self, case: &RomTestCase, machine: &mut RunnerMachine) {
        if let Some(startup_timer_state) = case.startup_timer_state {
            machine.apply_timer_startup_state(startup_timer_state);
        }
    }

    fn apply_startup_memory_writes(&self, case: &RomTestCase, machine: &mut RunnerMachine) {
        for write in &case.startup_memory_writes {
            machine.write_bus(write.address, write.value);
        }
    }

    fn capture_artifacts(
        &self,
        case: &RomTestCase,
        machine: &mut RunnerMachine,
        serial_bytes: &[u8],
    ) -> CapturedArtifacts {
        let mut artifacts = CapturedArtifacts::default();

        if case.capture_plan.contains(CaptureKind::Serial) {
            artifacts.serial = Some(String::from_utf8_lossy(serial_bytes).into_owned());
        }

        if case.capture_plan.contains(CaptureKind::SerialHex) {
            artifacts.serial_hex = Some(encode_bytes_as_upper_hex(serial_bytes));
        }

        if case.capture_plan.contains(CaptureKind::MemoryBytes)
            && let PassCondition::MemoryBytesEqual(expectations) = &case.pass_condition
        {
            artifacts.memory_bytes = Some(capture_memory_bytes(expectations, machine));
        }

        if case.capture_plan.contains(CaptureKind::MemoryTextOutput)
            && let Some(spec) = memory_text_output_spec(&case.pass_condition)
        {
            artifacts.memory_text_output = Some(capture_memory_text_output(spec, machine));
        }

        if case.capture_plan.contains(CaptureKind::BlarggConsoleText) {
            artifacts.blargg_console_text = Some(capture_blargg_console_text(machine));
        }

        if case.capture_plan.contains(CaptureKind::Framebuffer) {
            artifacts.framebuffer_pgm = Some(encode_framebuffer_pgm(machine.framebuffer()));
            if let Some(framebuffer_rgb555) = machine.host_framebuffer_rgb555() {
                artifacts.framebuffer_rgb555 = Some(framebuffer_rgb555.into_owned());
            }
        }

        if case.capture_plan.contains(CaptureKind::Trace) {
            artifacts.trace = machine.trace_text();
        }

        if case.capture_plan.contains(CaptureKind::Snapshot) {
            artifacts.snapshot_text = Some(machine.snapshot_text());
        }

        artifacts
    }

    fn evaluate_case(
        &self,
        case: &RomTestCase,
        evaluation: &CaseEvaluationInputs<'_>,
    ) -> Result<RomCaseOutcome, RomExecutionError> {
        if let Some(trap) = evaluation.diagnostic_trap {
            return Ok(RomCaseOutcome::Failed(RomCaseFailure::CpuDiagnosticTrap {
                trap,
            }));
        }

        Ok(match &case.pass_condition {
            PassCondition::SerialContains(expected_substring) => {
                if evaluation.serial_contains_matched {
                    RomCaseOutcome::Passed
                } else if evaluation.budget_exhausted(case.timeout) {
                    RomCaseOutcome::Failed(RomCaseFailure::SerialMissingSubstring {
                        expected_substring: expected_substring.clone(),
                        actual: evaluation.artifacts.serial.clone().unwrap_or_default(),
                    })
                } else {
                    RomCaseOutcome::Failed(RomCaseFailure::TimeoutExceeded)
                }
            }
            PassCondition::SerialExact(expected) => {
                if !evaluation.budget_exhausted(case.timeout) {
                    RomCaseOutcome::Failed(RomCaseFailure::TimeoutExceeded)
                } else if evaluation.artifacts.serial.as_deref() == Some(expected.as_str()) {
                    RomCaseOutcome::Passed
                } else {
                    RomCaseOutcome::Failed(RomCaseFailure::SerialExactMismatch {
                        expected: expected.clone(),
                        actual: evaluation.artifacts.serial.clone().unwrap_or_default(),
                    })
                }
            }
            PassCondition::SerialHexExact(expected) => {
                if !evaluation.budget_exhausted(case.timeout) {
                    RomCaseOutcome::Failed(RomCaseFailure::TimeoutExceeded)
                } else if evaluation.artifacts.serial_hex.as_deref() == Some(expected.as_str()) {
                    RomCaseOutcome::Passed
                } else {
                    RomCaseOutcome::Failed(RomCaseFailure::SerialExactMismatch {
                        expected: expected.clone(),
                        actual: evaluation.artifacts.serial_hex.clone().unwrap_or_default(),
                    })
                }
            }
            PassCondition::MemoryBytesEqual(_) => {
                let captured = evaluation
                    .artifacts
                    .memory_bytes
                    .clone()
                    .unwrap_or_default();
                if captured
                    .bytes
                    .iter()
                    .all(|byte| byte.actual == byte.expected)
                {
                    RomCaseOutcome::Passed
                } else {
                    RomCaseOutcome::Failed(RomCaseFailure::MemoryByteMismatch {
                        bytes: captured.bytes,
                    })
                }
            }
            PassCondition::MemoryTextOutputContains {
                spec,
                expected_substring,
            } => {
                let captured = evaluation
                    .artifacts
                    .memory_text_output
                    .clone()
                    .unwrap_or_default();
                if captured.signature == spec.expected_signature
                    && captured.status == spec.pass_status
                    && captured.text.contains(expected_substring)
                {
                    RomCaseOutcome::Passed
                } else {
                    RomCaseOutcome::Failed(RomCaseFailure::MemoryTextOutputMismatch {
                        expected_substring: expected_substring.clone(),
                        pass_status: spec.pass_status,
                        expected_signature: spec.expected_signature,
                        actual_status: captured.status,
                        actual_signature: captured.signature,
                        actual_text: captured.text,
                    })
                }
            }
            PassCondition::BlarggConsoleTextContains(expected_substring) => {
                let actual = evaluation
                    .artifacts
                    .blargg_console_text
                    .clone()
                    .unwrap_or_default();
                if actual.contains(expected_substring) {
                    RomCaseOutcome::Passed
                } else {
                    RomCaseOutcome::Failed(RomCaseFailure::BlarggConsoleTextMissingSubstring {
                        expected_substring: expected_substring.clone(),
                        actual,
                    })
                }
            }
            PassCondition::MooneyeResult => match evaluation.mooneye_result {
                Some(MooneyeTestResult::Passed) => RomCaseOutcome::Passed,
                Some(MooneyeTestResult::Failed) => {
                    RomCaseOutcome::Failed(RomCaseFailure::MooneyeFailureSignature)
                }
                None => RomCaseOutcome::Failed(RomCaseFailure::MooneyeResultNotReached),
            },
            PassCondition::Informational(_) => RomCaseOutcome::Informational,
            PassCondition::TraceFixture(fixture_path) => {
                let resolved_fixture = self.resolve_path(fixture_path);
                let expected = fs::read_to_string(&resolved_fixture).map_err(|source| {
                    RomExecutionError::ReadFile {
                        path: resolved_fixture.clone(),
                        operation: "read trace fixture",
                        source,
                    }
                })?;

                if evaluation.artifacts.trace.as_deref() == Some(expected.as_str()) {
                    RomCaseOutcome::Passed
                } else {
                    RomCaseOutcome::Failed(RomCaseFailure::TraceFixtureMismatch {
                        fixture_path: resolved_fixture,
                    })
                }
            }
            PassCondition::FramebufferFixture(fixture_path) => {
                let resolved_fixture = self.resolve_path(fixture_path);
                let actual = decode_local_pgm_framebuffer(
                    case.id.as_str(),
                    evaluation
                        .artifacts
                        .framebuffer_pgm
                        .as_deref()
                        .ok_or_else(|| RomExecutionError::ReadFile {
                            path: PathBuf::from(format!("<local framebuffer for {}>", case.id)),
                            operation: "decode local framebuffer artifact",
                            source: io::Error::new(
                                io::ErrorKind::InvalidData,
                                "missing local framebuffer capture",
                            ),
                        })?,
                )
                .map_err(|error| {
                    let path = error.path.clone();
                    RomExecutionError::ReadFile {
                        path,
                        operation: "decode local framebuffer artifact",
                        source: error.into_invalid_data_error(),
                    }
                })?;
                let expected =
                    decode_fixture_framebuffer_path(&resolved_fixture).map_err(|error| {
                        let path = error.path.clone();
                        RomExecutionError::ReadFile {
                            path,
                            operation: "decode framebuffer fixture",
                            source: error.into_invalid_data_error(),
                        }
                    })?;

                if actual == expected {
                    RomCaseOutcome::Passed
                } else {
                    RomCaseOutcome::Failed(RomCaseFailure::FramebufferFixtureMismatch {
                        fixture_path: resolved_fixture,
                    })
                }
            }
            PassCondition::FramebufferFixtureUntilMatch {
                fixture_path,
                check_at_tcycles,
                ..
            }
            | PassCondition::FramebufferRgb555FixtureUntilMatch {
                fixture_path,
                check_at_tcycles,
                ..
            } => {
                if evaluation.framebuffer_until_match_matched {
                    RomCaseOutcome::Passed
                } else if let Some(check_at_tcycles) = check_at_tcycles
                    && !evaluation.framebuffer_until_match_check_at_reached
                {
                    RomCaseOutcome::Failed(RomCaseFailure::FramebufferCheckAtNotReached {
                        check_at_tcycles: *check_at_tcycles,
                        executed_t_cycles: evaluation.executed_t_cycles,
                    })
                } else {
                    let resolved_fixture = self.resolve_path(fixture_path);
                    let actual = match &case.pass_condition {
                        PassCondition::FramebufferFixtureUntilMatch { .. } => {
                            decode_local_pgm_framebuffer(
                                case.id.as_str(),
                                evaluation.artifacts.framebuffer_pgm.as_deref().ok_or_else(
                                    || RomExecutionError::ReadFile {
                                        path: PathBuf::from(format!(
                                            "<local framebuffer for {}>",
                                            case.id
                                        )),
                                        operation: "decode local framebuffer artifact",
                                        source: io::Error::new(
                                            io::ErrorKind::InvalidData,
                                            "missing local framebuffer capture",
                                        ),
                                    },
                                )?,
                            )
                            .map_err(|error| {
                                let path = error.path.clone();
                                RomExecutionError::ReadFile {
                                    path,
                                    operation: "decode local framebuffer artifact",
                                    source: error.into_invalid_data_error(),
                                }
                            })?
                        }
                        PassCondition::FramebufferRgb555FixtureUntilMatch { .. } => {
                            decode_local_rgb555_framebuffer(
                                case.id.as_str(),
                                evaluation
                                    .artifacts
                                    .framebuffer_rgb555
                                    .as_deref()
                                    .ok_or_else(|| RomExecutionError::ReadFile {
                                        path: PathBuf::from(format!(
                                            "<local host RGB555 framebuffer for {}>",
                                            case.id
                                        )),
                                        operation: "decode local host RGB555 framebuffer artifact",
                                        source: io::Error::new(
                                            io::ErrorKind::InvalidData,
                                            "missing local host RGB555 framebuffer capture",
                                        ),
                                    })?,
                            )
                            .map_err(|error| {
                                let path = error.path.clone();
                                RomExecutionError::ReadFile {
                                    path,
                                    operation: "decode local host RGB555 framebuffer artifact",
                                    source: error.into_invalid_data_error(),
                                }
                            })?
                        }
                        _ => unreachable!("matched until-match pass condition"),
                    };
                    let expected =
                        decode_fixture_framebuffer_path(&resolved_fixture).map_err(|error| {
                            let path = error.path.clone();
                            RomExecutionError::ReadFile {
                                path,
                                operation: "decode framebuffer fixture",
                                source: error.into_invalid_data_error(),
                            }
                        })?;

                    if actual == expected {
                        RomCaseOutcome::Passed
                    } else {
                        RomCaseOutcome::Failed(RomCaseFailure::FramebufferFixtureMismatch {
                            fixture_path: resolved_fixture,
                        })
                    }
                }
            }
            PassCondition::FramebufferGrayscaleFixture(fixture_path) => {
                let resolved_fixture = self.resolve_path(fixture_path);
                let actual = decode_local_pgm_grayscale_framebuffer(
                    case.id.as_str(),
                    evaluation
                        .artifacts
                        .framebuffer_pgm
                        .as_deref()
                        .ok_or_else(|| RomExecutionError::ReadFile {
                            path: PathBuf::from(format!("<local framebuffer for {}>", case.id)),
                            operation: "decode local framebuffer artifact",
                            source: io::Error::new(
                                io::ErrorKind::InvalidData,
                                "missing local framebuffer capture",
                            ),
                        })?,
                )
                .map_err(|error| {
                    let path = error.path.clone();
                    RomExecutionError::ReadFile {
                        path,
                        operation: "decode local framebuffer artifact",
                        source: error.into_invalid_data_error(),
                    }
                })?;
                let expected = decode_fixture_grayscale_framebuffer_path(&resolved_fixture)
                    .map_err(|error| {
                        let path = error.path.clone();
                        RomExecutionError::ReadFile {
                            path,
                            operation: "decode framebuffer grayscale fixture",
                            source: error.into_invalid_data_error(),
                        }
                    })?;

                if actual == expected {
                    RomCaseOutcome::Passed
                } else {
                    RomCaseOutcome::Failed(RomCaseFailure::FramebufferFixtureMismatch {
                        fixture_path: resolved_fixture,
                    })
                }
            }
            PassCondition::FramebufferRgb555Fixture(fixture_path) => {
                let resolved_fixture = self.resolve_path(fixture_path);
                let actual = decode_local_rgb555_framebuffer(
                    case.id.as_str(),
                    evaluation
                        .artifacts
                        .framebuffer_rgb555
                        .as_deref()
                        .ok_or_else(|| RomExecutionError::ReadFile {
                            path: PathBuf::from(format!(
                                "<local host RGB555 framebuffer for {}>",
                                case.id
                            )),
                            operation: "decode local host RGB555 framebuffer artifact",
                            source: io::Error::new(
                                io::ErrorKind::InvalidData,
                                "missing local host RGB555 framebuffer capture",
                            ),
                        })?,
                )
                .map_err(|error| {
                    let path = error.path.clone();
                    RomExecutionError::ReadFile {
                        path,
                        operation: "decode local host RGB555 framebuffer artifact",
                        source: error.into_invalid_data_error(),
                    }
                })?;
                let expected =
                    decode_fixture_framebuffer_path(&resolved_fixture).map_err(|error| {
                        let path = error.path.clone();
                        RomExecutionError::ReadFile {
                            path,
                            operation: "decode framebuffer fixture",
                            source: error.into_invalid_data_error(),
                        }
                    })?;

                if actual == expected {
                    RomCaseOutcome::Passed
                } else {
                    RomCaseOutcome::Failed(RomCaseFailure::FramebufferFixtureMismatch {
                        fixture_path: resolved_fixture,
                    })
                }
            }
            PassCondition::FramebufferRgb555GrayscaleFixture(fixture_path) => {
                let resolved_fixture = self.resolve_path(fixture_path);
                let actual = decode_local_rgb555_grayscale_framebuffer(
                    case.id.as_str(),
                    evaluation
                        .artifacts
                        .framebuffer_rgb555
                        .as_deref()
                        .ok_or_else(|| RomExecutionError::ReadFile {
                            path: PathBuf::from(format!(
                                "<local host RGB555 framebuffer for {}>",
                                case.id
                            )),
                            operation: "decode local host RGB555 framebuffer artifact",
                            source: io::Error::new(
                                io::ErrorKind::InvalidData,
                                "missing local host RGB555 framebuffer capture",
                            ),
                        })?,
                )
                .map_err(|error| {
                    let path = error.path.clone();
                    RomExecutionError::ReadFile {
                        path,
                        operation: "decode local host RGB555 framebuffer artifact",
                        source: error.into_invalid_data_error(),
                    }
                })?;
                let expected = decode_fixture_grayscale_framebuffer_path(&resolved_fixture)
                    .map_err(|error| {
                        let path = error.path.clone();
                        RomExecutionError::ReadFile {
                            path,
                            operation: "decode framebuffer grayscale fixture",
                            source: error.into_invalid_data_error(),
                        }
                    })?;

                if actual == expected {
                    RomCaseOutcome::Passed
                } else {
                    RomCaseOutcome::Failed(RomCaseFailure::FramebufferFixtureMismatch {
                        fixture_path: resolved_fixture,
                    })
                }
            }
            PassCondition::FramebufferRgb555GrayscaleToleranceFixture(fixture_path) => {
                let resolved_fixture = self.resolve_path(fixture_path);
                let actual = decode_local_rgb555_grayscale_framebuffer(
                    case.id.as_str(),
                    evaluation
                        .artifacts
                        .framebuffer_rgb555
                        .as_deref()
                        .ok_or_else(|| RomExecutionError::ReadFile {
                            path: PathBuf::from(format!(
                                "<local host RGB555 framebuffer for {}>",
                                case.id
                            )),
                            operation: "decode local host RGB555 framebuffer artifact",
                            source: io::Error::new(
                                io::ErrorKind::InvalidData,
                                "missing local host RGB555 framebuffer capture",
                            ),
                        })?,
                )
                .map_err(|error| {
                    let path = error.path.clone();
                    RomExecutionError::ReadFile {
                        path,
                        operation: "decode local host RGB555 framebuffer artifact",
                        source: error.into_invalid_data_error(),
                    }
                })?;
                let expected = decode_fixture_grayscale_framebuffer_path(&resolved_fixture)
                    .map_err(|error| {
                        let path = error.path.clone();
                        RomExecutionError::ReadFile {
                            path,
                            operation: "decode framebuffer grayscale fixture",
                            source: error.into_invalid_data_error(),
                        }
                    })?;

                if grayscale_framebuffers_match_with_tolerance(
                    &actual,
                    &expected,
                    GBEMU_SHOOTOUT_GRAYSCALE_TOLERANCE,
                ) {
                    RomCaseOutcome::Passed
                } else {
                    RomCaseOutcome::Failed(RomCaseFailure::FramebufferFixtureMismatch {
                        fixture_path: resolved_fixture,
                    })
                }
            }
            PassCondition::FramebufferFixtureSet(fixture_paths) => {
                let resolved_fixtures = fixture_paths
                    .iter()
                    .map(|fixture_path| self.resolve_path(fixture_path))
                    .collect::<Vec<_>>();
                let actual = decode_local_pgm_framebuffer(
                    case.id.as_str(),
                    evaluation
                        .artifacts
                        .framebuffer_pgm
                        .as_deref()
                        .ok_or_else(|| RomExecutionError::ReadFile {
                            path: PathBuf::from(format!("<local framebuffer for {}>", case.id)),
                            operation: "decode local framebuffer artifact",
                            source: io::Error::new(
                                io::ErrorKind::InvalidData,
                                "missing local framebuffer capture",
                            ),
                        })?,
                )
                .map_err(|error| {
                    let path = error.path.clone();
                    RomExecutionError::ReadFile {
                        path,
                        operation: "decode local framebuffer artifact",
                        source: error.into_invalid_data_error(),
                    }
                })?;

                for resolved_fixture in &resolved_fixtures {
                    let expected =
                        decode_fixture_framebuffer_path(resolved_fixture).map_err(|error| {
                            let path = error.path.clone();
                            RomExecutionError::ReadFile {
                                path,
                                operation: "decode framebuffer fixture",
                                source: error.into_invalid_data_error(),
                            }
                        })?;

                    if actual == expected {
                        return Ok(RomCaseOutcome::Passed);
                    }
                }

                RomCaseOutcome::Failed(RomCaseFailure::FramebufferFixtureSetMismatch {
                    fixture_paths: resolved_fixtures,
                })
            }
        })
    }

    fn persist_failure_artifacts(
        &self,
        case: &RomTestCase,
        artifacts: &CapturedArtifacts,
    ) -> Result<Vec<PathBuf>, RomExecutionError> {
        let Some(root) = &self.failure_artifact_root else {
            return Ok(Vec::new());
        };

        let case_dir = root.join(&case.id);
        fs::create_dir_all(&case_dir).map_err(|source| RomExecutionError::CreateDirectory {
            path: case_dir.clone(),
            source,
        })?;

        let mut written_paths = Vec::new();
        for artifact in case.failure_artifacts.retained() {
            match artifact {
                CaptureKind::Serial => {
                    let Some(serial) = &artifacts.serial else {
                        continue;
                    };
                    let path = case_dir.join("serial.txt");
                    fs::write(&path, serial).map_err(|source| RomExecutionError::ReadFile {
                        path: path.clone(),
                        operation: "write serial artifact",
                        source,
                    })?;
                    written_paths.push(path);
                }
                CaptureKind::SerialHex => {
                    let Some(serial_hex) = &artifacts.serial_hex else {
                        continue;
                    };
                    let path = case_dir.join("serial_hex.txt");
                    fs::write(&path, serial_hex).map_err(|source| RomExecutionError::ReadFile {
                        path: path.clone(),
                        operation: "write serial hex artifact",
                        source,
                    })?;
                    written_paths.push(path);
                }
                CaptureKind::MemoryBytes => {
                    let Some(memory_bytes) = &artifacts.memory_bytes else {
                        continue;
                    };
                    let path = case_dir.join("memory_bytes.txt");
                    fs::write(&path, render_memory_bytes(memory_bytes)).map_err(|source| {
                        RomExecutionError::ReadFile {
                            path: path.clone(),
                            operation: "write memory bytes artifact",
                            source,
                        }
                    })?;
                    written_paths.push(path);
                }
                CaptureKind::MemoryTextOutput => {
                    let Some(memory_text_output) = &artifacts.memory_text_output else {
                        continue;
                    };
                    let path = case_dir.join("memory_text_output.txt");
                    fs::write(&path, render_memory_text_output(memory_text_output)).map_err(
                        |source| RomExecutionError::ReadFile {
                            path: path.clone(),
                            operation: "write memory text output artifact",
                            source,
                        },
                    )?;
                    written_paths.push(path);
                }
                CaptureKind::BlarggConsoleText => {
                    let Some(blargg_console_text) = &artifacts.blargg_console_text else {
                        continue;
                    };
                    let path = case_dir.join("blargg_console.txt");
                    fs::write(&path, blargg_console_text).map_err(|source| {
                        RomExecutionError::ReadFile {
                            path: path.clone(),
                            operation: "write blargg console artifact",
                            source,
                        }
                    })?;
                    written_paths.push(path);
                }
                CaptureKind::Framebuffer => {
                    if let Some(framebuffer_rgb555) = &artifacts.framebuffer_rgb555 {
                        let png_path = case_dir.join("framebuffer.png");
                        let rgb555_png = encode_rgb555_framebuffer_png(framebuffer_rgb555)
                            .map_err(|source| RomExecutionError::ReadFile {
                                path: png_path.clone(),
                                operation: "encode host RGB555 framebuffer artifact",
                                source,
                            })?;
                        fs::write(&png_path, rgb555_png).map_err(|source| {
                            RomExecutionError::ReadFile {
                                path: png_path.clone(),
                                operation: "write host RGB555 framebuffer artifact",
                                source,
                            }
                        })?;
                        written_paths.push(png_path);
                        continue;
                    }

                    let Some(framebuffer_pgm) = &artifacts.framebuffer_pgm else {
                        continue;
                    };
                    let png_path = case_dir.join("framebuffer.png");
                    let framebuffer_png = convert_pgm_to_png(framebuffer_pgm).map_err(|error| {
                        let path = error.path.clone();
                        RomExecutionError::ReadFile {
                            path,
                            operation: "decode local framebuffer artifact",
                            source: error.into_invalid_data_error(),
                        }
                    })?;
                    fs::write(&png_path, framebuffer_png).map_err(|source| {
                        RomExecutionError::ReadFile {
                            path: png_path.clone(),
                            operation: "write framebuffer artifact",
                            source,
                        }
                    })?;
                    written_paths.push(png_path);

                    let pgm_path = case_dir.join("framebuffer.pgm");
                    fs::write(&pgm_path, framebuffer_pgm).map_err(|source| {
                        RomExecutionError::ReadFile {
                            path: pgm_path.clone(),
                            operation: "write legacy framebuffer artifact",
                            source,
                        }
                    })?;
                    written_paths.push(pgm_path);
                }
                CaptureKind::Trace => {
                    let Some(trace) = &artifacts.trace else {
                        continue;
                    };
                    let path = case_dir.join("trace.txt");
                    fs::write(&path, trace).map_err(|source| RomExecutionError::ReadFile {
                        path: path.clone(),
                        operation: "write trace artifact",
                        source,
                    })?;
                    written_paths.push(path);
                }
                CaptureKind::Snapshot => {
                    let Some(snapshot_text) = &artifacts.snapshot_text else {
                        continue;
                    };
                    let path = case_dir.join("snapshot.txt");
                    fs::write(&path, snapshot_text).map_err(|source| {
                        RomExecutionError::ReadFile {
                            path: path.clone(),
                            operation: "write snapshot artifact",
                            source,
                        }
                    })?;
                    written_paths.push(path);
                }
            }
        }

        Ok(written_paths)
    }
}

pub fn default_workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should be two levels above gb-test-runner")
        .to_path_buf()
}

fn budget_exhausted(timeout: Timeout, executed_t_cycles: u64, completed_frames: u32) -> bool {
    match timeout {
        Timeout::TCycles(limit) => executed_t_cycles >= limit,
        Timeout::Frames(limit) => {
            let fallback_t_cycle_budget =
                u64::from(limit).saturating_mul(DMG_FAMILY_FRAME_T_CYCLES);
            completed_frames >= limit || executed_t_cycles >= fallback_t_cycle_budget
        }
    }
}

fn memory_text_output_spec(pass_condition: &PassCondition) -> Option<&MemoryTextOutputSpec> {
    match pass_condition {
        PassCondition::MemoryTextOutputContains { spec, .. } => Some(spec),
        _ => None,
    }
}

fn memory_bytes_terminal(pass_condition: &PassCondition, machine: &mut RunnerMachine) -> bool {
    let PassCondition::MemoryBytesEqual(expectations) = pass_condition else {
        return false;
    };

    let captured = capture_memory_bytes(expectations, machine);
    captured
        .bytes
        .iter()
        .all(|byte| byte.actual == byte.expected)
        || captured
            .bytes
            .iter()
            .any(|byte| byte.fail_value == Some(byte.actual))
}

fn framebuffer_until_match_poll_due(
    case_id: &str,
    machine: &mut RunnerMachine,
    executed_t_cycles: u64,
    oracle: &mut FramebufferUntilMatchOracle,
) -> Result<bool, RomExecutionError> {
    if let Some(check_at_tcycles) = oracle.check_at_tcycles {
        if executed_t_cycles == check_at_tcycles {
            oracle.check_at_reached = true;
            oracle.matched = framebuffer_matches_fixture(case_id, machine, oracle)?;
            return Ok(true);
        }
        return Ok(false);
    }

    if executed_t_cycles != 0 && executed_t_cycles.is_multiple_of(oracle.check_interval_tcycles) {
        oracle.pending_periodic_check = true;
    }

    if oracle.pending_periodic_check && machine.in_vblank() {
        oracle.pending_periodic_check = false;
        if framebuffer_matches_fixture(case_id, machine, oracle)? {
            oracle.matched = true;
            return Ok(true);
        }
    }

    Ok(false)
}

fn framebuffer_matches_fixture(
    case_id: &str,
    machine: &RunnerMachine,
    oracle: &FramebufferUntilMatchOracle,
) -> Result<bool, RomExecutionError> {
    let actual = match oracle.source {
        FramebufferUntilMatchSource::Dmg => {
            normalize_dmg_framebuffer(case_id, machine.framebuffer()).map_err(|error| {
                let path = error.path.clone();
                RomExecutionError::ReadFile {
                    path,
                    operation: "normalize local framebuffer",
                    source: error.into_invalid_data_error(),
                }
            })?
        }
        FramebufferUntilMatchSource::Rgb555 => {
            let framebuffer_rgb555 =
                machine
                    .host_framebuffer_rgb555()
                    .ok_or_else(|| RomExecutionError::ReadFile {
                        path: PathBuf::from(format!(
                            "<local host RGB555 framebuffer for {case_id}>"
                        )),
                        operation: "decode local host RGB555 framebuffer artifact",
                        source: io::Error::new(
                            io::ErrorKind::InvalidData,
                            "missing local host RGB555 framebuffer capture",
                        ),
                    })?;
            decode_local_rgb555_framebuffer(case_id, framebuffer_rgb555.as_ref()).map_err(
                |error| {
                    let path = error.path.clone();
                    RomExecutionError::ReadFile {
                        path,
                        operation: "decode local host RGB555 framebuffer artifact",
                        source: error.into_invalid_data_error(),
                    }
                },
            )?
        }
    };
    Ok(actual == oracle.expected)
}

fn memory_text_output_completion_candidate(
    pass_condition: &PassCondition,
    machine: &mut RunnerMachine,
) -> Option<CapturedMemoryTextOutput> {
    let spec = memory_text_output_spec(pass_condition)?;

    let captured = capture_memory_text_output(spec, machine);
    (captured.signature == spec.expected_signature && captured.status != spec.running_status)
        .then_some(captured)
}

fn memory_text_output_completion_reached(
    last_candidate: &mut Option<CapturedMemoryTextOutput>,
    current_candidate: Option<CapturedMemoryTextOutput>,
) -> bool {
    match current_candidate {
        Some(candidate) if last_candidate.as_ref() == Some(&candidate) => true,
        Some(candidate) => {
            *last_candidate = Some(candidate);
            false
        }
        None => {
            *last_candidate = None;
            false
        }
    }
}

fn mooneye_result_completion_candidate(
    pass_condition: &PassCondition,
    machine: &mut RunnerMachine,
) -> Option<MooneyeTestResult> {
    match pass_condition {
        PassCondition::MooneyeResult => detect_mooneye_result(machine),
        _ => None,
    }
}

fn detect_mooneye_result(machine: &mut RunnerMachine) -> Option<MooneyeTestResult> {
    let snapshot = machine.cpu_snapshot();
    let result = mooneye_result_for_signature(&snapshot)?;

    if snapshot.current_opcode == Some(MOONEYE_MAGIC_BREAKPOINT_OPCODE)
        || mooneye_halt_loop_reached(machine, snapshot.registers.pc)
    {
        Some(result)
    } else {
        None
    }
}

fn mooneye_result_for_signature(snapshot: &CpuSnapshot) -> Option<MooneyeTestResult> {
    let signature = [
        snapshot.registers.b,
        snapshot.registers.c,
        snapshot.registers.d,
        snapshot.registers.e,
        snapshot.registers.h,
        snapshot.registers.l,
    ];

    if signature == MOONEYE_PASS_SIGNATURE {
        Some(MooneyeTestResult::Passed)
    } else if signature == MOONEYE_FAIL_SIGNATURE {
        Some(MooneyeTestResult::Failed)
    } else {
        None
    }
}

fn mooneye_halt_loop_reached(machine: &mut RunnerMachine, pc: u16) -> bool {
    (1..=4).any(|offset| mooneye_halt_loop_matches_at(machine, pc.wrapping_sub(offset)))
}

fn mooneye_halt_loop_matches_at(machine: &mut RunnerMachine, breakpoint_pc: u16) -> bool {
    MOONEYE_HALT_LOOP_BYTES
        .iter()
        .enumerate()
        .all(|(offset, expected)| {
            machine.read_bus(breakpoint_pc.wrapping_add(offset as u16)) == *expected
        })
}

fn blargg_console_text_complete(
    pass_condition: &PassCondition,
    machine: &mut RunnerMachine,
) -> bool {
    let PassCondition::BlarggConsoleTextContains(expected_substring) = pass_condition else {
        return false;
    };

    capture_blargg_console_text(machine).contains(expected_substring)
}

fn capture_memory_text_output(
    spec: &MemoryTextOutputSpec,
    machine: &mut RunnerMachine,
) -> CapturedMemoryTextOutput {
    let status = machine.read_bus(spec.status_address);
    let signature = [
        machine.read_bus(spec.signature_address),
        machine.read_bus(spec.signature_address.wrapping_add(1)),
        machine.read_bus(spec.signature_address.wrapping_add(2)),
    ];

    let max_text_bytes = spec
        .max_text_bytes
        .min(usize::from(u16::MAX - spec.text_address) + 1);
    let mut text_bytes = Vec::new();
    for offset in 0..max_text_bytes {
        let byte = machine.read_bus(spec.text_address.wrapping_add(offset as u16));
        if byte == 0 {
            break;
        }
        text_bytes.push(byte);
    }

    CapturedMemoryTextOutput {
        status,
        signature,
        text: String::from_utf8_lossy(&text_bytes).into_owned(),
    }
}

fn capture_memory_bytes(
    expectations: &[MemoryByteExpectation],
    machine: &mut RunnerMachine,
) -> CapturedMemoryBytes {
    CapturedMemoryBytes {
        bytes: expectations
            .iter()
            .map(|expectation| CapturedMemoryByte {
                address: expectation.address,
                expected: expectation.value,
                fail_value: expectation.fail_value,
                actual: machine.read_bus(expectation.address),
            })
            .collect(),
    }
}

pub(crate) fn render_memory_bytes(captured: &CapturedMemoryBytes) -> String {
    let mut rendered = String::new();
    for byte in &captured.bytes {
        if let Some(fail_value) = byte.fail_value {
            let _ = writeln!(
                &mut rendered,
                "address=0x{address:04X} expected=0x{expected:02X} fail_value=0x{fail_value:02X} actual=0x{actual:02X}",
                address = byte.address,
                expected = byte.expected,
                actual = byte.actual,
            );
        } else {
            let _ = writeln!(
                &mut rendered,
                "address=0x{address:04X} expected=0x{expected:02X} actual=0x{actual:02X}",
                address = byte.address,
                expected = byte.expected,
                actual = byte.actual,
            );
        }
    }
    rendered
}

pub(crate) fn render_memory_text_output(captured: &CapturedMemoryTextOutput) -> String {
    format!(
        "status=0x{status:02X}\nsignature={sig0:02X} {sig1:02X} {sig2:02X}\ntext={text:?}\n",
        status = captured.status,
        sig0 = captured.signature[0],
        sig1 = captured.signature[1],
        sig2 = captured.signature[2],
        text = captured.text,
    )
}

fn encode_bytes_as_upper_hex(bytes: &[u8]) -> String {
    let mut rendered = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut rendered, "{byte:02X}");
    }
    rendered
}

#[cfg(test)]
pub(crate) fn artifact_file_name(capture: CaptureKind) -> &'static str {
    match capture {
        CaptureKind::Serial => "serial.txt",
        CaptureKind::SerialHex => "serial_hex.txt",
        CaptureKind::MemoryBytes => "memory_bytes.txt",
        CaptureKind::MemoryTextOutput => "memory_text_output.txt",
        CaptureKind::BlarggConsoleText => "blargg_console.txt",
        CaptureKind::Framebuffer => "framebuffer.png",
        CaptureKind::Trace => "trace.txt",
        CaptureKind::Snapshot => "snapshot.txt",
    }
}

fn capture_blargg_console_text(machine: &mut RunnerMachine) -> String {
    const BLARGG_CONSOLE_WIDTH: usize = 20;
    const BLARGG_CONSOLE_HEIGHT: usize = 18;
    const BLARGG_CONSOLE_BG_MAP0: u16 = 0x9800;
    const BG_MAP_STRIDE: usize = 32;
    const SCY_ADDRESS: u16 = 0xFF42;

    let scroll_y = machine.read_bus(SCY_ADDRESS);
    let top_row = (usize::from(scroll_y) / 8) % BG_MAP_STRIDE;
    let mut lines = Vec::with_capacity(BLARGG_CONSOLE_HEIGHT);

    for visible_row in 0..BLARGG_CONSOLE_HEIGHT {
        let map_row = (top_row + visible_row) % BG_MAP_STRIDE;
        let mut line = String::with_capacity(BLARGG_CONSOLE_WIDTH);

        for column in 0..BLARGG_CONSOLE_WIDTH {
            let address = BLARGG_CONSOLE_BG_MAP0 + (map_row * BG_MAP_STRIDE + column) as u16;
            let tile = machine.read_bus(address) & 0x7F;
            let ch = match tile {
                0x20..=0x7E => char::from(tile),
                _ => ' ',
            };
            line.push(ch);
        }

        lines.push(line.trim_end().to_string());
    }

    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }

    lines.join("\n")
}

fn stop_condition_satisfied(
    stop_condition: Option<ExecutionStopCondition>,
    machine: &mut RunnerMachine,
) -> bool {
    match stop_condition {
        Some(ExecutionStopCondition::MemoryEquals { address, value }) => {
            machine.read_bus(address) == value
        }
        Some(ExecutionStopCondition::CurrentOpcodeEquals { opcode }) => {
            let snapshot = machine.cpu_snapshot();
            snapshot.current_opcode == Some(opcode)
                || snapshot.last_bus_activity.is_some_and(|activity| {
                    activity.kind == CpuBusAccessKind::OpcodeFetch && activity.value == opcode
                })
        }
        None => false,
    }
}

fn discard_trace_events_if_needed(trace_buffer: &mut TraceBuffer, executed_t_cycles: u64) {
    const TRACE_CLEAR_PERIOD_T_CYCLES: u64 = 8_192;

    if executed_t_cycles != 0 && executed_t_cycles.is_multiple_of(TRACE_CLEAR_PERIOD_T_CYCLES) {
        trace_buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BOOT_ROM_ROOT_ENV_VAR, BootRomAssets, BootRomVerificationIssue, BootRomVerificationMode,
        CaptureKind, CapturedArtifacts, CapturedMemoryTextOutput, CaseEvaluationInputs,
        DMG_FAMILY_FRAME_T_CYCLES, DeterministicMbc3RtcClock, ExecutionStopCondition,
        FailureArtifactPolicy, GB_EMULATOR_SHOOTOUT_REPORT_ID, InformationalCaptureKind,
        MOONEYE_FAIL_SIGNATURE, MOONEYE_PASS_SIGNATURE, MemoryTextOutputSpec, MooneyeTestResult,
        PassCondition, RomCaseFailure, RomCaseOutcome, RomCaseValidationError, RomExecutionError,
        RomRunner, RomTestCase, RunnerMachine, TEST_ROM_ROOT_ENV_VAR, TEST_ROM_STORE_DIR, Timeout,
        artifact_file_name, blargg_console_text_complete, budget_exhausted,
        capture_blargg_console_text, capture_memory_text_output, detect_mooneye_result,
        memory_text_output_completion_reached, mooneye_result_completion_candidate,
        mooneye_result_for_signature, render_memory_text_output,
    };
    use crate::framebuffer_oracle::{
        decode_fixture_framebuffer_path, encode_framebuffer_pgm, encode_rgb555_framebuffer_png,
    };
    use gb_core::{
        BootRomAssetKind, CgbSpeedMode, ConsoleModel, CpuExecutionState, CpuRegisters, CpuSnapshot,
        CpuStartupState, CpuStatus, HostPlatform, StartupMode,
    };
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    const TEST_ROM_MINIMUM_LEN: usize = 32 * 1024;

    fn unique_temp_dir(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "gb-cycle-test-runner-lib-{}-{}-{}",
            label,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ))
    }

    fn set_env_var(key: &str, value: impl AsRef<std::ffi::OsStr>) {
        // SAFETY: these tests serialize environment mutation through `env_lock()`
        // and restore touched variables before dropping the guard.
        unsafe {
            env::set_var(key, value);
        }
    }

    fn remove_env_var(key: &str) {
        // SAFETY: these tests serialize environment mutation through `env_lock()`
        // and restore touched variables before dropping the guard.
        unsafe {
            env::remove_var(key);
        }
    }

    fn build_test_rom(program: &[u8]) -> Vec<u8> {
        let mut rom = vec![0xFF; TEST_ROM_MINIMUM_LEN];
        for (offset, byte) in program.iter().copied().enumerate() {
            rom[0x0100 + offset] = byte;
        }
        rom[0x0147] = 0x00;
        rom[0x0148] = 0x00;
        rom[0x0149] = 0x00;
        rom
    }

    fn build_mooneye_result_rom(signature: [u8; 6]) -> Vec<u8> {
        build_test_rom(&[
            0x06,
            signature[0], // LD B,d8
            0x0E,
            signature[1], // LD C,d8
            0x16,
            signature[2], // LD D,d8
            0x1E,
            signature[3], // LD E,d8
            0x26,
            signature[4], // LD H,d8
            0x2E,
            signature[5], // LD L,d8
            0x40,         // LD B,B
            0x00,         // NOP
            0x18,
            0xFD, // JR -3
        ])
    }

    fn mooneye_result_machine(signature: [u8; 6]) -> RunnerMachine {
        let case = RomTestCase::new(
            "mooneye-result-fixture",
            "/dev/null",
            Timeout::TCycles(512),
            PassCondition::MooneyeResult,
        );
        let mut machine = RunnerMachine::new(&case, BootRomAssets::none());
        machine
            .load_cartridge(build_mooneye_result_rom(signature))
            .expect("fixture rom should load");
        machine
    }

    fn evaluation_inputs<'a>(
        artifacts: &'a CapturedArtifacts,
        executed_t_cycles: u64,
        completed_frames: u32,
    ) -> CaseEvaluationInputs<'a> {
        CaseEvaluationInputs {
            artifacts,
            serial_contains_matched: false,
            diagnostic_trap: None,
            mooneye_result: None,
            framebuffer_until_match_matched: false,
            framebuffer_until_match_check_at_reached: false,
            executed_t_cycles,
            completed_frames,
        }
    }

    #[test]
    fn memory_text_output_completion_requires_two_identical_final_observations() {
        let transient = CapturedMemoryTextOutput {
            status: 0x00,
            signature: [0xDE, 0xB0, 0x61],
            text: String::new(),
        };
        let final_failure = CapturedMemoryTextOutput {
            status: 0x03,
            signature: [0xDE, 0xB0, 0x61],
            text: "Failed #3\n".to_string(),
        };

        let mut last_candidate = None;
        assert!(!memory_text_output_completion_reached(
            &mut last_candidate,
            Some(transient.clone())
        ));
        assert!(!memory_text_output_completion_reached(
            &mut last_candidate,
            None
        ));
        assert!(!memory_text_output_completion_reached(
            &mut last_candidate,
            Some(final_failure.clone())
        ));
        assert!(memory_text_output_completion_reached(
            &mut last_candidate,
            Some(final_failure)
        ));
    }

    #[test]
    fn deterministic_mbc3_rtc_clock_scales_with_cgb_speed_mode() {
        let mut clock = DeterministicMbc3RtcClock::default();

        for _ in 0..127 {
            assert_eq!(clock.tick_t_cycle_for_speed(CgbSpeedMode::Normal), 0);
        }
        assert_eq!(clock.tick_t_cycle_for_speed(CgbSpeedMode::Normal), 1);

        for _ in 0..255 {
            assert_eq!(clock.tick_t_cycle_for_speed(CgbSpeedMode::Double), 0);
        }
        assert_eq!(clock.tick_t_cycle_for_speed(CgbSpeedMode::Double), 1);

        for _ in 0..64 {
            assert_eq!(clock.tick_t_cycle_for_speed(CgbSpeedMode::Normal), 0);
        }
        for _ in 0..127 {
            assert_eq!(clock.tick_t_cycle_for_speed(CgbSpeedMode::Double), 0);
        }
        assert_eq!(clock.tick_t_cycle_for_speed(CgbSpeedMode::Double), 1);
    }

    #[test]
    fn mooneye_result_for_signature_requires_matching_registers() {
        let mut snapshot = CpuSnapshot {
            console_model: ConsoleModel::GameBoy,
            status: CpuStatus::Ready,
            startup_state: CpuStartupState {
                a: 0,
                f: 0,
                b: 0,
                c: 0,
                d: 0,
                e: 0,
                h: 0,
                l: 0,
                sp: 0,
                pc: 0,
            },
            registers: CpuRegisters {
                a: 0,
                f: 0,
                b: 3,
                c: 5,
                d: 8,
                e: 13,
                h: 21,
                l: 34,
                sp: 0,
                pc: 0x0150,
            },
            execution_state: CpuExecutionState::Execute {
                step: 0,
                t_cycle: 0,
            },
            current_opcode: Some(0x40),
            ime: false,
            delayed_ime_enable: false,
            last_bus_activity: None,
            last_address_event: None,
        };

        assert_eq!(
            mooneye_result_for_signature(&snapshot),
            Some(MooneyeTestResult::Passed)
        );

        snapshot.registers.b = 0x42;
        snapshot.registers.c = 0x42;
        snapshot.registers.d = 0x42;
        snapshot.registers.e = 0x42;
        snapshot.registers.h = 0x42;
        snapshot.registers.l = 0x42;
        assert_eq!(
            mooneye_result_for_signature(&snapshot),
            Some(MooneyeTestResult::Failed)
        );

        snapshot.registers.b = 0x00;
        assert_eq!(mooneye_result_for_signature(&snapshot), None);
    }

    #[test]
    fn detect_mooneye_result_recognizes_the_post_breakpoint_halt_loop() {
        let mut passing_machine = mooneye_result_machine(MOONEYE_PASS_SIGNATURE);
        for _ in 0..200 {
            passing_machine.step_t_cycle();
        }
        assert_eq!(
            detect_mooneye_result(&mut passing_machine),
            Some(MooneyeTestResult::Passed)
        );

        let mut failing_machine = mooneye_result_machine(MOONEYE_FAIL_SIGNATURE);
        for _ in 0..200 {
            failing_machine.step_t_cycle();
        }
        assert_eq!(
            detect_mooneye_result(&mut failing_machine),
            Some(MooneyeTestResult::Failed)
        );
    }

    #[test]
    fn evaluate_case_covers_serial_text_and_mooneye_outcomes() {
        let runner = RomRunner::new();

        let serial_case = RomTestCase::new(
            "serial-contains",
            "unused.gb",
            Timeout::TCycles(4),
            PassCondition::SerialContains("OK".to_string()),
        );
        let serial_artifacts = CapturedArtifacts {
            serial: Some("no".to_string()),
            ..CapturedArtifacts::default()
        };
        let mut serial_matched = evaluation_inputs(&serial_artifacts, 2, 0);
        serial_matched.serial_contains_matched = true;
        assert_eq!(
            runner
                .evaluate_case(&serial_case, &serial_matched)
                .expect("matched serial case should evaluate"),
            RomCaseOutcome::Passed
        );
        assert_eq!(
            runner
                .evaluate_case(&serial_case, &evaluation_inputs(&serial_artifacts, 4, 0))
                .expect("exhausted serial case should evaluate"),
            RomCaseOutcome::Failed(RomCaseFailure::SerialMissingSubstring {
                expected_substring: "OK".to_string(),
                actual: "no".to_string(),
            })
        );
        assert_eq!(
            runner
                .evaluate_case(&serial_case, &evaluation_inputs(&serial_artifacts, 2, 0))
                .expect("non-exhausted serial case should evaluate"),
            RomCaseOutcome::Failed(RomCaseFailure::TimeoutExceeded)
        );

        let serial_exact_case = RomTestCase::new(
            "serial-exact",
            "unused.gb",
            Timeout::TCycles(4),
            PassCondition::SerialExact("OK".to_string()),
        );
        let serial_exact_artifacts = CapturedArtifacts {
            serial: Some("OK".to_string()),
            ..CapturedArtifacts::default()
        };
        assert_eq!(
            runner
                .evaluate_case(
                    &serial_exact_case,
                    &evaluation_inputs(&serial_exact_artifacts, 4, 0)
                )
                .expect("serial exact pass should evaluate"),
            RomCaseOutcome::Passed
        );
        assert_eq!(
            runner
                .evaluate_case(
                    &serial_exact_case,
                    &evaluation_inputs(&serial_exact_artifacts, 2, 0)
                )
                .expect("serial exact timeout should evaluate"),
            RomCaseOutcome::Failed(RomCaseFailure::TimeoutExceeded)
        );

        let serial_hex_case = RomTestCase::new(
            "serial-hex",
            "unused.gb",
            Timeout::TCycles(4),
            PassCondition::SerialHexExact("4F4B".to_string()),
        );
        let serial_hex_pass = CapturedArtifacts {
            serial_hex: Some("4F4B".to_string()),
            ..CapturedArtifacts::default()
        };
        assert_eq!(
            runner
                .evaluate_case(&serial_hex_case, &evaluation_inputs(&serial_hex_pass, 4, 0))
                .expect("serial hex pass should evaluate"),
            RomCaseOutcome::Passed
        );
        let serial_hex_mismatch = CapturedArtifacts {
            serial_hex: Some("4F".to_string()),
            ..CapturedArtifacts::default()
        };
        assert_eq!(
            runner
                .evaluate_case(
                    &serial_hex_case,
                    &evaluation_inputs(&serial_hex_mismatch, 4, 0)
                )
                .expect("serial hex mismatch should evaluate"),
            RomCaseOutcome::Failed(RomCaseFailure::SerialExactMismatch {
                expected: "4F4B".to_string(),
                actual: "4F".to_string(),
            })
        );

        let memory_spec =
            MemoryTextOutputSpec::new(0xA000, 0x80, 0x00, 0xA001, [0xDE, 0xB0, 0x61], 0xA004, 64);
        let memory_case = RomTestCase::new(
            "memory-text",
            "unused.gb",
            Timeout::TCycles(4),
            PassCondition::MemoryTextOutputContains {
                spec: memory_spec,
                expected_substring: "Passed".to_string(),
            },
        );
        let memory_artifacts = CapturedArtifacts {
            memory_text_output: Some(CapturedMemoryTextOutput {
                status: 0x03,
                signature: [0xDE, 0xB0, 0x61],
                text: "Failed".to_string(),
            }),
            ..CapturedArtifacts::default()
        };
        assert_eq!(
            runner
                .evaluate_case(&memory_case, &evaluation_inputs(&memory_artifacts, 4, 0))
                .expect("memory mismatch should evaluate"),
            RomCaseOutcome::Failed(RomCaseFailure::MemoryTextOutputMismatch {
                expected_substring: "Passed".to_string(),
                pass_status: 0x00,
                expected_signature: [0xDE, 0xB0, 0x61],
                actual_status: 0x03,
                actual_signature: [0xDE, 0xB0, 0x61],
                actual_text: "Failed".to_string(),
            })
        );

        let blargg_case = RomTestCase::new(
            "blargg-text",
            "unused.gb",
            Timeout::TCycles(4),
            PassCondition::BlarggConsoleTextContains("Passed".to_string()),
        );
        let blargg_artifacts = CapturedArtifacts {
            blargg_console_text: Some("Still running".to_string()),
            ..CapturedArtifacts::default()
        };
        assert_eq!(
            runner
                .evaluate_case(&blargg_case, &evaluation_inputs(&blargg_artifacts, 4, 0))
                .expect("blargg mismatch should evaluate"),
            RomCaseOutcome::Failed(RomCaseFailure::BlarggConsoleTextMissingSubstring {
                expected_substring: "Passed".to_string(),
                actual: "Still running".to_string(),
            })
        );

        let mooneye_case = RomTestCase::new(
            "mooneye",
            "unused.gb",
            Timeout::TCycles(4),
            PassCondition::MooneyeResult,
        );
        let empty_artifacts = CapturedArtifacts::default();
        let mut mooneye_pass = evaluation_inputs(&empty_artifacts, 1, 0);
        mooneye_pass.mooneye_result = Some(MooneyeTestResult::Passed);
        assert_eq!(
            runner
                .evaluate_case(&mooneye_case, &mooneye_pass)
                .expect("mooneye pass should evaluate"),
            RomCaseOutcome::Passed
        );
        let mut mooneye_fail = evaluation_inputs(&empty_artifacts, 1, 0);
        mooneye_fail.mooneye_result = Some(MooneyeTestResult::Failed);
        assert_eq!(
            runner
                .evaluate_case(&mooneye_case, &mooneye_fail)
                .expect("mooneye fail should evaluate"),
            RomCaseOutcome::Failed(RomCaseFailure::MooneyeFailureSignature)
        );
        assert_eq!(
            runner
                .evaluate_case(&mooneye_case, &evaluation_inputs(&empty_artifacts, 1, 0))
                .expect("missing mooneye result should evaluate"),
            RomCaseOutcome::Failed(RomCaseFailure::MooneyeResultNotReached)
        );
    }

    #[test]
    fn evaluate_case_covers_trace_and_framebuffer_fixture_oracles() {
        let runner = RomRunner::new();
        let temp_dir = unique_temp_dir("fixture-oracles");
        fs::create_dir_all(&temp_dir).expect("temp dir should be creatable");

        let trace_path = temp_dir.join("expected.trace");
        fs::write(&trace_path, "trace-ok").expect("trace fixture should be writable");
        let trace_case = RomTestCase::new(
            "trace-case",
            "unused.gb",
            Timeout::TCycles(1),
            PassCondition::TraceFixture(trace_path.clone()),
        );
        let trace_artifacts = CapturedArtifacts {
            trace: Some("trace-ok".to_string()),
            ..CapturedArtifacts::default()
        };
        assert_eq!(
            runner
                .evaluate_case(&trace_case, &evaluation_inputs(&trace_artifacts, 1, 0))
                .expect("trace fixture should match"),
            RomCaseOutcome::Passed
        );
        let trace_mismatch_artifacts = CapturedArtifacts {
            trace: Some("trace-other".to_string()),
            ..CapturedArtifacts::default()
        };
        assert_eq!(
            runner
                .evaluate_case(
                    &trace_case,
                    &evaluation_inputs(&trace_mismatch_artifacts, 1, 0)
                )
                .expect("trace mismatch should evaluate"),
            RomCaseOutcome::Failed(RomCaseFailure::TraceFixtureMismatch {
                fixture_path: trace_path.clone(),
            })
        );

        let missing_trace_case = RomTestCase::new(
            "trace-missing",
            "unused.gb",
            Timeout::TCycles(1),
            PassCondition::TraceFixture(temp_dir.join("missing.trace")),
        );
        let missing_trace = runner
            .evaluate_case(
                &missing_trace_case,
                &evaluation_inputs(&trace_artifacts, 1, 0),
            )
            .expect_err("missing trace fixture should fail");
        assert!(matches!(
            missing_trace,
            RomExecutionError::ReadFile {
                operation: "read trace fixture",
                ..
            }
        ));

        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("data/gb-emulator-shootout/fixtures/samesuite/sgb/command_mlt_req.png");
        let expected = decode_fixture_framebuffer_path(&fixture_path)
            .expect("fixture framebuffer should decode");
        let framebuffer_case = RomTestCase::new(
            "framebuffer-case",
            "unused.gb",
            Timeout::TCycles(1),
            PassCondition::FramebufferFixture(fixture_path.clone()),
        );
        let framebuffer_artifacts = CapturedArtifacts {
            framebuffer_pgm: Some(encode_framebuffer_pgm(&expected.palette_ranks)),
            ..CapturedArtifacts::default()
        };
        assert_eq!(
            runner
                .evaluate_case(
                    &framebuffer_case,
                    &evaluation_inputs(&framebuffer_artifacts, 1, 0)
                )
                .expect("framebuffer fixture should match"),
            RomCaseOutcome::Passed
        );

        let mut altered_ranks = expected.palette_ranks.clone();
        altered_ranks[0] = (altered_ranks[0] + 1) % 4;
        let framebuffer_mismatch_artifacts = CapturedArtifacts {
            framebuffer_pgm: Some(encode_framebuffer_pgm(&altered_ranks)),
            ..CapturedArtifacts::default()
        };
        assert_eq!(
            runner
                .evaluate_case(
                    &framebuffer_case,
                    &evaluation_inputs(&framebuffer_mismatch_artifacts, 1, 0)
                )
                .expect("framebuffer mismatch should evaluate"),
            RomCaseOutcome::Failed(RomCaseFailure::FramebufferFixtureMismatch {
                fixture_path: fixture_path.clone(),
            })
        );

        let framebuffer_until_match_case = RomTestCase::new(
            "framebuffer-until-match",
            "unused.gb",
            Timeout::TCycles(1),
            PassCondition::FramebufferFixtureUntilMatch {
                fixture_path: fixture_path.clone(),
                check_interval_tcycles: 1,
                check_at_tcycles: Some(1),
            },
        );
        let mut matched_early = evaluation_inputs(&framebuffer_mismatch_artifacts, 1, 0);
        matched_early.framebuffer_until_match_matched = true;
        assert_eq!(
            runner
                .evaluate_case(&framebuffer_until_match_case, &matched_early)
                .expect("framebuffer until-match should trust an early match"),
            RomCaseOutcome::Passed
        );
        assert_eq!(
            runner
                .evaluate_case(
                    &framebuffer_until_match_case,
                    &evaluation_inputs(&framebuffer_artifacts, 0, 0)
                )
                .expect("framebuffer until-match should report an unreached check_at"),
            RomCaseOutcome::Failed(RomCaseFailure::FramebufferCheckAtNotReached {
                check_at_tcycles: 1,
                executed_t_cycles: 0,
            })
        );
        let mut check_at_reached_match = evaluation_inputs(&framebuffer_artifacts, 1, 0);
        check_at_reached_match.framebuffer_until_match_check_at_reached = true;
        assert_eq!(
            runner
                .evaluate_case(&framebuffer_until_match_case, &check_at_reached_match)
                .expect("framebuffer until-match fallback fixture should match"),
            RomCaseOutcome::Passed
        );
        let mut check_at_reached_mismatch =
            evaluation_inputs(&framebuffer_mismatch_artifacts, 1, 0);
        check_at_reached_mismatch.framebuffer_until_match_check_at_reached = true;
        assert_eq!(
            runner
                .evaluate_case(&framebuffer_until_match_case, &check_at_reached_mismatch)
                .expect("framebuffer until-match mismatch should evaluate"),
            RomCaseOutcome::Failed(RomCaseFailure::FramebufferFixtureMismatch {
                fixture_path: fixture_path.clone(),
            })
        );

        let grayscale_fixture_path = temp_dir.join("white.pgm");
        fs::write(
            &grayscale_fixture_path,
            encode_framebuffer_pgm(&vec![0; 160 * 144]),
        )
        .expect("grayscale fixture should be writable");
        let grayscale_case = RomTestCase::new(
            "grayscale-framebuffer-case",
            "unused.gb",
            Timeout::TCycles(1),
            PassCondition::FramebufferGrayscaleFixture(grayscale_fixture_path.clone()),
        );
        let grayscale_white_artifacts = CapturedArtifacts {
            framebuffer_pgm: Some(encode_framebuffer_pgm(&vec![0; 160 * 144])),
            ..CapturedArtifacts::default()
        };
        let grayscale_black_artifacts = CapturedArtifacts {
            framebuffer_pgm: Some(encode_framebuffer_pgm(&vec![3; 160 * 144])),
            ..CapturedArtifacts::default()
        };
        assert_eq!(
            runner
                .evaluate_case(
                    &grayscale_case,
                    &evaluation_inputs(&grayscale_white_artifacts, 1, 0)
                )
                .expect("grayscale framebuffer fixture should match"),
            RomCaseOutcome::Passed
        );
        assert_eq!(
            runner
                .evaluate_case(
                    &grayscale_case,
                    &evaluation_inputs(&grayscale_black_artifacts, 1, 0)
                )
                .expect("grayscale framebuffer mismatch should evaluate"),
            RomCaseOutcome::Failed(RomCaseFailure::FramebufferFixtureMismatch {
                fixture_path: grayscale_fixture_path.clone(),
            })
        );

        let mut rgb555_pixels = vec![0x0000_u16; 160 * 144];
        rgb555_pixels[0] = 0x7FFF;
        let rgb555_fixture_path = temp_dir.join("rgb555.png");
        fs::write(
            &rgb555_fixture_path,
            encode_rgb555_framebuffer_png(&rgb555_pixels)
                .expect("RGB555 fixture should encode to PNG"),
        )
        .expect("RGB555 fixture should be writable");
        let rgb555_case = RomTestCase::new(
            "rgb555-framebuffer-case",
            "unused.gb",
            Timeout::TCycles(1),
            PassCondition::FramebufferRgb555Fixture(rgb555_fixture_path.clone()),
        );
        let rgb555_artifacts = CapturedArtifacts {
            framebuffer_rgb555: Some(rgb555_pixels.clone()),
            ..CapturedArtifacts::default()
        };
        assert_eq!(
            runner
                .evaluate_case(&rgb555_case, &evaluation_inputs(&rgb555_artifacts, 1, 0))
                .expect("RGB555 fixture should match"),
            RomCaseOutcome::Passed
        );
        let rgb555_mismatch_artifacts = CapturedArtifacts {
            framebuffer_rgb555: Some(vec![0x0000; 160 * 144]),
            ..CapturedArtifacts::default()
        };
        assert_eq!(
            runner
                .evaluate_case(
                    &rgb555_case,
                    &evaluation_inputs(&rgb555_mismatch_artifacts, 1, 0)
                )
                .expect("RGB555 fixture mismatch should evaluate"),
            RomCaseOutcome::Failed(RomCaseFailure::FramebufferFixtureMismatch {
                fixture_path: rgb555_fixture_path.clone(),
            })
        );

        let rgb555_grayscale_fixture_path = temp_dir.join("rgb555-white.pgm");
        fs::write(
            &rgb555_grayscale_fixture_path,
            encode_framebuffer_pgm(&vec![0; 160 * 144]),
        )
        .expect("RGB555 grayscale fixture should be writable");
        let rgb555_grayscale_case = RomTestCase::new(
            "rgb555-grayscale-framebuffer-case",
            "unused.gb",
            Timeout::TCycles(1),
            PassCondition::FramebufferRgb555GrayscaleFixture(rgb555_grayscale_fixture_path.clone()),
        );
        let rgb555_white_artifacts = CapturedArtifacts {
            framebuffer_rgb555: Some(vec![0x7FFF; 160 * 144]),
            ..CapturedArtifacts::default()
        };
        let rgb555_black_artifacts = CapturedArtifacts {
            framebuffer_rgb555: Some(vec![0x0000; 160 * 144]),
            ..CapturedArtifacts::default()
        };
        assert_eq!(
            runner
                .evaluate_case(
                    &rgb555_grayscale_case,
                    &evaluation_inputs(&rgb555_white_artifacts, 1, 0)
                )
                .expect("RGB555 grayscale fixture should match"),
            RomCaseOutcome::Passed
        );
        assert_eq!(
            runner
                .evaluate_case(
                    &rgb555_grayscale_case,
                    &evaluation_inputs(&rgb555_black_artifacts, 1, 0)
                )
                .expect("RGB555 grayscale mismatch should evaluate"),
            RomCaseOutcome::Failed(RomCaseFailure::FramebufferFixtureMismatch {
                fixture_path: rgb555_grayscale_fixture_path.clone(),
            })
        );

        let rgb555_grayscale_tolerance_fixture_path = temp_dir.join("rgb555-tolerance.pgm");
        let mut tolerance_fixture_pixels = vec![255_u8; 160 * 144];
        tolerance_fixture_pixels[0] = 205;
        fs::write(
            &rgb555_grayscale_tolerance_fixture_path,
            format!("P5\n{} {}\n255\n", 160, 144)
                .into_bytes()
                .into_iter()
                .chain(tolerance_fixture_pixels.iter().copied())
                .collect::<Vec<_>>(),
        )
        .expect("RGB555 grayscale tolerance fixture should be writable");
        let rgb555_grayscale_tolerance_case = RomTestCase::new(
            "rgb555-grayscale-tolerance-framebuffer-case",
            "unused.gb",
            Timeout::TCycles(1),
            PassCondition::FramebufferRgb555GrayscaleToleranceFixture(
                rgb555_grayscale_tolerance_fixture_path.clone(),
            ),
        );
        let mut rgb555_tolerance_pixels = vec![0x7FFF_u16; 160 * 144];
        rgb555_tolerance_pixels[0] = 0x7FFF;
        let rgb555_tolerance_artifacts = CapturedArtifacts {
            framebuffer_rgb555: Some(rgb555_tolerance_pixels.clone()),
            ..CapturedArtifacts::default()
        };
        assert_eq!(
            runner
                .evaluate_case(
                    &rgb555_grayscale_tolerance_case,
                    &evaluation_inputs(&rgb555_tolerance_artifacts, 1, 0)
                )
                .expect("RGB555 grayscale tolerance fixture should match within tolerance"),
            RomCaseOutcome::Passed
        );
        tolerance_fixture_pixels[0] = 204;
        fs::write(
            &rgb555_grayscale_tolerance_fixture_path,
            format!("P5\n{} {}\n255\n", 160, 144)
                .into_bytes()
                .into_iter()
                .chain(tolerance_fixture_pixels.iter().copied())
                .collect::<Vec<_>>(),
        )
        .expect("RGB555 grayscale tolerance fixture should be writable");
        assert_eq!(
            runner
                .evaluate_case(
                    &rgb555_grayscale_tolerance_case,
                    &evaluation_inputs(&rgb555_tolerance_artifacts, 1, 0)
                )
                .expect("RGB555 grayscale tolerance mismatch should evaluate"),
            RomCaseOutcome::Failed(RomCaseFailure::FramebufferFixtureMismatch {
                fixture_path: rgb555_grayscale_tolerance_fixture_path.clone(),
            })
        );

        let missing_local_artifacts = CapturedArtifacts::default();
        let missing_local = runner
            .evaluate_case(
                &framebuffer_case,
                &evaluation_inputs(&missing_local_artifacts, 1, 0),
            )
            .expect_err("missing local framebuffer should fail");
        assert!(matches!(
            missing_local,
            RomExecutionError::ReadFile {
                operation: "decode local framebuffer artifact",
                ..
            }
        ));
        let missing_rgb555_local = runner
            .evaluate_case(
                &rgb555_case,
                &evaluation_inputs(&missing_local_artifacts, 1, 0),
            )
            .expect_err("missing local RGB555 framebuffer should fail");
        assert!(matches!(
            missing_rgb555_local,
            RomExecutionError::ReadFile {
                operation: "decode local host RGB555 framebuffer artifact",
                ..
            }
        ));

        let framebuffer_set_case = RomTestCase::new(
            "framebuffer-set",
            "unused.gb",
            Timeout::TCycles(1),
            PassCondition::FramebufferFixtureSet(vec![fixture_path.clone()]),
        );
        assert_eq!(
            runner
                .evaluate_case(
                    &framebuffer_set_case,
                    &evaluation_inputs(&framebuffer_artifacts, 1, 0)
                )
                .expect("framebuffer set should match"),
            RomCaseOutcome::Passed
        );
        assert_eq!(
            runner
                .evaluate_case(
                    &framebuffer_set_case,
                    &evaluation_inputs(&framebuffer_mismatch_artifacts, 1, 0)
                )
                .expect("framebuffer set mismatch should evaluate"),
            RomCaseOutcome::Failed(RomCaseFailure::FramebufferFixtureSetMismatch {
                fixture_paths: vec![fixture_path.clone()],
            })
        );

        fs::remove_dir_all(temp_dir).expect("temp dir should be removable");
    }

    #[test]
    fn persist_failure_artifacts_writes_all_supported_capture_channels() {
        let artifact_root = unique_temp_dir("persist-artifacts");
        let runner = RomRunner::new().with_failure_artifact_root(&artifact_root);
        let case = RomTestCase::new(
            "artifact-case",
            "unused.gb",
            Timeout::TCycles(1),
            PassCondition::SerialExact("unused".to_string()),
        )
        .with_failure_artifacts(
            FailureArtifactPolicy::new()
                .with_artifact(CaptureKind::Serial)
                .with_artifact(CaptureKind::SerialHex)
                .with_artifact(CaptureKind::MemoryTextOutput)
                .with_artifact(CaptureKind::BlarggConsoleText)
                .with_artifact(CaptureKind::Framebuffer)
                .with_artifact(CaptureKind::Trace)
                .with_artifact(CaptureKind::Snapshot),
        );
        let artifacts = CapturedArtifacts {
            serial: Some("serial".to_string()),
            serial_hex: Some("73657269616C".to_string()),
            memory_bytes: None,
            memory_text_output: Some(CapturedMemoryTextOutput {
                status: 0x00,
                signature: [0xDE, 0xB0, 0x61],
                text: "Passed".to_string(),
            }),
            blargg_console_text: Some("console".to_string()),
            framebuffer_pgm: Some(encode_framebuffer_pgm(&vec![0; 160 * 144])),
            framebuffer_rgb555: Some(vec![0x7FFF; 160 * 144]),
            trace: Some("trace".to_string()),
            snapshot_text: Some("snapshot".to_string()),
        };

        let without_root = RomRunner::new()
            .persist_failure_artifacts(&case, &artifacts)
            .expect("persist without root should succeed");
        assert!(without_root.is_empty());

        let written = runner
            .persist_failure_artifacts(&case, &artifacts)
            .expect("persisting failure artifacts should succeed");
        assert_eq!(written.len(), 7);
        for capture in [
            CaptureKind::Serial,
            CaptureKind::SerialHex,
            CaptureKind::MemoryTextOutput,
            CaptureKind::BlarggConsoleText,
            CaptureKind::Framebuffer,
            CaptureKind::Trace,
            CaptureKind::Snapshot,
        ] {
            assert!(
                written
                    .iter()
                    .any(|path| path.ends_with(artifact_file_name(capture))),
                "missing persisted artifact for {capture:?}"
            );
        }
        assert!(
            !written
                .iter()
                .any(|path| path.ends_with(Path::new("framebuffer.pgm"))),
            "host RGB555 framebuffer artifacts should not persist a legacy PGM"
        );

        let case_dir = artifact_root.join("artifact-case");
        assert_eq!(
            fs::read_to_string(case_dir.join("serial_hex.txt"))
                .expect("serial hex artifact should be readable"),
            "73657269616C"
        );
        assert_eq!(
            fs::read_to_string(case_dir.join("memory_text_output.txt"))
                .expect("memory text output artifact should be readable"),
            render_memory_text_output(
                artifacts
                    .memory_text_output
                    .as_ref()
                    .expect("memory text output should be present")
            )
        );
        assert!(case_dir.join("framebuffer.png").is_file());
        assert!(!case_dir.join("framebuffer.pgm").exists());
        assert!(!case_dir.join("framebuffer_rgb555.png").exists());

        fs::remove_dir_all(artifact_root).expect("artifact root should be removable");
    }

    #[test]
    fn validate_rejects_framebuffer_check_at_after_tcycle_timeout() {
        let case = RomTestCase::new(
            "framebuffer-check-at-after-timeout",
            "unused.gb",
            Timeout::TCycles(8),
            PassCondition::FramebufferFixtureUntilMatch {
                fixture_path: PathBuf::from("unused.pgm"),
                check_interval_tcycles: 1,
                check_at_tcycles: Some(16),
            },
        );

        assert_eq!(
            case.validate(),
            Err(RomCaseValidationError::FramebufferCheckAtExceedsTimeout {
                check_at_tcycles: 16,
                timeout_tcycles: 8,
            })
        );
    }

    #[test]
    fn run_case_supports_framebuffer_until_match_check_at_tcycle() {
        let workspace = unique_temp_dir("framebuffer-until-match-check-at-pass");
        fs::create_dir_all(&workspace).expect("workspace should be creatable");
        let rom_path = workspace.join("idle.gb");
        let fixture_path = workspace.join("white.pgm");
        fs::write(&rom_path, build_test_rom(&[0xC3, 0x00, 0x01]))
            .expect("test ROM should be writable");
        fs::write(&fixture_path, encode_framebuffer_pgm(&vec![0; 160 * 144]))
            .expect("framebuffer fixture should be writable");

        let case = RomTestCase::new(
            "framebuffer-until-match-check-at-pass",
            &rom_path,
            Timeout::TCycles(8),
            PassCondition::FramebufferFixtureUntilMatch {
                fixture_path,
                check_interval_tcycles: 1,
                check_at_tcycles: Some(1),
            },
        );

        let report = RomRunner::new()
            .run_case(&case)
            .expect("framebuffer until-match ROM should run");
        assert_eq!(report.outcome, RomCaseOutcome::Passed);
        assert_eq!(report.executed_t_cycles, 1);
        assert!(report.artifacts.framebuffer_pgm.is_some());

        fs::remove_dir_all(workspace).expect("workspace should be removable");
    }

    #[test]
    fn run_case_reports_framebuffer_until_match_unreached_check_at() {
        let workspace = unique_temp_dir("framebuffer-until-match-check-at-missed");
        fs::create_dir_all(&workspace).expect("workspace should be creatable");
        let rom_path = workspace.join("idle.gb");
        let fixture_path = workspace.join("white.pgm");
        fs::write(&rom_path, build_test_rom(&[0x40, 0x18, 0xFE]))
            .expect("test ROM should be writable");
        fs::write(&fixture_path, encode_framebuffer_pgm(&vec![0; 160 * 144]))
            .expect("framebuffer fixture should be writable");

        let case = RomTestCase::new(
            "framebuffer-until-match-check-at-missed",
            &rom_path,
            Timeout::TCycles(32),
            PassCondition::FramebufferFixtureUntilMatch {
                fixture_path,
                check_interval_tcycles: 1,
                check_at_tcycles: Some(16),
            },
        )
        .with_stop_condition(ExecutionStopCondition::CurrentOpcodeEquals { opcode: 0x40 });

        let report = RomRunner::new()
            .run_case(&case)
            .expect("framebuffer until-match ROM should run");
        let RomCaseOutcome::Failed(RomCaseFailure::FramebufferCheckAtNotReached {
            check_at_tcycles,
            executed_t_cycles,
        }) = report.outcome
        else {
            panic!("unreached check_at should fail, got {:?}", report.outcome);
        };
        assert_eq!(check_at_tcycles, 16);
        assert_eq!(executed_t_cycles, report.executed_t_cycles);
        assert!(
            report.executed_t_cycles < 16,
            "stop condition should end before the exact framebuffer probe, got {} T-cycles",
            report.executed_t_cycles
        );
        assert!(report.artifacts.framebuffer_pgm.is_some());

        fs::remove_dir_all(workspace).expect("workspace should be removable");
    }

    #[test]
    fn run_case_reports_framebuffer_until_match_fallback_mismatches() {
        let workspace = unique_temp_dir("framebuffer-until-match-mismatch");
        let artifact_root = workspace.join("artifacts");
        fs::create_dir_all(&workspace).expect("workspace should be creatable");
        let rom_path = workspace.join("idle.gb");
        let fixture_path = workspace.join("mismatching.pgm");
        fs::write(&rom_path, build_test_rom(&[0xC3, 0x00, 0x01]))
            .expect("test ROM should be writable");
        let mut mismatching_framebuffer = vec![0; 160 * 144];
        mismatching_framebuffer[0] = 1;
        fs::write(
            &fixture_path,
            encode_framebuffer_pgm(&mismatching_framebuffer),
        )
        .expect("framebuffer fixture should be writable");

        let case = RomTestCase::new(
            "framebuffer-until-match-mismatch",
            &rom_path,
            Timeout::TCycles(8),
            PassCondition::FramebufferFixtureUntilMatch {
                fixture_path: fixture_path.clone(),
                check_interval_tcycles: 1,
                check_at_tcycles: Some(1),
            },
        );

        let report = RomRunner::new()
            .with_failure_artifact_root(&artifact_root)
            .run_case(&case)
            .expect("framebuffer until-match ROM should run");
        assert_eq!(
            report.outcome,
            RomCaseOutcome::Failed(RomCaseFailure::FramebufferFixtureMismatch { fixture_path })
        );
        assert!(
            artifact_root
                .join("framebuffer-until-match-mismatch")
                .join("framebuffer.png")
                .is_file()
        );

        fs::remove_dir_all(workspace).expect("workspace should be removable");
    }

    #[test]
    fn run_case_polling_framebuffer_until_match_waits_for_vblank() {
        let workspace = unique_temp_dir("framebuffer-until-match-vblank-pass");
        fs::create_dir_all(&workspace).expect("workspace should be creatable");
        let rom_path = workspace.join("idle.gb");
        let fixture_path = workspace.join("white.pgm");
        fs::write(&rom_path, build_test_rom(&[0xAF, 0xE0, 0x47, 0x18, 0xFE]))
            .expect("test ROM should be writable");
        fs::write(&fixture_path, encode_framebuffer_pgm(&vec![0; 160 * 144]))
            .expect("framebuffer fixture should be writable");

        let case = RomTestCase::new(
            "framebuffer-until-match-vblank-pass",
            &rom_path,
            Timeout::TCycles(80_000),
            PassCondition::FramebufferFixtureUntilMatch {
                fixture_path,
                check_interval_tcycles: 1,
                check_at_tcycles: None,
            },
        );

        let report = RomRunner::new()
            .run_case(&case)
            .expect("framebuffer until-match ROM should run");
        assert_eq!(report.outcome, RomCaseOutcome::Passed);
        assert!(
            report.executed_t_cycles > 1,
            "periodic framebuffer matching should wait past the first T-cycle"
        );
        assert!(
            report.executed_t_cycles < 80_000,
            "matching during VBlank should stop before timeout"
        );

        fs::remove_dir_all(workspace).expect("workspace should be removable");
    }

    #[test]
    fn current_opcode_stop_condition_breaks_on_single_cycle_sentinels() {
        let workspace = unique_temp_dir("opcode-stop");
        fs::create_dir_all(&workspace).expect("workspace should be creatable");
        let rom_path = workspace.join("opcode-stop.gb");
        fs::write(&rom_path, build_test_rom(&[0x40, 0x18, 0xFE]))
            .expect("test ROM should be writable");

        let case = RomTestCase::new(
            "opcode-stop",
            &rom_path,
            Timeout::TCycles(64),
            PassCondition::Informational(InformationalCaptureKind::Snapshot),
        )
        .with_stop_condition(ExecutionStopCondition::CurrentOpcodeEquals { opcode: 0x40 });

        let report = RomRunner::new()
            .run_case(&case)
            .expect("opcode-stop ROM should run");
        assert_eq!(report.outcome, RomCaseOutcome::Informational);
        assert!(
            report.executed_t_cycles < 64,
            "the opcode sentinel should stop before timeout, got {} T-cycles",
            report.executed_t_cycles
        );

        fs::remove_dir_all(workspace).expect("workspace should be removable");
    }

    #[test]
    fn runner_path_resolution_and_boot_rom_loading_cover_explicit_roots() {
        let workspace = unique_temp_dir("path-resolution");
        fs::create_dir_all(&workspace).expect("workspace dir should be creatable");
        let runner = RomRunner::new().with_workspace_root(&workspace);

        let absolute = PathBuf::from("/tmp/gb-cycle-absolute-test.gb");
        assert_eq!(
            runner
                .resolve_case_path(&absolute)
                .expect("absolute path should resolve"),
            absolute
        );

        let _guard = crate::test_support::lock_env();
        let previous_test_root = env::var_os(TEST_ROM_ROOT_ENV_VAR);
        let previous_boot_rom_root = env::var_os(BOOT_ROM_ROOT_ENV_VAR);
        remove_env_var(TEST_ROM_ROOT_ENV_VAR);
        remove_env_var(BOOT_ROM_ROOT_ENV_VAR);

        let missing_default = runner
            .resolve_case_path(Path::new("test/gb-emulator-shootout/acid/dmg-acid2.gb"))
            .expect_err("missing repo-managed root should fail");
        assert!(matches!(
            missing_default,
            RomExecutionError::MissingExternalRomRoot {
                key,
                relative_path,
            } if key == TEST_ROM_ROOT_ENV_VAR
                && relative_path == Path::new("gb-emulator-shootout/acid/dmg-acid2.gb")
        ));

        let default_report_root = workspace
            .join(TEST_ROM_STORE_DIR)
            .join(GB_EMULATOR_SHOOTOUT_REPORT_ID);
        fs::create_dir_all(&default_report_root)
            .expect("default report ROM store should be creatable");
        assert_eq!(
            runner
                .resolve_case_path(Path::new("test/gb-emulator-shootout/acid/dmg-acid2.gb",))
                .expect("report-managed root should resolve below the report store"),
            default_report_root.join("acid/dmg-acid2.gb")
        );
        fs::remove_dir_all(workspace.join(TEST_ROM_STORE_DIR))
            .expect("default test ROM store should be removable");

        let global_env_test_root = workspace.join("global-env-test-root");
        set_env_var(TEST_ROM_ROOT_ENV_VAR, &global_env_test_root);
        assert_eq!(
            runner
                .resolve_case_path(Path::new("test/gb-emulator-shootout/acid/dmg-acid2.gb",))
                .expect("report-managed env root should resolve below the report id"),
            global_env_test_root
                .join(GB_EMULATOR_SHOOTOUT_REPORT_ID)
                .join("acid/dmg-acid2.gb")
        );
        remove_env_var(TEST_ROM_ROOT_ENV_VAR);

        assert_eq!(
            runner
                .resolve_case_path(Path::new("retrio/case.gb"))
                .expect("manifest-relative paths should resolve below the workspace root"),
            workspace.join("retrio/case.gb")
        );

        match previous_test_root {
            Some(value) => set_env_var(TEST_ROM_ROOT_ENV_VAR, value),
            None => remove_env_var(TEST_ROM_ROOT_ENV_VAR),
        }

        let cgb_real_boot_case = RomTestCase::new(
            "cgb-real-boot",
            "unused.gb",
            Timeout::TCycles(1),
            PassCondition::Informational(InformationalCaptureKind::Snapshot),
        )
        .with_console_model(ConsoleModel::GameBoyColor)
        .with_startup_mode(StartupMode::RealBoot);
        let cgb_error = runner
            .load_boot_rom_assets(&cgb_real_boot_case)
            .expect_err("strict CGB real-boot should require a configured CGB boot ROM root");
        assert!(matches!(
            cgb_error,
            RomExecutionError::BootRomVerification {
                issue: BootRomVerificationIssue::MissingRoot { .. },
            }
        ));

        let missing_boot_root = workspace.join("missing-bootrom");
        let dmg_real_boot_case = RomTestCase::new(
            "dmg-real-boot",
            "unused.gb",
            Timeout::TCycles(1),
            PassCondition::Informational(InformationalCaptureKind::Snapshot),
        )
        .with_startup_mode(StartupMode::RealBoot);
        assert!(
            RomRunner::new()
                .with_workspace_root(&workspace)
                .with_boot_rom_root(&missing_boot_root)
                .with_boot_rom_verification_mode(BootRomVerificationMode::Off)
                .load_boot_rom_assets(&dmg_real_boot_case)
                .expect("missing boot root should fall back to no assets")
                .is_empty()
        );

        let sgb_boot_root = workspace.join("sgb-bootrom");
        fs::create_dir_all(&sgb_boot_root).expect("SGB boot ROM root should be creatable");
        fs::write(sgb_boot_root.join("dmg_boot.bin"), vec![0xD0; 0x100])
            .expect("DMG boot ROM should be writable");
        fs::write(sgb_boot_root.join("sgb_boot.bin"), vec![0x51; 0x100])
            .expect("SGB boot ROM should be writable");
        fs::write(sgb_boot_root.join("sgb2_boot.bin"), vec![0x52; 0x100])
            .expect("SGB2 boot ROM should be writable");
        let sgb_real_boot_case = RomTestCase::new(
            "sgb-real-boot",
            "unused.gb",
            Timeout::TCycles(1),
            PassCondition::Informational(InformationalCaptureKind::Snapshot),
        )
        .with_host_platform(HostPlatform::Sgb)
        .with_startup_mode(StartupMode::RealBoot);
        let sgb_assets = RomRunner::new()
            .with_workspace_root(&workspace)
            .with_boot_rom_root(&sgb_boot_root)
            .with_boot_rom_verification_mode(BootRomVerificationMode::Off)
            .load_boot_rom_assets(&sgb_real_boot_case)
            .expect("SGB real-boot should load SGB boot ROM assets");
        assert_eq!(
            sgb_assets.read_asset_byte(BootRomAssetKind::Sgb, 0),
            Some(0x51)
        );
        assert_eq!(
            sgb_assets.read_byte(ConsoleModel::GameBoy.default_revision(), 0),
            Some(0xD0)
        );

        match previous_boot_rom_root {
            Some(value) => set_env_var(BOOT_ROM_ROOT_ENV_VAR, value),
            None => remove_env_var(BOOT_ROM_ROOT_ENV_VAR),
        }

        fs::remove_dir_all(workspace).expect("workspace dir should be removable");
    }

    #[test]
    fn helper_functions_cover_frame_budget_memory_capture_blargg_and_mooneye_nonmatches() {
        assert!(budget_exhausted(Timeout::Frames(3), 0, 3));
        assert!(!budget_exhausted(Timeout::Frames(3), 0, 2));
        assert!(budget_exhausted(
            Timeout::Frames(3),
            3 * DMG_FAMILY_FRAME_T_CYCLES,
            0,
        ));

        let case = RomTestCase::new(
            "helper-machine",
            "/dev/null",
            Timeout::TCycles(8),
            PassCondition::Informational(InformationalCaptureKind::Snapshot),
        );
        let mut machine = RunnerMachine::new(&case, BootRomAssets::none());
        machine
            .load_cartridge(build_test_rom(&[0x00]))
            .expect("helper ROM should load");

        machine.write_bus(0xFFFC, 0x00);
        machine.write_bus(0xFFFD, 0xDE);
        machine.write_bus(0xFFFE, 0xB0);
        machine.write_bus(0xFFFF, 0x61);
        let memory = capture_memory_text_output(
            &MemoryTextOutputSpec::new(0xFFFC, 0x80, 0x00, 0xFFFD, [0xDE, 0xB0, 0x61], 0xFFFF, 4),
            &mut machine,
        );
        assert_eq!(memory.status, 0x00);
        assert_eq!(memory.signature, [0xDE, 0xB0, 0x61]);
        assert_eq!(memory.text, "a");

        assert!(!blargg_console_text_complete(
            &PassCondition::SerialContains("unused".to_string()),
            &mut machine
        ));
        machine.write_bus(0xFF42, 8);
        machine.write_bus(0x9800 + 32, b'Q');
        machine.write_bus(0x9800 + 33, 0x01);
        assert_eq!(capture_blargg_console_text(&mut machine), "Q");
        assert!(!blargg_console_text_complete(
            &PassCondition::BlarggConsoleTextContains("Missing".to_string()),
            &mut machine
        ));

        let mut mooneye_machine = mooneye_result_machine(MOONEYE_PASS_SIGNATURE);
        assert_eq!(detect_mooneye_result(&mut mooneye_machine), None);
        assert_eq!(
            mooneye_result_completion_candidate(
                &PassCondition::SerialContains("unused".to_string()),
                &mut mooneye_machine
            ),
            None
        );
    }
}
