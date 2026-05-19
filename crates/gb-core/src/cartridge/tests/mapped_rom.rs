use super::*;

fn load_strict(rom: Vec<u8>) -> CartridgeSlot {
    CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect("cartridge should load")
        .cartridge
}

fn rom_window(bank: usize, bank_size: usize, bank_offset: usize) -> CartridgeMappedRomWindow {
    CartridgeMappedRomWindow {
        source: CartridgeMappedRomSource::Rom,
        bank,
        bank_size,
        bank_offset,
    }
}

fn flash_window(bank: usize, bank_size: usize, bank_offset: usize) -> CartridgeMappedRomWindow {
    CartridgeMappedRomWindow {
        source: CartridgeMappedRomSource::Flash,
        bank,
        bank_size,
        bank_offset,
    }
}

#[test]
fn mapped_rom_window_returns_none_for_empty_slots_and_non_rom_addresses() {
    let empty = CartridgeSlot::empty();
    assert_eq!(empty.mapped_rom_window(0x0000), None);

    let cartridge = load_strict(build_test_rom(32 * 1024, 0x00, 0x00, 0x00));
    assert_eq!(cartridge.mapped_rom_window(0x8000), None);
    assert_eq!(cartridge.mapped_rom_window(0xFFFF), None);
}

#[test]
fn mapped_rom_window_reports_fixed_and_switchable_rom_windows_for_standard_mappers() {
    let no_mbc = load_strict(build_test_rom(32 * 1024, 0x00, 0x00, 0x00));
    assert_eq!(
        no_mbc.mapped_rom_window(0x0000),
        Some(rom_window(0, 0x4000, 0))
    );
    assert_eq!(
        no_mbc.mapped_rom_window(0x7FFF),
        Some(rom_window(1, 0x4000, 0x3FFF))
    );

    let mut mbc1 = load_strict(build_banked_mbc1_rom(0x03, 0x00));
    assert_eq!(
        mbc1.mapped_rom_window(0x0123),
        Some(rom_window(0, 0x4000, 0x0123))
    );
    mbc1.write_rom(0x2000, 0x02);
    assert_eq!(
        mbc1.mapped_rom_window(0x4567),
        Some(rom_window(2, 0x4000, 0x0567))
    );

    let mut mbc2 = load_strict(build_banked_mbc2_rom(0x06, 0x03, 0x00));
    assert_eq!(
        mbc2.mapped_rom_window(0x2345),
        Some(rom_window(0, 0x4000, 0x2345))
    );
    mbc2.write_rom(0x2100, 0x03);
    assert_eq!(
        mbc2.mapped_rom_window(0x4567),
        Some(rom_window(3, 0x4000, 0x0567))
    );

    let mut mbc3 = load_strict(build_banked_mbc3_rom(0x13, 0x03, 0x03));
    assert_eq!(
        mbc3.mapped_rom_window(0x0100),
        Some(rom_window(0, 0x4000, 0x0100))
    );
    mbc3.write_rom(0x2000, 0x05);
    assert_eq!(
        mbc3.mapped_rom_window(0x4001),
        Some(rom_window(5, 0x4000, 0x0001))
    );

    let mut mbc5 = load_strict(build_banked_mbc5_rom(0x1B, 0x04, 0x03));
    assert_eq!(
        mbc5.mapped_rom_window(0x3FFF),
        Some(rom_window(0, 0x4000, 0x3FFF))
    );
    mbc5.write_rom(0x2000, 0x06);
    mbc5.write_rom(0x3000, 0x00);
    assert_eq!(
        mbc5.mapped_rom_window(0x6002),
        Some(rom_window(6, 0x4000, 0x2002))
    );
}

