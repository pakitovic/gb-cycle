use crate::model::ConsoleModel;
use crate::scheduler::{CycleContext, InterruptSource};

const SC_TRANSFER_REQUEST_BIT: u8 = 0x80;
const SC_FORCED_HIGH_BITS: u8 = 0x7E;
const SC_CLOCK_MODE_BIT: u8 = 0x01;
const DMG_INTERNAL_SERIAL_CLOCK_PERIOD_T_CYCLES: u16 = 512;

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
        }
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
    ticks_until_next_shift: Option<u16>,
    external_clock_pulses_pending: u8,
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
            ticks_until_next_shift: None,
            external_clock_pulses_pending: 0,
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
        self.reset_transfer_timing();
    }

    pub fn apply_startup_state(&mut self, startup_state: SerialStartupState) {
        self.sb = startup_state.sb;
        self.clock_mode = startup_state.clock_mode;
        self.transfer_state = startup_state.transfer_state;
        self.peer = SerialPeer::Disconnected;
        self.external_clock_pulses_pending = 0;
        self.reset_transfer_timing();
    }

    pub fn set_peer(&mut self, peer: SerialPeer) {
        self.peer = peer;
    }

    pub fn queue_external_clock_pulse(&mut self) {
        self.external_clock_pulses_pending = self.external_clock_pulses_pending.saturating_add(1);
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
        let SerialTransferState::TransferRequested { .. } = self.transfer_state else {
            return;
        };

        match self.clock_mode {
            SerialClockMode::Internal => self.advance_internal_clock(context),
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

    fn reset_transfer_timing(&mut self) {
        self.ticks_until_next_shift = match (self.clock_mode, self.transfer_state) {
            (SerialClockMode::Internal, SerialTransferState::TransferRequested { .. }) => {
                Some(DMG_INTERNAL_SERIAL_CLOCK_PERIOD_T_CYCLES)
            }
            _ => None,
        };
    }

    fn advance_internal_clock(&mut self, context: &mut CycleContext) {
        let Some(ticks_until_next_shift) = self.ticks_until_next_shift else {
            self.ticks_until_next_shift = Some(DMG_INTERNAL_SERIAL_CLOCK_PERIOD_T_CYCLES);
            return;
        };

        if ticks_until_next_shift > 1 {
            self.ticks_until_next_shift = Some(ticks_until_next_shift - 1);
            return;
        }

        self.shift_one_bit(context);
        if matches!(
            self.transfer_state,
            SerialTransferState::TransferRequested { .. }
        ) {
            self.ticks_until_next_shift = Some(DMG_INTERNAL_SERIAL_CLOCK_PERIOD_T_CYCLES);
        }
    }

    fn consume_external_clock_if_present(&mut self, context: &mut CycleContext) {
        if self.external_clock_pulses_pending == 0 {
            return;
        }

        self.external_clock_pulses_pending -= 1;
        self.shift_one_bit(context);
    }

    fn shift_one_bit(&mut self, context: &mut CycleContext) {
        let SerialTransferState::TransferRequested { bits_shifted } = self.transfer_state else {
            return;
        };

        let outgoing_bit = self.sb & 0x80 != 0;
        let incoming_bit = incoming_bit_from_peer(self.peer, outgoing_bit);
        self.sb = (self.sb << 1) | u8::from(incoming_bit);

        let bits_shifted = bits_shifted + 1;
        if bits_shifted == 8 {
            self.transfer_state = SerialTransferState::Idle;
            self.ticks_until_next_shift = None;
            context.queue_interrupt_request(InterruptSource::Serial);
        } else {
            self.transfer_state = SerialTransferState::TransferRequested { bits_shifted };
        }
    }
}

const fn incoming_bit_from_peer(peer: SerialPeer, outgoing_bit: bool) -> bool {
    match peer {
        SerialPeer::Disconnected => true,
        SerialPeer::Loopback => outgoing_bit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sc_forces_reserved_bits_high_and_tracks_control_fields() {
        let mut serial = Serial::new(ConsoleModel::Dmg);

        serial.write_sc(0x81);

        assert_eq!(serial.read_sc(), 0xFF);
        assert_eq!(serial.clock_mode, SerialClockMode::Internal);
        assert_eq!(
            serial.transfer_state,
            SerialTransferState::TransferRequested { bits_shifted: 0 }
        );
    }

    #[test]
    fn startup_state_recreates_the_documented_post_boot_sb_and_sc_snapshot() {
        let mut serial = Serial::new(ConsoleModel::Dmg);

        serial.apply_startup_state(SerialStartupState::from_registers(0x00, 0x7E));

        assert_eq!(serial.read_sb(), 0x00);
        assert_eq!(serial.read_sc(), 0x7E);
        assert_eq!(serial.clock_mode, SerialClockMode::External);
        assert_eq!(serial.transfer_state, SerialTransferState::Idle);
    }

    #[test]
    fn startup_state_and_sc_writes_cover_internal_and_external_transfer_modes() {
        let startup_state = SerialStartupState::from_registers(0xA5, 0x81);
        assert_eq!(startup_state.sb, 0xA5);
        assert_eq!(startup_state.clock_mode, SerialClockMode::Internal);
        assert_eq!(
            startup_state.transfer_state,
            SerialTransferState::TransferRequested { bits_shifted: 0 }
        );

        let mut serial = Serial::new(ConsoleModel::Dmg);
        serial.apply_startup_state(startup_state);
        assert_eq!(serial.read_sc(), 0xFF);

        serial.write_sc(0x00);
        assert_eq!(serial.read_sc(), 0x7E);
        assert_eq!(serial.clock_mode, SerialClockMode::External);
        assert_eq!(serial.transfer_state, SerialTransferState::Idle);
    }

    #[test]
    fn scheduler_trace_message_reports_cycle_phase_and_console_model() {
        let serial = Serial::new(ConsoleModel::Dmg);
        let context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);
        let trace = serial.scheduler_trace_message(&context);

        assert_eq!(
            trace,
            "t_cycle=0 phase=external_event_ingress console_model=Dmg status=Ready sb=0x00 clock_mode=External transfer_state=Idle peer=Disconnected"
        );
    }

    #[test]
    fn internal_clock_shifts_sb_bit_by_bit_and_requests_irq_on_completion() {
        let mut serial = Serial::new(ConsoleModel::Dmg);
        let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

        serial.write_sb(0x81);
        serial.write_sc(0x81);

        for _ in 0..511 {
            serial.tick_t_cycle(&mut context);
            assert!(context.interrupt_requests().is_empty());
            assert_eq!(
                serial.transfer_state(),
                SerialTransferState::TransferRequested { bits_shifted: 0 }
            );
        }

        serial.tick_t_cycle(&mut context);
        assert_eq!(serial.read_sb(), 0x03);
        assert_eq!(
            serial.transfer_state(),
            SerialTransferState::TransferRequested { bits_shifted: 1 }
        );
        assert!(context.interrupt_requests().is_empty());

        for _ in 0..(7 * 512) {
            serial.tick_t_cycle(&mut context);
        }

        assert_eq!(serial.read_sb(), 0xFF);
        assert_eq!(serial.read_sc(), 0x7F);
        assert_eq!(serial.transfer_state(), SerialTransferState::Idle);
        assert_eq!(context.interrupt_requests(), &[InterruptSource::Serial]);
    }

    #[test]
    fn slave_mode_waits_for_external_clocks() {
        let mut serial = Serial::new(ConsoleModel::Dmg);
        let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

        serial.write_sb(0xA5);
        serial.write_sc(0x80);

        for _ in 0..2048 {
            serial.tick_t_cycle(&mut context);
        }

        assert_eq!(serial.read_sb(), 0xA5);
        assert_eq!(serial.read_sc(), 0xFE);
        assert_eq!(
            serial.transfer_state(),
            SerialTransferState::TransferRequested { bits_shifted: 0 }
        );
        assert!(context.interrupt_requests().is_empty());
    }

    #[test]
    fn loopback_peer_returns_the_original_byte_after_eight_shifts() {
        let mut serial = Serial::new(ConsoleModel::Dmg);
        let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

        serial.set_peer(SerialPeer::Loopback);
        serial.write_sb(0x96);
        serial.write_sc(0x81);

        for _ in 0..(8 * 512) {
            serial.tick_t_cycle(&mut context);
        }

        assert_eq!(serial.read_sb(), 0x96);
        assert_eq!(serial.transfer_state(), SerialTransferState::Idle);
        assert_eq!(context.interrupt_requests(), &[InterruptSource::Serial]);
    }
}
