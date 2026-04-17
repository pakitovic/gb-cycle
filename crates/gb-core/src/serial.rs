use crate::model::ConsoleModel;
use crate::scheduler::{CycleContext, InterruptSource};

const SC_TRANSFER_REQUEST_BIT: u8 = 0x80;
const SC_FORCED_HIGH_BITS: u8 = 0x7E;
const SC_CLOCK_MODE_BIT: u8 = 0x01;
const DMG_INTERNAL_SERIAL_CLOCK_EDGE_BIT: u8 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerialStatus {
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SerialClockMode {
    #[default]
    External,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SerialPeer {
    #[default]
    Disconnected,
    Loopback,
    StagedIncomingByte {
        byte: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SerialTransferState {
    #[default]
    Idle,
    TransferRequested {
        bits_shifted: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SerialStartupState {
    pub sb: u8,
    pub clock_mode: SerialClockMode,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Serial {
    console_model: ConsoleModel,
    status: SerialStatus,
    sb: u8,
    clock_mode: SerialClockMode,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialSnapshot {
    pub console_model: ConsoleModel,
    pub status: SerialStatus,
    pub sb: u8,
    pub clock_mode: SerialClockMode,
    pub transfer_state: SerialTransferState,
    pub peer: SerialPeer,
}

impl Serial {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            status: SerialStatus::Ready,
            sb: 0,
            clock_mode: SerialClockMode::External,
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

    pub fn status(&self) -> SerialStatus {
        self.status
    }

    pub fn clock_mode(&self) -> SerialClockMode {
        self.clock_mode
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
        SC_FORCED_HIGH_BITS
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

    pub(crate) fn take_latest_completed_output_byte(&mut self) -> Option<u8> {
        self.latest_completed_output_byte.take()
    }

    pub(crate) fn internal_clock_edge_pending_this_t_cycle(&self) -> bool {
        self.clock_mode == SerialClockMode::Internal
            && matches!(
                self.transfer_state,
                SerialTransferState::TransferRequested { .. }
            )
            && serial_internal_clock_edge(self.clock_counter, self.clock_counter.wrapping_add(1))
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
            status: self.status,
            sb: self.sb,
            clock_mode: self.clock_mode,
            transfer_state: self.transfer_state,
            peer: self.peer,
        }
    }

    pub(crate) fn tick_t_cycle(&mut self, context: &mut CycleContext) {
        self.latest_completed_output_byte = None;

        let previous_clock_counter = self.clock_counter;
        self.clock_counter = self.clock_counter.wrapping_add(1);
        let internal_clock_edge =
            serial_internal_clock_edge(previous_clock_counter, self.clock_counter);

        let SerialTransferState::TransferRequested { .. } = self.transfer_state else {
            return;
        };

        match self.clock_mode {
            SerialClockMode::Internal => self.advance_internal_clock(context, internal_clock_edge),
            SerialClockMode::External => self.consume_external_clock_if_present(context),
        }
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

    fn advance_internal_clock(&mut self, context: &mut CycleContext, internal_clock_edge: bool) {
        if !internal_clock_edge {
            return;
        }

        self.shift_one_bit(context);
    }

    fn consume_external_clock_if_present(&mut self, context: &mut CycleContext) {
        if self.external_clock_pulses_pending == 0 {
            return;
        }

        self.external_clock_pulses_pending -= 1;
        self.shift_one_bit(context);
    }

    fn accepts_external_clock_pulse(&self) -> bool {
        self.clock_mode == SerialClockMode::External
            && matches!(
                self.transfer_state,
                SerialTransferState::TransferRequested { .. }
            )
    }

    fn shift_one_bit(&mut self, context: &mut CycleContext) {
        let SerialTransferState::TransferRequested { bits_shifted } = self.transfer_state else {
            return;
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
        } else {
            self.transfer_state = SerialTransferState::TransferRequested { bits_shifted };
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
) -> bool {
    previous_clock_counter & (1 << DMG_INTERNAL_SERIAL_CLOCK_EDGE_BIT) != 0
        && current_clock_counter & (1 << DMG_INTERNAL_SERIAL_CLOCK_EDGE_BIT) == 0
}

#[cfg(test)]
mod tests;
