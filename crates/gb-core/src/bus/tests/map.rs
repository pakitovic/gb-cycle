use super::*;
use crate::model::HardwareRevision;

#[test]
fn decode_address_covers_each_dmg_region_boundary() {
    let bus = Bus::new(ConsoleModel::GameBoy);
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
    let bus = Bus::new(ConsoleModel::GameBoy);

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
    assert_eq!(ff4c.implementation(), IoRegisterImplementation::Implemented);
    assert_eq!(ff4c.kind(), IoRegisterKind::Key0);
    assert_eq!(ff4d.owner(), IoRegisterOwner::CgbSystem);
    assert_eq!(ff4d.availability(), IoRegisterAvailability::CgbOnly);
    assert_eq!(ff4d.implementation(), IoRegisterImplementation::Implemented);
    assert_eq!(ff4d.kind(), IoRegisterKind::Key1);
    assert_eq!(ff51.owner(), IoRegisterOwner::Dma);
    assert_eq!(ff51.availability(), IoRegisterAvailability::CgbOnly);
    assert_eq!(ff51.implementation(), IoRegisterImplementation::Implemented);
    assert_eq!(ff51.access(), IoRegisterAccess::WriteOnly);
    assert_eq!(ff51.kind(), IoRegisterKind::Hdma1);
    assert_eq!(ff68.owner(), IoRegisterOwner::Ppu);
    assert_eq!(ff68.availability(), IoRegisterAvailability::CgbOnly);
    assert_eq!(ff68.implementation(), IoRegisterImplementation::Implemented);
    assert_eq!(ff68.kind(), IoRegisterKind::Bcps);
    assert_eq!(ff70.owner(), IoRegisterOwner::MemoryController);
    assert_eq!(ff70.implementation(), IoRegisterImplementation::Implemented);
    assert_eq!(ff72.owner(), IoRegisterOwner::CgbSystem);
    assert_eq!(ff72.availability(), IoRegisterAvailability::CgbOnly);
    assert_eq!(ff72.implementation(), IoRegisterImplementation::Implemented);
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
    let bus = Bus::new(ConsoleModel::GameBoy);

    assert_eq!(bus.read_io_target(0xFF4C, BusIoReadView::default()), 0xFF);
    assert_eq!(bus.read_io_target(0xFF4D, BusIoReadView::default()), 0xFF);
    assert_eq!(bus.read_io_target(0xFF56, BusIoReadView::default()), 0xFF);
    assert_eq!(bus.read_io_target(0xFF70, BusIoReadView::default()), 0xFF);
    assert_eq!(bus.read_io_target(0xFF76, BusIoReadView::default()), 0xFF);
    assert_eq!(bus.read_io_target(0xFF77, BusIoReadView::default()), 0xFF);
}

#[test]
fn native_cgb_bus_owned_io_registers_publish_slice3_readback() {
    let bus = Bus::new(ConsoleModel::GameBoyColor);

    assert_eq!(bus.read_io_target(0xFF4C, BusIoReadView::default()), 0xFF);
    assert_eq!(bus.read_io_target(0xFF4F, BusIoReadView::default()), 0xFE);
    assert_eq!(bus.read_io_target(0xFF70, BusIoReadView::default()), 0xF8);
    assert_eq!(bus.read_io_target(0xFF72, BusIoReadView::default()), 0x00);
    assert_eq!(bus.read_io_target(0xFF75, BusIoReadView::default()), 0x8F);
    assert_eq!(bus.read_io_target(0xFF56, BusIoReadView::default()), 0x3E);

    assert_eq!(bus.read_io_target(0xFF51, BusIoReadView::default()), 0xFF);
    assert_eq!(bus.read_io_target(0xFF55, BusIoReadView::default()), 0xFF);
    assert_eq!(bus.read_io_target(0xFF68, BusIoReadView::default()), 0xFF);
}

