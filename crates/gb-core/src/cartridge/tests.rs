use super::*;
use crate::model::{CompatibilityPolicy, DiagnosticPolicy, HeuristicPolicy, OverridePolicy};

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
    build_banked_mbc1_rom_with_type(0x03, rom_size_code, ram_size_code)
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

fn mark_mbc1_multicart_subheaders(rom: &mut [u8]) {
    let logo = rom[NINTENDO_LOGO_START..NINTENDO_LOGO_START + NINTENDO_LOGO_LEN].to_vec();

    for bank in [0x10usize, 0x20, 0x30] {
        let start = bank * 0x4000 + NINTENDO_LOGO_START;
        rom[start..start + NINTENDO_LOGO_LEN].copy_from_slice(&logo);
    }
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

fn warn_policy() -> CompatibilityPolicy {
    CompatibilityPolicy {
        execution_mode: ExecutionMode::Permissive,
        validation_policy: ValidationPolicy::Warn,
        heuristic_policy: HeuristicPolicy::Disabled,
        override_policy: OverridePolicy::default(),
        diagnostic_policy: DiagnosticPolicy::Standard,
    }
}

#[test]
fn header_parser_decodes_typed_core_fields() {
    let rom = build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x09, 0x00, 0x02);
    let header = CartridgeHeader::parse(&rom).expect("header should parse");

    assert_eq!(header.entry_point, [0x00, 0xC3, 0x50, 0x01]);
    assert_eq!(header.title, "GBTEST1");
    assert_eq!(header.cgb_flag, CgbFlag::Supported);
    assert_eq!(header.sgb_flag, SgbFlag::Supported);
    assert_eq!(header.cartridge_type, 0x09);
    assert_eq!(
        header.rom_size.decoded_bytes,
        Some(NO_MBC_SUPPORTED_ROM_BYTES)
    );
    assert_eq!(
        header.ram_size.decoded_bytes,
        Some(NO_MBC_SUPPORTED_RAM_BYTES)
    );
}

#[test]
fn classification_keeps_supported_families_and_structured_unsupported_categories_explicit() {
    let no_mbc = CartridgeClassification::classify(0x09);
    let mbc1 = CartridgeClassification::classify(0x03);
    let camera = CartridgeClassification::classify(0xFC);
    let unknown = CartridgeClassification::classify(0xAA);

    assert_eq!(
        no_mbc.selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::NoMbc)
    );
    assert_eq!(
        mbc1.selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc1)
    );
    assert_eq!(
        camera.selection(),
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::AccessorySpecialCase)
    );
    assert_eq!(
        unknown.selection(),
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::UnknownCode)
    );
    assert_eq!(camera.detected_name(), "POCKET CAMERA");
    assert_eq!(unknown.raw_type(), 0xAA);
}

#[test]
fn contextual_classification_promotes_mbc30_and_opt_in_heuristics_over_the_raw_header() {
    let mbc30_rom = build_test_rom(256 * 1024, 0x13, 0x03, 0x05);
    let mbc30_header = CartridgeHeader::parse(&mbc30_rom).expect("header should parse");
    let mbc30_classification =
        classify_loaded_cartridge(&mbc30_header, &mbc30_rom, &CompatibilityPolicy::strict());

    assert_eq!(mbc30_classification.detected_name(), "MBC30");
    assert_eq!(
        mbc30_classification.selection(),
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::PlannedVariant)
    );

    let mut ems_rom = build_test_rom(256 * 1024, 0x1B, 0x03, 0x03);
    ems_rom[TITLE_START..TITLE_START + 7].copy_from_slice(b"EMSMENU");
    ems_rom[TITLE_START + 7..=TITLE_END_INCLUSIVE].fill(0x00);
    ems_rom[DESTINATION_CODE_ADDRESS] = 0xE1;
    let ems_header = CartridgeHeader::parse(&ems_rom).expect("header should parse");

    let strict_classification =
        classify_loaded_cartridge(&ems_header, &ems_rom, &CompatibilityPolicy::strict());
    assert_eq!(
        strict_classification.selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc5)
    );

    let experimental_classification =
        classify_loaded_cartridge(&ems_header, &ems_rom, &CompatibilityPolicy::experimental());
    assert_eq!(experimental_classification.detected_name(), "EMS");
    assert_eq!(
        experimental_classification.selection(),
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::ExperimentalHeuristic)
    );

    let mut mbc1m_rom = build_banked_mbc1_rom_with_type(0x01, 0x05, 0x00);
    mark_mbc1_multicart_subheaders(&mut mbc1m_rom);
    let mbc1m_header = CartridgeHeader::parse(&mbc1m_rom).expect("header should parse");

    let strict_mbc1m =
        classify_loaded_cartridge(&mbc1m_header, &mbc1m_rom, &CompatibilityPolicy::strict());
    assert_eq!(strict_mbc1m.detected_name(), "MBC1");
    assert_eq!(
        strict_mbc1m.selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc1)
    );

    let experimental_mbc1m = classify_loaded_cartridge(
        &mbc1m_header,
        &mbc1m_rom,
        &CompatibilityPolicy::experimental(),
    );
    assert_eq!(experimental_mbc1m.detected_name(), "MBC1M");
    assert_eq!(
        experimental_mbc1m.selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc1)
    );

    let m161_rom = build_m161_signature_rom();
    let m161_header = CartridgeHeader::parse(&m161_rom).expect("header should parse");
    let m161_classification =
        classify_loaded_cartridge(&m161_header, &m161_rom, &CompatibilityPolicy::strict());
    assert_eq!(m161_classification.detected_name(), "M161");
    assert_eq!(
        m161_classification.selection(),
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::DocumentedButUnsupported)
    );
}

#[test]
fn no_mbc_loader_builds_rom_only_and_ram_variants() {
    let rom_only = build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x00, 0x00, 0x00);
    let with_ram = build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x09, 0x00, 0x02);

    let rom_only_report = CartridgeSlot::load(rom_only, &CompatibilityPolicy::strict())
        .expect("ROM-only NoMBC should load");
    let with_ram_report = CartridgeSlot::load(with_ram, &CompatibilityPolicy::strict())
        .expect("RAM NoMBC should load");

    assert_eq!(
        rom_only_report.cartridge().state(),
        CartridgeSlotState::NoMbc
    );
    assert_eq!(
        with_ram_report.cartridge().state(),
        CartridgeSlotState::NoMbc
    );
    assert_eq!(
        rom_only_report.cartridge().read_ram(0xA000),
        RAM_ABSENT_READ_VALUE
    );
    assert!(with_ram_report.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rare but still treated as a valid No MBC variant")
    }));
}

#[test]
fn strict_validation_rejects_invalid_no_mbc_ram_configuration() {
    let rom = build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x08, 0x00, 0x03);
    let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect_err("invalid RAM config must fail");

    match error {
        CartridgeLoadError::Rejected {
            classification,
            execution_mode,
            reason,
            ..
        } => {
            assert_eq!(classification.detected_name(), "ROM+RAM");
            assert_eq!(execution_mode, ExecutionMode::Strict);
            assert!(reason.contains("expects RAM size code"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn warn_validation_can_admit_unambiguous_no_mbc_size_mismatches_with_diagnostics() {
    let rom = build_test_rom(64 * 1024, 0x00, 0x01, 0x00);
    let report =
        CartridgeSlot::load(rom, &warn_policy()).expect("warn policy should admit the mismatch");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::NoMbc);
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("expects ROM size code 0x00"))
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("expects a 32 KiB image"))
    );
}

#[test]
fn warn_validation_can_admit_unambiguous_no_mbc_ram_header_mismatches_with_diagnostics() {
    let rom = build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x08, 0x00, 0x01);
    let report = CartridgeSlot::load(rom, &warn_policy())
        .expect("warn policy should admit the legacy RAM-size mismatch");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::NoMbc);
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("expects RAM size code 0x02"))
    );
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unsupported RAM configuration"))
    );
}

#[test]
fn loading_supported_mbc1_family_constructs_the_mapper_device() {
    let rom = build_banked_mbc1_rom(0x02, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc1);
    assert_eq!(report.cartridge().read_rom(0x0000), 0x00);
    assert_eq!(report.cartridge().read_rom(0x4000), 0x01);
}

#[test]
fn loading_32kib_mbc1_images_keeps_the_switchable_window_on_bank_one() {
    let rom = build_banked_mbc1_rom(0x00, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc1);
    assert_eq!(report.cartridge().read_rom(0x0000), 0x00);
    assert_eq!(report.cartridge().read_rom(0x4000), 0x01);
}

#[test]
fn mbc1_power_up_state_is_explicit_and_starts_the_high_window_on_bank_one() {
    let rom = build_banked_mbc1_rom(0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");

    let Some(CartridgeDevice::Mbc1(cartridge)) = report.cartridge().device.as_ref() else {
        panic!("expected MBC1 cartridge");
    };

    assert_eq!(cartridge.wiring, Mbc1Wiring::Standard);
    assert_eq!(cartridge.variant, Mbc1Variant::Standard);
    assert!(!cartridge.ram_enabled);
    assert_eq!(cartridge.rom_bank_low5, 0);
    assert_eq!(cartridge.secondary_bank, 0);
    assert_eq!(cartridge.banking_mode, 0);
    assert_eq!(cartridge.read_rom(0x4000), 0x01);
}

#[test]
fn mbc1_raw_low_bank_zero_translates_to_bank_one_before_size_masking() {
    let rom = build_banked_mbc1_rom(0x04, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC1 cartridge");
    };

    cartridge.write_rom(0x2000, 0x00);
    assert_eq!(cartridge.rom_bank_low5, 0);
    assert_eq!(cartridge.read_rom(0x4000), 0x01);

    cartridge.write_rom(0x2000, 0x1F);
    assert_eq!(cartridge.read_rom(0x4000), 0x1F);
}

#[test]
fn mbc1_small_rom_masking_can_make_bank_zero_visible_in_the_high_window() {
    let rom = build_banked_mbc1_rom(0x01, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC1 cartridge");
    };

    cartridge.write_rom(0x2000, 0x04);

    assert_eq!(cartridge.read_rom(0x4000), 0x00);
}

#[test]
fn mbc1_control_writes_update_raw_registers_and_gate_ram_access_immediately() {
    let rom = build_banked_mbc1_rom(0x02, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC1 cartridge");
    };

    assert_eq!(cartridge.read_ram(0xA000), RAM_ABSENT_READ_VALUE);
    cartridge.write_ram(0xA000, 0x5A);
    assert_eq!(cartridge.read_ram(0xA000), RAM_ABSENT_READ_VALUE);

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x4000, 0x02);
    cartridge.write_rom(0x6000, 0x01);
    cartridge.write_ram(0xA000, 0x5A);

    assert!(cartridge.ram_enabled);
    assert_eq!(cartridge.secondary_bank, 0x02);
    assert_eq!(cartridge.banking_mode, 0x01);
    assert_eq!(cartridge.read_ram(0xA000), 0x5A);
}

