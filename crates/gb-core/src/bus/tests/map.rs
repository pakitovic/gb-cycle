use super::*;

#[test]
fn decode_address_covers_each_dmg_region_boundary() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let cases = [
        (
            0x0000,
            BusRegion::CartridgeRomBank0,
            BusDomain::Cartridge,
            BusRegionOwner::Cartridge,
            0x0000,
        ),
        (
            0x3FFF,
            BusRegion::CartridgeRomBank0,
            BusDomain::Cartridge,
            BusRegionOwner::Cartridge,
            0x3FFF,
        ),
        (
            0x4000,
            BusRegion::CartridgeRomBankN,
            BusDomain::Cartridge,
            BusRegionOwner::Cartridge,
            0x0000,
        ),
        (
            0x7FFF,
            BusRegion::CartridgeRomBankN,
            BusDomain::Cartridge,
            BusRegionOwner::Cartridge,
            0x3FFF,
        ),
        (
            0x8000,
            BusRegion::Vram,
            BusDomain::Vram,
            BusRegionOwner::Ppu,
            0x0000,
        ),
        (
            0x9FFF,
            BusRegion::Vram,
            BusDomain::Vram,
            BusRegionOwner::Ppu,
            0x1FFF,
        ),
        (
            0xA000,
            BusRegion::CartridgeExternal,
            BusDomain::Cartridge,
            BusRegionOwner::Cartridge,
            0x0000,
        ),
        (
            0xBFFF,
            BusRegion::CartridgeExternal,
            BusDomain::Cartridge,
            BusRegionOwner::Cartridge,
            0x1FFF,
        ),
        (
            0xC000,
            BusRegion::WramBank0,
            BusDomain::Wram,
            BusRegionOwner::Bus,
            0x0000,
        ),
        (
            0xCFFF,
            BusRegion::WramBank0,
            BusDomain::Wram,
            BusRegionOwner::Bus,
            0x0FFF,
        ),
        (
            0xD000,
            BusRegion::WramBankN,
            BusDomain::Wram,
            BusRegionOwner::Bus,
            0x0000,
        ),
        (
            0xDFFF,
            BusRegion::WramBankN,
            BusDomain::Wram,
            BusRegionOwner::Bus,
            0x0FFF,
        ),
        (
            0xE000,
            BusRegion::EchoRam,
            BusDomain::Wram,
            BusRegionOwner::Bus,
            0x0000,
        ),
        (
            0xFDFF,
            BusRegion::EchoRam,
            BusDomain::Wram,
            BusRegionOwner::Bus,
            0x1DFF,
        ),
        (
            0xFE00,
            BusRegion::Oam,
            BusDomain::Oam,
            BusRegionOwner::Ppu,
            0x0000,
        ),
        (
            0xFE9F,
            BusRegion::Oam,
            BusDomain::Oam,
            BusRegionOwner::Ppu,
            0x009F,
        ),
        (
            0xFEA0,
            BusRegion::Unusable,
            BusDomain::Unusable,
            BusRegionOwner::Bus,
            0x0000,
        ),
        (
            0xFEFF,
            BusRegion::Unusable,
            BusDomain::Unusable,
            BusRegionOwner::Bus,
            0x005F,
        ),
        (
            0xFF00,
            BusRegion::Mmio,
            BusDomain::IoHram,
            BusRegionOwner::Mmio,
            0x0000,
        ),
        (
            0xFF7F,
            BusRegion::Mmio,
            BusDomain::IoHram,
            BusRegionOwner::Mmio,
            0x007F,
        ),
        (
            0xFF80,
            BusRegion::Hram,
            BusDomain::IoHram,
            BusRegionOwner::Bus,
            0x0000,
        ),
        (
            0xFFFE,
            BusRegion::Hram,
            BusDomain::IoHram,
            BusRegionOwner::Bus,
            0x007E,
        ),
        (
            0xFFFF,
            BusRegion::InterruptEnable,
            BusDomain::IoHram,
            BusRegionOwner::InterruptController,
            0x0000,
        ),
    ];

    for (address, region, domain, owner, region_offset) in cases {
        let decoded = bus.decode_address(address);
        assert_eq!(decoded.address(), address);
        assert_eq!(decoded.region(), region);
        assert_eq!(decoded.domain(), domain);
        assert_eq!(decoded.owner(), owner);
        assert_eq!(decoded.region_offset(), region_offset);
    }
}