#[test]
fn native_cgb_hdma_registers_route_to_dma_owner() {
    let mut bus =
        Bus::new_with_operating_mode(ConsoleModel::GameBoyColor, crate::model::OperatingMode::Cgb);
    let mut dma = crate::dma::DmaController::new(ConsoleModel::GameBoyColor);

    for (address, value) in [
        (0xFF51, 0x12),
        (0xFF52, 0x3F),
        (0xFF53, 0x9A),
        (0xFF54, 0xBC),
        (0xFF55, 0x82),
    ] {
        bus.write_with_context(
            address,
            value,
            BusRequester::Cpu,
            &BusArbitrationState::default(),
            None,
            BusIoWriteView {
                dma: Some(&mut dma),
                ..BusIoWriteView::default()
            },
        );
    }

    assert_eq!(dma.vram_dma_registers().source_start(), 0x1230);
    assert_eq!(dma.vram_dma_registers().destination_start(), 0x9AB0);
    assert_eq!(
        bus.read_io_target(
            0xFF55,
            BusIoReadView {
                dma: Some(&dma),
                ..BusIoReadView::default()
            }
        ),
        0x02
    );
    assert_eq!(
        bus.read_io_target(
            0xFF51,
            BusIoReadView {
                dma: Some(&dma),
                ..BusIoReadView::default()
            }
        ),
        0xFF
    );
}

#[test]
fn native_cgb_rp_register_exposes_only_the_phase10_infrared_latches() {
    let mut bus =
        Bus::new_with_operating_mode(ConsoleModel::GameBoyColor, crate::model::OperatingMode::Cgb);

    assert_eq!(bus.read_io_target(0xFF56, BusIoReadView::default()), 0x3E);

    bus.write_with_context(
        0xFF56,
        0xC1,
        BusRequester::Cpu,
        &BusArbitrationState::default(),
        None,
        BusIoWriteView::default(),
    );
    assert_eq!(bus.read_io_target(0xFF56, BusIoReadView::default()), 0xFF);

    bus.write_with_context(
        0xFF56,
        0x40,
        BusRequester::Cpu,
        &BusArbitrationState::default(),
        None,
        BusIoWriteView::default(),
    );
    assert_eq!(bus.read_io_target(0xFF56, BusIoReadView::default()), 0x7E);
}

#[test]
fn agb_cgb_family_profile_does_not_expose_the_physical_cgb_ir_register() {
    let mut native = Bus::new_with_operating_mode(
        ConsoleModel::GameBoyAdvance,
        crate::model::OperatingMode::Cgb,
    );
    let mut dmg_ext = Bus::new_with_operating_mode(
        ConsoleModel::GameBoyAdvance,
        crate::model::OperatingMode::CgbDmgExt,
    );

    for bus in [&mut native, &mut dmg_ext] {
        assert_eq!(bus.read_io_target(0xFF56, BusIoReadView::default()), 0xFF);
        bus.write_with_context(
            0xFF56,
            0x00,
            BusRequester::Cpu,
            &BusArbitrationState::default(),
            None,
            BusIoWriteView::default(),
        );
        assert_eq!(bus.read_io_target(0xFF56, BusIoReadView::default()), 0xFF);
        assert_eq!(bus.cgb_infrared_status(), None);
    }
}

