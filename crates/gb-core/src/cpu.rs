use crate::model::ConsoleModel;
use crate::scheduler::InterruptSource;

mod alu;
mod api;
mod bus;
mod cycle;
mod decode;
mod execute;
mod interrupt_control;
mod registers;
mod state;
mod trace;

#[cfg(test)]
use decode::{
    AluOperation, ConditionCode, DecodedOpcode, DirectAddressSource, Register8, Register8Operand,
    Register16, StackRegister16, decode_absolute_jump_condition, decode_alu_operation,
    decode_call_condition, decode_hl_update_direction, decode_register8_operand, decode_register16,
    decode_relative_jump_condition, decode_return_condition, decode_stack_register16,
};
use decode::{CbInstructionKind, CpuInstructionKind, InstructionExecutionGroup};
use trace::CpuTraceBusActivity;

const LAST_MACHINE_CYCLE_T: u8 = 3;

const FLAG_Z: u8 = 0x80;
const FLAG_N: u8 = 0x40;
const FLAG_H: u8 = 0x20;
const FLAG_C: u8 = 0x10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) enum CpuBusOperation {
    Read { address: u16 },
    Write { address: u16, value: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) enum CpuExternalOperation {
    Bus(CpuBusOperation),
    PendingInterruptMask,
    InterruptEnableMask,
    StopWakeLineAsserted,
    AcknowledgeInterrupt { source: InterruptSource },
    RequestInterrupt { source: InterruptSource },
}

type CpuExternalCallback<'a> = dyn FnMut(CpuExternalOperation) -> Option<u8> + 'a;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CpuStatus {
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CpuAddressEventKind {
    Read,
    Write,
    IncDec,
    ReadWithIncDec,
    WriteWithIncDec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CpuAddressUpdateDirection {
    Increment,
    Decrement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CpuBusAccessKind {
    OpcodeFetch,
    OperandRead,
    DataRead,
    DataWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CpuBusActivitySnapshot {
    pub kind: CpuBusAccessKind,
    pub address: u16,
    pub value: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CpuAddressEvent {
    pub kind: CpuAddressEventKind,
    pub access_address: Option<u16>,
    pub idu_address: Option<u16>,
    pub update_direction: Option<CpuAddressUpdateDirection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CpuStartupState {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CpuRegisters {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CpuExecutionState {
    FetchOpcode {
        t_cycle: u8,
    },
    Execute {
        step: u8,
        t_cycle: u8,
    },
    ServiceInterrupt {
        source: InterruptSource,
        step: u8,
        t_cycle: u8,
    },
    ServiceStopWakeBuggedInterrupt {
        step: u8,
        t_cycle: u8,
    },
    DiagnosticTrap {
        trap: CpuDiagnosticTrap,
    },
    Halted,
    ZombieStopped,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CpuDiagnosticTrap {
    InvalidOpcode { opcode: u8, address: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum StopEntryResolution {
    CompleteOnCurrentMachineCycle,
    EnterStoppedAfterPaddingFetch,
    EnterZombieStopped,
    EnterHaltAfterPaddingFetch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
struct InFlightInstruction {
    opcode: Option<u8>,
    kind: Option<CpuInstructionKind>,
    execution_group: Option<InstructionExecutionGroup>,
    cb_instruction_kind: Option<CbInstructionKind>,
    operand8_latch: u8,
    operand16_latch: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
enum ImeState {
    #[default]
    Disabled,
    DisabledPendingEnable {
        instructions_remaining: u8,
    },
    Enabled,
    EnabledPendingEnable {
        instructions_remaining: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct HaltRequestContext {
    ime_enabled: bool,
    had_pending_ei: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
enum HaltControlState {
    #[default]
    Idle,
    PendingRequest(HaltRequestContext),
    HaltBugPending,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CpuCore {
    console_model: ConsoleModel,
    status: CpuStatus,
    startup_state: CpuStartupState,
    registers: CpuRegisters,
    execution_state: CpuExecutionState,
    in_flight: InFlightInstruction,
    ime_state: ImeState,
    halt_control: HaltControlState,
    last_bus_activity: Option<CpuTraceBusActivity>,
    last_address_event: Option<CpuAddressEvent>,
    stop_div_reset_requested: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CpuSaveState {
    console_model: ConsoleModel,
    status: CpuStatus,
    startup_state: CpuStartupState,
    registers: CpuRegisters,
    execution_state: CpuExecutionState,
    in_flight: InFlightInstruction,
    ime_state: ImeState,
    halt_control: HaltControlState,
    last_bus_activity: Option<CpuTraceBusActivity>,
    last_address_event: Option<CpuAddressEvent>,
    stop_div_reset_requested: bool,
}

impl CpuSaveState {
    pub(crate) const fn dynamic_payload_bytes(&self) -> usize {
        0
    }
}

impl CpuCore {
    pub(crate) fn capture_save_state(&self) -> CpuSaveState {
        CpuSaveState {
            console_model: self.console_model,
            status: self.status,
            startup_state: self.startup_state,
            registers: self.registers,
            execution_state: self.execution_state,
            in_flight: self.in_flight,
            ime_state: self.ime_state,
            halt_control: self.halt_control,
            last_bus_activity: self.last_bus_activity,
            last_address_event: self.last_address_event,
            stop_div_reset_requested: self.stop_div_reset_requested,
        }
    }

    pub(crate) fn restore_save_state(&mut self, state: &CpuSaveState) {
        self.console_model = state.console_model;
        self.status = state.status;
        self.startup_state = state.startup_state;
        self.registers = state.registers;
        self.execution_state = state.execution_state;
        self.in_flight = state.in_flight;
        self.ime_state = state.ime_state;
        self.halt_control = state.halt_control;
        self.last_bus_activity = state.last_bus_activity;
        self.last_address_event = state.last_address_event;
        self.stop_div_reset_requested = state.stop_div_reset_requested;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CpuSnapshot {
    pub console_model: ConsoleModel,
    pub status: CpuStatus,
    pub startup_state: CpuStartupState,
    pub registers: CpuRegisters,
    pub execution_state: CpuExecutionState,
    pub current_opcode: Option<u8>,
    pub ime: bool,
    pub delayed_ime_enable: bool,
    pub last_bus_activity: Option<CpuBusActivitySnapshot>,
    pub last_address_event: Option<CpuAddressEvent>,
}

#[cfg(test)]
mod tests;