#[test]
fn mbc1_standard_8kib_ram_ignores_mode_one_ram_bank_selection() {
    let rom = build_banked_mbc1_rom(0x01, 0x02);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC1 cartridge");
    };

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0x11);
    cartridge.write_ram(0xB000, 0x22);

    cartridge.write_rom(0x6000, 0x01);
    for bank in 0..=3 {
        cartridge.write_rom(0x4000, bank);
        assert_eq!(cartridge.read_ram(0xA000), 0x11);
        assert_eq!(cartridge.read_ram(0xB000), 0x22);
    }

    cartridge.write_rom(0x4000, 0x03);
    cartridge.write_ram(0xA000, 0x33);
    cartridge.write_ram(0xB000, 0x44);

    cartridge.write_rom(0x4000, 0x01);
    assert_eq!(cartridge.read_ram(0xA000), 0x33);
    assert_eq!(cartridge.read_ram(0xB000), 0x44);
}

#[test]
fn mbc1_large_rom_high_window_reaches_documented_odd_bank_entries_only() {
    let rom = build_banked_mbc1_rom(0x06, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC1 cartridge");
    };

    assert_eq!(cartridge.wiring, Mbc1Wiring::LargeRom);

    cartridge.write_rom(0x2000, 0x00);
    cartridge.write_rom(0x4000, 0x01);
    assert_eq!(cartridge.read_rom(0x4000), 0x21);

    cartridge.write_rom(0x4000, 0x02);
    assert_eq!(cartridge.read_rom(0x4000), 0x41);

    cartridge.write_rom(0x4000, 0x03);
    assert_eq!(cartridge.read_rom(0x4000), 0x61);
}

#[test]
fn mbc1_large_rom_mode_one_remaps_the_low_window_from_secondary_bits() {
    let rom = build_banked_mbc1_rom(0x06, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC1 cartridge");
    };

    cartridge.write_rom(0x2000, 0x01);
    cartridge.write_rom(0x4000, 0x02);
    assert_eq!(cartridge.read_rom(0x0000), 0x00);
    assert_eq!(cartridge.read_rom(0x4000), 0x41);

    cartridge.write_rom(0x6000, 0x01);
    assert_eq!(cartridge.read_rom(0x0000), 0x40);
    assert_eq!(cartridge.read_rom(0x4000), 0x41);
}

#[test]
fn mbc1_large_rom_keeps_one_fixed_8kib_ram_window_across_modes() {
    let rom = build_banked_mbc1_rom(0x05, 0x02);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC1 cartridge");
    };

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0x11);

    cartridge.write_rom(0x4000, 0x01);
    cartridge.write_rom(0x6000, 0x01);

    assert_eq!(cartridge.read_ram(0xA000), 0x11);
    cartridge.write_ram(0xA000, 0x22);

    cartridge.write_rom(0x4000, 0x00);
    cartridge.write_rom(0x6000, 0x00);
    assert_eq!(cartridge.read_ram(0xA000), 0x22);
}

#[test]
fn experimental_mbc1m_multicart_banking_uses_the_documented_game_select_layout() {
    let mut rom = build_banked_mbc1_rom_with_type(0x01, 0x05, 0x00);
    mark_mbc1_multicart_subheaders(&mut rom);
    let report = CartridgeSlot::load(rom, &CompatibilityPolicy::experimental())
        .expect("experimental MBC1M should load");
    let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC1 cartridge");
    };

    assert_eq!(cartridge.variant, Mbc1Variant::Mbc1M);
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("explicit experimental multicart heuristic")
    }));

    cartridge.write_rom(0x2000, 0x00);
    cartridge.write_rom(0x4000, 0x00);
    assert_eq!(cartridge.read_rom(0x4000), 0x01);

    cartridge.write_rom(0x2000, 0x10);
    assert_eq!(cartridge.read_rom(0x4000), 0x00);

    cartridge.write_rom(0x2000, 0x00);
    cartridge.write_rom(0x4000, 0x01);
    assert_eq!(cartridge.read_rom(0x4000), 0x11);

    cartridge.write_rom(0x2000, 0x10);
    assert_eq!(cartridge.read_rom(0x4000), 0x10);

    cartridge.write_rom(0x6000, 0x01);
    cartridge.write_rom(0x4000, 0x02);
    assert_eq!(cartridge.read_rom(0x0000), 0x20);
    assert_eq!(cartridge.read_rom(0x4000), 0x20);

    cartridge.write_rom(0x2000, 0x01);
    assert_eq!(cartridge.read_rom(0x4000), 0x21);
}

