use super::*;

#[test]
fn cb_decoder_and_apply_helpers_cover_private_paths() {
    let mut cpu = power_on_cpu();

    let rlc = cpu.decode_cb_opcode(0x07);
    assert_eq!(rlc.target(), Register8Operand::Register(Register8::A));
    assert_eq!(cpu.apply_cb_operation(rlc, 0x80), Some(0x01));
    assert_eq!(cpu.registers.f, FLAG_C);

    let rrc = cpu.decode_cb_opcode(0x08);
    assert_eq!(cpu.apply_cb_operation(rrc, 0x01), Some(0x80));
    assert_eq!(cpu.registers.f, FLAG_C);

    let sla = cpu.decode_cb_opcode(0x20);
    assert_eq!(cpu.apply_cb_operation(sla, 0x81), Some(0x02));
    assert_eq!(cpu.registers.f, FLAG_C);

    let sra = cpu.decode_cb_opcode(0x28);
    assert_eq!(cpu.apply_cb_operation(sra, 0x81), Some(0xC0));
    assert_eq!(cpu.registers.f, FLAG_C);

    let swap = cpu.decode_cb_opcode(0x30);
    assert_eq!(cpu.apply_cb_operation(swap, 0xF0), Some(0x0F));
    assert_eq!(cpu.registers.f, 0);

    let srl = cpu.decode_cb_opcode(0x38);
    assert_eq!(cpu.apply_cb_operation(srl, 0x01), Some(0x00));
    assert_eq!(cpu.registers.f, FLAG_Z | FLAG_C);

    cpu.registers.f = FLAG_C;
    let bit = cpu.decode_cb_opcode(0x58);
    assert_eq!(cpu.apply_cb_operation(bit, 0x00), None);
    assert_eq!(cpu.registers.f, FLAG_Z | FLAG_H | FLAG_C);

    let reset = cpu.decode_cb_opcode(0x80);
    assert_eq!(cpu.apply_cb_operation(reset, 0xFF), Some(0xFE));

    let set = cpu.decode_cb_opcode(0xFF);
    assert_eq!(set.target(), Register8Operand::Register(Register8::A));
    assert_eq!(cpu.apply_cb_operation(set, 0x00), Some(0x80));
}

#[test]
fn cb_bit_test_on_hl_finishes_without_writeback() {
    let mut cpu = power_on_cpu();
    cpu.write_register16(Register16::HL, 0xC123);
    cpu.instruction_kind = Some(CpuInstructionKind::CbPrefixed);
    cpu.cb_instruction_kind = Some(CbInstructionKind::BitTest {
        bit: 0,
        target: Register8Operand::IndirectHl,
    });

    cpu.complete_execute_machine_cycle(0xCB, 1, &mut |_| Some(0x00));

    assert_eq!(cpu.execution_state, CpuExecutionState::fetch_opcode());
}