#[test]
fn cgb_infrared_status_is_available_for_native_cgb_and_dmg_ext_profiles() {
    const IR_WARMUP_T_CYCLES: usize = 19_900;

    let dmg = Bus::new(ConsoleModel::GameBoy);
    assert_eq!(dmg.cgb_infrared_status(), None);

    let cgb_compat = Bus::new_with_operating_mode(
        ConsoleModel::GameBoyColor,
        crate::model::OperatingMode::GbCompatible,
    );
    assert_eq!(cgb_compat.cgb_infrared_status(), None);

    let cgb_dmg_ext = Bus::new_with_operating_mode(
        ConsoleModel::GameBoyColor,
        crate::model::OperatingMode::CgbDmgExt,
    );
    assert!(cgb_dmg_ext.cgb_infrared_status().is_some());

    let mut bus =
        Bus::new_with_operating_mode(ConsoleModel::GameBoyColor, crate::model::OperatingMode::Cgb);
    let initial = bus
        .cgb_infrared_status()
        .expect("native CGB mode exposes RP status");
    assert_eq!(initial.rp_latch, 0x00);
    assert!(!initial.receive_ready());

    bus.write_with_context(
        0xFF56,
        0xC0,
        BusRequester::Cpu,
        &BusArbitrationState::default(),
        None,
        BusIoWriteView::default(),
    );
    for _ in 0..IR_WARMUP_T_CYCLES {
        bus.tick_cgb_infrared_t_cycle();
    }

    let ready = bus
        .cgb_infrared_status()
        .expect("native CGB mode exposes warmed RP status");
    assert_eq!(ready.rp_latch, 0xC0);
    assert!(ready.receive_ready());

    bus.set_cgb_infrared_external_input(true);
    bus.tick_cgb_infrared_t_cycle();

    let receiving = bus
        .cgb_infrared_status()
        .expect("native CGB mode exposes receiving RP status");
    assert!(receiving.effective_signal_detected);
    assert!(receiving.signal_visible_to_rp);
    assert!(!receiving.receive_ready());
}

#[test]
fn native_cgb_pcm_registers_route_to_apu_owner_as_read_only_taps() {
    let mut bus =
        Bus::new_with_operating_mode(ConsoleModel::GameBoyColor, crate::model::OperatingMode::Cgb);
    let apu = crate::apu::Apu::new(ConsoleModel::GameBoyColor);

    assert_eq!(
        bus.read_io_target(
            0xFF76,
            BusIoReadView {
                apu: Some(&apu),
                ..BusIoReadView::default()
            }
        ),
        0x00
    );
    assert_eq!(
        bus.read_io_target(
            0xFF77,
            BusIoReadView {
                apu: Some(&apu),
                ..BusIoReadView::default()
            }
        ),
        0x00
    );

    let mut apu = apu;
    bus.write_with_context(
        0xFF76,
        0xFF,
        BusRequester::Cpu,
        &BusArbitrationState::default(),
        None,
        BusIoWriteView {
            apu: Some(&mut apu),
            ..BusIoWriteView::default()
        },
    );
    assert_eq!(
        bus.read_io_target(
            0xFF76,
            BusIoReadView {
                apu: Some(&apu),
                ..BusIoReadView::default()
            }
        ),
        0x00
    );
}

