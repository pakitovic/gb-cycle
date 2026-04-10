use std::fmt;

use crate::apu::ApuSnapshot;
use crate::boot::BootSnapshot;
use crate::bus::BusSnapshot;
use crate::cartridge::CartridgeSnapshot;
use crate::cpu::CpuSnapshot;
use crate::dma::DmaSnapshot;
use crate::interrupts::InterruptControllerSnapshot;
use crate::joypad::JoypadSnapshot;
use crate::model::MachineConfig;
use crate::ppu::PpuSnapshot;
use crate::scheduler::SchedulerSnapshot;
use crate::serial::SerialSnapshot;
use crate::timer::TimerSnapshot;

pub const TRACE_FORMAT_VERSION: u16 = 2;

const SUPPORTED_TRACE_SUBSYSTEMS: [TraceSubsystem; 14] = [
    TraceSubsystem::Core,
    TraceSubsystem::Scheduler,
    TraceSubsystem::Cpu,
    TraceSubsystem::Bus,
    TraceSubsystem::Apu,
    TraceSubsystem::Ppu,
    TraceSubsystem::Dma,
    TraceSubsystem::Serial,
    TraceSubsystem::Timer,
    TraceSubsystem::Joypad,
    TraceSubsystem::Cartridge,
    TraceSubsystem::Boot,
    TraceSubsystem::Interrupts,
    TraceSubsystem::Debugger,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceSubsystem {
    Core,
    Scheduler,
    Cpu,
    Bus,
    Apu,
    Ppu,
    Dma,
    Serial,
    Timer,
    Joypad,
    Cartridge,
    Boot,
    Interrupts,
    Debugger,
}

impl fmt::Display for TraceSubsystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Core => "core",
            Self::Scheduler => "scheduler",
            Self::Cpu => "cpu",
            Self::Bus => "bus",
            Self::Apu => "apu",
            Self::Ppu => "ppu",
            Self::Dma => "dma",
            Self::Serial => "serial",
            Self::Timer => "timer",
            Self::Joypad => "joypad",
            Self::Cartridge => "cartridge",
            Self::Boot => "boot",
            Self::Interrupts => "interrupts",
            Self::Debugger => "debugger",
        };

        f.write_str(name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl fmt::Display for TraceLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        };

        f.write_str(name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEvent {
    sequence: u64,
    subsystem: TraceSubsystem,
    level: TraceLevel,
    message: String,
}

impl TraceEvent {
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    pub fn subsystem(&self) -> TraceSubsystem {
        self.subsystem
    }

    pub fn level(&self) -> TraceLevel {
        self.level
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for TraceEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "seq={} subsystem={} level={} message={:?}",
            self.sequence, self.subsystem, self.level, self.message
        )
    }
}

pub trait TraceSink {
    fn push(&mut self, event: TraceEvent);

    fn records_events(&self) -> bool {
        true
    }

    fn reset(&mut self) {}
}

pub trait TraceSnapshotProvider {
    fn snapshot(&self, next_sequence: u64) -> DebugSnapshot;
}

pub trait TraceTextRenderer {
    fn render_text(&self) -> String;
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TraceBuffer {
    events: Vec<TraceEvent>,
}

impl TraceBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> &[TraceEvent] {
        &self.events
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn render_text(&self) -> String {
        let mut rendered = String::new();

        for event in &self.events {
            rendered.push_str(&event.to_string());
            rendered.push('\n');
        }

        rendered
    }
}

impl TraceSink for TraceBuffer {
    fn push(&mut self, event: TraceEvent) {
        self.events.push(event);
    }

