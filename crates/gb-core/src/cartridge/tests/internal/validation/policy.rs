use super::*;

#[test]
fn validation_helpers_cover_ignore_and_quiet_policy_paths() {
    let mut diagnostics = Vec::new();
    let ignore = ignore_policy();
    let no_mbc_header = CartridgeHeader::parse(&build_test_rom(64 * 1024, 0x00, 0x01, 0x00))
        .expect("header should parse");

    validate_no_mbc(
        &no_mbc_header,
        64 * 1024,
        &ignore,
        &CartridgeClassification::classify(0x00),
        &mut diagnostics,
    )
    .expect("ignore policy should admit degradable NoMBC metadata mismatches");
    assert!(diagnostics.is_empty());

    let mut mbc1m_rom = build_banked_mbc1_rom_with_type(0x01, 0x05, 0x00);
    mark_mbc1_multicart_subheaders(&mut mbc1m_rom);
    let mbc1m_header = CartridgeHeader::parse(&mbc1m_rom).expect("header should parse");
    let quiet_experimental = CompatibilityPolicy {
        execution_mode: ExecutionMode::Permissive,
        validation_policy: ValidationPolicy::Warn,
        heuristic_policy: HeuristicPolicy::AllowExperimental,
        override_policy: OverridePolicy::default(),
        diagnostic_policy: DiagnosticPolicy::Quiet,
    };
    let classification = classify_loaded_cartridge(&mbc1m_header, &mbc1m_rom, &quiet_experimental);

    diagnostics.clear();
    let layout = validate_mbc1(
        &mbc1m_header,
        mbc1m_rom.len(),
        &quiet_experimental,
        &classification,
        &mut diagnostics,
    )
    .expect("quiet policy should admit MBC1M without adding warnings");

    assert_eq!(classification.detected_name(), "MBC1M");
    assert_eq!(
        layout,
        Mbc1Layout {
            wiring: Mbc1Wiring::LargeRom,
            variant: Mbc1Variant::Mbc1M,
        }
    );
    assert!(diagnostics.is_empty());
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