#[test]
fn cgb_compatibility_mode_exposes_boot_hwio_visible_register_subset() {
    let bus = Bus::new_with_operating_mode(
        ConsoleModel::GameBoyColor,
        crate::model::OperatingMode::GbCompatible,
    );
    let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);
    ppu.apply_operating_mode_state(crate::model::OperatingMode::GbCompatible);
    let apu = crate::apu::Apu::new(ConsoleModel::GameBoyColor);

    assert_eq!(bus.read_io_target(0xFF4C, BusIoReadView::default()), 0xFF);
    assert_eq!(bus.read_io_target(0xFF4F, BusIoReadView::default()), 0xFE);
    assert_eq!(bus.read_io_target(0xFF51, BusIoReadView::default()), 0xFF);
    assert_eq!(bus.read_io_target(0xFF55, BusIoReadView::default()), 0xFF);
    assert_eq!(bus.read_io_target(0xFF56, BusIoReadView::default()), 0xFF);
    assert_eq!(bus.read_io_target(0xFF70, BusIoReadView::default()), 0xFF);
    assert_eq!(bus.read_io_target(0xFF72, BusIoReadView::default()), 0x00);
    assert_eq!(bus.read_io_target(0xFF73, BusIoReadView::default()), 0x00);
    assert_eq!(bus.read_io_target(0xFF74, BusIoReadView::default()), 0xFF);
    assert_eq!(bus.read_io_target(0xFF75, BusIoReadView::default()), 0x8F);
    assert_eq!(
        bus.read_io_target(
            0xFF68,
            BusIoReadView {
                ppu: Some(&ppu),
                ..BusIoReadView::default()
            }
        ),
        0x40
    );
    assert_eq!(
        bus.read_io_target(
            0xFF69,
            BusIoReadView {
                ppu: Some(&ppu),
                ..BusIoReadView::default()
            }
        ),
        0xFF
    );
    assert_eq!(
        bus.read_io_target(
            0xFF6A,
            BusIoReadView {
                ppu: Some(&ppu),
                ..BusIoReadView::default()
            }
        ),
        0x40
    );
    assert_eq!(
        bus.read_io_target(
            0xFF6C,
            BusIoReadView {
                ppu: Some(&ppu),
                ..BusIoReadView::default()
            }
        ),
        0xFF
    );
    assert_eq!(
        bus.read_io_target(
            0xFF76,
            BusIoReadView {
                apu: Some(&apu),
                ..BusIoReadView::default()
            }
        ),
        0x00
    );
    assert_eq!(
        bus.read_io_target(
            0xFF77,
            BusIoReadView {
                apu: Some(&apu),
                ..BusIoReadView::default()
            }
        ),
        0x00
    );
}

