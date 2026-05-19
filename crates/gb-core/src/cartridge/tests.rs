use super::*;
use crate::model::{
    CompatibilityPolicy, DiagnosticPolicy, HeuristicPolicy, OverridePolicy, ValidationPolicy,
};

mod general;
mod huc1;
mod huc3;
mod internal;
mod m161;
mod mapped_rom;
mod mbc1;
mod mbc2;
mod mbc3;
mod mbc3_rtc;
mod mbc5;
mod mbc6;
mod mbc7;
mod mmm01;
mod no_mbc;
mod persistence;
mod pocket_camera;

fn build_test_rom(len: usize, cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let mut rom = vec![0xFF; len.max(HEADER_MINIMUM_ROM_LEN)];
    rom[ENTRY_POINT_START..ENTRY_POINT_START + ENTRY_POINT_LEN]
        .copy_from_slice(&[0x00, 0xC3, 0x50, 0x01]);
    rom[NINTENDO_LOGO_START..NINTENDO_LOGO_START + NINTENDO_LOGO_LEN]
        .copy_from_slice(&[0xCE; NINTENDO_LOGO_LEN]);
    rom[TITLE_START..TITLE_START + 7].copy_from_slice(b"GBTEST1");
    rom[CGB_FLAG_ADDRESS] = 0x80;
    rom[SGB_FLAG_ADDRESS] = 0x03;
    rom[CARTRIDGE_TYPE_ADDRESS] = cartridge_type;
    rom[ROM_SIZE_ADDRESS] = rom_size_code;
    rom[RAM_SIZE_ADDRESS] = ram_size_code;
    rom
}

