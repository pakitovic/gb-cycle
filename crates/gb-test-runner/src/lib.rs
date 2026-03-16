use std::collections::BTreeSet;
use std::path::PathBuf;

use gb_core::{ConsoleModel, ExecutionMode, StartupMode};

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
    Framebuffer,
    Trace,
    Snapshot,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PassCondition {
    SerialExact(String),
    SerialContains(String),
    FramebufferFixture(PathBuf),
    TraceFixture(PathBuf),
}

impl PassCondition {
    pub fn required_capture(&self) -> CaptureKind {
        match self {
            Self::SerialExact(_) | Self::SerialContains(_) => CaptureKind::Serial,
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
    InvalidTimeout,
    MissingRequiredCapture(CaptureKind),
    MissingRequiredFailureArtifact(CaptureKind),
    ArtifactNotCaptured(CaptureKind),
    MissingFailureArtifacts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomTestCase {
    pub id: String,
    pub rom_path: PathBuf,
    pub console_model: ConsoleModel,
    pub startup_mode: StartupMode,
    pub execution_mode: ExecutionMode,
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
            console_model: ConsoleModel::Dmg,
            startup_mode: StartupMode::SkipBoot,
            execution_mode: ExecutionMode::Strict,
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
    PathBuf::from("crates/gb-core/tests/fixtures/traces").join(name)
}

pub fn phase_2_cpu_timing_suite() -> RomSuite {
    RomSuite::new("phase-2-cpu-timing", TestSubsystem::Cpu)
        .with_case(RomTestCase::new(
            "phase2-fetch-immediate-order",
            phase_2_rom_path("phase2_fetch_immediate_order.gb"),
            Timeout::TCycles(256),
            PassCondition::TraceFixture(phase_2_trace_path("phase2_fetch_immediate_order.trace")),
        ))
        .with_case(RomTestCase::new(
            "phase2-control-flow-stack-cb",
            phase_2_rom_path("phase2_control_flow_stack_cb.gb"),
            Timeout::TCycles(512),
            PassCondition::TraceFixture(phase_2_trace_path("phase2_control_flow_stack_cb.trace")),
        ))
}

pub fn phase_2_interrupt_timing_suite() -> RomSuite {
    RomSuite::new("phase-2-interrupt-timing", TestSubsystem::Interrupts)
        .with_case(RomTestCase::new(
            "phase2-ei-delay-priority",
            phase_2_rom_path("phase2_ei_delay_priority.gb"),
            Timeout::TCycles(256),
            PassCondition::TraceFixture(phase_2_trace_path("phase2_ei_delay_priority.trace")),
        ))
        .with_case(RomTestCase::new(
            "phase2-halt-stop-and-halt-bug",
            phase_2_rom_path("phase2_halt_stop_and_halt_bug.gb"),
            Timeout::TCycles(512),
            PassCondition::TraceFixture(phase_2_trace_path("phase2_halt_stop_and_halt_bug.trace")),
        ))
        .with_case(RomTestCase::new(
            "phase2-timer-if-visibility-and-service",
            phase_2_rom_path("phase2_timer_if_visibility_and_service.gb"),
            Timeout::TCycles(512),
            PassCondition::TraceFixture(phase_2_trace_path(
                "phase2_timer_if_visibility_and_service.trace",
            )),
        ))
}