#[test]
fn cgb_dmg_ext_mode_exposes_direct_boot_register_subset_without_native_palette_or_hdma_data() {
    let mut bus = Bus::new_with_operating_mode(
        ConsoleModel::GameBoyColor,
        crate::model::OperatingMode::CgbDmgExt,
    );
    let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);
    ppu.apply_operating_mode_state(crate::model::OperatingMode::CgbDmgExt);
    let mut speed = crate::speed::SpeedController::new(
        ConsoleModel::GameBoyColor,
        crate::model::OperatingMode::CgbDmgExt,
    );
    let mut dma = crate::dma::DmaController::new(ConsoleModel::GameBoyColor);
    let apu = crate::apu::Apu::new(ConsoleModel::GameBoyColor);

    assert_eq!(bus.read_io_target(0xFF4C, BusIoReadView::default()), 0xFF);
    assert_eq!(
        bus.read_io_target(
            0xFF4D,
            BusIoReadView {
                speed: Some(&speed),
                ..BusIoReadView::default()
            }
        ),
        0x7E
    );
    bus.write_with_context(
        0xFF4D,
        0x01,
        BusRequester::Cpu,
        &BusArbitrationState::default(),
        None,
        BusIoWriteView {
            speed: Some(&mut speed),
            ..BusIoWriteView::default()
        },
    );
    assert_eq!(
        bus.read_io_target(
            0xFF4D,
            BusIoReadView {
                speed: Some(&speed),
                ..BusIoReadView::default()
            }
        ),
        0x7F
    );

    assert_eq!(bus.read_io_target(0xFF4F, BusIoReadView::default()), 0xFE);
    bus.write_with_context(
        0xFF4F,
        0x01,
        BusRequester::Cpu,
        &BusArbitrationState::default(),
        None,
        BusIoWriteView::default(),
    );
    assert_eq!(bus.read_io_target(0xFF4F, BusIoReadView::default()), 0xFF);

    assert_eq!(bus.read_io_target(0xFF56, BusIoReadView::default()), 0x3E);
    bus.write_with_context(
        0xFF56,
        0xC1,
        BusRequester::Cpu,
        &BusArbitrationState::default(),
        None,
        BusIoWriteView::default(),
    );
    assert_eq!(bus.read_io_target(0xFF56, BusIoReadView::default()), 0xFF);

    assert_eq!(bus.read_io_target(0xFF70, BusIoReadView::default()), 0xF8);
    bus.write_with_context(
        0xFF70,
        0x05,
        BusRequester::Cpu,
        &BusArbitrationState::default(),
        None,
        BusIoWriteView::default(),
    );
    assert_eq!(bus.read_io_target(0xFF70, BusIoReadView::default()), 0xFD);

    for (address, value, expected) in [
        (0xFF72, 0x12, 0x12),
        (0xFF73, 0x23, 0x23),
        (0xFF74, 0x34, 0x34),
        (0xFF75, 0x70, 0xFF),
    ] {
        bus.write_with_context(
            address,
            value,
            BusRequester::Cpu,
            &BusArbitrationState::default(),
            None,
            BusIoWriteView::default(),
        );
        assert_eq!(
            bus.read_io_target(address, BusIoReadView::default()),
            expected
        );
    }

    assert_eq!(
        bus.read_io_target(
            0xFF68,
            BusIoReadView {
                ppu: Some(&ppu),
                ..BusIoReadView::default()
            }
        ),
        0x40
    );
    bus.write_with_context(
        0xFF68,
        0x85,
        BusRequester::Cpu,
        &BusArbitrationState::default(),
        None,
        BusIoWriteView {
            ppu: Some(&mut ppu),
            ..BusIoWriteView::default()
        },
    );
    assert_eq!(
        bus.read_io_target(
            0xFF68,
            BusIoReadView {
                ppu: Some(&ppu),
                ..BusIoReadView::default()
            }
        ),
        0xC5
    );
    assert_eq!(
        bus.read_io_target(
            0xFF69,
            BusIoReadView {
                ppu: Some(&ppu),
                ..BusIoReadView::default()
            }
        ),
        0xFF
    );
    bus.write_with_context(
        0xFF69,
        0xA5,
        BusRequester::Cpu,
        &BusArbitrationState::default(),
        None,
        BusIoWriteView {
            ppu: Some(&mut ppu),
            ..BusIoWriteView::default()
        },
    );
    assert_eq!(
        bus.read_io_target(
            0xFF68,
            BusIoReadView {
                ppu: Some(&ppu),
                ..BusIoReadView::default()
            }
        ),
        0xC5,
        "DMG-ext BCPD writes are blocked and must not auto-increment BCPS"
    );

    assert_eq!(
        bus.read_io_target(
            0xFF6A,
            BusIoReadView {
                ppu: Some(&ppu),
                ..BusIoReadView::default()
            }
        ),
        0x40
    );
    assert_eq!(
        bus.read_io_target(
            0xFF6B,
            BusIoReadView {
                ppu: Some(&ppu),
                ..BusIoReadView::default()
            }
        ),
        0xFF
    );
    assert_eq!(
        bus.read_io_target(
            0xFF6C,
            BusIoReadView {
                ppu: Some(&ppu),
                ..BusIoReadView::default()
            }
        ),
        0xFF
    );
    bus.write_with_context(
        0xFF6C,
        0x00,
        BusRequester::Cpu,
        &BusArbitrationState::default(),
        None,
        BusIoWriteView {
            ppu: Some(&mut ppu),
            ..BusIoWriteView::default()
        },
    );
    assert_eq!(
        bus.read_io_target(
            0xFF6C,
            BusIoReadView {
                ppu: Some(&ppu),
                ..BusIoReadView::default()
            }
        ),
        0xFE
    );

    let hdma_registers_before = dma.vram_dma_registers();
    for address in 0xFF51..=0xFF55 {
        bus.write_with_context(
            address,
            0x12,
            BusRequester::Cpu,
            &BusArbitrationState::default(),
            None,
            BusIoWriteView {
                dma: Some(&mut dma),
                ..BusIoWriteView::default()
            },
        );
        assert_eq!(
            bus.read_io_target(
                address,
                BusIoReadView {
                    dma: Some(&dma),
                    ..BusIoReadView::default()
                }
            ),
            0xFF,
            "DMG-ext should block HDMA register {address:#06X}"
        );
    }
    assert_eq!(dma.vram_dma_registers(), hdma_registers_before);

    assert_eq!(
        bus.read_io_target(
            0xFF76,
            BusIoReadView {
                apu: Some(&apu),
                ..BusIoReadView::default()
            }
        ),
        0x00
    );
    assert_eq!(
        bus.read_io_target(
            0xFF77,
            BusIoReadView {
                apu: Some(&apu),
                ..BusIoReadView::default()
            }
        ),
        0x00
    );
}

