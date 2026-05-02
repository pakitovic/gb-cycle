use super::*;

#[test]
fn resolve_access_uses_boot_overlay_for_reads_but_not_for_writes() {
    let bus = Bus::new(ConsoleModel::GameBoy);
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
fn boot_overlay_reads_report_the_boot_domain() {
    let bus = Bus::new(ConsoleModel::GameBoy);
    let state = BusArbitrationState::default().with_boot_rom(BootRomBusState::map_dmg_low_bytes());

    let read = bus.resolve_access(BusAccessKind::Read, 0x0000, &state, None);

    assert_eq!(read.target().domain(), BusDomain::BootRom);
}

#[test]
fn cgb_boot_overlay_can_cover_the_upper_window_without_changing_write_ownership() {
    let bus = Bus::new(ConsoleModel::GameBoyColor);
    let state = BusArbitrationState::default().with_boot_rom(BootRomBusState::map_cgb_windows());

    let read = bus.resolve_access(BusAccessKind::Read, 0x0200, &state, None);
    let write = bus.resolve_access(BusAccessKind::Write, 0x0200, &state, None);

    assert_eq!(read.target().region(), BusRegion::BootRom);
    assert_eq!(read.target().owner(), BusRegionOwner::Boot);
    assert!(read.disposition().is_allowed());
    assert_eq!(write.target().region(), BusRegion::CartridgeRomBank0);
    assert_eq!(write.target().owner(), BusRegionOwner::Cartridge);
    assert!(write.disposition().is_allowed());
}

#[test]
fn resolve_access_keeps_nominal_target_and_policy_separate() {
    let bus = Bus::new(ConsoleModel::GameBoy);
    let state =
        BusArbitrationState::default().with_ppu(PpuBusState::lcd_enabled(PpuAccessMode::OamScan));

    let resolution = bus.resolve_access(BusAccessKind::Read, 0xFE00, &state, None);

    assert_eq!(resolution.target().region(), BusRegion::Oam);
    assert_eq!(resolution.target().owner(), BusRegionOwner::Ppu);
    assert_eq!(
        resolution.disposition(),
        BusAccessDisposition::BlockedRead {
            value: BLOCKED_READ_VALUE,
            reason: BusBlockReason::PpuOamBlockedDuringMode2,
        }
    );
}

#[test]
fn echo_ram_aliases_shared_wram_storage() {
    let mut bus = Bus::new(ConsoleModel::GameBoy);

    bus.write(0xC123, 0x42);
    assert_eq!(bus.read(0xE123), 0x42);

    bus.write(0xFDFF, 0x7E);
    assert_eq!(bus.read(0xDDFF), 0x7E);
}

#[test]
fn cartridge_mmio_and_unusable_placeholders_do_not_behave_like_storage() {
    let mut bus = Bus::new(ConsoleModel::GameBoy);

    bus.write(0x0000, 0x12);
    bus.write(0x4000, 0x23);
    bus.write(0xA000, 0x34);
    bus.write(0xFF10, 0x45);
    bus.write(0xFEA0, 0x56);

    assert_eq!(bus.read(0x0000), BLOCKED_READ_VALUE);
    assert_eq!(bus.read(0x4000), BLOCKED_READ_VALUE);
    assert_eq!(bus.read(0xA000), BLOCKED_READ_VALUE);
    assert_eq!(bus.read(0xFF10), BLOCKED_READ_VALUE);
    assert_eq!(bus.read(0xFEA0), DMG_UNUSABLE_READ_VALUE);
}

#[test]
fn cgb_unusable_placeholder_reads_stay_tied_to_the_public_revision_dependent_descriptor() {
    let mut bus = Bus::new(ConsoleModel::GameBoyColor);

    let descriptor = bus.describe_unusable_area(0xFEA0).unwrap();

    assert_eq!(
        descriptor.read_profile(),
        UnusableAreaReadProfile::CgbRevisionDependent
    );
    assert_eq!(
        descriptor.write_profile(),
        UnusableAreaWriteProfile::CgbRevisionDependentRam
    );
    assert!(descriptor.runtime_fallback_writes_ignored());
    assert_eq!(bus.read(0xFEA0), descriptor.runtime_fallback_read_value());
}

#[test]
fn cgb_unusable_placeholder_writes_are_currently_ignored_but_not_advertised_as_nominally_absent() {
    let mut bus = Bus::new(ConsoleModel::GameBoyColor);
    let descriptor = bus.describe_unusable_area(0xFEA0).unwrap();

    bus.write(0xFEA0, 0x12);

    assert_eq!(
        descriptor.write_profile(),
        UnusableAreaWriteProfile::CgbRevisionDependentRam
    );
    assert!(descriptor.runtime_fallback_writes_ignored());
    assert_eq!(bus.read(0xFEA0), descriptor.runtime_fallback_read_value());
}

#[test]
fn video_bus_dma_policy_has_precedence_over_ppu_region_rules() {
    let bus = Bus::new(ConsoleModel::GameBoy);
    let state = BusArbitrationState::default()
        .with_dma(DmaBusState::video_bus_blocked(Some(
            DmaMemoryRegionImpact::Oam,
        )))
        .with_ppu(PpuBusState::lcd_enabled(PpuAccessMode::Drawing));

    let resolution = bus.resolve_access(BusAccessKind::Read, 0x8000, &state, None);

    assert_eq!(resolution.target().region(), BusRegion::Vram);
    assert_eq!(
        resolution.disposition().blocked_reason(),
        Some(BusBlockReason::DmaVideoBusConflict)
    );
}

#[test]
fn external_bus_dma_policy_keeps_ff46_readable_and_writable_during_active_dma() {
    let bus = Bus::new(ConsoleModel::GameBoy);
    let state = BusArbitrationState::default().with_dma(DmaBusState::external_bus_blocked(Some(
        DmaMemoryRegionImpact::Oam,
    )));

    let read_resolution = bus.resolve_access(BusAccessKind::Read, 0xFF46, &state, None);
    assert_eq!(read_resolution.target().region(), BusRegion::Mmio);
    assert!(read_resolution.disposition().is_allowed());

    let write_resolution = bus.resolve_access(BusAccessKind::Write, 0xFF46, &state, None);
    assert_eq!(write_resolution.target().region(), BusRegion::Mmio);
    assert!(write_resolution.disposition().is_allowed());
}

#[test]
fn external_bus_dma_resolution_exposes_nominal_blocking_and_effective_redirection() {
    let bus = Bus::new(ConsoleModel::GameBoy);
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
            value: BLOCKED_READ_VALUE,
            reason: BusBlockReason::DmaExternalBusConflict,
        }
    );
    assert_eq!(resolution.target().address(), 0xC100);
    assert_eq!(resolution.target().region(), BusRegion::WramBank0);
    assert_eq!(resolution.disposition(), BusAccessDisposition::Allowed);
    assert!(resolution.is_redirected());
    assert_eq!(resolution.redirected_source_address(), Some(0xC100));
}