#[test]
fn mapped_rom_window_reports_dedicated_mapper_bank_windows() {
    let mut huc1 = load_strict(build_banked_huc1_rom(0x04, 0x03));
    assert_eq!(
        huc1.mapped_rom_window(0x0123),
        Some(rom_window(0, 0x4000, 0x0123))
    );
    huc1.write_rom(0x2000, 0x07);
    assert_eq!(
        huc1.mapped_rom_window(0x4567),
        Some(rom_window(7, 0x4000, 0x0567))
    );

    let mut huc3 = load_strict(build_banked_huc3_rom(0x04, 0x03));
    assert_eq!(
        huc3.mapped_rom_window(0x0123),
        Some(rom_window(0, 0x4000, 0x0123))
    );
    huc3.write_rom(0x2000, 0x07);
    assert_eq!(
        huc3.mapped_rom_window(0x4567),
        Some(rom_window(7, 0x4000, 0x0567))
    );

    let mut m161 = load_strict(build_m161_signature_rom());
    assert_eq!(
        m161.mapped_rom_window(0x0123),
        Some(rom_window(0, M161_BANK_BYTES, 0x0123))
    );
    m161.write_rom(0x6000, 0x03);
    m161.write_rom(0x0000, 0x01);
    assert_eq!(
        m161.mapped_rom_window(0x7FFE),
        Some(rom_window(3, M161_BANK_BYTES, 0x7FFE))
    );

    let mut mmm01 = load_strict(build_mmm01_rom(0x04, 0x03, 0x0D));
    assert_eq!(
        mmm01.mapped_rom_window(0x0123),
        Some(rom_window(30, 0x4000, 0x0123))
    );
    assert_eq!(
        mmm01.mapped_rom_window(0x4567),
        Some(rom_window(31, 0x4000, 0x0567))
    );
    mmm01.write_rom(0x0000, 0x40);
    assert_eq!(
        mmm01.mapped_rom_window(0x0123),
        Some(rom_window(0, 0x4000, 0x0123))
    );
    assert_eq!(
        mmm01.mapped_rom_window(0x4567),
        Some(rom_window(1, 0x4000, 0x0567))
    );
}

#[test]
fn mapped_rom_window_reports_special_mbc6_mbc7_and_camera_windows() {
    let mut mbc6 = load_strict(build_banked_mbc6_rom());
    assert_eq!(
        mbc6.mapped_rom_window(0x0123),
        Some(rom_window(0, 0x4000, 0x0123))
    );
    assert_eq!(
        mbc6.mapped_rom_window(0x4567),
        Some(rom_window(2, 0x2000, 0x0567))
    );
    assert_eq!(
        mbc6.mapped_rom_window(0x6ABC),
        Some(rom_window(3, 0x2000, 0x0ABC))
    );
    mbc6.write_rom(0x0C00, 0x01);
    mbc6.write_rom(0x2000, 0x04);
    mbc6.write_rom(0x2800, 0x08);
    mbc6.write_rom(0x3000, 0x05);
    mbc6.write_rom(0x3800, 0x08);
    assert_eq!(
        mbc6.mapped_rom_window(0x4567),
        Some(flash_window(4, 0x2000, 0x0567))
    );
    assert_eq!(
        mbc6.mapped_rom_window(0x6ABC),
        Some(flash_window(5, 0x2000, 0x0ABC))
    );

    let mut mbc7 = load_strict(build_banked_mbc7_rom(0x03));
    assert_eq!(
        mbc7.mapped_rom_window(0x0123),
        Some(rom_window(0, 0x4000, 0x0123))
    );
    mbc7.write_rom(0x2000, 0x06);
    assert_eq!(
        mbc7.mapped_rom_window(0x4567),
        Some(rom_window(6, 0x4000, 0x0567))
    );

    let mut camera = load_strict(build_pocket_camera_rom());
    assert_eq!(
        camera.mapped_rom_window(0x0123),
        Some(rom_window(0, 0x4000, 0x0123))
    );
    camera.write_rom(0x2000, 0x09);
    assert_eq!(
        camera.mapped_rom_window(0x4567),
        Some(rom_window(9, 0x4000, 0x0567))
    );
}