#[test]
fn strict_validation_rejects_large_rom_mbc1_with_32kib_ram_declaration() {
    let rom = build_banked_mbc1_rom(0x05, 0x03);
    let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect_err("invalid large-ROM MBC1 RAM config must fail");

    match error {
        CartridgeLoadError::Rejected { reason, .. } => {
            assert!(reason.contains("not valid for the current LargeRom MBC1 wiring baseline"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn strict_validation_admits_32kib_mbc1_images_as_small_standard_wiring() {
    let rom = build_banked_mbc1_rom(0x00, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let Some(CartridgeDevice::Mbc1(cartridge)) = report.cartridge().device.as_ref() else {
        panic!("expected MBC1 cartridge");
    };

    assert_eq!(cartridge.wiring, Mbc1Wiring::Standard);
    assert_eq!(cartridge.read_rom(0x4000), 0x01);
}

#[test]
fn mbc2_power_up_state_is_explicit_and_starts_the_high_window_on_bank_one() {
    let rom = build_banked_mbc2_rom(0x06, 0x03, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC2 should load");

    let Some(CartridgeDevice::Mbc2(cartridge)) = report.cartridge().device.as_ref() else {
        panic!("expected MBC2 cartridge");
    };

    assert!(!cartridge.ram_enabled);
    assert_eq!(cartridge.rom_bank_low4, 0);
    assert_eq!(cartridge.read_rom(0x4000), 0x01);
    assert!(cartridge.has_battery);
}

#[test]
fn mbc2_address_bit_8_decode_controls_enable_and_bank_registers_separately() {
    let rom = build_banked_mbc2_rom(0x05, 0x03, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC2 should load");
    let Some(CartridgeDevice::Mbc2(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC2 cartridge");
    };

    cartridge.write_rom(0x0000, 0x0A);
    assert!(cartridge.ram_enabled);
    assert_eq!(cartridge.rom_bank_low4, 0);

    cartridge.write_rom(0x0100, 0x03);
    assert!(cartridge.ram_enabled);
    assert_eq!(cartridge.rom_bank_low4, 0x03);
    assert_eq!(cartridge.read_rom(0x4000), 0x03);
}

#[test]
fn mbc2_internal_ram_masks_to_low_nibbles_and_aliases_on_low_9_bits() {
    let rom = build_banked_mbc2_rom(0x06, 0x03, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC2 should load");
    let Some(CartridgeDevice::Mbc2(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC2 cartridge");
    };

    assert_eq!(cartridge.read_ram(0xA000), RAM_ABSENT_READ_VALUE);
    cartridge.write_ram(0xA000, 0xAB);
    assert_eq!(cartridge.read_ram(0xA000), RAM_ABSENT_READ_VALUE);

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0xAB);

    assert_eq!(cartridge.read_ram(0xA000), 0xFB);
    assert_eq!(cartridge.read_ram(0xA200), 0xFB);
    assert_eq!(cartridge.read_ram(0xBFFF), 0xF0);
}

#[test]
fn mbc2_ignores_rom_space_writes_outside_the_control_window() {
    let rom = build_banked_mbc2_rom(0x06, 0x03, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC2 should load");
    let Some(CartridgeDevice::Mbc2(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC2 cartridge");
    };

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA000, 0x0B);
    assert!(cartridge.ram_enabled);
    assert_eq!(cartridge.read_ram(0xA000), 0xFB);

    cartridge.write_rom(0x0100, 0x03);
    assert_eq!(cartridge.read_rom(0x4000), 0x03);

    cartridge.write_rom(0x4000, 0x00);
    cartridge.write_rom(0x4100, 0x01);

    assert!(cartridge.ram_enabled);
    assert_eq!(cartridge.read_ram(0xA000), 0xFB);
    assert_eq!(cartridge.read_rom(0x4000), 0x03);
}

#[test]
fn strict_validation_rejects_oversized_mbc2_roms() {
    let rom = build_banked_mbc2_rom(0x05, 0x04, 0x00);
    let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect_err("oversized MBC2 should fail validation");

    match error {
        CartridgeLoadError::Rejected { reason, .. } => {
            assert!(reason.contains("exceeds the current MBC2 ROM limit"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn permissive_validation_can_warn_on_nonzero_mbc2_ram_size_metadata() {
    let rom = build_banked_mbc2_rom(0x06, 0x03, 0x02);
    let report = CartridgeSlot::load(rom, &warn_policy())
        .expect("warn policy should admit nonzero MBC2 RAM metadata");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc2);
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("expects RAM size code 0x00 because MBC2 RAM is internal")
    }));
}

#[test]
fn mbc3_power_up_state_is_explicit_and_starts_the_high_window_on_bank_one() {
    let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");

    let Some(CartridgeDevice::Mbc3(cartridge)) = report.cartridge().device.as_ref() else {
        panic!("expected MBC3 cartridge");
    };

    assert_eq!(cartridge.variant, Mbc3Variant::Standard);
    assert!(cartridge.has_rtc);
    assert!(!cartridge.ram_rtc_enabled);
    assert_eq!(cartridge.rom_bank, 0);
    assert_eq!(cartridge.ram_or_rtc_select, Mbc3RamRtcSelect::RamBank(0));
    assert!(!cartridge.rtc_latched_valid);
    assert!(cartridge.rtc_latch_armed);
    assert_eq!(cartridge.read_rom(0x4000), 0x01);
}

#[test]
fn mbc3_reaches_banks_0x20_0x40_and_0x60_without_mbc1_style_anomalies() {
    let rom = build_banked_mbc3_rom(0x13, 0x06, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC3 cartridge");
    };

    for bank in [0x20, 0x40, 0x60] {
        cartridge.write_rom(0x2000, bank);
        assert_eq!(cartridge.read_rom(0x4000), bank);
    }
}

#[test]
fn mbc3_selector_keeps_ram_reserved_and_rtc_targets_distinct() {
    let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC3 cartridge");
    };

    cartridge.write_rom(0x4000, 0x02);
    assert_eq!(cartridge.ram_or_rtc_select, Mbc3RamRtcSelect::RamBank(0x02));

    cartridge.write_rom(0x4000, 0x05);
    assert_eq!(
        cartridge.ram_or_rtc_select,
        Mbc3RamRtcSelect::ReservedSelector(0x05)
    );

    cartridge.write_rom(0x4000, 0x0C);
    assert_eq!(
        cartridge.ram_or_rtc_select,
        Mbc3RamRtcSelect::RtcRegister(Mbc3RtcRegister::DayHigh)
    );
}

#[test]
fn mbc3_selector_ignores_upper_data_bits_and_decodes_from_the_low_nibble() {
    let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC3 cartridge");
    };

    cartridge.write_rom(0x4000, 0x12);
    assert_eq!(cartridge.ram_or_rtc_select, Mbc3RamRtcSelect::RamBank(0x02));

    cartridge.write_rom(0x4000, 0x1C);
    assert_eq!(
        cartridge.ram_or_rtc_select,
        Mbc3RamRtcSelect::RtcRegister(Mbc3RtcRegister::DayHigh)
    );

    cartridge.write_rom(0x4000, 0x17);
    assert_eq!(
        cartridge.ram_or_rtc_select,
        Mbc3RamRtcSelect::ReservedSelector(0x07)
    );
}

#[test]
fn strict_validation_admits_mbc3_headers_with_2kib_ram_metadata() {
    let rom = build_banked_mbc3_rom(0x13, 0x00, 0x01);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc3);
}

#[test]
fn mbc3_rtc_latch_reads_from_snapshot_while_writes_hit_live_state() {
    let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC3 cartridge");
    };

    cartridge.advance_rtc_seconds(93_784);
    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x4000, 0x08);

    assert_eq!(cartridge.read_ram(0xA000), 0x00);

    cartridge.write_rom(0x6000, 0x01);
    assert_eq!(cartridge.read_ram(0xA000), 0x04);

    cartridge.advance_rtc_seconds(1);
    assert_eq!(cartridge.read_ram(0xA000), 0x04);

    cartridge.write_rom(0x6000, 0x00);
    cartridge.write_rom(0x6000, 0x01);
    assert_eq!(cartridge.read_ram(0xA000), 0x05);

    cartridge.write_ram(0xA000, 0x2A);
    assert_eq!(cartridge.rtc_live.seconds, 0x2A);
    assert_eq!(cartridge.read_ram(0xA000), 0x05);

    cartridge.write_rom(0x6000, 0x55);
    assert_eq!(cartridge.read_ram(0xA000), 0x2A);
}

#[test]
fn mbc3_rtc_register_writes_echo_raw_bytes_until_time_advances() {
    let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC3 cartridge");
    };

    cartridge.write_rom(0x0000, 0x0A);

    cartridge.write_rom(0x4000, 0x08);
    cartridge.write_ram(0xA000, 0x74);

    cartridge.write_rom(0x4000, 0x09);
    cartridge.write_ram(0xA000, 0xF2);

    cartridge.write_rom(0x4000, 0x0A);
    cartridge.write_ram(0xA000, 0x62);
    assert_eq!(cartridge.read_ram(0xA000), 0x00);

    cartridge.write_rom(0x6000, 0x01);

    cartridge.write_rom(0x4000, 0x08);
    assert_eq!(cartridge.read_ram(0xA000), 0x34);

    cartridge.advance_rtc_seconds(1);

    cartridge.write_rom(0x4000, 0x08);
    assert_eq!(cartridge.read_ram(0xA000), 0x34);

    cartridge.write_rom(0x4000, 0x09);
    assert_eq!(cartridge.read_ram(0xA000), 0x32);

    cartridge.write_rom(0x4000, 0x0A);
    assert_eq!(cartridge.read_ram(0xA000), 0x02);
}

#[test]
fn mbc3_latch_stays_armed_after_a_successful_latch_until_a_zero_write_resets_it() {
    let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC3 cartridge");
    };

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x4000, 0x08);

    cartridge.write_ram(0xA000, 0x11);
    cartridge.write_rom(0x6000, 0x00);
    cartridge.write_rom(0x6000, 0x01);
    assert_eq!(cartridge.read_ram(0xA000), 0x11);

    cartridge.write_ram(0xA000, 0x37);
    assert_eq!(cartridge.read_ram(0xA000), 0x11);

    cartridge.write_rom(0x6000, 0x44);
    assert_eq!(cartridge.read_ram(0xA000), 0x37);
}

#[test]
fn mbc3_halt_and_carry_behavior_follow_the_live_rtc_rules() {
    let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC3 cartridge");
    };

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x4000, 0x0C);
    cartridge.write_ram(0xA000, 0x40);
    cartridge.advance_rtc_seconds(86_400);
    assert_eq!(cartridge.rtc_live.day_counter, 0);

    cartridge.write_ram(0xA000, 0x00);
    cartridge.advance_rtc_seconds(86_400);
    assert_eq!(cartridge.rtc_live.day_counter, 1);

    cartridge.rtc_live.day_counter = 511;
    cartridge.rtc_live.hours = 23;
    cartridge.rtc_live.minutes = 59;
    cartridge.rtc_live.seconds = 59;
    cartridge.advance_rtc_seconds(1);
    assert_eq!(cartridge.rtc_live.day_counter, 0);
    assert!(cartridge.rtc_live.carry);

    cartridge.write_ram(0xA000, 0x00);
    assert!(!cartridge.rtc_live.carry);
}

#[test]
fn mbc3_persistent_rtc_elapsed_seconds_follow_the_live_rules() {
    let mut rtc = Mbc3RtcPersistentState {
        seconds: 59,
        minutes: 59,
        hours: 23,
        day_counter: 511,
        halt: false,
        carry: false,
    };

    rtc.apply_elapsed_seconds(2);

    assert_eq!(rtc.seconds, 1);
    assert_eq!(rtc.minutes, 0);
    assert_eq!(rtc.hours, 0);
    assert_eq!(rtc.day_counter, 0);
    assert!(rtc.carry);

    let halted = rtc;
    let mut halted = Mbc3RtcPersistentState {
        halt: true,
        ..halted
    };
    halted.apply_elapsed_seconds(86_400);
    assert_eq!(halted.seconds, rtc.seconds);
    assert_eq!(halted.minutes, rtc.minutes);
    assert_eq!(halted.hours, rtc.hours);
    assert_eq!(halted.day_counter, rtc.day_counter);
    assert!(halted.carry);
}

#[test]
fn strict_validation_rejects_mbc30_like_64kib_sram_configurations() {
    let rom = build_banked_mbc3_rom(0x13, 0x06, 0x05);
    let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect_err("MBC30-like SRAM should fail the standard MBC3 baseline");

    match error {
        CartridgeLoadError::Rejected {
            classification,
            reason,
            ..
        } => {
            assert_eq!(classification.detected_name(), "MBC30");
            assert_eq!(
                classification.selection(),
                CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::PlannedVariant)
            );
            assert!(reason.contains("known reserved variant"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn permissive_validation_can_warn_when_no_ram_mbc3_headers_still_declare_ram() {
    let rom = build_banked_mbc3_rom(0x11, 0x03, 0x02);
    let report = CartridgeSlot::load(rom, &warn_policy())
        .expect("warn policy should admit a no-RAM MBC3 mismatch");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc3);
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("does not provide external RAM"))
    );
}

#[test]
fn mbc5_power_up_state_starts_the_high_window_on_bank_one_while_keeping_bank_zero_reachable() {
    let rom = build_banked_mbc5_rom(0x1E, 0x08, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC5 should load");

    let Some(CartridgeDevice::Mbc5(cartridge)) = report.cartridge().device.as_ref() else {
        panic!("expected MBC5 cartridge");
    };

    assert_eq!(cartridge.variant, Mbc5Variant::RumbleRamBattery);
    assert!(!cartridge.ram_enabled);
    assert_eq!(cartridge.rom_bank_low8, 1);
    assert_eq!(cartridge.rom_bank_high1, 0);
    assert_eq!(cartridge.ram_bank_raw, 0);
    assert!(!cartridge.rumble_on());
    assert_eq!(cartridge.read_rom(0x4000), 0x01);
    assert_eq!(cartridge.read_rom(0x4001), 0x00);

    let mut cartridge = cartridge.clone();
    cartridge.write_rom(0x2000, 0x00);
    assert_eq!(cartridge.read_rom(0x4000), 0x00);
    assert_eq!(cartridge.read_rom(0x4001), 0x00);
}

#[test]
fn mbc5_reaches_bank_0x1ff_without_applying_a_zero_to_one_translation() {
    let rom = build_banked_mbc5_rom(0x1B, 0x08, 0x04);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC5 should load");
    let Some(CartridgeDevice::Mbc5(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC5 cartridge");
    };

    cartridge.write_rom(0x2000, 0xFF);
    cartridge.write_rom(0x3000, 0x00);
    assert_eq!(cartridge.read_rom(0x4000), 0xFF);
    assert_eq!(cartridge.read_rom(0x4001), 0x00);

    cartridge.write_rom(0x2000, 0x00);
    cartridge.write_rom(0x3000, 0x01);
    assert_eq!(cartridge.read_rom(0x4000), 0x00);
    assert_eq!(cartridge.read_rom(0x4001), 0x01);

    cartridge.write_rom(0x2000, 0xFF);
    assert_eq!(cartridge.read_rom(0x4000), 0xFF);
    assert_eq!(cartridge.read_rom(0x4001), 0x01);
}

#[test]
fn mbc5_rumble_control_keeps_motor_state_distinct_from_effective_ram_bank() {
    let rom = build_banked_mbc5_rom(0x1E, 0x03, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC5 should load");
    let Some(CartridgeDevice::Mbc5(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC5 cartridge");
    };

    assert_eq!(cartridge.read_ram(0xA000), RAM_ABSENT_READ_VALUE);
    cartridge.write_rom(0x0000, 0x0A);

    cartridge.write_rom(0x4000, 0x03);
    cartridge.write_ram(0xA000, 0x33);

    cartridge.write_rom(0x4000, 0x0B);
    assert!(cartridge.rumble_on());
    assert_eq!(cartridge.ram_bank_raw, 0x03);
    assert_eq!(cartridge.read_ram(0xA000), 0x33);

    cartridge.write_rom(0x4000, 0x03);
    assert!(!cartridge.rumble_on());
    assert_eq!(cartridge.read_ram(0xA000), 0x33);
}

#[test]
fn strict_validation_rejects_oversized_mbc5_images_and_invalid_rumble_ram_sizes() {
    let oversized = build_test_rom(16 * 1024 * 1024, 0x1B, 0x08, 0x04);
    let oversized_error = CartridgeSlot::load(oversized, &CompatibilityPolicy::strict())
        .expect_err("oversized MBC5 should fail validation");

    match oversized_error {
        CartridgeLoadError::Rejected { reason, .. } => {
            assert!(reason.contains("exceeds the current MBC5 ROM limit"));
        }
        other => panic!("unexpected error: {other:?}"),
    }

    let invalid_rumble_ram = build_banked_mbc5_rom(0x1E, 0x03, 0x04);
    let invalid_rumble_error =
        CartridgeSlot::load(invalid_rumble_ram, &CompatibilityPolicy::strict())
            .expect_err("128 KiB rumble MBC5 should fail validation");

    match invalid_rumble_error {
        CartridgeLoadError::Rejected { reason, .. } => {
            assert!(reason.contains("rumble-capable MBC5 baseline"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn permissive_validation_can_warn_when_no_ram_mbc5_headers_still_declare_ram() {
    let rom = build_banked_mbc5_rom(0x19, 0x03, 0x02);
    let report = CartridgeSlot::load(rom, &warn_policy())
        .expect("warn policy should admit a no-RAM MBC5 mismatch");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc5);
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("does not provide external RAM"))
    );
}

#[test]
fn persistence_metadata_keeps_ram_shapes_and_battery_policy_explicit() {
    let no_mbc_report = CartridgeSlot::load(
        build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x09, 0x00, 0x02),
        &CompatibilityPolicy::strict(),
    )
    .expect("NoMBC+BATTERY should load");
    assert_eq!(
        no_mbc_report.cartridge().persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: NO_MBC_SUPPORTED_RAM_BYTES,
                },
            },
        }
    );

    let mbc2_report = CartridgeSlot::load(
        build_banked_mbc2_rom(0x05, 0x03, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC2 should load");
    assert_eq!(
        mbc2_report.cartridge().persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: false,
            has_rtc: false,
            profile: CartridgePersistenceProfile::NonPersistentRam {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles {
                    cell_count: MBC2_RAM_CELL_COUNT,
                },
            },
        }
    );
}

#[test]
fn restore_persistent_state_validates_mbc2_nibble_payload_values() {
    let report = CartridgeSlot::load(
        build_banked_mbc2_rom(0x06, 0x03, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC2+BATTERY should load");
    let Some(CartridgeDevice::Mbc2(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected MBC2 cartridge");
    };

    let mut invalid_nibbles = [0u8; MBC2_RAM_CELL_COUNT];
    invalid_nibbles[7] = 0xF1;
    let error = cartridge
        .restore_persistent_state(&PersistentCartState::Mbc2Ram {
            ram_nibbles: invalid_nibbles,
        })
        .expect_err("invalid high bits must fail");

    assert_eq!(
        error,
        CartridgePersistentStateError::InvalidMbc2NibbleValue {
            index: 7,
            value: 0xF1,
        }
    );
}

#[test]
fn slot_accessors_and_restore_paths_cover_empty_no_mbc_mbc1_and_mbc2_families() {
    let mut empty = CartridgeSlot::empty();
    assert!(empty.is_empty());
    assert_eq!(empty.classification(), None);
    assert_eq!(empty.persistent_state(), PersistentCartState::None);
    assert_eq!(
        empty.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        empty.restore_persistent_state(&PersistentCartState::Mbc1Ram { ram: vec![] }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "Mbc1Ram",
        }),
    );

    let no_mbc_report = CartridgeSlot::load(
        build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x09, 0x00, 0x02),
        &CompatibilityPolicy::strict(),
    )
    .expect("NoMBC+BATTERY should load");
    let (mut no_mbc, _) = no_mbc_report.into_parts();
    assert!(!no_mbc.is_empty());
    assert_eq!(
        no_mbc
            .classification()
            .map(CartridgeClassification::selection),
        Some(CartridgeSelection::Supported(
            SupportedCartridgeFamily::NoMbc
        )),
    );
    assert_eq!(
        no_mbc.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: NO_MBC_SUPPORTED_RAM_BYTES,
                },
            },
        },
    );
    no_mbc.write_ram(0xA000, 0x12);
    assert_eq!(no_mbc.read_ram(0xA000), 0x12);
    let no_mbc_before_rtc = no_mbc.persistent_state();
    no_mbc.advance_rtc_seconds(7);
    assert_eq!(no_mbc.persistent_state(), no_mbc_before_rtc);

    let restored_no_mbc = PersistentCartState::NoMbcRam {
        ram: vec![0x5A; NO_MBC_SUPPORTED_RAM_BYTES],
    };
    no_mbc
        .restore_persistent_state(&restored_no_mbc)
        .expect("NoMBC RAM state should restore");
    assert_eq!(no_mbc.persistent_state(), restored_no_mbc);
    assert_eq!(
        no_mbc.restore_persistent_state(&PersistentCartState::NoMbcRam { ram: vec![0; 4] }),
        Err(CartridgePersistentStateError::RamLengthMismatch {
            expected: NO_MBC_SUPPORTED_RAM_BYTES,
            actual: 4,
        }),
    );
    assert_eq!(
        no_mbc.restore_persistent_state(&PersistentCartState::Mbc1Ram {
            ram: vec![0; NO_MBC_SUPPORTED_RAM_BYTES],
        }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "NoMbcRam",
            actual: "Mbc1Ram",
        }),
    );

    let mbc1_report = CartridgeSlot::load(
        build_banked_mbc1_rom(0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC1 should load");
    let (mut mbc1, _) = mbc1_report.into_parts();
    assert_eq!(
        mbc1.classification()
            .map(CartridgeClassification::selection),
        Some(CartridgeSelection::Supported(
            SupportedCartridgeFamily::Mbc1
        )),
    );
    mbc1.write_rom(0x0000, 0x0A);
    mbc1.write_ram(0xA000, 0x34);
    assert_eq!(mbc1.read_ram(0xA000), 0x34);
    let restored_mbc1 = PersistentCartState::Mbc1Ram {
        ram: vec![0x77; 32 * 1024],
    };
    mbc1.restore_persistent_state(&restored_mbc1)
        .expect("MBC1 RAM state should restore");
    assert_eq!(mbc1.persistent_state(), restored_mbc1);
    assert_eq!(
        mbc1.restore_persistent_state(&PersistentCartState::Mbc1Ram { ram: vec![0; 8] }),
        Err(CartridgePersistentStateError::RamLengthMismatch {
            expected: 32 * 1024,
            actual: 8,
        }),
    );
    assert_eq!(
        mbc1.restore_persistent_state(&PersistentCartState::None),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "Mbc1Ram",
            actual: "None",
        }),
    );

    let mbc2_report = CartridgeSlot::load(
        build_banked_mbc2_rom(0x06, 0x03, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC2+BATTERY should load");
    let (mut mbc2, _) = mbc2_report.into_parts();
    assert_eq!(
        mbc2.classification()
            .map(CartridgeClassification::selection),
        Some(CartridgeSelection::Supported(
            SupportedCartridgeFamily::Mbc2
        )),
    );
    assert_eq!(
        mbc2.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles {
                    cell_count: MBC2_RAM_CELL_COUNT,
                },
            },
        },
    );
    mbc2.write_rom(0x0000, 0x0A);
    mbc2.write_ram(0xA000, 0xAB);
    assert_eq!(mbc2.read_ram(0xA000), 0xFB);

    let mut restored_nibbles = [0_u8; MBC2_RAM_CELL_COUNT];
    restored_nibbles[0] = 0x0C;
    let restored_mbc2 = PersistentCartState::Mbc2Ram {
        ram_nibbles: restored_nibbles,
    };
    mbc2.restore_persistent_state(&restored_mbc2)
        .expect("MBC2 nibble state should restore");
    assert_eq!(mbc2.persistent_state(), restored_mbc2);
    assert_eq!(
        mbc2.restore_persistent_state(&PersistentCartState::None),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "Mbc2Ram",
            actual: "None",
        }),
    );
}

#[test]
fn slot_accessors_and_restore_paths_cover_mbc3_and_mbc5_rtc_and_rumble_paths() {
    let mbc3_report = CartridgeSlot::load(
        build_banked_mbc3_rom(0x10, 0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC3+TIMER+RAM+BATTERY should load");
    let (mut mbc3, _) = mbc3_report.into_parts();
    assert_eq!(
        mbc3.classification()
            .map(CartridgeClassification::selection),
        Some(CartridgeSelection::Supported(
            SupportedCartridgeFamily::Mbc3
        )),
    );
    assert_eq!(
        mbc3.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: 32 * 1024,
                },
            },
        },
    );
    mbc3.advance_rtc_seconds(3_661);
    assert_eq!(
        mbc3.persistent_state(),
        PersistentCartState::Mbc3RamRtc {
            ram: vec![0; 32 * 1024],
            rtc: Mbc3RtcPersistentState {
                seconds: 1,
                minutes: 1,
                hours: 1,
                day_counter: 0,
                halt: false,
                carry: false,
            },
        },
    );

    let restored_mbc3 = PersistentCartState::Mbc3RamRtc {
        ram: vec![0x6B; 32 * 1024],
        rtc: Mbc3RtcPersistentState {
            seconds: 9,
            minutes: 8,
            hours: 7,
            day_counter: 6,
            halt: true,
            carry: true,
        },
    };
    mbc3.restore_persistent_state(&restored_mbc3)
        .expect("MBC3 RAM+RTC state should restore");
    assert_eq!(mbc3.persistent_state(), restored_mbc3);
    assert_eq!(
        mbc3.restore_persistent_state(&PersistentCartState::Mbc3Ram {
            ram: vec![0; 32 * 1024],
        }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "Mbc3RamRtc",
            actual: "Mbc3Ram",
        }),
    );
    assert_eq!(
        mbc3.restore_persistent_state(&PersistentCartState::Mbc3RamRtc {
            ram: vec![0; 4],
            rtc: Mbc3RtcPersistentState {
                seconds: 0,
                minutes: 0,
                hours: 0,
                day_counter: 0,
                halt: false,
                carry: false,
            },
        }),
        Err(CartridgePersistentStateError::RamLengthMismatch {
            expected: 32 * 1024,
            actual: 4,
        }),
    );

    let mbc5_report = CartridgeSlot::load(
        build_banked_mbc5_rom(0x1E, 0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC5+RUMBLE+RAM+BATTERY should load");
    let (mut mbc5, _) = mbc5_report.into_parts();
    assert_eq!(
        mbc5.classification()
            .map(CartridgeClassification::selection),
        Some(CartridgeSelection::Supported(
            SupportedCartridgeFamily::Mbc5
        )),
    );
    assert!(!mbc5.rumble_on());
    mbc5.advance_rtc_seconds(99);
    assert!(!mbc5.rumble_on());
    mbc5.write_rom(0x0000, 0x0A);
    mbc5.write_rom(0x4000, 0x0B);
    mbc5.write_ram(0xA000, 0x44);
    assert!(mbc5.rumble_on());
    assert_eq!(mbc5.read_ram(0xA000), 0x44);
    assert_eq!(
        mbc5.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: 32 * 1024,
                },
            },
        },
    );

    let restored_mbc5 = PersistentCartState::Mbc5Ram {
        ram: vec![0x24; 32 * 1024],
    };
    mbc5.restore_persistent_state(&restored_mbc5)
        .expect("MBC5 RAM state should restore");
    assert_eq!(mbc5.persistent_state(), restored_mbc5);
    assert_eq!(
        mbc5.restore_persistent_state(&PersistentCartState::NoMbcRam {
            ram: vec![0; 32 * 1024],
        }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "Mbc5Ram",
            actual: "NoMbcRam",
        }),
    );
    assert_eq!(
        mbc5.restore_persistent_state(&PersistentCartState::Mbc5Ram { ram: vec![0; 8] }),
        Err(CartridgePersistentStateError::RamLengthMismatch {
            expected: 32 * 1024,
            actual: 8,
        }),
    );
}

