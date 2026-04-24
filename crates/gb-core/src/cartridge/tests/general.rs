use super::*;

#[test]
fn header_parser_decodes_typed_core_fields() {
    let mut rom = build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x09, 0x00, 0x02);
    rom[DESTINATION_CODE_ADDRESS] = 0x01;
    let header = CartridgeHeader::parse(&rom).expect("header should parse");

    assert_eq!(header.entry_point, [0x00, 0xC3, 0x50, 0x01]);
    assert_eq!(header.title, "GBTEST1");
    assert_eq!(&header.title_bytes[..7], b"GBTEST1");
    assert_eq!(header.title_bytes[TITLE_BYTES_LEN - 1], 0x80);
    assert_eq!(
        header.raw_title_suffix_or_manufacturer_code,
        [0xFF; MANUFACTURER_CODE_LEN]
    );
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
    assert_eq!(header.new_licensee_code, [0xFF; NEW_LICENSEE_CODE_LEN]);
    assert_eq!(header.destination_code, 0x01);
    assert_eq!(header.old_licensee_code, 0xFF);
}

#[test]
fn classification_keeps_supported_families_and_structured_unsupported_categories_explicit() {
    let no_mbc = CartridgeClassification::classify(0x09);
    let mmm01 = CartridgeClassification::classify(0x0B);
    let mbc1 = CartridgeClassification::classify(0x03);
    let camera = CartridgeClassification::classify(0xFC);
    let unknown = CartridgeClassification::classify(0xAA);

    assert_eq!(
        no_mbc.selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::NoMbc)
    );
    assert_eq!(
        mmm01.selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::Mmm01)
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

    let no_ram_mbc3_rom = build_test_rom(256 * 1024, 0x11, 0x03, 0x05);
    let no_ram_mbc3_header = CartridgeHeader::parse(&no_ram_mbc3_rom).expect("header should parse");
    let no_ram_mbc3_classification = classify_loaded_cartridge(
        &no_ram_mbc3_header,
        &no_ram_mbc3_rom,
        &CompatibilityPolicy::strict(),
    );

    assert_eq!(no_ram_mbc3_classification.detected_name(), "MBC3");
    assert_eq!(
        no_ram_mbc3_classification.selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc3)
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
    assert_eq!(strict_mbc1m.detected_name(), "MBC1M");
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
        CartridgeSelection::Supported(SupportedCartridgeFamily::M161)
    );

    let mani_mmm01_rom = build_mani_mmm01_rom(0x04);
    let mani_mmm01_header =
        CartridgeHeader::parse_for_load(&mani_mmm01_rom).expect("header should parse");
    let mani_mmm01_classification = classify_loaded_cartridge(
        &mani_mmm01_header,
        &mani_mmm01_rom,
        &CompatibilityPolicy::strict(),
    );
    assert_eq!(mani_mmm01_classification.detected_name(), "MMM01");
    assert_eq!(
        mani_mmm01_classification.selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::Mmm01)
    );
    assert_eq!(
        mani_mmm01_classification.reason(),
        "MMM01 classification came from the explicit later Mani trailing-menu signature path"
    );

    let standard_mmm01_rom = build_mmm01_rom(0x03, 0x00, 0x0B);
    let standard_mmm01_header =
        CartridgeHeader::parse_for_load(&standard_mmm01_rom).expect("header should parse");
    let standard_mmm01_classification = classify_loaded_cartridge(
        &standard_mmm01_header,
        &standard_mmm01_rom,
        &CompatibilityPolicy::strict(),
    );
    assert_eq!(standard_mmm01_classification.detected_name(), "MMM01");
    assert_eq!(
        standard_mmm01_classification.reason(),
        "supported cartridge family"
    );
}

#[test]
fn documented_special_cartridge_loads_fail_with_explicit_typed_classification_instead_of_fallback()
{
    let cases = [
        (
            build_test_rom(256 * 1024, 0x20, 0x03, 0x00),
            "MBC6",
            UnsupportedCartridgeCategory::DocumentedButUnsupported,
            "dedicated cartridge-local implementation",
        ),
        (
            build_test_rom(256 * 1024, 0x22, 0x03, 0x00),
            "MBC7+SENSOR+RUMBLE+RAM+BATTERY",
            UnsupportedCartridgeCategory::DocumentedButUnsupported,
            "EEPROM and accelerometer",
        ),
    ];

    for (rom, expected_name, expected_category, expected_reason_snippet) in cases {
        let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
            .expect_err("documented special cartridges must not fall back to nearby mappers");

        match error {
            CartridgeLoadError::Rejected {
                classification,
                execution_mode,
                reason,
                diagnostics,
            } => {
                assert_eq!(execution_mode, ExecutionMode::Strict);
                assert_eq!(classification.detected_name(), expected_name);
                assert_eq!(
                    classification.selection(),
                    CartridgeSelection::Unsupported(expected_category)
                );
                assert!(classification.reason().contains(expected_reason_snippet));
                assert!(reason.contains(expected_name));
                assert!(reason.contains(expected_reason_snippet));
                assert!(diagnostics.is_empty());
            }
            other => panic!("expected typed rejection, got {other:?}"),
        }
    }
}

