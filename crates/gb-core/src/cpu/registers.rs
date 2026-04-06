use super::decode::{ConditionCode, Register8, Register16, StackRegister16};
use super::*;

impl CpuCore {
    pub(super) fn condition_is_met(&self, condition: Option<ConditionCode>) -> bool {
        match condition {
            None => true,
            Some(ConditionCode::Nz) => self.registers.f & FLAG_Z == 0,
            Some(ConditionCode::Z) => self.registers.f & FLAG_Z != 0,
            Some(ConditionCode::Nc) => self.registers.f & FLAG_C == 0,
            Some(ConditionCode::C) => self.registers.f & FLAG_C != 0,
        }
    }

    pub(super) fn read_register8(&self, register: Register8) -> u8 {
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

    pub(super) fn write_register8(&mut self, register: Register8, value: u8) {
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

    pub(super) fn write_register16(&mut self, register: Register16, value: u16) {
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

    pub(super) fn read_stack_register16(&self, register: StackRegister16) -> u16 {
        match register {
            StackRegister16::BC => self.bc(),
            StackRegister16::DE => self.de(),
            StackRegister16::HL => self.hl(),
            StackRegister16::AF => u16::from_be_bytes([self.registers.a, self.registers.f]),
        }
    }

    pub(super) fn write_stack_register16(&mut self, register: StackRegister16, value: u16) {
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

    pub(super) fn increment_or_decrement_register_pair(
        &mut self,
        target: Register16,
        direction: CpuAddressUpdateDirection,
    ) {
        let updated = self.increment_or_decrement_register16(target, direction);
        self.record_address_event(CpuAddressEvent::incdec(updated, direction));
    }

    pub(super) fn increment_or_decrement_register16(
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

    pub(super) fn bc(&self) -> u16 {
        u16::from_be_bytes([self.registers.b, self.registers.c])
    }

    pub(super) fn de(&self) -> u16 {
        u16::from_be_bytes([self.registers.d, self.registers.e])
    }

    pub(super) fn hl(&self) -> u16 {
        u16::from_be_bytes([self.registers.h, self.registers.l])
    }
}
