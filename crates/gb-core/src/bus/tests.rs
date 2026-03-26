use super::*;
use crate::cpu::{CpuAddressEvent, CpuAddressEventKind, CpuAddressUpdateDirection};
use crate::ppu::{DmgObjPaletteReadPolicy, Ppu, PpuAccessMode, PpuBusState, PpuStartupState};
use crate::scheduler::{CycleContext, TCycle};

fn sync_test_video_ownership(ppu: &Ppu, oam: &mut OamDomain, vram: &mut VramDomain) {
    let bus_state = ppu.bus_state();
    let ppu_vram = bus_state.is_lcd_enabled() && bus_state.mode() == PpuAccessMode::Drawing;
    let ppu_oam = bus_state.is_lcd_enabled()
        && matches!(
            bus_state.mode(),
            PpuAccessMode::OamScan | PpuAccessMode::Drawing
        );

    oam.set_acquired(BusMaster::Ppu, ppu_oam);
    vram.set_acquired(BusMaster::Ppu, ppu_vram);
    oam.set_acquired(BusMaster::Dma, false);
    vram.set_acquired(BusMaster::Dma, false);
}

fn tick_ppu(ppu: &mut Ppu, t_cycle: u64) {
    let mut context = CycleContext::for_cycle(TCycle::new(t_cycle));
    let mut oam = OamDomain::new();
    let mut vram = VramDomain::new();
    sync_test_video_ownership(ppu, &mut oam, &mut vram);
    ppu.tick_t_cycle(
        &mut context,
        OamBusView::new(BusMaster::Ppu, &mut oam),
        VramBusView::new(BusMaster::Ppu, &mut vram),
        false,
        None,
    );
}

fn prepare_mode2_ppu_at_row(console_model: ConsoleModel, row: u8) -> Ppu {
    let mut ppu = Ppu::new(console_model);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    let ticks = if row == 0 { 0 } else { u64::from(row) * 4 + 1 };

    for t_cycle in 0..ticks {
        tick_ppu(&mut ppu, t_cycle);
    }

    assert_eq!(ppu.snapshot().current_oam_scan_row, Some(row));
    ppu
}

fn prepare_mode3_ppu(console_model: ConsoleModel) -> Ppu {
    let mut ppu = prepare_mode2_ppu_at_row(console_model, 0);
    for t_cycle in 0..80 {
        tick_ppu(&mut ppu, t_cycle);
    }
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::Drawing);
    ppu
}

fn write_oam_word_bytes(oam_bytes: &mut [u8], row: u8, word_index: usize, value: u16) {
    let word_start = row as usize * 8 + word_index * 2;
    let [low, high] = value.to_le_bytes();
    oam_bytes[word_start] = low;
    oam_bytes[word_start + 1] = high;
}

fn read_oam_word_bytes(oam_bytes: &[u8], row: u8, word_index: usize) -> u16 {
    let word_start = row as usize * 8 + word_index * 2;
    u16::from_le_bytes([oam_bytes[word_start], oam_bytes[word_start + 1]])
}

fn seed_oam_corruption_rows(oam_bytes: &mut [u8]) {
    write_oam_word_bytes(oam_bytes, 0, 0, 0x1357);
    write_oam_word_bytes(oam_bytes, 0, 1, 0x2468);
    write_oam_word_bytes(oam_bytes, 0, 2, 0xAAAA);
    write_oam_word_bytes(oam_bytes, 0, 3, 0xBBBB);
    write_oam_word_bytes(oam_bytes, 1, 0, 0x0F0F);
    write_oam_word_bytes(oam_bytes, 1, 1, 0x1111);
    write_oam_word_bytes(oam_bytes, 1, 2, 0x2222);
    write_oam_word_bytes(oam_bytes, 1, 3, 0x3333);
    write_oam_word_bytes(oam_bytes, 2, 0, 0x5555);
    write_oam_word_bytes(oam_bytes, 2, 1, 0x6666);
    write_oam_word_bytes(oam_bytes, 2, 2, 0x7777);
    write_oam_word_bytes(oam_bytes, 2, 3, 0x8888);
}

