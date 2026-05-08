use crate::model::{ConsoleModel, OperatingMode};
use crate::scheduler::{CycleContext, InterruptSource};
use crate::speed::CgbSpeedMode;

const SC_TRANSFER_REQUEST_BIT: u8 = 0x80;
const SC_UNUSED_READ_MASK: u8 = 0x7C;
const SC_CGB_HIGH_SPEED_BIT: u8 = 0x02;
const SC_CLOCK_MODE_BIT: u8 = 0x01;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SerialStatus {
    Ready,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum SerialClockMode {
    #[default]
    External,
    Internal,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum SerialPeer {
    #[default]
    Disconnected,
    Loopback,
    StagedIncomingByte {
        byte: u8,
    },
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum SerialTransferState {
    #[default]
    Idle,
    TransferRequested {
        bits_shifted: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SerialTickTelemetry {
    pub active_t_cycles: u64,
    pub internal_ticks: u64,
    pub external_ticks: u64,
    pub external_wait_ticks: u64,
    pub shift_edges: u64,
    pub completed_bytes: u64,
    pub external_port_ticks: u64,
}

impl SerialTickTelemetry {
    pub const fn external_port_tick() -> Self {
        Self {
            external_port_ticks: 1,
            active_t_cycles: 0,
            internal_ticks: 0,
            external_ticks: 0,
            external_wait_ticks: 0,
            shift_edges: 0,
            completed_bytes: 0,
        }
    }

    pub fn accumulate(&mut self, other: Self) {
        self.active_t_cycles = self.active_t_cycles.saturating_add(other.active_t_cycles);
        self.internal_ticks = self.internal_ticks.saturating_add(other.internal_ticks);
        self.external_ticks = self.external_ticks.saturating_add(other.external_ticks);
        self.external_wait_ticks = self
            .external_wait_ticks
            .saturating_add(other.external_wait_ticks);
        self.shift_edges = self.shift_edges.saturating_add(other.shift_edges);
        self.completed_bytes = self.completed_bytes.saturating_add(other.completed_bytes);
        self.external_port_ticks = self
            .external_port_ticks
            .saturating_add(other.external_port_ticks);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SerialStartupState {
    pub sb: u8,
    pub clock_mode: SerialClockMode,
    pub cgb_high_speed_clock: bool,
    pub transfer_state: SerialTransferState,
    pub clock_counter: u16,
}

impl SerialStartupState {
    pub const fn from_registers(sb: u8, sc: u8) -> Self {
        Self {
            sb,
            clock_mode: if sc & SC_CLOCK_MODE_BIT != 0 {
                SerialClockMode::Internal
            } else {
                SerialClockMode::External
            },
            cgb_high_speed_clock: sc & SC_CGB_HIGH_SPEED_BIT != 0,
            transfer_state: if sc & SC_TRANSFER_REQUEST_BIT != 0 {
                SerialTransferState::TransferRequested { bits_shifted: 0 }
            } else {
                SerialTransferState::Idle
            },
            clock_counter: 0,
        }
    }

    pub const fn with_clock_counter(mut self, clock_counter: u16) -> Self {
        self.clock_counter = clock_counter;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Serial {
    console_model: ConsoleModel,
    operating_mode: OperatingMode,
    status: SerialStatus,
    sb: u8,
    clock_mode: SerialClockMode,
    cgb_high_speed_clock: bool,
    transfer_state: SerialTransferState,
    peer: SerialPeer,
    clock_counter: u16,
    external_clock_pulses_pending: u8,
    staged_outgoing_byte: u8,
    current_outgoing_byte: u8,
    current_outgoing_shift_byte: u8,
    current_incoming_byte: u8,
    latest_completed_output_byte: Option<u8>,
    completed_output_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SerialSaveState {
    console_model: ConsoleModel,
    operating_mode: OperatingMode,
    status: SerialStatus,
    sb: u8,
    clock_mode: SerialClockMode,
    cgb_high_speed_clock: bool,
    transfer_state: SerialTransferState,
    peer: SerialPeer,
    clock_counter: u16,
    external_clock_pulses_pending: u8,
    staged_outgoing_byte: u8,
    current_outgoing_byte: u8,
    current_outgoing_shift_byte: u8,
    current_incoming_byte: u8,
    latest_completed_output_byte: Option<u8>,
    completed_output_bytes: Vec<u8>,
}

impl SerialSaveState {
    pub(crate) fn dynamic_payload_bytes(&self) -> usize {
        self.completed_output_bytes.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SerialSnapshot {
    pub console_model: ConsoleModel,
    pub operating_mode: OperatingMode,
    pub status: SerialStatus,
    pub sb: u8,
    pub clock_mode: SerialClockMode,
    pub cgb_high_speed_clock: bool,
    pub transfer_state: SerialTransferState,
    pub peer: SerialPeer,
}

impl Serial {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self::new_with_operating_mode(console_model, console_model.default_operating_mode())
    }

    pub fn new_with_operating_mode(
        console_model: ConsoleModel,
        operating_mode: OperatingMode,
    ) -> Self {
        Self {
            console_model,
            operating_mode,
            status: SerialStatus::Ready,
            sb: 0,
            clock_mode: SerialClockMode::External,
            cgb_high_speed_clock: false,
            transfer_state: SerialTransferState::Idle,
            peer: SerialPeer::Disconnected,
            clock_counter: 0,
            external_clock_pulses_pending: 0,
            staged_outgoing_byte: 0,
            current_outgoing_byte: 0,
            current_outgoing_shift_byte: 0,
            current_incoming_byte: 0,
            latest_completed_output_byte: None,
            completed_output_bytes: Vec::new(),
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn operating_mode(&self) -> OperatingMode {
        self.operating_mode
    }

    pub fn status(&self) -> SerialStatus {
        self.status
    }

    pub(crate) fn apply_operating_mode_state(&mut self, operating_mode: OperatingMode) {
        self.operating_mode = operating_mode;
        if !self.cgb_high_speed_serial_enabled() {
            self.cgb_high_speed_clock = false;
        }
    }

    pub(crate) fn capture_save_state(&self) -> SerialSaveState {
        SerialSaveState {
            console_model: self.console_model,
            operating_mode: self.operating_mode,
            status: self.status,
            sb: self.sb,
            clock_mode: self.clock_mode,
            cgb_high_speed_clock: self.cgb_high_speed_clock,
            transfer_state: self.transfer_state,
            peer: self.peer,
            clock_counter: self.clock_counter,
            external_clock_pulses_pending: self.external_clock_pulses_pending,
            staged_outgoing_byte: self.staged_outgoing_byte,
            current_outgoing_byte: self.current_outgoing_byte,
            current_outgoing_shift_byte: self.current_outgoing_shift_byte,
            current_incoming_byte: self.current_incoming_byte,
            latest_completed_output_byte: self.latest_completed_output_byte,
            completed_output_bytes: self.completed_output_bytes.clone(),
        }
    }

    pub(crate) fn restore_save_state(&mut self, state: &SerialSaveState) {
        self.console_model = state.console_model;
        self.operating_mode = state.operating_mode;
        self.status = state.status;
        self.sb = state.sb;
        self.clock_mode = state.clock_mode;
        self.cgb_high_speed_clock = state.cgb_high_speed_clock;
        self.transfer_state = state.transfer_state;
        self.peer = state.peer;
        self.clock_counter = state.clock_counter;
        self.external_clock_pulses_pending = state.external_clock_pulses_pending;
        self.staged_outgoing_byte = state.staged_outgoing_byte;
        self.current_outgoing_byte = state.current_outgoing_byte;
        self.current_outgoing_shift_byte = state.current_outgoing_shift_byte;
        self.current_incoming_byte = state.current_incoming_byte;
        self.latest_completed_output_byte = state.latest_completed_output_byte;
        self.completed_output_bytes = state.completed_output_bytes.clone();
    }

    pub fn clock_mode(&self) -> SerialClockMode {
        self.clock_mode
    }

    pub fn cgb_high_speed_clock(&self) -> bool {
        self.cgb_high_speed_serial_enabled() && self.cgb_high_speed_clock
    }

    pub fn transfer_state(&self) -> SerialTransferState {
        self.transfer_state
    }

    pub fn peer(&self) -> SerialPeer {
        self.peer
    }

    pub fn read_sb(&self) -> u8 {
        self.sb
    }

    pub fn write_sb(&mut self, value: u8) {
        self.sb = value;
        self.staged_outgoing_byte = value;
    }

    pub fn read_sc(&self) -> u8 {
        SC_UNUSED_READ_MASK
            | self.read_sc_high_speed_bit()
            | match self.transfer_state {
                SerialTransferState::Idle => 0,
                SerialTransferState::TransferRequested { .. } => SC_TRANSFER_REQUEST_BIT,
            }
            | match self.clock_mode {
                SerialClockMode::External => 0,
                SerialClockMode::Internal => SC_CLOCK_MODE_BIT,
            }
    }

    pub fn write_sc(&mut self, value: u8) {
        self.clock_mode = if value & SC_CLOCK_MODE_BIT != 0 {
            SerialClockMode::Internal
        } else {
            SerialClockMode::External
        };
        self.cgb_high_speed_clock =
            self.cgb_high_speed_serial_enabled() && value & SC_CGB_HIGH_SPEED_BIT != 0;
        self.transfer_state = if value & SC_TRANSFER_REQUEST_BIT != 0 {
            SerialTransferState::TransferRequested { bits_shifted: 0 }
        } else {
            SerialTransferState::Idle
        };
        self.external_clock_pulses_pending = 0;
        self.current_outgoing_byte = 0;
        self.current_outgoing_shift_byte = 0;
        self.current_incoming_byte = 0;
        self.latest_completed_output_byte = None;
    }

    pub fn apply_startup_state(&mut self, startup_state: SerialStartupState) {
        self.sb = startup_state.sb;
        self.clock_mode = startup_state.clock_mode;
        self.cgb_high_speed_clock =
            self.cgb_high_speed_serial_enabled() && startup_state.cgb_high_speed_clock;
        self.transfer_state = startup_state.transfer_state;
        self.peer = SerialPeer::Disconnected;
        self.clock_counter = startup_state.clock_counter;
        self.external_clock_pulses_pending = 0;
        self.staged_outgoing_byte = startup_state.sb;
        self.current_outgoing_byte = 0;
        self.current_outgoing_shift_byte = 0;
        self.current_incoming_byte = 0;
        self.latest_completed_output_byte = None;
        self.completed_output_bytes.clear();
    }

    pub fn set_peer(&mut self, peer: SerialPeer) {
        self.peer = peer;
    }

    pub fn queue_external_clock_pulse(&mut self) -> bool {
        if !self.accepts_external_clock_pulse() {
            return false;
        }

        let previous_pending = self.external_clock_pulses_pending;
        self.external_clock_pulses_pending = self.external_clock_pulses_pending.saturating_add(1);
        self.external_clock_pulses_pending != previous_pending
    }

    pub fn take_completed_output_bytes(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.completed_output_bytes)
    }

    pub(crate) fn latest_completed_output_byte(&self) -> Option<u8> {
        self.latest_completed_output_byte
    }

    pub(crate) fn requires_full_t_cycle_tick(&self) -> bool {
        matches!(
            self.transfer_state,
            SerialTransferState::TransferRequested { .. }
        ) || self.external_clock_pulses_pending != 0
    }

    pub(crate) fn external_wait_without_pending_clock(&self) -> bool {
        self.clock_mode == SerialClockMode::External
            && matches!(
                self.transfer_state,
                SerialTransferState::TransferRequested { .. }
            )
            && self.external_clock_pulses_pending == 0
    }

    pub(crate) fn tick_external_wait_t_cycle(&mut self) -> SerialTickTelemetry {
        debug_assert!(self.external_wait_without_pending_clock());

        self.latest_completed_output_byte = None;
        self.clock_counter = self.clock_counter.wrapping_add(1);
        SerialTickTelemetry {
            active_t_cycles: 1,
            external_ticks: 1,
            external_wait_ticks: 1,
            ..Default::default()
        }
    }

    pub(crate) fn tick_idle_t_cycle(&mut self) {
        self.latest_completed_output_byte = None;
        self.clock_counter = self.clock_counter.wrapping_add(1);
    }

    pub(crate) fn internal_clock_edge_pending_this_t_cycle_for_speed(
        &self,
        speed_mode: CgbSpeedMode,
    ) -> bool {
        self.clock_mode == SerialClockMode::Internal
            && matches!(
                self.transfer_state,
                SerialTransferState::TransferRequested { .. }
            )
            && serial_internal_clock_edge(
                self.clock_counter,
                self.clock_counter.wrapping_add(1),
                speed_mode,
                self.cgb_high_speed_clock(),
            )
    }

    pub(crate) fn endpoint_outgoing_byte(&self) -> u8 {
        match self.transfer_state {
            SerialTransferState::TransferRequested { bits_shifted } if bits_shifted != 0 => {
                self.current_outgoing_byte
            }
            _ => self.staged_outgoing_byte,
        }
    }

    pub fn snapshot(&self) -> SerialSnapshot {
        SerialSnapshot {
            console_model: self.console_model,
            operating_mode: self.operating_mode,
            status: self.status,
            sb: self.sb,
            clock_mode: self.clock_mode,
            cgb_high_speed_clock: self.cgb_high_speed_clock(),
            transfer_state: self.transfer_state,
            peer: self.peer,
        }
    }

    #[cfg(test)]
    pub(crate) fn tick_t_cycle(&mut self, context: &mut CycleContext) -> SerialTickTelemetry {
        self.tick_t_cycle_for_speed(context, CgbSpeedMode::Normal)
    }

    pub(crate) fn tick_t_cycle_for_speed(
        &mut self,
        context: &mut CycleContext,
        speed_mode: CgbSpeedMode,
    ) -> SerialTickTelemetry {
        self.latest_completed_output_byte = None;

        let previous_clock_counter = self.clock_counter;
        self.clock_counter = self.clock_counter.wrapping_add(1);
        let SerialTransferState::TransferRequested { .. } = self.transfer_state else {
            return SerialTickTelemetry::default();
        };

        let mut telemetry = SerialTickTelemetry {
            active_t_cycles: 1,
            ..Default::default()
        };
        match self.clock_mode {
            SerialClockMode::Internal => {
                telemetry.internal_ticks = 1;
                if serial_internal_clock_edge(
                    previous_clock_counter,
                    self.clock_counter,
                    speed_mode,
                    self.cgb_high_speed_clock(),
                ) {
                    telemetry.shift_edges = 1;
                    if self.shift_one_bit(context) {
                        telemetry.completed_bytes = 1;
                    }
                }
            }
            SerialClockMode::External => {
                telemetry.external_ticks = 1;
                if self.external_clock_pulses_pending != 0 {
                    self.external_clock_pulses_pending -= 1;
                    telemetry.shift_edges = 1;
                    if self.shift_one_bit(context) {
                        telemetry.completed_bytes = 1;
                    }
                } else {
                    telemetry.external_wait_ticks = 1;
                }
            }
        }
        telemetry
    }

    pub fn scheduler_trace_message(&self, context: &CycleContext) -> String {
        format!(
            "t_cycle={} phase={} console_model={:?} status={:?} sb={:#04X} clock_mode={:?} transfer_state={:?} peer={:?}",
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.status,
            self.sb,
            self.clock_mode,
            self.transfer_state,
            self.peer,
        )
    }

    pub(crate) fn external_event_ingress_trace_message(&self, context: &CycleContext) -> String {
        format!(
            "{} external_clock_pulses_pending={}",
            self.scheduler_trace_message(context),
            self.external_clock_pulses_pending,
        )
    }

    fn accepts_external_clock_pulse(&self) -> bool {
        self.clock_mode == SerialClockMode::External
            && matches!(
                self.transfer_state,
                SerialTransferState::TransferRequested { .. }
            )
    }

    fn cgb_high_speed_serial_enabled(&self) -> bool {
        self.console_model.is_cgb_family() && self.operating_mode.enables_cgb_extensions()
    }

    fn read_sc_high_speed_bit(&self) -> u8 {
        if self.cgb_high_speed_serial_enabled() {
            u8::from(self.cgb_high_speed_clock) * SC_CGB_HIGH_SPEED_BIT
        } else {
            SC_CGB_HIGH_SPEED_BIT
        }
    }

    fn shift_one_bit(&mut self, context: &mut CycleContext) -> bool {
        let SerialTransferState::TransferRequested { bits_shifted } = self.transfer_state else {
            return false;
        };

        if bits_shifted == 0 {
            self.current_outgoing_byte = self.staged_outgoing_byte;
            self.current_outgoing_shift_byte = self.staged_outgoing_byte;
            self.current_incoming_byte = staged_incoming_byte_for_peer(self.peer);
        }

        let outgoing_bit = self.current_outgoing_shift_byte & 0x80 != 0;
        self.current_outgoing_shift_byte <<= 1;
        let incoming_bit =
            incoming_bit_from_peer(self.peer, outgoing_bit, &mut self.current_incoming_byte);
        self.sb = (self.sb << 1) | u8::from(incoming_bit);

        let bits_shifted = bits_shifted + 1;
        if bits_shifted == 8 {
            self.transfer_state = SerialTransferState::Idle;
            self.external_clock_pulses_pending = 0;
            self.completed_output_bytes.push(self.current_outgoing_byte);
            self.latest_completed_output_byte = Some(self.current_outgoing_byte);
            self.current_outgoing_byte = 0;
            self.current_outgoing_shift_byte = 0;
            self.current_incoming_byte = 0;
            context.queue_interrupt_request(InterruptSource::Serial);
            true
        } else {
            self.transfer_state = SerialTransferState::TransferRequested { bits_shifted };
            false
        }
    }
}

const fn staged_incoming_byte_for_peer(peer: SerialPeer) -> u8 {
    match peer {
        SerialPeer::Disconnected | SerialPeer::Loopback => 0,
        SerialPeer::StagedIncomingByte { byte } => byte,
    }
}

fn incoming_bit_from_peer(
    peer: SerialPeer,
    outgoing_bit: bool,
    current_incoming_byte: &mut u8,
) -> bool {
    match peer {
        SerialPeer::Disconnected => true,
        SerialPeer::Loopback => outgoing_bit,
        SerialPeer::StagedIncomingByte { .. } => {
            let incoming_bit = *current_incoming_byte & 0x80 != 0;
            *current_incoming_byte <<= 1;
            incoming_bit
        }
    }
}

const fn serial_internal_clock_edge(
    previous_clock_counter: u16,
    current_clock_counter: u16,
    speed_mode: CgbSpeedMode,
    cgb_high_speed_clock: bool,
) -> bool {
    let edge_mask = 1 << serial_internal_clock_edge_bit(speed_mode, cgb_high_speed_clock);
    previous_clock_counter & edge_mask != 0 && current_clock_counter & edge_mask == 0
}

const fn serial_internal_clock_edge_bit(
    speed_mode: CgbSpeedMode,
    cgb_high_speed_clock: bool,
) -> u8 {
    match (speed_mode, cgb_high_speed_clock) {
        (CgbSpeedMode::Normal, false) => 8,
        (CgbSpeedMode::Double, false) => 7,
        (CgbSpeedMode::Normal, true) => 3,
        (CgbSpeedMode::Double, true) => 2,
    }
}

#[cfg(test)]
mod tests;