#[test]
fn native_cgb_palette_registers_route_to_ppu_owner() {
    let bus =
        Bus::new_with_operating_mode(ConsoleModel::GameBoyColor, crate::model::OperatingMode::Cgb);
    let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);

    assert_eq!(
        bus.read_io_target(
            0xFF68,
            BusIoReadView {
                ppu: Some(&ppu),
                ..BusIoReadView::default()
            }
        ),
        0x40
    );
    assert_eq!(
        bus.read_io_target(
            0xFF6C,
            BusIoReadView {
                ppu: Some(&ppu),
                ..BusIoReadView::default()
            }
        ),
        0xFE
    );

    let mut bus =
        Bus::new_with_operating_mode(ConsoleModel::GameBoyColor, crate::model::OperatingMode::Cgb);
    bus.write_with_context(
        0xFF6C,
        0x01,
        BusRequester::Cpu,
        &BusArbitrationState::default(),
        None,
        BusIoWriteView {
            ppu: Some(&mut ppu),
            ..BusIoWriteView::default()
        },
    );
    assert_eq!(
        bus.read_io_target(
            0xFF6C,
            BusIoReadView {
                ppu: Some(&ppu),
                ..BusIoReadView::default()
            }
        ),
        0xFF
    );
}

#[test]
fn cgb_key0_direct_boot_state_tracks_header_policy_without_runtime_mutability() {
    let mut bus =
        Bus::new_with_operating_mode(ConsoleModel::GameBoyColor, crate::model::OperatingMode::Cgb);

    bus.apply_cgb_startup_state(crate::model::StartupMode::SkipBoot, None);
    assert_eq!(bus.iohram.key0_state().value(), 0x80);
    assert!(bus.iohram.key0_state().is_locked());

    bus.iohram.write_key0(0x04);
    assert_eq!(bus.iohram.key0_state().value(), 0x80);

    let mut cgb_compat = Bus::new_with_operating_mode(
        ConsoleModel::GameBoyColor,
        crate::model::OperatingMode::GbCompatible,
    );
    cgb_compat.apply_cgb_startup_state(crate::model::StartupMode::SkipBoot, None);
    assert_eq!(cgb_compat.iohram.key0_state().value(), 0x04);
    assert!(cgb_compat.iohram.key0_state().is_locked());

    let mut custom_boot =
        Bus::new_with_operating_mode(ConsoleModel::GameBoyColor, crate::model::OperatingMode::Cgb);
    custom_boot.apply_cgb_startup_state(crate::model::StartupMode::CustomBoot, None);
    assert_eq!(custom_boot.iohram.key0_state().value(), 0x80);
    assert!(custom_boot.iohram.key0_state().is_locked());

    let mut cgb_dmg_ext = Bus::new_with_operating_mode(
        ConsoleModel::GameBoyColor,
        crate::model::OperatingMode::CgbDmgExt,
    );
    cgb_dmg_ext.apply_cgb_startup_state(crate::model::StartupMode::SkipBoot, None);
    assert_eq!(cgb_dmg_ext.iohram.key0_state().value(), 0x88);
    assert!(cgb_dmg_ext.iohram.key0_state().is_locked());
}

