use crate::interrupts::InterruptController;
use crate::joypad::Joypad;
use crate::model::ConsoleModel;
use crate::scheduler::{CycleContext, InterruptSource};

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
}

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
    DiagnosticTrap {
        trap: CpuDiagnosticTrap,
    },
    Halted,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CpuDiagnosticTrap {
    UnsupportedOpcode { opcode: u8, address: u16 },
    UnsupportedCbOpcode { opcode: u8, address: u16 },
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CpuTraceBusAccessKind {
    OpcodeFetch,
    OperandRead,
    DataRead,
    DataWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CpuTraceBusActivity {
    kind: CpuTraceBusAccessKind,
    address: u16,
    value: u8,
}

impl CpuTraceBusAccessKind {
    const fn trace_label(self) -> &'static str {
        match self {
            Self::OpcodeFetch => "opcode_fetch",
            Self::OperandRead => "operand_read",
            Self::DataRead => "data_read",
            Self::DataWrite => "data_write",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Register8 {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Register16 {
    BC,
    DE,
    HL,
    SP,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum AluOperation {
    Add,
    Adc,
    Sub,
    Sbc,
    And,
    Xor,
    Or,
    Compare,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum StackRegister16 {
    BC,
    DE,
    HL,
    AF,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ConditionCode {
    Nz,
    Z,
    Nc,
    C,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Register8Operand {
    Register(Register8),
    IndirectHl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum MemoryAddressSource {
    BC,
    DE,
    Immediate16,
    HighImmediate8,
    HighC,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CbInstructionKind {
    RotateLeftCarry { target: Register8Operand },
    RotateRightCarry { target: Register8Operand },
    RotateLeftThroughCarry { target: Register8Operand },
    RotateRightThroughCarry { target: Register8Operand },
    ShiftLeftArithmetic { target: Register8Operand },
    ShiftRightArithmetic { target: Register8Operand },
    SwapNibbles { target: Register8Operand },
    ShiftRightLogical { target: Register8Operand },
    BitTest { bit: u8, target: Register8Operand },
    ResetBit { bit: u8, target: Register8Operand },
    SetBit { bit: u8, target: Register8Operand },
}

impl CbInstructionKind {
    fn target(self) -> Register8Operand {
        match self {
            Self::RotateLeftCarry { target }
            | Self::RotateRightCarry { target }
            | Self::RotateLeftThroughCarry { target }
            | Self::RotateRightThroughCarry { target }
            | Self::ShiftLeftArithmetic { target }
            | Self::ShiftRightArithmetic { target }
            | Self::SwapNibbles { target }
            | Self::ShiftRightLogical { target }
            | Self::BitTest { target, .. }
            | Self::ResetBit { target, .. }
            | Self::SetBit { target, .. } => target,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CpuInstructionKind {
    LoadRegisterImmediate {
        target: Register8,
    },
    LoadRegisterPairImmediate {
        target: Register16,
    },
    LoadRegisterFromHl {
        target: Register8,
    },
    StoreRegisterToHl {
        source: Register8,
    },
    StoreImmediateToHl,
    LoadAFromHlWithUpdate {
        direction: CpuAddressUpdateDirection,
    },
    StoreAToHlWithUpdate {
        direction: CpuAddressUpdateDirection,
    },
    LoadAFromAddress {
        source: MemoryAddressSource,
    },
    StoreAToAddress {
        destination: MemoryAddressSource,
    },
    StoreSpToImmediate16,
    LoadHlFromSpPlusImmediate,
    AddSpImmediate,
    LoadSpFromHl,
    AddHl {
        source: Register16,
    },
    IncrementRegisterPair {
        target: Register16,
    },
    DecrementRegisterPair {
        target: Register16,
    },
    IncrementHlMemory,
    DecrementHlMemory,
    AluImmediate {
        operation: AluOperation,
    },
    AluFromHl {
        operation: AluOperation,
    },
    RelativeJump {
        condition: Option<ConditionCode>,
    },
    AbsoluteJump {
        condition: Option<ConditionCode>,
    },
    Call {
        condition: Option<ConditionCode>,
    },
    Return {
        condition: Option<ConditionCode>,
    },
    ReturnFromInterrupt,
    Stop,
    Restart {
        vector: u16,
    },
    PushRegisterPair {
        source: StackRegister16,
    },
    PopRegisterPair {
        target: StackRegister16,
    },
    CbPrefixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum DecodedOpcode {
    Complete,
    Execute(CpuInstructionKind),
    Unsupported,
}

impl CpuCore {
    pub fn new(console_model: ConsoleModel) -> Self {
        let startup_state = CpuStartupState::power_on_reset();

        Self {
            console_model,
            status: CpuStatus::Ready,
            startup_state,
            registers: CpuRegisters::from_startup_state(startup_state),
            execution_state: CpuExecutionState::fetch_opcode(),
            current_opcode: None,
            ime: false,
            delayed_ime_enable: false,
            delayed_ime_enable_steps: 0,
            halt_request_pending: false,
            halt_request_ime: false,
            halt_request_had_delayed_ei: false,
            halt_bug_pending: false,
            instruction_kind: None,
            cb_instruction_kind: None,
            operand8_latch: 0,
            operand16_latch: 0,
            last_bus_activity: None,
            last_address_event: None,
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn status(&self) -> CpuStatus {
        self.status
    }

    pub fn startup_state(&self) -> CpuStartupState {
        self.startup_state
    }

    pub fn registers(&self) -> CpuRegisters {
        self.registers
    }

    pub fn execution_state(&self) -> CpuExecutionState {
        self.execution_state
    }

    pub fn current_opcode(&self) -> Option<u8> {
        self.current_opcode
    }

    pub fn ime(&self) -> bool {
        self.ime
    }

    pub fn delayed_ime_enable(&self) -> bool {
        self.delayed_ime_enable
    }

    pub fn last_address_event(&self) -> Option<CpuAddressEvent> {
        self.last_address_event
    }

    pub fn apply_startup_state(&mut self, startup_state: CpuStartupState) {
        self.startup_state = startup_state;
        self.registers = CpuRegisters::from_startup_state(startup_state);
        self.execution_state = CpuExecutionState::fetch_opcode();
        self.current_opcode = None;
        self.ime = false;
        self.delayed_ime_enable = false;
        self.delayed_ime_enable_steps = 0;
        self.halt_request_pending = false;
        self.halt_request_ime = false;
        self.halt_request_had_delayed_ei = false;
        self.halt_bug_pending = false;
        self.instruction_kind = None;
        self.cb_instruction_kind = None;
        self.operand8_latch = 0;
        self.operand16_latch = 0;
        self.last_bus_activity = None;
        self.last_address_event = None;
    }

    pub(crate) fn tick_t_cycle<F>(&mut self, mut bus_operation: F) -> Option<InterruptSource>
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        self.last_bus_activity = None;
        self.last_address_event = None;

        match self.execution_state {
            CpuExecutionState::FetchOpcode { t_cycle } => {
                if t_cycle < LAST_MACHINE_CYCLE_T {
                    self.execution_state = CpuExecutionState::FetchOpcode {
                        t_cycle: t_cycle + 1,
                    };
                    return None;
                }

                self.complete_fetch_opcode(&mut bus_operation);
            }
            CpuExecutionState::Execute {
                opcode,
                step,
                t_cycle,
            } => {
                if t_cycle < LAST_MACHINE_CYCLE_T {
                    self.execution_state = CpuExecutionState::Execute {
                        opcode,
                        step,
                        t_cycle: t_cycle + 1,
                    };
                    return None;
                }

                self.complete_execute_machine_cycle(opcode, step, &mut bus_operation);
            }
            CpuExecutionState::ServiceInterrupt {
                source,
                step,
                t_cycle,
            } => {
                if t_cycle < LAST_MACHINE_CYCLE_T {
                    self.execution_state = CpuExecutionState::ServiceInterrupt {
                        source,
                        step,
                        t_cycle: t_cycle + 1,
                    };
                    return None;
                }

                return self.complete_interrupt_service_machine_cycle(
                    source,
                    step,
                    &mut bus_operation,
                );
            }
            CpuExecutionState::DiagnosticTrap { .. }
            | CpuExecutionState::Halted
            | CpuExecutionState::Stopped => {}
        }

        None
    }

    pub(crate) fn evaluate_wake_and_interrupts(
        &mut self,
        interrupts: &mut InterruptController,
        joypad: &mut Joypad,
    ) {
        if matches!(
            self.execution_state,
            CpuExecutionState::DiagnosticTrap { .. }
        ) {
            return;
        }

        if matches!(self.execution_state, CpuExecutionState::Stopped) {
            if joypad.consume_stop_wake_event() {
                self.execution_state = CpuExecutionState::fetch_opcode();
            }
            return;
        }

        let pending = interrupts.pending_mask() != 0;

        if self.halt_request_pending {
            self.halt_request_pending = false;
            let halt_request_ime = self.halt_request_ime;
            let halt_request_had_delayed_ei = self.halt_request_had_delayed_ei;
            self.halt_request_ime = false;
            self.halt_request_had_delayed_ei = false;

            if !halt_request_ime && pending {
                if halt_request_had_delayed_ei && self.ime {
                    self.registers.pc = self.registers.pc.wrapping_sub(1);
                    self.accept_pending_interrupt(interrupts);
                } else {
                    self.halt_bug_pending = true;
                    self.execution_state = CpuExecutionState::fetch_opcode();
                }
            } else if pending {
                self.accept_pending_interrupt(interrupts);
            } else {
                self.execution_state = CpuExecutionState::Halted;
            }
            return;
        }

        if matches!(self.execution_state, CpuExecutionState::Halted) {
            if !pending {
                return;
            }

            if self.ime {
                self.accept_pending_interrupt(interrupts);
            } else {
                self.execution_state = CpuExecutionState::fetch_opcode();
            }
            return;
        }

        if !self.ime || !self.can_accept_interrupt() {
            return;
        }

        self.accept_pending_interrupt(interrupts);
    }

    pub fn snapshot(&self) -> CpuSnapshot {
        CpuSnapshot {
            console_model: self.console_model,
            status: self.status,
            startup_state: self.startup_state,
            registers: self.registers,
            execution_state: self.execution_state,
            current_opcode: self.current_opcode,
            ime: self.ime,
            delayed_ime_enable: self.delayed_ime_enable,
        }
    }

    pub fn scheduler_trace_message(&self, context: &CycleContext) -> String {
        format!(
            "t_cycle={} phase={} console_model={:?} status={:?} pc={:#06X} execution_state={:?} current_opcode={:?} ime={} delayed_ime_enable={} last_bus_activity={} last_address_event={}",
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.status,
            self.registers.pc,
            self.execution_state,
            self.current_opcode,
            self.ime,
            self.delayed_ime_enable,
            self.last_bus_activity_trace_value(),
            self.last_address_event_trace_value(),
        )
    }

    fn complete_fetch_opcode<F>(&mut self, bus_operation: &mut F)
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        let opcode = self.read_opcode_u8(bus_operation);
        self.current_opcode = Some(opcode);

        match self.decode_fetched_opcode(opcode) {
            DecodedOpcode::Complete => self.finish_instruction(),
            DecodedOpcode::Execute(kind) => self.begin_instruction(opcode, kind),
            DecodedOpcode::Unsupported => self.enter_unsupported_opcode_trap(opcode),
        }
    }

    fn complete_execute_machine_cycle<F>(&mut self, opcode: u8, step: u8, bus_operation: &mut F)
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        let Some(kind) = self.instruction_kind else {
            self.execution_state = CpuExecutionState::Execute {
                opcode,
                step,
                t_cycle: LAST_MACHINE_CYCLE_T,
            };
            return;
        };

        match kind {
            CpuInstructionKind::LoadRegisterImmediate { target } => match step {
                0 => {
                    let value = self.read_pc_u8(bus_operation);
                    self.write_register8(target, value);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::LoadRegisterPairImmediate { target } => match step {
                0 => {
                    self.operand16_latch = u16::from(self.read_pc_u8(bus_operation));
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let high = self.read_pc_u8(bus_operation);
                    let value = self.operand16_latch | (u16::from(high) << 8);
                    self.write_register16(target, value);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::LoadRegisterFromHl { target } => match step {
                0 => {
                    let value = self.read_byte(self.hl(), bus_operation);
                    self.write_register8(target, value);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::StoreRegisterToHl { source } => match step {
                0 => {
                    self.write_byte(self.hl(), self.read_register8(source), bus_operation);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::StoreImmediateToHl => match step {
                0 => {
                    self.operand8_latch = self.read_pc_u8(bus_operation);
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    self.write_byte(self.hl(), self.operand8_latch, bus_operation);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::LoadAFromHlWithUpdate { direction } => match step {
                0 => {
                    let value = self.read_hl_with_update(direction, bus_operation);
                    self.registers.a = value;
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::StoreAToHlWithUpdate { direction } => match step {
                0 => {
                    self.write_hl_with_update(self.registers.a, direction, bus_operation);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::LoadAFromAddress { source } => match (source, step) {
                (
                    MemoryAddressSource::BC | MemoryAddressSource::DE | MemoryAddressSource::HighC,
                    0,
                ) => {
                    let value = self.read_byte(self.resolve_memory_address(source), bus_operation);
                    self.registers.a = value;
                    self.finish_instruction();
                }
                (MemoryAddressSource::Immediate16, 0) => {
                    self.operand16_latch = u16::from(self.read_pc_u8(bus_operation));
                    self.advance_instruction(opcode, 1);
                }
                (MemoryAddressSource::HighImmediate8, 0) => {
                    self.operand8_latch = self.read_pc_u8(bus_operation);
                    self.advance_instruction(opcode, 1);
                }
                (MemoryAddressSource::Immediate16, 1) => {
                    let high = self.read_pc_u8(bus_operation);
                    self.operand16_latch |= u16::from(high) << 8;
                    self.advance_instruction(opcode, 2);
                }
                (MemoryAddressSource::HighImmediate8, 1) => {
                    let value = self.read_byte(self.resolve_memory_address(source), bus_operation);
                    self.registers.a = value;
                    self.finish_instruction();
                }
                (MemoryAddressSource::Immediate16, 2) => {
                    let value = self.read_byte(self.operand16_latch, bus_operation);
                    self.registers.a = value;
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::StoreAToAddress { destination } => match (destination, step) {
                (
                    MemoryAddressSource::BC | MemoryAddressSource::DE | MemoryAddressSource::HighC,
                    0,
                ) => {
                    self.write_byte(
                        self.resolve_memory_address(destination),
                        self.registers.a,
                        bus_operation,
                    );
                    self.finish_instruction();
                }
                (MemoryAddressSource::Immediate16, 0) => {
                    self.operand16_latch = u16::from(self.read_pc_u8(bus_operation));
                    self.advance_instruction(opcode, 1);
                }
                (MemoryAddressSource::HighImmediate8, 0) => {
                    self.operand8_latch = self.read_pc_u8(bus_operation);
                    self.advance_instruction(opcode, 1);
                }
                (MemoryAddressSource::Immediate16, 1) => {
                    let high = self.read_pc_u8(bus_operation);
                    self.operand16_latch |= u16::from(high) << 8;
                    self.advance_instruction(opcode, 2);
                }
                (MemoryAddressSource::HighImmediate8, 1) => {
                    self.write_byte(
                        self.resolve_memory_address(destination),
                        self.registers.a,
                        bus_operation,
                    );
                    self.finish_instruction();
                }
                (MemoryAddressSource::Immediate16, 2) => {
                    self.write_byte(self.operand16_latch, self.registers.a, bus_operation);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::StoreSpToImmediate16 => match step {
                0 => {
                    self.operand16_latch = u16::from(self.read_pc_u8(bus_operation));
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let high = self.read_pc_u8(bus_operation);
                    self.operand16_latch |= u16::from(high) << 8;
                    self.advance_instruction(opcode, 2);
                }
                2 => {
                    let [low, _high] = self.registers.sp.to_le_bytes();
                    self.write_byte(self.operand16_latch, low, bus_operation);
                    self.advance_instruction(opcode, 3);
                }
                3 => {
                    let [_low, high] = self.registers.sp.to_le_bytes();
                    self.write_byte(self.operand16_latch.wrapping_add(1), high, bus_operation);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::LoadHlFromSpPlusImmediate => match step {
                0 => {
                    self.operand8_latch = self.read_pc_u8(bus_operation);
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let result = self.sp_plus_signed_immediate(self.operand8_latch);
                    self.write_register16(Register16::HL, result);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::AddSpImmediate => match step {
                0 => {
                    self.operand8_latch = self.read_pc_u8(bus_operation);
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    self.advance_instruction(opcode, 2);
                }
                2 => {
                    let result = self.sp_plus_signed_immediate(self.operand8_latch);
                    self.write_register16(Register16::SP, result);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::LoadSpFromHl => match step {
                0 => {
                    self.write_register16(Register16::SP, self.hl());
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::AddHl { source } => match step {
                0 => {
                    let value = match source {
                        Register16::BC => self.bc(),
                        Register16::DE => self.de(),
                        Register16::HL => self.hl(),
                        Register16::SP => self.registers.sp,
                    };
                    self.add_to_hl(value);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::IncrementRegisterPair { target } => match step {
                0 => {
                    self.increment_or_decrement_register_pair(
                        target,
                        CpuAddressUpdateDirection::Increment,
                    );
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::DecrementRegisterPair { target } => match step {
                0 => {
                    self.increment_or_decrement_register_pair(
                        target,
                        CpuAddressUpdateDirection::Decrement,
                    );
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::IncrementHlMemory => match step {
                0 => {
                    self.operand8_latch = self.read_byte(self.hl(), bus_operation);
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let before = self.operand8_latch;
                    let result = before.wrapping_add(1);
                    self.operand8_latch = result;
                    self.write_byte(self.hl(), result, bus_operation);
                    self.update_inc_flags(before, result);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::DecrementHlMemory => match step {
                0 => {
                    self.operand8_latch = self.read_byte(self.hl(), bus_operation);
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let before = self.operand8_latch;
                    let result = before.wrapping_sub(1);
                    self.operand8_latch = result;
                    self.write_byte(self.hl(), result, bus_operation);
                    self.update_dec_flags(before, result);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::AluImmediate { operation } => match step {
                0 => {
                    let value = self.read_pc_u8(bus_operation);
                    self.apply_alu_operation(operation, value);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::AluFromHl { operation } => match step {
                0 => {
                    let value = self.read_byte(self.hl(), bus_operation);
                    self.apply_alu_operation(operation, value);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::RelativeJump { condition } => match step {
                0 => {
                    self.operand8_latch = self.read_pc_u8(bus_operation);
                    if self.condition_is_met(condition) {
                        self.advance_instruction(opcode, 1);
                    } else {
                        self.finish_instruction();
                    }
                }
                1 => {
                    self.registers.pc = self
                        .registers
                        .pc
                        .wrapping_add_signed(i16::from(self.operand8_latch as i8));
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::AbsoluteJump { condition } => match step {
                0 => {
                    self.operand16_latch = u16::from(self.read_pc_u8(bus_operation));
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let high = self.read_pc_u8(bus_operation);
                    self.operand16_latch |= u16::from(high) << 8;
                    if self.condition_is_met(condition) {
                        self.advance_instruction(opcode, 2);
                    } else {
                        self.finish_instruction();
                    }
                }
                2 => {
                    self.registers.pc = self.operand16_latch;
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::Call { condition } => match step {
                0 => {
                    self.operand16_latch = u16::from(self.read_pc_u8(bus_operation));
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let high = self.read_pc_u8(bus_operation);
                    self.operand16_latch |= u16::from(high) << 8;
                    if self.condition_is_met(condition) {
                        self.advance_instruction(opcode, 2);
                    } else {
                        self.finish_instruction();
                    }
                }
                2 => {
                    self.decrement_sp_and_record_idu_event();
                    self.advance_instruction(opcode, 3);
                }
                3 => {
                    let [low, high] = self.registers.pc.to_le_bytes();
                    self.write_byte_at_sp(high, bus_operation);
                    self.operand8_latch = low;
                    self.advance_instruction(opcode, 4);
                }
                4 => {
                    self.write_byte_with_decremented_sp(self.operand8_latch, bus_operation);
                    self.registers.pc = self.operand16_latch;
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::Return { condition } => match step {
                0 => {
                    if condition.is_some() {
                        if self.condition_is_met(condition) {
                            self.advance_instruction(opcode, 1);
                        } else {
                            self.finish_instruction();
                        }
                    } else {
                        let low = self.read_byte_and_increment_sp(bus_operation);
                        self.operand16_latch = u16::from(low);
                        self.advance_instruction(opcode, 1);
                    }
                }
                1 => {
                    if condition.is_some() {
                        let low = self.read_byte_and_increment_sp(bus_operation);
                        self.operand16_latch = u16::from(low);
                        self.advance_instruction(opcode, 2);
                    } else {
                        let high = self.read_byte_and_increment_sp(bus_operation);
                        self.operand16_latch |= u16::from(high) << 8;
                        self.advance_instruction(opcode, 2);
                    }
                }
                2 => {
                    if condition.is_some() {
                        let high = self.read_byte_and_increment_sp(bus_operation);
                        self.operand16_latch |= u16::from(high) << 8;
                        self.advance_instruction(opcode, 3);
                    } else {
                        self.registers.pc = self.operand16_latch;
                        self.finish_instruction();
                    }
                }
                3 => {
                    self.registers.pc = self.operand16_latch;
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::ReturnFromInterrupt => match step {
                0 => {
                    let low = self.read_byte_and_increment_sp(bus_operation);
                    self.operand16_latch = u16::from(low);
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let high = self.read_byte_and_increment_sp(bus_operation);
                    self.operand16_latch |= u16::from(high) << 8;
                    self.advance_instruction(opcode, 2);
                }
                2 => {
                    self.registers.pc = self.operand16_latch;
                    self.ime = true;
                    self.cancel_delayed_ime_enable();
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::Stop => match step {
                0 => {
                    let _ = self.read_pc_u8(bus_operation);
                    self.enter_stopped_state();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::Restart { vector } => match step {
                0 => {
                    self.decrement_sp_and_record_idu_event();
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let [low, high] = self.registers.pc.to_le_bytes();
                    self.write_byte_at_sp(high, bus_operation);
                    self.operand8_latch = low;
                    self.advance_instruction(opcode, 2);
                }
                2 => {
                    self.write_byte_with_decremented_sp(self.operand8_latch, bus_operation);
                    self.registers.pc = vector;
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::PushRegisterPair { source } => match step {
                0 => {
                    self.operand16_latch = self.read_stack_register16(source);
                    self.decrement_sp_and_record_idu_event();
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let [low, high] = self.operand16_latch.to_le_bytes();
                    self.write_byte_at_sp(high, bus_operation);
                    self.operand8_latch = low;
                    self.advance_instruction(opcode, 2);
                }
                2 => {
                    self.write_byte_with_decremented_sp(self.operand8_latch, bus_operation);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::PopRegisterPair { target } => match step {
                0 => {
                    let low = self.read_byte_and_increment_sp(bus_operation);
                    self.operand16_latch = u16::from(low);
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let high = self.read_byte_and_increment_sp(bus_operation);
                    let value = self.operand16_latch | (u16::from(high) << 8);
                    self.write_stack_register16(target, value);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::CbPrefixed => match step {
                0 => {
                    let cb_opcode = self.read_pc_u8(bus_operation);
                    self.operand8_latch = cb_opcode;

                    match self.decode_cb_opcode(cb_opcode) {
                        Some(kind) => match kind.target() {
                            Register8Operand::Register(target) => {
                                let value = self.read_register8(target);
                                if let Some(result) = self.apply_cb_operation(kind, value) {
                                    self.write_register8(target, result);
                                }
                                self.finish_instruction();
                            }
                            Register8Operand::IndirectHl => {
                                self.cb_instruction_kind = Some(kind);
                                self.advance_instruction(opcode, 1);
                            }
                        },
                        None => {
                            self.execution_state = CpuExecutionState::DiagnosticTrap {
                                trap: CpuDiagnosticTrap::UnsupportedCbOpcode {
                                    opcode: cb_opcode,
                                    address: self.registers.pc.wrapping_sub(1),
                                },
                            };
                        }
                    }
                }
                1 => match self.cb_instruction_kind {
                    Some(kind) if kind.target() == Register8Operand::IndirectHl => {
                        let value = self.read_byte(self.hl(), bus_operation);
                        if let Some(result) = self.apply_cb_operation(kind, value) {
                            self.operand8_latch = result;
                            self.advance_instruction(opcode, 2);
                        } else {
                            self.finish_instruction();
                        }
                    }
                    _ => self.stall_instruction(opcode, step),
                },
                2 => match self.cb_instruction_kind {
                    Some(kind)
                        if kind.target() == Register8Operand::IndirectHl
                            && !matches!(kind, CbInstructionKind::BitTest { .. }) =>
                    {
                        self.write_byte(self.hl(), self.operand8_latch, bus_operation);
                        self.finish_instruction();
                    }
                    _ => self.stall_instruction(opcode, step),
                },
                _ => self.stall_instruction(opcode, step),
            },
        }
    }

    fn decode_fetched_opcode(&mut self, opcode: u8) -> DecodedOpcode {
        if opcode == 0x00 {
            return DecodedOpcode::Complete;
        }

        if opcode == 0xAF {
            self.registers.a = 0;
            self.write_flags(true, false, false, false);
            return DecodedOpcode::Complete;
        }

        if opcode == 0x27 {
            self.decimal_adjust_a();
            return DecodedOpcode::Complete;
        }

        if opcode == 0x2F {
            self.complement_a();
            return DecodedOpcode::Complete;
        }

        if opcode == 0x37 {
            self.set_carry_flag();
            return DecodedOpcode::Complete;
        }

        if opcode == 0x3F {
            self.complement_carry_flag();
            return DecodedOpcode::Complete;
        }

        if opcode == 0x07 {
            let result = self.rotate_left_carry(self.registers.a);
            self.registers.a = result;
            self.write_flags(false, false, false, self.registers.f & FLAG_C != 0);
            return DecodedOpcode::Complete;
        }

        if opcode == 0x17 {
            let result = self.rotate_left_through_carry(self.registers.a);
            self.registers.a = result;
            self.write_flags(false, false, false, self.registers.f & FLAG_C != 0);
            return DecodedOpcode::Complete;
        }

        if opcode == 0x0F {
            let result = self.rotate_right_carry(self.registers.a);
            self.registers.a = result;
            self.write_flags(false, false, false, self.registers.f & FLAG_C != 0);
            return DecodedOpcode::Complete;
        }

        if opcode == 0x1F {
            let result = self.rotate_right_through_carry(self.registers.a);
            self.registers.a = result;
            self.write_flags(false, false, false, self.registers.f & FLAG_C != 0);
            return DecodedOpcode::Complete;
        }

        if opcode == 0x76 {
            self.finish_and_request_halt();
            return DecodedOpcode::Complete;
        }

        if opcode == 0xF3 {
            self.ime = false;
            self.cancel_delayed_ime_enable();
            return DecodedOpcode::Complete;
        }

        if opcode == 0xFB {
            self.schedule_delayed_ime_enable();
            return DecodedOpcode::Complete;
        }

        if opcode == 0x10 {
            return DecodedOpcode::Execute(CpuInstructionKind::Stop);
        }

        if matches!(opcode, 0x01 | 0x11 | 0x21 | 0x31) {
            return DecodedOpcode::Execute(CpuInstructionKind::LoadRegisterPairImmediate {
                target: decode_register16((opcode >> 4) & 0x03),
            });
        }

        if opcode & 0b1100_0111 == 0b0000_0110 {
            return match decode_register8_operand((opcode >> 3) & 0x07) {
                Register8Operand::Register(target) => {
                    DecodedOpcode::Execute(CpuInstructionKind::LoadRegisterImmediate { target })
                }
                Register8Operand::IndirectHl => {
                    DecodedOpcode::Execute(CpuInstructionKind::StoreImmediateToHl)
                }
            };
        }

        if (0x40..=0x7F).contains(&opcode) && opcode != 0x76 {
            let destination = decode_register8_operand((opcode >> 3) & 0x07);
            let source = decode_register8_operand(opcode & 0x07);

            return match (destination, source) {
                (Register8Operand::Register(destination), Register8Operand::Register(source)) => {
                    let value = self.read_register8(source);
                    self.write_register8(destination, value);
                    DecodedOpcode::Complete
                }
                (Register8Operand::Register(target), Register8Operand::IndirectHl) => {
                    DecodedOpcode::Execute(CpuInstructionKind::LoadRegisterFromHl { target })
                }
                (Register8Operand::IndirectHl, Register8Operand::Register(source)) => {
                    DecodedOpcode::Execute(CpuInstructionKind::StoreRegisterToHl { source })
                }
                (Register8Operand::IndirectHl, Register8Operand::IndirectHl) => {
                    DecodedOpcode::Unsupported
                }
            };
        }

        if matches!(opcode, 0x02 | 0x12 | 0xEA) {
            return DecodedOpcode::Execute(CpuInstructionKind::StoreAToAddress {
                destination: match opcode {
                    0x02 => MemoryAddressSource::BC,
                    0x12 => MemoryAddressSource::DE,
                    0xEA => MemoryAddressSource::Immediate16,
                    _ => unreachable!("opcode filter already constrained"),
                },
            });
        }

        if matches!(opcode, 0xE0 | 0xE2) {
            return DecodedOpcode::Execute(CpuInstructionKind::StoreAToAddress {
                destination: match opcode {
                    0xE0 => MemoryAddressSource::HighImmediate8,
                    0xE2 => MemoryAddressSource::HighC,
                    _ => unreachable!("opcode filter already constrained"),
                },
            });
        }

        if matches!(opcode, 0x22 | 0x32) {
            return DecodedOpcode::Execute(CpuInstructionKind::StoreAToHlWithUpdate {
                direction: decode_hl_update_direction(opcode),
            });
        }

        if matches!(opcode, 0x0A | 0x1A | 0xFA) {
            return DecodedOpcode::Execute(CpuInstructionKind::LoadAFromAddress {
                source: match opcode {
                    0x0A => MemoryAddressSource::BC,
                    0x1A => MemoryAddressSource::DE,
                    0xFA => MemoryAddressSource::Immediate16,
                    _ => unreachable!("opcode filter already constrained"),
                },
            });
        }

        if matches!(opcode, 0xF0 | 0xF2) {
            return DecodedOpcode::Execute(CpuInstructionKind::LoadAFromAddress {
                source: match opcode {
                    0xF0 => MemoryAddressSource::HighImmediate8,
                    0xF2 => MemoryAddressSource::HighC,
                    _ => unreachable!("opcode filter already constrained"),
                },
            });
        }

        if opcode == 0x08 {
            return DecodedOpcode::Execute(CpuInstructionKind::StoreSpToImmediate16);
        }

        if opcode == 0xF8 {
            return DecodedOpcode::Execute(CpuInstructionKind::LoadHlFromSpPlusImmediate);
        }

        if opcode == 0xE8 {
            return DecodedOpcode::Execute(CpuInstructionKind::AddSpImmediate);
        }

        if opcode == 0xF9 {
            return DecodedOpcode::Execute(CpuInstructionKind::LoadSpFromHl);
        }

        if matches!(opcode, 0x2A | 0x3A) {
            return DecodedOpcode::Execute(CpuInstructionKind::LoadAFromHlWithUpdate {
                direction: decode_hl_update_direction(opcode),
            });
        }

        if opcode == 0xE9 {
            self.registers.pc = self.hl();
            return DecodedOpcode::Complete;
        }

        if opcode & 0xCF == 0x03 {
            return DecodedOpcode::Execute(CpuInstructionKind::IncrementRegisterPair {
                target: decode_register16((opcode >> 4) & 0x03),
            });
        }

        if opcode & 0xCF == 0x09 {
            return DecodedOpcode::Execute(CpuInstructionKind::AddHl {
                source: decode_register16((opcode >> 4) & 0x03),
            });
        }

        if opcode & 0xCF == 0x0B {
            return DecodedOpcode::Execute(CpuInstructionKind::DecrementRegisterPair {
                target: decode_register16((opcode >> 4) & 0x03),
            });
        }

        if opcode & 0b1100_0111 == 0b0000_0100 {
            return match decode_register8_operand((opcode >> 3) & 0x07) {
                Register8Operand::Register(target) => {
                    let before = self.read_register8(target);
                    let result = before.wrapping_add(1);
                    self.write_register8(target, result);
                    self.update_inc_flags(before, result);
                    DecodedOpcode::Complete
                }
                Register8Operand::IndirectHl => {
                    DecodedOpcode::Execute(CpuInstructionKind::IncrementHlMemory)
                }
            };
        }

        if opcode & 0b1100_0111 == 0b0000_0101 {
            return match decode_register8_operand((opcode >> 3) & 0x07) {
                Register8Operand::Register(target) => {
                    let before = self.read_register8(target);
                    let result = before.wrapping_sub(1);
                    self.write_register8(target, result);
                    self.update_dec_flags(before, result);
                    DecodedOpcode::Complete
                }
                Register8Operand::IndirectHl => {
                    DecodedOpcode::Execute(CpuInstructionKind::DecrementHlMemory)
                }
            };
        }

        if matches!(
            opcode,
            0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE
        ) {
            return DecodedOpcode::Execute(CpuInstructionKind::AluImmediate {
                operation: decode_alu_operation((opcode >> 3) & 0x07),
            });
        }

        if (0x80..=0xBF).contains(&opcode) {
            let operation = decode_alu_operation((opcode >> 3) & 0x07);
            return match decode_register8_operand(opcode & 0x07) {
                Register8Operand::Register(source) => {
                    let value = self.read_register8(source);
                    self.apply_alu_operation(operation, value);
                    DecodedOpcode::Complete
                }
                Register8Operand::IndirectHl => {
                    DecodedOpcode::Execute(CpuInstructionKind::AluFromHl { operation })
                }
            };
        }

        if opcode == 0xCB {
            return DecodedOpcode::Execute(CpuInstructionKind::CbPrefixed);
        }

        if matches!(opcode, 0x18 | 0x20 | 0x28 | 0x30 | 0x38) {
            return DecodedOpcode::Execute(CpuInstructionKind::RelativeJump {
                condition: decode_relative_jump_condition(opcode),
            });
        }

        if matches!(opcode, 0xC3 | 0xC2 | 0xCA | 0xD2 | 0xDA) {
            return DecodedOpcode::Execute(CpuInstructionKind::AbsoluteJump {
                condition: decode_absolute_jump_condition(opcode),
            });
        }

        if matches!(opcode, 0xCD | 0xC4 | 0xCC | 0xD4 | 0xDC) {
            return DecodedOpcode::Execute(CpuInstructionKind::Call {
                condition: decode_call_condition(opcode),
            });
        }

        if matches!(opcode, 0xC9 | 0xC0 | 0xC8 | 0xD0 | 0xD8) {
            return DecodedOpcode::Execute(CpuInstructionKind::Return {
                condition: decode_return_condition(opcode),
            });
        }

        if opcode == 0xD9 {
            return DecodedOpcode::Execute(CpuInstructionKind::ReturnFromInterrupt);
        }

        if opcode & 0xC7 == 0xC7 {
            return DecodedOpcode::Execute(CpuInstructionKind::Restart {
                vector: u16::from(opcode & 0x38),
            });
        }

        if opcode & 0xCF == 0xC5 {
            return DecodedOpcode::Execute(CpuInstructionKind::PushRegisterPair {
                source: decode_stack_register16((opcode >> 4) & 0x03),
            });
        }

        if opcode & 0xCF == 0xC1 {
            return DecodedOpcode::Execute(CpuInstructionKind::PopRegisterPair {
                target: decode_stack_register16((opcode >> 4) & 0x03),
            });
        }

        DecodedOpcode::Unsupported
    }

    fn begin_instruction(&mut self, opcode: u8, kind: CpuInstructionKind) {
        self.instruction_kind = Some(kind);
        self.cb_instruction_kind = None;
        self.execution_state = CpuExecutionState::Execute {
            opcode,
            step: 0,
            t_cycle: 0,
        };
    }

    fn advance_instruction(&mut self, opcode: u8, next_step: u8) {
        self.execution_state = CpuExecutionState::Execute {
            opcode,
            step: next_step,
            t_cycle: 0,
        };
    }

    fn stall_instruction(&mut self, opcode: u8, step: u8) {
        self.execution_state = CpuExecutionState::Execute {
            opcode,
            step,
            t_cycle: LAST_MACHINE_CYCLE_T,
        };
    }

    fn begin_interrupt_service(&mut self, source: InterruptSource) {
        self.ime = false;
        self.cancel_delayed_ime_enable();
        self.current_opcode = None;
        self.instruction_kind = None;
        self.cb_instruction_kind = None;
        self.operand8_latch = 0;
        self.operand16_latch = 0;
        self.execution_state = CpuExecutionState::ServiceInterrupt {
            source,
            step: 0,
            t_cycle: 0,
        };
    }

    fn advance_interrupt_service(&mut self, source: InterruptSource, next_step: u8) {
        self.execution_state = CpuExecutionState::ServiceInterrupt {
            source,
            step: next_step,
            t_cycle: 0,
        };
    }

    fn finish_instruction(&mut self) {
        self.current_opcode = None;
        self.instruction_kind = None;
        self.cb_instruction_kind = None;
        self.operand8_latch = 0;
        self.operand16_latch = 0;
        self.advance_delayed_ime_enable();
        self.execution_state = CpuExecutionState::fetch_opcode();
    }

    fn finish_and_request_halt(&mut self) {
        self.current_opcode = None;
        self.instruction_kind = None;
        self.cb_instruction_kind = None;
        self.operand8_latch = 0;
        self.operand16_latch = 0;
        self.halt_request_ime = self.ime;
        self.halt_request_had_delayed_ei = self.delayed_ime_enable;
        self.advance_delayed_ime_enable();
        self.halt_request_pending = true;
        self.execution_state = CpuExecutionState::fetch_opcode();
    }

    fn enter_stopped_state(&mut self) {
        self.current_opcode = None;
        self.instruction_kind = None;
        self.cb_instruction_kind = None;
        self.operand8_latch = 0;
        self.operand16_latch = 0;
        self.advance_delayed_ime_enable();
        self.execution_state = CpuExecutionState::Stopped;
    }

    fn finish_interrupt_service(&mut self) {
        self.current_opcode = None;
        self.instruction_kind = None;
        self.cb_instruction_kind = None;
        self.operand8_latch = 0;
        self.operand16_latch = 0;
        self.execution_state = CpuExecutionState::fetch_opcode();
    }

    fn enter_unsupported_opcode_trap(&mut self, opcode: u8) {
        let address = self
            .last_address_event
            .and_then(|event| event.access_address)
            .unwrap_or_else(|| self.registers.pc.wrapping_sub(1));
        self.instruction_kind = None;
        self.cb_instruction_kind = None;
        self.operand8_latch = 0;
        self.operand16_latch = 0;
        self.execution_state = CpuExecutionState::DiagnosticTrap {
            trap: CpuDiagnosticTrap::UnsupportedOpcode { opcode, address },
        };
    }

    fn last_bus_activity_trace_value(&self) -> String {
        match self.last_bus_activity {
            Some(CpuTraceBusActivity {
                kind,
                address,
                value,
            }) => format!("{}@{address:#06X}={value:#04X}", kind.trace_label()),
            None => "none".to_string(),
        }
    }

    fn last_address_event_trace_value(&self) -> String {
        match self.last_address_event {
            Some(event) => event.trace_value(),
            None => "none".to_string(),
        }
    }

    fn record_bus_activity(&mut self, kind: CpuTraceBusAccessKind, address: u16, value: u8) {
        self.last_bus_activity = Some(CpuTraceBusActivity {
            kind,
            address,
            value,
        });
    }

    fn record_address_event(&mut self, event: CpuAddressEvent) {
        self.last_address_event = Some(event);
    }

    fn read_opcode_u8<F>(&mut self, bus_operation: &mut F) -> u8
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        self.read_pc_u8_with_kind(bus_operation, CpuTraceBusAccessKind::OpcodeFetch)
    }

    fn read_pc_u8<F>(&mut self, bus_operation: &mut F) -> u8
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        self.read_pc_u8_with_kind(bus_operation, CpuTraceBusAccessKind::OperandRead)
    }

    fn read_pc_u8_with_kind<F>(&mut self, bus_operation: &mut F, kind: CpuTraceBusAccessKind) -> u8
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        let address = self.registers.pc;
        let value = self.read_byte_with_kind(address, bus_operation, kind);

        if self.halt_bug_pending {
            self.halt_bug_pending = false;
            self.record_address_event(CpuAddressEvent::read(address));
        } else {
            self.registers.pc = self.registers.pc.wrapping_add(1);
            self.record_address_event(CpuAddressEvent::read_with_incdec(
                address,
                self.registers.pc,
                CpuAddressUpdateDirection::Increment,
            ));
        }
        value
    }

    fn read_byte<F>(&mut self, address: u16, bus_operation: &mut F) -> u8
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        let value =
            self.read_byte_with_kind(address, bus_operation, CpuTraceBusAccessKind::DataRead);
        self.record_address_event(CpuAddressEvent::read(address));
        value
    }

    fn read_byte_with_kind<F>(
        &mut self,
        address: u16,
        bus_operation: &mut F,
        kind: CpuTraceBusAccessKind,
    ) -> u8
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        let value = bus_operation(CpuBusOperation::Read { address })
            .expect("CPU bus read must produce a byte result");
        self.record_bus_activity(kind, address, value);
        value
    }

    fn write_byte<F>(&mut self, address: u16, value: u8, bus_operation: &mut F)
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        let _ = bus_operation(CpuBusOperation::Write { address, value });
        self.record_bus_activity(CpuTraceBusAccessKind::DataWrite, address, value);
        self.record_address_event(CpuAddressEvent::write(address));
    }

    fn resolve_memory_address(&self, source: MemoryAddressSource) -> u16 {
        match source {
            MemoryAddressSource::BC => self.bc(),
            MemoryAddressSource::DE => self.de(),
            MemoryAddressSource::Immediate16 => self.operand16_latch,
            MemoryAddressSource::HighImmediate8 => 0xFF00 | u16::from(self.operand8_latch),
            MemoryAddressSource::HighC => 0xFF00 | u16::from(self.registers.c),
        }
    }

    fn read_hl_with_update<F>(
        &mut self,
        direction: CpuAddressUpdateDirection,
        bus_operation: &mut F,
    ) -> u8
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        let address = self.hl();
        let value = self.read_byte(address, bus_operation);
        let updated = self.increment_or_decrement_register16(Register16::HL, direction);
        self.record_address_event(CpuAddressEvent::read_with_incdec(
            address, updated, direction,
        ));
        value
    }

    fn write_hl_with_update<F>(
        &mut self,
        value: u8,
        direction: CpuAddressUpdateDirection,
        bus_operation: &mut F,
    ) where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        let address = self.hl();
        self.write_byte(address, value, bus_operation);
        let updated = self.increment_or_decrement_register16(Register16::HL, direction);
        self.record_address_event(CpuAddressEvent::write_with_incdec(
            address, updated, direction,
        ));
    }

    fn read_byte_and_increment_sp<F>(&mut self, bus_operation: &mut F) -> u8
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        let address = self.registers.sp;
        let value = self.read_byte(address, bus_operation);
        self.registers.sp = self.registers.sp.wrapping_add(1);
        self.record_address_event(CpuAddressEvent::read_with_incdec(
            address,
            self.registers.sp,
            CpuAddressUpdateDirection::Increment,
        ));
        value
    }

    fn decrement_sp_and_record_idu_event(&mut self) {
        self.registers.sp = self.registers.sp.wrapping_sub(1);
        self.record_address_event(CpuAddressEvent::incdec(
            self.registers.sp,
            CpuAddressUpdateDirection::Decrement,
        ));
    }

    fn write_byte_at_sp<F>(&mut self, value: u8, bus_operation: &mut F)
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        self.write_byte(self.registers.sp, value, bus_operation);
    }

    fn write_byte_with_decremented_sp<F>(&mut self, value: u8, bus_operation: &mut F)
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        self.registers.sp = self.registers.sp.wrapping_sub(1);
        let address = self.registers.sp;
        self.write_byte(address, value, bus_operation);
        self.record_address_event(CpuAddressEvent::write_with_incdec(
            address,
            address,
            CpuAddressUpdateDirection::Decrement,
        ));
    }

    fn increment_or_decrement_register_pair(
        &mut self,
        target: Register16,
        direction: CpuAddressUpdateDirection,
    ) {
        let updated = self.increment_or_decrement_register16(target, direction);
        self.record_address_event(CpuAddressEvent::incdec(updated, direction));
    }

    fn increment_or_decrement_register16(
        &mut self,
        target: Register16,
        direction: CpuAddressUpdateDirection,
    ) -> u16 {
        let current = match target {
            Register16::BC => self.bc(),
            Register16::DE => self.de(),
            Register16::HL => self.hl(),
            Register16::SP => self.registers.sp,
        };
        let updated = match direction {
            CpuAddressUpdateDirection::Increment => current.wrapping_add(1),
            CpuAddressUpdateDirection::Decrement => current.wrapping_sub(1),
        };
        self.write_register16(target, updated);
        updated
    }

    fn complete_interrupt_service_machine_cycle<F>(
        &mut self,
        source: InterruptSource,
        step: u8,
        bus_operation: &mut F,
    ) -> Option<InterruptSource>
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        match step {
            0 | 1 => {
                self.advance_interrupt_service(source, step + 1);
                None
            }
            2 => {
                let [low, _high] = self.registers.pc.to_le_bytes();
                self.operand8_latch = low;
                self.decrement_sp_and_record_idu_event();
                self.advance_interrupt_service(source, 3);
                None
            }
            3 => {
                let [_low, high] = self.registers.pc.to_le_bytes();
                let upper_pc_push_targets_ie = self.registers.sp == 0xFFFF;
                self.write_byte_at_sp(high, bus_operation);
                // IE can be the target of the upper-byte push at 0xFFFF, so the
                // dispatch source must stay live until after this write commits.
                if upper_pc_push_targets_ie {
                    if let Some(next_source) = self.current_highest_pending_interrupt(bus_operation)
                    {
                        self.advance_interrupt_service(next_source, 4);
                    } else {
                        self.registers.pc = 0x0000;
                        self.finish_interrupt_service();
                    }
                } else {
                    self.advance_interrupt_service(source, 4);
                }
                None
            }
            4 => {
                self.write_byte_with_decremented_sp(self.operand8_latch, bus_operation);
                self.registers.pc = interrupt_vector(source);
                self.finish_interrupt_service();
                Some(source)
            }
            _ => {
                self.advance_interrupt_service(source, step);
                None
            }
        }
    }

    fn can_accept_interrupt(&self) -> bool {
        matches!(self.execution_state, CpuExecutionState::FetchOpcode { .. })
            && self.current_opcode.is_none()
    }

    fn accept_pending_interrupt(&mut self, interrupts: &mut InterruptController) {
        let Some(source) = interrupts.highest_pending() else {
            return;
        };

        self.begin_interrupt_service(source);
    }

    fn current_highest_pending_interrupt<F>(
        &mut self,
        bus_operation: &mut F,
    ) -> Option<InterruptSource>
    where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        let pending_mask = bus_operation(CpuBusOperation::PendingInterruptMask).unwrap_or(0);
        highest_pending_interrupt_from_mask(pending_mask)
    }

    fn schedule_delayed_ime_enable(&mut self) {
        if self.delayed_ime_enable {
            return;
        }

        self.delayed_ime_enable = true;
        self.delayed_ime_enable_steps = 2;
    }

    fn cancel_delayed_ime_enable(&mut self) {
        self.delayed_ime_enable = false;
        self.delayed_ime_enable_steps = 0;
    }

    fn advance_delayed_ime_enable(&mut self) {
        if self.delayed_ime_enable_steps == 0 {
            self.delayed_ime_enable = false;
            return;
        }

        self.delayed_ime_enable_steps -= 1;
        if self.delayed_ime_enable_steps == 0 {
            self.ime = true;
            self.delayed_ime_enable = false;
        }
    }

    fn condition_is_met(&self, condition: Option<ConditionCode>) -> bool {
        match condition {
            None => true,
            Some(ConditionCode::Nz) => self.registers.f & FLAG_Z == 0,
            Some(ConditionCode::Z) => self.registers.f & FLAG_Z != 0,
            Some(ConditionCode::Nc) => self.registers.f & FLAG_C == 0,
            Some(ConditionCode::C) => self.registers.f & FLAG_C != 0,
        }
    }

    fn read_register8(&self, register: Register8) -> u8 {
        match register {
            Register8::A => self.registers.a,
            Register8::B => self.registers.b,
            Register8::C => self.registers.c,
            Register8::D => self.registers.d,
            Register8::E => self.registers.e,
            Register8::H => self.registers.h,
            Register8::L => self.registers.l,
        }
    }

    fn write_register8(&mut self, register: Register8, value: u8) {
        match register {
            Register8::A => self.registers.a = value,
            Register8::B => self.registers.b = value,
            Register8::C => self.registers.c = value,
            Register8::D => self.registers.d = value,
            Register8::E => self.registers.e = value,
            Register8::H => self.registers.h = value,
            Register8::L => self.registers.l = value,
        }
    }

    fn write_register16(&mut self, register: Register16, value: u16) {
        let [low, high] = value.to_le_bytes();

        match register {
            Register16::BC => {
                self.registers.b = high;
                self.registers.c = low;
            }
            Register16::DE => {
                self.registers.d = high;
                self.registers.e = low;
            }
            Register16::HL => {
                self.registers.h = high;
                self.registers.l = low;
            }
            Register16::SP => self.registers.sp = value,
        }
    }

    fn read_stack_register16(&self, register: StackRegister16) -> u16 {
        match register {
            StackRegister16::BC => self.bc(),
            StackRegister16::DE => self.de(),
            StackRegister16::HL => self.hl(),
            StackRegister16::AF => u16::from_be_bytes([self.registers.a, self.registers.f]),
        }
    }

    fn write_stack_register16(&mut self, register: StackRegister16, value: u16) {
        let [high, low] = value.to_be_bytes();

        match register {
            StackRegister16::BC => {
                self.registers.b = high;
                self.registers.c = low;
            }
            StackRegister16::DE => {
                self.registers.d = high;
                self.registers.e = low;
            }
            StackRegister16::HL => {
                self.registers.h = high;
                self.registers.l = low;
            }
            StackRegister16::AF => {
                self.registers.a = high;
                self.registers.f = low & 0xF0;
            }
        }
    }

    fn bc(&self) -> u16 {
        u16::from_be_bytes([self.registers.b, self.registers.c])
    }

    fn de(&self) -> u16 {
        u16::from_be_bytes([self.registers.d, self.registers.e])
    }

    fn hl(&self) -> u16 {
        u16::from_be_bytes([self.registers.h, self.registers.l])
    }

    fn update_inc_flags(&mut self, before: u8, result: u8) {
        let carry = self.registers.f & FLAG_C != 0;
        self.registers.f = 0;

        if result == 0 {
            self.registers.f |= FLAG_Z;
        }
        if (before & 0x0F) == 0x0F {
            self.registers.f |= FLAG_H;
        }
        if carry {
            self.registers.f |= FLAG_C;
        }
    }

    fn update_dec_flags(&mut self, before: u8, result: u8) {
        let carry = self.registers.f & FLAG_C != 0;
        self.registers.f = FLAG_N;

        if result == 0 {
            self.registers.f |= FLAG_Z;
        }
        if before & 0x0F == 0 {
            self.registers.f |= FLAG_H;
        }
        if carry {
            self.registers.f |= FLAG_C;
        }
    }

    fn add_to_a(&mut self, value: u8) {
        let a = self.registers.a;
        let (result, carry) = a.overflowing_add(value);
        let half_carry = (a & 0x0F) + (value & 0x0F) > 0x0F;

        self.registers.a = result;
        self.write_flags(result == 0, false, half_carry, carry);
    }

    fn adc_to_a(&mut self, value: u8) {
        let a = self.registers.a;
        let carry_in = u8::from(self.registers.f & FLAG_C != 0);
        let result16 = u16::from(a) + u16::from(value) + u16::from(carry_in);
        let result = result16 as u8;
        let half_carry = (a & 0x0F) + (value & 0x0F) + carry_in > 0x0F;

        self.registers.a = result;
        self.write_flags(result == 0, false, half_carry, result16 > 0xFF);
    }

    fn sub_from_a(&mut self, value: u8) {
        let a = self.registers.a;
        let result = a.wrapping_sub(value);
        let half_carry = (a & 0x0F) < (value & 0x0F);
        let carry = a < value;

        self.registers.a = result;
        self.write_flags(result == 0, true, half_carry, carry);
    }

    fn sbc_from_a(&mut self, value: u8) {
        let a = self.registers.a;
        let carry_in = u8::from(self.registers.f & FLAG_C != 0);
        let result = a.wrapping_sub(value).wrapping_sub(carry_in);
        let half_carry = (a & 0x0F) < ((value & 0x0F) + carry_in);
        let carry = u16::from(a) < (u16::from(value) + u16::from(carry_in));

        self.registers.a = result;
        self.write_flags(result == 0, true, half_carry, carry);
    }

    fn and_with_a(&mut self, value: u8) {
        self.registers.a &= value;
        self.write_flags(self.registers.a == 0, false, true, false);
    }

    fn xor_with_a(&mut self, value: u8) {
        self.registers.a ^= value;
        self.write_flags(self.registers.a == 0, false, false, false);
    }

    fn or_with_a(&mut self, value: u8) {
        self.registers.a |= value;
        self.write_flags(self.registers.a == 0, false, false, false);
    }

    fn compare_a(&mut self, value: u8) {
        let a = self.registers.a;
        let result = a.wrapping_sub(value);
        let half_carry = (a & 0x0F) < (value & 0x0F);
        let carry = a < value;

        self.write_flags(result == 0, true, half_carry, carry);
    }

    fn add_to_hl(&mut self, value: u16) {
        let hl = self.hl();
        let result = hl.wrapping_add(value);
        let zero = self.registers.f & FLAG_Z != 0;
        let half_carry = (hl & 0x0FFF) + (value & 0x0FFF) > 0x0FFF;
        let carry = u32::from(hl) + u32::from(value) > 0xFFFF;

        self.write_register16(Register16::HL, result);
        self.write_flags(zero, false, half_carry, carry);
    }

    fn sp_plus_signed_immediate(&mut self, value: u8) -> u16 {
        let sp = self.registers.sp;
        let signed = i16::from(i8::from_ne_bytes([value]));
        let result = sp.wrapping_add_signed(signed);
        let half_carry = (sp & 0x000F) + u16::from(value & 0x0F) > 0x000F;
        let carry = (sp & 0x00FF) + u16::from(value) > 0x00FF;

        self.write_flags(false, false, half_carry, carry);
        result
    }

    fn apply_alu_operation(&mut self, operation: AluOperation, value: u8) {
        match operation {
            AluOperation::Add => self.add_to_a(value),
            AluOperation::Adc => self.adc_to_a(value),
            AluOperation::Sub => self.sub_from_a(value),
            AluOperation::Sbc => self.sbc_from_a(value),
            AluOperation::And => self.and_with_a(value),
            AluOperation::Xor => self.xor_with_a(value),
            AluOperation::Or => self.or_with_a(value),
            AluOperation::Compare => self.compare_a(value),
        }
    }

    fn decimal_adjust_a(&mut self) {
        let mut a = self.registers.a;
        let subtract = self.registers.f & FLAG_N != 0;
        let half_carry = self.registers.f & FLAG_H != 0;
        let carry = self.registers.f & FLAG_C != 0;
        let mut adjust = 0_u8;
        let mut next_carry = carry;

        if !subtract {
            if half_carry || (a & 0x0F) > 0x09 {
                adjust |= 0x06;
            }
            if carry || a > 0x99 {
                adjust |= 0x60;
                next_carry = true;
            }
            a = a.wrapping_add(adjust);
        } else {
            if half_carry {
                adjust |= 0x06;
            }
            if carry {
                adjust |= 0x60;
            }
            a = a.wrapping_sub(adjust);
        }

        self.registers.a = a;
        self.write_flags(a == 0, subtract, false, next_carry);
    }

    fn complement_a(&mut self) {
        self.registers.a = !self.registers.a;
        let zero = self.registers.f & FLAG_Z != 0;
        let carry = self.registers.f & FLAG_C != 0;
        self.write_flags(zero, true, true, carry);
    }

    fn set_carry_flag(&mut self) {
        let zero = self.registers.f & FLAG_Z != 0;
        self.write_flags(zero, false, false, true);
    }

    fn complement_carry_flag(&mut self) {
        let zero = self.registers.f & FLAG_Z != 0;
        let carry = self.registers.f & FLAG_C == 0;
        self.write_flags(zero, false, false, carry);
    }

    fn decode_cb_opcode(&self, opcode: u8) -> Option<CbInstructionKind> {
        if opcode & 0xF8 == 0x00 {
            return Some(CbInstructionKind::RotateLeftCarry {
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xF8 == 0x08 {
            return Some(CbInstructionKind::RotateRightCarry {
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xF8 == 0x10 {
            return Some(CbInstructionKind::RotateLeftThroughCarry {
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xF8 == 0x18 {
            return Some(CbInstructionKind::RotateRightThroughCarry {
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xF8 == 0x20 {
            return Some(CbInstructionKind::ShiftLeftArithmetic {
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xF8 == 0x28 {
            return Some(CbInstructionKind::ShiftRightArithmetic {
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xF8 == 0x30 {
            return Some(CbInstructionKind::SwapNibbles {
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xF8 == 0x38 {
            return Some(CbInstructionKind::ShiftRightLogical {
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xC0 == 0x40 {
            return Some(CbInstructionKind::BitTest {
                bit: (opcode >> 3) & 0x07,
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xC0 == 0x80 {
            return Some(CbInstructionKind::ResetBit {
                bit: (opcode >> 3) & 0x07,
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xC0 == 0xC0 {
            return Some(CbInstructionKind::SetBit {
                bit: (opcode >> 3) & 0x07,
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        None
    }

    fn apply_cb_operation(&mut self, kind: CbInstructionKind, value: u8) -> Option<u8> {
        match kind {
            CbInstructionKind::RotateLeftCarry { .. } => Some(self.rotate_left_carry(value)),
            CbInstructionKind::RotateRightCarry { .. } => Some(self.rotate_right_carry(value)),
            CbInstructionKind::RotateLeftThroughCarry { .. } => {
                Some(self.rotate_left_through_carry(value))
            }
            CbInstructionKind::RotateRightThroughCarry { .. } => {
                Some(self.rotate_right_through_carry(value))
            }
            CbInstructionKind::ShiftLeftArithmetic { .. } => {
                Some(self.shift_left_arithmetic(value))
            }
            CbInstructionKind::ShiftRightArithmetic { .. } => {
                Some(self.shift_right_arithmetic(value))
            }
            CbInstructionKind::SwapNibbles { .. } => Some(self.swap_nibbles(value)),
            CbInstructionKind::ShiftRightLogical { .. } => Some(self.shift_right_logical(value)),
            CbInstructionKind::BitTest { bit, .. } => {
                self.bit_test(bit, value);
                None
            }
            CbInstructionKind::ResetBit { bit, .. } => Some(self.reset_bit(value, bit)),
            CbInstructionKind::SetBit { bit, .. } => Some(self.set_bit(value, bit)),
        }
    }

    fn rotate_left_carry(&mut self, value: u8) -> u8 {
        let carry = value & 0x80 != 0;
        let result = value.rotate_left(1);
        self.write_flags(result == 0, false, false, carry);
        result
    }

    fn rotate_right_carry(&mut self, value: u8) -> u8 {
        let carry = value & 0x01 != 0;
        let result = value.rotate_right(1);
        self.write_flags(result == 0, false, false, carry);
        result
    }

    fn rotate_left_through_carry(&mut self, value: u8) -> u8 {
        let carry_in = u8::from(self.registers.f & FLAG_C != 0);
        let carry_out = value & 0x80 != 0;
        let result = (value << 1) | carry_in;
        self.write_flags(result == 0, false, false, carry_out);
        result
    }

    fn rotate_right_through_carry(&mut self, value: u8) -> u8 {
        let carry_in = u8::from(self.registers.f & FLAG_C != 0) << 7;
        let carry_out = value & 0x01 != 0;
        let result = (value >> 1) | carry_in;
        self.write_flags(result == 0, false, false, carry_out);
        result
    }

    fn shift_left_arithmetic(&mut self, value: u8) -> u8 {
        let carry_out = value & 0x80 != 0;
        let result = value << 1;
        self.write_flags(result == 0, false, false, carry_out);
        result
    }

    fn shift_right_arithmetic(&mut self, value: u8) -> u8 {
        let carry_out = value & 0x01 != 0;
        let result = (value >> 1) | (value & 0x80);
        self.write_flags(result == 0, false, false, carry_out);
        result
    }

    fn swap_nibbles(&mut self, value: u8) -> u8 {
        let result = value.rotate_left(4);
        self.write_flags(result == 0, false, false, false);
        result
    }

    fn shift_right_logical(&mut self, value: u8) -> u8 {
        let carry_out = value & 0x01 != 0;
        let result = value >> 1;
        self.write_flags(result == 0, false, false, carry_out);
        result
    }

    fn bit_test(&mut self, bit: u8, value: u8) {
        let carry = self.registers.f & FLAG_C != 0;
        self.registers.f = FLAG_H;

        if value & (1 << bit) == 0 {
            self.registers.f |= FLAG_Z;
        }
        if carry {
            self.registers.f |= FLAG_C;
        }
    }

    fn reset_bit(&mut self, value: u8, bit: u8) -> u8 {
        value & !(1 << bit)
    }

    fn set_bit(&mut self, value: u8, bit: u8) -> u8 {
        value | (1 << bit)
    }

    fn write_flags(&mut self, zero: bool, subtract: bool, half_carry: bool, carry: bool) {
        self.registers.f = 0;

        if zero {
            self.registers.f |= FLAG_Z;
        }
        if subtract {
            self.registers.f |= FLAG_N;
        }
        if half_carry {
            self.registers.f |= FLAG_H;
        }
        if carry {
            self.registers.f |= FLAG_C;
        }
    }
}

impl CpuAddressEvent {
    const fn read(address: u16) -> Self {
        Self {
            kind: CpuAddressEventKind::Read,
            access_address: Some(address),
            idu_address: None,
            update_direction: None,
        }
    }

    const fn write(address: u16) -> Self {
        Self {
            kind: CpuAddressEventKind::Write,
            access_address: Some(address),
            idu_address: None,
            update_direction: None,
        }
    }

    const fn incdec(address: u16, direction: CpuAddressUpdateDirection) -> Self {
        Self {
            kind: CpuAddressEventKind::IncDec,
            access_address: None,
            idu_address: Some(address),
            update_direction: Some(direction),
        }
    }

    const fn read_with_incdec(
        access_address: u16,
        idu_address: u16,
        direction: CpuAddressUpdateDirection,
    ) -> Self {
        Self {
            kind: CpuAddressEventKind::ReadWithIncDec,
            access_address: Some(access_address),
            idu_address: Some(idu_address),
            update_direction: Some(direction),
        }
    }

    const fn write_with_incdec(
        access_address: u16,
        idu_address: u16,
        direction: CpuAddressUpdateDirection,
    ) -> Self {
        Self {
            kind: CpuAddressEventKind::WriteWithIncDec,
            access_address: Some(access_address),
            idu_address: Some(idu_address),
            update_direction: Some(direction),
        }
    }

    fn trace_value(self) -> String {
        match self.kind {
            CpuAddressEventKind::Read => {
                let address = self
                    .access_address
                    .expect("read event must carry an access address");
                format!("read@{address:#06X}")
            }
            CpuAddressEventKind::Write => {
                let address = self
                    .access_address
                    .expect("write event must carry an access address");
                format!("write@{address:#06X}")
            }
            CpuAddressEventKind::IncDec => {
                let address = self
                    .idu_address
                    .expect("inc/dec event must carry an IDU address");
                format!(
                    "{}@{address:#06X}",
                    self.update_direction
                        .expect("inc/dec event must carry a direction")
                        .trace_label()
                )
            }
            CpuAddressEventKind::ReadWithIncDec | CpuAddressEventKind::WriteWithIncDec => {
                let access_address = self
                    .access_address
                    .expect("combined event must carry an access address");
                let idu_address = self
                    .idu_address
                    .expect("combined event must carry an IDU address");
                let access_label = match self.kind {
                    CpuAddressEventKind::ReadWithIncDec => "read",
                    CpuAddressEventKind::WriteWithIncDec => "write",
                    _ => unreachable!("combined event match already constrained"),
                };
                format!(
                    "{access_label}+{}@{access_address:#06X}->{idu_address:#06X}",
                    self.update_direction
                        .expect("combined event must carry a direction")
                        .trace_label()
                )
            }
        }
    }
}

impl CpuAddressUpdateDirection {
    const fn trace_label(self) -> &'static str {
        match self {
            Self::Increment => "inc",
            Self::Decrement => "dec",
        }
    }
}

impl CpuStartupState {
    pub const fn power_on_reset() -> Self {
        Self {
            a: 0,
            f: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,
            sp: 0,
            pc: 0x0000,
        }
    }
}

impl CpuRegisters {
    pub const fn from_startup_state(startup_state: CpuStartupState) -> Self {
        Self {
            a: startup_state.a,
            f: startup_state.f & 0xF0,
            b: startup_state.b,
            c: startup_state.c,
            d: startup_state.d,
            e: startup_state.e,
            h: startup_state.h,
            l: startup_state.l,
            sp: startup_state.sp,
            pc: startup_state.pc,
        }
    }
}

impl CpuExecutionState {
    pub const fn fetch_opcode() -> Self {
        Self::FetchOpcode { t_cycle: 0 }
    }
}

const fn interrupt_vector(source: InterruptSource) -> u16 {
    match source {
        InterruptSource::VBlank => 0x0040,
        InterruptSource::LcdStat => 0x0048,
        InterruptSource::Timer => 0x0050,
        InterruptSource::Serial => 0x0058,
        InterruptSource::Joypad => 0x0060,
    }
}

const fn highest_pending_interrupt_from_mask(mask: u8) -> Option<InterruptSource> {
    if mask & 0x01 != 0 {
        Some(InterruptSource::VBlank)
    } else if mask & 0x02 != 0 {
        Some(InterruptSource::LcdStat)
    } else if mask & 0x04 != 0 {
        Some(InterruptSource::Timer)
    } else if mask & 0x08 != 0 {
        Some(InterruptSource::Serial)
    } else if mask & 0x10 != 0 {
        Some(InterruptSource::Joypad)
    } else {
        None
    }
}

fn decode_register16(bits: u8) -> Register16 {
    match bits {
        0 => Register16::BC,
        1 => Register16::DE,
        2 => Register16::HL,
        3 => Register16::SP,
        _ => unreachable!("2-bit register pair selector must be in 0..=3"),
    }
}

fn decode_register8_operand(bits: u8) -> Register8Operand {
    match bits {
        0 => Register8Operand::Register(Register8::B),
        1 => Register8Operand::Register(Register8::C),
        2 => Register8Operand::Register(Register8::D),
        3 => Register8Operand::Register(Register8::E),
        4 => Register8Operand::Register(Register8::H),
        5 => Register8Operand::Register(Register8::L),
        6 => Register8Operand::IndirectHl,
        7 => Register8Operand::Register(Register8::A),
        _ => unreachable!("3-bit register selector must be in 0..=7"),
    }
}

fn decode_stack_register16(bits: u8) -> StackRegister16 {
    match bits {
        0 => StackRegister16::BC,
        1 => StackRegister16::DE,
        2 => StackRegister16::HL,
        3 => StackRegister16::AF,
        _ => unreachable!("2-bit stack register selector must be in 0..=3"),
    }
}

fn decode_alu_operation(bits: u8) -> AluOperation {
    match bits {
        0 => AluOperation::Add,
        1 => AluOperation::Adc,
        2 => AluOperation::Sub,
        3 => AluOperation::Sbc,
        4 => AluOperation::And,
        5 => AluOperation::Xor,
        6 => AluOperation::Or,
        7 => AluOperation::Compare,
        _ => unreachable!("3-bit ALU selector must be in 0..=7"),
    }
}

fn decode_relative_jump_condition(opcode: u8) -> Option<ConditionCode> {
    match opcode {
        0x18 => None,
        0x20 => Some(ConditionCode::Nz),
        0x28 => Some(ConditionCode::Z),
        0x30 => Some(ConditionCode::Nc),
        0x38 => Some(ConditionCode::C),
        _ => unreachable!("opcode must be a JR form"),
    }
}

fn decode_absolute_jump_condition(opcode: u8) -> Option<ConditionCode> {
    match opcode {
        0xC3 => None,
        0xC2 => Some(ConditionCode::Nz),
        0xCA => Some(ConditionCode::Z),
        0xD2 => Some(ConditionCode::Nc),
        0xDA => Some(ConditionCode::C),
        _ => unreachable!("opcode must be a JP form"),
    }
}

fn decode_call_condition(opcode: u8) -> Option<ConditionCode> {
    match opcode {
        0xCD => None,
        0xC4 => Some(ConditionCode::Nz),
        0xCC => Some(ConditionCode::Z),
        0xD4 => Some(ConditionCode::Nc),
        0xDC => Some(ConditionCode::C),
        _ => unreachable!("opcode must be a CALL form"),
    }
}

fn decode_return_condition(opcode: u8) -> Option<ConditionCode> {
    match opcode {
        0xC9 => None,
        0xC0 => Some(ConditionCode::Nz),
        0xC8 => Some(ConditionCode::Z),
        0xD0 => Some(ConditionCode::Nc),
        0xD8 => Some(ConditionCode::C),
        _ => unreachable!("opcode must be a RET form"),
    }
}

fn decode_hl_update_direction(opcode: u8) -> CpuAddressUpdateDirection {
    match opcode {
        0x22 | 0x2A => CpuAddressUpdateDirection::Increment,
        0x32 | 0x3A => CpuAddressUpdateDirection::Decrement,
        _ => unreachable!("opcode must be an [hli]/[hld] transfer form"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::{Bus, BusArbitrationState, BusIoReadView, BusIoWriteView, BusRequester};
    use crate::cartridge::CartridgeSlot;
    use crate::interrupts::InterruptController;
    use crate::joypad::Joypad;
    use crate::model::CompatibilityPolicy;

    fn build_test_rom(program: &[u8]) -> Vec<u8> {
        let mut rom = vec![0xFF; 32 * 1024];
        for (offset, byte) in program.iter().copied().enumerate() {
            rom[0x0100 + offset] = byte;
        }
        rom[0x0147] = 0x00;
        rom[0x0148] = 0x00;
        rom[0x0149] = 0x00;
        rom
    }

    fn build_test_cartridge(program: &[u8]) -> CartridgeSlot {
        let compatibility = CompatibilityPolicy::default();
        let report = CartridgeSlot::load(build_test_rom(program), &compatibility)
            .expect("test cartridge should load as NoMBC");
        let (cartridge, _) = report.into_parts();
        cartridge
    }

    fn tick_cpu(cpu: &mut CpuCore, bus: &mut Bus, cartridge: &mut CartridgeSlot) {
        let arbitration_state = BusArbitrationState::default();

        let _ = cpu.tick_t_cycle(|operation| match operation {
            CpuBusOperation::Read { address } => Some(bus.read_with_context(
                address,
                BusRequester::Cpu,
                &arbitration_state,
                Some(&*cartridge),
                BusIoReadView::default(),
            )),
            CpuBusOperation::Write { address, value } => {
                bus.write_with_context(
                    address,
                    value,
                    BusRequester::Cpu,
                    &arbitration_state,
                    Some(cartridge),
                    BusIoWriteView::default(),
                );
                None
            }
            CpuBusOperation::PendingInterruptMask => Some(0),
        });
    }

    fn tick_cpu_with_interrupts(
        cpu: &mut CpuCore,
        bus: &mut Bus,
        cartridge: &mut CartridgeSlot,
        interrupts: &mut InterruptController,
    ) {
        let arbitration_state = BusArbitrationState::default();
        let acknowledged_interrupt = cpu.tick_t_cycle(|operation| match operation {
            CpuBusOperation::Read { address } => Some(bus.read_with_context(
                address,
                BusRequester::Cpu,
                &arbitration_state,
                Some(&*cartridge),
                BusIoReadView {
                    interrupts: Some(&*interrupts),
                    ..BusIoReadView::default()
                },
            )),
            CpuBusOperation::Write { address, value } => {
                bus.write_with_context(
                    address,
                    value,
                    BusRequester::Cpu,
                    &arbitration_state,
                    Some(cartridge),
                    BusIoWriteView {
                        interrupts: Some(interrupts),
                        ..BusIoWriteView::default()
                    },
                );
                None
            }
            CpuBusOperation::PendingInterruptMask => Some(interrupts.pending_mask()),
        });

        if let Some(source) = acknowledged_interrupt {
            interrupts.clear(source);
        }
    }

    fn tick_cpu_n(cpu: &mut CpuCore, bus: &mut Bus, cartridge: &mut CartridgeSlot, steps: usize) {
        for _ in 0..steps {
            tick_cpu(cpu, bus, cartridge);
        }
    }

    fn tick_cpu_n_with_interrupts(
        cpu: &mut CpuCore,
        bus: &mut Bus,
        cartridge: &mut CartridgeSlot,
        interrupts: &mut InterruptController,
        steps: usize,
    ) {
        for _ in 0..steps {
            tick_cpu_with_interrupts(cpu, bus, cartridge, interrupts);
        }
    }

    fn crc32_iso_hdlc(mut crc: u32, byte: u8) -> u32 {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
        crc
    }

    #[test]
    fn private_helpers_cover_remaining_flag_paths_decoder_tables_and_cb_operations() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        cpu.apply_startup_state(CpuStartupState::power_on_reset());

        assert_eq!(interrupt_vector(InterruptSource::VBlank), 0x0040);
        assert_eq!(interrupt_vector(InterruptSource::LcdStat), 0x0048);
        assert_eq!(interrupt_vector(InterruptSource::Timer), 0x0050);
        assert_eq!(interrupt_vector(InterruptSource::Serial), 0x0058);
        assert_eq!(interrupt_vector(InterruptSource::Joypad), 0x0060);
        assert_eq!(highest_pending_interrupt_from_mask(0x00), None,);
        assert_eq!(
            highest_pending_interrupt_from_mask(0x02),
            Some(InterruptSource::LcdStat),
        );
        assert_eq!(
            highest_pending_interrupt_from_mask(0x01),
            Some(InterruptSource::VBlank),
        );
        assert_eq!(
            highest_pending_interrupt_from_mask(0x04),
            Some(InterruptSource::Timer),
        );
        assert_eq!(
            highest_pending_interrupt_from_mask(0x08),
            Some(InterruptSource::Serial),
        );
        assert_eq!(
            highest_pending_interrupt_from_mask(0x10),
            Some(InterruptSource::Joypad),
        );

        assert_eq!(decode_register16(0), Register16::BC);
        assert_eq!(decode_register16(1), Register16::DE);
        assert_eq!(decode_register16(2), Register16::HL);
        assert_eq!(decode_register16(3), Register16::SP);
        assert_eq!(
            decode_register8_operand(2),
            Register8Operand::Register(Register8::D),
        );
        assert_eq!(
            decode_register8_operand(3),
            Register8Operand::Register(Register8::E),
        );
        assert_eq!(
            decode_register8_operand(4),
            Register8Operand::Register(Register8::H),
        );
        assert_eq!(
            decode_register8_operand(5),
            Register8Operand::Register(Register8::L),
        );
        assert_eq!(decode_register8_operand(6), Register8Operand::IndirectHl,);
        assert_eq!(
            decode_register8_operand(7),
            Register8Operand::Register(Register8::A),
        );
        assert_eq!(decode_stack_register16(0), StackRegister16::BC);
        assert_eq!(decode_stack_register16(1), StackRegister16::DE);
        assert_eq!(decode_stack_register16(2), StackRegister16::HL);
        assert_eq!(decode_stack_register16(3), StackRegister16::AF);
        assert_eq!(decode_alu_operation(0), AluOperation::Add);
        assert_eq!(decode_alu_operation(1), AluOperation::Adc);
        assert_eq!(decode_alu_operation(2), AluOperation::Sub);
        assert_eq!(decode_alu_operation(3), AluOperation::Sbc);
        assert_eq!(decode_alu_operation(4), AluOperation::And);
        assert_eq!(decode_alu_operation(5), AluOperation::Xor);
        assert_eq!(decode_alu_operation(6), AluOperation::Or);
        assert_eq!(decode_alu_operation(7), AluOperation::Compare);
        assert_eq!(decode_relative_jump_condition(0x18), None);
        assert_eq!(
            decode_relative_jump_condition(0x20),
            Some(ConditionCode::Nz),
        );
        assert_eq!(decode_relative_jump_condition(0x28), Some(ConditionCode::Z),);
        assert_eq!(
            decode_relative_jump_condition(0x30),
            Some(ConditionCode::Nc),
        );
        assert_eq!(decode_relative_jump_condition(0x38), Some(ConditionCode::C),);
        assert_eq!(decode_absolute_jump_condition(0xC3), None);
        assert_eq!(
            decode_absolute_jump_condition(0xC2),
            Some(ConditionCode::Nz),
        );
        assert_eq!(decode_absolute_jump_condition(0xCA), Some(ConditionCode::Z),);
        assert_eq!(
            decode_absolute_jump_condition(0xD2),
            Some(ConditionCode::Nc),
        );
        assert_eq!(decode_absolute_jump_condition(0xDA), Some(ConditionCode::C),);
        assert_eq!(decode_call_condition(0xCD), None);
        assert_eq!(decode_call_condition(0xC4), Some(ConditionCode::Nz));
        assert_eq!(decode_call_condition(0xCC), Some(ConditionCode::Z));
        assert_eq!(decode_call_condition(0xD4), Some(ConditionCode::Nc));
        assert_eq!(decode_call_condition(0xDC), Some(ConditionCode::C));
        assert_eq!(decode_return_condition(0xC9), None);
        assert_eq!(decode_return_condition(0xC0), Some(ConditionCode::Nz));
        assert_eq!(decode_return_condition(0xC8), Some(ConditionCode::Z));
        assert_eq!(decode_return_condition(0xD0), Some(ConditionCode::Nc));
        assert_eq!(decode_return_condition(0xD8), Some(ConditionCode::C));
        assert_eq!(
            decode_hl_update_direction(0x22),
            CpuAddressUpdateDirection::Increment,
        );
        assert_eq!(
            decode_hl_update_direction(0x2A),
            CpuAddressUpdateDirection::Increment,
        );
        assert_eq!(
            decode_hl_update_direction(0x32),
            CpuAddressUpdateDirection::Decrement,
        );
        assert_eq!(
            decode_hl_update_direction(0x3A),
            CpuAddressUpdateDirection::Decrement,
        );

        cpu.write_register16(Register16::DE, 0x1234);
        cpu.write_register16(Register16::HL, 0xABCD);
        assert_eq!(cpu.de(), 0x1234);
        assert_eq!(cpu.hl(), 0xABCD);
        assert_eq!(cpu.read_stack_register16(StackRegister16::DE), 0x1234);
        assert_eq!(cpu.read_stack_register16(StackRegister16::HL), 0xABCD);
        cpu.write_stack_register16(StackRegister16::HL, 0x5678);
        assert_eq!(cpu.hl(), 0x5678);

        cpu.registers.sp = 0xC000;
        assert_eq!(
            cpu.increment_or_decrement_register16(
                Register16::SP,
                CpuAddressUpdateDirection::Increment,
            ),
            0xC001,
        );
        assert_eq!(cpu.registers.sp, 0xC001);
        assert_eq!(
            cpu.increment_or_decrement_register16(
                Register16::DE,
                CpuAddressUpdateDirection::Decrement,
            ),
            0x1233,
        );
        assert_eq!(cpu.de(), 0x1233);

        cpu.registers.f = 0;
        assert!(cpu.condition_is_met(Some(ConditionCode::Nc)));
        cpu.registers.f = FLAG_C;
        assert!(cpu.condition_is_met(Some(ConditionCode::C)));

        cpu.registers.f = FLAG_C;
        cpu.update_dec_flags(0x10, 0x0F);
        assert_eq!(cpu.registers.f, FLAG_N | FLAG_H | FLAG_C);
        cpu.registers.f = 0;
        cpu.update_dec_flags(0x01, 0x00);
        assert_eq!(cpu.registers.f, FLAG_Z | FLAG_N);

        cpu.registers.a = 0xFF;
        cpu.registers.f = FLAG_C;
        cpu.adc_to_a(0x00);
        assert_eq!(cpu.registers.a, 0x00);
        assert_eq!(cpu.registers.f, FLAG_Z | FLAG_H | FLAG_C);

        cpu.registers.a = 0x10;
        cpu.sub_from_a(0x01);
        assert_eq!(cpu.registers.a, 0x0F);
        assert_eq!(cpu.registers.f, FLAG_N | FLAG_H);

        cpu.registers.a = 0x00;
        cpu.registers.f = FLAG_C;
        cpu.sbc_from_a(0x00);
        assert_eq!(cpu.registers.a, 0xFF);
        assert_eq!(cpu.registers.f, FLAG_N | FLAG_H | FLAG_C);

        cpu.registers.a = 0xF0;
        cpu.and_with_a(0x0F);
        assert_eq!(cpu.registers.a, 0x00);
        assert_eq!(cpu.registers.f, FLAG_Z | FLAG_H);

        cpu.registers.a = 0xFF;
        cpu.xor_with_a(0x0F);
        assert_eq!(cpu.registers.a, 0xF0);
        assert_eq!(cpu.registers.f, 0);

        cpu.registers.a = 0x00;
        cpu.or_with_a(0x00);
        assert_eq!(cpu.registers.a, 0x00);
        assert_eq!(cpu.registers.f, FLAG_Z);

        cpu.registers.a = 0x01;
        cpu.compare_a(0x01);
        assert_eq!(cpu.registers.a, 0x01);
        assert_eq!(cpu.registers.f, FLAG_Z | FLAG_N);

        cpu.write_register16(Register16::HL, 0xFFFF);
        cpu.registers.f = FLAG_Z;
        cpu.add_to_hl(0x0001);
        assert_eq!(cpu.hl(), 0x0000);
        assert_eq!(cpu.registers.f, FLAG_Z | FLAG_H | FLAG_C);

        cpu.registers.a = 0x55;
        cpu.registers.f = FLAG_Z | FLAG_C;
        cpu.complement_a();
        assert_eq!(cpu.registers.a, 0xAA);
        assert_eq!(cpu.registers.f, FLAG_Z | FLAG_N | FLAG_H | FLAG_C);

        cpu.registers.f = FLAG_Z;
        cpu.set_carry_flag();
        assert_eq!(cpu.registers.f, FLAG_Z | FLAG_C);

        cpu.registers.f = FLAG_Z | FLAG_C;
        cpu.complement_carry_flag();
        assert_eq!(cpu.registers.f, FLAG_Z);

        cpu.registers.a = 0x01;
        assert!(matches!(
            cpu.decode_fetched_opcode(0x0F),
            DecodedOpcode::Complete
        ));
        assert_eq!(cpu.registers.a, 0x80);
        assert_eq!(cpu.registers.f, FLAG_C);

        cpu.registers.a = 0x80;
        cpu.registers.f = FLAG_C;
        assert!(matches!(
            cpu.decode_fetched_opcode(0x1F),
            DecodedOpcode::Complete
        ));
        assert_eq!(cpu.registers.a, 0xC0);
        assert_eq!(cpu.registers.f, 0);

        assert!(matches!(
            cpu.decode_fetched_opcode(0x10),
            DecodedOpcode::Execute(CpuInstructionKind::Stop)
        ));
        assert!(matches!(
            cpu.decode_fetched_opcode(0x02),
            DecodedOpcode::Execute(CpuInstructionKind::StoreAToAddress {
                destination: MemoryAddressSource::BC,
            })
        ));
        assert!(matches!(
            cpu.decode_fetched_opcode(0x12),
            DecodedOpcode::Execute(CpuInstructionKind::StoreAToAddress {
                destination: MemoryAddressSource::DE,
            })
        ));
        assert!(matches!(
            cpu.decode_fetched_opcode(0xEA),
            DecodedOpcode::Execute(CpuInstructionKind::StoreAToAddress {
                destination: MemoryAddressSource::Immediate16,
            })
        ));
        assert!(matches!(
            cpu.decode_fetched_opcode(0xE0),
            DecodedOpcode::Execute(CpuInstructionKind::StoreAToAddress {
                destination: MemoryAddressSource::HighImmediate8,
            })
        ));
        assert!(matches!(
            cpu.decode_fetched_opcode(0xE2),
            DecodedOpcode::Execute(CpuInstructionKind::StoreAToAddress {
                destination: MemoryAddressSource::HighC,
            })
        ));
        assert!(matches!(
            cpu.decode_fetched_opcode(0x0A),
            DecodedOpcode::Execute(CpuInstructionKind::LoadAFromAddress {
                source: MemoryAddressSource::BC,
            })
        ));
        assert!(matches!(
            cpu.decode_fetched_opcode(0x1A),
            DecodedOpcode::Execute(CpuInstructionKind::LoadAFromAddress {
                source: MemoryAddressSource::DE,
            })
        ));
        assert!(matches!(
            cpu.decode_fetched_opcode(0xFA),
            DecodedOpcode::Execute(CpuInstructionKind::LoadAFromAddress {
                source: MemoryAddressSource::Immediate16,
            })
        ));
        assert!(matches!(
            cpu.decode_fetched_opcode(0xF0),
            DecodedOpcode::Execute(CpuInstructionKind::LoadAFromAddress {
                source: MemoryAddressSource::HighImmediate8,
            })
        ));
        assert!(matches!(
            cpu.decode_fetched_opcode(0xF2),
            DecodedOpcode::Execute(CpuInstructionKind::LoadAFromAddress {
                source: MemoryAddressSource::HighC,
            })
        ));
        assert!(matches!(
            cpu.decode_fetched_opcode(0x08),
            DecodedOpcode::Execute(CpuInstructionKind::StoreSpToImmediate16)
        ));
        assert!(matches!(
            cpu.decode_fetched_opcode(0xF8),
            DecodedOpcode::Execute(CpuInstructionKind::LoadHlFromSpPlusImmediate)
        ));
        assert!(matches!(
            cpu.decode_fetched_opcode(0xE8),
            DecodedOpcode::Execute(CpuInstructionKind::AddSpImmediate)
        ));

        let mut pending_mask_queries = 0;
        assert_eq!(
            cpu.current_highest_pending_interrupt(&mut |operation| {
                pending_mask_queries += 1;
                assert_eq!(operation, CpuBusOperation::PendingInterruptMask);
                Some(0x10)
            }),
            Some(InterruptSource::Joypad),
        );
        assert_eq!(pending_mask_queries, 1);

        let rlc = cpu.decode_cb_opcode(0x07).expect("RLC A should decode");
        assert_eq!(rlc.target(), Register8Operand::Register(Register8::A));
        assert_eq!(cpu.apply_cb_operation(rlc, 0x80), Some(0x01));
        assert_eq!(cpu.registers.f, FLAG_C);

        let rrc = cpu.decode_cb_opcode(0x08).expect("RRC B should decode");
        assert_eq!(cpu.apply_cb_operation(rrc, 0x01), Some(0x80));
        assert_eq!(cpu.registers.f, FLAG_C);

        let sla = cpu.decode_cb_opcode(0x20).expect("SLA B should decode");
        assert_eq!(cpu.apply_cb_operation(sla, 0x81), Some(0x02));
        assert_eq!(cpu.registers.f, FLAG_C);

        let sra = cpu.decode_cb_opcode(0x28).expect("SRA B should decode");
        assert_eq!(cpu.apply_cb_operation(sra, 0x81), Some(0xC0));
        assert_eq!(cpu.registers.f, FLAG_C);

        let swap = cpu.decode_cb_opcode(0x30).expect("SWAP B should decode");
        assert_eq!(cpu.apply_cb_operation(swap, 0xF0), Some(0x0F));
        assert_eq!(cpu.registers.f, 0);

        let srl = cpu.decode_cb_opcode(0x38).expect("SRL B should decode");
        assert_eq!(cpu.apply_cb_operation(srl, 0x01), Some(0x00));
        assert_eq!(cpu.registers.f, FLAG_Z | FLAG_C);

        cpu.registers.f = FLAG_C;
        let bit = cpu.decode_cb_opcode(0x58).expect("BIT 3,B should decode");
        assert_eq!(cpu.apply_cb_operation(bit, 0x00), None);
        assert_eq!(cpu.registers.f, FLAG_Z | FLAG_H | FLAG_C);

        let reset = cpu.decode_cb_opcode(0x80).expect("RES 0,B should decode");
        assert_eq!(cpu.apply_cb_operation(reset, 0xFF), Some(0xFE));

        let set = cpu.decode_cb_opcode(0xFF).expect("SET 7,A should decode");
        assert_eq!(set.target(), Register8Operand::Register(Register8::A));
        assert_eq!(cpu.apply_cb_operation(set, 0x00), Some(0x80));
    }

    #[test]
    fn private_execute_machine_cycle_paths_cover_remaining_decoder_and_invariant_regions() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        cpu.complete_execute_machine_cycle(0xAA, 2, &mut |_| None);
        assert_eq!(
            cpu.execution_state,
            CpuExecutionState::Execute {
                opcode: 0xAA,
                step: 2,
                t_cycle: LAST_MACHINE_CYCLE_T,
            },
        );

        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        cpu.apply_startup_state(CpuStartupState::power_on_reset());
        cpu.write_register16(Register16::HL, 0x0001);
        cpu.registers.sp = 0x0001;
        cpu.instruction_kind = Some(CpuInstructionKind::AddHl {
            source: Register16::SP,
        });
        cpu.complete_execute_machine_cycle(0x39, 0, &mut |_| None);
        assert_eq!(cpu.hl(), 0x0002);
        assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        cpu.apply_startup_state(CpuStartupState::power_on_reset());
        cpu.write_register16(Register16::BC, 0x0001);
        cpu.write_register16(Register16::HL, 0x0001);
        cpu.instruction_kind = Some(CpuInstructionKind::AddHl {
            source: Register16::BC,
        });
        cpu.complete_execute_machine_cycle(0x09, 0, &mut |_| None);
        assert_eq!(cpu.hl(), 0x0002);

        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        cpu.apply_startup_state(CpuStartupState::power_on_reset());
        cpu.write_register16(Register16::DE, 0x0002);
        cpu.write_register16(Register16::HL, 0x0001);
        cpu.instruction_kind = Some(CpuInstructionKind::AddHl {
            source: Register16::DE,
        });
        cpu.complete_execute_machine_cycle(0x19, 0, &mut |_| None);
        assert_eq!(cpu.hl(), 0x0003);

        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        cpu.apply_startup_state(CpuStartupState::power_on_reset());
        cpu.write_register16(Register16::HL, 0xC000);
        cpu.instruction_kind = Some(CpuInstructionKind::DecrementHlMemory);
        cpu.complete_execute_machine_cycle(0x35, 0, &mut |_| Some(0x10));
        cpu.complete_execute_machine_cycle(0x35, 1, &mut |_| None);
        assert_eq!(cpu.registers.f, FLAG_N | FLAG_H);
        assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        cpu.apply_startup_state(CpuStartupState::power_on_reset());
        cpu.registers.a = 0x0F;
        cpu.instruction_kind = Some(CpuInstructionKind::AluImmediate {
            operation: AluOperation::Add,
        });
        cpu.complete_execute_machine_cycle(0xC6, 0, &mut |_| Some(0x01));
        assert_eq!(cpu.registers.a, 0x10);
        assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        cpu.apply_startup_state(CpuStartupState::power_on_reset());
        cpu.registers.a = 0xF0;
        cpu.write_register16(Register16::HL, 0xC100);
        cpu.instruction_kind = Some(CpuInstructionKind::AluFromHl {
            operation: AluOperation::And,
        });
        cpu.complete_execute_machine_cycle(0xA6, 0, &mut |_| Some(0x0F));
        assert_eq!(cpu.registers.a, 0x00);
        assert_eq!(cpu.registers.f, FLAG_Z | FLAG_H);
        assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        cpu.apply_startup_state(CpuStartupState::power_on_reset());
        cpu.registers.f = 0;
        cpu.operand16_latch = 0x0034;
        cpu.instruction_kind = Some(CpuInstructionKind::Call {
            condition: Some(ConditionCode::Z),
        });
        cpu.complete_execute_machine_cycle(0xCC, 1, &mut |_| Some(0x12));
        assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        cpu.apply_startup_state(CpuStartupState::power_on_reset());
        cpu.registers.f = FLAG_Z;
        cpu.registers.sp = 0xC000;
        cpu.instruction_kind = Some(CpuInstructionKind::Return {
            condition: Some(ConditionCode::Z),
        });
        cpu.complete_execute_machine_cycle(0xC8, 0, &mut |_| None);
        cpu.complete_execute_machine_cycle(0xC8, 1, &mut |_| Some(0x78));
        cpu.complete_execute_machine_cycle(0xC8, 2, &mut |_| Some(0x56));
        cpu.complete_execute_machine_cycle(0xC8, 3, &mut |_| None);
        assert_eq!(cpu.registers.pc, 0x5678);
        assert_eq!(cpu.registers.sp, 0xC002);
        assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        cpu.apply_startup_state(CpuStartupState::power_on_reset());
        cpu.registers.f = 0;
        cpu.instruction_kind = Some(CpuInstructionKind::Return {
            condition: Some(ConditionCode::Z),
        });
        cpu.complete_execute_machine_cycle(0xC8, 0, &mut |_| None);
        assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        cpu.apply_startup_state(CpuStartupState::power_on_reset());
        cpu.registers.a = 0x81;
        assert!(matches!(
            cpu.decode_fetched_opcode(0x07),
            DecodedOpcode::Complete
        ));
        assert_eq!(cpu.registers.a, 0x03);
        assert_eq!(cpu.registers.f, FLAG_C);

        cpu.registers.a = 0x80;
        cpu.registers.f = FLAG_C;
        assert!(matches!(
            cpu.decode_fetched_opcode(0x17),
            DecodedOpcode::Complete
        ));
        assert_eq!(cpu.registers.a, 0x01);
        assert_eq!(cpu.registers.f, FLAG_C);

        cpu.registers.f = 0;
        cpu.registers.d = 0x01;
        assert!(matches!(
            cpu.decode_fetched_opcode(0x15),
            DecodedOpcode::Complete
        ));
        assert_eq!(cpu.registers.d, 0x00);
        assert_eq!(cpu.registers.f, FLAG_Z | FLAG_N);
        assert!(matches!(
            cpu.decode_fetched_opcode(0x35),
            DecodedOpcode::Execute(CpuInstructionKind::DecrementHlMemory)
        ));

        cpu.operand16_latch = 0xBEEF;
        assert_eq!(
            cpu.resolve_memory_address(MemoryAddressSource::Immediate16),
            0xBEEF,
        );

        cpu.write_register16(Register16::HL, 0xC123);
        cpu.instruction_kind = Some(CpuInstructionKind::CbPrefixed);
        cpu.cb_instruction_kind = Some(CbInstructionKind::BitTest {
            bit: 0,
            target: Register8Operand::IndirectHl,
        });
        cpu.complete_execute_machine_cycle(0xCB, 1, &mut |_| Some(0x00));
        assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());

        assert!(std::panic::catch_unwind(|| decode_register16(4)).is_err());
        assert!(std::panic::catch_unwind(|| decode_register8_operand(8)).is_err());
        assert!(std::panic::catch_unwind(|| decode_stack_register16(4)).is_err());
        assert!(std::panic::catch_unwind(|| decode_alu_operation(8)).is_err());
        assert!(std::panic::catch_unwind(|| decode_relative_jump_condition(0x00)).is_err());
        assert!(std::panic::catch_unwind(|| decode_absolute_jump_condition(0x00)).is_err());
        assert!(std::panic::catch_unwind(|| decode_call_condition(0x00)).is_err());
        assert!(std::panic::catch_unwind(|| decode_return_condition(0x00)).is_err());
        assert!(std::panic::catch_unwind(|| decode_hl_update_direction(0x00)).is_err());
    }

    #[test]
    fn unexpected_machine_steps_stall_in_place_instead_of_mutating_cpu_state() {
        let cases = [
            (
                0x06,
                CpuInstructionKind::LoadRegisterImmediate {
                    target: Register8::B,
                },
                1,
            ),
            (
                0x01,
                CpuInstructionKind::LoadRegisterPairImmediate {
                    target: Register16::BC,
                },
                2,
            ),
            (
                0x46,
                CpuInstructionKind::LoadRegisterFromHl {
                    target: Register8::B,
                },
                1,
            ),
            (
                0x70,
                CpuInstructionKind::StoreRegisterToHl {
                    source: Register8::B,
                },
                1,
            ),
            (0x36, CpuInstructionKind::StoreImmediateToHl, 2),
            (
                0x2A,
                CpuInstructionKind::LoadAFromHlWithUpdate {
                    direction: CpuAddressUpdateDirection::Increment,
                },
                1,
            ),
            (
                0x32,
                CpuInstructionKind::StoreAToHlWithUpdate {
                    direction: CpuAddressUpdateDirection::Decrement,
                },
                1,
            ),
            (
                0xFA,
                CpuInstructionKind::LoadAFromAddress {
                    source: MemoryAddressSource::Immediate16,
                },
                3,
            ),
            (
                0xEA,
                CpuInstructionKind::StoreAToAddress {
                    destination: MemoryAddressSource::Immediate16,
                },
                3,
            ),
            (0x08, CpuInstructionKind::StoreSpToImmediate16, 4),
            (0xF8, CpuInstructionKind::LoadHlFromSpPlusImmediate, 2),
            (0xE8, CpuInstructionKind::AddSpImmediate, 3),
            (0xF9, CpuInstructionKind::LoadSpFromHl, 1),
            (
                0x39,
                CpuInstructionKind::AddHl {
                    source: Register16::SP,
                },
                1,
            ),
            (
                0x33,
                CpuInstructionKind::IncrementRegisterPair {
                    target: Register16::SP,
                },
                1,
            ),
            (
                0x3B,
                CpuInstructionKind::DecrementRegisterPair {
                    target: Register16::SP,
                },
                1,
            ),
            (0x34, CpuInstructionKind::IncrementHlMemory, 2),
            (
                0xFE,
                CpuInstructionKind::AluImmediate {
                    operation: AluOperation::Compare,
                },
                1,
            ),
            (
                0xB6,
                CpuInstructionKind::AluFromHl {
                    operation: AluOperation::Or,
                },
                1,
            ),
            (
                0x38,
                CpuInstructionKind::RelativeJump {
                    condition: Some(ConditionCode::C),
                },
                2,
            ),
            (
                0xDA,
                CpuInstructionKind::AbsoluteJump {
                    condition: Some(ConditionCode::C),
                },
                3,
            ),
            (
                0xDC,
                CpuInstructionKind::Call {
                    condition: Some(ConditionCode::C),
                },
                5,
            ),
            (
                0xD8,
                CpuInstructionKind::Return {
                    condition: Some(ConditionCode::C),
                },
                4,
            ),
            (0xD9, CpuInstructionKind::ReturnFromInterrupt, 3),
            (0x10, CpuInstructionKind::Stop, 1),
            (0xDF, CpuInstructionKind::Restart { vector: 0x0018 }, 3),
            (
                0xF5,
                CpuInstructionKind::PushRegisterPair {
                    source: StackRegister16::AF,
                },
                3,
            ),
            (
                0xF1,
                CpuInstructionKind::PopRegisterPair {
                    target: StackRegister16::AF,
                },
                2,
            ),
            (0xCB, CpuInstructionKind::CbPrefixed, 3),
        ];

        for (opcode, kind, step) in cases {
            let mut cpu = CpuCore::new(ConsoleModel::Dmg);
            cpu.instruction_kind = Some(kind);
            cpu.execution_state = CpuExecutionState::Execute {
                opcode,
                step,
                t_cycle: 0,
            };

            cpu.complete_execute_machine_cycle(opcode, step, &mut |_| Some(0xFF));

            assert_eq!(
                cpu.execution_state,
                CpuExecutionState::Execute {
                    opcode,
                    step,
                    t_cycle: LAST_MACHINE_CYCLE_T,
                },
            );
        }

        for step in [1_u8, 2, 3] {
            let mut cpu = CpuCore::new(ConsoleModel::Dmg);
            cpu.instruction_kind = Some(CpuInstructionKind::CbPrefixed);
            cpu.execution_state = CpuExecutionState::Execute {
                opcode: 0xCB,
                step,
                t_cycle: 0,
            };
            cpu.cb_instruction_kind = None;

            cpu.complete_execute_machine_cycle(0xCB, step, &mut |_| Some(0xFF));

            assert_eq!(
                cpu.execution_state,
                CpuExecutionState::Execute {
                    opcode: 0xCB,
                    step,
                    t_cycle: LAST_MACHINE_CYCLE_T,
                },
            );
        }

        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        let acknowledged =
            cpu.complete_interrupt_service_machine_cycle(InterruptSource::Serial, 5, &mut |_| None);
        assert_eq!(acknowledged, None);
        assert_eq!(
            cpu.execution_state,
            CpuExecutionState::ServiceInterrupt {
                source: InterruptSource::Serial,
                step: 5,
                t_cycle: 0,
            },
        );
    }

    #[test]
    fn startup_state_resets_live_registers_and_fetch_state() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        let startup_state = CpuStartupState {
            a: 0x01,
            f: 0xB0,
            b: 0x00,
            c: 0x13,
            d: 0x00,
            e: 0xD8,
            h: 0x01,
            l: 0x4D,
            sp: 0xFFFE,
            pc: 0x0100,
        };

        cpu.apply_startup_state(startup_state);

        assert_eq!(cpu.status(), CpuStatus::Ready);
        assert_eq!(cpu.startup_state(), startup_state);
        assert_eq!(
            cpu.registers(),
            CpuRegisters::from_startup_state(startup_state)
        );
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
        assert_eq!(cpu.current_opcode(), None);
        assert!(!cpu.ime());
        assert!(!cpu.delayed_ime_enable());
    }

    #[test]
    fn opcode_fetch_reads_bus_at_pc_on_the_fourth_t_cycle() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut cartridge = build_test_cartridge(&[0xCB]);

        cpu.apply_startup_state(CpuStartupState {
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::Execute {
                opcode: 0xCB,
                step: 0,
                t_cycle: 0,
            }
        );
        assert_eq!(cpu.registers().pc, 0x0101);
        assert_eq!(cpu.current_opcode(), Some(0xCB));
        assert_eq!(
            cpu.last_address_event(),
            Some(CpuAddressEvent {
                kind: CpuAddressEventKind::ReadWithIncDec,
                access_address: Some(0x0100),
                idu_address: Some(0x0101),
                update_direction: Some(CpuAddressUpdateDirection::Increment),
            })
        );
    }

    #[test]
    fn unsupported_opcode_enters_an_explicit_diagnostic_trap() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut cartridge = build_test_cartridge(&[0xD3]);

        cpu.apply_startup_state(CpuStartupState {
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

        assert_eq!(cpu.registers().pc, 0x0101);
        assert_eq!(cpu.current_opcode(), Some(0xD3));
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::DiagnosticTrap {
                trap: CpuDiagnosticTrap::UnsupportedOpcode {
                    opcode: 0xD3,
                    address: 0x0100,
                },
            }
        );
        assert_eq!(
            cpu.last_address_event(),
            Some(CpuAddressEvent {
                kind: CpuAddressEventKind::ReadWithIncDec,
                access_address: Some(0x0100),
                idu_address: Some(0x0101),
                update_direction: Some(CpuAddressUpdateDirection::Increment),
            })
        );

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 8);

        assert_eq!(cpu.registers().pc, 0x0101);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::DiagnosticTrap {
                trap: CpuDiagnosticTrap::UnsupportedOpcode {
                    opcode: 0xD3,
                    address: 0x0100,
                },
            }
        );
        assert_eq!(cpu.last_address_event(), None);
    }

    #[test]
    fn cb_set_opcode_executes_instead_of_entering_a_diagnostic_trap() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut cartridge = build_test_cartridge(&[0xCB, 0xFF]);

        cpu.apply_startup_state(CpuStartupState {
            f: FLAG_Z | FLAG_C,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 8);

        assert_eq!(cpu.registers().pc, 0x0102);
        assert_eq!(cpu.registers().a, 0x80);
        assert_eq!(cpu.registers().f, FLAG_Z | FLAG_C);
        assert_eq!(cpu.current_opcode(), None);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
        assert_eq!(
            cpu.last_address_event(),
            Some(CpuAddressEvent {
                kind: CpuAddressEventKind::ReadWithIncDec,
                access_address: Some(0x0101),
                idu_address: Some(0x0102),
                update_direction: Some(CpuAddressUpdateDirection::Increment),
            })
        );
    }

    #[test]
    fn hli_and_hld_transfer_forms_publish_combined_access_and_idu_events() {
        let mut load_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut load_bus = Bus::new(ConsoleModel::Dmg);
        let mut load_cartridge = build_test_cartridge(&[0x2A]);

        load_cpu.apply_startup_state(CpuStartupState {
            h: 0xC0,
            l: 0x00,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });
        load_bus.write(0xC000, 0x77);

        tick_cpu_n(&mut load_cpu, &mut load_bus, &mut load_cartridge, 8);

        assert_eq!(load_cpu.registers().a, 0x77);
        assert_eq!(load_cpu.hl(), 0xC001);
        assert_eq!(
            load_cpu.last_address_event(),
            Some(CpuAddressEvent {
                kind: CpuAddressEventKind::ReadWithIncDec,
                access_address: Some(0xC000),
                idu_address: Some(0xC001),
                update_direction: Some(CpuAddressUpdateDirection::Increment),
            })
        );

        let mut store_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut store_bus = Bus::new(ConsoleModel::Dmg);
        let mut store_cartridge = build_test_cartridge(&[0x32]);

        store_cpu.apply_startup_state(CpuStartupState {
            a: 0x5A,
            h: 0xC0,
            l: 0x01,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(&mut store_cpu, &mut store_bus, &mut store_cartridge, 8);

        assert_eq!(store_bus.read(0xC001), 0x5A);
        assert_eq!(store_cpu.hl(), 0xC000);
        assert_eq!(
            store_cpu.last_address_event(),
            Some(CpuAddressEvent {
                kind: CpuAddressEventKind::WriteWithIncDec,
                access_address: Some(0xC001),
                idu_address: Some(0xC000),
                update_direction: Some(CpuAddressUpdateDirection::Decrement),
            })
        );
    }

    #[test]
    fn inc_and_dec_register_pairs_publish_pure_idu_events() {
        let mut inc_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut inc_bus = Bus::new(ConsoleModel::Dmg);
        let mut inc_cartridge = build_test_cartridge(&[0x23]);

        inc_cpu.apply_startup_state(CpuStartupState {
            h: 0xFE,
            l: 0xFF,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(&mut inc_cpu, &mut inc_bus, &mut inc_cartridge, 8);

        assert_eq!(inc_cpu.hl(), 0xFF00);
        assert_eq!(
            inc_cpu.last_address_event(),
            Some(CpuAddressEvent {
                kind: CpuAddressEventKind::IncDec,
                access_address: None,
                idu_address: Some(0xFF00),
                update_direction: Some(CpuAddressUpdateDirection::Increment),
            })
        );

        let mut dec_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut dec_bus = Bus::new(ConsoleModel::Dmg);
        let mut dec_cartridge = build_test_cartridge(&[0x3B]);

        dec_cpu.apply_startup_state(CpuStartupState {
            sp: 0xFE00,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(&mut dec_cpu, &mut dec_bus, &mut dec_cartridge, 8);

        assert_eq!(dec_cpu.registers().sp, 0xFDFF);
        assert_eq!(
            dec_cpu.last_address_event(),
            Some(CpuAddressEvent {
                kind: CpuAddressEventKind::IncDec,
                access_address: None,
                idu_address: Some(0xFDFF),
                update_direction: Some(CpuAddressUpdateDirection::Decrement),
            })
        );
    }

    #[test]
    fn register_only_and_hl_indirect_loads_have_distinct_timing_paths() {
        let startup_state = CpuStartupState {
            b: 0x12,
            c: 0x34,
            h: 0xC0,
            l: 0x00,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        };

        let mut register_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut register_bus = Bus::new(ConsoleModel::Dmg);
        let mut register_cartridge = build_test_cartridge(&[0x41]);
        register_cpu.apply_startup_state(startup_state);

        tick_cpu_n(
            &mut register_cpu,
            &mut register_bus,
            &mut register_cartridge,
            4,
        );

        assert_eq!(register_cpu.registers().b, 0x34);
        assert_eq!(
            register_cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );

        let mut hl_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut hl_bus = Bus::new(ConsoleModel::Dmg);
        let mut hl_cartridge = build_test_cartridge(&[0x46]);
        hl_cpu.apply_startup_state(startup_state);
        hl_bus.write(0xC000, 0x77);

        tick_cpu_n(&mut hl_cpu, &mut hl_bus, &mut hl_cartridge, 4);

        assert_eq!(hl_cpu.registers().b, 0x12);
        assert_eq!(
            hl_cpu.execution_state(),
            CpuExecutionState::Execute {
                opcode: 0x46,
                step: 0,
                t_cycle: 0,
            }
        );

        tick_cpu_n(&mut hl_cpu, &mut hl_bus, &mut hl_cartridge, 4);

        assert_eq!(hl_cpu.registers().b, 0x77);
        assert_eq!(hl_cpu.registers().pc, 0x0101);
        assert_eq!(
            hl_cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
    }

    #[test]
    fn ld_bc_d16_fetches_low_then_high_in_order() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut cartridge = build_test_cartridge(&[0x01, 0x34, 0x12]);

        cpu.apply_startup_state(CpuStartupState {
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 8);

        assert_eq!(cpu.registers().b, 0x00);
        assert_eq!(cpu.registers().c, 0x00);
        assert_eq!(cpu.registers().pc, 0x0102);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::Execute {
                opcode: 0x01,
                step: 1,
                t_cycle: 0,
            }
        );

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

        assert_eq!(cpu.registers().b, 0x12);
        assert_eq!(cpu.registers().c, 0x34);
        assert_eq!(cpu.registers().pc, 0x0103);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
    }

    #[test]
    fn ld_hl_d8_fetches_the_immediate_before_writing_memory() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut cartridge = build_test_cartridge(&[0x36, 0x5A]);

        cpu.apply_startup_state(CpuStartupState {
            h: 0xC0,
            l: 0x00,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 8);

        assert_eq!(cpu.registers().pc, 0x0102);
        assert_eq!(bus.read(0xC000), 0x00);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::Execute {
                opcode: 0x36,
                step: 1,
                t_cycle: 0,
            }
        );

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

        assert_eq!(bus.read(0xC000), 0x5A);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
    }

    #[test]
    fn inc_hl_uses_distinct_read_and_write_machine_cycles() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut cartridge = build_test_cartridge(&[0x34]);

        cpu.apply_startup_state(CpuStartupState {
            h: 0xC0,
            l: 0x00,
            f: FLAG_C,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });
        bus.write(0xC000, 0x0F);

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 8);

        assert_eq!(bus.read(0xC000), 0x0F);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::Execute {
                opcode: 0x34,
                step: 1,
                t_cycle: 0,
            }
        );

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

        assert_eq!(bus.read(0xC000), 0x10);
        assert_eq!(cpu.registers().f, FLAG_H | FLAG_C);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
    }

    #[test]
    fn add_a_d8_updates_flags_from_the_fetched_immediate() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut cartridge = build_test_cartridge(&[0xC6, 0x01]);

        cpu.apply_startup_state(CpuStartupState {
            a: 0x0F,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 8);

        assert_eq!(cpu.registers().a, 0x10);
        assert_eq!(cpu.registers().f, FLAG_H);
        assert_eq!(cpu.registers().pc, 0x0102);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
    }

    #[test]
    fn ld_hl_sp_plus_signed_immediate_uses_three_machine_cycles_and_sets_flags_from_sp_math() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut cartridge = build_test_cartridge(&[0xF8, 0x08]);

        cpu.apply_startup_state(CpuStartupState {
            sp: 0xFFF8,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 8);

        assert_eq!(cpu.registers().pc, 0x0102);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::Execute {
                opcode: 0xF8,
                step: 1,
                t_cycle: 0,
            }
        );

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

        assert_eq!(cpu.hl(), 0x0000);
        assert_eq!(cpu.registers().sp, 0xFFF8);
        assert_eq!(cpu.registers().f, FLAG_H | FLAG_C);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
    }

    #[test]
    fn add_sp_signed_immediate_uses_four_machine_cycles_and_sets_flags_from_sp_math() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut cartridge = build_test_cartridge(&[0xE8, 0x08]);

        cpu.apply_startup_state(CpuStartupState {
            sp: 0xFFF8,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 8);

        assert_eq!(cpu.registers().pc, 0x0102);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::Execute {
                opcode: 0xE8,
                step: 1,
                t_cycle: 0,
            }
        );

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

        assert_eq!(cpu.registers().sp, 0xFFF8);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::Execute {
                opcode: 0xE8,
                step: 2,
                t_cycle: 0,
            }
        );

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

        assert_eq!(cpu.registers().sp, 0x0000);
        assert_eq!(cpu.registers().f, FLAG_H | FLAG_C);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
    }

    #[test]
    fn ld_a16_sp_writes_sp_little_endian_over_five_machine_cycles() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut cartridge = build_test_cartridge(&[0x08, 0x00, 0xC0]);

        cpu.apply_startup_state(CpuStartupState {
            sp: 0x1234,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 12);

        assert_eq!(bus.read(0xC000), 0x00);
        assert_eq!(bus.read(0xC001), 0x00);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::Execute {
                opcode: 0x08,
                step: 2,
                t_cycle: 0,
            }
        );

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

        assert_eq!(bus.read(0xC000), 0x34);
        assert_eq!(bus.read(0xC001), 0x00);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::Execute {
                opcode: 0x08,
                step: 3,
                t_cycle: 0,
            }
        );

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

        assert_eq!(bus.read(0xC000), 0x34);
        assert_eq!(bus.read(0xC001), 0x12);
        assert_eq!(cpu.registers().pc, 0x0103);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
    }

    #[test]
    fn ld_sp_hl_uses_two_machine_cycles_and_preserves_flags() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut cartridge = build_test_cartridge(&[0xF9]);

        cpu.apply_startup_state(CpuStartupState {
            h: 0xC1,
            l: 0x23,
            f: FLAG_Z | FLAG_C,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 8);

        assert_eq!(cpu.registers().sp, 0xC123);
        assert_eq!(cpu.registers().f, FLAG_Z | FLAG_C);
        assert_eq!(cpu.registers().pc, 0x0101);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
    }

    #[test]
    fn jr_nz_taken_and_untaken_use_different_temporal_sequences() {
        let startup_taken = CpuStartupState {
            f: 0x00,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        };
        let startup_untaken = CpuStartupState {
            f: FLAG_Z,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        };

        let mut taken_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut taken_bus = Bus::new(ConsoleModel::Dmg);
        let mut taken_cartridge = build_test_cartridge(&[0x20, 0x02]);
        taken_cpu.apply_startup_state(startup_taken);

        tick_cpu_n(&mut taken_cpu, &mut taken_bus, &mut taken_cartridge, 8);

        assert_eq!(taken_cpu.registers().pc, 0x0102);
        assert_eq!(
            taken_cpu.execution_state(),
            CpuExecutionState::Execute {
                opcode: 0x20,
                step: 1,
                t_cycle: 0,
            }
        );

        tick_cpu_n(&mut taken_cpu, &mut taken_bus, &mut taken_cartridge, 4);

        assert_eq!(taken_cpu.registers().pc, 0x0104);
        assert_eq!(
            taken_cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );

        let mut untaken_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut untaken_bus = Bus::new(ConsoleModel::Dmg);
        let mut untaken_cartridge = build_test_cartridge(&[0x20, 0x02]);
        untaken_cpu.apply_startup_state(startup_untaken);

        tick_cpu_n(
            &mut untaken_cpu,
            &mut untaken_bus,
            &mut untaken_cartridge,
            8,
        );

        assert_eq!(untaken_cpu.registers().pc, 0x0102);
        assert_eq!(
            untaken_cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
    }

    #[test]
    fn call_and_ret_use_bytewise_stack_transfers_in_order() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut cartridge =
            build_test_cartridge(&[0xCD, 0x08, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC9]);

        cpu.apply_startup_state(CpuStartupState {
            sp: 0xFFFE,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 20);

        assert_eq!(cpu.registers().sp, 0xFFFD);
        assert_eq!(bus.read(0xFFFD), 0x01);
        assert_eq!(bus.read(0xFFFC), 0x00);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::Execute {
                opcode: 0xCD,
                step: 4,
                t_cycle: 0,
            }
        );
        assert_eq!(
            cpu.last_address_event(),
            Some(CpuAddressEvent {
                kind: CpuAddressEventKind::Write,
                access_address: Some(0xFFFD),
                idu_address: None,
                update_direction: None,
            })
        );

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

        assert_eq!(cpu.registers().sp, 0xFFFC);
        assert_eq!(bus.read(0xFFFD), 0x01);
        assert_eq!(bus.read(0xFFFC), 0x03);
        assert_eq!(cpu.registers().pc, 0x0108);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
        assert_eq!(
            cpu.last_address_event(),
            Some(CpuAddressEvent {
                kind: CpuAddressEventKind::WriteWithIncDec,
                access_address: Some(0xFFFC),
                idu_address: Some(0xFFFC),
                update_direction: Some(CpuAddressUpdateDirection::Decrement),
            })
        );

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 12);

        assert_eq!(cpu.registers().sp, 0xFFFE);
        assert_eq!(cpu.registers().pc, 0x0109);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::Execute {
                opcode: 0xC9,
                step: 2,
                t_cycle: 0,
            }
        );
        assert_eq!(
            cpu.last_address_event(),
            Some(CpuAddressEvent {
                kind: CpuAddressEventKind::ReadWithIncDec,
                access_address: Some(0xFFFD),
                idu_address: Some(0xFFFE),
                update_direction: Some(CpuAddressUpdateDirection::Increment),
            })
        );

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

        assert_eq!(cpu.registers().pc, 0x0103);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
    }

    #[test]
    fn push_and_pop_share_the_same_stack_byte_order_model() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut cartridge = build_test_cartridge(&[0xC5, 0xD1]);

        cpu.apply_startup_state(CpuStartupState {
            b: 0x12,
            c: 0x34,
            sp: 0xFFFE,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 16);

        assert_eq!(cpu.registers().sp, 0xFFFC);
        assert_eq!(bus.read(0xFFFD), 0x12);
        assert_eq!(bus.read(0xFFFC), 0x34);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
        assert_eq!(
            cpu.last_address_event(),
            Some(CpuAddressEvent {
                kind: CpuAddressEventKind::WriteWithIncDec,
                access_address: Some(0xFFFC),
                idu_address: Some(0xFFFC),
                update_direction: Some(CpuAddressUpdateDirection::Decrement),
            })
        );

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 12);

        assert_eq!(cpu.registers().d, 0x12);
        assert_eq!(cpu.registers().e, 0x34);
        assert_eq!(cpu.registers().sp, 0xFFFE);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
        assert_eq!(
            cpu.last_address_event(),
            Some(CpuAddressEvent {
                kind: CpuAddressEventKind::ReadWithIncDec,
                access_address: Some(0xFFFD),
                idu_address: Some(0xFFFE),
                update_direction: Some(CpuAddressUpdateDirection::Increment),
            })
        );
    }

    #[test]
    fn pop_af_masks_the_low_flag_nibble_across_the_full_blargg_special_loop() {
        for high in u8::MIN..=u8::MAX {
            for low in u8::MIN..=u8::MAX {
                let mut cpu = CpuCore::new(ConsoleModel::Dmg);
                let mut bus = Bus::new(ConsoleModel::Dmg);
                let mut cartridge = build_test_cartridge(&[0xC5, 0xF1, 0xF5, 0xD1]);

                cpu.apply_startup_state(CpuStartupState {
                    b: high,
                    c: low,
                    sp: 0xFFFE,
                    pc: 0x0100,
                    ..CpuStartupState::power_on_reset()
                });

                tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 56);

                assert_eq!(cpu.registers().d, high);
                assert_eq!(cpu.registers().e, low & 0xF0);
                assert_eq!(cpu.registers().sp, 0xFFFE);
                assert_eq!(
                    cpu.execution_state(),
                    CpuExecutionState::FetchOpcode { t_cycle: 0 }
                );
            }
        }
    }

    #[test]
    fn jr_positive_negative_and_jp_hl_match_blargg_special_control_flow_cases() {
        let mut jr_negative_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut jr_negative_bus = Bus::new(ConsoleModel::Dmg);
        let mut jr_negative_cartridge = build_test_cartridge(&[0x18, 0xFE]);

        jr_negative_cpu.apply_startup_state(CpuStartupState {
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(
            &mut jr_negative_cpu,
            &mut jr_negative_bus,
            &mut jr_negative_cartridge,
            12,
        );

        assert_eq!(jr_negative_cpu.registers().pc, 0x0100);
        assert_eq!(
            jr_negative_cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );

        let mut jr_positive_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut jr_positive_bus = Bus::new(ConsoleModel::Dmg);
        let mut jr_positive_cartridge = build_test_cartridge(&[0x18, 0x01, 0x00, 0x00]);

        jr_positive_cpu.apply_startup_state(CpuStartupState {
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(
            &mut jr_positive_cpu,
            &mut jr_positive_bus,
            &mut jr_positive_cartridge,
            12,
        );

        assert_eq!(jr_positive_cpu.registers().pc, 0x0103);
        assert_eq!(
            jr_positive_cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );

        let mut jp_hl_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut jp_hl_bus = Bus::new(ConsoleModel::Dmg);
        let mut jp_hl_cartridge = build_test_cartridge(&[0xE9]);

        jp_hl_cpu.apply_startup_state(CpuStartupState {
            h: 0xC1,
            l: 0x23,
            f: FLAG_Z | FLAG_C,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(&mut jp_hl_cpu, &mut jp_hl_bus, &mut jp_hl_cartridge, 4);

        assert_eq!(jp_hl_cpu.registers().pc, 0xC123);
        assert_eq!(jp_hl_cpu.registers().f, FLAG_Z | FLAG_C);
        assert_eq!(
            jp_hl_cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
    }

    #[test]
    fn decimal_adjust_accumulator_crc_matches_blargg_01_special_reference() {
        let mut crc = 0xFFFF_FFFF_u32;

        for flags in (0_u8..=0xF0).step_by(0x10) {
            for a in u8::MIN..=u8::MAX {
                let mut cpu = CpuCore::new(ConsoleModel::Dmg);
                let mut bus = Bus::new(ConsoleModel::Dmg);
                let mut cartridge = build_test_cartridge(&[0x27]);

                cpu.apply_startup_state(CpuStartupState {
                    a,
                    f: flags,
                    pc: 0x0100,
                    ..CpuStartupState::power_on_reset()
                });

                tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

                crc = crc32_iso_hdlc(crc, cpu.registers().a);
                crc = crc32_iso_hdlc(crc, cpu.registers().f);
            }
        }

        assert_eq!(crc ^ 0xFFFF_FFFF, 0x6A9F_8D8A);
    }

    #[test]
    fn cb_prefix_register_and_hl_variants_keep_double_fetch_and_memory_timing_distinct() {
        let mut register_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut register_bus = Bus::new(ConsoleModel::Dmg);
        let mut register_cartridge = build_test_cartridge(&[0xCB, 0x11]);
        register_cpu.apply_startup_state(CpuStartupState {
            c: 0x81,
            f: FLAG_C,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(
            &mut register_cpu,
            &mut register_bus,
            &mut register_cartridge,
            4,
        );

        assert_eq!(
            register_cpu.execution_state(),
            CpuExecutionState::Execute {
                opcode: 0xCB,
                step: 0,
                t_cycle: 0,
            }
        );

        tick_cpu_n(
            &mut register_cpu,
            &mut register_bus,
            &mut register_cartridge,
            4,
        );

        assert_eq!(register_cpu.registers().c, 0x03);
        assert_eq!(register_cpu.registers().f, FLAG_C);
        assert_eq!(register_cpu.registers().pc, 0x0102);
        assert_eq!(
            register_cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );

        let mut hl_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut hl_bus = Bus::new(ConsoleModel::Dmg);
        let mut hl_cartridge = build_test_cartridge(&[0xCB, 0x06]);
        hl_cpu.apply_startup_state(CpuStartupState {
            h: 0xC0,
            l: 0x00,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });
        hl_bus.write(0xC000, 0x81);

        tick_cpu_n(&mut hl_cpu, &mut hl_bus, &mut hl_cartridge, 8);

        assert_eq!(hl_bus.read(0xC000), 0x81);
        assert_eq!(
            hl_cpu.execution_state(),
            CpuExecutionState::Execute {
                opcode: 0xCB,
                step: 1,
                t_cycle: 0,
            }
        );

        tick_cpu_n(&mut hl_cpu, &mut hl_bus, &mut hl_cartridge, 4);

        assert_eq!(hl_bus.read(0xC000), 0x81);
        assert_eq!(
            hl_cpu.execution_state(),
            CpuExecutionState::Execute {
                opcode: 0xCB,
                step: 2,
                t_cycle: 0,
            }
        );

        tick_cpu_n(&mut hl_cpu, &mut hl_bus, &mut hl_cartridge, 4);

        assert_eq!(hl_bus.read(0xC000), 0x03);
        assert_eq!(hl_cpu.registers().f, FLAG_C);
        assert_eq!(hl_cpu.registers().pc, 0x0102);
        assert_eq!(
            hl_cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
    }

    #[test]
    fn cb_rr_and_srl_support_the_blargg_crc_runtime_path() {
        let mut rr_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut rr_bus = Bus::new(ConsoleModel::Dmg);
        let mut rr_cartridge = build_test_cartridge(&[0xCB, 0x19]);
        rr_cpu.apply_startup_state(CpuStartupState {
            c: 0x80,
            f: FLAG_C,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(&mut rr_cpu, &mut rr_bus, &mut rr_cartridge, 8);

        assert_eq!(rr_cpu.registers().c, 0xC0);
        assert_eq!(rr_cpu.registers().f, 0x00);
        assert_eq!(rr_cpu.registers().pc, 0x0102);
        assert_eq!(
            rr_cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );

        let mut srl_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut srl_bus = Bus::new(ConsoleModel::Dmg);
        let mut srl_cartridge = build_test_cartridge(&[0xCB, 0x3E]);
        srl_cpu.apply_startup_state(CpuStartupState {
            h: 0xC0,
            l: 0x00,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });
        srl_bus.write(0xC000, 0x81);

        tick_cpu_n(&mut srl_cpu, &mut srl_bus, &mut srl_cartridge, 12);

        assert_eq!(srl_bus.read(0xC000), 0x81);
        assert_eq!(
            srl_cpu.execution_state(),
            CpuExecutionState::Execute {
                opcode: 0xCB,
                step: 2,
                t_cycle: 0,
            }
        );

        tick_cpu_n(&mut srl_cpu, &mut srl_bus, &mut srl_cartridge, 4);

        assert_eq!(srl_bus.read(0xC000), 0x40);
        assert_eq!(srl_cpu.registers().f, FLAG_C);
        assert_eq!(srl_cpu.registers().pc, 0x0102);
        assert_eq!(
            srl_cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
    }

    #[test]
    fn cb_rrc_register_and_hl_variants_support_the_external_bitop_paths() {
        let mut register_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut register_bus = Bus::new(ConsoleModel::Dmg);
        let mut register_cartridge = build_test_cartridge(&[0xCB, 0x08]);
        register_cpu.apply_startup_state(CpuStartupState {
            b: 0x01,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(
            &mut register_cpu,
            &mut register_bus,
            &mut register_cartridge,
            8,
        );

        assert_eq!(register_cpu.registers().b, 0x80);
        assert_eq!(register_cpu.registers().f, FLAG_C);
        assert_eq!(register_cpu.registers().pc, 0x0102);
        assert_eq!(
            register_cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );

        let mut hl_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut hl_bus = Bus::new(ConsoleModel::Dmg);
        let mut hl_cartridge = build_test_cartridge(&[0xCB, 0x0E]);
        hl_cpu.apply_startup_state(CpuStartupState {
            h: 0xC0,
            l: 0x00,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });
        hl_bus.write(0xC000, 0x01);

        tick_cpu_n(&mut hl_cpu, &mut hl_bus, &mut hl_cartridge, 12);

        assert_eq!(hl_bus.read(0xC000), 0x01);
        assert_eq!(
            hl_cpu.execution_state(),
            CpuExecutionState::Execute {
                opcode: 0xCB,
                step: 2,
                t_cycle: 0,
            }
        );

        tick_cpu_n(&mut hl_cpu, &mut hl_bus, &mut hl_cartridge, 4);

        assert_eq!(hl_bus.read(0xC000), 0x80);
        assert_eq!(hl_cpu.registers().f, FLAG_C);
        assert_eq!(hl_cpu.registers().pc, 0x0102);
        assert_eq!(
            hl_cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
    }

    #[test]
    fn cb_sla_sra_and_swap_register_variants_update_flags_as_documented() {
        let mut sla_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut sla_bus = Bus::new(ConsoleModel::Dmg);
        let mut sla_cartridge = build_test_cartridge(&[0xCB, 0x20]);
        sla_cpu.apply_startup_state(CpuStartupState {
            b: 0x81,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(&mut sla_cpu, &mut sla_bus, &mut sla_cartridge, 8);

        assert_eq!(sla_cpu.registers().b, 0x02);
        assert_eq!(sla_cpu.registers().f, FLAG_C);

        let mut sra_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut sra_bus = Bus::new(ConsoleModel::Dmg);
        let mut sra_cartridge = build_test_cartridge(&[0xCB, 0x28]);
        sra_cpu.apply_startup_state(CpuStartupState {
            b: 0x81,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(&mut sra_cpu, &mut sra_bus, &mut sra_cartridge, 8);

        assert_eq!(sra_cpu.registers().b, 0xC0);
        assert_eq!(sra_cpu.registers().f, FLAG_C);

        let mut swap_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut swap_bus = Bus::new(ConsoleModel::Dmg);
        let mut swap_cartridge = build_test_cartridge(&[0xCB, 0x30]);
        swap_cpu.apply_startup_state(CpuStartupState {
            b: 0xF0,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(&mut swap_cpu, &mut swap_bus, &mut swap_cartridge, 8);

        assert_eq!(swap_cpu.registers().b, 0x0F);
        assert_eq!(swap_cpu.registers().f, 0x00);
    }

    #[test]
    fn cb_res_and_set_preserve_flags_for_register_and_hl_targets() {
        let mut register_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut register_bus = Bus::new(ConsoleModel::Dmg);
        let mut register_cartridge = build_test_cartridge(&[0xCB, 0x80, 0xCB, 0xC0]);
        register_cpu.apply_startup_state(CpuStartupState {
            b: 0xFF,
            f: FLAG_Z | FLAG_C,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(
            &mut register_cpu,
            &mut register_bus,
            &mut register_cartridge,
            16,
        );

        assert_eq!(register_cpu.registers().b, 0xFF);
        assert_eq!(register_cpu.registers().f, FLAG_Z | FLAG_C);
        assert_eq!(register_cpu.registers().pc, 0x0104);

        let mut hl_cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut hl_bus = Bus::new(ConsoleModel::Dmg);
        let mut hl_cartridge = build_test_cartridge(&[0xCB, 0x86, 0xCB, 0xC6]);
        hl_cpu.apply_startup_state(CpuStartupState {
            h: 0xC0,
            l: 0x00,
            f: FLAG_Z | FLAG_C,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });
        hl_bus.write(0xC000, 0xFF);

        tick_cpu_n(&mut hl_cpu, &mut hl_bus, &mut hl_cartridge, 32);

        assert_eq!(hl_bus.read(0xC000), 0xFF);
        assert_eq!(hl_cpu.registers().f, FLAG_Z | FLAG_C);
        assert_eq!(hl_cpu.registers().pc, 0x0104);
        assert_eq!(
            hl_cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
    }

    #[test]
    fn bit_cb_operation_preserves_carry_while_setting_half_carry() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut cartridge = build_test_cartridge(&[0xCB, 0x7C]);

        cpu.apply_startup_state(CpuStartupState {
            h: 0x80,
            f: FLAG_C,
            pc: 0x0100,
            ..CpuStartupState::power_on_reset()
        });

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 8);

        assert_eq!(cpu.registers().f, FLAG_H | FLAG_C);
        assert_eq!(cpu.registers().pc, 0x0102);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
    }

    #[test]
    fn interrupt_service_uses_a_five_machine_cycle_sequence_and_pushes_pc_bytewise() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut cartridge = build_test_cartridge(&[]);
        let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
        let mut joypad = Joypad::new(ConsoleModel::Dmg);

        cpu.apply_startup_state(CpuStartupState {
            pc: 0x0150,
            sp: 0xFFFE,
            ..CpuStartupState::power_on_reset()
        });
        cpu.ime = true;
        interrupts.write_ie(0x01);
        interrupts.write_if(0x01);

        cpu.evaluate_wake_and_interrupts(&mut interrupts, &mut joypad);

        assert!(!cpu.ime());
        assert_eq!(interrupts.read_if(), 0xE1);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::ServiceInterrupt {
                source: InterruptSource::VBlank,
                step: 0,
                t_cycle: 0,
            }
        );

        tick_cpu_n_with_interrupts(&mut cpu, &mut bus, &mut cartridge, &mut interrupts, 4);

        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::ServiceInterrupt {
                source: InterruptSource::VBlank,
                step: 1,
                t_cycle: 0,
            }
        );
        assert_eq!(cpu.registers().sp, 0xFFFE);

        tick_cpu_n_with_interrupts(&mut cpu, &mut bus, &mut cartridge, &mut interrupts, 4);

        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::ServiceInterrupt {
                source: InterruptSource::VBlank,
                step: 2,
                t_cycle: 0,
            }
        );
        assert_eq!(cpu.registers().sp, 0xFFFE);

        tick_cpu_n_with_interrupts(&mut cpu, &mut bus, &mut cartridge, &mut interrupts, 4);

        assert_eq!(cpu.registers().sp, 0xFFFD);
        assert_eq!(bus.read(0xFFFD), 0x00);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::ServiceInterrupt {
                source: InterruptSource::VBlank,
                step: 3,
                t_cycle: 0,
            }
        );
        assert_eq!(
            cpu.last_address_event(),
            Some(CpuAddressEvent {
                kind: CpuAddressEventKind::IncDec,
                access_address: None,
                idu_address: Some(0xFFFD),
                update_direction: Some(CpuAddressUpdateDirection::Decrement),
            })
        );

        tick_cpu_n_with_interrupts(&mut cpu, &mut bus, &mut cartridge, &mut interrupts, 4);

        assert_eq!(cpu.registers().sp, 0xFFFD);
        assert_eq!(bus.read(0xFFFD), 0x01);
        assert_eq!(bus.read(0xFFFC), 0x00);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::ServiceInterrupt {
                source: InterruptSource::VBlank,
                step: 4,
                t_cycle: 0,
            }
        );
        assert_eq!(
            cpu.last_address_event(),
            Some(CpuAddressEvent {
                kind: CpuAddressEventKind::Write,
                access_address: Some(0xFFFD),
                idu_address: None,
                update_direction: None,
            })
        );

        tick_cpu_n_with_interrupts(&mut cpu, &mut bus, &mut cartridge, &mut interrupts, 4);

        assert_eq!(cpu.registers().sp, 0xFFFC);
        assert_eq!(bus.read(0xFFFC), 0x50);
        assert_eq!(cpu.registers().pc, 0x0040);
        assert_eq!(interrupts.read_if(), 0xE0);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
    }

    #[test]
    fn pending_interrupt_can_preempt_the_in_flight_fetch_before_the_opcode_latches() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
        let mut joypad = Joypad::new(ConsoleModel::Dmg);

        cpu.apply_startup_state(CpuStartupState {
            pc: 0x0150,
            ..CpuStartupState::power_on_reset()
        });
        cpu.ime = true;
        cpu.execution_state = CpuExecutionState::FetchOpcode { t_cycle: 3 };
        interrupts.write_ie(0x04);
        interrupts.write_if(0x04);

        cpu.evaluate_wake_and_interrupts(&mut interrupts, &mut joypad);

        assert!(!cpu.ime());
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::ServiceInterrupt {
                source: InterruptSource::Timer,
                step: 0,
                t_cycle: 0,
            }
        );
    }

    #[test]
    fn interrupt_service_keeps_the_accepted_source_latched_until_vector_commit() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut bus = Bus::new(ConsoleModel::Dmg);
        let mut cartridge = build_test_cartridge(&[]);
        let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
        let mut joypad = Joypad::new(ConsoleModel::Dmg);

        cpu.apply_startup_state(CpuStartupState {
            pc: 0x0150,
            sp: 0xFFFE,
            ..CpuStartupState::power_on_reset()
        });
        cpu.ime = true;
        interrupts.write_ie(0x05);
        interrupts.write_if(0x04);

        cpu.evaluate_wake_and_interrupts(&mut interrupts, &mut joypad);

        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::ServiceInterrupt {
                source: InterruptSource::Timer,
                step: 0,
                t_cycle: 0,
            }
        );

        tick_cpu_n_with_interrupts(&mut cpu, &mut bus, &mut cartridge, &mut interrupts, 12);
        interrupts.request(InterruptSource::VBlank);
        tick_cpu_n_with_interrupts(&mut cpu, &mut bus, &mut cartridge, &mut interrupts, 8);

        assert_eq!(cpu.registers().pc, 0x0050);
        assert_eq!(interrupts.read_if(), 0xE1);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        );
    }

    #[test]
    fn halt_with_a_pending_interrupt_and_ime_enabled_enters_service_without_staying_halted() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
        let mut joypad = Joypad::new(ConsoleModel::Dmg);

        cpu.apply_startup_state(CpuStartupState {
            pc: 0x0150,
            ..CpuStartupState::power_on_reset()
        });
        cpu.ime = true;
        cpu.finish_and_request_halt();
        interrupts.write_ie(0x01);
        interrupts.write_if(0x01);

        cpu.evaluate_wake_and_interrupts(&mut interrupts, &mut joypad);

        assert_eq!(interrupts.read_if(), 0xE1);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::ServiceInterrupt {
                source: InterruptSource::VBlank,
                step: 0,
                t_cycle: 0,
            }
        );
    }

    #[test]
    fn ei_halt_with_a_pending_interrupt_services_and_preserves_the_halt_return_address() {
        let mut cpu = CpuCore::new(ConsoleModel::Dmg);
        let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
        let mut joypad = Joypad::new(ConsoleModel::Dmg);

        cpu.apply_startup_state(CpuStartupState {
            pc: 0x0151,
            ..CpuStartupState::power_on_reset()
        });
        cpu.schedule_delayed_ime_enable();
        cpu.advance_delayed_ime_enable();
        cpu.finish_and_request_halt();
        interrupts.write_ie(0x01);
        interrupts.write_if(0x01);

        cpu.evaluate_wake_and_interrupts(&mut interrupts, &mut joypad);

        assert!(!cpu.ime());
        assert_eq!(interrupts.read_if(), 0xE1);
        assert_eq!(cpu.registers().pc, 0x0150);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::ServiceInterrupt {
                source: InterruptSource::VBlank,
                step: 0,
                t_cycle: 0,
            }
        );
    }
}
