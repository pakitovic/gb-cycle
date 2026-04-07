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
    AluOperation, ConditionCode, DecodedOpcode, MemoryAddressSource, Register8, Register8Operand,
    Register16, StackRegister16, decode_absolute_jump_condition, decode_alu_operation,
    decode_call_condition, decode_hl_update_direction, decode_register8_operand, decode_register16,
    decode_relative_jump_condition, decode_return_condition, decode_stack_register16,
};
use decode::{CbInstructionKind, CpuInstructionKind};
#[cfg(test)]
use state::{highest_pending_interrupt_from_mask, interrupt_vector};
use trace::CpuTraceBusActivity;

const LAST_MACHINE_CYCLE_T: u8 = 3;

const FLAG_Z: u8 = 0x80;
const FLAG_N: u8 = 0x40;
const FLAG_H: u8 = 0x20;
const FLAG_C: u8 = 0x10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CpuBusOperation {
    Read { address: u16 },
    Write { address: u16, value: u8 },
    PendingInterruptMask,
    InterruptEnableMask,
    StopWakeLineAsserted,
    AcknowledgeInterrupt { source: InterruptSource },
    RequestInterrupt { source: InterruptSource },
}

type CpuBusCallback<'a> = dyn FnMut(CpuBusOperation) -> Option<u8> + 'a;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuStatus {
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CpuAddressEventKind {
    Read,
    Write,
    IncDec,
    ReadWithIncDec,
    WriteWithIncDec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CpuAddressUpdateDirection {
    Increment,
    Decrement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CpuBusAccessKind {
    OpcodeFetch,
    OperandRead,
    DataRead,
    DataWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CpuBusActivitySnapshot {
    pub kind: CpuBusAccessKind,
    pub address: u16,
    pub value: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CpuAddressEvent {
    pub kind: CpuAddressEventKind,
    pub access_address: Option<u16>,
    pub idu_address: Option<u16>,
    pub update_direction: Option<CpuAddressUpdateDirection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CpuExecutionState {
    FetchOpcode {
        t_cycle: u8,
    },
    Execute {
        opcode: u8,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CpuDiagnosticTrap {
    InvalidOpcode { opcode: u8, address: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuCore {
    console_model: ConsoleModel,
    status: CpuStatus,
    startup_state: CpuStartupState,
    registers: CpuRegisters,
    execution_state: CpuExecutionState,
    current_opcode: Option<u8>,
    ime: bool,
    delayed_ime_enable: bool,
    delayed_ime_enable_steps: u8,
    halt_request_pending: bool,
    halt_request_ime: bool,
    halt_request_had_delayed_ei: bool,
    halt_bug_pending: bool,
    instruction_kind: Option<CpuInstructionKind>,
    cb_instruction_kind: Option<CbInstructionKind>,
    operand8_latch: u8,
    operand16_latch: u16,
    last_bus_activity: Option<CpuTraceBusActivity>,
    last_address_event: Option<CpuAddressEvent>,
    stop_div_reset_requested: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