#[test]
fn io_contract_table_covers_ff00_ff7f_and_ie() {
    let bus = Bus::new(ConsoleModel::Dmg);

    for address in 0xFF00..=0xFF7F {
        assert!(
            bus.describe_io_register(address).is_some(),
            "missing IO contract for {address:#06X}"
        );
    }

    let ff46 = bus.describe_io_register(0xFF46).unwrap();
    let ff47 = bus.describe_io_register(0xFF47).unwrap();
    let ff03 = bus.describe_io_register(0xFF03).unwrap();
    let ff15 = bus.describe_io_register(0xFF15).unwrap();
    let ff27 = bus.describe_io_register(0xFF27).unwrap();
    let ff4e = bus.describe_io_register(0xFF4E).unwrap();
    let ff4c = bus.describe_io_register(0xFF4C).unwrap();
    let ff51 = bus.describe_io_register(0xFF51).unwrap();
    let ff68 = bus.describe_io_register(0xFF68).unwrap();
    let ff70 = bus.describe_io_register(0xFF70).unwrap();
    let ff72 = bus.describe_io_register(0xFF72).unwrap();
    let ff73 = bus.describe_io_register(0xFF73).unwrap();
    let ff74 = bus.describe_io_register(0xFF74).unwrap();
    let ff75 = bus.describe_io_register(0xFF75).unwrap();
    let ff4d = bus.describe_io_register(0xFF4D).unwrap();
    let ff50 = bus.describe_io_register(0xFF50).unwrap();
    let ff13 = bus.describe_io_register(0xFF13).unwrap();
    let ff30 = bus.describe_io_register(0xFF30).unwrap();
    let ff44 = bus.describe_io_register(0xFF44).unwrap();
    let ie = bus.describe_io_register(0xFFFF).unwrap();

    assert_eq!(ff46.owner(), IoRegisterOwner::Dma);
    assert_eq!(ff46.kind(), IoRegisterKind::OamDma);
    assert_eq!(ff47.availability(), IoRegisterAvailability::DmgCompatible);
    assert_eq!(ff47.implementation(), IoRegisterImplementation::Implemented);
    assert_eq!(ff03.access(), IoRegisterAccess::Mixed);
    assert_eq!(ff03.implementation(), IoRegisterImplementation::Unavailable);
    assert_eq!(ff15.access(), IoRegisterAccess::Mixed);
    assert_eq!(ff27.access(), IoRegisterAccess::Mixed);
    assert_eq!(ff4e.access(), IoRegisterAccess::Mixed);
    assert_eq!(ff4c.owner(), IoRegisterOwner::CgbSystem);
    assert_eq!(ff4c.availability(), IoRegisterAvailability::CgbOnly);
    assert_eq!(ff4c.implementation(), IoRegisterImplementation::Stubbed);
    assert_eq!(ff4c.kind(), IoRegisterKind::Key0);
    assert_eq!(ff4d.kind(), IoRegisterKind::Key1);
    assert_eq!(ff51.owner(), IoRegisterOwner::Dma);
    assert_eq!(ff51.implementation(), IoRegisterImplementation::Stubbed);
    assert_eq!(ff68.owner(), IoRegisterOwner::Ppu);
    assert_eq!(ff70.owner(), IoRegisterOwner::MemoryController);
    assert_eq!(ff72.owner(), IoRegisterOwner::CgbSystem);
    assert_eq!(ff72.availability(), IoRegisterAvailability::CgbOnly);
    assert_eq!(ff72.implementation(), IoRegisterImplementation::Stubbed);
    assert_eq!(ff72.access(), IoRegisterAccess::ReadWrite);
    assert_eq!(ff72.kind(), IoRegisterKind::CgbUndocumented72);
    assert_eq!(ff73.kind(), IoRegisterKind::CgbUndocumented73);
    assert_eq!(ff74.kind(), IoRegisterKind::CgbUndocumented74);
    assert_eq!(ff74.access(), IoRegisterAccess::ReadWrite);
    assert_eq!(ff75.kind(), IoRegisterKind::CgbUndocumented75);
    assert_eq!(ff75.access(), IoRegisterAccess::Mixed);
    assert_eq!(ff13.access(), IoRegisterAccess::WriteOnly);
    assert_eq!(ff13.kind(), IoRegisterKind::Nr13);
    assert_eq!(ff30.access(), IoRegisterAccess::ReadWrite);
    assert_eq!(ff30.kind(), IoRegisterKind::WaveRam);
    assert_eq!(ff44.access(), IoRegisterAccess::ReadOnly);
    assert_eq!(ff44.kind(), IoRegisterKind::Ly);
    assert_eq!(ff50.owner(), IoRegisterOwner::Boot);
    assert_eq!(ff50.access(), IoRegisterAccess::WriteOnly);
    assert_eq!(ie.kind(), IoRegisterKind::InterruptEnable);
}