    fn reset(&mut self) {
        self.clear();
    }
}

impl TraceSnapshotProvider for TraceBuffer {
    fn snapshot(&self, next_sequence: u64) -> DebugSnapshot {
        DebugSnapshot {
            trace_format_version: TRACE_FORMAT_VERSION,
            next_sequence,
            buffered_event_count: self.events.len(),
            last_event: self.events.last().cloned(),
        }
    }
}

impl TraceTextRenderer for TraceBuffer {
    fn render_text(&self) -> String {
        self.render_text()
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TraceSummaryBuffer;

impl TraceSummaryBuffer {
    pub fn new() -> Self {
        Self
    }
}

impl TraceSink for TraceSummaryBuffer {
    fn push(&mut self, _event: TraceEvent) {}

    fn records_events(&self) -> bool {
        false
    }
}

impl TraceSnapshotProvider for TraceSummaryBuffer {
    fn snapshot(&self, next_sequence: u64) -> DebugSnapshot {
        DebugSnapshot {
            trace_format_version: TRACE_FORMAT_VERSION,
            next_sequence,
            buffered_event_count: 0,
            last_event: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BreakpointId(u64);

impl BreakpointId {
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BreakpointCondition {
    ProgramCounter(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BreakpointProbe {
    program_counter: u16,
}

impl BreakpointProbe {
    pub const fn at_program_counter(program_counter: u16) -> Self {
        Self { program_counter }
    }

    pub const fn program_counter(self) -> u16 {
        self.program_counter
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Breakpoint {
    pub id: BreakpointId,
    pub condition: BreakpointCondition,
    pub enabled: bool,
}

impl Breakpoint {
    pub fn matches(&self, probe: BreakpointProbe) -> bool {
        if !self.enabled {
            return false;
        }

        match self.condition {
            BreakpointCondition::ProgramCounter(address) => address == probe.program_counter(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WatchpointId(u64);

impl WatchpointId {
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CartridgeWatchTarget {
    RomBank,
    RamBank,
    MapperState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WatchpointTarget {
    MemoryAddress(u16),
    MmioRegister(u16),
    Cartridge(CartridgeWatchTarget),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WatchpointCondition {
    Read,
    Write,
    ReadWrite,
    Change,
}

impl WatchpointCondition {
    pub fn matches(self, observation: WatchpointObservation) -> bool {
        matches!(
            (self, observation),
            (Self::Read, WatchpointObservation::Read)
                | (Self::Write, WatchpointObservation::Write)
                | (Self::ReadWrite, WatchpointObservation::Read)
                | (Self::ReadWrite, WatchpointObservation::Write)
                | (Self::Change, WatchpointObservation::Change)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WatchpointObservation {
    Read,
    Write,
    Change,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WatchpointProbe {
    target: WatchpointTarget,
    observation: WatchpointObservation,
}

impl WatchpointProbe {
    pub const fn new(target: WatchpointTarget, observation: WatchpointObservation) -> Self {
        Self {
            target,
            observation,
        }
    }

    pub const fn target(self) -> WatchpointTarget {
        self.target
    }

    pub const fn observation(self) -> WatchpointObservation {
        self.observation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watchpoint {
    pub id: WatchpointId,
    pub target: WatchpointTarget,
    pub condition: WatchpointCondition,
    pub enabled: bool,
}

impl Watchpoint {
    pub fn matches(&self, probe: WatchpointProbe) -> bool {
        self.enabled && self.target == probe.target() && self.condition.matches(probe.observation())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugControlSnapshot {
    pub next_breakpoint_id: u64,
    pub next_watchpoint_id: u64,
    pub breakpoint_count: usize,
    pub enabled_breakpoint_count: usize,
    pub watchpoint_count: usize,
    pub enabled_watchpoint_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DebugControl {
    next_breakpoint_id: u64,
    next_watchpoint_id: u64,
    breakpoints: Vec<Breakpoint>,
    watchpoints: Vec<Watchpoint>,
}

impl DebugControl {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn breakpoints(&self) -> &[Breakpoint] {
        &self.breakpoints
    }

    pub fn watchpoints(&self) -> &[Watchpoint] {
        &self.watchpoints
    }

    pub fn add_breakpoint(&mut self, condition: BreakpointCondition) -> BreakpointId {
        let id = BreakpointId(self.next_breakpoint_id);
        self.next_breakpoint_id = self
            .next_breakpoint_id
            .checked_add(1)
            .expect("breakpoint identifier overflow");

        self.breakpoints.push(Breakpoint {
            id,
            condition,
            enabled: true,
        });

        id
    }

    pub fn add_watchpoint(
        &mut self,
        target: WatchpointTarget,
        condition: WatchpointCondition,
    ) -> WatchpointId {
        let id = WatchpointId(self.next_watchpoint_id);
        self.next_watchpoint_id = self
            .next_watchpoint_id
            .checked_add(1)
            .expect("watchpoint identifier overflow");

        self.watchpoints.push(Watchpoint {
            id,
            target,
            condition,
            enabled: true,
        });

        id
    }

    pub fn set_breakpoint_enabled(&mut self, id: BreakpointId, enabled: bool) -> bool {
        if let Some(breakpoint) = self
            .breakpoints
            .iter_mut()
            .find(|breakpoint| breakpoint.id == id)
        {
            breakpoint.enabled = enabled;
            return true;
        }

        false
    }

    pub fn set_watchpoint_enabled(&mut self, id: WatchpointId, enabled: bool) -> bool {
        if let Some(watchpoint) = self
            .watchpoints
            .iter_mut()
            .find(|watchpoint| watchpoint.id == id)
        {
            watchpoint.enabled = enabled;
            return true;
        }

        false
    }

    pub fn remove_breakpoint(&mut self, id: BreakpointId) -> Option<Breakpoint> {
        let index = self
            .breakpoints
            .iter()
            .position(|breakpoint| breakpoint.id == id)?;

        Some(self.breakpoints.remove(index))
    }

    pub fn remove_watchpoint(&mut self, id: WatchpointId) -> Option<Watchpoint> {
        let index = self
            .watchpoints
            .iter()
            .position(|watchpoint| watchpoint.id == id)?;

        Some(self.watchpoints.remove(index))
    }

    pub fn matching_breakpoints(&self, probe: BreakpointProbe) -> Vec<BreakpointId> {
        self.breakpoints
            .iter()
            .filter(|breakpoint| breakpoint.matches(probe))
            .map(|breakpoint| breakpoint.id)
            .collect()
    }

    pub fn matching_watchpoints(&self, probe: WatchpointProbe) -> Vec<WatchpointId> {
        self.watchpoints
            .iter()
            .filter(|watchpoint| watchpoint.matches(probe))
            .map(|watchpoint| watchpoint.id)
            .collect()
    }

    pub fn snapshot(&self) -> DebugControlSnapshot {
        DebugControlSnapshot {
            next_breakpoint_id: self.next_breakpoint_id,
            next_watchpoint_id: self.next_watchpoint_id,
            breakpoint_count: self.breakpoints.len(),
            enabled_breakpoint_count: self
                .breakpoints
                .iter()
                .filter(|breakpoint| breakpoint.enabled)
                .count(),
            watchpoint_count: self.watchpoints.len(),
            enabled_watchpoint_count: self
                .watchpoints
                .iter()
                .filter(|watchpoint| watchpoint.enabled)
                .count(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugSnapshot {
    pub trace_format_version: u16,
    pub next_sequence: u64,
    pub buffered_event_count: usize,
    pub last_event: Option<TraceEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineSnapshot {
    pub config: MachineConfig,
    pub scheduler: SchedulerSnapshot,
    pub trace: DebugSnapshot,
    pub debug_controls: DebugControlSnapshot,
    pub cpu: CpuSnapshot,
    pub bus: BusSnapshot,
    pub apu: ApuSnapshot,
    pub ppu: PpuSnapshot,
    pub dma: DmaSnapshot,
    pub timer: TimerSnapshot,
    pub serial: SerialSnapshot,
    pub boot: BootSnapshot,
    pub interrupts: InterruptControllerSnapshot,
    pub joypad: JoypadSnapshot,
    pub cartridge: CartridgeSnapshot,
}

impl MachineSnapshot {
    pub fn render_text(&self) -> String {
        let last_trace_line = self
            .trace
            .last_event
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "none".to_string());

        format!(
            concat!(
                "config.console_model={:?}\n",
                "config.startup_mode={:?}\n",
                "config.execution_mode={:?}\n",
                "scheduler.next_t_cycle={}\n",
                "trace.buffered_event_count={}\n",
                "trace.last_event={}\n",
                "debug.breakpoint_count={} debug.enabled_breakpoint_count={}\n",
                "debug.watchpoint_count={} debug.enabled_watchpoint_count={}\n",
                "cpu.console_model={:?} cpu.status={:?} cpu.startup_pc={:#06X} cpu.pc={:#06X} cpu.execution_state={:?} cpu.current_opcode={:?}\n",
                "bus.console_model={:?} bus.status={:?}\n",
                "apu.console_model={:?} apu.status={:?} apu.powered={} apu.div_apu={}\n",
                "ppu.console_model={:?} ppu.status={:?} ppu.lcd_state={:?} ppu.mode={:?} ppu.visible_output={:?} ppu.ly={} ppu.line_dot={} ppu.mode_dot={} ppu.mode0_start_dot={} ppu.visible_pixels_output={} ppu.scx_discard_remaining={}\n",
                "ppu.bg_fetcher_source={:?} ppu.bg_fetcher_stage={:?} ppu.bg_fetcher_stage_dot={} ppu.bg_fetcher_tile_map_address={:#06X} ppu.bg_fetcher_tile_data_address={:#06X} ppu.bg_push_pending={} ppu.bg_fill_pending={}\n",
                "ppu.bg_startup_source_state={:?} ppu.bg_startup_fetch_seam={:?} ppu.bg_startup_fifo_placeholders={} ppu.bg_push_entry_delay_remaining={} ppu.bg_fill_startup_dummy_pixels={} ppu.bg_fetcher_post_alignment_restart_delay_dots={} ppu.bg_transfer_phase={:?} ppu.bg_current_transfer_x={} ppu.bg_current_transfer_lane={:?} ppu.bg_current_transfer_source_window={:?} ppu.bg_current_transfer_backing={:?} ppu.bg_current_transfer_readiness={:?} ppu.bg_current_transfer_kind={:?}\n",
                "ppu.bg_fifo_pixels={:?}\n",
                "ppu.bg_fifo_cached_pixels={:?}\n",
                "ppu.window_wy_latch={} ppu.window_started_this_line={} ppu.window_line_counter={}\n",
                "dma.console_model={:?} dma.status={:?}\n",
                "timer.console_model={:?} timer.status={:?}\n",
                "serial.console_model={:?} serial.status={:?} serial.sb={:#04X} serial.clock_mode={:?} serial.transfer_state={:?}\n",
                "boot.console_model={:?} boot.startup_mode={:?} boot.status={:?} boot.boot_rom_kind={:?} boot.boot_rom_mapped={} boot.asset_configured={} boot.memory_policy={:?}\n",
                "interrupts.console_model={:?} interrupts.status={:?}\n",
                "joypad.console_model={:?} joypad.status={:?}\n",
                "cartridge.state={:?}\n"
            ),
            self.config.console_model,
            self.config.startup_mode,
            self.config.compatibility.execution_mode,
            self.scheduler.next_t_cycle.get(),
            self.trace.buffered_event_count,
            last_trace_line,
            self.debug_controls.breakpoint_count,
            self.debug_controls.enabled_breakpoint_count,
            self.debug_controls.watchpoint_count,
            self.debug_controls.enabled_watchpoint_count,
            self.cpu.console_model,
            self.cpu.status,
            self.cpu.startup_state.pc,
            self.cpu.registers.pc,
            self.cpu.execution_state,
            self.cpu.current_opcode,
            self.bus.console_model,
            self.bus.status,
            self.apu.console_model,
            self.apu.status,
            self.apu.powered,
            self.apu.div_apu,
            self.ppu.console_model,
            self.ppu.status,
            self.ppu.lcd_state,
            self.ppu.mode,
            self.ppu.visible_output,
            self.ppu.ly,
            self.ppu.line_dot,
            self.ppu.mode_dot,
            self.ppu.mode0_start_dot,
            self.ppu.visible_pixels_output,
            self.ppu.scx_discard_remaining,
            self.ppu.bg_fetcher_source,
            self.ppu.bg_fetcher_stage,
            self.ppu.bg_fetcher_stage_dot,
            self.ppu.bg_fetcher_tile_map_address,
            self.ppu.bg_fetcher_tile_data_address,
            self.ppu.bg_push_pending,
            self.ppu.bg_fill_pending,
            self.ppu.bg_startup_source_state,
            self.ppu.bg_startup_fetch_seam,
            self.ppu.bg_startup_fifo_placeholders,
            self.ppu.bg_push_entry_delay_remaining,
            self.ppu.bg_fill_startup_dummy_pixels,
            self.ppu.bg_fetcher_post_alignment_restart_delay_dots,
            self.ppu.bg_transfer_phase,
            self.ppu.bg_current_transfer_x,
            self.ppu.bg_current_transfer_lane,
            self.ppu.bg_current_transfer_source_window,
            self.ppu.bg_current_transfer_backing,
            self.ppu.bg_current_transfer_readiness,
            self.ppu.bg_current_transfer_kind,
            self.ppu.bg_fifo_pixels,
            self.ppu.bg_fifo_cached_pixels,
            self.ppu.window_wy_latch,
            self.ppu.window_started_this_line,
            self.ppu.window_line_counter,
            self.dma.console_model,
            self.dma.status,
            self.timer.console_model,
            self.timer.status,
            self.serial.console_model,
            self.serial.status,
            self.serial.sb,
            self.serial.clock_mode,
            self.serial.transfer_state,
            self.boot.console_model,
            self.boot.startup_mode,
            self.boot.status,
            self.boot.boot_rom_kind,
            self.boot.boot_rom_mapped,
            self.boot.boot_rom_asset_configured,
            self.boot.startup_memory_policy,
            self.interrupts.console_model,
            self.interrupts.status,
            self.joypad.console_model,
            self.joypad.status,
            self.cartridge.state,
        )
    }
}

#[derive(Debug, Clone)]
pub struct Tracer<S> {
    next_sequence: u64,
    sink: S,
}

impl<S: TraceSink> Tracer<S> {
    pub fn new(sink: S) -> Self {
        Self {
            next_sequence: 0,
            sink,
        }
    }

    pub fn emit(
        &mut self,
        subsystem: TraceSubsystem,
        level: TraceLevel,
        message: impl Into<String>,
    ) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .expect("trace sequence overflow");

        if self.sink.records_events() {
            self.sink.push(TraceEvent {
                sequence,
                subsystem,
                level,
                message: message.into(),
            });
        }

        sequence
    }

    pub fn emit_with<F, M>(
        &mut self,
        subsystem: TraceSubsystem,
        level: TraceLevel,
        message: F,
    ) -> u64
    where
        F: FnOnce() -> M,
        M: Into<String>,
    {
        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .expect("trace sequence overflow");

        if self.sink.records_events() {
            self.sink.push(TraceEvent {
                sequence,
                subsystem,
                level,
                message: message().into(),
            });
        }

        sequence
    }

    pub fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn sink(&self) -> &S {
        &self.sink
    }

    pub fn sink_mut(&mut self) -> &mut S {
        &mut self.sink
    }

    pub fn into_sink(self) -> S {
        self.sink
    }

    pub fn reset(&mut self) {
        self.next_sequence = 0;
        self.sink.reset();
    }
}

impl<S: TraceSink + TraceSnapshotProvider> Tracer<S> {
    pub fn snapshot(&self) -> DebugSnapshot {
        self.sink.snapshot(self.next_sequence)
    }
}

impl Tracer<TraceBuffer> {
    pub fn in_memory() -> Self {
        Self::new(TraceBuffer::new())
    }
}

impl Tracer<TraceSummaryBuffer> {
    pub fn summary() -> Self {
        Self::new(TraceSummaryBuffer::new())
    }
}

pub fn supported_trace_subsystems() -> &'static [TraceSubsystem] {
    &SUPPORTED_TRACE_SUBSYSTEMS
}

#[cfg(test)]
mod tests;
