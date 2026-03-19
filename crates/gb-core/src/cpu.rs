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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CbInstructionKind {
    RotateLeftCarry { target: Register8Operand },
    RotateLeftThroughCarry { target: Register8Operand },
    BitTest { bit: u8, target: Register8Operand },
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
    IncrementRegisterPair {
        target: Register16,
    },
    DecrementRegisterPair {
        target: Register16,
    },
    IncrementHlMemory,
    DecrementHlMemory,
    AddAImmediate,
    CompareAImmediate,
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
        self.halt_bug_pending = false;
        self.instruction_kind = None;
        self.cb_instruction_kind = None;
        self.operand8_latch = 0;
        self.operand16_latch = 0;
        self.last_bus_activity = None;
        self.last_address_event = None;
    }

    pub(crate) fn tick_t_cycle<F>(&mut self, mut bus_operation: F)
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
                    return;
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
                    return;
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
                    return;
                }

                self.complete_interrupt_service_machine_cycle(source, step, &mut bus_operation);
            }
            CpuExecutionState::DiagnosticTrap { .. }
            | CpuExecutionState::Halted
            | CpuExecutionState::Stopped => {}
        }
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

            if !self.ime && pending {
                self.halt_bug_pending = true;
                self.execution_state = CpuExecutionState::fetch_opcode();
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
                (MemoryAddressSource::BC | MemoryAddressSource::DE, 0) => {
                    let value = self.read_byte(self.resolve_memory_address(source), bus_operation);
                    self.registers.a = value;
                    self.finish_instruction();
                }
                (MemoryAddressSource::Immediate16, 0) => {
                    self.operand16_latch = u16::from(self.read_pc_u8(bus_operation));
                    self.advance_instruction(opcode, 1);
                }
                (MemoryAddressSource::Immediate16, 1) => {
                    let high = self.read_pc_u8(bus_operation);
                    self.operand16_latch |= u16::from(high) << 8;
                    self.advance_instruction(opcode, 2);
                }
                (MemoryAddressSource::Immediate16, 2) => {
                    let value = self.read_byte(self.operand16_latch, bus_operation);
                    self.registers.a = value;
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::StoreAToAddress { destination } => match (destination, step) {
                (MemoryAddressSource::BC | MemoryAddressSource::DE, 0) => {
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
                (MemoryAddressSource::Immediate16, 1) => {
                    let high = self.read_pc_u8(bus_operation);
                    self.operand16_latch |= u16::from(high) << 8;
                    self.advance_instruction(opcode, 2);
                }
                (MemoryAddressSource::Immediate16, 2) => {
                    self.write_byte(self.operand16_latch, self.registers.a, bus_operation);
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
            CpuInstructionKind::AddAImmediate => match step {
                0 => {
                    let value = self.read_pc_u8(bus_operation);
                    self.add_to_a(value);
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::CompareAImmediate => match step {
                0 => {
                    let value = self.read_pc_u8(bus_operation);
                    self.compare_a(value);
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
                    self.advance_instruction(opcode, 3);
                }
                3 => {
                    let [low, high] = self.registers.pc.to_le_bytes();
                    self.write_byte_with_decremented_sp(high, bus_operation);
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
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let [low, high] = self.registers.pc.to_le_bytes();
                    self.write_byte_with_decremented_sp(high, bus_operation);
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
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let [low, high] = self.read_stack_register16(source).to_le_bytes();
                    self.write_byte_with_decremented_sp(high, bus_operation);
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
                        Some(CbInstructionKind::RotateLeftCarry { target }) => {
                            if let Register8Operand::Register(target) = target {
                                let result = self.rotate_left_carry(self.read_register8(target));
                                self.write_register8(target, result);
                                self.finish_instruction();
                            } else {
                                self.cb_instruction_kind =
                                    Some(CbInstructionKind::RotateLeftCarry { target });
                                self.advance_instruction(opcode, 1);
                            }
                        }
                        Some(CbInstructionKind::RotateLeftThroughCarry { target }) => {
                            if let Register8Operand::Register(target) = target {
                                let result =
                                    self.rotate_left_through_carry(self.read_register8(target));
                                self.write_register8(target, result);
                                self.finish_instruction();
                            } else {
                                self.cb_instruction_kind =
                                    Some(CbInstructionKind::RotateLeftThroughCarry { target });
                                self.advance_instruction(opcode, 1);
                            }
                        }
                        Some(CbInstructionKind::BitTest { bit, target }) => {
                            if let Register8Operand::Register(target) = target {
                                self.bit_test(bit, self.read_register8(target));
                                self.finish_instruction();
                            } else {
                                self.cb_instruction_kind =
                                    Some(CbInstructionKind::BitTest { bit, target });
                                self.advance_instruction(opcode, 1);
                            }
                        }
                        None => self.stall_instruction(opcode, step),
                    }
                }
                1 => match self.cb_instruction_kind {
                    Some(CbInstructionKind::RotateLeftCarry {
                        target: Register8Operand::IndirectHl,
                    }) => {
                        let value = self.read_byte(self.hl(), bus_operation);
                        self.operand8_latch = self.rotate_left_carry(value);
                        self.advance_instruction(opcode, 2);
                    }
                    Some(CbInstructionKind::RotateLeftThroughCarry {
                        target: Register8Operand::IndirectHl,
                    }) => {
                        let value = self.read_byte(self.hl(), bus_operation);
                        self.operand8_latch = self.rotate_left_through_carry(value);
                        self.advance_instruction(opcode, 2);
                    }
                    Some(CbInstructionKind::BitTest {
                        bit,
                        target: Register8Operand::IndirectHl,
                    }) => {
                        let value = self.read_byte(self.hl(), bus_operation);
                        self.bit_test(bit, value);
                        self.finish_instruction();
                    }
                    _ => self.stall_instruction(opcode, step),
                },
                2 => match self.cb_instruction_kind {
                    Some(CbInstructionKind::RotateLeftCarry {
                        target: Register8Operand::IndirectHl,
                    })
                    | Some(CbInstructionKind::RotateLeftThroughCarry {
                        target: Register8Operand::IndirectHl,
                    }) => {
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

        if matches!(opcode, 0x2A | 0x3A) {
            return DecodedOpcode::Execute(CpuInstructionKind::LoadAFromHlWithUpdate {
                direction: decode_hl_update_direction(opcode),
            });
        }

        if opcode & 0xCF == 0x03 {
            return DecodedOpcode::Execute(CpuInstructionKind::IncrementRegisterPair {
                target: decode_register16((opcode >> 4) & 0x03),
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

        if opcode == 0xC6 {
            return DecodedOpcode::Execute(CpuInstructionKind::AddAImmediate);
        }

        if opcode == 0xFE {
            return DecodedOpcode::Execute(CpuInstructionKind::CompareAImmediate);
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
    ) where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
        match step {
            0 | 1 => self.advance_interrupt_service(source, step + 1),
            2 => {
                let [low, high] = self.registers.pc.to_le_bytes();
                self.write_byte_with_decremented_sp(high, bus_operation);
                self.operand8_latch = low;
                self.advance_interrupt_service(source, 3);
            }
            3 => {
                self.write_byte_with_decremented_sp(self.operand8_latch, bus_operation);
                self.advance_interrupt_service(source, 4);
            }
            4 => {
                self.registers.pc = interrupt_vector(source);
                self.finish_interrupt_service();
            }
            _ => self.advance_interrupt_service(source, step),
        }
    }

    fn can_accept_interrupt(&self) -> bool {
        matches!(
            self.execution_state,
            CpuExecutionState::FetchOpcode { t_cycle: 0 }
        )
    }

    fn accept_pending_interrupt(&mut self, interrupts: &mut InterruptController) {
        let Some(source) = interrupts.highest_pending() else {
            return;
        };

        interrupts.clear(source);
        self.begin_interrupt_service(source);
    }

    fn schedule_delayed_ime_enable(&mut self) {
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

    fn compare_a(&mut self, value: u8) {
        let a = self.registers.a;
        let result = a.wrapping_sub(value);
        let half_carry = (a & 0x0F) < (value & 0x0F);
        let carry = a < value;

        self.write_flags(result == 0, true, half_carry, carry);
    }

    fn decode_cb_opcode(&self, opcode: u8) -> Option<CbInstructionKind> {
        if opcode & 0xF8 == 0x00 {
            return Some(CbInstructionKind::RotateLeftCarry {
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xF8 == 0x10 {
            return Some(CbInstructionKind::RotateLeftThroughCarry {
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        if opcode & 0xC0 == 0x40 {
            return Some(CbInstructionKind::BitTest {
                bit: (opcode >> 3) & 0x07,
                target: decode_register8_operand(opcode & 0x07),
            });
        }

        None
    }

    fn rotate_left_carry(&mut self, value: u8) -> u8 {
        let carry = value & 0x80 != 0;
        let result = value.rotate_left(1);
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

        cpu.tick_t_cycle(|operation| match operation {
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
        });
    }

    fn tick_cpu_n(cpu: &mut CpuCore, bus: &mut Bus, cartridge: &mut CartridgeSlot, steps: usize) {
        for _ in 0..steps {
            tick_cpu(cpu, bus, cartridge);
        }
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
                kind: CpuAddressEventKind::WriteWithIncDec,
                access_address: Some(0xFFFD),
                idu_address: Some(0xFFFD),
                update_direction: Some(CpuAddressUpdateDirection::Decrement),
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
        assert_eq!(interrupts.read_if(), 0xE0);
        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::ServiceInterrupt {
                source: InterruptSource::VBlank,
                step: 0,
                t_cycle: 0,
            }
        );

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::ServiceInterrupt {
                source: InterruptSource::VBlank,
                step: 1,
                t_cycle: 0,
            }
        );
        assert_eq!(cpu.registers().sp, 0xFFFE);

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

        assert_eq!(
            cpu.execution_state(),
            CpuExecutionState::ServiceInterrupt {
                source: InterruptSource::VBlank,
                step: 2,
                t_cycle: 0,
            }
        );
        assert_eq!(cpu.registers().sp, 0xFFFE);

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

        assert_eq!(cpu.registers().sp, 0xFFFD);
        assert_eq!(bus.read(0xFFFD), 0x01);
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
                kind: CpuAddressEventKind::WriteWithIncDec,
                access_address: Some(0xFFFD),
                idu_address: Some(0xFFFD),
                update_direction: Some(CpuAddressUpdateDirection::Decrement),
            })
        );

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

        assert_eq!(cpu.registers().sp, 0xFFFC);
        assert_eq!(bus.read(0xFFFC), 0x50);
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
                kind: CpuAddressEventKind::WriteWithIncDec,
                access_address: Some(0xFFFC),
                idu_address: Some(0xFFFC),
                update_direction: Some(CpuAddressUpdateDirection::Decrement),
            })
        );

        tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

        assert_eq!(cpu.registers().pc, 0x0040);
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

        assert_eq!(interrupts.read_if(), 0xE0);
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
