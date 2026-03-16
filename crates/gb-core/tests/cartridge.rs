use gb_core::{
    BootRomBusState, Bus, BusAccessKind, BusArbitrationState, BusRequester,
    CartridgeClassification, CartridgeHeader, CartridgeLoadError, CartridgeSelection,
    CartridgeSlot, CartridgeSlotState, CompatibilityPolicy, ConsoleModel, ExecutionMode, Machine,
    MachineConfig, SupportedCartridgeFamily, UnsupportedCartridgeCategory,
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
fn loading_known_but_unimplemented_mbc_family_fails_explicitly() {
    let rom = build_test_rom(128 * 1024, 0x01, 0x02, 0x00);
    let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
        .expect_err("MBC1 should stay reserved for a later phase");

    match error {
        CartridgeLoadError::Rejected {
            classification,
            execution_mode,
            reason,
            ..
        } => {
            assert_eq!(execution_mode, ExecutionMode::Strict);
            assert_eq!(
                classification.selection(),
                CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc1)
            );
            assert!(reason.contains("reserved for a later phase"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
