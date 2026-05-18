use gb_core::{
    BootRomBusState, Bus, BusAccessKind, BusArbitrationState, BusRequester,
    CartridgeClassification, CartridgeExternalAccessInfo, CartridgeExternalAvailability,
    CartridgeExternalReadBehavior, CartridgeExternalTarget, CartridgeExternalWriteBehavior,
    CartridgeHeader, CartridgePersistenceProfile, CartridgePersistentStateError,
    CartridgeRamPayloadKind, CartridgeRomLayoutSource, CartridgeRtcRegister, CartridgeSelection,
    CartridgeSlot, CartridgeSlotState, CompatibilityPolicy, ConsoleModel, DiagnosticPolicy,
    HeuristicPolicy, Huc3RtcPersistentState, Machine, MachineConfig, Mbc3RtcPersistentState,
    OverridePolicy, PersistentCartState, RomSizeInfo, StartupMode, SupportedCartridgeFamily,
    TCycle, UnsupportedCartridgeCategory, ValidationPolicy,
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
const M161_BANK_BYTES: usize = 32 * 1024;

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

fn mark_mbc1_multicart_subheaders_in_banks(rom: &mut [u8], banks: &[usize]) {
    let logo = rom[LOGO_START..LOGO_START + 48].to_vec();

    for &bank in banks {
        let start = bank * 0x4000 + LOGO_START;
        rom[start..start + 48].copy_from_slice(&logo);
    }
}

fn mark_mbc1_multicart_subheaders(rom: &mut [u8]) {
    mark_mbc1_multicart_subheaders_in_banks(rom, &[0x10, 0x20, 0x30]);
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
    let secondary_header_offset = (menu_offset / 2 / 0x4000) * 0x4000;
    rom[secondary_header_offset + ENTRY_POINT_START
        ..secondary_header_offset + ENTRY_POINT_START + 4]
        .copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
    rom[secondary_header_offset + LOGO_START..secondary_header_offset + LOGO_START + 48]
        .copy_from_slice(&[0xCE; 48]);
    rom[secondary_header_offset + TITLE_START..=secondary_header_offset + TITLE_START + 15]
        .fill(0x00);
    rom[secondary_header_offset + TITLE_START..secondary_header_offset + TITLE_START + 7]
        .copy_from_slice(b"GAMETWO");
    rom[secondary_header_offset + CARTRIDGE_TYPE_ADDRESS] = 0x00;
    rom[secondary_header_offset + ROM_SIZE_ADDRESS] = 0x00;
    rom[secondary_header_offset + RAM_SIZE_ADDRESS] = 0x00;
    rom[secondary_header_offset + HEADER_CHECKSUM_ADDRESS] = 0x7F;

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

fn build_mani_mmm01_rom(rom_size_code: u8) -> Vec<u8> {
    let (rom_size, game_headers, menu_title) = match rom_size_code {
        0x04 => (
            512 * 1024,
            [
                (0x00000usize, b"SAGAIA".as_slice(), 0x02),
                (0x20000, b"CHASE HQ".as_slice(), 0x02),
                (0x40000, b"BUBBLE BOBBLE".as_slice(), 0x02),
                (0x60000, b"ELEVATOR ACTION".as_slice(), 0x01),
            ],
            b"SAGAIA SET".as_slice(),
        ),
        0x05 => (
            1024 * 1024,
            [
                (0x00000usize, b"GB GENJIN".as_slice(), 0x03),
                (0x40000, b"BOMBER BOY".as_slice(), 0x02),
                (0x60000, b"MILON CASTLE".as_slice(), 0x02),
                (0x80000, b"BOUKENJIMA2".as_slice(), 0x02),
            ],
            b"GB GENJIN SET".as_slice(),
        ),
        _ => panic!("unsupported Mani MMM01 ROM size code for test"),
    };
    let bank_count = rom_size / 0x4000;
    let mut rom = vec![0xFF; rom_size.max(HEADER_MINIMUM_ROM_LEN)];

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    for (base_offset, title, game_rom_size_code) in game_headers {
        rom[base_offset + ENTRY_POINT_START..base_offset + ENTRY_POINT_START + 4]
            .copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
        rom[base_offset + LOGO_START..base_offset + LOGO_START + 48].copy_from_slice(&[0xCE; 48]);
        rom[base_offset + TITLE_START..=base_offset + TITLE_START + 15].fill(0x00);
        rom[base_offset + TITLE_START..base_offset + TITLE_START + title.len()]
            .copy_from_slice(title);
        rom[base_offset + CARTRIDGE_TYPE_ADDRESS] = 0x01;
        rom[base_offset + ROM_SIZE_ADDRESS] = game_rom_size_code;
        rom[base_offset + RAM_SIZE_ADDRESS] = 0x00;
        rom[base_offset + HEADER_CHECKSUM_ADDRESS] = 0x7F;
    }

    let menu_offset = rom_size - 32 * 1024;
    rom[menu_offset + ENTRY_POINT_START..menu_offset + ENTRY_POINT_START + 4]
        .copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
    rom[menu_offset + LOGO_START..menu_offset + LOGO_START + 48].copy_from_slice(&[0xCE; 48]);
    rom[menu_offset + TITLE_START..=menu_offset + TITLE_START + 15].fill(0x00);
    rom[menu_offset + TITLE_START..menu_offset + TITLE_START + menu_title.len()]
        .copy_from_slice(menu_title);
    rom[menu_offset + CARTRIDGE_TYPE_ADDRESS] = 0x11;
    rom[menu_offset + ROM_SIZE_ADDRESS] = rom_size_code;
    rom[menu_offset + RAM_SIZE_ADDRESS] = 0x00;
    rom[menu_offset + HEADER_CHECKSUM_ADDRESS] = 0x7F;

    rom
}

fn build_m161_signature_rom() -> Vec<u8> {
    let mut rom = vec![0xFF; 5 * M161_BANK_BYTES];
    let titles = [
        b"MANI 4 IN 1".as_slice(),
        b"TETRIS".as_slice(),
        b"TENNIS".as_slice(),
        b"ALLEY WAY".as_slice(),
        b"YAKUMAN".as_slice(),
    ];

    for (bank, title) in titles.into_iter().enumerate() {
        let start = bank * M161_BANK_BYTES;
        let bank_rom = &mut rom[start..start + M161_BANK_BYTES];
        bank_rom.fill(bank as u8);
        bank_rom[ENTRY_POINT_START..ENTRY_POINT_START + 4]
            .copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
        bank_rom[LOGO_START..LOGO_START + 48].copy_from_slice(&[0xCE; 48]);
        bank_rom[TITLE_START..=TITLE_START + 15].fill(0x00);
        bank_rom[TITLE_START..TITLE_START + title.len()].copy_from_slice(title);
        bank_rom[CARTRIDGE_TYPE_ADDRESS] = 0x00;
        bank_rom[ROM_SIZE_ADDRESS] = 0x00;
        bank_rom[RAM_SIZE_ADDRESS] = 0x00;
        bank_rom[HEADER_CHECKSUM_ADDRESS] = 0x7F;
    }

    rom
}

fn build_m161_commercial_rom() -> Vec<u8> {
    let mut rom = vec![0x00; 8 * M161_BANK_BYTES];
    let titles = [
        b"TETRIS SET".as_slice(),
        b"TETRIS".as_slice(),
        b"ALLEY WAY".as_slice(),
        b"YAKUMAN".as_slice(),
        b"TENNIS".as_slice(),
    ];

    for (bank, title) in titles.into_iter().enumerate() {
        let start = bank * M161_BANK_BYTES;
        let bank_rom = &mut rom[start..start + M161_BANK_BYTES];
        bank_rom.fill(bank as u8);
        bank_rom[ENTRY_POINT_START..ENTRY_POINT_START + 4]
            .copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
        bank_rom[LOGO_START..LOGO_START + 48].copy_from_slice(&[0xCE; 48]);
        bank_rom[TITLE_START..=TITLE_START + 15].fill(0x00);
        bank_rom[TITLE_START..TITLE_START + title.len()].copy_from_slice(title);
        bank_rom[RAM_SIZE_ADDRESS] = 0x00;
        if bank != 0 {
            bank_rom[CARTRIDGE_TYPE_ADDRESS] = 0x00;
            bank_rom[ROM_SIZE_ADDRESS] = 0x00;
        }
        bank_rom[HEADER_CHECKSUM_ADDRESS] = 0x7F;
    }

    rom[CARTRIDGE_TYPE_ADDRESS] = 0x10;
    rom[ROM_SIZE_ADDRESS] = 0x03;
    rom[RAM_SIZE_ADDRESS] = 0x00;
    rom[HEADER_CHECKSUM_ADDRESS] = 0x7F;

    rom
}

fn build_banked_huc1_rom(rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let rom_size = RomSizeInfo::decode(rom_size_code)
        .decoded_bytes
        .expect("test ROM size should decode");
    let bank_count = RomSizeInfo::decode(rom_size_code)
        .bank_count
        .expect("test ROM bank count should decode");
    let mut rom = build_test_rom(rom_size, 0xFF, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

fn build_banked_huc3_rom(rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let rom_size = RomSizeInfo::decode(rom_size_code)
        .decoded_bytes
        .expect("test ROM size should decode");
    let bank_count = RomSizeInfo::decode(rom_size_code)
        .bank_count
        .expect("test ROM bank count should decode");
    let mut rom = build_test_rom(rom_size, 0xFE, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

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
        0x07 => 4 * 1024 * 1024,
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
#[path = "cartridge/cartridge_huc1.rs"]
mod cartridge_huc1;
#[path = "cartridge/cartridge_huc3.rs"]
mod cartridge_huc3;
#[path = "cartridge/cartridge_m161.rs"]
mod cartridge_m161;
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