#[test]
fn cgb_key0_real_boot_handoff_locks_boot_written_compatibility_mode() {
    let mut bus =
        Bus::new_with_operating_mode(ConsoleModel::GameBoyColor, crate::model::OperatingMode::Cgb);

    bus.apply_cgb_startup_state(crate::model::StartupMode::RealBoot, None);
    assert_eq!(bus.iohram.key0_state().value(), 0x00);
    assert!(!bus.iohram.key0_state().is_locked());

    bus.write_with_context(
        0xFF4C,
        0x04,
        BusRequester::Cpu,
        &BusArbitrationState::default(),
        None,
        BusIoWriteView::default(),
    );
    assert_eq!(bus.iohram.key0_state().value(), 0x04);
    assert!(!bus.iohram.key0_state().is_locked());

    assert_eq!(
        bus.lock_cgb_real_boot_key0_at_handoff(crate::model::HeuristicPolicy::Disabled),
        Some(crate::model::OperatingMode::GbCompatible)
    );
    assert_eq!(
        bus.operating_mode(),
        crate::model::OperatingMode::GbCompatible
    );
    assert_eq!(bus.iohram.key0_state().value(), 0x04);
    assert!(bus.iohram.key0_state().is_locked());

    bus.iohram.write_key0(0x80);
    assert_eq!(bus.iohram.key0_state().value(), 0x04);
}

#[test]
fn cgb_key0_real_boot_handoff_locks_boot_written_native_mode() {
    let mut bus =
        Bus::new_with_operating_mode(ConsoleModel::GameBoyColor, crate::model::OperatingMode::Cgb);

    bus.apply_cgb_startup_state(crate::model::StartupMode::RealBoot, None);
    bus.write_with_context(
        0xFF4C,
        0x80,
        BusRequester::Cpu,
        &BusArbitrationState::default(),
        None,
        BusIoWriteView::default(),
    );

    assert_eq!(
        bus.lock_cgb_real_boot_key0_at_handoff(crate::model::HeuristicPolicy::Disabled),
        Some(crate::model::OperatingMode::Cgb)
    );
    assert_eq!(bus.operating_mode(), crate::model::OperatingMode::Cgb);
    assert_eq!(bus.iohram.key0_state().value(), 0x80);
    assert!(bus.iohram.key0_state().is_locked());
}

