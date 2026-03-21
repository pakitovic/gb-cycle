pub mod external_roms;
mod fetch_external_roms;
mod run_rom_suite_cli;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::{fs, io};

use external_roms::ExternalRomSourceManifestError;
use gb_core::{
    CartridgeDiagnostic, CartridgeLoadError, ConsoleModel, CpuDiagnosticTrap, CpuExecutionState,
    ExecutionMode, JoypadButton, Machine, MachineConfig, StartupMode, TraceBuffer,
    TraceSummaryBuffer,
};

pub use external_roms::{
    EXTERNAL_ROM_SOURCE_MANIFEST_PATH, EXTERNAL_ROM_STORE_DIR, ExternalRomRequiredFile,
    ExternalRomSource, ExternalRomSourceManifest, LOCAL_COMMERCIAL_ROM_STORE_DIR,
    discover_external_rom_root_for_key, external_rom_source_manifest_path, external_rom_store_root,
    load_external_rom_source_manifest, local_commercial_rom_store_root,
};
pub use fetch_external_roms::{fetch_external_roms_help_text, run_fetch_external_roms_command};
pub use run_rom_suite_cli::{rom_suite_cli_help_text, run_rom_suite_command};

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CaptureKind {
    Serial,
    MemoryTextOutput,
    BlarggConsoleText,
    Framebuffer,
    Trace,
    Snapshot,
}

pub const RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR: &str = "GB_CYCLE_RETRIO_GB_TEST_ROMS_ROOT";

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
    MemoryTextOutputContains {
        spec: MemoryTextOutputSpec,
        expected_substring: String,
    },
    BlarggConsoleTextContains(String),
    FramebufferFixture(PathBuf),
    TraceFixture(PathBuf),
}

