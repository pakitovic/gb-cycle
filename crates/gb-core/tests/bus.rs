use gb_core::{
    BootRomBusState, Bus, BusAccessDisposition, BusAccessKind, BusArbitrationState, BusBlockReason,
    BusRegion, BusRegionOwner, BusRequester, ConsoleModel, DmaBusState, DmaMemoryRegionImpact,
    PpuAccessMode, PpuBusState,
};

#[test]
fn public_bus_decode_covers_the_complete_dmg_region_map() {
    let bus = Bus::new(ConsoleModel::Dmg);
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
fn public_bus_round_trips_only_through_explicit_storage_regions() {
    let mut bus = Bus::new(ConsoleModel::Dmg);

    bus.write(0x8000, 0x11);
    bus.write(0xC000, 0x22);
    bus.write(0xDFFF, 0x33);
    bus.write(0xFE9F, 0x44);
    bus.write(0xFF80, 0x55);

    assert_eq!(bus.read(0x8000), 0x11);
    assert_eq!(bus.read(0xC000), 0x22);
    assert_eq!(bus.read(0xDFFF), 0x33);
    assert_eq!(bus.read(0xFE9F), 0x44);
    assert_eq!(bus.read(0xFF80), 0x55);
}

#[test]
fn public_bus_echo_ram_aliases_wram_in_both_directions() {
    let mut bus = Bus::new(ConsoleModel::Dmg);

    bus.write(0xC000, 0xA1);
    bus.write(0xE321, 0xB2);

    assert_eq!(bus.read(0xE000), 0xA1);
    assert_eq!(bus.read(0xC321), 0xB2);
}

#[test]
fn placeholder_regions_do_not_fall_back_to_generic_byte_storage() {
    let mut bus = Bus::new(ConsoleModel::Dmg);

    bus.write(0x0100, 0x77);
    bus.write(0xA123, 0x88);
    bus.write(0xFF40, 0x99);
    bus.write(0xFEA0, 0xAA);

    assert_eq!(bus.read(0x0100), 0xFF);
    assert_eq!(bus.read(0xA123), 0xFF);
    assert_eq!(bus.read(0xFF40), 0xFF);
    assert_eq!(bus.read(0xFEA0), 0x00);
}

#[test]
fn boot_overlay_changes_nominal_read_ownership_without_changing_rom_space_writes() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default().with_boot_rom(BootRomBusState::map_dmg_low_bytes());

    let read = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0x0000, &state);
    let write = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Write, 0x0000, &state);

    assert_eq!(read.target().region(), BusRegion::BootRom);
    assert_eq!(read.target().owner(), BusRegionOwner::Boot);
    assert!(read.disposition().is_allowed());

    assert_eq!(write.target().region(), BusRegion::CartridgeRomBank0);
    assert_eq!(write.target().owner(), BusRegionOwner::Cartridge);
    assert!(write.disposition().is_allowed());
}

#[test]
fn cgb_boot_overlay_state_exposes_both_boot_windows() {
    let bus = Bus::new(ConsoleModel::Cgb);
    let state = BusArbitrationState::default().with_boot_rom(BootRomBusState::map_cgb_windows());

    let low_window = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0x0000, &state);
    let upper_window = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0x0200, &state);
    let cartridge_gap = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0x0100, &state);
    let write = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Write, 0x0200, &state);

    assert_eq!(low_window.target().region(), BusRegion::BootRom);
    assert_eq!(upper_window.target().region(), BusRegion::BootRom);
    assert_eq!(
        cartridge_gap.target().region(),
        BusRegion::CartridgeRomBank0
    );
    assert_eq!(write.target().region(), BusRegion::CartridgeRomBank0);
}

#[test]
fn cpu_vram_and_oam_access_policy_depends_on_live_ppu_state() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let mode3_state =
        BusArbitrationState::default().with_ppu(PpuBusState::lcd_enabled(PpuAccessMode::Drawing));
    let mode2_state =
        BusArbitrationState::default().with_ppu(PpuBusState::lcd_enabled(PpuAccessMode::OamScan));

    let vram = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0x8000, &mode3_state);
    let oam = bus.resolve_access(
        BusRequester::Cpu,
        BusAccessKind::Write,
        0xFE00,
        &mode2_state,
    );
    let lcd_off = bus.resolve_access(
        BusRequester::Cpu,
        BusAccessKind::Read,
        0x8000,
        &BusArbitrationState::default(),
    );

    assert_eq!(
        vram.disposition(),
        BusAccessDisposition::BlockedRead {
            value: 0xFF,
            reason: BusBlockReason::PpuVramBlockedDuringMode3,
        }
    );
    assert_eq!(
        oam.disposition(),
        BusAccessDisposition::IgnoredWrite {
            reason: BusBlockReason::PpuOamBlockedDuringMode2,
        }
    );
    assert!(lcd_off.disposition().is_allowed());
}

#[test]
fn external_bus_dma_policy_blocks_wram_but_not_vram_or_dma_requesters() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default().with_dma(DmaBusState::external_bus_blocked(Some(
        DmaMemoryRegionImpact::Oam,
    )));

    let cpu_blocked = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0xC000, &state);
    let cpu_vram = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0x8000, &state);
    let cpu_hram = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0xFF80, &state);
    let dma_allowed = bus.resolve_access(BusRequester::Dma, BusAccessKind::Read, 0xC000, &state);

    assert_eq!(
        cpu_blocked.disposition(),
        BusAccessDisposition::BlockedRead {
            value: 0xFF,
            reason: BusBlockReason::DmaExternalBusConflict,
        }
    );
    assert!(cpu_vram.disposition().is_allowed());
    assert!(cpu_hram.disposition().is_allowed());
    assert!(dma_allowed.disposition().is_allowed());
}

