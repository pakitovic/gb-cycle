use super::super::{CpuCore, FLAG_C};
use super::{
    CpuInstructionKind, DecodedOpcode, DirectAddressSource, Register8Operand,
    decode_absolute_jump_condition, decode_alu_operation, decode_call_condition,
    decode_hl_update_direction, decode_register8_operand, decode_register16,
    decode_relative_jump_condition, decode_return_condition, decode_stack_register16,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpcodeDecodeGroup {
    Misc,
    Load,
    Arithmetic,
    ControlFlow,
}

impl CpuCore {
    pub(in crate::cpu) fn decode_fetched_opcode(&mut self, opcode: u8) -> DecodedOpcode {
        if let Some(group) = decode_fast_group(opcode) {
            return self
                .decode_opcode_in_group(opcode, group)
                .unwrap_or(DecodedOpcode::Unsupported);
        }

        self.decode_fetched_opcode_slow_path(opcode)
    }

    pub(in crate::cpu) fn decode_fetched_opcode_slow_path(&mut self, opcode: u8) -> DecodedOpcode {
        self.decode_misc_opcode(opcode)
            .or_else(|| self.decode_load_opcode(opcode))
            .or_else(|| self.decode_arithmetic_opcode(opcode))
            .or_else(|| self.decode_control_flow_opcode(opcode))
            .unwrap_or(DecodedOpcode::Unsupported)
    }

    fn decode_opcode_in_group(
        &mut self,
        opcode: u8,
        group: OpcodeDecodeGroup,
    ) -> Option<DecodedOpcode> {
        match group {
            OpcodeDecodeGroup::Misc => self.decode_misc_opcode(opcode),
            OpcodeDecodeGroup::Load => self.decode_load_opcode(opcode),
            OpcodeDecodeGroup::Arithmetic => self.decode_arithmetic_opcode(opcode),
            OpcodeDecodeGroup::ControlFlow => self.decode_control_flow_opcode(opcode),
        }
    }

    fn decode_misc_opcode(&mut self, opcode: u8) -> Option<DecodedOpcode> {
        match opcode {
            0x00 => Some(DecodedOpcode::Complete),
            0xAF => {
                self.registers.a = 0;
                self.write_flags(true, false, false, false);
                Some(DecodedOpcode::Complete)
            }
            0x27 => {
                self.decimal_adjust_a();
                Some(DecodedOpcode::Complete)
            }
            0x2F => {
                self.complement_a();
                Some(DecodedOpcode::Complete)
            }
            0x37 => {
                self.set_carry_flag();
                Some(DecodedOpcode::Complete)
            }
            0x3F => {
                self.complement_carry_flag();
                Some(DecodedOpcode::Complete)
            }
            0x07 => {
                let result = self.rotate_left_carry(self.registers.a);
                self.registers.a = result;
                self.write_flags(false, false, false, self.registers.f & FLAG_C != 0);
                Some(DecodedOpcode::Complete)
            }
            0x17 => {
                let result = self.rotate_left_through_carry(self.registers.a);
                self.registers.a = result;
                self.write_flags(false, false, false, self.registers.f & FLAG_C != 0);
                Some(DecodedOpcode::Complete)
            }
            0x0F => {
                let result = self.rotate_right_carry(self.registers.a);
                self.registers.a = result;
                self.write_flags(false, false, false, self.registers.f & FLAG_C != 0);
                Some(DecodedOpcode::Complete)
            }
            0x1F => {
                let result = self.rotate_right_through_carry(self.registers.a);
                self.registers.a = result;
                self.write_flags(false, false, false, self.registers.f & FLAG_C != 0);
                Some(DecodedOpcode::Complete)
            }
            0x76 => {
                self.finish_and_request_halt();
                Some(DecodedOpcode::Complete)
            }
            0xF3 => {
                self.set_ime_disabled();
                self.cancel_delayed_ime_enable();
                Some(DecodedOpcode::Complete)
            }
            0xFB => {
                self.schedule_delayed_ime_enable();
                Some(DecodedOpcode::Complete)
            }
            0x10 => Some(DecodedOpcode::Execute(CpuInstructionKind::Stop)),
            _ => None,
        }
    }

    fn decode_load_opcode(&mut self, opcode: u8) -> Option<DecodedOpcode> {
        if matches!(opcode, 0x01 | 0x11 | 0x21 | 0x31) {
            return Some(DecodedOpcode::Execute(
                CpuInstructionKind::LoadRegisterPairImmediate {
                    target: decode_register16((opcode >> 4) & 0x03),
                },
            ));
        }

        if opcode & 0b1100_0111 == 0b0000_0110 {
            return Some(match decode_register8_operand((opcode >> 3) & 0x07) {
                Register8Operand::Register(target) => {
                    DecodedOpcode::Execute(CpuInstructionKind::LoadRegisterImmediate { target })
                }
                Register8Operand::IndirectHl => {
                    DecodedOpcode::Execute(CpuInstructionKind::StoreImmediateToHl)
                }
            });
        }

        if (0x40..=0x7F).contains(&opcode) && opcode != 0x76 {
            let destination = decode_register8_operand((opcode >> 3) & 0x07);
            let source = decode_register8_operand(opcode & 0x07);

            return Some(match (destination, source) {
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
            });
        }

        if matches!(opcode, 0x02 | 0x12 | 0xEA) {
            return Some(DecodedOpcode::Execute(match opcode {
                0x02 => CpuInstructionKind::StoreAToDirectAddress {
                    destination: DirectAddressSource::BC,
                },
                0x12 => CpuInstructionKind::StoreAToDirectAddress {
                    destination: DirectAddressSource::DE,
                },
                0xEA => CpuInstructionKind::StoreAToImmediate16Address,
                _ => unreachable!("opcode filter already constrained"),
            }));
        }

        if matches!(opcode, 0xE0 | 0xE2) {
            return Some(DecodedOpcode::Execute(match opcode {
                0xE0 => CpuInstructionKind::StoreAToHighImmediateAddress,
                0xE2 => CpuInstructionKind::StoreAToDirectAddress {
                    destination: match opcode {
                        0xE2 => DirectAddressSource::HighC,
                        _ => unreachable!("opcode filter already constrained"),
                    },
                },
                _ => unreachable!("opcode filter already constrained"),
            }));
        }

        if matches!(opcode, 0x22 | 0x32) {
            return Some(DecodedOpcode::Execute(
                CpuInstructionKind::StoreAToHlWithUpdate {
                    direction: decode_hl_update_direction(opcode),
                },
            ));
        }

        if matches!(opcode, 0x0A | 0x1A | 0xFA) {
            return Some(DecodedOpcode::Execute(match opcode {
                0x0A => CpuInstructionKind::LoadAFromDirectAddress {
                    source: DirectAddressSource::BC,
                },
                0x1A => CpuInstructionKind::LoadAFromDirectAddress {
                    source: DirectAddressSource::DE,
                },
                0xFA => CpuInstructionKind::LoadAFromImmediate16Address,
                _ => unreachable!("opcode filter already constrained"),
            }));
        }

        if matches!(opcode, 0xF0 | 0xF2) {
            return Some(DecodedOpcode::Execute(match opcode {
                0xF0 => CpuInstructionKind::LoadAFromHighImmediateAddress,
                0xF2 => CpuInstructionKind::LoadAFromDirectAddress {
                    source: match opcode {
                        0xF2 => DirectAddressSource::HighC,
                        _ => unreachable!("opcode filter already constrained"),
                    },
                },
                _ => unreachable!("opcode filter already constrained"),
            }));
        }

        if opcode == 0x08 {
            return Some(DecodedOpcode::Execute(
                CpuInstructionKind::StoreSpToImmediate16,
            ));
        }

        if opcode == 0xF8 {
            return Some(DecodedOpcode::Execute(
                CpuInstructionKind::LoadHlFromSpPlusImmediate,
            ));
        }

        if opcode == 0xE8 {
            return Some(DecodedOpcode::Execute(CpuInstructionKind::AddSpImmediate));
        }

        if opcode == 0xF9 {
            return Some(DecodedOpcode::Execute(CpuInstructionKind::LoadSpFromHl));
        }

        if matches!(opcode, 0x2A | 0x3A) {
            return Some(DecodedOpcode::Execute(
                CpuInstructionKind::LoadAFromHlWithUpdate {
                    direction: decode_hl_update_direction(opcode),
                },
            ));
        }

        None
    }

    fn decode_arithmetic_opcode(&mut self, opcode: u8) -> Option<DecodedOpcode> {
        if opcode & 0xCF == 0x03 {
            return Some(DecodedOpcode::Execute(
                CpuInstructionKind::IncrementRegisterPair {
                    target: decode_register16((opcode >> 4) & 0x03),
                },
            ));
        }

        if opcode & 0xCF == 0x09 {
            return Some(DecodedOpcode::Execute(CpuInstructionKind::AddHl {
                source: decode_register16((opcode >> 4) & 0x03),
            }));
        }

        if opcode & 0xCF == 0x0B {
            return Some(DecodedOpcode::Execute(
                CpuInstructionKind::DecrementRegisterPair {
                    target: decode_register16((opcode >> 4) & 0x03),
                },
            ));
        }

        if opcode & 0b1100_0111 == 0b0000_0100 {
            return Some(match decode_register8_operand((opcode >> 3) & 0x07) {
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
            });
        }

        if opcode & 0b1100_0111 == 0b0000_0101 {
            return Some(match decode_register8_operand((opcode >> 3) & 0x07) {
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
            });
        }

        if matches!(
            opcode,
            0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE
        ) {
            return Some(DecodedOpcode::Execute(CpuInstructionKind::AluImmediate {
                operation: decode_alu_operation((opcode >> 3) & 0x07),
            }));
        }

        if (0x80..=0xBF).contains(&opcode) {
            let operation = decode_alu_operation((opcode >> 3) & 0x07);
            return Some(match decode_register8_operand(opcode & 0x07) {
                Register8Operand::Register(source) => {
                    let value = self.read_register8(source);
                    self.apply_alu_operation(operation, value);
                    DecodedOpcode::Complete
                }
                Register8Operand::IndirectHl => {
                    DecodedOpcode::Execute(CpuInstructionKind::AluFromHl { operation })
                }
            });
        }

        None
    }

    fn decode_control_flow_opcode(&mut self, opcode: u8) -> Option<DecodedOpcode> {
        if opcode == 0xE9 {
            self.registers.pc = self.hl();
            return Some(DecodedOpcode::Complete);
        }

        if opcode == 0xCB {
            return Some(DecodedOpcode::Execute(CpuInstructionKind::CbPrefixed));
        }

        if matches!(opcode, 0x18 | 0x20 | 0x28 | 0x30 | 0x38) {
            return Some(DecodedOpcode::Execute(
                match decode_relative_jump_condition(opcode) {
                    Some(condition) => CpuInstructionKind::ConditionalRelativeJump { condition },
                    None => CpuInstructionKind::RelativeJump,
                },
            ));
        }

        if matches!(opcode, 0xC3 | 0xC2 | 0xCA | 0xD2 | 0xDA) {
            return Some(DecodedOpcode::Execute(
                match decode_absolute_jump_condition(opcode) {
                    Some(condition) => CpuInstructionKind::ConditionalAbsoluteJump { condition },
                    None => CpuInstructionKind::AbsoluteJump,
                },
            ));
        }

        if matches!(opcode, 0xCD | 0xC4 | 0xCC | 0xD4 | 0xDC) {
            return Some(DecodedOpcode::Execute(
                match decode_call_condition(opcode) {
                    Some(condition) => CpuInstructionKind::ConditionalCall { condition },
                    None => CpuInstructionKind::Call,
                },
            ));
        }

        if matches!(opcode, 0xC9 | 0xC0 | 0xC8 | 0xD0 | 0xD8) {
            return Some(DecodedOpcode::Execute(
                match decode_return_condition(opcode) {
                    Some(condition) => CpuInstructionKind::ConditionalReturn { condition },
                    None => CpuInstructionKind::Return,
                },
            ));
        }

        if opcode == 0xD9 {
            return Some(DecodedOpcode::Execute(
                CpuInstructionKind::ReturnFromInterrupt,
            ));
        }

        if opcode & 0xC7 == 0xC7 {
            return Some(DecodedOpcode::Execute(CpuInstructionKind::Restart {
                vector: u16::from(opcode & 0x38),
            }));
        }

        if opcode & 0xCF == 0xC5 {
            return Some(DecodedOpcode::Execute(
                CpuInstructionKind::PushRegisterPair {
                    source: decode_stack_register16((opcode >> 4) & 0x03),
                },
            ));
        }

        if opcode & 0xCF == 0xC1 {
            return Some(DecodedOpcode::Execute(
                CpuInstructionKind::PopRegisterPair {
                    target: decode_stack_register16((opcode >> 4) & 0x03),
                },
            ));
        }

        None
    }
}

const fn decode_fast_group(opcode: u8) -> Option<OpcodeDecodeGroup> {
    match opcode {
        0x00 | 0x07 | 0x0F | 0x10 | 0x17 | 0x1F | 0x27 | 0x2F | 0x37 | 0x3F | 0x76 | 0xAF
        | 0xF3 | 0xFB => Some(OpcodeDecodeGroup::Misc),
        0x40..=0x7F => Some(OpcodeDecodeGroup::Load),
        0x80..=0xBF => Some(OpcodeDecodeGroup::Arithmetic),
        0x18 | 0x20 | 0x28 | 0x30 | 0x38 | 0xC0 | 0xC1 | 0xC2 | 0xC3 | 0xC4 | 0xC5 | 0xC7
        | 0xC8 | 0xC9 | 0xCA | 0xCB | 0xCC | 0xCD | 0xCF | 0xD0 | 0xD1 | 0xD2 | 0xD4 | 0xD5
        | 0xD7 | 0xD8 | 0xD9 | 0xDA | 0xDC | 0xDF | 0xE7 | 0xEF | 0xF7 | 0xFF => {
            Some(OpcodeDecodeGroup::ControlFlow)
        }
        _ if opcode & 0b1100_0111 == 0b0000_0110 => Some(OpcodeDecodeGroup::Load),
        _ if opcode & 0b1100_0111 == 0b0000_0100
            || opcode & 0b1100_0111 == 0b0000_0101
            || opcode & 0xCF == 0x03
            || opcode & 0xCF == 0x09
            || opcode & 0xCF == 0x0B =>
        {
            Some(OpcodeDecodeGroup::Arithmetic)
        }
        _ if matches!(
            opcode,
            0x01 | 0x02
                | 0x08
                | 0x0A
                | 0x11
                | 0x12
                | 0x1A
                | 0x21
                | 0x22
                | 0x2A
                | 0x31
                | 0x32
                | 0x3A
                | 0xE0
                | 0xE2
                | 0xE8
                | 0xEA
                | 0xF0
                | 0xF2
                | 0xF8
                | 0xF9
                | 0xFA
        ) =>
        {
            Some(OpcodeDecodeGroup::Load)
        }
        _ if matches!(
            opcode,
            0xC6 | 0xCE | 0xD6 | 0xDE | 0xE6 | 0xEE | 0xF6 | 0xFE
        ) =>
        {
            Some(OpcodeDecodeGroup::Arithmetic)
        }
        _ => None,
    }
}
