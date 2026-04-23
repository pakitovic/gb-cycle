use super::*;

#[test]
fn loading_m161_signature_rom_builds_the_dedicated_mapper() {
    let report = CartridgeSlot::load(build_m161_signature_rom(), &CompatibilityPolicy::strict())
        .expect("M161 should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::M161);
    assert_eq!(
        report
            .cartridge()
            .classification()
            .expect("classification should exist")
            .selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::M161)
    );

    let Some(CartridgeDevice::M161(cartridge)) = report.cartridge().device.as_ref() else {
        panic!("expected M161 cartridge");
    };

    assert_eq!(cartridge.selected_bank, 0);
    assert!(!cartridge.bank_switch_locked);
    assert_eq!(cartridge.read_rom(0x0000), 0x00);
    assert_eq!(cartridge.read_rom(0x4000), 0x00);
}

#[test]
fn loading_m161_commercial_header_shape_still_builds_the_dedicated_mapper() {
    let report = CartridgeSlot::load(build_m161_commercial_rom(), &CompatibilityPolicy::strict())
        .expect("commercial M161 header shape should load");

    assert_eq!(report.cartridge().state(), CartridgeSlotState::M161);
    assert_eq!(
        report
            .cartridge()
            .classification()
            .expect("classification should exist")
            .selection(),
        CartridgeSelection::Supported(SupportedCartridgeFamily::M161)
    );

    let classification = report
        .cartridge()
        .classification()
        .expect("classification should exist");
    assert_eq!(classification.raw_type(), 0x10);
    assert_eq!(
        report
            .cartridge()
            .header()
            .expect("header should exist")
            .title,
        "TETRIS SET"
    );
}

#[test]
fn m161_switches_the_full_32kib_window_once_and_then_locks() {
    let report = CartridgeSlot::load(build_m161_signature_rom(), &CompatibilityPolicy::strict())
        .expect("M161 should load");
    let Some(CartridgeDevice::M161(mut cartridge)) = report.cartridge().device.clone() else {
        panic!("expected M161 cartridge");
    };

    cartridge.write_rom(0x2000, 0x03);
    assert_eq!(cartridge.read_rom(0x0000), 0x03);
    assert_eq!(cartridge.read_rom(0x4000), 0x03);
    assert!(cartridge.bank_switch_locked);

    cartridge.write_rom(0x0000, 0x01);
    assert_eq!(cartridge.read_rom(0x0000), 0x03);
    assert_eq!(cartridge.read_rom(0x4000), 0x03);
}

#[test]
fn m161_first_write_of_zero_still_locks_future_bankswitches_and_has_no_persistence() {
    let report = CartridgeSlot::load(build_m161_signature_rom(), &CompatibilityPolicy::strict())
        .expect("M161 should load");
    let (mut cartridge, _) = report.into_parts();

    cartridge.write_rom(0x7FFF, 0x00);
    cartridge.write_rom(0x0000, 0x04);

    assert_eq!(cartridge.read_rom(0x0000), 0x00);
    assert_eq!(
        cartridge.describe_external_access(0xA000),
        CartridgeExternalAccessInfo::new(
            0xA000,
            CartridgeExternalTarget::LinearRam,
            CartridgeExternalAvailability::Absent,
            CartridgeExternalReadBehavior::FallbackValue(RAM_ABSENT_READ_VALUE),
            CartridgeExternalWriteBehavior::Ignored,
        )
    );
    assert_eq!(
        cartridge.persistence_metadata(),
        CartridgePersistenceMetadata {
            has_battery: false,
            has_rtc: false,
            profile: CartridgePersistenceProfile::None,
        }
    );
    assert_eq!(cartridge.persistent_state(), PersistentCartState::None);
}

#[test]
fn m161_trace_helpers_and_restore_contract_cover_the_remaining_runtime_paths() {
    let report = CartridgeSlot::load(build_m161_signature_rom(), &CompatibilityPolicy::strict())
        .expect("M161 should load");
    let (mut cartridge, _) = report.into_parts();

    assert_eq!(
        cartridge.trace_summary(),
        "state=M161 selected_bank=0x00 bank_switch_locked=false last_bank_write=None"
    );
    assert_eq!(
        cartridge.read_ram_timed(0xA000, crate::scheduler::TCycle::new(3)),
        0xFF
    );
    cartridge.write_ram_timed(0xA000, 0x77, crate::scheduler::TCycle::new(4));
    assert_eq!(cartridge.read_ram(0xA000), 0xFF);

    cartridge.write_rom(0x1234, 0x07);
    assert_eq!(cartridge.read_rom(0x0000), 0xFF);
    assert!(cartridge.trace_summary().contains("selected_bank=0x07"));

    let restore_error = cartridge
        .restore_persistent_state(&PersistentCartState::NoMbcRam { ram: vec![0x11] })
        .expect_err("M161 must reject foreign persistence payloads");
    assert_eq!(
        restore_error,
        CartridgePersistentStateError::KindMismatch {
            expected: "None",
            actual: "NoMbcRam",
        }
    );
}
