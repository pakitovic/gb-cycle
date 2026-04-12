use super::*;

#[test]
fn public_header_parser_exposes_typed_core_fields() {
    let rom = build_test_rom(32 * 1024, 0x09, 0x00, 0x02);
    let header = CartridgeHeader::parse(&rom).expect("header should parse");

    assert_eq!(header.entry_point, [0x31, 0xFE, 0xFF, 0xAF]);
    assert_eq!(header.title, "PHASE1C!");
    assert_eq!(header.cartridge_type, 0x09);
    assert_eq!(header.rom_size.decoded_bytes, Some(32 * 1024));
    assert_eq!(header.ram_size.decoded_bytes, Some(8 * 1024));
    assert_eq!(header.header_checksum, 0x7F);
}

#[test]
fn public_header_parser_treats_any_bit7_set_0x0143_byte_as_outside_the_visible_title() {
    let mut rom = build_test_rom(32 * 1024, 0x09, 0x00, 0x02);
    rom[TITLE_START..CGB_FLAG_ADDRESS].copy_from_slice(b"CGBTITLE1234567");
    rom[CGB_FLAG_ADDRESS] = 0xA0;

    let header = CartridgeHeader::parse(&rom).expect("header should parse");

    assert_eq!(header.title, "CGBTITLE1234567");
}

#[test]
fn public_header_parser_keeps_cgb_titles_conservative_when_manufacturer_bytes_are_ambiguous() {
    let mut rom = build_test_rom(32 * 1024, 0x09, 0x00, 0x02);
    rom[TITLE_START..MANUFACTURER_CODE_START].copy_from_slice(b"HELLOTITLE1");
    rom[MANUFACTURER_CODE_START..=MANUFACTURER_CODE_END_INCLUSIVE].copy_from_slice(b"ABCD");
    rom[NEW_LICENSEE_CODE_START..NEW_LICENSEE_CODE_START + 2].copy_from_slice(b"01");
    rom[OLD_LICENSEE_CODE_ADDRESS] = 0x33;

    let header = CartridgeHeader::parse(&rom).expect("header should parse");

    assert_eq!(header.title, "HELLOTITLE1ABCD");
}

#[test]
fn public_classification_distinguishes_supported_and_structured_unsupported_types() {
    let supported = CartridgeClassification::classify(0x1B);
    let accessory = CartridgeClassification::classify(0xFC);
    let unknown = CartridgeClassification::classify(0xAA);

    assert_eq!(
        supported.selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc5)
    );
    assert_eq!(
        accessory.selection(),
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::AccessorySpecialCase)
    );
    assert_eq!(
        unknown.selection(),
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::UnknownCode)
    );
}

#[test]
fn no_mbc_rom_reads_and_external_ram_writes_route_through_the_cartridge_device() {
    let rom = build_test_rom(32 * 1024, 0x09, 0x00, 0x02);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("NoMBC should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default();

    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x0000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x12
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x0100,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x31
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0x4000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x56
    );

    bus.write_partial_harness_with_cartridge(
        0xA000,
        0x9A,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_partial_harness_with_cartridge(
            0xA000,
            BusRequester::Cpu,
            &state,
            Some(&cartridge)
        ),
        0x9A
    );

    let boot_overlay_state =
        BusArbitrationState::default().with_boot_rom(BootRomBusState::map_dmg_low_bytes());
    let resolution = bus.resolve_access(BusAccessKind::Read, 0x0000, &boot_overlay_state, None);
    assert_eq!(resolution.target().region(), gb_core::BusRegion::BootRom);
}

#[test]
fn machine_load_cartridge_installs_the_loaded_slot() {
    let rom = build_test_rom(32 * 1024, 0x00, 0x00, 0x00);
    let mut machine = Machine::new(MachineConfig::new(ConsoleModel::Dmg));

    let diagnostics = machine
        .load_cartridge(rom)
        .expect("machine should accept a supported NoMBC image");

    assert!(diagnostics.is_empty());
    assert_eq!(machine.cartridge().state(), CartridgeSlotState::NoMbc);
    assert_eq!(machine.cartridge().read_rom(0x0100), 0x31);
}

