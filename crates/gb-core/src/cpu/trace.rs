#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum CpuTraceBusAccessKind {
    OpcodeFetch,
    OperandRead,
    DataRead,
    DataWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct CpuTraceBusActivity {
    pub(super) kind: CpuTraceBusAccessKind,
    pub(super) address: u16,
    pub(super) value: u8,
}

impl CpuTraceBusAccessKind {
    pub(super) const fn trace_label(self) -> &'static str {
        match self {
            Self::OpcodeFetch => "opcode_fetch",
            Self::OperandRead => "operand_read",
            Self::DataRead => "data_read",
            Self::DataWrite => "data_write",
        }
    }
}