#[test]
fn dmg_cgb_only_io_fallback_reads_as_ff() {
    let bus = Bus::new(ConsoleModel::Dmg);

    assert_eq!(bus.read_io_target(0xFF4C, BusIoReadView::default()), 0xFF);
    assert_eq!(bus.read_io_target(0xFF4D, BusIoReadView::default()), 0xFF);
    assert_eq!(bus.read_io_target(0xFF70, BusIoReadView::default()), 0xFF);
}

#[test]
fn cgb_ready_stubbed_io_registers_still_read_as_ff_until_implemented() {
    let bus = Bus::new(ConsoleModel::Cgb);

    assert_eq!(bus.read_io_target(0xFF4C, BusIoReadView::default()), 0xFF);
    assert_eq!(bus.read_io_target(0xFF51, BusIoReadView::default()), 0xFF);
    assert_eq!(bus.read_io_target(0xFF68, BusIoReadView::default()), 0xFF);
    assert_eq!(bus.read_io_target(0xFF72, BusIoReadView::default()), 0xFF);
    assert_eq!(bus.read_io_target(0xFF75, BusIoReadView::default()), 0xFF);
}

#[test]
fn bus_address_and_io_metadata_accessors_keep_domain_information_explicit() {
    let address = BusAddressInfo::new(0x8000, BusRegion::Vram, 0x0012);
    let io = IoRegisterInfo::new(
        0xFF46,
        IoRegisterOwner::Dma,
        IoRegisterAvailability::Shared,
        IoRegisterAccess::WriteOnly,
        IoRegisterKind::OamDma,
    );

    assert_eq!(address.address(), 0x8000);
    assert_eq!(address.region(), BusRegion::Vram);
    assert_eq!(address.domain(), BusDomain::Vram);
    assert_eq!(address.owner(), BusRegionOwner::Ppu);
    assert_eq!(address.region_offset(), 0x0012);

    assert_eq!(io.address(), 0xFF46);
    assert_eq!(io.owner(), IoRegisterOwner::Dma);
    assert_eq!(io.availability(), IoRegisterAvailability::Shared);
    assert_eq!(io.implementation(), IoRegisterImplementation::Implemented);
    assert_eq!(io.access(), IoRegisterAccess::WriteOnly);
    assert_eq!(io.kind(), IoRegisterKind::OamDma);
}

#[test]
fn unusable_area_descriptor_is_model_aware() {
    let dmg_bus = Bus::new(ConsoleModel::Dmg);
    let cgb_bus = Bus::new(ConsoleModel::Cgb);

    let dmg = dmg_bus.describe_unusable_area(0xFEA0).unwrap();
    let cgb = cgb_bus.describe_unusable_area(0xFEA0).unwrap();

    assert_eq!(dmg.address(), 0xFEA0);
    assert_eq!(
        dmg.read_profile(),
        UnusableAreaReadProfile::DmgFamilyFixedZero
    );
    assert_eq!(dmg.write_profile(), UnusableAreaWriteProfile::Ignored);
    assert_eq!(dmg.runtime_fallback_read_value(), 0x00);
    assert!(dmg.runtime_fallback_writes_ignored());

    assert_eq!(cgb.address(), 0xFEA0);
    assert_eq!(
        cgb.read_profile(),
        UnusableAreaReadProfile::CgbRevisionDependent
    );
    assert_eq!(
        cgb.write_profile(),
        UnusableAreaWriteProfile::CgbRevisionDependentRam
    );
    assert_eq!(cgb.runtime_fallback_read_value(), 0xFF);
    assert!(cgb.runtime_fallback_writes_ignored());

    assert!(dmg_bus.describe_unusable_area(0xFE9F).is_none());
    assert!(cgb_bus.describe_unusable_area(0xFF00).is_none());
}
