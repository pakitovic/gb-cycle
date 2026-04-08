use gb_core::{
    BootRomBusState, Bus, BusAccessDisposition, BusAccessKind, BusArbitrationState, BusBlockReason,
    BusRegion, BusRegionOwner, BusRequester, ConsoleModel, CycleContext, DmaBusState,
    DmaMemoryRegionImpact, PpuAccessMode, PpuBusState, SchedulerPhase, TCycle,
    UnusableAreaReadProfile, UnusableAreaWriteProfile,
};

fn read_cartridgeless_bus_harness(bus: &mut Bus, address: u16) -> u8 {
    let state = BusArbitrationState::default();
    bus.read_partial_harness_with_cartridge(address, BusRequester::Cpu, &state, None)
}

fn write_cartridgeless_bus_harness(bus: &mut Bus, address: u16, value: u8) {
    let state = BusArbitrationState::default();
    bus.write_partial_harness_with_cartridge(address, value, BusRequester::Cpu, &state, None);
}

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
fn explicit_cartridgeless_bus_harness_round_trips_only_through_storage_regions() {
    let mut bus = Bus::new(ConsoleModel::Dmg);

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
    let mut bus = Bus::new(ConsoleModel::Dmg);

    write_cartridgeless_bus_harness(&mut bus, 0xC000, 0xA1);
    write_cartridgeless_bus_harness(&mut bus, 0xE321, 0xB2);

    assert_eq!(read_cartridgeless_bus_harness(&mut bus, 0xE000), 0xA1);
    assert_eq!(read_cartridgeless_bus_harness(&mut bus, 0xC321), 0xB2);
}

#[test]
fn explicit_cartridgeless_bus_harness_uses_placeholders_for_unowned_regions() {
    let mut bus = Bus::new(ConsoleModel::Dmg);

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
    let dmg_bus = Bus::new(ConsoleModel::Dmg);
    let cgb_bus = Bus::new(ConsoleModel::Cgb);

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
    assert_eq!(cgb.runtime_fallback_read_value(), 0xFF);
    assert!(cgb.runtime_fallback_writes_ignored());
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
fn external_bus_dma_policy_blocks_all_cpu_regions_except_hram_ff46_and_non_cpu_requesters() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default().with_dma(DmaBusState::external_bus_blocked(Some(
        DmaMemoryRegionImpact::Oam,
    )));

    let cpu_blocked = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0xC000, &state);
    let cpu_vram = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0x8000, &state);
    let cpu_hram = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0xFF80, &state);
    let cpu_ff46 = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0xFF46, &state);
    let dma_allowed = bus.resolve_access(BusRequester::Dma, BusAccessKind::Read, 0xC000, &state);

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
    assert!(dma_allowed.disposition().is_allowed());
}

#[test]
fn external_bus_dma_policy_ignores_cpu_writes_outside_hram_and_ff46() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default().with_dma(DmaBusState::external_bus_blocked(Some(
        DmaMemoryRegionImpact::Oam,
    )));

    let blocked_wram = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Write, 0xC000, &state);
    let allowed_vram = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Write, 0x8000, &state);
    let allowed_hram = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Write, 0xFF80, &state);
    let allowed_ff46 = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Write, 0xFF46, &state);

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
fn public_bus_resolution_exposes_nominal_and_effective_targets_during_dma_redirection() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default().with_dma(
        DmaBusState::external_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
            .with_cpu_conflict_source_address(Some(0xC100)),
    );

    let resolution = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0xC200, &state);

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

#[test]
fn bus_snapshot_and_trace_expose_live_arbitration_state() {
    let bus = Bus::new(ConsoleModel::Cgb);
    let state = BusArbitrationState::default()
        .with_boot_rom(BootRomBusState::map_cgb_windows())
        .with_ppu(PpuBusState::lcd_enabled(PpuAccessMode::Drawing))
        .with_dma(
            DmaBusState::video_bus_blocked(Some(DmaMemoryRegionImpact::Vram))
                .with_cpu_conflict_source_address(Some(0x8120)),
        );

    let snapshot = bus.snapshot(state);

    assert_eq!(snapshot.console_model, ConsoleModel::Cgb);
    assert_eq!(snapshot.status, gb_core::BusStatus::Ready);
    assert_eq!(snapshot.arbitration, state);

    let mut context = CycleContext::for_cycle(TCycle::new(7));
    context.enter_phase(SchedulerPhase::BusArbitration);

    let trace = bus.scheduler_trace_message(&context, &state);

    assert!(trace.contains("boot_low_window_mapped=true"));
    assert!(trace.contains("boot_cgb_upper_window_mapped=true"));
    assert!(trace.contains("ppu_lcd_enabled=true"));
    assert!(trace.contains("ppu_mode=Drawing"));
    assert!(trace.contains("dma_cpu_access_policy=VideoBusBlocked"));
    assert!(trace.contains("dma_active_region=Some(Vram)"));
    assert!(trace.contains("dma_cpu_conflict_source_address=Some(33056)"));
}