#[test]
fn decode_address_covers_each_dmg_region_boundary() {
    let bus = Bus::new(ConsoleModel::Dmg);
    let cases = [
        (
            0x0000,
            BusRegion::CartridgeRomBank0,
            BusRegionOwner::Cartridge,
            0x0000,
        ),
        (
            0x3FFF,
            BusRegion::CartridgeRomBank0,
            BusRegionOwner::Cartridge,
            0x3FFF,
        ),
        (
            0x4000,
            BusRegion::CartridgeRomBankN,
            BusRegionOwner::Cartridge,
            0x0000,
        ),
        (
            0x7FFF,
            BusRegion::CartridgeRomBankN,
            BusRegionOwner::Cartridge,
            0x3FFF,
        ),
        (0x8000, BusRegion::Vram, BusRegionOwner::Ppu, 0x0000),
        (0x9FFF, BusRegion::Vram, BusRegionOwner::Ppu, 0x1FFF),
        (
            0xA000,
            BusRegion::CartridgeExternal,
            BusRegionOwner::Cartridge,
            0x0000,
        ),
        (
            0xBFFF,
            BusRegion::CartridgeExternal,
            BusRegionOwner::Cartridge,
            0x1FFF,
        ),
        (0xC000, BusRegion::WramBank0, BusRegionOwner::Bus, 0x0000),
        (0xCFFF, BusRegion::WramBank0, BusRegionOwner::Bus, 0x0FFF),
        (0xD000, BusRegion::WramBankN, BusRegionOwner::Bus, 0x0000),
        (0xDFFF, BusRegion::WramBankN, BusRegionOwner::Bus, 0x0FFF),
        (0xE000, BusRegion::EchoRam, BusRegionOwner::Bus, 0x0000),
        (0xFDFF, BusRegion::EchoRam, BusRegionOwner::Bus, 0x1DFF),
        (0xFE00, BusRegion::Oam, BusRegionOwner::Ppu, 0x0000),
        (0xFE9F, BusRegion::Oam, BusRegionOwner::Ppu, 0x009F),
        (0xFEA0, BusRegion::Unusable, BusRegionOwner::Bus, 0x0000),
        (0xFEFF, BusRegion::Unusable, BusRegionOwner::Bus, 0x005F),
        (0xFF00, BusRegion::Mmio, BusRegionOwner::Mmio, 0x0000),
        (0xFF7F, BusRegion::Mmio, BusRegionOwner::Mmio, 0x007F),
        (0xFF80, BusRegion::Hram, BusRegionOwner::Bus, 0x0000),
        (0xFFFE, BusRegion::Hram, BusRegionOwner::Bus, 0x007E),
        (
            0xFFFF,
            BusRegion::InterruptEnable,
            BusRegionOwner::InterruptController,
            0x0000,
        ),
    ];

    for (address, region, owner, region_offset) in cases {
        let decoded = bus.decode_address(address);
        assert_eq!(decoded.address(), address);
        assert_eq!(decoded.region(), region);
        assert_eq!(decoded.owner(), owner);
        assert_eq!(decoded.region_offset(), region_offset);
    }
}

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
fn io_contract_table_covers_ff00_ff7f_and_ie() {
    let bus = Bus::new(ConsoleModel::Dmg);

    for address in 0xFF00..=0xFF7F {
        assert!(
            bus.describe_io_register(address).is_some(),
            "missing IO contract for {address:#06X}"
        );
    }

    let ff46 = bus.describe_io_register(0xFF46).unwrap();
    let ff50 = bus.describe_io_register(0xFF50).unwrap();
    let ie = bus.describe_io_register(0xFFFF).unwrap();

    assert_eq!(ff46.owner(), IoRegisterOwner::Dma);
    assert_eq!(ff46.kind(), IoRegisterKind::OamDma);
    assert_eq!(ff50.owner(), IoRegisterOwner::Boot);
    assert_eq!(ff50.access(), IoRegisterAccess::WriteOnly);
    assert_eq!(ie.kind(), IoRegisterKind::InterruptEnable);
}

