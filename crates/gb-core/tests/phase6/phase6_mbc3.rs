use super::*;

const MBC3_ROM_NAME: &str = "phase6_mbc3_banking_ram_and_rtc.gb";
const MBC3_SERIAL: &[u8] = &[
    b'M', b'3', b':', 0x01, 0x20, 0x40, 0x60, 0x33, 0x55, 0x04, 0x03, 0x02, 0x01, 0x04, 0x2A,
];
const MBC3_SENTINEL_ADDRESS: u16 = 0xC13F;

fn build_mbc3_program() -> Vec<u8> {
    let mut program = ProgramBuilder::default();

    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC130);

    for (bank, destination) in [(0x20, 0xC131), (0x40, 0xC132), (0x60, 0xC133)] {
        program.ld_a_imm(bank);
        program.ld_a16_from_a(0x2000);
        program.ld_a_from_a16(0x4000);
        program.ld_a16_from_a(destination);
    }

    program.ld_a_imm(0x0A);
    program.ld_a16_from_a(0x0000);

    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x4000);
    program.ld_a_imm(0x33);
    program.ld_a16_from_a(0xA000);

    program.ld_a_imm(0x02);
    program.ld_a16_from_a(0x4000);
    program.ld_a_imm(0x55);
    program.ld_a16_from_a(0xA000);

    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x4000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC134);

    program.ld_a_imm(0x02);
    program.ld_a16_from_a(0x4000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC135);

    program.ld_a_imm(0x08);
    program.ld_a16_from_a(0x4000);
    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x6000);
    program.ld_a_imm(0x01);
    program.ld_a16_from_a(0x6000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC136);

    program.ld_a_imm(0x09);
    program.ld_a16_from_a(0x4000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC137);

    program.ld_a_imm(0x0A);
    program.ld_a16_from_a(0x4000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC138);

    program.ld_a_imm(0x0B);
    program.ld_a16_from_a(0x4000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC139);

    program.ld_a_imm(0x08);
    program.ld_a16_from_a(0x4000);
    program.ld_a_imm(0x2A);
    program.ld_a16_from_a(0xA000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC13A);

    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x6000);
    program.ld_a_imm(0x01);
    program.ld_a16_from_a(0x6000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC13B);

    program.emit_serial_bytes(b"M3:");
    for address in [
        0xC130, 0xC131, 0xC132, 0xC133, 0xC134, 0xC135, 0xC136, 0xC137, 0xC138, 0xC139, 0xC13A,
        0xC13B,
    ] {
        program.emit_serial_from_a16(address);
    }

    program.ld_a_imm(SENTINEL_VALUE);
    program.ld_a16_from_a(MBC3_SENTINEL_ADDRESS);
    program.jr_self();

    program.into_bytes()
}

fn build_mbc3_rom() -> Vec<u8> {
    BankedCartridgeBuilder::new(0x06, 0x10, 0x03)
        .stamp_bank_start_markers()
        .write_program(&build_mbc3_program())
        .build()
}

#[test]
fn phase_6_mbc3_banking_ram_and_rtc_fixture_matches_expected_state_and_serial() {
    let expected_rom = build_mbc3_rom();
    let mut machine = load_fixture_machine(MBC3_ROM_NAME, &expected_rom);
    machine.advance_cartridge_rtc_seconds(93_784);

    step_until_wram_sentinel(&mut machine, MBC3_SENTINEL_ADDRESS);

    assert_eq!(machine.read_bus(0xC130), 0x01);
    assert_eq!(machine.read_bus(0xC131), 0x20);
    assert_eq!(machine.read_bus(0xC132), 0x40);
    assert_eq!(machine.read_bus(0xC133), 0x60);
    assert_eq!(machine.read_bus(0xC134), 0x33);
    assert_eq!(machine.read_bus(0xC135), 0x55);
    assert_eq!(machine.read_bus(0xC136), 0x04);
    assert_eq!(machine.read_bus(0xC137), 0x03);
    assert_eq!(machine.read_bus(0xC138), 0x02);
    assert_eq!(machine.read_bus(0xC139), 0x01);
    assert_eq!(machine.read_bus(0xC13A), 0x04);
    assert_eq!(machine.read_bus(0xC13B), 0x2A);
    assert_serial_output(&mut machine, MBC3_SERIAL);
}