#[test]
fn documented_special_headers_keep_explicit_categories_and_do_not_fall_back_silently() {
    let cases = [
        (
            0x0B,
            "MMM01",
            UnsupportedCartridgeCategory::DocumentedButUnsupported,
        ),
        (
            0x20,
            "MBC6",
            UnsupportedCartridgeCategory::DocumentedButUnsupported,
        ),
        (
            0x22,
            "MBC7+SENSOR+RUMBLE+RAM+BATTERY",
            UnsupportedCartridgeCategory::DocumentedButUnsupported,
        ),
        (
            0xFC,
            "POCKET CAMERA",
            UnsupportedCartridgeCategory::AccessorySpecialCase,
        ),
        (
            0xFD,
            "BANDAI TAMA5",
            UnsupportedCartridgeCategory::AccessorySpecialCase,
        ),
        (
            0xFE,
            "HuC-3",
            UnsupportedCartridgeCategory::DocumentedButUnsupported,
        ),
        (
            0xFF,
            "HuC1+RAM+BATTERY",
            UnsupportedCartridgeCategory::DocumentedButUnsupported,
        ),
        (0xAA, "UNKNOWN", UnsupportedCartridgeCategory::UnknownCode),
    ];

    for (raw_type, detected_name, category) in cases {
        let rom = build_test_rom(32 * 1024, raw_type, 0x00, 0x00);
        let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
            .expect_err("special cartridge should reject explicitly");

        let classification = match error {
            gb_core::CartridgeLoadError::Rejected { classification, .. } => classification,
            other => panic!("unexpected error: {other:?}"),
        };
        assert_eq!(classification.detected_name(), detected_name);
        assert_eq!(
            classification.selection(),
            CartridgeSelection::Unsupported(category)
        );
    }
}

#[test]
fn experimental_heuristic_policy_reclassifies_magic_signatures_while_strict_mode_keeps_them_disabled()
 {
    let mut ems_rom = build_banked_mbc5_rom(0x1B, 0x03, 0x03);
    ems_rom[TITLE_START..TITLE_START + 7].copy_from_slice(b"EMSMENU");
    ems_rom[TITLE_START + 7..TITLE_START + 16].fill(0x00);
    ems_rom[DESTINATION_CODE_ADDRESS] = 0xE1;

    let strict_ems = CartridgeSlot::load(ems_rom.clone(), &CompatibilityPolicy::strict())
        .expect("strict mode should keep the header-driven MBC5 path");
    assert_eq!(strict_ems.cartridge().state(), CartridgeSlotState::Mbc5);

    let experimental_ems = CartridgeSlot::load(ems_rom, &CompatibilityPolicy::experimental())
        .expect_err("experimental mode should expose the EMS heuristic classification");
    let ems_classification = match experimental_ems {
        gb_core::CartridgeLoadError::Rejected {
            classification,
            reason,
            ..
        } => {
            assert!(reason.contains("experimental heuristic path"));
            classification
        }
        other => panic!("unexpected error: {other:?}"),
    };
    assert_eq!(ems_classification.detected_name(), "EMS");
    assert_eq!(
        ems_classification.selection(),
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::ExperimentalHeuristic)
    );

    let bung_rom = build_test_rom(32 * 1024, 0xBE, 0x00, 0x00);
    let experimental_bung = CartridgeSlot::load(bung_rom, &CompatibilityPolicy::experimental())
        .expect_err("experimental mode should expose the Bung heuristic classification");
    let bung_classification = match experimental_bung {
        gb_core::CartridgeLoadError::Rejected { classification, .. } => classification,
        other => panic!("unexpected error: {other:?}"),
    };
    assert_eq!(bung_classification.detected_name(), "BUNG");
    assert_eq!(
        bung_classification.selection(),
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::ExperimentalHeuristic)
    );

    let mut wisdom_rom = build_test_rom(64 * 1024, 0x00, 0x00, 0x00);
    wisdom_rom[TITLE_START..TITLE_START + 6].copy_from_slice(b"WISDOM");
    wisdom_rom[TITLE_START + 6] = 0x00;
    wisdom_rom[TITLE_START + 7..TITLE_START + 11].copy_from_slice(b"TREE");
    wisdom_rom[TITLE_START + 11..TITLE_START + 16].fill(0x00);
    let strict_wisdom = CartridgeSlot::load(wisdom_rom.clone(), &CompatibilityPolicy::strict())
        .expect_err("strict mode should keep heuristics disabled for Wisdom Tree");
    let strict_wisdom_classification = match strict_wisdom {
        gb_core::CartridgeLoadError::Rejected { classification, .. } => classification,
        other => panic!("unexpected error: {other:?}"),
    };
    assert_eq!(
        strict_wisdom_classification.selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::NoMbc)
    );

    let experimental_wisdom = CartridgeSlot::load(wisdom_rom, &CompatibilityPolicy::experimental())
        .expect_err("experimental mode should expose the Wisdom Tree heuristic classification");
    let wisdom_classification = match experimental_wisdom {
        gb_core::CartridgeLoadError::Rejected { classification, .. } => classification,
        other => panic!("unexpected error: {other:?}"),
    };
    assert_eq!(wisdom_classification.detected_name(), "WISDOM TREE");
    assert_eq!(
        wisdom_classification.selection(),
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::ExperimentalHeuristic)
    );
}