#[test]
fn cgb_key0_real_boot_handoff_uses_experimental_gate_for_dmg_ext_bit() {
    let mut strict_bus =
        Bus::new_with_operating_mode(ConsoleModel::GameBoyColor, crate::model::OperatingMode::Cgb);
    strict_bus.apply_cgb_startup_state(crate::model::StartupMode::RealBoot, None);
    strict_bus.write_with_context(
        0xFF4C,
        0x08,
        BusRequester::Cpu,
        &BusArbitrationState::default(),
        None,
        BusIoWriteView::default(),
    );
    assert_eq!(
        strict_bus.lock_cgb_real_boot_key0_at_handoff(crate::model::HeuristicPolicy::Disabled),
        Some(crate::model::OperatingMode::Cgb)
    );

    let mut experimental_bus =
        Bus::new_with_operating_mode(ConsoleModel::GameBoyColor, crate::model::OperatingMode::Cgb);
    experimental_bus.apply_cgb_startup_state(crate::model::StartupMode::RealBoot, None);
    experimental_bus.write_with_context(
        0xFF4C,
        0x0C,
        BusRequester::Cpu,
        &BusArbitrationState::default(),
        None,
        BusIoWriteView::default(),
    );
    assert_eq!(
        experimental_bus
            .lock_cgb_real_boot_key0_at_handoff(crate::model::HeuristicPolicy::AllowExperimental),
        Some(crate::model::OperatingMode::CgbDmgExt)
    );
    assert_eq!(
        experimental_bus.operating_mode(),
        crate::model::OperatingMode::CgbDmgExt
    );
    assert_eq!(experimental_bus.iohram.key0_state().value(), 0x0C);
    assert!(experimental_bus.iohram.key0_state().is_locked());

    experimental_bus.iohram.write_key0(0x00);
    assert_eq!(experimental_bus.iohram.key0_state().value(), 0x0C);
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
    let dmg_bus = Bus::new(ConsoleModel::GameBoy);
    let cgb_default_bus = Bus::new(ConsoleModel::GameBoyColor);
    let cgb_e_bus = Bus::new_with_revision(ConsoleModel::GameBoyColor, HardwareRevision::CpuCgbE);
    let agb_bus = Bus::new(ConsoleModel::GameBoyAdvance);

    let dmg = dmg_bus.describe_unusable_area(0xFEA0).unwrap();
    let cgb_default = cgb_default_bus.describe_unusable_area(0xFEA0).unwrap();
    let cgb_e = cgb_e_bus.describe_unusable_area(0xFEA0).unwrap();
    let agb = agb_bus.describe_unusable_area(0xFEA0).unwrap();

    assert_eq!(dmg.address(), 0xFEA0);
    assert_eq!(
        dmg.read_profile(),
        UnusableAreaReadProfile::DmgFamilyFixedZero
    );
    assert_eq!(dmg.write_profile(), UnusableAreaWriteProfile::Ignored);
    assert_eq!(dmg.runtime_fallback_read_value(), 0x00);
    assert!(dmg.runtime_fallback_writes_ignored());

    assert_eq!(cgb_default.address(), 0xFEA0);
    assert_eq!(
        cgb_default.read_profile(),
        UnusableAreaReadProfile::CgbRevisionDependent
    );
    assert_eq!(
        cgb_default.write_profile(),
        UnusableAreaWriteProfile::CgbRevisionDependentRam
    );
    assert_eq!(cgb_default.runtime_fallback_read_value(), 0xFF);
    assert!(cgb_default.runtime_fallback_writes_ignored());
    assert_eq!(cgb_e.address(), 0xFEA0);
    assert_eq!(
        cgb_e.read_profile(),
        UnusableAreaReadProfile::CgbRevisionDependent
    );
    assert_eq!(
        cgb_e.write_profile(),
        UnusableAreaWriteProfile::CgbRevisionDependentRam
    );
    assert_eq!(cgb_e.runtime_fallback_read_value(), 0xAA);
    assert!(cgb_e.runtime_fallback_writes_ignored());

    assert_eq!(agb.address(), 0xFEA0);
    assert_eq!(
        agb.read_profile(),
        UnusableAreaReadProfile::CgbRevisionDependent
    );
    assert_eq!(
        agb.write_profile(),
        UnusableAreaWriteProfile::CgbRevisionDependentRam
    );
    assert_eq!(agb.runtime_fallback_read_value(), 0xAA);
    assert!(agb.runtime_fallback_writes_ignored());

    assert!(dmg_bus.describe_unusable_area(0xFE9F).is_none());
    assert!(cgb_default_bus.describe_unusable_area(0xFF00).is_none());
    assert!(agb_bus.describe_unusable_area(0xFF00).is_none());
}

#[test]
fn cpu_visible_ppu_mmio_read_source_applies_to_stat_and_ly() {
    let bus = Bus::new(ConsoleModel::GameBoy);
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x85,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.apply_dmg_skip_boot_stat_irq_startup_phase();
    for t_cycle in 0..36 {
        tick_ppu(&mut ppu, t_cycle);
    }

    assert_eq!(
        bus.read_io_target(
            0xFF44,
            BusIoReadView {
                ppu: Some(&ppu),
                ppu_cpu_visible_read: false,
                ..BusIoReadView::default()
            }
        ),
        0x99
    );
    assert_eq!(
        bus.read_io_target(
            0xFF44,
            BusIoReadView {
                ppu: Some(&ppu),
                ppu_cpu_visible_read: true,
                ..BusIoReadView::default()
            }
        ),
        0x00
    );
    assert_eq!(
        bus.read_io_target(
            0xFF41,
            BusIoReadView {
                ppu: Some(&ppu),
                ppu_cpu_visible_read: true,
                ..BusIoReadView::default()
            }
        ),
        0x85
    );
}