#[test]
fn private_validation_helpers_cover_remaining_mapper_rejection_and_signature_paths() {
    let strict = CompatibilityPolicy::strict();
    let mut diagnostics = Vec::new();

    let mbc2_header = CartridgeHeader::parse(&build_banked_mbc2_rom(0x06, 0x03, 0x02))
        .expect("header should parse");
    let mbc2_error = validate_mbc2(
        &mbc2_header,
        256 * 1024,
        &strict,
        &CartridgeClassification::classify(0x06),
        &mut diagnostics,
    )
    .expect_err("strict MBC2 validation should reject external-RAM metadata");
    assert!(matches!(mbc2_error, CartridgeLoadError::Rejected { .. }));

    diagnostics.clear();
    let mbc3_header = CartridgeHeader::parse(&build_banked_mbc3_rom(0x11, 0x03, 0x02))
        .expect("header should parse");
    let mbc3_error = validate_mbc3(
        &mbc3_header,
        256 * 1024,
        &strict,
        &CartridgeClassification::classify(0x11),
        &mut diagnostics,
    )
    .expect_err("strict no-RAM MBC3 validation should reject RAM metadata");
    assert!(matches!(mbc3_error, CartridgeLoadError::Rejected { .. }));

    diagnostics.clear();
    let mbc5_header = CartridgeHeader::parse(&build_banked_mbc5_rom(0x19, 0x03, 0x02))
        .expect("header should parse");
    let mbc5_error = validate_mbc5(
        &mbc5_header,
        256 * 1024,
        &strict,
        &CartridgeClassification::classify(0x19),
        &mut diagnostics,
    )
    .expect_err("strict no-RAM MBC5 validation should reject RAM metadata");
    assert!(matches!(mbc5_error, CartridgeLoadError::Rejected { .. }));

    let ordinary_mbc1_rom = build_banked_mbc1_rom_with_type(0x03, 0x05, 0x00);
    let ordinary_mbc1_header =
        CartridgeHeader::parse(&ordinary_mbc1_rom).expect("header should parse");
    assert!(!is_mbc1m_multicart_signature(
        &ordinary_mbc1_header,
        &ordinary_mbc1_rom,
    ));

    let mut mbc1m_candidate = build_banked_mbc1_rom_with_type(0x01, 0x05, 0x00);
    mark_mbc1_multicart_subheaders(&mut mbc1m_candidate);
    let mbc1m_header = CartridgeHeader::parse(&mbc1m_candidate).expect("header should parse");
    assert!(is_mbc1m_multicart_signature(
        &mbc1m_header,
        &mbc1m_candidate
    ));

    let title_bytes = b"NOTHING\0\0\0\0\0\0\0\0";
    assert!(is_wisdom_tree_signature(
        title_bytes,
        0x00,
        0x00,
        64 * 1024,
        0x00,
    ));
    assert!(is_wisdom_tree_signature(
        title_bytes,
        0xC0,
        0x00,
        NO_MBC_SUPPORTED_ROM_BYTES,
        0xD1,
    ));
    assert!(!is_wisdom_tree_signature(
        title_bytes,
        0x01,
        0x00,
        NO_MBC_SUPPORTED_ROM_BYTES,
        0x00,
    ));
}

