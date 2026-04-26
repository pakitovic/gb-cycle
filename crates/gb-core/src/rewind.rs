use std::collections::VecDeque;
use std::fmt;
use std::mem;

use crate::debugger::TraceSink;
use crate::machine::Machine;
use crate::save_state::{MachineSaveState, MachineSaveStateRestoreError};
use crate::scheduler::TCycle;

pub const DMG_T_CYCLES_PER_FRAME: u64 = 456 * 154;
pub const DEFAULT_REWIND_HISTORY_FRAMES: u64 = 600;
pub const DEFAULT_REWIND_HISTORY_T_CYCLES: u64 =
    DMG_T_CYCLES_PER_FRAME * DEFAULT_REWIND_HISTORY_FRAMES;
pub const DEFAULT_REWIND_MAX_ESTIMATED_BYTES: usize = 256 * 1024 * 1024;
const REWIND_SAVE_STATE_DYNAMIC_BASELINE_BYTES: usize = 192 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MachineRewindCaptureKind {
    FrameBoundary,
    Subframe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MachineRewindSubframeCadence {
    Disabled,
    EveryTCycles(u64),
    FixedPerFrame { captures_per_frame: u16 },
}

impl MachineRewindSubframeCadence {
    const fn normalized(self) -> Self {
        match self {
            Self::EveryTCycles(0)
            | Self::FixedPerFrame {
                captures_per_frame: 0,
            } => Self::Disabled,
            cadence => cadence,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MachineRewindConfig {
    pub target_history_t_cycles: u64,
    pub max_estimated_bytes: usize,
    pub subframe_cadence: MachineRewindSubframeCadence,
}

impl MachineRewindConfig {
    pub const fn new() -> Self {
        Self {
            target_history_t_cycles: DEFAULT_REWIND_HISTORY_T_CYCLES,
            max_estimated_bytes: DEFAULT_REWIND_MAX_ESTIMATED_BYTES,
            subframe_cadence: MachineRewindSubframeCadence::FixedPerFrame {
                captures_per_frame: 1,
            },
        }
    }

    pub const fn with_target_history_t_cycles(mut self, target_history_t_cycles: u64) -> Self {
        self.target_history_t_cycles = if target_history_t_cycles == 0 {
            1
        } else {
            target_history_t_cycles
        };
        self
    }

    pub const fn with_max_estimated_bytes(mut self, max_estimated_bytes: usize) -> Self {
        self.max_estimated_bytes = if max_estimated_bytes == 0 {
            1
        } else {
            max_estimated_bytes
        };
        self
    }

    pub const fn with_subframe_cadence(
        mut self,
        subframe_cadence: MachineRewindSubframeCadence,
    ) -> Self {
        self.subframe_cadence = subframe_cadence;
        self
    }
}

impl Default for MachineRewindConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MachineRewindStats {
    pub len: usize,
    pub estimated_bytes: usize,
    pub oldest_next_t_cycle: Option<TCycle>,
    pub newest_next_t_cycle: Option<TCycle>,
    pub frame_boundary_captures: u64,
    pub subframe_captures: u64,
    pub skipped_subframes: u64,
    pub duplicate_captures: u64,
    pub evicted_snapshots: u64,
    pub restored_snapshots: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachineRewindRestore {
    pub capture_kind: MachineRewindCaptureKind,
    pub restored_next_t_cycle: TCycle,
    pub estimated_bytes: usize,
    pub remaining_snapshots: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineRewindRestoreError {
    SaveStateRestore(MachineSaveStateRestoreError),
}

impl fmt::Display for MachineRewindRestoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SaveStateRestore(error) => write!(f, "rewind restore failed: {error}"),
        }
    }
}

impl std::error::Error for MachineRewindRestoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SaveStateRestore(error) => Some(error),
        }
    }
}

