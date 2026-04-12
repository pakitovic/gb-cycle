use super::*;

const MBC2_ROM_NAME: &str = "phase6_mbc2_control_decode_and_nibble_ram.gb";
const MBC2_SERIAL: &[u8] = &[b'M', b'2', b':', 0x01, 0x03, 0xFB, 0xFB, 0xFB];
const MBC2_SENTINEL_ADDRESS: u16 = 0xC12F;

fn build_mbc2_program() -> Vec<u8> {
    let mut program = ProgramBuilder::default();

    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC120);

    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x0100);
    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC121);

    program.ld_a_imm(0x03);
    program.ld_a16_from_a(0x2100);
    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC122);

    program.ld_a_imm(0x0A);
    program.ld_a16_from_a(0x0000);
    program.ld_a_imm(0xAB);
    program.ld_a16_from_a(0xA000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC123);
    program.ld_a_from_a16(0xA200);
    program.ld_a16_from_a(0xC124);

    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x0000);
    program.ld_a_imm(0x0C);
    program.ld_a16_from_a(0xA000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC125);

    program.ld_a_imm(0x0A);
    program.ld_a16_from_a(0x0000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC126);

    program.emit_serial_bytes(b"M2:");
    for address in [0xC120, 0xC122, 0xC123, 0xC124, 0xC126] {
        program.emit_serial_from_a16(address);
    }

    program.ld_a_imm(SENTINEL_VALUE);
    program.ld_a16_from_a(MBC2_SENTINEL_ADDRESS);
    program.jr_self();

    program.into_bytes()
}

fn build_mbc2_rom() -> Vec<u8> {
    BankedCartridgeBuilder::new(0x03, 0x06, 0x00)
        .stamp_bank_start_markers()
        .write_program(&build_mbc2_program())
        .build()
}

#[test]
fn phase_6_mbc2_control_decode_and_nibble_ram_fixture_matches_expected_state_and_serial() {
    let expected_rom = build_mbc2_rom();
    let mut machine = load_fixture_machine(MBC2_ROM_NAME, &expected_rom);

    step_until_wram_sentinel(&mut machine, MBC2_SENTINEL_ADDRESS);

    assert_eq!(machine.read_bus(0xC120), 0x01);
    assert_eq!(machine.read_bus(0xC121), 0x01);
    assert_eq!(machine.read_bus(0xC122), 0x03);
    assert_eq!(machine.read_bus(0xC123), 0xFB);
    assert_eq!(machine.read_bus(0xC124), 0xFB);
    assert_eq!(machine.read_bus(0xC125), 0xFF);
    assert_eq!(machine.read_bus(0xC126), 0xFB);
    assert_serial_output(&mut machine, MBC2_SERIAL);
}