#[test]
fn private_validation_helpers_cover_remaining_size_code_and_image_mismatch_rejections() {
    let strict = CompatibilityPolicy::strict();
    let mut diagnostics = Vec::new();

    let mbc1_unknown_size = CartridgeHeader::parse(&build_test_rom(
        NO_MBC_SUPPORTED_ROM_BYTES,
        0x03,
        0xFF,
        0x00,
    ))
    .expect("header should parse");
    let mbc1_unknown_size_error = validate_mbc1(
        &mbc1_unknown_size,
        NO_MBC_SUPPORTED_ROM_BYTES,
        &strict,
        &CartridgeClassification::classify(0x03),
        &mut diagnostics,
    )
    .expect_err("unknown MBC1 ROM size code should fail");
    assert!(matches!(
        mbc1_unknown_size_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("unsupported ROM size code")
    ));

    diagnostics.clear();
    let mbc1_mismatch =
        CartridgeHeader::parse(&build_banked_mbc1_rom(0x03, 0x00)).expect("header should parse");
    let mbc1_mismatch_error = validate_mbc1(
        &mbc1_mismatch,
        128 * 1024,
        &strict,
        &CartridgeClassification::classify(0x03),
        &mut diagnostics,
    )
    .expect_err("MBC1 image-size mismatches should fail");
    assert!(matches!(
        mbc1_mismatch_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("loaded ROM is 131072 bytes")
    ));

    diagnostics.clear();
    let mbc1_invalid_layout =
        CartridgeHeader::parse(&build_banked_mbc1_rom(0x52, 0x00)).expect("header should parse");
    let mbc1_invalid_layout_error = validate_mbc1(
        &mbc1_invalid_layout,
        72 * 16 * 1024,
        &strict,
        &CartridgeClassification::classify(0x03),
        &mut diagnostics,
    )
    .expect_err("non-baseline MBC1 ROM sizes should fail");
    assert!(matches!(
        mbc1_invalid_layout_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("not valid for the current MBC1 baseline")
    ));

    diagnostics.clear();
    let mbc2_unknown_size = CartridgeHeader::parse(&build_test_rom(
        NO_MBC_SUPPORTED_ROM_BYTES,
        0x06,
        0xFF,
        0x00,
    ))
    .expect("header should parse");
    let mbc2_unknown_size_error = validate_mbc2(
        &mbc2_unknown_size,
        NO_MBC_SUPPORTED_ROM_BYTES,
        &strict,
        &CartridgeClassification::classify(0x06),
        &mut diagnostics,
    )
    .expect_err("unknown MBC2 ROM size code should fail");
    assert!(matches!(
        mbc2_unknown_size_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("unsupported ROM size code")
    ));

    diagnostics.clear();
    let mbc2_mismatch = CartridgeHeader::parse(&build_banked_mbc2_rom(0x06, 0x03, 0x00))
        .expect("header should parse");
    let mbc2_mismatch_error = validate_mbc2(
        &mbc2_mismatch,
        128 * 1024,
        &strict,
        &CartridgeClassification::classify(0x06),
        &mut diagnostics,
    )
    .expect_err("MBC2 image-size mismatches should fail");
    assert!(matches!(
        mbc2_mismatch_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("loaded ROM is 131072 bytes")
    ));

    diagnostics.clear();
    let mbc3_unknown_size = CartridgeHeader::parse(&build_test_rom(
        NO_MBC_SUPPORTED_ROM_BYTES,
        0x10,
        0xFF,
        0x03,
    ))
    .expect("header should parse");
    let mbc3_unknown_size_error = validate_mbc3(
        &mbc3_unknown_size,
        NO_MBC_SUPPORTED_ROM_BYTES,
        &strict,
        &CartridgeClassification::classify(0x10),
        &mut diagnostics,
    )
    .expect_err("unknown MBC3 ROM size code should fail");
    assert!(matches!(
        mbc3_unknown_size_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("unsupported ROM size code")
    ));

    diagnostics.clear();
    let mbc5_unknown_size = CartridgeHeader::parse(&build_test_rom(
        NO_MBC_SUPPORTED_ROM_BYTES,
        0x1A,
        0xFF,
        0x02,
    ))
    .expect("header should parse");
    let mbc5_unknown_size_error = validate_mbc5(
        &mbc5_unknown_size,
        NO_MBC_SUPPORTED_ROM_BYTES,
        &strict,
        &CartridgeClassification::classify(0x1A),
        &mut diagnostics,
    )
    .expect_err("unknown MBC5 ROM size code should fail");
    assert!(matches!(
        mbc5_unknown_size_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("unsupported ROM size code")
    ));

    diagnostics.clear();
    let mbc5_mismatch = CartridgeHeader::parse(&build_banked_mbc5_rom(0x1A, 0x03, 0x02))
        .expect("header should parse");
    let mbc5_mismatch_error = validate_mbc5(
        &mbc5_mismatch,
        128 * 1024,
        &strict,
        &CartridgeClassification::classify(0x1A),
        &mut diagnostics,
    )
    .expect_err("MBC5 image-size mismatches should fail");
    assert!(matches!(
        mbc5_mismatch_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("loaded ROM is 131072 bytes")
    ));
}