#[test]
fn external_bus_dma_policy_ignores_cpu_writes_outside_hram_and_vram_but_keeps_them_writable() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default().with_dma(DmaBusState::external_bus_blocked(Some(
        DmaMemoryRegionImpact::Oam,
    )));

    let blocked_wram = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Write, 0xC000, &state);
    let allowed_vram = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Write, 0x8000, &state);
    let allowed_hram = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Write, 0xFF80, &state);

    assert_eq!(
        blocked_wram.disposition(),
        BusAccessDisposition::IgnoredWrite {
            reason: BusBlockReason::DmaExternalBusConflict,
        }
    );
    assert!(allowed_vram.disposition().is_allowed());
    assert!(allowed_hram.disposition().is_allowed());
}

#[test]
fn video_bus_dma_policy_blocks_vram_and_oam_but_keeps_wram_accessible() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default().with_dma(DmaBusState::video_bus_blocked(Some(
        DmaMemoryRegionImpact::Oam,
    )));

    let wram = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0xC000, &state);
    let vram = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0x8000, &state);
    let oam = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0xFE00, &state);

    assert!(wram.disposition().is_allowed());
    assert_eq!(
        vram.disposition(),
        BusAccessDisposition::BlockedRead {
            value: 0xFF,
            reason: BusBlockReason::DmaVideoBusConflict,
        }
    );
    assert_eq!(
        oam.disposition(),
        BusAccessDisposition::BlockedRead {
            value: 0xFF,
            reason: BusBlockReason::DmaVideoBusConflict,
        }
    );
}

#[test]
fn video_bus_dma_constraints_take_precedence_over_ppu_mode_restrictions() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default()
        .with_dma(DmaBusState::video_bus_blocked(Some(
            DmaMemoryRegionImpact::Oam,
        )))
        .with_ppu(PpuBusState::lcd_enabled(PpuAccessMode::Drawing));

    let vram = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0x8000, &state);
    let oam = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0xFE00, &state);

    assert_eq!(
        vram.disposition().blocked_reason(),
        Some(BusBlockReason::DmaVideoBusConflict)
    );
    assert_eq!(
        oam.disposition().blocked_reason(),
        Some(BusBlockReason::DmaVideoBusConflict)
    );
}

#[test]
fn public_bus_state_accessors_expose_blocked_values_and_built_resolutions() {
    let blocked = BusAccessDisposition::BlockedRead {
        value: 0xA5,
        reason: BusBlockReason::PpuOamBlockedDuringMode3,
    };
    let ignored = BusAccessDisposition::IgnoredWrite {
        reason: BusBlockReason::DmaExternalBusConflict,
    };
    let dma_state = DmaBusState::video_bus_blocked(Some(DmaMemoryRegionImpact::Vram))
        .with_cpu_conflict_source_address(Some(0x8123));
    let boot_state = BootRomBusState::map_dmg_low_bytes();
    let bus = Bus::new(ConsoleModel::Dmg);

    assert_eq!(blocked.blocked_read_value(), Some(0xA5));
    assert_eq!(ignored.blocked_read_value(), None);
    assert_eq!(dma_state.cpu_conflict_source_address(), Some(0x8123));
    assert!(boot_state.maps_dmg_low_bytes());
    assert!(!boot_state.maps_cgb_upper_window());
    assert!(BootRomBusState::map_cgb_windows().maps_cgb_upper_window());

    let resolution = bus.resolve_access(
        BusRequester::Boot,
        BusAccessKind::Write,
        0x0000,
        &BusArbitrationState::default(),
    );
    assert_eq!(resolution.requester(), BusRequester::Boot);
    assert_eq!(resolution.kind(), BusAccessKind::Write);
}

#[test]
fn unusable_area_readback_tracks_oam_blocked_periods() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let oam_blocked =
        BusArbitrationState::default().with_ppu(PpuBusState::lcd_enabled(PpuAccessMode::Drawing));

    let blocked = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0xFEA0, &oam_blocked);
    let ordinary = bus.resolve_access(
        BusRequester::Cpu,
        BusAccessKind::Read,
        0xFEA0,
        &BusArbitrationState::default(),
    );

    assert_eq!(
        blocked.disposition(),
        BusAccessDisposition::BlockedRead {
            value: 0xFF,
            reason: BusBlockReason::UnusableRegionDuringOamBlock,
        }
    );
    assert!(ordinary.disposition().is_allowed());
    assert_eq!(ordinary.target().region(), BusRegion::Unusable);
}

#[test]
fn unusable_area_readback_tracks_dma_video_bus_oam_conflicts() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let dma_video_bus_blocked = BusArbitrationState::default().with_dma(
        DmaBusState::video_bus_blocked(Some(DmaMemoryRegionImpact::Oam)),
    );

    let blocked = bus.resolve_access(
        BusRequester::Cpu,
        BusAccessKind::Read,
        0xFEA0,
        &dma_video_bus_blocked,
    );

    assert_eq!(
        blocked.disposition(),
        BusAccessDisposition::BlockedRead {
            value: 0xFF,
            reason: BusBlockReason::UnusableRegionDuringDmaVideoBusConflict,
        }
    );
    assert_eq!(blocked.target().region(), BusRegion::Unusable);
}