#[test]
fn external_bus_only_dma_blocks_and_redirects_only_cartridge_bus_accesses() {
    let bus = Bus::new(ConsoleModel::GameBoyColor);
    let state = BusArbitrationState::default().with_dma(
        DmaBusState::external_bus_only_blocked(Some(DmaMemoryRegionImpact::Oam))
            .with_cpu_conflict_source_address(Some(0x1234)),
    );

    let rom_resolution = bus.resolve_access(BusAccessKind::Read, 0x0150, &state, None);
    assert_eq!(
        rom_resolution.nominal_target().region(),
        BusRegion::CartridgeRomBank0
    );
    assert_eq!(
        rom_resolution.nominal_disposition(),
        BusAccessDisposition::BlockedRead {
            value: BLOCKED_READ_VALUE,
            reason: BusBlockReason::DmaExternalBusConflict,
        }
    );
    assert_eq!(rom_resolution.target().address(), 0x1234);
    assert_eq!(
        rom_resolution.target().region(),
        BusRegion::CartridgeRomBank0
    );
    assert_eq!(rom_resolution.disposition(), BusAccessDisposition::Allowed);
    assert!(rom_resolution.is_redirected());

    let external_resolution = bus.resolve_access(BusAccessKind::Read, 0xA000, &state, None);
    assert_eq!(
        external_resolution.nominal_target().region(),
        BusRegion::CartridgeExternal
    );
    assert_eq!(external_resolution.target().address(), 0x1234);
    assert!(external_resolution.is_redirected());

    let wram_resolution = bus.resolve_access(BusAccessKind::Read, 0xC200, &state, None);
    assert_eq!(wram_resolution.target().region(), BusRegion::WramBank0);
    assert!(wram_resolution.disposition().is_allowed());
    assert!(!wram_resolution.is_redirected());
}

#[test]
fn external_bus_only_dma_keeps_internal_wram_writes_and_hram_access_available() {
    let bus = Bus::new(ConsoleModel::GameBoyColor);
    let state = BusArbitrationState::default().with_dma(
        DmaBusState::external_bus_only_blocked(Some(DmaMemoryRegionImpact::Oam))
            .with_cpu_conflict_source_address(Some(0x1234)),
    );

    let wram_write = bus.resolve_access(BusAccessKind::Write, 0xC200, &state, None);
    assert_eq!(wram_write.target().region(), BusRegion::WramBank0);
    assert!(wram_write.disposition().is_allowed());
    assert!(!wram_write.is_redirected());

    let hram_read = bus.resolve_access(BusAccessKind::Read, 0xFF80, &state, None);
    assert_eq!(hram_read.target().region(), BusRegion::Hram);
    assert!(hram_read.disposition().is_allowed());

    let dma_register_write = bus.resolve_access(BusAccessKind::Write, 0xFF46, &state, None);
    assert_eq!(dma_register_write.target().region(), BusRegion::Mmio);
    assert!(dma_register_write.disposition().is_allowed());
}