#[test]
fn classification_and_private_helper_paths_cover_remaining_documented_types_and_flags() {
    let mmm01_ram = CartridgeClassification::classify(0x0C);
    assert_eq!(mmm01_ram.detected_name(), "MMM01+RAM");
    assert_eq!(
        mmm01_ram.selection(),
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::DocumentedButUnsupported)
    );

    let mmm01_battery = CartridgeClassification::classify(0x0D);
    assert_eq!(mmm01_battery.detected_name(), "MMM01+RAM+BATTERY");
    assert_eq!(
        mmm01_battery.selection(),
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::DocumentedButUnsupported)
    );

    let tama5 = CartridgeClassification::classify(0xFD);
    assert_eq!(tama5.detected_name(), "BANDAI TAMA5");
    assert_eq!(
        tama5.selection(),
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::AccessorySpecialCase)
    );

    assert_eq!(decode_cgb_flag(0xC0), CgbFlag::Only);
    assert_eq!(decode_cgb_flag(0x42), CgbFlag::Unknown(0x42));
    assert_eq!(decode_sgb_flag(0x7F), SgbFlag::Unknown(0x7F));
    assert_eq!(expected_ram_code_decompressed(0x99), 0);
    assert!(!matches_padded_title(b"GB", b"GBTEST"));

    let rtc = Mbc3RtcPersistentState {
        seconds: 1,
        minutes: 2,
        hours: 3,
        day_counter: 0x0104,
        halt: true,
        carry: true,
    };
    assert_eq!(
        Mbc3RtcState::from(rtc),
        Mbc3RtcState {
            seconds: 1,
            minutes: 2,
            hours: 3,
            day_counter: 0x0104,
            halt: true,
            carry: true,
        }
    );
    assert_eq!(PersistentCartState::Mbc3Rtc { rtc }.kind_name(), "Mbc3Rtc");
    assert_eq!(
        PersistentCartState::Mbc3RamRtc { ram: vec![], rtc }.kind_name(),
        "Mbc3RamRtc"
    );
    assert_eq!(
        PersistentCartState::Mbc5Ram { ram: vec![] }.kind_name(),
        "Mbc5Ram"
    );

    let mut diagnostics = Vec::new();
    assert_eq!(
        record_degradable_issue(
            &mut diagnostics,
            ValidationPolicy::Ignore,
            "ignored warning".to_owned(),
        ),
        Ok(())
    );
    assert!(diagnostics.is_empty());
}

#[test]
fn slot_accessors_and_restore_paths_cover_non_battery_and_ramless_variants() {
    let no_mbc_report = CartridgeSlot::load(
        build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x08, 0x00, 0x02),
        &CompatibilityPolicy::strict(),
    )
    .expect("NoMBC+RAM should load");
    assert!(!no_mbc_report.cartridge().rumble_on());
    let Some(CartridgeDevice::NoMbc(mut no_mbc)) = no_mbc_report.cartridge().device.clone() else {
        panic!("expected NoMBC cartridge");
    };
    assert!(!no_mbc.has_battery());
    assert_eq!(
        no_mbc.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: false,
            has_rtc: false,
            profile: CartridgePersistenceProfile::NonPersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: NO_MBC_SUPPORTED_RAM_BYTES,
                },
            },
        }
    );
    assert_eq!(no_mbc.persistent_state(), PersistentCartState::None);
    assert_eq!(
        no_mbc.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        no_mbc.restore_persistent_state(&PersistentCartState::NoMbcRam {
            ram: vec![0; NO_MBC_SUPPORTED_RAM_BYTES],
        }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "NoMbcRam",
        }),
    );

    let no_mbc_battery_report = CartridgeSlot::load(
        build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x09, 0x00, 0x02),
        &CompatibilityPolicy::strict(),
    )
    .expect("NoMBC+RAM+BATTERY should load");
    let Some(CartridgeDevice::NoMbc(mut no_mbc_battery)) =
        no_mbc_battery_report.cartridge().device.clone()
    else {
        panic!("expected NoMBC battery cartridge");
    };
    no_mbc_battery.ram = None;
    assert!(no_mbc_battery.has_battery());
    assert_eq!(
        no_mbc_battery.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        no_mbc_battery.restore_persistent_state(&PersistentCartState::NoMbcRam { ram: vec![] }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "NoMbcRam",
        }),
    );

    let mbc1_report = CartridgeSlot::load(
        build_banked_mbc1_rom_with_type(0x02, 0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC1+RAM should load");
    let Some(CartridgeDevice::Mbc1(mut mbc1)) = mbc1_report.cartridge().device.clone() else {
        panic!("expected MBC1 cartridge");
    };
    assert!(!mbc1.has_battery());
    assert_eq!(
        mbc1.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: false,
            has_rtc: false,
            profile: CartridgePersistenceProfile::NonPersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: 32 * 1024,
                },
            },
        }
    );
    assert_eq!(mbc1.persistent_state(), PersistentCartState::None);
    assert_eq!(
        mbc1.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        mbc1.restore_persistent_state(&PersistentCartState::Mbc1Ram {
            ram: vec![0; 32 * 1024],
        }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "Mbc1Ram",
        }),
    );

    let mbc1_battery_report = CartridgeSlot::load(
        build_banked_mbc1_rom_with_type(0x03, 0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC1+RAM+BATTERY should load");
    let Some(CartridgeDevice::Mbc1(mut mbc1_battery)) =
        mbc1_battery_report.cartridge().device.clone()
    else {
        panic!("expected MBC1 battery cartridge");
    };
    mbc1_battery.ram = None;
    assert!(mbc1_battery.has_battery());
    assert_eq!(
        mbc1_battery.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        mbc1_battery.restore_persistent_state(&PersistentCartState::Mbc1Ram { ram: vec![] }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "Mbc1Ram",
        }),
    );

    let mbc2_report = CartridgeSlot::load(
        build_banked_mbc2_rom(0x05, 0x03, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC2 should load");
    let Some(CartridgeDevice::Mbc2(mut mbc2)) = mbc2_report.cartridge().device.clone() else {
        panic!("expected MBC2 cartridge");
    };
    assert!(!mbc2.has_battery());
    assert_eq!(mbc2.persistent_state(), PersistentCartState::None);
    assert_eq!(
        mbc2.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        mbc2.restore_persistent_state(&PersistentCartState::Mbc2Ram {
            ram_nibbles: [0; MBC2_RAM_CELL_COUNT],
        }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "Mbc2Ram",
        }),
    );

    let mbc5_report = CartridgeSlot::load(
        build_banked_mbc5_rom(0x1A, 0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC5+RAM should load");
    let Some(CartridgeDevice::Mbc5(mut mbc5)) = mbc5_report.cartridge().device.clone() else {
        panic!("expected MBC5 cartridge");
    };
    assert!(!mbc5.has_battery());
    assert_eq!(
        mbc5.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: false,
            has_rtc: false,
            profile: CartridgePersistenceProfile::NonPersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: 32 * 1024,
                },
            },
        }
    );
    assert_eq!(mbc5.persistent_state(), PersistentCartState::None);
    assert_eq!(
        mbc5.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        mbc5.restore_persistent_state(&PersistentCartState::Mbc5Ram {
            ram: vec![0; 32 * 1024],
        }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "Mbc5Ram",
        }),
    );

    let mbc5_battery_report = CartridgeSlot::load(
        build_banked_mbc5_rom(0x1B, 0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC5+RAM+BATTERY should load");
    let Some(CartridgeDevice::Mbc5(mut mbc5_battery)) =
        mbc5_battery_report.cartridge().device.clone()
    else {
        panic!("expected MBC5 battery cartridge");
    };
    mbc5_battery.ram = None;
    assert!(mbc5_battery.has_battery());
    assert_eq!(
        mbc5_battery.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::None,
        }
    );
    assert_eq!(
        mbc5_battery.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        mbc5_battery.restore_persistent_state(&PersistentCartState::Mbc5Ram { ram: vec![] }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "Mbc5Ram",
        }),
    );
}