#[test]
fn huc1_header_type_loads_through_the_supported_family_instead_of_the_documented_unsupported_path()
{
    let report = CartridgeSlot::load(
        build_banked_huc1_rom(0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("HuC1 should load");

    assert_eq!(
        report
            .cartridge()
            .classification()
            .expect("classification should exist")
            .detected_name(),
        "HuC1+RAM+BATTERY"
    );
    assert_eq!(
        report
            .cartridge()
            .classification()
            .expect("classification should exist")
            .selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::Huc1)
    );
}

#[test]
fn huc3_header_type_loads_through_the_supported_family_instead_of_the_documented_unsupported_path()
{
    let report = CartridgeSlot::load(
        build_banked_huc3_rom(0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("HuC-3 should load");

    assert_eq!(
        report
            .cartridge()
            .classification()
            .expect("classification should exist")
            .detected_name(),
        "HuC-3"
    );
    assert_eq!(
        report
            .cartridge()
            .classification()
            .expect("classification should exist")
            .selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::Huc3)
    );
}

#[test]
fn empty_slot_helpers_keep_the_no_device_contract_explicit() {
    let mut cartridge = CartridgeSlot::empty();

    assert!(cartridge.is_empty());
    assert_eq!(cartridge.trace_summary(), "state=Empty");
    assert_eq!(
        cartridge.describe_external_access(0xA000),
        CartridgeExternalAccessInfo::no_device(0xA000)
    );
    assert_eq!(
        cartridge.read_ram_timed(0xA000, crate::scheduler::TCycle::new(5)),
        0xFF
    );

    cartridge.write_ram(0xA000, 0x12);
    cartridge.write_ram_timed(0xA000, 0x34, crate::scheduler::TCycle::new(7));
    cartridge.advance_rtc_seconds(1);

    assert_eq!(cartridge.rtc_access_ready_at(), None);
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
fn header_parser_rejects_small_images_and_keeps_legacy_full_titles_without_terminators() {
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
    assert_eq!(header.title, "FULLTITLE1234567");
    assert_eq!(header.title_bytes, *b"FULLTITLE1234567");
    assert_eq!(header.cgb_flag, CgbFlag::Unknown(b'7'));
}

#[test]
fn header_parser_keeps_cgb_flag_out_of_the_visible_title_when_0x0143_is_a_real_flag() {
    let mut rom = build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x09, 0x00, 0x02);
    rom[TITLE_START..CGB_FLAG_ADDRESS].copy_from_slice(b"CGBTITLE1234567");
    rom[CGB_FLAG_ADDRESS] = 0x80;

    let header = CartridgeHeader::parse(&rom).expect("header should parse");

    assert_eq!(header.title, "CGBTITLE1234567");
    assert_eq!(&header.title_bytes[..15], b"CGBTITLE1234567");
    assert_eq!(header.title_bytes[15], 0x80);
    assert_eq!(header.cgb_flag, CgbFlag::Supported);
}

#[test]
fn header_parser_keeps_bit7_set_cgb_flag_bytes_out_of_the_visible_title_even_when_non_canonical() {
    let mut rom = build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x09, 0x00, 0x02);
    rom[TITLE_START..CGB_FLAG_ADDRESS].copy_from_slice(b"CGBTITLE1234567");
    rom[CGB_FLAG_ADDRESS] = 0xA0;

    let header = CartridgeHeader::parse(&rom).expect("header should parse");

    assert_eq!(header.title, "CGBTITLE1234567");
    assert_eq!(&header.title_bytes[..15], b"CGBTITLE1234567");
    assert_eq!(header.title_bytes[15], 0xA0);
    assert_eq!(header.cgb_flag, CgbFlag::SupportedNonCanonical(0xA0));
}

#[test]
fn header_parser_keeps_cgb_title_conservative_when_manufacturer_split_is_ambiguous() {
    let mut rom = build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x09, 0x00, 0x02);
    rom[TITLE_START..MANUFACTURER_CODE_START].copy_from_slice(b"HELLOTITLE1");
    rom[MANUFACTURER_CODE_START..=MANUFACTURER_CODE_END_INCLUSIVE].copy_from_slice(b"ABCD");
    rom[CGB_FLAG_ADDRESS] = 0x80;
    rom[NEW_LICENSEE_CODE_START..NEW_LICENSEE_CODE_START + NEW_LICENSEE_CODE_LEN]
        .copy_from_slice(b"01");
    rom[OLD_LICENSEE_CODE_ADDRESS] = 0x33;

    let header = CartridgeHeader::parse(&rom).expect("header should parse");

    assert_eq!(header.title, "HELLOTITLE1ABCD");
    assert_eq!(header.raw_title_suffix_or_manufacturer_code, *b"ABCD");
    assert_eq!(header.new_licensee_code, *b"01");
    assert_eq!(header.old_licensee_code, 0x33);
}