impl PassCondition {
    pub fn required_capture(&self) -> CaptureKind {
        match self {
            Self::SerialExact(_) | Self::SerialContains(_) => CaptureKind::Serial,
            Self::MemoryTextOutputContains { .. } => CaptureKind::MemoryTextOutput,
            Self::BlarggConsoleTextContains(_) => CaptureKind::BlarggConsoleText,
            Self::FramebufferFixture(_) => CaptureKind::Framebuffer,
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
    pub subsystem: TestSubsystem,
    pub cases: Vec<RomTestCase>,
}

impl RomSuite {
    pub fn new(name: impl Into<String>, subsystem: TestSubsystem) -> Self {
        Self {
            name: name.into(),
            subsystem,
            cases: Vec::new(),
        }
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

const PHASE_SENTINEL_ADDRESS: u16 = 0xC010;
const PHASE_SENTINEL_VALUE: u8 = 0xA5;

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
                380,
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

pub fn retrio_blargg_cpu_smoke_suite() -> RomSuite {
    RomSuite::new("retrio-blargg-cpu-smoke", TestSubsystem::Cpu)
        .with_case(retrio_blargg_cpu_smoke_case(
            "retrio-cpu-instrs-01-special",
            "cpu_instrs/individual/01-special.gb",
        ))
        .with_case(retrio_blargg_cpu_smoke_case(
            "retrio-cpu-instrs-02-interrupts",
            "cpu_instrs/individual/02-interrupts.gb",
        ))
        .with_case(retrio_blargg_cpu_smoke_case(
            "retrio-cpu-instrs-03-op-sp-hl",
            "cpu_instrs/individual/03-op sp,hl.gb",
        ))
        .with_case(retrio_blargg_cpu_smoke_case(
            "retrio-cpu-instrs-04-op-r-imm",
            "cpu_instrs/individual/04-op r,imm.gb",
        ))
        .with_case(retrio_blargg_cpu_smoke_case(
            "retrio-cpu-instrs-05-op-rp",
            "cpu_instrs/individual/05-op rp.gb",
        ))
        .with_case(retrio_blargg_cpu_smoke_case(
            "retrio-cpu-instrs-06-ld-r-r",
            "cpu_instrs/individual/06-ld r,r.gb",
        ))
        .with_case(retrio_blargg_cpu_smoke_case(
            "retrio-cpu-instrs-07-jr-jp-call-ret-rst",
            "cpu_instrs/individual/07-jr,jp,call,ret,rst.gb",
        ))
        .with_case(retrio_blargg_cpu_smoke_case(
            "retrio-cpu-instrs-08-misc-instrs",
            "cpu_instrs/individual/08-misc instrs.gb",
        ))
        .with_case(retrio_blargg_cpu_smoke_case(
            "retrio-cpu-instrs-09-op-r-r",
            "cpu_instrs/individual/09-op r,r.gb",
        ))
        .with_case(retrio_blargg_cpu_smoke_case(
            "retrio-cpu-instrs-10-bit-ops",
            "cpu_instrs/individual/10-bit ops.gb",
        ))
        .with_case(retrio_blargg_cpu_smoke_case(
            "retrio-cpu-instrs-11-op-a-hl",
            "cpu_instrs/individual/11-op a,(hl).gb",
        ))
}

pub fn retrio_blargg_cpu_instrs_full_suite() -> RomSuite {
    RomSuite::new("retrio-blargg-cpu-instrs-full", TestSubsystem::Cpu).with_case(
        retrio_blargg_serial_case(
            "retrio-cpu-instrs-full",
            "cpu_instrs/cpu_instrs.gb",
            Timeout::Frames(18_000),
            "Passed all tests",
        ),
    )
}

pub fn retrio_blargg_instr_timing_suite() -> RomSuite {
    RomSuite::new("retrio-blargg-instr-timing", TestSubsystem::Cpu).with_case(
        retrio_blargg_external_case(
            "retrio-instr-timing",
            "instr_timing/instr_timing.gb",
            Timeout::Frames(3_600),
        ),
    )
}

pub fn retrio_blargg_halt_bug_suite() -> RomSuite {
    RomSuite::new("retrio-blargg-halt-bug", TestSubsystem::Interrupts).with_case(
        retrio_blargg_console_case("retrio-halt-bug", "halt_bug.gb", Timeout::Frames(3_600)),
    )
}

pub fn retrio_blargg_mem_timing_suite() -> RomSuite {
    RomSuite::new("retrio-blargg-mem-timing", TestSubsystem::Bus)
        .with_case(retrio_blargg_external_case(
            "retrio-mem-timing",
            "mem_timing/mem_timing.gb",
            Timeout::Frames(7_200),
        ))
        .with_case(retrio_blargg_memory_output_case(
            "retrio-mem-timing-2",
            "mem_timing-2/mem_timing.gb",
            Timeout::Frames(7_200),
        ))
}

pub fn retrio_blargg_mem_timing_individual_suite() -> RomSuite {
    RomSuite::new("retrio-blargg-mem-timing-individual", TestSubsystem::Bus)
        .with_case(retrio_blargg_external_case(
            "retrio-mem-timing-01-read",
            "mem_timing/individual/01-read_timing.gb",
            Timeout::Frames(3_600),
        ))
        .with_case(retrio_blargg_external_case(
            "retrio-mem-timing-02-write",
            "mem_timing/individual/02-write_timing.gb",
            Timeout::Frames(3_600),
        ))
        .with_case(retrio_blargg_external_case(
            "retrio-mem-timing-03-modify",
            "mem_timing/individual/03-modify_timing.gb",
            Timeout::Frames(3_600),
        ))
        .with_case(retrio_blargg_memory_output_case(
            "retrio-mem-timing-2-01-read",
            "mem_timing-2/rom_singles/01-read_timing.gb",
            Timeout::Frames(3_600),
        ))
        .with_case(retrio_blargg_memory_output_case(
            "retrio-mem-timing-2-02-write",
            "mem_timing-2/rom_singles/02-write_timing.gb",
            Timeout::Frames(3_600),
        ))
        .with_case(retrio_blargg_memory_output_case(
            "retrio-mem-timing-2-03-modify",
            "mem_timing-2/rom_singles/03-modify_timing.gb",
            Timeout::Frames(3_600),
        ))
}

pub fn retrio_blargg_oam_bug_suite() -> RomSuite {
    RomSuite::new("retrio-blargg-oam-bug", TestSubsystem::Ppu)
        .with_case(retrio_blargg_memory_output_case(
            "retrio-oam-bug",
            "oam_bug/oam_bug.gb",
            Timeout::Frames(7_200),
        ))
        .with_case(retrio_blargg_memory_output_case(
            "retrio-oam-bug-1-lcd-sync",
            "oam_bug/rom_singles/1-lcd_sync.gb",
            Timeout::Frames(3_600),
        ))
        .with_case(retrio_blargg_memory_output_case(
            "retrio-oam-bug-2-causes",
            "oam_bug/rom_singles/2-causes.gb",
            Timeout::Frames(3_600),
        ))
        .with_case(retrio_blargg_memory_output_case(
            "retrio-oam-bug-3-non-causes",
            "oam_bug/rom_singles/3-non_causes.gb",
            Timeout::Frames(3_600),
        ))
        .with_case(retrio_blargg_memory_output_case(
            "retrio-oam-bug-4-scanline-timing",
            "oam_bug/rom_singles/4-scanline_timing.gb",
            Timeout::Frames(3_600),
        ))
        .with_case(retrio_blargg_memory_output_case(
            "retrio-oam-bug-5-timing-bug",
            "oam_bug/rom_singles/5-timing_bug.gb",
            Timeout::Frames(3_600),
        ))
        .with_case(retrio_blargg_memory_output_case(
            "retrio-oam-bug-6-timing-no-bug",
            "oam_bug/rom_singles/6-timing_no_bug.gb",
            Timeout::Frames(3_600),
        ))
        .with_case(retrio_blargg_memory_output_case(
            "retrio-oam-bug-7-timing-effect",
            "oam_bug/rom_singles/7-timing_effect.gb",
            Timeout::Frames(3_600),
        ))
        .with_case(retrio_blargg_memory_output_case(
            "retrio-oam-bug-8-instr-effect",
            "oam_bug/rom_singles/8-instr_effect.gb",
            Timeout::Frames(3_600),
        ))
}

pub fn built_in_rom_suites() -> Vec<RomSuite> {
    vec![
        phase_2_cpu_timing_suite(),
        phase_2_interrupt_timing_suite(),
        phase_4_ppu_oam_corruption_suite(),
        retrio_blargg_cpu_smoke_suite(),
        retrio_blargg_cpu_instrs_full_suite(),
        retrio_blargg_instr_timing_suite(),
        retrio_blargg_halt_bug_suite(),
        retrio_blargg_mem_timing_suite(),
        retrio_blargg_mem_timing_individual_suite(),
        retrio_blargg_oam_bug_suite(),
    ]
}

pub fn built_in_rom_suite_by_name(name: &str) -> Option<RomSuite> {
    built_in_rom_suites()
        .into_iter()
        .find(|suite| suite.name == name)
}

fn retrio_blargg_external_case(id: &str, rom_path: &str, timeout: Timeout) -> RomTestCase {
    retrio_blargg_serial_case(id, rom_path, timeout, "Passed")
}

fn retrio_blargg_serial_case(
    id: &str,
    rom_path: &str,
    timeout: Timeout,
    expected_substring: &str,
) -> RomTestCase {
    RomTestCase::new(
        id,
        PathBuf::from(rom_path),
        timeout,
        PassCondition::SerialContains(expected_substring.to_string()),
    )
    .with_external_rom_root_key(RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR)
    .with_capture_plan(
        CapturePlan::new()
            .with_capture(CaptureKind::Serial)
            .with_capture(CaptureKind::Snapshot),
    )
    .with_failure_artifacts(
        FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::Serial)
            .with_artifact(CaptureKind::Snapshot),
    )
}

fn retrio_blargg_memory_output_case(id: &str, rom_path: &str, timeout: Timeout) -> RomTestCase {
    RomTestCase::new(
        id,
        PathBuf::from(rom_path),
        timeout,
        PassCondition::MemoryTextOutputContains {
            spec: retrio_blargg_memory_text_output_spec(),
            expected_substring: "Passed".to_string(),
        },
    )
    .with_external_rom_root_key(RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR)
    .with_capture_plan(
        CapturePlan::new()
            .with_capture(CaptureKind::MemoryTextOutput)
            .with_capture(CaptureKind::Snapshot),
    )
    .with_failure_artifacts(
        FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::MemoryTextOutput)
            .with_artifact(CaptureKind::Snapshot),
    )
}

fn retrio_blargg_console_case(id: &str, rom_path: &str, timeout: Timeout) -> RomTestCase {
    RomTestCase::new(
        id,
        PathBuf::from(rom_path),
        timeout,
        PassCondition::BlarggConsoleTextContains("Passed".to_string()),
    )
    .with_external_rom_root_key(RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR)
    .with_capture_plan(
        CapturePlan::new()
            .with_capture(CaptureKind::BlarggConsoleText)
            .with_capture(CaptureKind::Snapshot),
    )
    .with_failure_artifacts(
        FailureArtifactPolicy::new()
            .with_artifact(CaptureKind::BlarggConsoleText)
            .with_artifact(CaptureKind::Snapshot),
    )
}

fn retrio_blargg_cpu_smoke_case(id: &str, rom_path: &str) -> RomTestCase {
    retrio_blargg_external_case(id, rom_path, Timeout::Frames(1_800))
}

const fn retrio_blargg_memory_text_output_spec() -> MemoryTextOutputSpec {
    MemoryTextOutputSpec::new(
        0xA000,
        0x80,
        0x00,
        0xA001,
        [0xDE, 0xB0, 0x61],
        0xA004,
        4_096,
    )
}

#[derive(Debug)]
pub enum RomExecutionError {
    InvalidCase(RomCaseValidationError),
    InvalidSuite(RomSuiteValidationError),
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
    TraceFixtureMismatch {
        fixture_path: PathBuf,
    },
    FramebufferFixtureMismatch {
        fixture_path: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RomCaseOutcome {
    Passed,
    Failed(RomCaseFailure),
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
    pub memory_text_output: Option<CapturedMemoryTextOutput>,
    pub blargg_console_text: Option<String>,
    pub framebuffer_pgm: Option<Vec<u8>>,
    pub trace: Option<String>,
    pub snapshot_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomCaseReport {
    pub case_id: String,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomSuiteReport {
    pub suite_name: String,
    pub subsystem: TestSubsystem,
    pub cases: Vec<RomCaseReport>,
}

impl RomSuiteReport {
    pub fn all_passed(&self) -> bool {
        self.cases.iter().all(RomCaseReport::passed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomRunner {
    workspace_root: PathBuf,
    failure_artifact_root: Option<PathBuf>,
    external_rom_roots: BTreeMap<String, PathBuf>,
}

enum RunnerMachine {
    Buffered(Machine<TraceBuffer>),
    Summary(Machine<TraceSummaryBuffer>),
}

impl RunnerMachine {
    fn new(case: &RomTestCase) -> Self {
        let config = MachineConfig::new(case.console_model)
            .with_startup_mode(case.startup_mode)
            .with_execution_mode(case.execution_mode);
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
            subsystem: suite.subsystem,
            cases: case_reports,
        })
    }

    pub fn run_case(&self, case: &RomTestCase) -> Result<RomCaseReport, RomExecutionError> {
        case.validate().map_err(RomExecutionError::InvalidCase)?;

        let rom_path =
            self.resolve_case_path(&case.rom_path, case.external_rom_root_key.as_deref())?;
        let rom_bytes = fs::read(&rom_path).map_err(|source| RomExecutionError::ReadFile {
            path: rom_path.clone(),
            operation: "read ROM",
            source,
        })?;

        let mut machine = RunnerMachine::new(case);
        let diagnostics = machine.load_cartridge(rom_bytes).map_err(|source| {
            RomExecutionError::CartridgeLoad {
                path: rom_path.clone(),
                source,
            }
        })?;

        let mut executed_t_cycles = 0_u64;
        let mut completed_frames = 0_u32;
        let mut at_frame_origin = machine.at_frame_origin();
        let mut serial_bytes = Vec::new();
        let mut applied_stimuli = vec![false; case.external_stimuli.stimuli().len()];
        let mut serial_contains_matched = false;
        let mut diagnostic_trap = None;
        let mut last_memory_text_output_completion_candidate = None;

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
        let outcome = self.evaluate_case(
            case,
            &artifacts,
            serial_contains_matched,
            diagnostic_trap,
            executed_t_cycles,
            completed_frames,
        )?;
        let retained_failure_artifacts = if matches!(outcome, RomCaseOutcome::Failed(_)) {
            self.persist_failure_artifacts(case, &artifacts)?
        } else {
            Vec::new()
        };

        Ok(RomCaseReport {
            case_id: case.id.clone(),
            outcome,
            executed_t_cycles,
            completed_frames,
            diagnostics,
            artifacts,
            retained_failure_artifacts,
        })
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
        artifacts: &CapturedArtifacts,
        serial_contains_matched: bool,
        diagnostic_trap: Option<CpuDiagnosticTrap>,
        executed_t_cycles: u64,
        completed_frames: u32,
    ) -> Result<RomCaseOutcome, RomExecutionError> {
        if let Some(trap) = diagnostic_trap {
            return Ok(RomCaseOutcome::Failed(RomCaseFailure::CpuDiagnosticTrap {
                trap,
            }));
        }

        Ok(match &case.pass_condition {
            PassCondition::SerialContains(expected_substring) => {
                if serial_contains_matched {
                    RomCaseOutcome::Passed
                } else if budget_exhausted(case.timeout, executed_t_cycles, completed_frames) {
                    RomCaseOutcome::Failed(RomCaseFailure::SerialMissingSubstring {
                        expected_substring: expected_substring.clone(),
                        actual: artifacts.serial.clone().unwrap_or_default(),
                    })
                } else {
                    RomCaseOutcome::Failed(RomCaseFailure::TimeoutExceeded)
                }
            }
            PassCondition::SerialExact(expected) => {
                if !budget_exhausted(case.timeout, executed_t_cycles, completed_frames) {
                    RomCaseOutcome::Failed(RomCaseFailure::TimeoutExceeded)
                } else if artifacts.serial.as_deref() == Some(expected.as_str()) {
                    RomCaseOutcome::Passed
                } else {
                    RomCaseOutcome::Failed(RomCaseFailure::SerialExactMismatch {
                        expected: expected.clone(),
                        actual: artifacts.serial.clone().unwrap_or_default(),
                    })
                }
            }
            PassCondition::MemoryTextOutputContains {
                spec,
                expected_substring,
            } => {
                let captured = artifacts.memory_text_output.clone().unwrap_or_default();
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
                let actual = artifacts.blargg_console_text.clone().unwrap_or_default();
                if actual.contains(expected_substring) {
                    RomCaseOutcome::Passed
                } else {
                    RomCaseOutcome::Failed(RomCaseFailure::BlarggConsoleTextMissingSubstring {
                        expected_substring: expected_substring.clone(),
                        actual,
                    })
                }
            }
            PassCondition::TraceFixture(fixture_path) => {
                let resolved_fixture = self.resolve_path(fixture_path);
                let expected = fs::read_to_string(&resolved_fixture).map_err(|source| {
                    RomExecutionError::ReadFile {
                        path: resolved_fixture.clone(),
                        operation: "read trace fixture",
                        source,
                    }
                })?;

                if artifacts.trace.as_deref() == Some(expected.as_str()) {
                    RomCaseOutcome::Passed
                } else {
                    RomCaseOutcome::Failed(RomCaseFailure::TraceFixtureMismatch {
                        fixture_path: resolved_fixture,
                    })
                }
            }
            PassCondition::FramebufferFixture(fixture_path) => {
                let resolved_fixture = self.resolve_path(fixture_path);
                let expected =
                    fs::read(&resolved_fixture).map_err(|source| RomExecutionError::ReadFile {
                        path: resolved_fixture.clone(),
                        operation: "read framebuffer fixture",
                        source,
                    })?;

                if artifacts.framebuffer_pgm.as_deref() == Some(expected.as_slice()) {
                    RomCaseOutcome::Passed
                } else {
                    RomCaseOutcome::Failed(RomCaseFailure::FramebufferFixtureMismatch {
                        fixture_path: resolved_fixture,
                    })
                }
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
                    let path = case_dir.join("framebuffer.pgm");
                    fs::write(&path, framebuffer_pgm).map_err(|source| {
                        RomExecutionError::ReadFile {
                            path: path.clone(),
                            operation: "write framebuffer artifact",
                            source,
                        }
                    })?;
                    written_paths.push(path);
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

fn render_memory_text_output(captured: &CapturedMemoryTextOutput) -> String {
    format!(
        "status=0x{status:02X}\nsignature={sig0:02X} {sig1:02X} {sig2:02X}\ntext={text:?}\n",
        status = captured.status,
        sig0 = captured.signature[0],
        sig1 = captured.signature[1],
        sig2 = captured.signature[2],
        text = captured.text,
    )
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

fn encode_framebuffer_pgm(framebuffer: &[u8]) -> Vec<u8> {
    const WIDTH: usize = 160;
    const HEIGHT: usize = 144;

    let mut encoded = format!("P5\n{} {}\n255\n", WIDTH, HEIGHT).into_bytes();
    encoded.reserve(framebuffer.len());

    for &pixel in framebuffer {
        encoded.push(match pixel {
            0 => 255,
            1 => 170,
            2 => 85,
            _ => 0,
        });
    }

    encoded
}

#[cfg(test)]
mod tests {
    use super::{
        CapturedMemoryTextOutput, built_in_rom_suite_by_name, memory_text_output_completion_reached,
    };

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
        let suite = built_in_rom_suite_by_name("retrio-blargg-oam-bug")
            .expect("known suite should be discoverable");

        assert_eq!(suite.name, "retrio-blargg-oam-bug");
        assert_eq!(suite.cases.len(), 9);
        assert!(
            suite
                .cases
                .iter()
                .any(|case| case.id == "retrio-oam-bug-1-lcd-sync")
        );
    }
}
