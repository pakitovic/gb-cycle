use super::super::decode::{CpuInstructionKind, Register16};
use super::super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Immediate16AddressPhase {
    ReadLow,
    ReadHigh,
    Access,
}

impl Immediate16AddressPhase {
    const fn from_step(step: u8) -> Option<Self> {
        match step {
            0 => Some(Self::ReadLow),
            1 => Some(Self::ReadHigh),
            2 => Some(Self::Access),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HighImmediateAddressPhase {
    ReadOffset,
    Access,
}

impl HighImmediateAddressPhase {
    const fn from_step(step: u8) -> Option<Self> {
        match step {
            0 => Some(Self::ReadOffset),
            1 => Some(Self::Access),
            _ => None,
        }
    }
}

impl CpuCore {
    pub(super) fn execute_load_machine_cycle(
        &mut self,
        kind: CpuInstructionKind,
        opcode: u8,
        step: u8,
        bus_operation: &mut CpuExternalCallback<'_>,
    ) {
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
                    self.in_flight.operand16_latch = u16::from(self.read_pc_u8(bus_operation));
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let high = self.read_pc_u8(bus_operation);
                    let value = self.in_flight.operand16_latch | (u16::from(high) << 8);
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
                    self.in_flight.operand8_latch = self.read_pc_u8(bus_operation);
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    self.write_byte(self.hl(), self.in_flight.operand8_latch, bus_operation);
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
            CpuInstructionKind::LoadAFromDirectAddress { source } => match step {
                0 => {
                    let value = self.read_byte(self.resolve_direct_address(source), bus_operation);
                    self.registers.a = value;
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::LoadAFromImmediate16Address => {
                match Immediate16AddressPhase::from_step(step) {
                    Some(Immediate16AddressPhase::ReadLow) => {
                        self.in_flight.operand16_latch = u16::from(self.read_pc_u8(bus_operation));
                        self.advance_instruction(opcode, 1);
                    }
                    Some(Immediate16AddressPhase::ReadHigh) => {
                        let high = self.read_pc_u8(bus_operation);
                        self.in_flight.operand16_latch |= u16::from(high) << 8;
                        self.advance_instruction(opcode, 2);
                    }
                    Some(Immediate16AddressPhase::Access) => {
                        let value =
                            self.read_byte(self.resolve_immediate16_address(), bus_operation);
                        self.registers.a = value;
                        self.finish_instruction();
                    }
                    None => self.stall_instruction(opcode, step),
                }
            }
            CpuInstructionKind::LoadAFromHighImmediateAddress => {
                match HighImmediateAddressPhase::from_step(step) {
                    Some(HighImmediateAddressPhase::ReadOffset) => {
                        self.in_flight.operand8_latch = self.read_pc_u8(bus_operation);
                        self.advance_instruction(opcode, 1);
                    }
                    Some(HighImmediateAddressPhase::Access) => {
                        let value =
                            self.read_byte(self.resolve_high_immediate_address(), bus_operation);
                        self.registers.a = value;
                        self.finish_instruction();
                    }
                    None => self.stall_instruction(opcode, step),
                }
            }
            CpuInstructionKind::StoreAToDirectAddress { destination } => match step {
                0 => {
                    self.write_byte(
                        self.resolve_direct_address(destination),
                        self.registers.a,
                        bus_operation,
                    );
                    self.finish_instruction();
                }
                _ => self.stall_instruction(opcode, step),
            },
            CpuInstructionKind::StoreAToImmediate16Address => {
                match Immediate16AddressPhase::from_step(step) {
                    Some(Immediate16AddressPhase::ReadLow) => {
                        self.in_flight.operand16_latch = u16::from(self.read_pc_u8(bus_operation));
                        self.advance_instruction(opcode, 1);
                    }
                    Some(Immediate16AddressPhase::ReadHigh) => {
                        let high = self.read_pc_u8(bus_operation);
                        self.in_flight.operand16_latch |= u16::from(high) << 8;
                        self.advance_instruction(opcode, 2);
                    }
                    Some(Immediate16AddressPhase::Access) => {
                        self.write_byte(
                            self.resolve_immediate16_address(),
                            self.registers.a,
                            bus_operation,
                        );
                        self.finish_instruction();
                    }
                    None => self.stall_instruction(opcode, step),
                }
            }
            CpuInstructionKind::StoreAToHighImmediateAddress => {
                match HighImmediateAddressPhase::from_step(step) {
                    Some(HighImmediateAddressPhase::ReadOffset) => {
                        self.in_flight.operand8_latch = self.read_pc_u8(bus_operation);
                        self.advance_instruction(opcode, 1);
                    }
                    Some(HighImmediateAddressPhase::Access) => {
                        self.write_byte(
                            self.resolve_high_immediate_address(),
                            self.registers.a,
                            bus_operation,
                        );
                        self.finish_instruction();
                    }
                    None => self.stall_instruction(opcode, step),
                }
            }
            CpuInstructionKind::StoreSpToImmediate16 => match step {
                0 => {
                    self.in_flight.operand16_latch = u16::from(self.read_pc_u8(bus_operation));
                    self.advance_instruction(opcode, 1);
                }
                1 => {
                    let high = self.read_pc_u8(bus_operation);
                    self.in_flight.operand16_latch |= u16::from(high) << 8;
                    self.advance_instruction(opcode, 2);
                }
                2 => {
                    let [low, _high] = self.registers.sp.to_le_bytes();
                    self.write_byte(self.in_flight.operand16_latch, low, bus_operation);
                    self.advance_instruction(opcode, 3);
                }
                3 => {
                    let [_low, high] = self.registers.sp.to_le_bytes();
                    self.write_byte(
                        self.in_flight.operand16_latch.wrapping_add(1),
                        high,
                        bus_operation,
                    );
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
