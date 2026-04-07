use gb_core::{
    BootRomBusState, Bus, BusAccessKind, BusArbitrationState, BusRequester,
    CartridgeClassification, CartridgeHeader, CartridgePersistenceProfile,
    CartridgePersistentStateError, CartridgeRamPayloadKind, CartridgeSelection, CartridgeSlot,
    CartridgeSlotState, CompatibilityPolicy, ConsoleModel, Machine, MachineConfig,
    Mbc3RtcPersistentState, PersistentCartState, SupportedCartridgeFamily,
    UnsupportedCartridgeCategory,
};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
const ENTRY_POINT_START: usize = 0x0100;
const LOGO_START: usize = 0x0104;
const TITLE_START: usize = 0x0134;
const CGB_FLAG_ADDRESS: usize = 0x0143;
const SGB_FLAG_ADDRESS: usize = 0x0146;
const CARTRIDGE_TYPE_ADDRESS: usize = 0x0147;
const ROM_SIZE_ADDRESS: usize = 0x0148;
const RAM_SIZE_ADDRESS: usize = 0x0149;
const DESTINATION_CODE_ADDRESS: usize = 0x014A;
const HEADER_CHECKSUM_ADDRESS: usize = 0x014D;

fn build_test_rom(len: usize, cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let mut rom = vec![0xFF; len.max(HEADER_MINIMUM_ROM_LEN)];
    rom[0x0000] = 0x12;
    rom[ENTRY_POINT_START..ENTRY_POINT_START + 4].copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
    rom[LOGO_START..LOGO_START + 48].copy_from_slice(&[0xCE; 48]);
    rom[TITLE_START..TITLE_START + 8].copy_from_slice(b"PHASE1C!");
    rom[CGB_FLAG_ADDRESS] = 0x80;
    rom[SGB_FLAG_ADDRESS] = 0x03;
    rom[CARTRIDGE_TYPE_ADDRESS] = cartridge_type;
    rom[ROM_SIZE_ADDRESS] = rom_size_code;
    rom[RAM_SIZE_ADDRESS] = ram_size_code;
    rom[HEADER_CHECKSUM_ADDRESS] = 0x7F;
    rom[0x3FFF] = 0x34;
    rom[0x4000] = 0x56;
    rom
}

