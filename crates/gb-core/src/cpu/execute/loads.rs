use super::super::decode::{CpuInstructionKind, MemoryAddressSource, Register16};
use super::super::*;

impl CpuCore {
    pub(super) fn execute_load_machine_cycle<F>(
        &mut self,
        kind: CpuInstructionKind,
        opcode: u8,
        step: u8,
        bus_operation: &mut F,
    ) where
        F: FnMut(CpuBusOperation) -> Option<u8>,
    {
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
            CpuInstructionKind::LoadSpFromHl => match step {
                0 => {
                    self.write_register16(Register16::SP, self.hl());
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            _ => unreachable!("non-load instruction dispatched to load executor"),
        }
    }
}
