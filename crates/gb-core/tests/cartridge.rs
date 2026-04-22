use gb_core::{
    BootRomBusState, Bus, BusAccessKind, BusArbitrationState, BusRequester,
    CartridgeClassification, CartridgeExternalAccessInfo, CartridgeExternalAvailability,
    CartridgeExternalReadBehavior, CartridgeExternalTarget, CartridgeExternalWriteBehavior,
    CartridgeHeader, CartridgePersistenceProfile, CartridgePersistentStateError,
    CartridgeRamPayloadKind, CartridgeRtcRegister, CartridgeSelection, CartridgeSlot,
    CartridgeSlotState, CompatibilityPolicy, ConsoleModel, DiagnosticPolicy, HeuristicPolicy,
    Machine, MachineConfig, Mbc3RtcPersistentState, OverridePolicy, PersistentCartState,
    StartupMode, SupportedCartridgeFamily, TCycle, UnsupportedCartridgeCategory, ValidationPolicy,
};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
const ENTRY_POINT_START: usize = 0x0100;
const LOGO_START: usize = 0x0104;
const TITLE_START: usize = 0x0134;
const MANUFACTURER_CODE_START: usize = 0x013F;
const MANUFACTURER_CODE_END_INCLUSIVE: usize = 0x0142;
const CGB_FLAG_ADDRESS: usize = 0x0143;
const NEW_LICENSEE_CODE_START: usize = 0x0144;
const SGB_FLAG_ADDRESS: usize = 0x0146;
const CARTRIDGE_TYPE_ADDRESS: usize = 0x0147;
const ROM_SIZE_ADDRESS: usize = 0x0148;
const RAM_SIZE_ADDRESS: usize = 0x0149;
const DESTINATION_CODE_ADDRESS: usize = 0x014A;
const OLD_LICENSEE_CODE_ADDRESS: usize = 0x014B;
const HEADER_CHECKSUM_ADDRESS: usize = 0x014D;

fn build_test_rom(len: usize, cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let mut rom = vec![0xFF; len.max(HEADER_MINIMUM_ROM_LEN)];
    rom[0x0000] = 0x12;
    rom[ENTRY_POINT_START..ENTRY_POINT_START + 4].copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
    rom[LOGO_START..LOGO_START + 48].copy_from_slice(&[0xCE; 48]);
    rom[TITLE_START..TITLE_START + 8].copy_from_slice(b"PHASE1C!");
    rom[CGB_FLAG_ADDRESS] = 0x80;
    rom[SGB_FLAG_ADDRESS] = 0x03;
    rom[CARTRIDGE_TYPE_ADDRESS] = cartridge_type;
    rom[ROM_SIZE_ADDRESS] = rom_size_code;
    rom[RAM_SIZE_ADDRESS] = ram_size_code;
    rom[HEADER_CHECKSUM_ADDRESS] = 0x7F;
    rom[0x3FFF] = 0x34;
    rom[0x4000] = 0x56;
    rom
}

