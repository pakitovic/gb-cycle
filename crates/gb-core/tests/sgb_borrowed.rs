mod common;

use gb_core::{CompatibilityPolicy, extract_initial_sgb_borrowed_border};

const SGB_COMMAND_PAL01: u8 = 0x00;
const SGB_COMMAND_PCT_TRN: u8 = 0x14;

fn build_sgb_pct_packet_program() -> Vec<u8> {
    let mut program = Vec::new();
    write_sgb_packet(&mut program, sgb_command_packet(SGB_COMMAND_PCT_TRN));
    write_in_frame_delay(&mut program);
    write_sgb_packet(&mut program, sgb_pal01_packet_with_backdrop(0x7BFF));
    write_program_loop(&mut program);
    program
}

fn sgb_command_packet(command_id: u8) -> [u8; 16] {
    let mut packet = [0_u8; 16];
    packet[0] = (command_id << 3) | 1;
    packet
}

fn sgb_pal01_packet_with_backdrop(rgb555: u16) -> [u8; 16] {
    let mut packet = sgb_command_packet(SGB_COMMAND_PAL01);
    let [low, high] = rgb555.to_le_bytes();
    packet[1] = low;
    packet[2] = high;
    packet
}

fn write_sgb_packet(program: &mut Vec<u8>, packet: [u8; 16]) {
    write_joyp(program, 0x00);
    write_joyp(program, 0x30);
    for byte in packet {
        for bit_index in 0..8 {
            write_joyp(
                program,
                if (byte >> bit_index) & 0x01 == 0 {
                    0x20
                } else {
                    0x10
                },
            );
            write_joyp(program, 0x30);
        }
    }
    write_joyp(program, 0x20);
    write_joyp(program, 0x30);
}

fn write_in_frame_delay(program: &mut Vec<u8>) {
    program.extend_from_slice(&[
        0x01, 0x00, 0x40, // ld bc,$4000
        0x0B, // dec bc
        0x78, // ld a,b
        0xB1, // or c
        0x20, 0xFB, // jr nz,$-5
    ]);
}

fn write_program_loop(program: &mut Vec<u8>) {
    let loop_address = common::synthetic_cartridge::PROGRAM_ENTRY_ADDRESS + program.len();
    let [loop_low, loop_high] = (loop_address as u16).to_le_bytes();
    program.extend_from_slice(&[0xC3, loop_low, loop_high]);
}

fn write_joyp(program: &mut Vec<u8>, value: u8) {
    program.extend_from_slice(&[0x3E, value, 0xE0, 0x00]);
}

fn build_sgb_supported_pct_rom() -> Vec<u8> {
    let mut rom = common::synthetic_cartridge::build_nom_bc_test_rom_with_program_entry(
        &build_sgb_pct_packet_program(),
        0x00,
        common::synthetic_cartridge::PROGRAM_ENTRY_ADDRESS,
        &[],
    );
    rom[0x0146] = 0x03;
    rom[0x014B] = 0x33;
    rom
}

#[test]
fn borrowed_border_extractor_runs_temporary_sgb_until_pct_transfer() {
    let borrowed = extract_initial_sgb_borrowed_border(
        &build_sgb_supported_pct_rom(),
        &CompatibilityPolicy::strict(),
    )
    .expect("synthetic SGB ROM should transfer an initial border packet");

    assert!(borrowed.border().pct_loaded);
    assert_eq!(borrowed.border().pct_transfer_count, 1);
    assert_eq!(
        borrowed.backdrop_color().raw(),
        0x7BFF,
        "extractor should keep the temporary SGB running until the initial border presentation settles, so Pokémon Yellow-style palette commands after PCT_TRN can update transparent border pixels"
    );
}
