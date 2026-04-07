use super::*;

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
