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
    let quiet_signature_policy = CompatibilityPolicy {
        execution_mode: ExecutionMode::Permissive,
        validation_policy: ValidationPolicy::Warn,
        heuristic_policy: HeuristicPolicy::Disabled,
        override_policy: OverridePolicy::default(),
        diagnostic_policy: DiagnosticPolicy::Quiet,
    };
    let classification =
        classify_loaded_cartridge(&mbc1m_header, &mbc1m_rom, &quiet_signature_policy);

    diagnostics.clear();
    let layout = validate_mbc1(
        &mbc1m_header,
        mbc1m_rom.len(),
        &quiet_signature_policy,
        &classification,
        &mut diagnostics,
    )
    .expect("quiet policy should admit MBC1M without adding diagnostics");

    assert_eq!(classification.detected_name(), "MBC1M");
    assert_eq!(
        layout,
        Mbc1Layout {
            wiring: Mbc1Wiring::LargeRom,
            variant: Mbc1Variant::Mbc1M,
            ram_len: 0,
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
    let mut mbc1m_with_ram_rom = build_banked_mbc1_rom_with_type(0x03, 0x05, 0x02);
    mark_mbc1_multicart_subheaders_in_banks(&mut mbc1m_with_ram_rom, &[0x10, 0x20]);
    let mbc1m_with_ram_header =
        CartridgeHeader::parse(&mbc1m_with_ram_rom).expect("header should parse");
    let mbc1m_with_ram_classification = supported(0x03, "MBC1M", SupportedCartridgeFamily::Mbc1);
    let mbc1m_with_ram_layout = validate_mbc1(
        &mbc1m_with_ram_header,
        mbc1m_with_ram_rom.len(),
        &experimental,
        &mbc1m_with_ram_classification,
        &mut diagnostics,
    )
    .expect("MBC1M with fixed 8 KiB RAM should validate");
    assert_eq!(
        mbc1m_with_ram_layout,
        Mbc1Layout {
            wiring: Mbc1Wiring::LargeRom,
            variant: Mbc1Variant::Mbc1M,
            ram_len: 8 * 1024,
        }
    );

    diagnostics.clear();
    let mbc1_no_ram_with_sram_header =
        CartridgeHeader::parse(&build_banked_mbc1_rom_with_type(0x01, 0x02, 0x02))
            .expect("header should parse");
    let mbc1_no_ram_layout = validate_mbc1(
        &mbc1_no_ram_with_sram_header,
        128 * 1024,
        &warn_policy(),
        &CartridgeClassification::classify(0x01),
        &mut diagnostics,
    )
    .expect("warn policy should keep MBC1 no-RAM loads deterministic");
    assert_eq!(
        mbc1_no_ram_layout,
        Mbc1Layout {
            wiring: Mbc1Wiring::Standard,
            variant: Mbc1Variant::Standard,
            ram_len: 0,
        }
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("contradicts the current MBC1 without RAM Standard wiring baseline")
    }));

    diagnostics.clear();
    let mbc1_large_rom_missing_ram_header =
        CartridgeHeader::parse(&build_banked_mbc1_rom_with_type(0x03, 0x05, 0x00))
            .expect("header should parse");
    let mbc1_large_rom_layout = validate_mbc1(
        &mbc1_large_rom_missing_ram_header,
        1024 * 1024,
        &warn_policy(),
        &CartridgeClassification::classify(0x03),
        &mut diagnostics,
    )
    .expect("warn policy should keep large-ROM MBC1 RAM on the explicit fixed-window baseline");
    assert_eq!(
        mbc1_large_rom_layout,
        Mbc1Layout {
            wiring: Mbc1Wiring::LargeRom,
            variant: Mbc1Variant::Standard,
            ram_len: MBC1_LARGE_ROM_RAM_BYTES,
        }
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("contradicts the current MBC1+RAM LargeRom wiring baseline")
    }));

    diagnostics.clear();
    let mbc30_header = CartridgeHeader::parse(&build_test_rom(256 * 1024, 0x13, 0x03, 0x05))
        .expect("header should parse");
    let mbc30_variant = validate_mbc3(
        &mbc30_header,
        256 * 1024,
        &strict,
        &supported(0x13, "MBC30", SupportedCartridgeFamily::Mbc3),
        &mut diagnostics,
    )
    .expect("MBC30 should validate as an explicit MBC3-family variant");
    assert_eq!(mbc30_variant, Mbc3Variant::Mbc30);

    diagnostics.clear();
    let no_ram_mbc3_with_mbc30_code_header =
        CartridgeHeader::parse(&build_test_rom(256 * 1024, 0x11, 0x03, 0x05))
            .expect("header should parse");
    let no_ram_mbc3_with_mbc30_code_error = validate_mbc3(
        &no_ram_mbc3_with_mbc30_code_header,
        256 * 1024,
        &strict,
        &CartridgeClassification::classify(0x11),
        &mut diagnostics,
    )
    .expect_err("RAM-less MBC3 should reject 64 KiB code as a header contradiction");
    assert!(matches!(
        no_ram_mbc3_with_mbc30_code_error,
        CartridgeLoadError::Rejected { reason, .. }
            if reason.contains("does not provide external RAM")
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

    diagnostics.clear();
    let mbc5_rumble_64k_ram_header =
        CartridgeHeader::parse(&build_test_rom(256 * 1024, 0x1D, 0x03, 0x05))
            .expect("header should parse");
    let mbc5_rumble_variant = validate_mbc5(
        &mbc5_rumble_64k_ram_header,
        256 * 1024,
        &strict,
        &CartridgeClassification::classify(0x1D),
        &mut diagnostics,
    )
    .expect("rumble-capable MBC5 should accept 64 KiB SRAM");
    assert_eq!(mbc5_rumble_variant, Mbc5Variant::RumbleRam);
    assert!(diagnostics.is_empty());
}