#[test]
fn ignore_validation_keeps_degradable_mbc3_and_mbc5_loader_paths_warning_free() {
    let mbc3_rom = build_banked_mbc3_rom(0x11, 0x03, 0x02);
    let mbc3_report = CartridgeSlot::load(mbc3_rom, &ignore_policy())
        .expect("ignore policy should admit no-RAM MBC3 metadata mismatches");
    assert_eq!(mbc3_report.cartridge().state(), CartridgeSlotState::Mbc3);
    assert!(mbc3_report.diagnostics().is_empty());

    let mbc5_rom = build_banked_mbc5_rom(0x19, 0x03, 0x02);
    let mbc5_report = CartridgeSlot::load(mbc5_rom, &ignore_policy())
        .expect("ignore policy should admit no-RAM MBC5 metadata mismatches");
    assert_eq!(mbc5_report.cartridge().state(), CartridgeSlotState::Mbc5);
    assert!(mbc5_report.diagnostics().is_empty());
}

#[test]
fn persistence_metadata_distinguishes_none_nonpersistent_ram_and_persistent_rtc_shapes() {
    let no_mbc_none = CartridgeSlot::load(
        build_test_rom(32 * 1024, 0x00, 0x00, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("ROM-only NoMBC should load");
    assert_eq!(
        no_mbc_none.cartridge().persistence_metadata().profile,
        CartridgePersistenceProfile::None
    );

    let no_mbc_nonpersistent = CartridgeSlot::load(
        build_test_rom(32 * 1024, 0x08, 0x00, 0x02),
        &CompatibilityPolicy::strict(),
    )
    .expect("NoMBC+RAM should load");
    assert_eq!(
        no_mbc_nonpersistent
            .cartridge()
            .persistence_metadata()
            .profile,
        CartridgePersistenceProfile::NonPersistentRam {
            ram: CartridgeRamPayloadKind::Linear { byte_len: 8 * 1024 }
        }
    );

    let no_mbc_persistent = CartridgeSlot::load(
        build_test_rom(32 * 1024, 0x09, 0x00, 0x02),
        &CompatibilityPolicy::strict(),
    )
    .expect("NoMBC+RAM+BATTERY should load");
    assert_eq!(
        no_mbc_persistent.cartridge().persistence_metadata().profile,
        CartridgePersistenceProfile::PersistentRam {
            ram: CartridgeRamPayloadKind::Linear { byte_len: 8 * 1024 }
        }
    );

    let mbc2_persistent = CartridgeSlot::load(
        build_banked_mbc2_rom(0x06, 0x03, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC2+BATTERY should load");
    assert_eq!(
        mbc2_persistent.cartridge().persistence_metadata().profile,
        CartridgePersistenceProfile::PersistentRam {
            ram: CartridgeRamPayloadKind::Mbc2Nibbles { cell_count: 512 }
        }
    );

    let mbc3_rtc_only = CartridgeSlot::load(
        build_banked_mbc3_rom(0x0F, 0x03, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC3+TIMER+BATTERY should load");
    assert_eq!(
        mbc3_rtc_only.cartridge().persistence_metadata().profile,
        CartridgePersistenceProfile::PersistentRtc
    );

    let mbc3_ram_rtc = CartridgeSlot::load(
        build_banked_mbc3_rom(0x10, 0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC3+TIMER+RAM+BATTERY should load");
    assert_eq!(
        mbc3_ram_rtc.cartridge().persistence_metadata().profile,
        CartridgePersistenceProfile::PersistentRamAndRtc {
            ram: CartridgeRamPayloadKind::Linear {
                byte_len: 32 * 1024
            }
        }
    );

    let mbc5_nonpersistent = CartridgeSlot::load(
        build_banked_mbc5_rom(0x1A, 0x03, 0x02),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC5+RAM should load");
    assert_eq!(
        mbc5_nonpersistent
            .cartridge()
            .persistence_metadata()
            .profile,
        CartridgePersistenceProfile::NonPersistentRam {
            ram: CartridgeRamPayloadKind::Linear { byte_len: 8 * 1024 }
        }
    );
}

#[test]
fn restore_persistent_state_rejects_mismatched_payload_kinds() {
    let report = CartridgeSlot::load(
        build_banked_mbc5_rom(0x1B, 0x03, 0x04),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC5+RAM+BATTERY should load");
    let (mut cartridge, _) = report.into_parts();

    let error = cartridge
        .restore_persistent_state(&PersistentCartState::Mbc1Ram {
            ram: vec![0; 32 * 1024],
        })
        .expect_err("mismatched payload kinds should fail");
    assert_eq!(
        error,
        CartridgePersistentStateError::KindMismatch {
            expected: "Mbc5Ram",
            actual: "Mbc1Ram"
        }
    );
}