fn build_banked_mbc1_rom(rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let rom_size = match rom_size_code {
        0x00 => 32 * 1024,
        0x01 => 64 * 1024,
        0x02 => 128 * 1024,
        0x03 => 256 * 1024,
        0x04 => 512 * 1024,
        0x05 => 1024 * 1024,
        0x06 => 2 * 1024 * 1024,
        _ => panic!("unsupported MBC1 ROM size code for test"),
    };
    let bank_count = rom_size / 0x4000;
    let cartridge_type = if ram_size_code == 0x00 { 0x01 } else { 0x03 };
    let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

fn build_banked_mbc2_rom(cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let rom_size = match rom_size_code {
        0x00 => 32 * 1024,
        0x01 => 64 * 1024,
        0x02 => 128 * 1024,
        0x03 => 256 * 1024,
        0x04 => 512 * 1024,
        _ => panic!("unsupported MBC2 ROM size code for test"),
    };
    let bank_count = rom_size / 0x4000;
    let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

fn build_banked_mbc3_rom(cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let rom_size = match rom_size_code {
        0x00 => 32 * 1024,
        0x01 => 64 * 1024,
        0x02 => 128 * 1024,
        0x03 => 256 * 1024,
        0x04 => 512 * 1024,
        0x05 => 1024 * 1024,
        0x06 => 2 * 1024 * 1024,
        _ => panic!("unsupported MBC3 ROM size code for test"),
    };
    let bank_count = rom_size / 0x4000;
    let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

fn build_banked_mbc5_rom(cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let rom_size = match rom_size_code {
        0x00 => 32 * 1024,
        0x01 => 64 * 1024,
        0x02 => 128 * 1024,
        0x03 => 256 * 1024,
        0x04 => 512 * 1024,
        0x05 => 1024 * 1024,
        0x06 => 2 * 1024 * 1024,
        0x07 => 4 * 1024 * 1024,
        0x08 => 8 * 1024 * 1024,
        _ => panic!("unsupported MBC5 ROM size code for test"),
    };
    let bank_count = rom_size / 0x4000;
    let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

    for bank in 0..bank_count {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 1] = ((bank >> 8) & 0x01) as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

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
        bus.read_with_cartridge(0x0000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x12
    );
    assert_eq!(
        bus.read_with_cartridge(0x0100, BusRequester::Cpu, &state, Some(&cartridge)),
        0x31
    );
    assert_eq!(
        bus.read_with_cartridge(0x4000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x56
    );

    bus.write_with_cartridge(
        0xA000,
        0x9A,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_with_cartridge(0xA000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x9A
    );

    let boot_overlay_state =
        BusArbitrationState::default().with_boot_rom(BootRomBusState::map_dmg_low_bytes());
    let resolution = bus.resolve_access(
        BusRequester::Cpu,
        BusAccessKind::Read,
        0x0000,
        &boot_overlay_state,
    );
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
fn loading_supported_mbc1_family_constructs_the_mapper_device() {
    let rom = build_banked_mbc1_rom(0x02, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc1);
}

#[test]
fn loading_32kib_mbc1_family_constructs_the_mapper_device() {
    let rom = build_banked_mbc1_rom(0x00, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc1);
    assert_eq!(report.cartridge().read_rom(0x0000), 0x00);
    assert_eq!(report.cartridge().read_rom(0x4000), 0x01);
}

#[test]
fn mbc1_rom_bank_writes_take_effect_immediately_for_later_bus_reads() {
    let rom = build_banked_mbc1_rom(0x03, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default();

    assert_eq!(
        bus.read_with_cartridge(0x4000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x01
    );

    bus.write_with_cartridge(
        0x2000,
        0x02,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_with_cartridge(0x4000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x02
    );

    bus.write_with_cartridge(
        0x2000,
        0x00,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_with_cartridge(0x4000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x01
    );
}

#[test]
fn mbc1_standard_high_window_supports_bank_0x1f_through_the_bus() {
    let rom = build_banked_mbc1_rom(0x04, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default();

    bus.write_with_cartridge(
        0x2000,
        0x1F,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    assert_eq!(
        bus.read_with_cartridge(0x4000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x1F
    );
}

#[test]
fn mbc1_small_rom_masking_can_surface_bank_zero_in_the_high_window_through_the_bus() {
    let rom = build_banked_mbc1_rom(0x01, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default();

    assert_eq!(
        bus.read_with_cartridge(0x4000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x01
    );

    bus.write_with_cartridge(
        0x2000,
        0x04,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    assert_eq!(
        bus.read_with_cartridge(0x4000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x00
    );
}

#[test]
fn mbc1_large_rom_high_window_exposes_0x21_0x41_and_0x61_not_0x20_0x40_or_0x60() {
    let rom = build_banked_mbc1_rom(0x06, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default();

    bus.write_with_cartridge(
        0x2000,
        0x00,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    bus.write_with_cartridge(
        0x4000,
        0x01,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_with_cartridge(0x4000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x21
    );

    bus.write_with_cartridge(
        0x4000,
        0x02,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_with_cartridge(0x4000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x41
    );

    bus.write_with_cartridge(
        0x4000,
        0x03,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_with_cartridge(0x4000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x61
    );
}

#[test]
fn mbc1_large_rom_mode_one_remaps_the_low_window_through_the_bus() {
    let rom = build_banked_mbc1_rom(0x06, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default();

    bus.write_with_cartridge(
        0x2000,
        0x01,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_with_cartridge(
        0x4000,
        0x02,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    assert_eq!(
        bus.read_with_cartridge(0x0000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x00
    );
    assert_eq!(
        bus.read_with_cartridge(0x4000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x41
    );

    bus.write_with_cartridge(
        0x6000,
        0x01,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    assert_eq!(
        bus.read_with_cartridge(0x0000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x40
    );
    assert_eq!(
        bus.read_with_cartridge(0x4000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x41
    );
}

#[test]
fn mbc1_ram_enable_controls_external_ram_visibility_through_the_bus() {
    let rom = build_banked_mbc1_rom(0x02, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default();

    bus.write_with_cartridge(
        0xA000,
        0x9A,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_with_cartridge(0xA000, BusRequester::Cpu, &state, Some(&cartridge)),
        0xFF
    );

    bus.write_with_cartridge(
        0x0000,
        0x0A,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_with_cartridge(
        0xA000,
        0x9A,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_with_cartridge(0xA000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x9A
    );
}

#[test]
fn mbc1_standard_ram_mode_zero_and_mode_one_select_the_expected_ram_banks() {
    let rom = build_banked_mbc1_rom(0x02, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default();

    bus.write_with_cartridge(
        0x0000,
        0x0A,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_with_cartridge(
        0x4000,
        0x02,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_with_cartridge(
        0xA000,
        0x11,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    bus.write_with_cartridge(
        0x6000,
        0x01,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_with_cartridge(
        0xA000,
        0x22,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    bus.write_with_cartridge(
        0x6000,
        0x00,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_with_cartridge(0xA000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x11
    );

    bus.write_with_cartridge(
        0x6000,
        0x01,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_with_cartridge(0xA000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x22
    );
}

#[test]
fn mbc1_large_rom_keeps_a_fixed_8kib_ram_window_through_the_bus() {
    let rom = build_banked_mbc1_rom(0x05, 0x02);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default();

    bus.write_with_cartridge(
        0x0000,
        0x0A,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_with_cartridge(
        0xA000,
        0x33,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_with_cartridge(
        0x4000,
        0x01,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_with_cartridge(
        0x6000,
        0x01,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    assert_eq!(
        bus.read_with_cartridge(0xA000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x33
    );
}

#[test]
fn loading_supported_mbc2_families_constructs_the_mapper_device() {
    for cartridge_type in [0x05, 0x06] {
        let rom = build_banked_mbc2_rom(cartridge_type, 0x03, 0x00);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC2 should load");

        assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc2);
    }
}

#[test]
fn mbc2_address_bit_8_decode_and_bank_zero_translation_are_visible_through_the_bus() {
    let rom = build_banked_mbc2_rom(0x06, 0x03, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC2 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default();

    assert_eq!(
        bus.read_with_cartridge(0x4000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x01
    );

    bus.write_with_cartridge(
        0x0000,
        0x0A,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_with_cartridge(
        0x0000,
        0x00,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_with_cartridge(0xA000, BusRequester::Cpu, &state, Some(&cartridge)),
        0xFF
    );

    bus.write_with_cartridge(
        0x0100,
        0x00,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_with_cartridge(0x4000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x01
    );

    bus.write_with_cartridge(
        0x2100,
        0x03,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_with_cartridge(0x4000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x03
    );
}

#[test]
fn mbc2_internal_nibble_ram_aliases_and_honors_the_repo_readback_policy() {
    let rom = build_banked_mbc2_rom(0x06, 0x03, 0x00);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC2 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default();

    bus.write_with_cartridge(
        0x0000,
        0x0A,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_with_cartridge(
        0xA000,
        0xAB,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    assert_eq!(
        bus.read_with_cartridge(0xA000, BusRequester::Cpu, &state, Some(&cartridge)),
        0xFB
    );
    assert_eq!(
        bus.read_with_cartridge(0xA200, BusRequester::Cpu, &state, Some(&cartridge)),
        0xFB
    );

    bus.write_with_cartridge(
        0x0000,
        0x00,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_with_cartridge(
        0xA000,
        0x0C,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_with_cartridge(0xA000, BusRequester::Cpu, &state, Some(&cartridge)),
        0xFF
    );

    bus.write_with_cartridge(
        0x0000,
        0x0A,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_with_cartridge(0xA000, BusRequester::Cpu, &state, Some(&cartridge)),
        0xFB
    );
}

#[test]
fn strict_validation_rejects_mbc2_images_above_256kib() {
    let rom = build_banked_mbc2_rom(0x05, 0x04, 0x00);
    let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect_err("oversized MBC2 should fail validation");

    let reason = match error {
        gb_core::CartridgeLoadError::Rejected { reason, .. } => reason,
        other => panic!("unexpected error: {other:?}"),
    };
    assert!(reason.contains("exceeds the current MBC2 ROM limit"));
}

#[test]
fn permissive_validation_can_warn_on_nonzero_mbc2_ram_size_metadata() {
    let rom = build_banked_mbc2_rom(0x06, 0x03, 0x02);
    let report = CartridgeSlot::load(rom, &CompatibilityPolicy::permissive())
        .expect("permissive mode should admit nonzero MBC2 RAM metadata");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc2);
    assert!(report.diagnostics().iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("expects RAM size code 0x00 because MBC2 RAM is internal")
    }));
}

#[test]
fn loading_supported_mbc3_families_constructs_the_mapper_device() {
    let cases = [
        (0x0F, 0x03, 0x00),
        (0x10, 0x03, 0x03),
        (0x11, 0x03, 0x00),
        (0x12, 0x03, 0x02),
        (0x13, 0x03, 0x03),
    ];

    for (cartridge_type, rom_size_code, ram_size_code) in cases {
        let rom = build_banked_mbc3_rom(cartridge_type, rom_size_code, ram_size_code);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");

        assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc3);
    }
}

#[test]
fn mbc3_high_window_can_reach_banks_0x20_0x40_and_0x60_through_the_bus() {
    let rom = build_banked_mbc3_rom(0x13, 0x06, 0x03);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default();

    for bank in [0x20, 0x40, 0x60] {
        bus.write_with_cartridge(
            0x2000,
            bank,
            BusRequester::Cpu,
            &state,
            Some(&mut cartridge),
        );
        assert_eq!(
            bus.read_with_cartridge(0x4000, BusRequester::Cpu, &state, Some(&cartridge)),
            bank
        );
    }
}

#[test]
fn mbc3_ram_banking_and_rtc_latch_are_visible_through_machine_access() {
    let rom = build_banked_mbc3_rom(0x10, 0x06, 0x03);
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(gb_core::StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("MBC3 test image should load");
    machine.advance_cartridge_rtc_seconds(93_784);

    machine.write_bus(0x0000, 0x0A);
    machine.write_bus(0x4000, 0x00);
    machine.write_bus(0xA000, 0x33);
    machine.write_bus(0x4000, 0x02);
    machine.write_bus(0xA000, 0x55);

    machine.write_bus(0x4000, 0x00);
    assert_eq!(machine.read_bus(0xA000), 0x33);
    machine.write_bus(0x4000, 0x02);
    assert_eq!(machine.read_bus(0xA000), 0x55);

    machine.write_bus(0x4000, 0x08);
    machine.write_bus(0x6000, 0x00);
    machine.write_bus(0x6000, 0x01);
    assert_eq!(machine.read_bus(0xA000), 0x04);

    machine.write_bus(0xA000, 0x2A);
    assert_eq!(machine.read_bus(0xA000), 0x04);

    machine.write_bus(0x6000, 0x00);
    machine.write_bus(0x6000, 0x01);
    assert_eq!(machine.read_bus(0xA000), 0x2A);
}

#[test]
fn strict_validation_rejects_mbc30_like_64kib_sram_on_mbc3() {
    let rom = build_banked_mbc3_rom(0x13, 0x06, 0x05);
    let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect_err("MBC30-like SRAM should stay reserved");

    let (classification, reason) = match error {
        gb_core::CartridgeLoadError::Rejected {
            classification,
            reason,
            ..
        } => (classification, reason),
        other => panic!("unexpected error: {other:?}"),
    };
    assert_eq!(classification.detected_name(), "MBC30");
    assert_eq!(
        classification.selection(),
        CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::PlannedVariant)
    );
    assert!(reason.contains("known reserved variant"));
}

#[test]
fn permissive_validation_can_warn_on_no_ram_mbc3_headers_with_nonzero_ram_metadata() {
    let rom = build_banked_mbc3_rom(0x11, 0x03, 0x02);
    let report = CartridgeSlot::load(rom, &CompatibilityPolicy::permissive())
        .expect("permissive mode should admit no-RAM MBC3 with warning");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc3);
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("does not provide external RAM"))
    );
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
fn loading_supported_mbc5_families_constructs_the_mapper_device() {
    let cases = [
        (0x19, 0x03, 0x00),
        (0x1A, 0x03, 0x02),
        (0x1B, 0x03, 0x03),
        (0x1C, 0x03, 0x00),
        (0x1D, 0x03, 0x02),
        (0x1E, 0x03, 0x03),
    ];

    for (cartridge_type, rom_size_code, ram_size_code) in cases {
        let rom = build_banked_mbc5_rom(cartridge_type, rom_size_code, ram_size_code);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC5 should load");

        assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc5);
    }
}

#[test]
fn mbc5_power_up_bank_one_and_the_0xff_to_0x100_boundary_are_visible_through_the_bus() {
    let rom = build_banked_mbc5_rom(0x1B, 0x08, 0x04);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC5 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default();

    assert_eq!(
        bus.read_with_cartridge(0x4000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x01
    );
    assert_eq!(
        bus.read_with_cartridge(0x4001, BusRequester::Cpu, &state, Some(&cartridge)),
        0x00
    );

    bus.write_with_cartridge(
        0x2000,
        0xFF,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_with_cartridge(
        0x3000,
        0x00,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_with_cartridge(0x4000, BusRequester::Cpu, &state, Some(&cartridge)),
        0xFF
    );
    assert_eq!(
        bus.read_with_cartridge(0x4001, BusRequester::Cpu, &state, Some(&cartridge)),
        0x00
    );

    bus.write_with_cartridge(
        0x2000,
        0x00,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_with_cartridge(
        0x3000,
        0x01,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_with_cartridge(0x4000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x00
    );
    assert_eq!(
        bus.read_with_cartridge(0x4001, BusRequester::Cpu, &state, Some(&cartridge)),
        0x01
    );

    bus.write_with_cartridge(
        0x2000,
        0xFF,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_with_cartridge(0x4000, BusRequester::Cpu, &state, Some(&cartridge)),
        0xFF
    );
    assert_eq!(
        bus.read_with_cartridge(0x4001, BusRequester::Cpu, &state, Some(&cartridge)),
        0x01
    );
}

#[test]
fn mbc5_linear_ram_banking_supports_128kib_sram_through_the_bus() {
    let rom = build_banked_mbc5_rom(0x1B, 0x03, 0x04);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC5 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default();

    assert_eq!(
        bus.read_with_cartridge(0xA000, BusRequester::Cpu, &state, Some(&cartridge)),
        0xFF
    );

    bus.write_with_cartridge(
        0xA000,
        0x22,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_with_cartridge(0xA000, BusRequester::Cpu, &state, Some(&cartridge)),
        0xFF
    );

    bus.write_with_cartridge(
        0x0000,
        0x0A,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_with_cartridge(
        0x4000,
        0x00,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_with_cartridge(
        0xA000,
        0x11,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    bus.write_with_cartridge(
        0x4000,
        0x0F,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_with_cartridge(
        0xA000,
        0xEE,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    bus.write_with_cartridge(
        0x4000,
        0x00,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_with_cartridge(0xA000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x11
    );

    bus.write_with_cartridge(
        0x4000,
        0x0F,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert_eq!(
        bus.read_with_cartridge(0xA000, BusRequester::Cpu, &state, Some(&cartridge)),
        0xEE
    );
}

#[test]
fn rumble_capable_mbc5_keeps_motor_state_distinct_from_the_effective_ram_bank() {
    let rom = build_banked_mbc5_rom(0x1E, 0x03, 0x03);
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(gb_core::StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("MBC5 rumble test image should load");

    machine.write_bus(0x0000, 0x0A);
    machine.write_bus(0x4000, 0x03);
    machine.write_bus(0xA000, 0x33);

    machine.write_bus(0x4000, 0x0B);
    assert!(machine.cartridge().rumble_on());
    assert_eq!(machine.read_bus(0xA000), 0x33);

    machine.write_bus(0x4000, 0x03);
    assert!(!machine.cartridge().rumble_on());
    assert_eq!(machine.read_bus(0xA000), 0x33);
}

#[test]
fn rumble_capable_mbc5_supports_64kib_sram_while_preserving_motor_control_through_the_bus() {
    let rom = build_banked_mbc5_rom(0x1E, 0x03, 0x05);
    let report =
        CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC5 should load");
    let (mut cartridge, _) = report.into_parts();
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default();

    bus.write_with_cartridge(
        0x0000,
        0x0A,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    bus.write_with_cartridge(
        0x4000,
        0x00,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_with_cartridge(
        0xA000,
        0x10,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    bus.write_with_cartridge(
        0x4000,
        0x07,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    bus.write_with_cartridge(
        0xA000,
        0x70,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );

    bus.write_with_cartridge(
        0x4000,
        0x00,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert!(!cartridge.rumble_on());
    assert_eq!(
        bus.read_with_cartridge(0xA000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x10
    );

    bus.write_with_cartridge(
        0x4000,
        0x07,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert!(!cartridge.rumble_on());
    assert_eq!(
        bus.read_with_cartridge(0xA000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x70
    );

    bus.write_with_cartridge(
        0x4000,
        0x0F,
        BusRequester::Cpu,
        &state,
        Some(&mut cartridge),
    );
    assert!(cartridge.rumble_on());
    assert_eq!(
        bus.read_with_cartridge(0xA000, BusRequester::Cpu, &state, Some(&cartridge)),
        0x70
    );
}

#[test]
fn strict_validation_rejects_oversized_and_invalid_128kib_rumble_mbc5_configurations() {
    let oversized = build_test_rom(16 * 1024 * 1024, 0x1B, 0x08, 0x04);
    let oversized_error = CartridgeSlot::load(oversized, &CompatibilityPolicy::strict())
        .expect_err("oversized MBC5 should fail validation");

    let oversized_reason = match oversized_error {
        gb_core::CartridgeLoadError::Rejected { reason, .. } => reason,
        other => panic!("unexpected error: {other:?}"),
    };
    assert!(oversized_reason.contains("exceeds the current MBC5 ROM limit"));

    let invalid_rumble = build_banked_mbc5_rom(0x1E, 0x03, 0x04);
    let invalid_rumble_error = CartridgeSlot::load(invalid_rumble, &CompatibilityPolicy::strict())
        .expect_err("invalid rumble MBC5 RAM size should fail validation");

    let invalid_rumble_reason = match invalid_rumble_error {
        gb_core::CartridgeLoadError::Rejected { reason, .. } => reason,
        other => panic!("unexpected error: {other:?}"),
    };
    assert!(invalid_rumble_reason.contains("rumble-capable MBC5 baseline"));

    let valid_rumble_64k = build_banked_mbc5_rom(0x1E, 0x03, 0x05);
    CartridgeSlot::load(valid_rumble_64k, &CompatibilityPolicy::strict())
        .expect("64 KiB rumble MBC5 should load");
}

#[test]
fn permissive_validation_can_warn_on_no_ram_mbc5_headers_with_nonzero_ram_metadata() {
    let rom = build_banked_mbc5_rom(0x19, 0x03, 0x02);
    let report = CartridgeSlot::load(rom, &CompatibilityPolicy::permissive())
        .expect("permissive mode should admit no-RAM MBC5 with warning");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc5);
    assert!(
        report
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("does not provide external RAM"))
    );
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
fn mbc1_persistent_state_round_trips_the_full_banked_ram_backing_store() {
    let report = CartridgeSlot::load(
        build_banked_mbc1_rom(0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC1 should load");
    let (mut cartridge, _) = report.into_parts();

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_rom(0x6000, 0x01);
    cartridge.write_rom(0x4000, 0x02);
    cartridge.write_ram(0xA000, 0x66);
    cartridge.write_rom(0x0000, 0x00);

    let state = cartridge.persistent_state();
    match &state {
        PersistentCartState::Mbc1Ram { ram } => {
            assert_eq!(ram[2 * 0x2000], 0x66);
        }
        other => panic!("unexpected persistent state: {other:?}"),
    }

    let fresh_report = CartridgeSlot::load(
        build_banked_mbc1_rom(0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("fresh MBC1 should load");
    let (mut restored, _) = fresh_report.into_parts();
    restored
        .restore_persistent_state(&state)
        .expect("MBC1 persistence should restore");

    restored.write_rom(0x0000, 0x0A);
    restored.write_rom(0x6000, 0x01);
    restored.write_rom(0x4000, 0x02);
    assert_eq!(restored.read_ram(0xA000), 0x66);
}

#[test]
fn mbc2_persistent_state_round_trips_the_nibble_array_and_rejects_invalid_values() {
    let report = CartridgeSlot::load(
        build_banked_mbc2_rom(0x06, 0x03, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("MBC2+BATTERY should load");
    let (mut cartridge, _) = report.into_parts();

    cartridge.write_rom(0x0000, 0x0A);
    cartridge.write_ram(0xA123, 0x0B);
    cartridge.write_rom(0x0000, 0x00);

    let state = cartridge.persistent_state();
    match &state {
        PersistentCartState::Mbc2Ram { ram_nibbles } => {
            assert_eq!(ram_nibbles[0x123], 0x0B);
        }
        other => panic!("unexpected persistent state: {other:?}"),
    }

    let fresh_report = CartridgeSlot::load(
        build_banked_mbc2_rom(0x06, 0x03, 0x00),
        &CompatibilityPolicy::strict(),
    )
    .expect("fresh MBC2 should load");
    let (mut restored, _) = fresh_report.into_parts();
    restored
        .restore_persistent_state(&state)
        .expect("MBC2 persistence should restore");
    restored.write_rom(0x0000, 0x0A);
    assert_eq!(restored.read_ram(0xA123), 0xFB);

    let mut invalid_nibbles = [0u8; 512];
    invalid_nibbles[3] = 0x1F;
    let error = restored
        .restore_persistent_state(&PersistentCartState::Mbc2Ram {
            ram_nibbles: invalid_nibbles,
        })
        .expect_err("high bits in MBC2 persistent nibbles should fail");
    assert_eq!(
        error,
        CartridgePersistentStateError::InvalidMbc2NibbleValue {
            index: 3,
            value: 0x1F
        }
    );
}

#[test]
fn mbc3_persistent_state_serializes_live_rtc_state_not_the_latched_snapshot() {
    let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(gb_core::StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("MBC3+TIMER+RAM+BATTERY should load");

    machine.advance_cartridge_rtc_seconds(93_784);
    machine.write_bus(0x0000, 0x0A);
    machine.write_bus(0x4000, 0x08);
    machine.write_bus(0x6000, 0x00);
    machine.write_bus(0x6000, 0x01);
    assert_eq!(machine.read_bus(0xA000), 0x04);

    machine.write_bus(0xA000, 0x2A);
    let state = machine.cartridge().persistent_state();
    match &state {
        PersistentCartState::Mbc3RamRtc { rtc, .. } => {
            assert_eq!(
                *rtc,
                Mbc3RtcPersistentState {
                    seconds: 0x2A,
                    minutes: 0x03,
                    hours: 0x02,
                    day_counter: 1,
                    halt: false,
                    carry: false,
                }
            );
        }
        other => panic!("unexpected persistent state: {other:?}"),
    }

    machine
        .restore_cartridge_persistent_state(&state)
        .expect("hot MBC3 persistence restore should succeed");
    assert_eq!(machine.read_bus(0xA000), 0x00);
    machine.write_bus(0x6000, 0x00);
    machine.write_bus(0x6000, 0x01);
    assert_eq!(machine.read_bus(0xA000), 0x2A);

    let fresh_report = CartridgeSlot::load(
        build_banked_mbc3_rom(0x10, 0x03, 0x03),
        &CompatibilityPolicy::strict(),
    )
    .expect("fresh MBC3 should load");
    let (mut restored, _) = fresh_report.into_parts();
    restored
        .restore_persistent_state(&state)
        .expect("MBC3 persistence should restore");

    restored.write_rom(0x0000, 0x0A);
    restored.write_rom(0x4000, 0x08);
    assert_eq!(restored.read_ram(0xA000), 0x00);
    restored.write_rom(0x6000, 0x00);
    restored.write_rom(0x6000, 0x01);
    assert_eq!(restored.read_ram(0xA000), 0x2A);
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