#[test]
fn dmg_cgb_only_io_fallback_reads_as_ff() {
    let bus = Bus::new(ConsoleModel::Dmg);

    assert_eq!(bus.read_io_target(0xFF4D, BusIoReadView::default()), 0xFF);
    assert_eq!(bus.read_io_target(0xFF70, BusIoReadView::default()), 0xFF);
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

#[test]
fn route_cpu_address_event_turns_mode2_oam_reads_into_corruption_events() {
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut ppu = prepare_mode2_ppu_at_row(ConsoleModel::Dmg, 1);
    seed_oam_corruption_rows(bus.oam.bytes_mut());

    let state = BusArbitrationState::default().with_ppu(ppu.bus_state());
    bus.route_cpu_address_event(
        CpuAddressEvent {
            kind: CpuAddressEventKind::Read,
            access_address: Some(0xFE20),
            idu_address: None,
            update_direction: None,
        },
        &state,
        &mut ppu,
    );

    let expected_first = 0x1357_u16 | (0x0F0F & 0xAAAA);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 0), expected_first);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 1), 0x2468);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 2), 0xAAAA);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 3), 0xBBBB);
}

#[test]
fn route_cpu_address_event_uses_the_unusable_mode2_read_path_for_corruption() {
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut ppu = prepare_mode2_ppu_at_row(ConsoleModel::Dmg, 1);
    seed_oam_corruption_rows(bus.oam.bytes_mut());

    let state = BusArbitrationState::default().with_ppu(ppu.bus_state());
    bus.route_cpu_address_event(
        CpuAddressEvent {
            kind: CpuAddressEventKind::Read,
            access_address: Some(0xFEA0),
            idu_address: None,
            update_direction: None,
        },
        &state,
        &mut ppu,
    );

    let expected_first = 0x1357_u16 | (0x0F0F & 0xAAAA);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 0), expected_first);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 1), 0x2468);
}

#[test]
fn route_cpu_address_event_uses_the_unusable_mode2_write_path_for_corruption() {
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut ppu = prepare_mode2_ppu_at_row(ConsoleModel::Dmg, 1);
    seed_oam_corruption_rows(bus.oam.bytes_mut());

    let state = BusArbitrationState::default().with_ppu(ppu.bus_state());
    bus.route_cpu_address_event(
        CpuAddressEvent {
            kind: CpuAddressEventKind::Write,
            access_address: Some(0xFEA0),
            idu_address: None,
            update_direction: None,
        },
        &state,
        &mut ppu,
    );

    let expected_first = ((0x0F0F_u16 ^ 0xAAAA) & (0x1357 ^ 0xAAAA)) ^ 0xAAAA;
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 0), expected_first);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 1), 0x2468);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 2), 0xAAAA);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 3), 0xBBBB);
}

#[test]
fn route_cpu_address_event_uses_pure_idu_activity_in_fe_range() {
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut ppu = prepare_mode2_ppu_at_row(ConsoleModel::Dmg, 2);
    seed_oam_corruption_rows(bus.oam.bytes_mut());

    let state = BusArbitrationState::default().with_ppu(ppu.bus_state());
    bus.route_cpu_address_event(
        CpuAddressEvent {
            kind: CpuAddressEventKind::IncDec,
            access_address: None,
            idu_address: Some(0xFE11),
            update_direction: Some(CpuAddressUpdateDirection::Increment),
        },
        &state,
        &mut ppu,
    );

    let expected_first = ((0x5555_u16 ^ 0x2222) & (0x0F0F ^ 0x2222)) ^ 0x2222;
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 2, 0), expected_first);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 2, 1), 0x1111);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 2, 2), 0x2222);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 2, 3), 0x3333);
}

