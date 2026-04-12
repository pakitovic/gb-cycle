use super::*;

const MBC5_ROM_NAME: &str = "phase6_mbc5_rom_banking_rumble_and_ram.gb";
const MBC5_SERIAL: &[u8] = &[
    b'M', b'5', b':', 0x01, 0x00, 0xFF, 0x00, 0x00, 0x01, 0x33, 0x11, 0x33,
];
const MBC5_SENTINEL_ADDRESS: u16 = 0xC14F;

fn build_mbc5_program() -> Vec<u8> {
    let mut program = ProgramBuilder::default();

    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC140);
    program.ld_a_from_a16(0x4001);
    program.ld_a16_from_a(0xC141);

    program.ld_a_imm(0xFF);
    program.ld_a16_from_a(0x2000);
    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x3000);
    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC142);
    program.ld_a_from_a16(0x4001);
    program.ld_a16_from_a(0xC143);

    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x2000);
    program.ld_a_imm(0x01);
    program.ld_a16_from_a(0x3000);
    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC144);
    program.ld_a_from_a16(0x4001);
    program.ld_a16_from_a(0xC145);

    program.ld_a_imm(0x0A);
    program.ld_a16_from_a(0x0000);

    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x4000);
    program.ld_a_imm(0x11);
    program.ld_a16_from_a(0xA000);

    program.ld_a_imm(0x03);
    program.ld_a16_from_a(0x4000);
    program.ld_a_imm(0x33);
    program.ld_a16_from_a(0xA000);

    program.ld_a_imm(0x0B);
    program.ld_a16_from_a(0x4000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC146);

    program.ld_a_imm(0x08);
    program.ld_a16_from_a(0x4000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC147);

    program.ld_a_imm(0x0B);
    program.ld_a16_from_a(0x4000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC148);

    program.emit_serial_bytes(b"M5:");
    for address in [
        0xC140, 0xC141, 0xC142, 0xC143, 0xC144, 0xC145, 0xC146, 0xC147, 0xC148,
    ] {
        program.emit_serial_from_a16(address);
    }

    program.ld_a_imm(SENTINEL_VALUE);
    program.ld_a16_from_a(MBC5_SENTINEL_ADDRESS);
    program.jr_self();

    program.into_bytes()
}

fn build_mbc5_rom() -> Vec<u8> {
    BankedCartridgeBuilder::new(0x08, 0x1E, 0x03)
        .stamp_bank_identity_markers()
        .write_program(&build_mbc5_program())
        .build()
}

#[test]
fn phase_6_mbc5_rom_banking_rumble_and_ram_fixture_matches_expected_state_and_serial() {
    let expected_rom = build_mbc5_rom();
    let mut machine = load_fixture_machine(MBC5_ROM_NAME, &expected_rom);

    step_until_wram_sentinel(&mut machine, MBC5_SENTINEL_ADDRESS);

    assert_eq!(machine.read_bus(0xC140), 0x01);
    assert_eq!(machine.read_bus(0xC141), 0x00);
    assert_eq!(machine.read_bus(0xC142), 0xFF);
    assert_eq!(machine.read_bus(0xC143), 0x00);
    assert_eq!(machine.read_bus(0xC144), 0x00);
    assert_eq!(machine.read_bus(0xC145), 0x01);
    assert_eq!(machine.read_bus(0xC146), 0x33);
    assert_eq!(machine.read_bus(0xC147), 0x11);
    assert_eq!(machine.read_bus(0xC148), 0x33);
    assert!(machine.cartridge().rumble_on());
    assert_serial_output(&mut machine, MBC5_SERIAL);
}