#[test]
fn external_bus_only_dma_still_blocks_oam_destination_access() {
    let bus = Bus::new(ConsoleModel::GameBoyColor);
    let state = BusArbitrationState::default().with_dma(
        DmaBusState::external_bus_only_blocked(Some(DmaMemoryRegionImpact::Oam))
            .with_cpu_conflict_source_address(Some(0x1234)),
    );

    let resolution = bus.resolve_access(BusAccessKind::Read, 0xFE00, &state, None);

    assert_eq!(resolution.target().region(), BusRegion::Oam);
    assert_eq!(
        resolution.disposition(),
        BusAccessDisposition::BlockedRead {
            value: BLOCKED_READ_VALUE,
            reason: BusBlockReason::DmaExternalBusConflict,
        }
    );
    assert!(!resolution.is_redirected());
}

#[test]
fn wram_bus_dma_blocks_and_redirects_only_wram_bus_accesses() {
    let bus = Bus::new(ConsoleModel::GameBoyColor);
    let state = BusArbitrationState::default().with_dma(
        DmaBusState::wram_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
            .with_cpu_conflict_source_address(Some(0xC123)),
    );

    let wram_resolution = bus.resolve_access(BusAccessKind::Read, 0xD200, &state, None);
    assert_eq!(
        wram_resolution.nominal_target().region(),
        BusRegion::WramBankN
    );
    assert_eq!(
        wram_resolution.nominal_disposition(),
        BusAccessDisposition::BlockedRead {
            value: BLOCKED_READ_VALUE,
            reason: BusBlockReason::DmaWramBusConflict,
        }
    );
    assert_eq!(wram_resolution.target().address(), 0xC123);
    assert_eq!(wram_resolution.target().region(), BusRegion::WramBank0);
    assert_eq!(wram_resolution.disposition(), BusAccessDisposition::Allowed);
    assert!(wram_resolution.is_redirected());

    let echo_resolution = bus.resolve_access(BusAccessKind::Read, 0xE200, &state, None);
    assert_eq!(
        echo_resolution.nominal_target().region(),
        BusRegion::EchoRam
    );
    assert_eq!(echo_resolution.target().address(), 0xC123);
    assert!(echo_resolution.is_redirected());

    let rom_resolution = bus.resolve_access(BusAccessKind::Read, 0x0150, &state, None);
    assert_eq!(
        rom_resolution.target().region(),
        BusRegion::CartridgeRomBank0
    );
    assert!(rom_resolution.disposition().is_allowed());
    assert!(!rom_resolution.is_redirected());

    let hram_resolution = bus.resolve_access(BusAccessKind::Read, 0xFF80, &state, None);
    assert_eq!(hram_resolution.target().region(), BusRegion::Hram);
    assert!(hram_resolution.disposition().is_allowed());
}

#[test]
fn wram_bus_dma_still_blocks_oam_destination_access() {
    let bus = Bus::new(ConsoleModel::GameBoyColor);
    let state = BusArbitrationState::default().with_dma(
        DmaBusState::wram_bus_blocked(Some(DmaMemoryRegionImpact::Oam))
            .with_cpu_conflict_source_address(Some(0xC123)),
    );

    let resolution = bus.resolve_access(BusAccessKind::Read, 0xFE00, &state, None);

    assert_eq!(resolution.target().region(), BusRegion::Oam);
    assert_eq!(
        resolution.disposition(),
        BusAccessDisposition::BlockedRead {
            value: BLOCKED_READ_VALUE,
            reason: BusBlockReason::DmaWramBusConflict,
        }
    );
    assert!(!resolution.is_redirected());
}

#[test]
fn external_bus_only_dma_does_not_redirect_internal_boot_rom_reads() {
    let bus = Bus::new(ConsoleModel::GameBoyColor);
    let state = BusArbitrationState::default()
        .with_boot_rom(BootRomBusState::map_cgb_windows())
        .with_dma(
            DmaBusState::external_bus_only_blocked(Some(DmaMemoryRegionImpact::Oam))
                .with_cpu_conflict_source_address(Some(0x1234)),
        );

    let resolution = bus.resolve_access(BusAccessKind::Read, 0x0000, &state, None);

    assert_eq!(resolution.target().region(), BusRegion::BootRom);
    assert!(resolution.disposition().is_allowed());
    assert!(!resolution.is_redirected());
}

#[test]
fn requester_aware_resolution_keeps_non_cpu_dma_accesses_unblocked() {
    let bus = Bus::new(ConsoleModel::GameBoy);
    let state = BusArbitrationState::default().with_dma(DmaBusState::external_bus_blocked(Some(
        DmaMemoryRegionImpact::Oam,
    )));

    let resolution =
        bus.resolve_requester_access(BusRequester::Dma, BusAccessKind::Read, 0xC000, &state, None);

    assert_eq!(resolution.requester(), BusRequester::Dma);
    assert!(resolution.disposition().is_allowed());
}

#[test]
fn requester_aware_resolution_can_tag_non_cpu_requesters_for_runtime_observability() {
    let bus = Bus::new(ConsoleModel::GameBoy);

    let resolution = bus.resolve_requester_access(
        BusRequester::Boot,
        BusAccessKind::Write,
        0x0000,
        &BusArbitrationState::default(),
        None,
    );

    assert_eq!(resolution.requester(), BusRequester::Boot);
    assert_eq!(resolution.kind(), BusAccessKind::Write);
}
