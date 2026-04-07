use super::*;

#[test]
fn resolve_access_uses_boot_overlay_for_reads_but_not_for_writes() {
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
fn boot_overlay_reads_report_the_boot_domain() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default().with_boot_rom(BootRomBusState::map_dmg_low_bytes());

    let read = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0x0000, &state);

    assert_eq!(read.target().domain(), BusDomain::BootRom);
}

#[test]
fn resolve_access_keeps_nominal_target_and_policy_separate() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let state =
        BusArbitrationState::default().with_ppu(PpuBusState::lcd_enabled(PpuAccessMode::OamScan));

    let resolution = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0xFE00, &state);

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
    let mut bus = Bus::new(ConsoleModel::Dmg);

    bus.write(0xC123, 0x42);
    assert_eq!(bus.read(0xE123), 0x42);

    bus.write(0xFDFF, 0x7E);
    assert_eq!(bus.read(0xDDFF), 0x7E);
}

#[test]
fn cartridge_mmio_and_unusable_placeholders_do_not_behave_like_storage() {
    let mut bus = Bus::new(ConsoleModel::Dmg);

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
fn video_bus_dma_policy_has_precedence_over_ppu_region_rules() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default()
        .with_dma(DmaBusState::video_bus_blocked(Some(
            DmaMemoryRegionImpact::Oam,
        )))
        .with_ppu(PpuBusState::lcd_enabled(PpuAccessMode::Drawing));

    let resolution = bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0x8000, &state);

    assert_eq!(resolution.target().region(), BusRegion::Vram);
    assert_eq!(
        resolution.disposition().blocked_reason(),
        Some(BusBlockReason::DmaVideoBusConflict)
    );
}

#[test]
fn external_bus_dma_policy_keeps_ff46_readable_and_writable_during_active_dma() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let state = BusArbitrationState::default().with_dma(DmaBusState::external_bus_blocked(Some(
        DmaMemoryRegionImpact::Oam,
    )));

    let read_resolution =
        bus.resolve_access(BusRequester::Cpu, BusAccessKind::Read, 0xFF46, &state);
    assert_eq!(read_resolution.target().region(), BusRegion::Mmio);
    assert!(read_resolution.disposition().is_allowed());

    let write_resolution =
        bus.resolve_access(BusRequester::Cpu, BusAccessKind::Write, 0xFF46, &state);
    assert_eq!(write_resolution.target().region(), BusRegion::Mmio);
    assert!(write_resolution.disposition().is_allowed());
}