#[test]
fn mbc3_private_persistence_and_mapper_helpers_cover_remaining_variants() {
    let no_battery_report = CartridgeSlot::load(
        build_banked_mbc3_rom(0x12, 0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC3+RAM should load");
    let Some(CartridgeDevice::Mbc3(mut no_battery)) = no_battery_report.cartridge().device.clone()
    else {
        panic!("expected MBC3 cartridge");
    };
    assert!(!no_battery.has_battery());
    assert_eq!(
        no_battery.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: false,
            has_rtc: false,
            profile: CartridgePersistenceProfile::NonPersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: 32 * 1024,
                },
            },
        }
    );
    assert_eq!(no_battery.persistent_state(), PersistentCartState::None);
    assert_eq!(
        no_battery.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        no_battery.restore_persistent_state(&PersistentCartState::Mbc3Ram {
            ram: vec![0; 32 * 1024],
        }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "Mbc3Ram",
        }),
    );
    assert_eq!(no_battery.read_ram(0xA000), RAM_ABSENT_READ_VALUE);
    let before_disabled_write = no_battery.ram.clone();
    no_battery.write_ram(0xA000, 0x44);
    assert_eq!(no_battery.ram, before_disabled_write);
    no_battery.write_rom(0x8000, 0x99);
    no_battery.write_rom(0x6000, 0x01);
    assert!(!no_battery.rtc_latch_armed);
    no_battery.write_rom(0x0000, 0x0A);
    no_battery.write_rom(0x4000, 0x05);
    assert_eq!(no_battery.read_ram(0xA000), RAM_ABSENT_READ_VALUE);
    no_battery.write_rom(0x4000, 0x08);
    assert_eq!(no_battery.read_ram(0xA000), RAM_ABSENT_READ_VALUE);
    no_battery.write_ram(0xA000, 0x12);
    assert_eq!(no_battery.rtc_live.seconds, 0);
    assert_eq!(no_battery.effective_rom_bank(0), 0);

    let rtc_only_report = CartridgeSlot::load(
        build_banked_mbc3_rom(0x0F, 0x03, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC3+TIMER+BATTERY should load");
    let Some(CartridgeDevice::Mbc3(mut rtc_only)) = rtc_only_report.cartridge().device.clone()
    else {
        panic!("expected RTC-only MBC3 cartridge");
    };
    assert!(rtc_only.has_battery());
    assert_eq!(
        rtc_only.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRtc,
        }
    );
    assert_eq!(
        rtc_only.persistent_state(),
        PersistentCartState::Mbc3Rtc {
            rtc: Mbc3RtcPersistentState {
                seconds: 0,
                minutes: 0,
                hours: 0,
                day_counter: 0,
                halt: false,
                carry: false,
            },
        }
    );
    let restored_rtc_only = PersistentCartState::Mbc3Rtc {
        rtc: Mbc3RtcPersistentState {
            seconds: 9,
            minutes: 8,
            hours: 7,
            day_counter: 6,
            halt: true,
            carry: true,
        },
    };
    rtc_only
        .restore_persistent_state(&restored_rtc_only)
        .expect("RTC-only state should restore");
    assert_eq!(rtc_only.persistent_state(), restored_rtc_only);
    assert_eq!(
        rtc_only.restore_persistent_state(&PersistentCartState::None),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "Mbc3Rtc",
            actual: "None",
        }),
    );

    let ram_only_report = CartridgeSlot::load(
        build_banked_mbc3_rom(0x13, 0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC3+RAM+BATTERY should load");
    let Some(CartridgeDevice::Mbc3(mut ram_only)) = ram_only_report.cartridge().device.clone()
    else {
        panic!("expected RAM-only MBC3 cartridge");
    };
    assert!(ram_only.has_battery());
    assert_eq!(
        ram_only.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: 32 * 1024,
                },
            },
        }
    );
    let restored_ram_only = PersistentCartState::Mbc3Ram {
        ram: vec![0x6D; 32 * 1024],
    };
    ram_only
        .restore_persistent_state(&restored_ram_only)
        .expect("RAM-only MBC3 state should restore");
    assert_eq!(ram_only.persistent_state(), restored_ram_only);
    assert_eq!(
        ram_only.restore_persistent_state(&PersistentCartState::None),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "Mbc3Ram",
            actual: "None",
        }),
    );

    let mut ramless_no_rtc = ram_only.clone();
    ramless_no_rtc.ram = None;
    assert_eq!(
        ramless_no_rtc.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: false,
            profile: CartridgePersistenceProfile::None,
        }
    );
    assert_eq!(ramless_no_rtc.persistent_state(), PersistentCartState::None);
    assert_eq!(
        ramless_no_rtc.restore_persistent_state(&PersistentCartState::None),
        Ok(())
    );
    assert_eq!(
        ramless_no_rtc.restore_persistent_state(&PersistentCartState::Mbc3Ram { ram: vec![] }),
        Err(CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "Mbc3Ram",
        }),
    );
}

#[test]
fn private_validation_helpers_cover_remaining_policy_and_mapper_branches() {
    let strict = CompatibilityPolicy::strict();
    let experimental = CompatibilityPolicy::experimental();
    let mut diagnostics = Vec::new();

    let no_mbc_rare_rom = build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x08, 0x00, 0x02);
    let no_mbc_rare_header = CartridgeHeader::parse(&no_mbc_rare_rom).expect("header should parse");
    validate_no_mbc(
        &no_mbc_rare_header,
        no_mbc_rare_rom.len(),
        &strict,
        &CartridgeClassification::classify(0x08),
        &mut diagnostics,
    )
    .expect("rare NoMBC baseline should still validate");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("rare but still treated as a valid No MBC variant")
    }));

    diagnostics.clear();
    let mut mbc1m_with_ram_rom = build_banked_mbc1_rom_with_type(0x01, 0x05, 0x02);
    mark_mbc1_multicart_subheaders(&mut mbc1m_with_ram_rom);
    let mbc1m_with_ram_header =
        CartridgeHeader::parse(&mbc1m_with_ram_rom).expect("header should parse");
    let mbc1m_with_ram_classification = supported(0x01, "MBC1M", SupportedCartridgeFamily::Mbc1);
    let mbc1m_with_ram_error = validate_mbc1(
        &mbc1m_with_ram_header,
        mbc1m_with_ram_rom.len(),
        &experimental,
        &mbc1m_with_ram_classification,
        &mut diagnostics,
    )
    .expect_err("MBC1M with RAM should remain rejected");
    assert!(matches!(
        mbc1m_with_ram_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("no-RAM 1 MiB multicart baseline")
    ));

    diagnostics.clear();
    let mbc3_reserved_header =
        CartridgeHeader::parse(&build_test_rom(256 * 1024, 0x13, 0x03, 0x05))
            .expect("header should parse");
    let mbc3_reserved_error = validate_mbc3(
        &mbc3_reserved_header,
        256 * 1024,
        &strict,
        &CartridgeClassification::classify(0x13),
        &mut diagnostics,
    )
    .expect_err("standard MBC3 should reject the reserved 64 KiB SRAM shape");
    assert!(matches!(
        mbc3_reserved_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("reserved for the future MBC30 variant")
    ));

    diagnostics.clear();
    let mbc3_invalid_ram_header =
        CartridgeHeader::parse(&build_test_rom(256 * 1024, 0x13, 0x03, 0x04))
            .expect("header should parse");
    let mbc3_invalid_ram_error = validate_mbc3(
        &mbc3_invalid_ram_header,
        256 * 1024,
        &strict,
        &CartridgeClassification::classify(0x13),
        &mut diagnostics,
    )
    .expect_err("standard MBC3 should reject unsupported RAM codes");
    assert!(matches!(
        mbc3_invalid_ram_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("not valid for the current standard MBC3 baseline")
    ));

    diagnostics.clear();
    let mbc5_standard_invalid_ram_header =
        CartridgeHeader::parse(&build_test_rom(256 * 1024, 0x1A, 0x03, 0x01))
            .expect("header should parse");
    let mbc5_standard_invalid_ram_error = validate_mbc5(
        &mbc5_standard_invalid_ram_header,
        256 * 1024,
        &strict,
        &CartridgeClassification::classify(0x1A),
        &mut diagnostics,
    )
    .expect_err("standard MBC5 should reject unsupported RAM codes");
    assert!(matches!(
        mbc5_standard_invalid_ram_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("current standard MBC5 baseline")
    ));

    diagnostics.clear();
    let mbc5_rumble_invalid_ram_header =
        CartridgeHeader::parse(&build_test_rom(256 * 1024, 0x1D, 0x03, 0x04))
            .expect("header should parse");
    let mbc5_rumble_invalid_ram_error = validate_mbc5(
        &mbc5_rumble_invalid_ram_header,
        256 * 1024,
        &strict,
        &CartridgeClassification::classify(0x1D),
        &mut diagnostics,
    )
    .expect_err("rumble-capable MBC5 should reject unsupported RAM codes");
    assert!(matches!(
        mbc5_rumble_invalid_ram_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("rumble-capable")
    ));
}

