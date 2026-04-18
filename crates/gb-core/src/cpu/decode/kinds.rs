use super::super::CpuAddressUpdateDirection;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::cpu) enum Register8 {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::cpu) enum Register16 {
    BC,
    DE,
    HL,
    SP,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::cpu) enum AluOperation {
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
pub(in crate::cpu) enum StackRegister16 {
    BC,
    DE,
    HL,
    AF,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::cpu) enum ConditionCode {
    Nz,
    Z,
    Nc,
    C,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::cpu) enum Register8Operand {
    Register(Register8),
    IndirectHl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::cpu) enum DirectAddressSource {
    BC,
    DE,
    HighC,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::cpu) enum InstructionExecutionGroup {
    Load,
    Arithmetic,
    ControlFlow,
    Stack,
    CbPrefixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::cpu) enum FetchCompletionKind {
    Nop,
    DecimalAdjustAccumulator,
    ComplementAccumulator,
    SetCarryFlag,
    ComplementCarryFlag,
    RotateLeftAccumulatorCarry,
    RotateLeftAccumulatorThroughCarry,
    RotateRightAccumulatorCarry,
    RotateRightAccumulatorThroughCarry,
    Halt,
    DisableInterrupts,
    EnableInterrupts,
    JumpHl,
    LoadRegisterToRegister {
        destination: Register8,
        source: Register8,
    },
    IncrementRegister {
        target: Register8,
    },
    DecrementRegister {
        target: Register8,
    },
    AluRegister {
        operation: AluOperation,
        source: Register8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::cpu) enum CbInstructionKind {
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
    pub(in crate::cpu) fn target(self) -> Register8Operand {
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
pub(in crate::cpu) enum CpuInstructionKind {
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
    LoadAFromDirectAddress {
        source: DirectAddressSource,
    },
    LoadAFromImmediate16Address,
    LoadAFromHighImmediateAddress,
    StoreAToDirectAddress {
        destination: DirectAddressSource,
    },
    StoreAToImmediate16Address,
    StoreAToHighImmediateAddress,
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
    RelativeJump,
    ConditionalRelativeJump {
        condition: ConditionCode,
    },
    AbsoluteJump,
    ConditionalAbsoluteJump {
        condition: ConditionCode,
    },
    Call,
    ConditionalCall {
        condition: ConditionCode,
    },
    Return,
    ConditionalReturn {
        condition: ConditionCode,
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

impl CpuInstructionKind {
    pub(in crate::cpu) const fn execution_group(self) -> InstructionExecutionGroup {
        match self {
            Self::LoadRegisterImmediate { .. }
            | Self::LoadRegisterPairImmediate { .. }
            | Self::LoadRegisterFromHl { .. }
            | Self::StoreRegisterToHl { .. }
            | Self::StoreImmediateToHl
            | Self::LoadAFromHlWithUpdate { .. }
            | Self::StoreAToHlWithUpdate { .. }
            | Self::LoadAFromDirectAddress { .. }
            | Self::LoadAFromImmediate16Address
            | Self::LoadAFromHighImmediateAddress
            | Self::StoreAToDirectAddress { .. }
            | Self::StoreAToImmediate16Address
            | Self::StoreAToHighImmediateAddress
            | Self::StoreSpToImmediate16
            | Self::LoadSpFromHl => InstructionExecutionGroup::Load,
            Self::LoadHlFromSpPlusImmediate
            | Self::AddSpImmediate
            | Self::AddHl { .. }
            | Self::IncrementRegisterPair { .. }
            | Self::DecrementRegisterPair { .. }
            | Self::IncrementHlMemory
            | Self::DecrementHlMemory
            | Self::AluImmediate { .. }
            | Self::AluFromHl { .. } => InstructionExecutionGroup::Arithmetic,
            Self::RelativeJump
            | Self::ConditionalRelativeJump { .. }
            | Self::AbsoluteJump
            | Self::ConditionalAbsoluteJump { .. }
            | Self::Call
            | Self::ConditionalCall { .. }
            | Self::Return
            | Self::ConditionalReturn { .. }
            | Self::ReturnFromInterrupt
            | Self::Stop
            | Self::Restart { .. } => InstructionExecutionGroup::ControlFlow,
            Self::PushRegisterPair { .. } | Self::PopRegisterPair { .. } => {
                InstructionExecutionGroup::Stack
            }
            Self::CbPrefixed => InstructionExecutionGroup::CbPrefixed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::cpu) enum DecodedOpcode {
    CompleteOnFetch(FetchCompletionKind),
    Execute(CpuInstructionKind),
    Unsupported,
}