#[test]
fn route_cpu_address_event_uses_write_with_incdec_when_the_idu_edge_reaches_oam() {
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut ppu = prepare_mode2_ppu_at_row(ConsoleModel::Dmg, 2);
    seed_oam_corruption_rows(bus.oam.bytes_mut());

    let state = BusArbitrationState::default().with_ppu(ppu.bus_state());
    bus.route_cpu_address_event(
        CpuAddressEvent {
            kind: CpuAddressEventKind::WriteWithIncDec,
            access_address: Some(0xFDFF),
            idu_address: Some(0xFDFF),
            update_direction: Some(CpuAddressUpdateDirection::Decrement),
        },
        &state,
        &mut ppu,
    );

    let expected_first = ((0x5555_u16 ^ 0x2222) & (0x0F0F ^ 0x2222)) ^ 0x2222;
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 2, 0), expected_first);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 2, 1), 0x1111);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 2, 2), 0x2222);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 2, 3), 0x3333);
}

#[test]
fn route_cpu_address_event_does_not_turn_mode3_oam_blocking_into_corruption() {
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut ppu = prepare_mode3_ppu(ConsoleModel::Dmg);
    seed_oam_corruption_rows(bus.oam.bytes_mut());
    let before = bus.oam.clone();

    let state = BusArbitrationState::default().with_ppu(ppu.bus_state());
    bus.route_cpu_address_event(
        CpuAddressEvent {
            kind: CpuAddressEventKind::Read,
            access_address: Some(0xFE20),
            idu_address: None,
            update_direction: None,
        },
        &state,
        &mut ppu,
    );

    assert_eq!(bus.oam, before);
}

#[test]
fn bus_address_and_io_metadata_accessors_keep_domain_information_explicit() {
    let address = BusAddressInfo::new(0x8000, BusRegion::Vram, 0x0012);
    let io = IoRegisterInfo::new(
        0xFF46,
        IoRegisterOwner::Dma,
        IoRegisterAvailability::AllModels,
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
    assert_eq!(io.availability(), IoRegisterAvailability::AllModels);
    assert_eq!(io.access(), IoRegisterAccess::WriteOnly);
    assert_eq!(io.kind(), IoRegisterKind::OamDma);
}

#[test]
fn video_views_expose_master_owned_acquire_release_and_bytes() {
    let mut oam = OamDomain::new();
    let mut vram = VramDomain::new();

    {
        let mut oam_view = OamBusView::new(BusMaster::Ppu, &mut oam);
        assert_eq!(oam_view.master(), BusMaster::Ppu);
        assert!(!oam_view.is_acquired());
        assert!(!oam_view.is_acquired_by_master());
        assert!(oam_view.read(OAM_LEN - 1).is_some());
        assert!(oam_view.read(OAM_LEN).is_none());

        oam_view.acquire();
        assert!(oam_view.is_acquired());
        assert!(oam_view.is_acquired_by_master());

        oam_view.release();
        assert!(!oam_view.is_acquired());
        assert!(!oam_view.is_acquired_by_master());
    }

    {
        let mut vram_view = VramBusView::new(BusMaster::Dma, &mut vram);
        assert_eq!(vram_view.master(), BusMaster::Dma);
        assert!(!vram_view.is_acquired());
        assert!(!vram_view.is_acquired_by_master());
        assert!(vram_view.read(VRAM_LEN - 1).is_some());
        assert!(vram_view.read(VRAM_LEN).is_none());

        vram_view.acquire();
        assert!(vram_view.is_acquired());
        assert!(vram_view.is_acquired_by_master());

        vram_view.release();
        assert!(!vram_view.is_acquired());
        assert!(!vram_view.is_acquired_by_master());
    }
}
