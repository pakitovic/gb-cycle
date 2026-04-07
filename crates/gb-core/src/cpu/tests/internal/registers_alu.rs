use super::*;

#[test]
fn register_pair_and_stack_helpers_cover_private_paths() {
    let mut cpu = power_on_cpu();

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
}

#[test]
fn condition_and_flag_helpers_cover_private_paths() {
    let mut cpu = power_on_cpu();

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
}

#[test]
fn alu_helpers_cover_private_paths() {
    let mut cpu = power_on_cpu();

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
}
