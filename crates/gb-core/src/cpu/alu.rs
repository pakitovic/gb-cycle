use super::decode::{AluOperation, CbInstructionKind};
use super::*;

impl CpuCore {
    pub(super) fn update_inc_flags(&mut self, before: u8, result: u8) {
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

    pub(super) fn update_dec_flags(&mut self, before: u8, result: u8) {
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

    pub(super) fn adc_to_a(&mut self, value: u8) {
        let a = self.registers.a;
        let carry_in = u8::from(self.registers.f & FLAG_C != 0);
        let result16 = u16::from(a) + u16::from(value) + u16::from(carry_in);
        let result = result16 as u8;
        let half_carry = (a & 0x0F) + (value & 0x0F) + carry_in > 0x0F;

        self.registers.a = result;
        self.write_flags(result == 0, false, half_carry, result16 > 0xFF);
    }

    pub(super) fn sub_from_a(&mut self, value: u8) {
        let a = self.registers.a;
        let result = a.wrapping_sub(value);
        let half_carry = (a & 0x0F) < (value & 0x0F);
        let carry = a < value;

        self.registers.a = result;
        self.write_flags(result == 0, true, half_carry, carry);
    }

    pub(super) fn sbc_from_a(&mut self, value: u8) {
        let a = self.registers.a;
        let carry_in = u8::from(self.registers.f & FLAG_C != 0);
        let result = a.wrapping_sub(value).wrapping_sub(carry_in);
        let half_carry = (a & 0x0F) < ((value & 0x0F) + carry_in);
        let carry = u16::from(a) < (u16::from(value) + u16::from(carry_in));

        self.registers.a = result;
        self.write_flags(result == 0, true, half_carry, carry);
    }

    pub(super) fn and_with_a(&mut self, value: u8) {
        self.registers.a &= value;
        self.write_flags(self.registers.a == 0, false, true, false);
    }

    pub(super) fn xor_with_a(&mut self, value: u8) {
        self.registers.a ^= value;
        self.write_flags(self.registers.a == 0, false, false, false);
    }

    pub(super) fn or_with_a(&mut self, value: u8) {
        self.registers.a |= value;
        self.write_flags(self.registers.a == 0, false, false, false);
    }

    pub(super) fn compare_a(&mut self, value: u8) {
        let a = self.registers.a;
        let result = a.wrapping_sub(value);
        let half_carry = (a & 0x0F) < (value & 0x0F);
        let carry = a < value;

        self.write_flags(result == 0, true, half_carry, carry);
    }

    pub(super) fn add_to_hl(&mut self, value: u16) {
        let hl = self.hl();
        let result = hl.wrapping_add(value);
        let zero = self.registers.f & FLAG_Z != 0;
        let half_carry = (hl & 0x0FFF) + (value & 0x0FFF) > 0x0FFF;
        let carry = u32::from(hl) + u32::from(value) > 0xFFFF;

        self.write_register16(super::decode::Register16::HL, result);
        self.write_flags(zero, false, half_carry, carry);
    }

    pub(super) fn sp_plus_signed_immediate(&mut self, value: u8) -> u16 {
        let sp = self.registers.sp;
        let signed = i16::from(i8::from_ne_bytes([value]));
        let result = sp.wrapping_add_signed(signed);
        let half_carry = (sp & 0x000F) + u16::from(value & 0x0F) > 0x000F;
        let carry = (sp & 0x00FF) + u16::from(value) > 0x00FF;

        self.write_flags(false, false, half_carry, carry);
        result
    }

    pub(super) fn apply_alu_operation(&mut self, operation: AluOperation, value: u8) {
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

    pub(super) fn decimal_adjust_a(&mut self) {
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

    pub(super) fn complement_a(&mut self) {
        self.registers.a = !self.registers.a;
        let zero = self.registers.f & FLAG_Z != 0;
        let carry = self.registers.f & FLAG_C != 0;
        self.write_flags(zero, true, true, carry);
    }

    pub(super) fn set_carry_flag(&mut self) {
        let zero = self.registers.f & FLAG_Z != 0;
        self.write_flags(zero, false, false, true);
    }

    pub(super) fn complement_carry_flag(&mut self) {
        let zero = self.registers.f & FLAG_Z != 0;
        let carry = self.registers.f & FLAG_C == 0;
        self.write_flags(zero, false, false, carry);
    }

    pub(super) fn apply_cb_operation(&mut self, kind: CbInstructionKind, value: u8) -> Option<u8> {
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

    pub(super) fn rotate_left_carry(&mut self, value: u8) -> u8 {
        let carry = value & 0x80 != 0;
        let result = value.rotate_left(1);
        self.write_flags(result == 0, false, false, carry);
        result
    }

    pub(super) fn rotate_right_carry(&mut self, value: u8) -> u8 {
        let carry = value & 0x01 != 0;
        let result = value.rotate_right(1);
        self.write_flags(result == 0, false, false, carry);
        result
    }

    pub(super) fn rotate_left_through_carry(&mut self, value: u8) -> u8 {
        let carry_in = u8::from(self.registers.f & FLAG_C != 0);
        let carry_out = value & 0x80 != 0;
        let result = (value << 1) | carry_in;
        self.write_flags(result == 0, false, false, carry_out);
        result
    }

    pub(super) fn rotate_right_through_carry(&mut self, value: u8) -> u8 {
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

    pub(super) fn write_flags(
        &mut self,
        zero: bool,
        subtract: bool,
        half_carry: bool,
        carry: bool,
    ) {
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