impl From<MachineSaveStateRestoreError> for MachineRewindRestoreError {
    fn from(error: MachineSaveStateRestoreError) -> Self {
        Self::SaveStateRestore(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MachineRewindFramePosition {
    pub next_t_cycle: TCycle,
    pub ly: u8,
    pub dot: u16,
}

impl MachineRewindFramePosition {
    pub const fn is_frame_boundary(self) -> bool {
        self.ly == 0 && self.dot == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct MachineRewindFrameBoundaryTracker {
    previous: Option<MachineRewindFramePosition>,
}

impl MachineRewindFrameBoundaryTracker {
    pub const fn new() -> Self {
        Self { previous: None }
    }

    pub fn reset(&mut self) {
        self.previous = None;
    }

    pub fn previous(&self) -> Option<MachineRewindFramePosition> {
        self.previous
    }

    pub fn observe<S: TraceSink>(&mut self, machine: &Machine<S>) -> bool {
        let current = machine_rewind_frame_position(machine);
        let crossed = match self.previous {
            None => current.is_frame_boundary(),
            Some(previous) => {
                (!previous.is_frame_boundary() && current.is_frame_boundary())
                    || current.ly < previous.ly
            }
        };
        self.previous = Some(current);
        crossed
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MachineRewindEntry {
    state: MachineSaveState,
    capture_kind: MachineRewindCaptureKind,
    next_t_cycle: TCycle,
    estimated_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct MachineRewindBuffer {
    config: MachineRewindConfig,
    entries: VecDeque<MachineRewindEntry>,
    estimated_bytes: usize,
    frame_boundary_captures: u64,
    subframe_captures: u64,
    skipped_subframes: u64,
    duplicate_captures: u64,
    evicted_snapshots: u64,
    restored_snapshots: u64,
    current_frame_start_t_cycle: Option<TCycle>,
    subframes_recorded_this_frame: u16,
    last_subframe_t_cycle: Option<TCycle>,
}

impl MachineRewindBuffer {
    pub fn new(config: MachineRewindConfig) -> Self {
        Self {
            config,
            entries: VecDeque::new(),
            estimated_bytes: 0,
            frame_boundary_captures: 0,
            subframe_captures: 0,
            skipped_subframes: 0,
            duplicate_captures: 0,
            evicted_snapshots: 0,
            restored_snapshots: 0,
            current_frame_start_t_cycle: None,
            subframes_recorded_this_frame: 0,
            last_subframe_t_cycle: None,
        }
    }

    pub fn config(&self) -> MachineRewindConfig {
        self.config
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.estimated_bytes = 0;
        self.frame_boundary_captures = 0;
        self.subframe_captures = 0;
        self.skipped_subframes = 0;
        self.duplicate_captures = 0;
        self.evicted_snapshots = 0;
        self.restored_snapshots = 0;
        self.current_frame_start_t_cycle = None;
        self.subframes_recorded_this_frame = 0;
        self.last_subframe_t_cycle = None;
    }

    pub fn stats(&self) -> MachineRewindStats {
        MachineRewindStats {
            len: self.len(),
            estimated_bytes: self.estimated_bytes,
            oldest_next_t_cycle: self.entries.front().map(|entry| entry.next_t_cycle),
            newest_next_t_cycle: self.entries.back().map(|entry| entry.next_t_cycle),
            frame_boundary_captures: self.frame_boundary_captures,
            subframe_captures: self.subframe_captures,
            skipped_subframes: self.skipped_subframes,
            duplicate_captures: self.duplicate_captures,
            evicted_snapshots: self.evicted_snapshots,
            restored_snapshots: self.restored_snapshots,
        }
    }

    pub fn record_frame_boundary<S: TraceSink>(&mut self, machine: &Machine<S>) -> bool {
        self.current_frame_start_t_cycle = Some(machine.next_t_cycle());
        self.subframes_recorded_this_frame = 0;
        let captured = self.push_capture(machine, MachineRewindCaptureKind::FrameBoundary);
        if captured {
            self.frame_boundary_captures = self.frame_boundary_captures.saturating_add(1);
        }
        captured
    }

    pub fn record_subframe<S: TraceSink>(&mut self, machine: &Machine<S>) -> bool {
        if !self.should_record_subframe(machine.next_t_cycle()) {
            self.skipped_subframes = self.skipped_subframes.saturating_add(1);
            return false;
        }

        let captured = self.push_capture(machine, MachineRewindCaptureKind::Subframe);
        if captured {
            self.subframe_captures = self.subframe_captures.saturating_add(1);
            self.last_subframe_t_cycle = Some(machine.next_t_cycle());
            if matches!(
                self.config.subframe_cadence.normalized(),
                MachineRewindSubframeCadence::FixedPerFrame { .. }
            ) {
                self.subframes_recorded_this_frame =
                    self.subframes_recorded_this_frame.saturating_add(1);
            }
        }
        captured
    }

    pub fn rewind_one<S: TraceSink>(
        &mut self,
        machine: &mut Machine<S>,
    ) -> Result<Option<MachineRewindRestore>, MachineRewindRestoreError> {
        if self.entries.is_empty() {
            return Ok(None);
        }

        let mut target_index = self.entries.len() - 1;
        if self.entries.len() > 1
            && self.entries[target_index].next_t_cycle == machine.next_t_cycle()
        {
            target_index -= 1;
        }

        let target = &self.entries[target_index];
        let restored = MachineRewindRestore {
            capture_kind: target.capture_kind,
            restored_next_t_cycle: target.next_t_cycle,
            estimated_bytes: target.estimated_bytes,
            remaining_snapshots: target_index,
        };
        machine.restore_save_state(&target.state)?;

        while self.entries.len() > target_index {
            self.pop_back();
        }
        self.restored_snapshots = self.restored_snapshots.saturating_add(1);
        self.current_frame_start_t_cycle = None;
        self.subframes_recorded_this_frame = 0;
        self.last_subframe_t_cycle = self.entries.back().and_then(|entry| {
            (entry.capture_kind == MachineRewindCaptureKind::Subframe).then_some(entry.next_t_cycle)
        });

        Ok(Some(restored))
    }

    fn should_record_subframe(&self, current: TCycle) -> bool {
        match self.config.subframe_cadence.normalized() {
            MachineRewindSubframeCadence::Disabled => false,
            MachineRewindSubframeCadence::EveryTCycles(interval) => {
                let anchor = self
                    .last_subframe_t_cycle
                    .or(self.current_frame_start_t_cycle)
                    .or_else(|| self.entries.back().map(|entry| entry.next_t_cycle));
                anchor
                    .map(|anchor| current.get().saturating_sub(anchor.get()) >= interval)
                    .unwrap_or(true)
            }
            MachineRewindSubframeCadence::FixedPerFrame { captures_per_frame } => {
                if self.subframes_recorded_this_frame >= captures_per_frame {
                    return false;
                }

                let interval =
                    DMG_T_CYCLES_PER_FRAME / (u64::from(captures_per_frame).saturating_add(1));
                let threshold =
                    interval * u64::from(self.subframes_recorded_this_frame.saturating_add(1));

                match self.current_frame_start_t_cycle {
                    Some(frame_start) => {
                        current.get().saturating_sub(frame_start.get()) >= threshold
                    }
                    None => self
                        .last_subframe_t_cycle
                        .or_else(|| self.entries.back().map(|entry| entry.next_t_cycle))
                        .map(|anchor| current.get().saturating_sub(anchor.get()) >= interval)
                        .unwrap_or(true),
                }
            }
        }
    }

    fn push_capture<S: TraceSink>(
        &mut self,
        machine: &Machine<S>,
        capture_kind: MachineRewindCaptureKind,
    ) -> bool {
        let state = machine.capture_save_state();
        let next_t_cycle = state.metadata().next_t_cycle;
        let estimated_bytes = estimate_machine_save_state_bytes(&state);

        if let Some(latest) = self.entries.back_mut()
            && latest.next_t_cycle == next_t_cycle
        {
            self.estimated_bytes = self
                .estimated_bytes
                .saturating_sub(latest.estimated_bytes)
                .saturating_add(estimated_bytes);
            *latest = MachineRewindEntry {
                state,
                capture_kind: if capture_kind == MachineRewindCaptureKind::FrameBoundary {
                    MachineRewindCaptureKind::FrameBoundary
                } else {
                    latest.capture_kind
                },
                next_t_cycle,
                estimated_bytes,
            };
            self.duplicate_captures = self.duplicate_captures.saturating_add(1);
            return false;
        }

        self.estimated_bytes = self.estimated_bytes.saturating_add(estimated_bytes);
        self.entries.push_back(MachineRewindEntry {
            state,
            capture_kind,
            next_t_cycle,
            estimated_bytes,
        });
        self.enforce_limits();
        true
    }

    fn enforce_limits(&mut self) {
        let Some(newest) = self.entries.back().map(|entry| entry.next_t_cycle) else {
            return;
        };

        while self.entries.len() > 1 {
            let Some(oldest) = self.entries.front().map(|entry| entry.next_t_cycle) else {
                break;
            };
            if newest.get().saturating_sub(oldest.get()) <= self.config.target_history_t_cycles {
                break;
            }
            self.pop_front_evicted();
        }

        while self.entries.len() > 1 && self.estimated_bytes > self.config.max_estimated_bytes {
            self.pop_front_evicted();
        }
    }

    fn pop_front_evicted(&mut self) {
        if let Some(entry) = self.entries.pop_front() {
            self.estimated_bytes = self.estimated_bytes.saturating_sub(entry.estimated_bytes);
            self.evicted_snapshots = self.evicted_snapshots.saturating_add(1);
        }
    }

    fn pop_back(&mut self) {
        if let Some(entry) = self.entries.pop_back() {
            self.estimated_bytes = self.estimated_bytes.saturating_sub(entry.estimated_bytes);
        }
    }
}

impl Default for MachineRewindBuffer {
    fn default() -> Self {
        Self::new(MachineRewindConfig::default())
    }
}

pub fn machine_rewind_frame_position<S: TraceSink>(
    machine: &Machine<S>,
) -> MachineRewindFramePosition {
    MachineRewindFramePosition {
        next_t_cycle: machine.next_t_cycle(),
        ly: machine.ppu().ly(),
        dot: machine.ppu().line_dot(),
    }
}

pub fn machine_is_rewind_frame_boundary<S: TraceSink>(machine: &Machine<S>) -> bool {
    machine_rewind_frame_position(machine).is_frame_boundary()
}

fn estimate_machine_save_state_bytes(state: &MachineSaveState) -> usize {
    mem::size_of_val(state)
        .saturating_add(REWIND_SAVE_STATE_DYNAMIC_BASELINE_BYTES)
        .saturating_add(
            state
                .metadata()
                .cartridge
                .rom_fingerprint
                .map(|fingerprint| saturating_usize_from_u64(fingerprint.len))
                .unwrap_or(0),
        )
}

fn saturating_usize_from_u64(value: u64) -> usize {
    value.try_into().unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ConsoleModel, MachineConfig, StartupMode};

    const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;

    fn build_test_rom(program: &[u8]) -> Vec<u8> {
        let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
        for (offset, byte) in program.iter().copied().enumerate() {
            rom[0x0100 + offset] = byte;
        }
        rom[0x0147] = 0x00;
        rom[0x0148] = 0x00;
        rom[0x0149] = 0x00;
        rom
    }

    fn test_machine() -> Machine {
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );
        machine
            .load_cartridge(build_test_rom(&[0x00]))
            .expect("NoMBC test ROM should load");
        machine
    }

    fn step_t_cycles(machine: &mut Machine, t_cycles: u64) {
        for _ in 0..t_cycles {
            machine.step_t_cycle();
        }
    }

    #[test]
    fn rewind_buffer_records_frame_and_subframe_order() {
        let mut machine = test_machine();
        let mut buffer = MachineRewindBuffer::new(
            MachineRewindConfig::default()
                .with_subframe_cadence(MachineRewindSubframeCadence::EveryTCycles(4)),
        );

        assert!(buffer.record_frame_boundary(&machine));
        step_t_cycles(&mut machine, 3);
        assert!(!buffer.record_subframe(&machine));
        step_t_cycles(&mut machine, 1);
        assert!(buffer.record_subframe(&machine));

        let stats = buffer.stats();
        assert_eq!(stats.len, 2);
        assert_eq!(stats.oldest_next_t_cycle, Some(TCycle::ZERO));
        assert_eq!(stats.newest_next_t_cycle, Some(TCycle::new(4)));
        assert_eq!(stats.frame_boundary_captures, 1);
        assert_eq!(stats.subframe_captures, 1);
        assert_eq!(stats.skipped_subframes, 1);
    }

    #[test]
    fn rewind_buffer_evictions_follow_duration_and_byte_limits() {
        let mut machine = test_machine();
        let mut duration_buffer = MachineRewindBuffer::new(
            MachineRewindConfig::default()
                .with_target_history_t_cycles(5)
                .with_max_estimated_bytes(usize::MAX),
        );
        assert!(duration_buffer.record_frame_boundary(&machine));
        step_t_cycles(&mut machine, 6);
        assert!(duration_buffer.record_frame_boundary(&machine));
        assert_eq!(duration_buffer.stats().len, 1);
        assert_eq!(duration_buffer.stats().evicted_snapshots, 1);

        let mut byte_buffer = MachineRewindBuffer::new(
            MachineRewindConfig::default()
                .with_target_history_t_cycles(u64::MAX)
                .with_max_estimated_bytes(1),
        );
        assert!(byte_buffer.record_frame_boundary(&machine));
        step_t_cycles(&mut machine, 1);
        assert!(byte_buffer.record_frame_boundary(&machine));
        assert_eq!(byte_buffer.stats().len, 1);
        assert_eq!(byte_buffer.stats().evicted_snapshots, 1);
        assert!(
            byte_buffer.stats().estimated_bytes > byte_buffer.config().max_estimated_bytes,
            "a single oversized full snapshot is retained even when it exceeds the byte budget"
        );
    }

    #[test]
    fn rewind_buffer_clear_resets_snapshots_and_stats() {
        let machine = test_machine();
        let mut buffer = MachineRewindBuffer::default();

        assert!(buffer.record_frame_boundary(&machine));
        assert!(!buffer.is_empty());
        assert_ne!(buffer.stats(), MachineRewindStats::default());

        buffer.clear();

        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.stats(), MachineRewindStats::default());
    }

    #[test]
    fn rewind_subframe_fixed_per_frame_cadence_is_deterministic() {
        let mut machine = test_machine();
        let mut buffer =
            MachineRewindBuffer::new(MachineRewindConfig::default().with_subframe_cadence(
                MachineRewindSubframeCadence::FixedPerFrame {
                    captures_per_frame: 2,
                },
            ));
        let interval = DMG_T_CYCLES_PER_FRAME / 3;

        assert!(buffer.record_frame_boundary(&machine));
        step_t_cycles(&mut machine, interval - 1);
        assert!(!buffer.record_subframe(&machine));
        step_t_cycles(&mut machine, 1);
        assert!(buffer.record_subframe(&machine));
        step_t_cycles(&mut machine, interval - 1);
        assert!(!buffer.record_subframe(&machine));
        step_t_cycles(&mut machine, 1);
        assert!(buffer.record_subframe(&machine));
        step_t_cycles(&mut machine, interval);
        assert!(!buffer.record_subframe(&machine));

        assert_eq!(buffer.stats().len, 3);
        assert_eq!(buffer.stats().subframe_captures, 2);
    }

    #[test]
    fn rewind_config_clamps_zero_limits_and_zero_cadence_disables_subframes() {
        let machine = test_machine();
        let config = MachineRewindConfig::default()
            .with_target_history_t_cycles(0)
            .with_max_estimated_bytes(0)
            .with_subframe_cadence(MachineRewindSubframeCadence::EveryTCycles(0));
        assert_eq!(config.target_history_t_cycles, 1);
        assert_eq!(config.max_estimated_bytes, 1);

        let mut every_zero_buffer = MachineRewindBuffer::new(config);
        assert!(!every_zero_buffer.record_subframe(&machine));
        assert_eq!(every_zero_buffer.stats().skipped_subframes, 1);
        assert!(every_zero_buffer.is_empty());

        let mut fixed_zero_buffer =
            MachineRewindBuffer::new(MachineRewindConfig::default().with_subframe_cadence(
                MachineRewindSubframeCadence::FixedPerFrame {
                    captures_per_frame: 0,
                },
            ));
        assert!(!fixed_zero_buffer.record_subframe(&machine));
        assert_eq!(fixed_zero_buffer.stats().skipped_subframes, 1);
        assert!(fixed_zero_buffer.is_empty());
    }

    #[test]
    fn rewind_subframe_cadence_can_start_without_a_frame_boundary() {
        let mut machine = test_machine();
        let mut every_buffer = MachineRewindBuffer::new(
            MachineRewindConfig::default()
                .with_subframe_cadence(MachineRewindSubframeCadence::EveryTCycles(8)),
        );

        assert!(every_buffer.record_subframe(&machine));
        assert!(!every_buffer.record_subframe(&machine));
        step_t_cycles(&mut machine, 8);
        assert!(every_buffer.record_subframe(&machine));
        assert_eq!(every_buffer.stats().len, 2);
        assert_eq!(every_buffer.stats().subframe_captures, 2);
        assert_eq!(every_buffer.stats().skipped_subframes, 1);

        let mut fixed_machine = test_machine();
        let mut fixed_buffer =
            MachineRewindBuffer::new(MachineRewindConfig::default().with_subframe_cadence(
                MachineRewindSubframeCadence::FixedPerFrame {
                    captures_per_frame: 1,
                },
            ));

        assert!(fixed_buffer.record_subframe(&fixed_machine));
        step_t_cycles(&mut fixed_machine, DMG_T_CYCLES_PER_FRAME);
        assert!(!fixed_buffer.record_subframe(&fixed_machine));
        assert_eq!(fixed_buffer.stats().len, 1);
        assert_eq!(fixed_buffer.stats().subframe_captures, 1);
        assert_eq!(fixed_buffer.stats().skipped_subframes, 1);
    }

    #[test]
    fn rewind_duplicate_same_t_cycle_replaces_latest_and_preserves_frame_priority() {
        let mut machine = test_machine();
        let mut buffer = MachineRewindBuffer::new(
            MachineRewindConfig::default()
                .with_subframe_cadence(MachineRewindSubframeCadence::EveryTCycles(1)),
        );

        assert!(buffer.record_subframe(&machine));
        assert!(!buffer.record_frame_boundary(&machine));
        let stats = buffer.stats();
        assert_eq!(stats.len, 1);
        assert_eq!(stats.duplicate_captures, 1);
        assert_eq!(stats.frame_boundary_captures, 0);
        assert_eq!(stats.subframe_captures, 1);

        step_t_cycles(&mut machine, 4);
        let restored = buffer
            .rewind_one(&mut machine)
            .expect("duplicate snapshot should restore")
            .expect("buffer should still hold the replaced snapshot");

        assert_eq!(
            restored.capture_kind,
            MachineRewindCaptureKind::FrameBoundary
        );
        assert_eq!(restored.remaining_snapshots, 0);
        assert!(restored.estimated_bytes > 0);
        assert_eq!(restored.restored_next_t_cycle, TCycle::ZERO);
        assert!(buffer.is_empty());
    }

    #[test]
    fn rewind_one_restores_latest_past_snapshot_and_then_older_snapshots() {
        let mut machine = test_machine();
        let mut buffer = MachineRewindBuffer::new(
            MachineRewindConfig::default()
                .with_subframe_cadence(MachineRewindSubframeCadence::EveryTCycles(1)),
        );

        assert!(buffer.record_frame_boundary(&machine));
        step_t_cycles(&mut machine, 10);
        assert!(buffer.record_subframe(&machine));
        let ten_t_cycle_state = machine.capture_save_state();
        step_t_cycles(&mut machine, 10);
        assert!(buffer.record_subframe(&machine));
        step_t_cycles(&mut machine, 10);

        let restored = buffer
            .rewind_one(&mut machine)
            .expect("rewind restore should succeed")
            .expect("buffer should contain a snapshot");
        assert_eq!(restored.restored_next_t_cycle, TCycle::new(20));
        assert_eq!(machine.next_t_cycle(), TCycle::new(20));

        let restored = buffer
            .rewind_one(&mut machine)
            .expect("second rewind restore should succeed")
            .expect("older snapshot should remain");
        assert_eq!(restored.restored_next_t_cycle, TCycle::new(10));
        assert_eq!(machine.capture_save_state(), ten_t_cycle_state);
    }

    #[test]
    fn rewind_one_skips_the_current_snapshot_when_present() {
        let mut machine = test_machine();
        let mut buffer = MachineRewindBuffer::default();

        assert!(buffer.record_frame_boundary(&machine));
        step_t_cycles(&mut machine, 10);
        assert!(buffer.record_frame_boundary(&machine));

        let restored = buffer
            .rewind_one(&mut machine)
            .expect("rewind restore should succeed")
            .expect("older snapshot should be selected instead of the current one");

        assert_eq!(restored.restored_next_t_cycle, TCycle::ZERO);
        assert_eq!(machine.next_t_cycle(), TCycle::ZERO);
    }

    #[test]
    fn rewind_one_reports_empty_and_preserves_buffer_on_restore_failure() {
        let source = test_machine();
        let mut target = Machine::new(
            MachineConfig::new(ConsoleModel::Mgb).with_startup_mode(StartupMode::SkipBoot),
        );
        let before = target.capture_save_state();
        let mut empty_buffer = MachineRewindBuffer::default();

        assert_eq!(
            empty_buffer
                .rewind_one(&mut target)
                .expect("empty rewind should not be an error"),
            None
        );

        let mut incompatible_buffer = MachineRewindBuffer::default();
        assert!(incompatible_buffer.record_frame_boundary(&source));

        let error = incompatible_buffer
            .rewind_one(&mut target)
            .expect_err("incompatible target should reject the save state");
        assert!(matches!(
            error,
            MachineRewindRestoreError::SaveStateRestore(
                MachineSaveStateRestoreError::ConsoleModelMismatch { .. }
            )
        ));
        assert_eq!(target.capture_save_state(), before);
        assert_eq!(incompatible_buffer.len(), 1);
    }

    #[test]
    fn rewind_restore_error_exposes_wrapped_save_state_error() {
        let source = test_machine();
        let mut target = Machine::new(
            MachineConfig::new(ConsoleModel::Mgb).with_startup_mode(StartupMode::SkipBoot),
        );
        let mut buffer = MachineRewindBuffer::default();

        assert!(buffer.record_frame_boundary(&source));

        let error = buffer
            .rewind_one(&mut target)
            .expect_err("incompatible target should reject the save state");
        assert!(
            error.to_string().contains("rewind restore failed"),
            "Display should keep rewind context"
        );
        assert!(
            std::error::Error::source(&error).is_some(),
            "rewind errors should expose the MachineSaveStateRestoreError source"
        );
    }

    #[test]
    fn frame_boundary_tracker_reports_frame_origin_crossings_without_recording_policy() {
        let mut machine = test_machine();
        let mut tracker = MachineRewindFrameBoundaryTracker::new();

        assert!(machine_is_rewind_frame_boundary(&machine));
        assert!(tracker.observe(&machine));
        assert!(!tracker.observe(&machine));

        machine.step_t_cycle();

        assert!(!machine_is_rewind_frame_boundary(&machine));
        assert!(!tracker.observe(&machine));

        tracker.reset();

        assert_eq!(tracker.previous(), None);
    }

    #[test]
    fn frame_boundary_tracker_reports_ly_wraps_between_observations() {
        let machine = test_machine();
        assert!(machine_is_rewind_frame_boundary(&machine));

        let mut dot_tracker = MachineRewindFrameBoundaryTracker {
            previous: Some(MachineRewindFramePosition {
                next_t_cycle: TCycle::new(1),
                ly: 0,
                dot: 1,
            }),
        };
        assert!(dot_tracker.observe(&machine));

        let mut ly_wrap_tracker = MachineRewindFrameBoundaryTracker {
            previous: Some(MachineRewindFramePosition {
                next_t_cycle: TCycle::new(DMG_T_CYCLES_PER_FRAME - 1),
                ly: 153,
                dot: 455,
            }),
        };
        assert!(ly_wrap_tracker.observe(&machine));
    }
}
