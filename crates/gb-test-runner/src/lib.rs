mod boot_rom_verification;
mod curated_test_roms;
mod differential;
pub mod external_roms;
mod fetch_external_roms;
mod framebuffer_oracle;
mod run_differential_cli;
mod run_rom_suite_cli;
mod run_sameboy_case_bundle_cli;
mod run_sameboy_tester_cli;
mod sameboy_case_bundle;
mod sameboy_tester;
mod workspace_paths;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::{fs, io};

use external_roms::ExternalRomSourceManifestError;
use framebuffer_oracle::{
    convert_pgm_to_png, decode_fixture_framebuffer_path, decode_local_pgm_framebuffer,
    encode_framebuffer_pgm,
};
use gb_core::{
    BootRomAssetError, BootRomAssets, CartridgeDiagnostic, CartridgeLoadError, CompatibilityPolicy,
    ConsoleModel, CpuDiagnosticTrap, CpuExecutionState, CpuSnapshot, ExecutionMode, JoypadButton,
    Machine, MachineConfig, StartupMode, TraceBuffer, TraceSummaryBuffer,
};

pub use boot_rom_verification::{
    BootRomVerificationIssue, BootRomVerificationMode, enforce_boot_rom_verification,
    expected_boot_rom_sha256, verify_boot_rom_file,
};
pub use curated_test_roms::{
    TEST_ROM_REPORT_FILE_NAME, TEST_ROM_ROOT_ENV_VAR, TEST_ROM_STORE_DIR, acid_dmg_curated_suite,
    blargg_dmg_curated_suite, cpp_dmg_curated_suite, curated_test_rom_families,
    curated_test_rom_family_suites, daid_dmg_curated_suite, discover_test_rom_store_root,
    hacktix_dmg_curated_suite, materialize_curated_test_rom_families,
    materialize_curated_test_rom_store, test_rom_store_root, update_curated_test_report,
};
pub use differential::{
    DifferentialCaseMismatch, DifferentialCaseOutcome, DifferentialCaseReport,
    DifferentialExecutionError, DifferentialOracle, DifferentialOracleLayout, DifferentialRunner,
    DifferentialSuiteReport,
};
pub use external_roms::{
    EXTERNAL_ROM_SOURCE_MANIFEST_PATH, EXTERNAL_ROM_STORE_DIR, ExternalRomRequiredFile,
    ExternalRomSource, ExternalRomSourceManifest, LOCAL_COMMERCIAL_ROM_STORE_DIR,
    discover_external_rom_root_for_key, external_rom_source_manifest_path, external_rom_store_root,
    load_external_rom_source_manifest, local_commercial_rom_store_root,
};
pub use fetch_external_roms::{fetch_external_roms_help_text, run_fetch_external_roms_command};
pub use run_differential_cli::{differential_cli_help_text, run_differential_command};
pub use run_rom_suite_cli::{rom_suite_cli_help_text, run_rom_suite_command};
pub use run_sameboy_case_bundle_cli::{
    run_sameboy_case_bundle_command, sameboy_case_bundle_cli_help_text,
};
pub use run_sameboy_tester_cli::{run_sameboy_tester_command, sameboy_tester_cli_help_text};
pub use sameboy_case_bundle::{
    SameBoyCaseBundleExecutionError, SameBoyCaseBundleRunner, SameBoyCaseBundleSuiteReport,
};
pub use sameboy_tester::{
    SameBoyTesterExecutionError, SameBoyTesterImageFormat, SameBoyTesterRunner,
    SameBoyTesterSuiteReport,
};
pub use workspace_paths::{
    BOOT_ROM_ROOT_ENV_VAR, BOOT_ROM_STORE_DIR, ORACLE_STORE_DIR, boot_rom_image_path,
    boot_rom_kind_for_console_model, boot_rom_store_root, discover_boot_rom_store_root,
    oracle_layout_root, oracle_store_root, sameboy_case_bundle_oracle_root,
    sameboy_tester_oracle_root,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TestSubsystem {
    Cpu,
    Interrupts,
    Bus,
    Cartridge,
    Timer,
    Ppu,
    Dma,
    Apu,
    Boot,
    Joypad,
    Serial,
    Scheduler,
    CrossSubsystem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EarlyHardeningStatus {
    InternalGateOnly,
    RepoGatePresent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarlyHardeningChecklistEntry {
    pub subsystem: TestSubsystem,
    pub status: EarlyHardeningStatus,
    pub current_evidence: &'static [&'static str],
    pub active_oracles: &'static [&'static str],
    pub remaining_gaps: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CaptureKind {
    Serial,
    SerialHex,
    MemoryTextOutput,
    BlarggConsoleText,
    Framebuffer,
    Trace,
    Snapshot,
}

pub const GBEMU_SHOOTOUT_ROOT_ENV_VAR: &str = "GB_CYCLE_GBEMU_SHOOTOUT_ROOT";

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
    MemoryTextOutputContains {
        spec: MemoryTextOutputSpec,
        expected_substring: String,
    },
    BlarggConsoleTextContains(String),
    MooneyeResult,
    Informational(CaptureKind),
    FramebufferFixture(PathBuf),
    FramebufferFixtureSet(Vec<PathBuf>),
    TraceFixture(PathBuf),
}

impl PassCondition {
    pub fn required_capture(&self) -> CaptureKind {
        match self {
            Self::SerialExact(_) | Self::SerialContains(_) => CaptureKind::Serial,
            Self::SerialHexExact(_) => CaptureKind::SerialHex,
            Self::MemoryTextOutputContains { .. } => CaptureKind::MemoryTextOutput,
            Self::BlarggConsoleTextContains(_) => CaptureKind::BlarggConsoleText,
            Self::MooneyeResult => CaptureKind::Snapshot,
            Self::Informational(capture) => *capture,
            Self::FramebufferFixture(_) | Self::FramebufferFixtureSet(_) => {
                CaptureKind::Framebuffer
            }
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
    EmptyExternalRomRootKey,
    InvalidTimeout,
    MissingRequiredCapture(CaptureKind),
    MissingRequiredFailureArtifact(CaptureKind),
    ArtifactNotCaptured(CaptureKind),
    MissingFailureArtifacts,
    DuplicateExternalStimulus(ExternalStimulus),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomTestCase {
    pub id: String,
    pub rom_path: PathBuf,
    pub external_rom_root_key: Option<String>,
    pub console_model: ConsoleModel,
    pub startup_mode: StartupMode,
    pub execution_mode: ExecutionMode,
    pub startup_cartridge_rtc_seconds: Option<u64>,
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
            external_rom_root_key: None,
            console_model: ConsoleModel::Dmg,
            startup_mode: StartupMode::SkipBoot,
            execution_mode: ExecutionMode::Strict,
            startup_cartridge_rtc_seconds: None,
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

    pub fn with_external_rom_root_key(mut self, external_rom_root_key: impl Into<String>) -> Self {
        self.external_rom_root_key = Some(external_rom_root_key.into());
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

        if self
            .external_rom_root_key
            .as_deref()
            .is_some_and(|key| key.trim().is_empty())
        {
            return Err(RomCaseValidationError::EmptyExternalRomRootKey);
        }

        if !self.timeout.is_valid() {
            return Err(RomCaseValidationError::InvalidTimeout);
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
    pub subsystem: TestSubsystem,
    pub cases: Vec<RomTestCase>,
}

impl RomSuite {
    pub fn new(name: impl Into<String>, subsystem: TestSubsystem) -> Self {
        Self {
            name: name.into(),
            family: None,
            subsystem,
            cases: Vec::new(),
        }
    }

    pub fn with_family(mut self, family: impl Into<String>) -> Self {
        self.family = Some(family.into());
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
    RomSuite::new("phase-2-cpu-timing", TestSubsystem::Cpu)
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
    const PHASE_2_HALT_STOP_WAKE_T_CYCLE: u64 = 372;

    RomSuite::new("phase-2-interrupt-timing", TestSubsystem::Interrupts)
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
            .with_stop_condition(ExecutionStopCondition::MemoryEquals {
                address: PHASE_SENTINEL_ADDRESS,
                value: PHASE_SENTINEL_VALUE,
            }),
        )
}

pub fn phase_4_ppu_oam_corruption_suite() -> RomSuite {
    RomSuite::new("phase-4-ppu-oam-corruption", TestSubsystem::Ppu)
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
            .with_console_model(ConsoleModel::Dmg0)
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
            .with_console_model(ConsoleModel::Mgb)
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
            .with_console_model(ConsoleModel::Cgb)
            .with_stop_condition(ExecutionStopCondition::MemoryEquals {
                address: PHASE_SENTINEL_ADDRESS,
                value: PHASE_SENTINEL_VALUE,
            }),
        )
}

pub fn phase_6_cartridge_oracle_suite() -> RomSuite {
    RomSuite::new("phase-6-cartridge-oracle", TestSubsystem::Cartridge)
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

pub fn mealybug_tearoom_dmg_curated_suite() -> RomSuite {
    curated_test_roms::mealybug_tearoom_dmg_curated_suite()
}

pub fn mooneye_acceptance_dmg_curated_suite() -> RomSuite {
    curated_test_roms::mooneye_acceptance_dmg_curated_suite()
}

pub fn built_in_rom_suites() -> Vec<RomSuite> {
    let mut suites = vec![
        phase_2_cpu_timing_suite(),
        phase_2_interrupt_timing_suite(),
        phase_4_ppu_oam_corruption_suite(),
        phase_6_cartridge_oracle_suite(),
    ];
    suites.extend(curated_test_rom_family_suites());
    suites
}

pub fn built_in_rom_suite_by_name(name: &str) -> Option<RomSuite> {
    built_in_rom_suites()
        .into_iter()
        .find(|suite| suite.name == name)
}

pub fn early_phase_9_partial_checklist() -> Vec<EarlyHardeningChecklistEntry> {
    vec![
        EarlyHardeningChecklistEntry {
            subsystem: TestSubsystem::Cpu,
            status: EarlyHardeningStatus::RepoGatePresent,
            current_evidence: &["phase-2-cpu-timing", "blargg-dmg-curated"],
            active_oracles: &["trace-fixture", "serial-contains"],
            remaining_gaps: &[
                "differential-oracle",
                "replay-determinism",
                "broader-real-boot-arbitration",
            ],
        },
        EarlyHardeningChecklistEntry {
            subsystem: TestSubsystem::Interrupts,
            status: EarlyHardeningStatus::RepoGatePresent,
            current_evidence: &["phase-2-interrupt-timing", "blargg-dmg-curated"],
            active_oracles: &["trace-fixture", "blargg-console-text", "serial-contains"],
            remaining_gaps: &["differential-oracle", "longer-run-determinism"],
        },
        EarlyHardeningChecklistEntry {
            subsystem: TestSubsystem::Timer,
            status: EarlyHardeningStatus::InternalGateOnly,
            current_evidence: &["phase-2-interrupt-timing", "gb-core-unit-coverage"],
            active_oracles: &["trace-fixture"],
            remaining_gaps: &["promoted-external-suite", "differential-oracle"],
        },
        EarlyHardeningChecklistEntry {
            subsystem: TestSubsystem::Bus,
            status: EarlyHardeningStatus::RepoGatePresent,
            current_evidence: &[
                "phase-3-and-phase-4-integration-coverage",
                "blargg-dmg-curated",
            ],
            active_oracles: &["serial-contains", "memory-text-output"],
            remaining_gaps: &[
                "cartridge-family-oracle-comparison",
                "save-load-era-determinism",
            ],
        },
        EarlyHardeningChecklistEntry {
            subsystem: TestSubsystem::Dma,
            status: EarlyHardeningStatus::InternalGateOnly,
            current_evidence: &["phase-3-unit-and-integration-coverage"],
            active_oracles: &["trace-fixture"],
            remaining_gaps: &["promoted-external-suite", "differential-oracle"],
        },
        EarlyHardeningChecklistEntry {
            subsystem: TestSubsystem::Ppu,
            status: EarlyHardeningStatus::RepoGatePresent,
            current_evidence: &[
                "phase-4-ppu-oam-corruption",
                "blargg-dmg-curated",
                "acid-dmg-curated",
            ],
            active_oracles: &["trace-fixture", "memory-text-output", "framebuffer-fixture"],
            remaining_gaps: &[
                "green-repo-gated-mealybug-tearoom",
                "broader-rendering-differential-oracle",
            ],
        },
        EarlyHardeningChecklistEntry {
            subsystem: TestSubsystem::Cartridge,
            status: EarlyHardeningStatus::InternalGateOnly,
            current_evidence: &[
                "gb-core-unit-and-integration-coverage",
                "hardware-style-persistence-tests",
                "phase-6-cartridge-oracle",
            ],
            active_oracles: &[
                "unit-contracts",
                "trace-fixture",
                "differential-case-bundle",
            ],
            remaining_gaps: &[
                "imported-sameboy-cartridge-oracle-artifacts",
                "phase-8-save-load-determinism",
            ],
        },
        EarlyHardeningChecklistEntry {
            subsystem: TestSubsystem::Joypad,
            status: EarlyHardeningStatus::InternalGateOnly,
            current_evidence: &["phase-5-synthetic-coverage", "gb-core-subsystem-tests"],
            active_oracles: &["trace-fixture"],
            remaining_gaps: &["promoted-external-suite", "differential-oracle"],
        },
        EarlyHardeningChecklistEntry {
            subsystem: TestSubsystem::Serial,
            status: EarlyHardeningStatus::InternalGateOnly,
            current_evidence: &["phase-5-synthetic-coverage", "gb-core-subsystem-tests"],
            active_oracles: &["trace-fixture"],
            remaining_gaps: &["promoted-external-suite", "differential-oracle"],
        },
    ]
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
    ExternalRomSourceManifest {
        source: ExternalRomSourceManifestError,
    },
    MissingExternalRomRoot {
        key: String,
        relative_path: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RomCaseFailure {
    TimeoutExceeded,
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
    pub memory_text_output: Option<CapturedMemoryTextOutput>,
    pub blargg_console_text: Option<String>,
    pub framebuffer_pgm: Option<Vec<u8>>,
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
    pub subsystem: TestSubsystem,
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
    external_rom_roots: BTreeMap<String, PathBuf>,
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

impl RunnerMachine {
    fn new(case: &RomTestCase, boot_rom_assets: BootRomAssets) -> Self {
        let config = MachineConfig::new(case.console_model)
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

    fn at_frame_origin(&self) -> bool {
        match self {
            Self::Buffered(machine) => machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0,
            Self::Summary(machine) => machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0,
        }
    }

    fn framebuffer(&self) -> &[u8] {
        match self {
            Self::Buffered(machine) => machine.ppu().framebuffer(),
            Self::Summary(machine) => machine.ppu().framebuffer(),
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
            external_rom_roots: BTreeMap::new(),
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

    pub fn with_external_rom_root(
        mut self,
        key: impl Into<String>,
        root: impl Into<PathBuf>,
    ) -> Self {
        self.external_rom_roots.insert(key.into(), root.into());
        self
    }

    pub fn run_suite(&self, suite: &RomSuite) -> Result<RomSuiteReport, RomExecutionError> {
        suite.validate().map_err(RomExecutionError::InvalidSuite)?;

        let mut case_reports = Vec::with_capacity(suite.cases.len());
        for case in &suite.cases {
            case_reports.push(self.run_case(case)?);
        }

        Ok(RomSuiteReport {
            suite_name: suite.name.clone(),
            family: suite.family.clone(),
            subsystem: suite.subsystem,
            cases: case_reports,
        })
    }

    pub fn run_case(&self, case: &RomTestCase) -> Result<RomCaseReport, RomExecutionError> {
        case.validate().map_err(RomExecutionError::InvalidCase)?;

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
        self.apply_startup_cartridge_state(case, &mut machine);
        self.apply_startup_memory_writes(case, &mut machine);

        let mut executed_t_cycles = 0_u64;
        let mut completed_frames = 0_u32;
        let mut at_frame_origin = machine.at_frame_origin();
        let mut serial_bytes = Vec::new();
        let mut applied_stimuli = vec![false; case.external_stimuli.stimuli().len()];
        let mut serial_contains_matched = false;
        let mut diagnostic_trap = None;
        let mut last_memory_text_output_completion_candidate = None;
        let mut mooneye_result = None;

        while !budget_exhausted(case.timeout, executed_t_cycles, completed_frames) {
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
            executed_t_cycles += 1;

            serial_bytes.extend(machine.take_serial_output_bytes());

            let now_at_frame_origin = machine.at_frame_origin();
            if now_at_frame_origin && !at_frame_origin {
                completed_frames += 1;
            }
            at_frame_origin = now_at_frame_origin;

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
            executed_t_cycles,
            completed_frames,
        };
        let outcome = self.evaluate_case(case, &evaluation)?;
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
        self.resolve_case_path(&case.rom_path, case.external_rom_root_key.as_deref())
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    fn load_boot_rom_assets(&self, case: &RomTestCase) -> Result<BootRomAssets, RomExecutionError> {
        if case.startup_mode != StartupMode::RealBoot {
            return Ok(BootRomAssets::none());
        }

        let Some(kind) = boot_rom_kind_for_console_model(case.console_model) else {
            return Ok(BootRomAssets::none());
        };

        let root = self
            .boot_rom_root
            .clone()
            .or_else(|| discover_boot_rom_store_root(&self.workspace_root))
            .unwrap_or_else(|| boot_rom_store_root(&self.workspace_root));
        let image_path = boot_rom_image_path(&root, kind);
        enforce_boot_rom_verification(self.boot_rom_verification_mode, &image_path, kind)
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
        } else {
            self.workspace_root.join(path)
        }
    }

    fn resolve_case_path(
        &self,
        path: &Path,
        external_rom_root_key: Option<&str>,
    ) -> Result<PathBuf, RomExecutionError> {
        if path.is_absolute() {
            return Ok(path.to_path_buf());
        }

        if let Some(key) = external_rom_root_key {
            if let Some(root) = self.external_rom_roots.get(key) {
                return Ok(root.join(path));
            }

            if key == TEST_ROM_ROOT_ENV_VAR {
                if let Some(root) = discover_test_rom_store_root(&self.workspace_root) {
                    return Ok(root.join(path));
                }

                return Err(RomExecutionError::MissingExternalRomRoot {
                    key: key.to_string(),
                    relative_path: path.to_path_buf(),
                });
            }

            if let Some(root) = discover_external_rom_root_for_key(&self.workspace_root, key)
                .map_err(|source| RomExecutionError::ExternalRomSourceManifest { source })?
            {
                return Ok(root.join(path));
            }

            return Err(RomExecutionError::MissingExternalRomRoot {
                key: key.to_string(),
                relative_path: path.to_path_buf(),
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
            }

            applied_stimuli[index] = true;
        }
    }

    fn apply_startup_cartridge_state(&self, case: &RomTestCase, machine: &mut RunnerMachine) {
        if let Some(seconds) = case.startup_cartridge_rtc_seconds {
            machine.advance_cartridge_rtc_seconds(seconds);
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
        Timeout::Frames(limit) => completed_frames >= limit,
    }
}

fn memory_text_output_spec(pass_condition: &PassCondition) -> Option<&MemoryTextOutputSpec> {
    match pass_condition {
        PassCondition::MemoryTextOutputContains { spec, .. } => Some(spec),
        _ => None,
    }
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

pub(crate) fn artifact_file_name(capture: CaptureKind) -> &'static str {
    match capture {
        CaptureKind::Serial => "serial.txt",
        CaptureKind::SerialHex => "serial_hex.txt",
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
        BootRomAssets, CaptureKind, CapturedMemoryTextOutput, MOONEYE_FAIL_SIGNATURE,
        MOONEYE_PASS_SIGNATURE, MooneyeTestResult, PassCondition, RomTestCase, RunnerMachine,
        TEST_ROM_ROOT_ENV_VAR, TestSubsystem, Timeout, built_in_rom_suite_by_name,
        detect_mooneye_result, early_phase_9_partial_checklist, hacktix_dmg_curated_suite,
        memory_text_output_completion_reached, mooneye_result_for_signature,
    };
    use gb_core::{
        ConsoleModel, CpuExecutionState, CpuRegisters, CpuSnapshot, CpuStartupState, CpuStatus,
        ExecutionMode,
    };
    use std::path::PathBuf;

    const TEST_ROM_MINIMUM_LEN: usize = 32 * 1024;

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
    fn built_in_rom_suite_lookup_returns_known_suite() {
        let suite = built_in_rom_suite_by_name("blargg-dmg-curated")
            .expect("known suite should be discoverable");

        assert_eq!(suite.name, "blargg-dmg-curated");
        assert_eq!(suite.family.as_deref(), Some("blargg"));
        assert_eq!(suite.cases.len(), 26);
        assert!(
            suite
                .cases
                .iter()
                .any(|case| case.id == "blargg-cpu-instrs-01-special")
        );
        assert!(
            suite
                .cases
                .iter()
                .any(|case| case.id == "blargg-oam-bug-8-instr-effect")
        );
        assert!(
            suite
                .cases
                .iter()
                .any(|case| case.id == "blargg-instr-timing")
        );
    }

    #[test]
    fn built_in_rom_suite_lookup_returns_acid_dmg_curated_suite_with_framebuffer_oracle() {
        let suite =
            built_in_rom_suite_by_name("acid-dmg-curated").expect("known suite should exist");

        assert_eq!(suite.subsystem, TestSubsystem::Ppu);
        assert_eq!(suite.family.as_deref(), Some("acid"));
        assert_eq!(suite.cases.len(), 2);
        let which_case = suite
            .cases
            .iter()
            .find(|case| case.id == "which-dmg")
            .expect("acid suite should include which.gb as an informational case");
        assert_eq!(
            which_case.external_rom_root_key.as_deref(),
            Some(TEST_ROM_ROOT_ENV_VAR)
        );
        assert_eq!(which_case.rom_path, PathBuf::from("acid/which.gb"));
        assert!(which_case.capture_plan.contains(CaptureKind::Framebuffer));
        assert!(which_case.capture_plan.contains(CaptureKind::Snapshot));
        assert!(matches!(
            which_case.pass_condition,
            PassCondition::Informational(CaptureKind::Framebuffer)
        ));

        let acid2_case = suite
            .cases
            .iter()
            .find(|case| case.id == "dmg-acid2")
            .expect("acid suite should include dmg-acid2");
        assert_eq!(
            acid2_case.external_rom_root_key.as_deref(),
            Some(TEST_ROM_ROOT_ENV_VAR)
        );
        assert_eq!(acid2_case.rom_path, PathBuf::from("acid/dmg-acid2.gb"));
        assert!(acid2_case.capture_plan.contains(CaptureKind::Framebuffer));
        assert!(acid2_case.capture_plan.contains(CaptureKind::Snapshot));
        assert!(matches!(
            acid2_case.pass_condition,
            PassCondition::FramebufferFixture(_)
        ));
    }

    #[test]
    fn built_in_rom_suite_lookup_returns_curated_mealybug_suite_with_framebuffer_oracles() {
        let suite = built_in_rom_suite_by_name("mealybug-tearoom-dmg-curated")
            .expect("known suite should exist");

        assert_eq!(suite.subsystem, TestSubsystem::Ppu);
        assert_eq!(suite.family.as_deref(), Some("mealybug-tearoom-tests"));
        assert_eq!(suite.cases.len(), 24);
        assert!(suite.cases.iter().all(|case| {
            case.external_rom_root_key.as_deref() == Some(TEST_ROM_ROOT_ENV_VAR)
                && case.capture_plan.contains(CaptureKind::Framebuffer)
                && case.capture_plan.contains(CaptureKind::Snapshot)
                && matches!(case.pass_condition, PassCondition::FramebufferFixture(_))
        }));
        assert!(
            suite
                .cases
                .iter()
                .any(|case| case.id == "mealybug-m3-window-timing")
        );
        assert!(
            suite
                .cases
                .iter()
                .any(|case| case.id == "mealybug-m3-wx-4-change-sprites")
        );
        assert!(
            suite
                .cases
                .iter()
                .any(|case| case.id == "mealybug-m3-lcdc-bg-en-change")
        );
    }

    #[test]
    fn built_in_rom_suite_lookup_returns_curated_cpp_suite_with_framebuffer_oracles() {
        let suite =
            built_in_rom_suite_by_name("cpp-dmg-curated").expect("known suite should exist");

        assert_eq!(suite.subsystem, TestSubsystem::CrossSubsystem);
        assert_eq!(suite.family.as_deref(), Some("cpp"));
        assert_eq!(suite.cases.len(), 3);
        assert!(suite.cases.iter().all(|case| {
            case.external_rom_root_key.as_deref() == Some(TEST_ROM_ROOT_ENV_VAR)
                && case.capture_plan.contains(CaptureKind::Framebuffer)
                && case.capture_plan.contains(CaptureKind::Snapshot)
                && matches!(case.pass_condition, PassCondition::FramebufferFixture(_))
        }));
        assert!(suite.cases.iter().any(|case| case.id == "cpp-latch-rtc"));
    }

    #[test]
    fn hacktix_bully_startup_profile_seeds_the_expected_boot_logo_vram_bytes() {
        let suite = hacktix_dmg_curated_suite();
        let case = suite
            .cases
            .into_iter()
            .find(|case| case.id == "hacktix-bully")
            .expect("hacktix bully case should exist");
        let mut machine = RunnerMachine::new(&case, BootRomAssets::none());
        machine
            .load_cartridge(build_test_rom(&[0x00]))
            .expect("fixture rom should load");

        for write in &case.startup_memory_writes {
            machine.write_bus(write.address, write.value);
        }

        assert_eq!(machine.read_bus(0x8000), 0x00);
        assert_eq!(machine.read_bus(0x800F), 0x00);
        assert_eq!(machine.read_bus(0x8010), 0xF0);
        assert_eq!(machine.read_bus(0x8011), 0x00);
        assert_eq!(machine.read_bus(0x819E), 0x3C);
        assert_eq!(machine.read_bus(0x819F), 0x00);
        assert_eq!(machine.read_bus(0x9903), 0x00);
        assert_eq!(machine.read_bus(0x9904), 0x01);
    }

    #[test]
    fn mooneye_result_for_signature_requires_matching_registers() {
        let mut snapshot = CpuSnapshot {
            console_model: ConsoleModel::Dmg,
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
                opcode: 0x40,
                step: 0,
                t_cycle: 0,
            },
            current_opcode: Some(0x40),
            ime: false,
            delayed_ime_enable: false,
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
    fn built_in_rom_suite_lookup_returns_curated_mooneye_suite_with_case_specific_oracles() {
        let suite = built_in_rom_suite_by_name("mooneye-acceptance-dmg-curated")
            .expect("known suite should exist");

        assert_eq!(suite.subsystem, TestSubsystem::CrossSubsystem);
        assert_eq!(suite.family.as_deref(), Some("mooneye"));
        assert_eq!(suite.cases.len(), 95);
        assert!(
            suite
                .cases
                .iter()
                .all(|case| case.external_rom_root_key.as_deref() == Some(TEST_ROM_ROOT_ENV_VAR))
        );
        assert!(
            suite
                .cases
                .iter()
                .any(|case| case.id == "mooneye-ppu-intr-2-mode0-timing-sprites")
        );
        assert!(
            suite
                .cases
                .iter()
                .any(|case| case.id == "mooneye-serial-boot-sclk-align-dmgabcmgb")
        );
        assert!(
            suite
                .cases
                .iter()
                .any(|case| case.id == "mooneye-emulator-only-mbc1-bits-bank1")
        );
        let mbc1_multicart = suite
            .cases
            .iter()
            .find(|case| case.id == "mooneye-emulator-only-mbc1-multicart-rom-8mb")
            .expect("known suite should include the experimental multicart case");
        assert_eq!(mbc1_multicart.execution_mode, ExecutionMode::Experimental);
        let sprite_priority = suite
            .cases
            .iter()
            .find(|case| case.id == "mooneye-manual-only-sprite-priority")
            .expect("known suite should include the framebuffer sprite priority case");
        assert_eq!(
            sprite_priority.capture_plan,
            super::CapturePlan::new()
                .with_capture(CaptureKind::Framebuffer)
                .with_capture(CaptureKind::Snapshot)
        );
        assert_eq!(
            sprite_priority.failure_artifacts,
            super::FailureArtifactPolicy::new()
                .with_artifact(CaptureKind::Framebuffer)
                .with_artifact(CaptureKind::Snapshot)
        );
        assert!(matches!(
            sprite_priority.pass_condition,
            PassCondition::FramebufferFixture(_)
        ));
        assert!(suite.cases.iter().all(|case| {
            if case.id == "mooneye-manual-only-sprite-priority" {
                true
            } else {
                case.capture_plan
                    == super::CapturePlan::new()
                        .with_capture(CaptureKind::Snapshot)
                        .with_capture(CaptureKind::Serial)
                    && case.failure_artifacts
                        == super::FailureArtifactPolicy::new()
                            .with_artifact(CaptureKind::Snapshot)
                            .with_artifact(CaptureKind::Serial)
                    && matches!(case.pass_condition, PassCondition::MooneyeResult)
            }
        }));
    }

    #[test]
    fn early_phase_9_partial_checklist_tracks_cpu_and_ppu_repo_gates() {
        let checklist = early_phase_9_partial_checklist();

        let cpu = checklist
            .iter()
            .find(|entry| entry.subsystem == TestSubsystem::Cpu)
            .expect("cpu entry should exist");
        assert_eq!(cpu.status, super::EarlyHardeningStatus::RepoGatePresent);
        assert!(cpu.current_evidence.contains(&"blargg-dmg-curated"));
        assert!(cpu.active_oracles.contains(&"serial-contains"));

        let ppu = checklist
            .iter()
            .find(|entry| entry.subsystem == TestSubsystem::Ppu)
            .expect("ppu entry should exist");
        assert_eq!(ppu.status, super::EarlyHardeningStatus::RepoGatePresent);
        assert!(ppu.current_evidence.contains(&"blargg-dmg-curated"));
        assert!(ppu.current_evidence.contains(&"acid-dmg-curated"));
        assert!(!ppu.remaining_gaps.contains(&"repo-gated-dmg-acid2"));
    }
}
