use crate::model::ConsoleModel;
use crate::scheduler::CycleContext;

const SC_TRANSFER_REQUEST_BIT: u8 = 0x80;
const SC_FORCED_HIGH_BITS: u8 = 0x7E;
const SC_CLOCK_MODE_BIT: u8 = 0x01;

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
pub enum SerialTransferState {
    #[default]
    Idle,
    TransferRequested,
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
                SerialTransferState::TransferRequested
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialSnapshot {
    pub console_model: ConsoleModel,
    pub status: SerialStatus,
    pub sb: u8,
    pub clock_mode: SerialClockMode,
    pub transfer_state: SerialTransferState,
}

impl Serial {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            status: SerialStatus::Ready,
            sb: 0,
            clock_mode: SerialClockMode::External,
            transfer_state: SerialTransferState::Idle,
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn status(&self) -> SerialStatus {
        self.status
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
                SerialTransferState::TransferRequested => SC_TRANSFER_REQUEST_BIT,
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
            SerialTransferState::TransferRequested
        } else {
            SerialTransferState::Idle
        };
    }

    pub fn apply_startup_state(&mut self, startup_state: SerialStartupState) {
        self.sb = startup_state.sb;
        self.clock_mode = startup_state.clock_mode;
        self.transfer_state = startup_state.transfer_state;
    }

    pub fn snapshot(&self) -> SerialSnapshot {
        SerialSnapshot {
            console_model: self.console_model,
            status: self.status,
            sb: self.sb,
            clock_mode: self.clock_mode,
            transfer_state: self.transfer_state,
        }
    }

    pub fn scheduler_trace_message(&self, context: &CycleContext) -> String {
        format!(
            "t_cycle={} phase={} console_model={:?} status={:?}",
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.status,
        )
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
            SerialTransferState::TransferRequested
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
            SerialTransferState::TransferRequested
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
            "t_cycle=0 phase=external_event_ingress console_model=Dmg status=Ready"
        );
    }
}
