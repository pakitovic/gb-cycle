use super::*;

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

    diagnostics.clear();
    let huc1_unknown_size = CartridgeHeader::parse(&build_test_rom(
        NO_MBC_SUPPORTED_ROM_BYTES,
        0xFF,
        0xFF,
        0x03,
    ))
    .expect("header should parse");
    let huc1_unknown_size_error = validate_huc1(
        &huc1_unknown_size,
        NO_MBC_SUPPORTED_ROM_BYTES,
        &strict,
        &CartridgeClassification::classify(0xFF),
        &mut diagnostics,
    )
    .expect_err("unknown HuC1 ROM size code should fail");
    assert!(matches!(
        huc1_unknown_size_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("unsupported ROM size code")
    ));

    diagnostics.clear();
    let huc1_mismatch =
        CartridgeHeader::parse(&build_banked_huc1_rom(0x03, 0x03)).expect("header should parse");
    let huc1_mismatch_error = validate_huc1(
        &huc1_mismatch,
        128 * 1024,
        &strict,
        &CartridgeClassification::classify(0xFF),
        &mut diagnostics,
    )
    .expect_err("HuC1 image-size mismatches should fail");
    assert!(matches!(
        huc1_mismatch_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("loaded ROM is 131072 bytes")
    ));

    diagnostics.clear();
    let huc1_ram_overflow =
        CartridgeHeader::parse(&build_banked_huc1_rom(0x03, 0x04)).expect("header should parse");
    let huc1_ram_overflow_error = validate_huc1(
        &huc1_ram_overflow,
        256 * 1024,
        &strict,
        &CartridgeClassification::classify(0xFF),
        &mut diagnostics,
    )
    .expect_err("HuC1 should reject RAM sizes above the documented 32 KiB ceiling");
    assert!(matches!(
        huc1_ram_overflow_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("expects cartridge RAM between 1 and")
    ));

    diagnostics.clear();
    let huc3_unknown_size = CartridgeHeader::parse(&build_test_rom(
        NO_MBC_SUPPORTED_ROM_BYTES,
        0xFE,
        0xFF,
        0x03,
    ))
    .expect("header should parse");
    let huc3_unknown_size_error = validate_huc3(
        &huc3_unknown_size,
        NO_MBC_SUPPORTED_ROM_BYTES,
        &strict,
        &CartridgeClassification::classify(0xFE),
        &mut diagnostics,
    )
    .expect_err("unknown HuC-3 ROM size code should fail");
    assert!(matches!(
        huc3_unknown_size_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("unsupported ROM size code")
    ));

    diagnostics.clear();
    let huc3_rom_overflow =
        CartridgeHeader::parse(&build_banked_huc3_rom(0x07, 0x03)).expect("header should parse");
    let huc3_rom_overflow_error = validate_huc3(
        &huc3_rom_overflow,
        4 * 1024 * 1024,
        &strict,
        &CartridgeClassification::classify(0xFE),
        &mut diagnostics,
    )
    .expect_err("HuC-3 should reject ROM sizes above the documented 2 MiB ceiling");
    assert!(matches!(
        huc3_rom_overflow_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("exceeds the current HuC-3 ROM limit")
    ));

    diagnostics.clear();
    let huc3_mismatch =
        CartridgeHeader::parse(&build_banked_huc3_rom(0x03, 0x03)).expect("header should parse");
    let huc3_mismatch_error = validate_huc3(
        &huc3_mismatch,
        128 * 1024,
        &strict,
        &CartridgeClassification::classify(0xFE),
        &mut diagnostics,
    )
    .expect_err("HuC-3 image-size mismatches should fail");
    assert!(matches!(
        huc3_mismatch_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("loaded ROM is 131072 bytes")
    ));

    diagnostics.clear();
    let huc3_ram_overflow =
        CartridgeHeader::parse(&build_banked_huc3_rom(0x03, 0x05)).expect("header should parse");
    let huc3_ram_overflow_error = validate_huc3(
        &huc3_ram_overflow,
        256 * 1024,
        &strict,
        &CartridgeClassification::classify(0xFE),
        &mut diagnostics,
    )
    .expect_err("HuC-3 should reject RAM sizes above the documented 32 KiB ceiling");
    assert!(matches!(
        huc3_ram_overflow_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("expects cartridge RAM between 1 and")
    ));

    diagnostics.clear();
    let huc3_unknown_ram = CartridgeHeader::parse(&build_test_rom(256 * 1024, 0xFE, 0x03, 0xFF))
        .expect("header should parse");
    let huc3_unknown_ram_error = validate_huc3(
        &huc3_unknown_ram,
        256 * 1024,
        &strict,
        &CartridgeClassification::classify(0xFE),
        &mut diagnostics,
    )
    .expect_err("HuC-3 should reject unsupported RAM size codes");
    assert!(matches!(
        huc3_unknown_ram_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("unsupported RAM size code")
    ));

    diagnostics.clear();
    let valid_m161_rom = build_m161_signature_rom();
    let valid_m161_header =
        CartridgeHeader::parse(&valid_m161_rom).expect("M161 header should parse");
    let valid_m161_classification =
        classify_loaded_cartridge(&valid_m161_header, &valid_m161_rom, &strict);
    validate_m161(
        &valid_m161_header,
        valid_m161_rom.len(),
        &strict,
        &valid_m161_classification,
        &mut diagnostics,
    )
    .expect("baseline M161 image should validate");
    assert!(diagnostics.is_empty());

    diagnostics.clear();
    let m161_short_error = validate_m161(
        &valid_m161_header,
        4 * M161_BANK_BYTES,
        &strict,
        &valid_m161_classification,
        &mut diagnostics,
    )
    .expect_err("M161 should reject images smaller than the documented menu+game minimum");
    assert!(matches!(
        m161_short_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("expects between")
    ));

    diagnostics.clear();
    let m161_large_error = validate_m161(
        &valid_m161_header,
        9 * M161_BANK_BYTES,
        &strict,
        &valid_m161_classification,
        &mut diagnostics,
    )
    .expect_err("M161 should reject images above the documented 8-bank ceiling");
    assert!(matches!(
        m161_large_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("expects between")
    ));

    diagnostics.clear();
    let mut invalid_m161_ram = valid_m161_rom.clone();
    invalid_m161_ram[RAM_SIZE_ADDRESS] = 0x02;
    let invalid_m161_ram_header =
        CartridgeHeader::parse(&invalid_m161_ram).expect("mutated M161 header should parse");
    let m161_ram_error = validate_m161(
        &invalid_m161_ram_header,
        valid_m161_rom.len(),
        &strict,
        &valid_m161_classification,
        &mut diagnostics,
    )
    .expect_err("M161 should reject menu headers that claim RAM");
    assert!(matches!(
        m161_ram_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("expects no external RAM")
    ));
}

#[test]
fn m161_validation_accepts_the_known_commercial_menu_header_shape() {
    let strict = CompatibilityPolicy::strict();
    let mut diagnostics = Vec::new();
    let commercial_rom = build_m161_commercial_rom();
    let header = CartridgeHeader::parse(&commercial_rom).expect("header should parse");
    let classification = classify_loaded_cartridge(&header, &commercial_rom, &strict);

    validate_m161(
        &header,
        commercial_rom.len(),
        &strict,
        &classification,
        &mut diagnostics,
    )
    .expect("commercial M161 menu header should validate");
    assert!(diagnostics.is_empty());
}
