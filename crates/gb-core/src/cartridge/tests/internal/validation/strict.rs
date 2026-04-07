use super::*;

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
