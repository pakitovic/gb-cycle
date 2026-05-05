#![allow(dead_code)]

pub const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
pub const PROGRAM_ENTRY_ADDRESS: usize = 0x0150;
const TEST_ROM_SIZE: usize = 32 * 1024;

fn build_nom_bc_test_rom_base(boot_opcode: u8) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(TEST_ROM_SIZE)];
    rom[0x0000] = boot_opcode;
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom
}

pub fn build_nom_bc_test_rom(
    program: &[u8],
    boot_opcode: u8,
    extra_segments: &[(usize, &[u8])],
) -> Vec<u8> {
    let mut rom = build_nom_bc_test_rom_base(boot_opcode);
    rom[0x0100..0x0100 + program.len()].copy_from_slice(program);
    for &(address, bytes) in extra_segments {
        rom[address..address + bytes.len()].copy_from_slice(bytes);
    }
    rom
}

pub fn build_nom_bc_test_rom_with_program_entry(
    program: &[u8],
    boot_opcode: u8,
    program_entry_address: usize,
    extra_segments: &[(usize, &[u8])],
) -> Vec<u8> {
    let mut rom = build_nom_bc_test_rom_base(boot_opcode);
    let [entry_low, entry_high] = (program_entry_address as u16).to_le_bytes();
    rom[0x0100..0x0103].copy_from_slice(&[0xC3, entry_low, entry_high]);
    rom[program_entry_address..program_entry_address + program.len()].copy_from_slice(program);
    for &(address, bytes) in extra_segments {
        rom[address..address + bytes.len()].copy_from_slice(bytes);
    }
    rom
}

pub fn rom_size_bytes_from_standard_code(rom_size_code: u8) -> usize {
    match rom_size_code {
        0x00..=0x08 => 32 * 1024 * (1usize << rom_size_code),
        0x52 => 72 * 16 * 1024,
        0x53 => 80 * 16 * 1024,
        0x54 => 96 * 16 * 1024,
        _ => panic!("unsupported ROM size code for synthetic cartridge fixture"),
    }
}

pub struct BankedCartridgeBuilder {
    rom: Vec<u8>,
}

impl BankedCartridgeBuilder {
    pub fn new(rom_size_code: u8, cartridge_type: u8, ram_size_code: u8) -> Self {
        let rom_size = rom_size_bytes_from_standard_code(rom_size_code);
        let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(rom_size)];
        rom[0x0100..0x0104].copy_from_slice(&[0xC3, 0x50, 0x01, 0x00]);
        rom[0x0147] = cartridge_type;
        rom[0x0148] = rom_size_code;
        rom[0x0149] = ram_size_code;

        Self { rom }
    }

    pub fn write_program(mut self, program: &[u8]) -> Self {
        for (offset, byte) in program.iter().copied().enumerate() {
            self.rom[PROGRAM_ENTRY_ADDRESS + offset] = byte;
        }
        self
    }

    pub fn stamp_bank_start_markers(mut self) -> Self {
        let bank_count = self.rom.len() / 0x4000;
        for bank in 0..bank_count {
            self.rom[bank * 0x4000] = bank as u8;
        }
        self
    }

    pub fn stamp_bank_identity_markers(mut self) -> Self {
        let bank_count = self.rom.len() / 0x4000;
        for bank in 0..bank_count {
            let start = bank * 0x4000;
            self.rom[start] = bank as u8;
            self.rom[start + 1] = ((bank >> 8) & 0x01) as u8;
        }
        self
    }

    pub fn stamp_8kib_bank_identity_markers(mut self) -> Self {
        let bank_count = self.rom.len() / 0x2000;
        for bank in 0..bank_count {
            let start = bank * 0x2000;
            self.rom[start] = bank as u8;
            self.rom[start + 1] = (bank >> 8) as u8;
        }
        self
    }

    pub fn with_cgb_flag(mut self, cgb_flag: u8) -> Self {
        self.rom[0x0143] = cgb_flag;
        self
    }

    pub fn write_bank_bytes(mut self, bank: usize, offset_in_bank: usize, bytes: &[u8]) -> Self {
        let start = bank * 0x4000 + offset_in_bank;
        self.rom[start..start + bytes.len()].copy_from_slice(bytes);
        self
    }

    pub fn build(self) -> Vec<u8> {
        self.rom
    }
}

#[derive(Default)]
pub struct ProgramBuilder {
    bytes: Vec<u8>,
}

impl ProgramBuilder {
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn ld_a_imm(&mut self, value: u8) {
        self.bytes.extend_from_slice(&[0x3E, value]);
    }

    pub fn ld_a_from_a16(&mut self, address: u16) {
        let [low, high] = address.to_le_bytes();
        self.bytes.extend_from_slice(&[0xFA, low, high]);
    }

    pub fn ld_a16_from_a(&mut self, address: u16) {
        let [low, high] = address.to_le_bytes();
        self.bytes.extend_from_slice(&[0xEA, low, high]);
    }

    pub fn emit_serial_byte(&mut self, value: u8) {
        self.ld_a_imm(value);
        self.emit_serial_from_a();
    }

    pub fn emit_serial_bytes(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.emit_serial_byte(byte);
        }
    }

    pub fn emit_serial_from_a16(&mut self, address: u16) {
        self.ld_a_from_a16(address);
        self.emit_serial_from_a();
    }

    pub fn emit_serial_from_a(&mut self) {
        self.bytes.extend_from_slice(&[0xE0, 0x01]);
        self.ld_a_imm(0x81);
        self.bytes.extend_from_slice(&[0xE0, 0x02]);
        self.bytes.extend_from_slice(&[
            0xF0, 0x02, // ldh a, ($02)
            0xE6, 0x80, // and $80
            0x20, 0xFA, // jr nz, wait_serial_complete
        ]);
    }

    pub fn jr_self(&mut self) {
        self.bytes.extend_from_slice(&[0x18, 0xFE]);
    }
}
