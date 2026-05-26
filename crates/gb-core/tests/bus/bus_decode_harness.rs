use super::*;

#[test]
fn public_bus_decode_covers_the_complete_dmg_region_map() {
    let bus = Bus::new(ConsoleModel::GameBoy);
    let cases = [
        (
            0x0000,
            BusRegion::CartridgeRomBank0,
            BusRegionOwner::Cartridge,
        ),
        (
            0x4000,
            BusRegion::CartridgeRomBankN,
            BusRegionOwner::Cartridge,
        ),
        (0x8000, BusRegion::Vram, BusRegionOwner::Ppu),
        (
            0xA000,
            BusRegion::CartridgeExternal,
            BusRegionOwner::Cartridge,
        ),
        (0xC000, BusRegion::WramBank0, BusRegionOwner::Bus),
        (0xD000, BusRegion::WramBankN, BusRegionOwner::Bus),
        (0xE000, BusRegion::EchoRam, BusRegionOwner::Bus),
        (0xFE00, BusRegion::Oam, BusRegionOwner::Ppu),
        (0xFEA0, BusRegion::Unusable, BusRegionOwner::Bus),
        (0xFF00, BusRegion::Mmio, BusRegionOwner::Mmio),
        (0xFF80, BusRegion::Hram, BusRegionOwner::Bus),
        (
            0xFFFF,
            BusRegion::InterruptEnable,
            BusRegionOwner::InterruptController,
        ),
    ];

    for (address, region, owner) in cases {
        let decoded = bus.decode_address(address);
        assert_eq!(decoded.address(), address);
        assert_eq!(decoded.region(), region);
        assert_eq!(decoded.owner(), owner);
    }
}

#[test]
fn explicit_cartridgeless_bus_harness_round_trips_only_through_storage_regions() {
    let mut bus = Bus::new(ConsoleModel::GameBoy);

    write_cartridgeless_bus_harness(&mut bus, 0x8000, 0x11);
    write_cartridgeless_bus_harness(&mut bus, 0xC000, 0x22);
    write_cartridgeless_bus_harness(&mut bus, 0xDFFF, 0x33);
    write_cartridgeless_bus_harness(&mut bus, 0xFE9F, 0x44);
    write_cartridgeless_bus_harness(&mut bus, 0xFF80, 0x55);

    assert_eq!(read_cartridgeless_bus_harness(&mut bus, 0x8000), 0x11);
    assert_eq!(read_cartridgeless_bus_harness(&mut bus, 0xC000), 0x22);
    assert_eq!(read_cartridgeless_bus_harness(&mut bus, 0xDFFF), 0x33);
    assert_eq!(read_cartridgeless_bus_harness(&mut bus, 0xFE9F), 0x44);
    assert_eq!(read_cartridgeless_bus_harness(&mut bus, 0xFF80), 0x55);
}

#[test]
fn explicit_cartridgeless_bus_harness_keeps_echo_ram_aliased_to_wram() {
    let mut bus = Bus::new(ConsoleModel::GameBoy);

    write_cartridgeless_bus_harness(&mut bus, 0xC000, 0xA1);
    write_cartridgeless_bus_harness(&mut bus, 0xE321, 0xB2);

    assert_eq!(read_cartridgeless_bus_harness(&mut bus, 0xE000), 0xA1);
    assert_eq!(read_cartridgeless_bus_harness(&mut bus, 0xC321), 0xB2);
}

#[test]
fn explicit_cartridgeless_bus_harness_uses_placeholders_for_unowned_regions() {
    let mut bus = Bus::new(ConsoleModel::GameBoy);

    write_cartridgeless_bus_harness(&mut bus, 0x0100, 0x77);
    write_cartridgeless_bus_harness(&mut bus, 0xA123, 0x88);
    write_cartridgeless_bus_harness(&mut bus, 0xFF40, 0x99);
    write_cartridgeless_bus_harness(&mut bus, 0xFEA0, 0xAA);

    assert_eq!(read_cartridgeless_bus_harness(&mut bus, 0x0100), 0xFF);
    assert_eq!(read_cartridgeless_bus_harness(&mut bus, 0xA123), 0xFF);
    assert_eq!(read_cartridgeless_bus_harness(&mut bus, 0xFF40), 0xFF);
    assert_eq!(read_cartridgeless_bus_harness(&mut bus, 0xFEA0), 0x00);
}

#[test]
fn public_unusable_area_descriptor_keeps_cgb_readback_revision_dependent() {
    let dmg_bus = Bus::new(ConsoleModel::GameBoy);
    let cgb_bus = Bus::new(ConsoleModel::GameBoyColor);

    let dmg = dmg_bus.describe_unusable_area(0xFEA0).unwrap();
    let cgb = cgb_bus.describe_unusable_area(0xFEA0).unwrap();

    assert_eq!(
        dmg.read_profile(),
        UnusableAreaReadProfile::DmgFamilyFixedZero
    );
    assert_eq!(dmg.write_profile(), UnusableAreaWriteProfile::Ignored);
    assert_eq!(dmg.runtime_fallback_read_value(), 0x00);
    assert!(dmg.runtime_fallback_writes_ignored());
    assert_eq!(
        cgb.read_profile(),
        UnusableAreaReadProfile::CgbRevisionDependent
    );
    assert_eq!(
        cgb.write_profile(),
        UnusableAreaWriteProfile::CgbRevisionDependentRam
    );
    assert_eq!(cgb.runtime_fallback_read_value(), 0xAA);
    assert!(cgb.runtime_fallback_writes_ignored());
}
