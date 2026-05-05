use super::*;

const MBC6_ROM_NAME: &str = "phase6_mbc6_split_window_flash.gb";
const MBC6_SERIAL: &[u8] = &[
    b'M', b'6', b':', 0x02, 0x03, 0x04, 0x05, 0x00, 0x11, 0x22, 0x33, 0x44, 0xC2, 0x81, 0x80, 0x5A,
    0x80, 0x3C,
];
const MBC6_SENTINEL_ADDRESS: u16 = 0xC17F;

fn mbc6_flash_unlock(program: &mut ProgramBuilder) {
    program.ld_a_imm(0x02);
    program.ld_a16_from_a(0x2000);
    program.ld_a_imm(0xAA);
    program.ld_a16_from_a(0x5555);
    program.ld_a_imm(0x01);
    program.ld_a16_from_a(0x2000);
    program.ld_a_imm(0x55);
    program.ld_a16_from_a(0x4AAA);
    program.ld_a_imm(0x02);
    program.ld_a16_from_a(0x2000);
}

fn build_mbc6_program() -> Vec<u8> {
    let mut program = ProgramBuilder::default();

    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC160);
    program.ld_a_from_a16(0x6000);
    program.ld_a16_from_a(0xC161);

    program.ld_a_imm(0x04);
    program.ld_a16_from_a(0x2000);
    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC162);
    program.ld_a_imm(0x05);
    program.ld_a16_from_a(0x3000);
    program.ld_a_from_a16(0x6000);
    program.ld_a16_from_a(0xC163);
    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x2000);
    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC164);

    program.ld_a_imm(0x0A);
    program.ld_a16_from_a(0x0000);
    program.ld_a_imm(0x11);
    program.ld_a16_from_a(0xA000);
    program.ld_a_imm(0x22);
    program.ld_a16_from_a(0xB000);
    program.ld_a_imm(0x02);
    program.ld_a16_from_a(0x0400);
    program.ld_a_imm(0x03);
    program.ld_a16_from_a(0x0800);
    program.ld_a_imm(0x33);
    program.ld_a16_from_a(0xA000);
    program.ld_a_imm(0x44);
    program.ld_a16_from_a(0xB000);
    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x0400);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC165);
    program.ld_a_imm(0x01);
    program.ld_a16_from_a(0x0800);
    program.ld_a_from_a16(0xB000);
    program.ld_a16_from_a(0xC166);
    program.ld_a_imm(0x02);
    program.ld_a16_from_a(0x0400);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC167);
    program.ld_a_imm(0x03);
    program.ld_a16_from_a(0x0800);
    program.ld_a_from_a16(0xB000);
    program.ld_a16_from_a(0xC168);

    program.ld_a_imm(0x01);
    program.ld_a16_from_a(0x0C00);
    program.ld_a16_from_a(0x1000);
    program.ld_a_imm(0x08);
    program.ld_a16_from_a(0x2800);

    mbc6_flash_unlock(&mut program);
    program.ld_a_imm(0x90);
    program.ld_a16_from_a(0x5555);
    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC169);
    program.ld_a_from_a16(0x4001);
    program.ld_a16_from_a(0xC16A);
    program.ld_a_imm(0xF0);
    program.ld_a16_from_a(0x4000);

    mbc6_flash_unlock(&mut program);
    program.ld_a_imm(0xA0);
    program.ld_a16_from_a(0x5555);
    program.ld_a_imm(0x5A);
    program.ld_a16_from_a(0x4000);
    program.ld_a_imm(0x7F);
    program.ld_a16_from_a(0x407F);
    program.ld_a16_from_a(0x407F);
    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC16B);
    program.ld_a_imm(0xF0);
    program.ld_a16_from_a(0x4000);
    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC16C);

    mbc6_flash_unlock(&mut program);
    program.ld_a_imm(0x60);
    program.ld_a16_from_a(0x5555);
    mbc6_flash_unlock(&mut program);
    program.ld_a_imm(0xE0);
    program.ld_a16_from_a(0x5555);
    program.ld_a_imm(0x3C);
    program.ld_a16_from_a(0x4000);
    program.ld_a_imm(0x7E);
    program.ld_a16_from_a(0x407F);
    program.ld_a16_from_a(0x407F);
    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC16D);
    program.ld_a_imm(0xF0);
    program.ld_a16_from_a(0x4000);

    mbc6_flash_unlock(&mut program);
    program.ld_a_imm(0x77);
    program.ld_a16_from_a(0x5555);
    mbc6_flash_unlock(&mut program);
    program.ld_a_imm(0x77);
    program.ld_a16_from_a(0x5555);
    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC16E);
    program.ld_a_imm(0xF0);
    program.ld_a16_from_a(0x4000);

    program.emit_serial_bytes(b"M6:");
    for address in [
        0xC160, 0xC161, 0xC162, 0xC163, 0xC164, 0xC165, 0xC166, 0xC167, 0xC168, 0xC169, 0xC16A,
        0xC16B, 0xC16C, 0xC16D, 0xC16E,
    ] {
        program.emit_serial_from_a16(address);
    }

    program.ld_a_imm(SENTINEL_VALUE);
    program.ld_a16_from_a(MBC6_SENTINEL_ADDRESS);
    program.jr_self();

    program.into_bytes()
}

fn build_mbc6_rom() -> Vec<u8> {
    BankedCartridgeBuilder::new(0x05, 0x20, 0x03)
        .with_cgb_flag(0x80)
        .stamp_8kib_bank_identity_markers()
        .write_program(&build_mbc6_program())
        .build()
}

#[test]
fn phase_6_mbc6_split_window_flash_fixture_matches_expected_state_and_serial() {
    let expected_rom = build_mbc6_rom();
    let mut machine =
        load_fixture_machine_with_model(MBC6_ROM_NAME, &expected_rom, ConsoleModel::GameBoyColor);

    step_until_wram_sentinel(&mut machine, MBC6_SENTINEL_ADDRESS);

    assert_eq!(machine.read_bus(0xC160), 0x02);
    assert_eq!(machine.read_bus(0xC161), 0x03);
    assert_eq!(machine.read_bus(0xC162), 0x04);
    assert_eq!(machine.read_bus(0xC163), 0x05);
    assert_eq!(machine.read_bus(0xC164), 0x00);
    assert_eq!(machine.read_bus(0xC165), 0x11);
    assert_eq!(machine.read_bus(0xC166), 0x22);
    assert_eq!(machine.read_bus(0xC167), 0x33);
    assert_eq!(machine.read_bus(0xC168), 0x44);
    assert_eq!(machine.read_bus(0xC169), 0xC2);
    assert_eq!(machine.read_bus(0xC16A), 0x81);
    assert_eq!(machine.read_bus(0xC16B), 0x80);
    assert_eq!(machine.read_bus(0xC16C), 0x5A);
    assert_eq!(machine.read_bus(0xC16D), 0x80);
    assert_eq!(machine.read_bus(0xC16E), 0x3C);
    assert_serial_output(&mut machine, MBC6_SERIAL);
}
