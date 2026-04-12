use super::*;

#[test]
fn boot_overlay_changes_nominal_read_ownership_without_changing_rom_space_writes() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default().with_boot_rom(BootRomBusState::map_dmg_low_bytes());

    let read = bus.resolve_access(BusAccessKind::Read, 0x0000, &state, None);
    let write = bus.resolve_access(BusAccessKind::Write, 0x0000, &state, None);

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

    let low_window = bus.resolve_access(BusAccessKind::Read, 0x0000, &state, None);
    let upper_window = bus.resolve_access(BusAccessKind::Read, 0x0200, &state, None);
    let cartridge_gap = bus.resolve_access(BusAccessKind::Read, 0x0100, &state, None);
    let write = bus.resolve_access(BusAccessKind::Write, 0x0200, &state, None);

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

    let vram = bus.resolve_access(BusAccessKind::Read, 0x8000, &mode3_state, None);
    let oam = bus.resolve_access(BusAccessKind::Write, 0xFE00, &mode2_state, None);
    let lcd_off = bus.resolve_access(
        BusAccessKind::Read,
        0x8000,
        &BusArbitrationState::default(),
        None,
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
fn external_bus_dma_policy_blocks_all_cpu_regions_except_hram_and_ff46() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default().with_dma(DmaBusState::external_bus_blocked(Some(
        DmaMemoryRegionImpact::Oam,
    )));

    let cpu_blocked = bus.resolve_access(BusAccessKind::Read, 0xC000, &state, None);
    let cpu_vram = bus.resolve_access(BusAccessKind::Read, 0x8000, &state, None);
    let cpu_hram = bus.resolve_access(BusAccessKind::Read, 0xFF80, &state, None);
    let cpu_ff46 = bus.resolve_access(BusAccessKind::Read, 0xFF46, &state, None);

    assert_eq!(
        cpu_blocked.disposition(),
        BusAccessDisposition::BlockedRead {
            value: 0xFF,
            reason: BusBlockReason::DmaExternalBusConflict,
        }
    );
    assert_eq!(
        cpu_vram.disposition(),
        BusAccessDisposition::BlockedRead {
            value: 0xFF,
            reason: BusBlockReason::DmaExternalBusConflict,
        }
    );
    assert!(cpu_hram.disposition().is_allowed());
    assert!(cpu_ff46.disposition().is_allowed());
}

#[test]
fn external_bus_dma_policy_ignores_cpu_writes_outside_hram_and_ff46() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default().with_dma(DmaBusState::external_bus_blocked(Some(
        DmaMemoryRegionImpact::Oam,
    )));

    let blocked_wram = bus.resolve_access(BusAccessKind::Write, 0xC000, &state, None);
    let allowed_vram = bus.resolve_access(BusAccessKind::Write, 0x8000, &state, None);
    let allowed_hram = bus.resolve_access(BusAccessKind::Write, 0xFF80, &state, None);
    let allowed_ff46 = bus.resolve_access(BusAccessKind::Write, 0xFF46, &state, None);

    assert_eq!(
        blocked_wram.disposition(),
        BusAccessDisposition::IgnoredWrite {
            reason: BusBlockReason::DmaExternalBusConflict,
        }
    );
    assert_eq!(
        allowed_vram.disposition(),
        BusAccessDisposition::IgnoredWrite {
            reason: BusBlockReason::DmaExternalBusConflict,
        }
    );
    assert!(allowed_hram.disposition().is_allowed());
    assert!(allowed_ff46.disposition().is_allowed());
}

#[test]
fn video_bus_dma_policy_blocks_vram_and_oam_but_keeps_wram_accessible() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default().with_dma(DmaBusState::video_bus_blocked(Some(
        DmaMemoryRegionImpact::Oam,
    )));

    let wram = bus.resolve_access(BusAccessKind::Read, 0xC000, &state, None);
    let vram = bus.resolve_access(BusAccessKind::Read, 0x8000, &state, None);
    let oam = bus.resolve_access(BusAccessKind::Read, 0xFE00, &state, None);

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

    let vram = bus.resolve_access(BusAccessKind::Read, 0x8000, &state, None);
    let oam = bus.resolve_access(BusAccessKind::Read, 0xFE00, &state, None);

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
        BusAccessKind::Write,
        0x0000,
        &BusArbitrationState::default(),
        None,
    );
    assert_eq!(resolution.requester(), BusRequester::Cpu);
    assert_eq!(resolution.kind(), BusAccessKind::Write);
}

#[test]
fn public_bus_resolution_exposes_nominal_and_effective_targets_during_dma_redirection() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default().with_dma(
        DmaBusState::external_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
            .with_cpu_conflict_source_address(Some(0xC100)),
    );

    let resolution = bus.resolve_access(BusAccessKind::Read, 0xC200, &state, None);

    assert_eq!(resolution.requested_address(), 0xC200);
    assert_eq!(resolution.nominal_target().address(), 0xC200);
    assert_eq!(resolution.nominal_target().region(), BusRegion::WramBank0);
    assert_eq!(
        resolution.nominal_disposition(),
        BusAccessDisposition::BlockedRead {
            value: 0xFF,
            reason: BusBlockReason::DmaExternalBusConflict,
        }
    );
    assert_eq!(resolution.target().address(), 0xC100);
    assert_eq!(resolution.effective_target().address(), 0xC100);
    assert_eq!(resolution.disposition(), BusAccessDisposition::Allowed);
    assert!(resolution.is_redirected());
    assert_eq!(resolution.redirected_source_address(), Some(0xC100));
}