#[test]
fn private_mapper_helpers_cover_remaining_zero_bank_noop_and_none_profile_branches() {
    let mbc1_header = CartridgeHeader::parse(&build_banked_mbc1_rom_with_type(0x01, 0x05, 0x00))
        .expect("header should parse");
    let mut mbc1 = Mbc1Cartridge {
        rom: vec![0; 1024 * 1024],
        ram: None,
        has_battery: false,
        header: mbc1_header,
        classification: supported(0x01, "MBC1M", SupportedCartridgeFamily::Mbc1),
        variant: Mbc1Variant::Mbc1M,
        wiring: Mbc1Wiring::LargeRom,
        ram_enabled: false,
        rom_bank_low5: 0,
        secondary_bank: 2,
        banking_mode: 0,
    };
    let mbc1_before_noop = mbc1.clone();
    mbc1.write_rom(0x8000, 0xFF);
    assert_eq!(mbc1, mbc1_before_noop);
    assert_eq!(mbc1.effective_high_rom_bank(0), 0);
    assert_eq!(mbc1.effective_low_rom_bank(0), 0);
    assert_eq!(mbc1.effective_low_rom_bank(64), 0);
    assert_eq!(mbc1.effective_ram_offset(0xA123), 0x0123);
    assert_eq!(
        mbc1.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: false,
            has_rtc: false,
            profile: CartridgePersistenceProfile::None,
        },
    );

    let mbc2_header = CartridgeHeader::parse(&build_banked_mbc2_rom(0x05, 0x03, 0x00))
        .expect("header should parse");
    let mbc2 = Mbc2Cartridge {
        rom: vec![0; 256 * 1024],
        ram_nibbles: [0; MBC2_RAM_CELL_COUNT],
        has_battery: false,
        header: mbc2_header,
        classification: CartridgeClassification::classify(0x05),
        ram_enabled: false,
        rom_bank_low4: 0,
    };
    assert_eq!(mbc2.effective_high_rom_bank(0), 0);

    let mbc3_header = CartridgeHeader::parse(&build_banked_mbc3_rom(0x13, 0x03, 0x03))
        .expect("header should parse");
    let mut mbc3 = Mbc3Cartridge {
        rom: vec![0; 256 * 1024],
        ram: Some(vec![0x11; 32 * 1024]),
        has_battery: true,
        has_rtc: false,
        header: mbc3_header,
        classification: CartridgeClassification::classify(0x13),
        variant: Mbc3Variant::Standard,
        ram_rtc_enabled: true,
        rom_bank: 0,
        ram_or_rtc_select: Mbc3RamRtcSelect::ReservedSelector(0x05),
        rtc_live: Mbc3RtcState::default(),
        rtc_latched: Mbc3RtcState::default(),
        rtc_latched_valid: false,
        rtc_latch_armed: false,
    };
    mbc3.write_ram(0xA000, 0x77);
    assert_eq!(mbc3.ram.as_ref().expect("RAM should exist")[0], 0x11);
    assert_eq!(
        mbc3.restore_persistent_state(&PersistentCartState::Mbc3Ram { ram: vec![0; 8] }),
        Err(CartridgePersistentStateError::RamLengthMismatch {
            expected: 32 * 1024,
            actual: 8,
        }),
    );

    let rtc_only_report = CartridgeSlot::load(
        build_banked_mbc3_rom(0x0F, 0x03, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("RTC-only MBC3 should load");
    assert_eq!(
        rtc_only_report.cartridge().persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: true,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRtc,
        },
    );

    let mbc5_header = CartridgeHeader::parse(&build_banked_mbc5_rom(0x19, 0x03, 0x00))
        .expect("header should parse");
    let mut mbc5 = Mbc5Cartridge {
        rom: vec![0; 256 * 1024],
        ram: None,
        has_battery: false,
        has_rumble: false,
        header: mbc5_header,
        classification: CartridgeClassification::classify(0x19),
        variant: Mbc5Variant::NoRam,
        ram_enabled: false,
        rom_bank_low8: 0,
        rom_bank_high1: 0,
        ram_bank_raw: 0,
        rumble_on: false,
    };
    let mbc5_before_noop = mbc5.clone();
    mbc5.write_rom(0x8000, 0xFF);
    assert_eq!(mbc5, mbc5_before_noop);
    assert_eq!(mbc5.effective_rom_bank(0), 0);
}

#[test]
fn private_validation_helpers_cover_remaining_strict_rejection_blocks() {
    let strict = CompatibilityPolicy::strict();
    let mut diagnostics = Vec::new();
    let no_mbc_base_header = CartridgeHeader::parse(&build_test_rom(
        NO_MBC_SUPPORTED_ROM_BYTES,
        0x00,
        0x00,
        0x00,
    ))
    .expect("header should parse");

    let mut no_mbc_header = no_mbc_base_header.clone();
    no_mbc_header.ram_size = RamSizeInfo {
        raw_code: 0x02,
        decoded_bytes: Some(8 * 1024),
        bank_count: Some(1),
    };
    let no_mbc_ram_code_error = validate_no_mbc(
        &no_mbc_header,
        NO_MBC_SUPPORTED_ROM_BYTES,
        &strict,
        &CartridgeClassification::classify(0x00),
        &mut diagnostics,
    )
    .expect_err("strict NoMBC validation should reject mismatched RAM codes");
    assert!(matches!(
        no_mbc_ram_code_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("expects RAM size code 0x00")
    ));

    diagnostics.clear();
    let mut no_mbc_ram_decode_mismatch = no_mbc_base_header.clone();
    no_mbc_ram_decode_mismatch.ram_size = RamSizeInfo {
        raw_code: 0x00,
        decoded_bytes: Some(8 * 1024),
        bank_count: Some(1),
    };
    let no_mbc_ram_decode_error = validate_no_mbc(
        &no_mbc_ram_decode_mismatch,
        NO_MBC_SUPPORTED_ROM_BYTES,
        &strict,
        &CartridgeClassification::classify(0x00),
        &mut diagnostics,
    )
    .expect_err("strict NoMBC validation should reject decoded RAM mismatches");
    assert!(matches!(
        no_mbc_ram_decode_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("resolved to an unsupported RAM configuration")
    ));

    diagnostics.clear();
    let mut no_mbc_rom_code_mismatch = no_mbc_base_header.clone();
    no_mbc_rom_code_mismatch.rom_size = RomSizeInfo {
        raw_code: 0x01,
        decoded_bytes: Some(64 * 1024),
        bank_count: Some(4),
    };
    let no_mbc_rom_code_error = validate_no_mbc(
        &no_mbc_rom_code_mismatch,
        NO_MBC_SUPPORTED_ROM_BYTES,
        &strict,
        &CartridgeClassification::classify(0x00),
        &mut diagnostics,
    )
    .expect_err("strict NoMBC validation should reject ROM size code mismatches");
    assert!(matches!(
        no_mbc_rom_code_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("expects ROM size code 0x00")
    ));

    diagnostics.clear();
    let mut no_mbc_rom_decode_mismatch = no_mbc_base_header.clone();
    no_mbc_rom_decode_mismatch.rom_size = RomSizeInfo {
        raw_code: 0x00,
        decoded_bytes: Some(64 * 1024),
        bank_count: Some(4),
    };
    let no_mbc_rom_decode_error = validate_no_mbc(
        &no_mbc_rom_decode_mismatch,
        NO_MBC_SUPPORTED_ROM_BYTES,
        &strict,
        &CartridgeClassification::classify(0x00),
        &mut diagnostics,
    )
    .expect_err("strict NoMBC validation should reject decoded ROM mismatches");
    assert!(matches!(
        no_mbc_rom_decode_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("expects a 32 KiB ROM declaration")
    ));

    diagnostics.clear();
    let mbc3_header = CartridgeHeader::parse(&build_banked_mbc3_rom(0x13, 0x03, 0x02))
        .expect("header should parse");
    let mbc3_size_error = validate_mbc3(
        &mbc3_header,
        128 * 1024,
        &strict,
        &CartridgeClassification::classify(0x13),
        &mut diagnostics,
    )
    .expect_err("strict MBC3 validation should reject image length mismatches");
    assert!(matches!(
        mbc3_size_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("loaded ROM is 131072 bytes")
    ));

    diagnostics.clear();
    let mut oversized_mbc3_header = mbc3_header.clone();
    let oversized_rom_len = MBC3_SUPPORTED_ROM_BYTES_MAX + 0x4000;
    oversized_mbc3_header.rom_size = RomSizeInfo {
        raw_code: 0x80,
        decoded_bytes: Some(oversized_rom_len),
        bank_count: Some(oversized_rom_len / 0x4000),
    };
    let oversized_mbc3_error = validate_mbc3(
        &oversized_mbc3_header,
        oversized_rom_len,
        &strict,
        &CartridgeClassification::classify(0x13),
        &mut diagnostics,
    )
    .expect_err("strict MBC3 validation should reject oversized ROM declarations");
    assert!(matches!(
        oversized_mbc3_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("exceeds the current MBC3 ROM limit")
    ));
}

#[test]
fn size_decoders_cover_extended_and_unknown_header_codes() {
    assert_eq!(
        RomSizeInfo::decode(0x52),
        RomSizeInfo {
            raw_code: 0x52,
            decoded_bytes: Some(72 * 16 * 1024),
            bank_count: Some(72),
        }
    );
    assert_eq!(
        RomSizeInfo::decode(0x53),
        RomSizeInfo {
            raw_code: 0x53,
            decoded_bytes: Some(80 * 16 * 1024),
            bank_count: Some(80),
        }
    );
    assert_eq!(
        RomSizeInfo::decode(0x54),
        RomSizeInfo {
            raw_code: 0x54,
            decoded_bytes: Some(96 * 16 * 1024),
            bank_count: Some(96),
        }
    );
    assert_eq!(
        RomSizeInfo::decode(0xFF),
        RomSizeInfo {
            raw_code: 0xFF,
            decoded_bytes: None,
            bank_count: None,
        }
    );

    assert_eq!(
        RamSizeInfo::decode(0x01),
        RamSizeInfo {
            raw_code: 0x01,
            decoded_bytes: Some(2 * 1024),
            bank_count: Some(1),
        }
    );
    assert_eq!(
        RamSizeInfo::decode(0x04),
        RamSizeInfo {
            raw_code: 0x04,
            decoded_bytes: Some(128 * 1024),
            bank_count: Some(16),
        }
    );
    assert_eq!(
        RamSizeInfo::decode(0x05),
        RamSizeInfo {
            raw_code: 0x05,
            decoded_bytes: Some(64 * 1024),
            bank_count: Some(8),
        }
    );
    assert_eq!(
        RamSizeInfo::decode(0xFF),
        RamSizeInfo {
            raw_code: 0xFF,
            decoded_bytes: None,
            bank_count: None,
        }
    );
}

#[test]
fn header_parser_rejects_small_images_and_keeps_full_titles_without_terminators() {
    let error = CartridgeHeader::parse(&vec![0x00; HEADER_MINIMUM_ROM_LEN - 1])
        .expect_err("undersized images must be rejected");
    assert_eq!(
        error,
        CartridgeHeaderParseError::ImageTooSmall {
            actual_size: HEADER_MINIMUM_ROM_LEN - 1,
            minimum_size: HEADER_MINIMUM_ROM_LEN,
        }
    );

    let mut rom = build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x09, 0x00, 0x02);
    rom[TITLE_START..=TITLE_END_INCLUSIVE].copy_from_slice(b"FULLTITLE1234567");

    let header = CartridgeHeader::parse(&rom).expect("header should parse");
    assert_eq!(header.title, "FULLTITLE123456");
}