fn build_banked_mbc1_rom_with_type(
    cartridge_type: u8,
    rom_size_code: u8,
    ram_size_code: u8,
) -> Vec<u8> {
    let rom_size = RomSizeInfo::decode(rom_size_code)
        .decoded_bytes
        .expect("test ROM size should decode");
    let bank_count = RomSizeInfo::decode(rom_size_code)
        .bank_count
        .expect("test ROM bank count should decode");
    let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

fn build_banked_mbc1_rom(rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let cartridge_type = if ram_size_code == 0x00 { 0x01 } else { 0x03 };
    build_banked_mbc1_rom_with_type(cartridge_type, rom_size_code, ram_size_code)
}

fn build_mmm01_rom(rom_size_code: u8, ram_size_code: u8, cartridge_type: u8) -> Vec<u8> {
    let rom_size = RomSizeInfo::decode(rom_size_code)
        .decoded_bytes
        .expect("test ROM size should decode");
    let bank_count = RomSizeInfo::decode(rom_size_code)
        .bank_count
        .expect("test ROM bank count should decode");
    let mut rom = vec![0xFF; rom_size.max(HEADER_MINIMUM_ROM_LEN)];

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom[ENTRY_POINT_START..ENTRY_POINT_START + ENTRY_POINT_LEN]
        .copy_from_slice(&[0x00, 0xC3, 0x50, 0x01]);
    rom[NINTENDO_LOGO_START..NINTENDO_LOGO_START + NINTENDO_LOGO_LEN]
        .copy_from_slice(&[0xCE; NINTENDO_LOGO_LEN]);
    rom[TITLE_START..TITLE_START + 7].copy_from_slice(b"GAMEONE");
    rom[CARTRIDGE_TYPE_ADDRESS] = 0x00;
    rom[ROM_SIZE_ADDRESS] = 0x00;
    rom[RAM_SIZE_ADDRESS] = 0x00;

    let menu_offset = rom_size - MMM01_MENU_BYTES;
    let secondary_header_offset = (menu_offset / 2 / 0x4000) * 0x4000;
    rom[secondary_header_offset + ENTRY_POINT_START
        ..secondary_header_offset + ENTRY_POINT_START + ENTRY_POINT_LEN]
        .copy_from_slice(&[0x00, 0xC3, 0x50, 0x01]);
    rom[secondary_header_offset + NINTENDO_LOGO_START
        ..secondary_header_offset + NINTENDO_LOGO_START + NINTENDO_LOGO_LEN]
        .copy_from_slice(&[0xCE; NINTENDO_LOGO_LEN]);
    rom[secondary_header_offset + TITLE_START..=secondary_header_offset + TITLE_END_INCLUSIVE]
        .fill(0x00);
    rom[secondary_header_offset + TITLE_START..secondary_header_offset + TITLE_START + 7]
        .copy_from_slice(b"GAMETWO");
    rom[secondary_header_offset + CARTRIDGE_TYPE_ADDRESS] = 0x00;
    rom[secondary_header_offset + ROM_SIZE_ADDRESS] = 0x00;
    rom[secondary_header_offset + RAM_SIZE_ADDRESS] = 0x00;
    rom[menu_offset + ENTRY_POINT_START..menu_offset + ENTRY_POINT_START + ENTRY_POINT_LEN]
        .copy_from_slice(&[0x00, 0xC3, 0x50, 0x01]);
    rom[menu_offset + NINTENDO_LOGO_START..menu_offset + NINTENDO_LOGO_START + NINTENDO_LOGO_LEN]
        .copy_from_slice(&[0xCE; NINTENDO_LOGO_LEN]);
    rom[menu_offset + TITLE_START..menu_offset + TITLE_START + 7].copy_from_slice(b"MMM01!!");
    rom[menu_offset + CGB_FLAG_ADDRESS] = 0x80;
    rom[menu_offset + SGB_FLAG_ADDRESS] = 0x03;
    rom[menu_offset + CARTRIDGE_TYPE_ADDRESS] = cartridge_type;
    rom[menu_offset + ROM_SIZE_ADDRESS] = rom_size_code;
    rom[menu_offset + RAM_SIZE_ADDRESS] = ram_size_code;

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
        rom[base_offset + ENTRY_POINT_START..base_offset + ENTRY_POINT_START + ENTRY_POINT_LEN]
            .copy_from_slice(&[0x00, 0xC3, 0x50, 0x01]);
        rom[base_offset + NINTENDO_LOGO_START
            ..base_offset + NINTENDO_LOGO_START + NINTENDO_LOGO_LEN]
            .copy_from_slice(&[0xCE; NINTENDO_LOGO_LEN]);
        rom[base_offset + TITLE_START..=base_offset + TITLE_END_INCLUSIVE].fill(0x00);
        rom[base_offset + TITLE_START..base_offset + TITLE_START + title.len()]
            .copy_from_slice(title);
        rom[base_offset + CARTRIDGE_TYPE_ADDRESS] = 0x01;
        rom[base_offset + ROM_SIZE_ADDRESS] = game_rom_size_code;
        rom[base_offset + RAM_SIZE_ADDRESS] = 0x00;
    }

    let menu_offset = rom_size - MMM01_MENU_BYTES;
    rom[menu_offset + ENTRY_POINT_START..menu_offset + ENTRY_POINT_START + ENTRY_POINT_LEN]
        .copy_from_slice(&[0x00, 0xC3, 0x50, 0x01]);
    rom[menu_offset + NINTENDO_LOGO_START..menu_offset + NINTENDO_LOGO_START + NINTENDO_LOGO_LEN]
        .copy_from_slice(&[0xCE; NINTENDO_LOGO_LEN]);
    rom[menu_offset + TITLE_START..=menu_offset + TITLE_END_INCLUSIVE].fill(0x00);
    rom[menu_offset + TITLE_START..menu_offset + TITLE_START + menu_title.len()]
        .copy_from_slice(menu_title);
    rom[menu_offset + CARTRIDGE_TYPE_ADDRESS] = MANI_MMM01_MENU_TYPE;
    rom[menu_offset + ROM_SIZE_ADDRESS] = rom_size_code;
    rom[menu_offset + RAM_SIZE_ADDRESS] = 0x00;

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

fn build_m161_signature_rom() -> Vec<u8> {
    let mut rom = vec![0xFF; 5 * M161_BANK_BYTES];
    let titles = [
        M161_SYNTHETIC_MENU_TITLE,
        b"TETRIS".as_slice(),
        b"TENNIS".as_slice(),
        b"ALLEY WAY".as_slice(),
        b"YAKUMAN".as_slice(),
    ];

    for (bank, title) in titles.into_iter().enumerate() {
        let start = bank * M161_BANK_BYTES;
        let bank_rom = &mut rom[start..start + M161_BANK_BYTES];
        bank_rom.fill(bank as u8);
        bank_rom[ENTRY_POINT_START..ENTRY_POINT_START + ENTRY_POINT_LEN]
            .copy_from_slice(&[0x00, 0xC3, 0x50, 0x01]);
        bank_rom[NINTENDO_LOGO_START..NINTENDO_LOGO_START + NINTENDO_LOGO_LEN]
            .copy_from_slice(&[0xCE; NINTENDO_LOGO_LEN]);
        bank_rom[TITLE_START..=TITLE_END_INCLUSIVE].fill(0x00);
        bank_rom[TITLE_START..TITLE_START + title.len()].copy_from_slice(title);
        bank_rom[CARTRIDGE_TYPE_ADDRESS] = 0x00;
        bank_rom[ROM_SIZE_ADDRESS] = 0x00;
        bank_rom[RAM_SIZE_ADDRESS] = 0x00;
    }

    rom
}

fn build_m161_commercial_rom() -> Vec<u8> {
    let mut rom = vec![0x00; 8 * M161_BANK_BYTES];
    let titles = [
        M161_COMMERCIAL_MENU_TITLE,
        b"TETRIS".as_slice(),
        b"ALLEY WAY".as_slice(),
        b"YAKUMAN".as_slice(),
        b"TENNIS".as_slice(),
    ];

    for (bank, title) in titles.into_iter().enumerate() {
        let start = bank * M161_BANK_BYTES;
        let bank_rom = &mut rom[start..start + M161_BANK_BYTES];
        bank_rom.fill(bank as u8);
        bank_rom[ENTRY_POINT_START..ENTRY_POINT_START + ENTRY_POINT_LEN]
            .copy_from_slice(&[0x00, 0xC3, 0x50, 0x01]);
        bank_rom[NINTENDO_LOGO_START..NINTENDO_LOGO_START + NINTENDO_LOGO_LEN]
            .copy_from_slice(&[0xCE; NINTENDO_LOGO_LEN]);
        bank_rom[TITLE_START..=TITLE_END_INCLUSIVE].fill(0x00);
        bank_rom[TITLE_START..TITLE_START + title.len()].copy_from_slice(title);
        bank_rom[RAM_SIZE_ADDRESS] = 0x00;
        if bank != 0 {
            bank_rom[CARTRIDGE_TYPE_ADDRESS] = 0x00;
            bank_rom[ROM_SIZE_ADDRESS] = 0x00;
        }
    }

    rom[CARTRIDGE_TYPE_ADDRESS] = 0x10;
    rom[ROM_SIZE_ADDRESS] = 0x03;
    rom[RAM_SIZE_ADDRESS] = 0x00;

    rom
}

fn mark_mbc1_multicart_subheaders_in_banks(rom: &mut [u8], banks: &[usize]) {
    let logo = rom[NINTENDO_LOGO_START..NINTENDO_LOGO_START + NINTENDO_LOGO_LEN].to_vec();

    for &bank in banks {
        let start = bank * 0x4000 + NINTENDO_LOGO_START;
        rom[start..start + NINTENDO_LOGO_LEN].copy_from_slice(&logo);
    }
}

fn mark_mbc1_multicart_subheaders(rom: &mut [u8]) {
    mark_mbc1_multicart_subheaders_in_banks(rom, &[0x10, 0x20, 0x30]);
}

fn build_banked_mbc2_rom(cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let rom_size = RomSizeInfo::decode(rom_size_code)
        .decoded_bytes
        .expect("test ROM size should decode");
    let bank_count = RomSizeInfo::decode(rom_size_code)
        .bank_count
        .expect("test ROM bank count should decode");
    let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

fn build_banked_mbc3_rom(cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let rom_size = RomSizeInfo::decode(rom_size_code)
        .decoded_bytes
        .expect("test ROM size should decode");
    let bank_count = RomSizeInfo::decode(rom_size_code)
        .bank_count
        .expect("test ROM bank count should decode");
    let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

fn build_banked_mbc5_rom(cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let rom_size = RomSizeInfo::decode(rom_size_code)
        .decoded_bytes
        .expect("test ROM size should decode");
    let bank_count = RomSizeInfo::decode(rom_size_code)
        .bank_count
        .expect("test ROM bank count should decode");
    let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 1] = ((bank >> 8) & 0x01) as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

fn build_banked_mbc6_rom() -> Vec<u8> {
    let mut rom = build_test_rom(MBC6_SUPPORTED_ROM_BYTES, 0x20, 0x05, 0x03);
    for bank in 0..(MBC6_SUPPORTED_ROM_BYTES / MBC6_ROM_FLASH_BANK_BYTES) {
        let start = bank * MBC6_ROM_FLASH_BANK_BYTES;
        rom[start] = bank as u8;
        rom[start + 1] = (bank >> 8) as u8;
        if start + 0x0100 < rom.len() {
            rom[start + 0x0100] = bank as u8;
        }
    }
    rom
}

fn build_banked_mbc7_rom(rom_size_code: u8) -> Vec<u8> {
    let rom_size = RomSizeInfo::decode(rom_size_code)
        .decoded_bytes
        .expect("test ROM size should decode");
    let bank_count = RomSizeInfo::decode(rom_size_code)
        .bank_count
        .expect("test ROM bank count should decode");
    let mut rom = build_test_rom(rom_size, 0x22, rom_size_code, 0x00);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

fn build_pocket_camera_rom() -> Vec<u8> {
    let mut rom = build_test_rom(POCKET_CAMERA_SUPPORTED_ROM_BYTES, 0xFC, 0x05, 0x04);
    for bank in 0..(POCKET_CAMERA_SUPPORTED_ROM_BYTES / 0x4000) {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 1] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }
    rom
}

#[test]
fn runtime_save_state_validation_rejects_mapper_mismatch() {
    let mbc1 = CartridgeSlot::load(
        build_banked_mbc1_rom(0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC1 should load")
    .cartridge;
    let mbc5 = CartridgeSlot::load(
        build_banked_mbc5_rom(0x1B, 0x04, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC5 should load")
    .cartridge;
    let mbc1_state = mbc1.capture_save_state();

    assert!(matches!(
        mbc5.validate_save_state(&mbc1_state),
        Err(CartridgeRuntimeSaveStateError::SlotStateMismatch {
            expected: CartridgeSlotState::Mbc5,
            actual: CartridgeSlotState::Mbc1,
        })
    ));
}

#[test]
fn runtime_save_state_validation_rejects_ram_shape_mismatch() {
    let cartridge = CartridgeSlot::load(
        build_banked_mbc5_rom(0x1B, 0x04, 0x04),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC5 should load")
    .cartridge;
    let mut state = cartridge.capture_save_state();

    let Some(CartridgeDeviceSaveState::Mbc5(saved)) = &mut state.device else {
        panic!("expected MBC5 save state");
    };
    saved.ram = Some(vec![0; 8 * 1024]);

    assert!(matches!(
        cartridge.validate_save_state(&state),
        Err(CartridgeRuntimeSaveStateError::RamShapeMismatch {
            field: "MBC5 RAM",
            expected,
            actual,
        }) if expected == Some(128 * 1024) && actual == Some(8 * 1024)
    ));
}

fn warn_policy() -> CompatibilityPolicy {
    CompatibilityPolicy {
        execution_mode: ExecutionMode::Permissive,
        validation_policy: ValidationPolicy::Warn,
        heuristic_policy: HeuristicPolicy::Disabled,
        override_policy: OverridePolicy::default(),
        diagnostic_policy: DiagnosticPolicy::Standard,
    }
}

fn ignore_policy() -> CompatibilityPolicy {
    CompatibilityPolicy {
        execution_mode: ExecutionMode::Permissive,
        validation_policy: ValidationPolicy::Ignore,
        heuristic_policy: HeuristicPolicy::Disabled,
        override_policy: OverridePolicy::default(),
        diagnostic_policy: DiagnosticPolicy::Standard,
    }
}