fn build_banked_mbc1_rom(rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let rom_size = match rom_size_code {
        0x00 => 32 * 1024,
        0x01 => 64 * 1024,
        0x02 => 128 * 1024,
        0x03 => 256 * 1024,
        0x04 => 512 * 1024,
        0x05 => 1024 * 1024,
        0x06 => 2 * 1024 * 1024,
        _ => panic!("unsupported MBC1 ROM size code for test"),
    };
    let bank_count = rom_size / 0x4000;
    let cartridge_type = if ram_size_code == 0x00 { 0x01 } else { 0x03 };
    let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

fn build_mmm01_rom(rom_size_code: u8, ram_size_code: u8, cartridge_type: u8) -> Vec<u8> {
    let rom_size = match rom_size_code {
        0x01 => 64 * 1024,
        0x02 => 128 * 1024,
        0x03 => 256 * 1024,
        0x04 => 512 * 1024,
        0x05 => 1024 * 1024,
        0x06 => 2 * 1024 * 1024,
        0x07 => 4 * 1024 * 1024,
        0x08 => 8 * 1024 * 1024,
        _ => panic!("unsupported MMM01 ROM size code for test"),
    };
    let bank_count = rom_size / 0x4000;
    let mut rom = vec![0xFF; rom_size.max(HEADER_MINIMUM_ROM_LEN)];

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom[ENTRY_POINT_START..ENTRY_POINT_START + 4].copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
    rom[LOGO_START..LOGO_START + 48].copy_from_slice(&[0xCE; 48]);
    rom[TITLE_START..TITLE_START + 7].copy_from_slice(b"GAMEONE");
    rom[CARTRIDGE_TYPE_ADDRESS] = 0x00;
    rom[ROM_SIZE_ADDRESS] = 0x00;
    rom[RAM_SIZE_ADDRESS] = 0x00;

    let menu_offset = rom_size - 32 * 1024;
    rom[menu_offset + ENTRY_POINT_START..menu_offset + ENTRY_POINT_START + 4]
        .copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
    rom[menu_offset + LOGO_START..menu_offset + LOGO_START + 48].copy_from_slice(&[0xCE; 48]);
    rom[menu_offset + TITLE_START..menu_offset + TITLE_START + 7].copy_from_slice(b"MMM01!!");
    rom[menu_offset + CGB_FLAG_ADDRESS] = 0x80;
    rom[menu_offset + SGB_FLAG_ADDRESS] = 0x03;
    rom[menu_offset + CARTRIDGE_TYPE_ADDRESS] = cartridge_type;
    rom[menu_offset + ROM_SIZE_ADDRESS] = rom_size_code;
    rom[menu_offset + RAM_SIZE_ADDRESS] = ram_size_code;
    rom[menu_offset + HEADER_CHECKSUM_ADDRESS] = 0x7F;

    rom
}

fn build_banked_mbc2_rom(cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let rom_size = match rom_size_code {
        0x00 => 32 * 1024,
        0x01 => 64 * 1024,
        0x02 => 128 * 1024,
        0x03 => 256 * 1024,
        0x04 => 512 * 1024,
        _ => panic!("unsupported MBC2 ROM size code for test"),
    };
    let bank_count = rom_size / 0x4000;
    let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

fn build_banked_mbc3_rom(cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let rom_size = match rom_size_code {
        0x00 => 32 * 1024,
        0x01 => 64 * 1024,
        0x02 => 128 * 1024,
        0x03 => 256 * 1024,
        0x04 => 512 * 1024,
        0x05 => 1024 * 1024,
        0x06 => 2 * 1024 * 1024,
        _ => panic!("unsupported MBC3 ROM size code for test"),
    };
    let bank_count = rom_size / 0x4000;
    let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

fn build_banked_mbc5_rom(cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let rom_size = match rom_size_code {
        0x00 => 32 * 1024,
        0x01 => 64 * 1024,
        0x02 => 128 * 1024,
        0x03 => 256 * 1024,
        0x04 => 512 * 1024,
        0x05 => 1024 * 1024,
        0x06 => 2 * 1024 * 1024,
        0x07 => 4 * 1024 * 1024,
        0x08 => 8 * 1024 * 1024,
        _ => panic!("unsupported MBC5 ROM size code for test"),
    };
    let bank_count = rom_size / 0x4000;
    let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 1] = ((bank >> 8) & 0x01) as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

fn ignore_policy() -> CompatibilityPolicy {
    CompatibilityPolicy {
        execution_mode: gb_core::ExecutionMode::Permissive,
        validation_policy: ValidationPolicy::Ignore,
        heuristic_policy: HeuristicPolicy::Disabled,
        override_policy: OverridePolicy::default(),
        diagnostic_policy: DiagnosticPolicy::Standard,
    }
}

#[path = "cartridge/cartridge_header_basic.rs"]
mod cartridge_header_basic;
#[path = "cartridge/cartridge_mbc1.rs"]
mod cartridge_mbc1;
#[path = "cartridge/cartridge_mbc2.rs"]
mod cartridge_mbc2;
#[path = "cartridge/cartridge_mbc3.rs"]
mod cartridge_mbc3;
#[path = "cartridge/cartridge_mbc5.rs"]
mod cartridge_mbc5;
#[path = "cartridge/cartridge_mmm01.rs"]
mod cartridge_mmm01;
