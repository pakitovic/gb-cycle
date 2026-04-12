use super::*;

#[test]
fn unusable_area_readback_tracks_oam_blocked_periods() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let oam_blocked =
        BusArbitrationState::default().with_ppu(PpuBusState::lcd_enabled(PpuAccessMode::Drawing));

    let blocked = bus.resolve_access(BusAccessKind::Read, 0xFEA0, &oam_blocked, None);
    let ordinary = bus.resolve_access(
        BusAccessKind::Read,
        0xFEA0,
        &BusArbitrationState::default(),
        None,
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

    let blocked = bus.resolve_access(BusAccessKind::Read, 0xFEA0, &dma_video_bus_blocked, None);

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
