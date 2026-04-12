use super::*;

const MBC1_STANDARD_ROM_NAME: &str = "phase6_mbc1_standard_banking.gb";
const MBC1_STANDARD_SERIAL: &[u8] = &[b'M', b'1', b'S', b':', 0x01, 0x1F, 0x11, 0x22];
const MBC1_SMALL_ROM_NAME: &str = "phase6_mbc1_small_rom_mask_and_ram.gb";
const MBC1_SMALL_SERIAL: &[u8] = &[b'M', b'1', b'M', b':', 0x01, 0x00, 0x33, 0x44];
const STANDARD_SENTINEL_ADDRESS: u16 = 0xC10F;
const SMALL_SENTINEL_ADDRESS: u16 = 0xC11F;

fn build_standard_mbc1_program() -> Vec<u8> {
    let mut program = ProgramBuilder::default();

    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC100);

    program.ld_a_imm(0x1F);
    program.ld_a16_from_a(0x2000);
    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC101);

    program.ld_a_imm(0x0A);
    program.ld_a16_from_a(0x0000);
    program.ld_a_imm(0x11);
    program.ld_a16_from_a(0xA000);

    program.ld_a_imm(0x01);
    program.ld_a16_from_a(0x6000);
    program.ld_a_imm(0x02);
    program.ld_a16_from_a(0x4000);
    program.ld_a_imm(0x22);
    program.ld_a16_from_a(0xA000);

    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x6000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC102);

    program.ld_a_imm(0x01);
    program.ld_a16_from_a(0x6000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC103);

    program.emit_serial_bytes(b"M1S:");
    for address in [0xC100, 0xC101, 0xC102, 0xC103] {
        program.emit_serial_from_a16(address);
    }

    program.ld_a_imm(SENTINEL_VALUE);
    program.ld_a16_from_a(STANDARD_SENTINEL_ADDRESS);
    program.jr_self();

    program.into_bytes()
}

fn build_small_mask_mbc1_program() -> Vec<u8> {
    let mut program = ProgramBuilder::default();

    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC110);

    program.ld_a_imm(0x04);
    program.ld_a16_from_a(0x2000);
    program.ld_a_from_a16(0x4000);
    program.ld_a16_from_a(0xC111);

    program.ld_a_imm(0x0A);
    program.ld_a16_from_a(0x0000);
    program.ld_a_imm(0x33);
    program.ld_a16_from_a(0xA000);

    program.ld_a_imm(0x01);
    program.ld_a16_from_a(0x6000);
    program.ld_a_imm(0x03);
    program.ld_a16_from_a(0x4000);
    program.ld_a_imm(0x44);
    program.ld_a16_from_a(0xA000);

    program.ld_a_imm(0x00);
    program.ld_a16_from_a(0x6000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC112);

    program.ld_a_imm(0x01);
    program.ld_a16_from_a(0x6000);
    program.ld_a_from_a16(0xA000);
    program.ld_a16_from_a(0xC113);

    program.emit_serial_bytes(b"M1M:");
    for address in [0xC110, 0xC111, 0xC112, 0xC113] {
        program.emit_serial_from_a16(address);
    }

    program.ld_a_imm(SENTINEL_VALUE);
    program.ld_a16_from_a(SMALL_SENTINEL_ADDRESS);
    program.jr_self();

    program.into_bytes()
}

fn build_standard_mbc1_rom() -> Vec<u8> {
    BankedCartridgeBuilder::new(0x04, 0x03, 0x03)
        .stamp_bank_start_markers()
        .write_bank_bytes(16, 0x0104, &[0x10])
        .write_program(&build_standard_mbc1_program())
        .build()
}

fn build_small_mask_mbc1_rom() -> Vec<u8> {
    BankedCartridgeBuilder::new(0x01, 0x03, 0x03)
        .stamp_bank_start_markers()
        .write_program(&build_small_mask_mbc1_program())
        .build()
}

#[test]
fn phase_6_mbc1_standard_banking_rom_fixture_matches_expected_state_and_serial() {
    let expected_rom = build_standard_mbc1_rom();
    let mut machine = load_fixture_machine(MBC1_STANDARD_ROM_NAME, &expected_rom);

    step_until_wram_sentinel(&mut machine, STANDARD_SENTINEL_ADDRESS);

    assert_eq!(machine.read_bus(0xC100), 0x01);
    assert_eq!(machine.read_bus(0xC101), 0x1F);
    assert_eq!(machine.read_bus(0xC102), 0x11);
    assert_eq!(machine.read_bus(0xC103), 0x22);
    assert_serial_output(&mut machine, MBC1_STANDARD_SERIAL);
}

#[test]
fn phase_6_mbc1_small_rom_mask_and_ram_fixture_matches_expected_state_and_serial() {
    let expected_rom = build_small_mask_mbc1_rom();
    let mut machine = load_fixture_machine(MBC1_SMALL_ROM_NAME, &expected_rom);

    step_until_wram_sentinel(&mut machine, SMALL_SENTINEL_ADDRESS);

    assert_eq!(machine.read_bus(0xC110), 0x01);
    assert_eq!(machine.read_bus(0xC111), 0x00);
    assert_eq!(machine.read_bus(0xC112), 0x33);
    assert_eq!(machine.read_bus(0xC113), 0x44);
    assert_serial_output(&mut machine, MBC1_SMALL_SERIAL);
}
