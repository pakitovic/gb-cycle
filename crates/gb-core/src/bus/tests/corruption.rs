use super::*;

#[test]
fn route_cpu_address_event_turns_mode2_oam_reads_into_corruption_events() {
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let mut ppu = prepare_mode2_ppu_at_row(ConsoleModel::GameBoy, 1);
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
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 0, 0), expected_first);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 0), expected_first);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 1), 0x2468);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 2), 0xAAAA);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 3), 0xBBBB);
}

#[test]
fn route_cpu_address_event_uses_the_unusable_mode2_read_path_for_corruption() {
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let mut ppu = prepare_mode2_ppu_at_row(ConsoleModel::GameBoy, 1);
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
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 0, 0), expected_first);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 0), expected_first);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 1, 1), 0x2468);
}

#[test]
fn route_cpu_address_event_uses_the_unusable_mode2_write_path_for_corruption() {
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let mut ppu = prepare_mode2_ppu_at_row(ConsoleModel::GameBoy, 1);
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
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let mut ppu = prepare_mode2_ppu_at_row(ConsoleModel::GameBoy, 2);
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
fn route_cpu_address_event_keeps_the_last_mode2_row_corruptible_for_pure_idu_activity() {
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let mut ppu = prepare_mode2_ppu_at_row(ConsoleModel::GameBoy, 19);
    write_oam_word_bytes(bus.oam.bytes_mut(), 18, 0, 0x1234);
    write_oam_word_bytes(bus.oam.bytes_mut(), 18, 1, 0x1111);
    write_oam_word_bytes(bus.oam.bytes_mut(), 18, 2, 0x00FF);
    write_oam_word_bytes(bus.oam.bytes_mut(), 18, 3, 0x2222);
    write_oam_word_bytes(bus.oam.bytes_mut(), 19, 0, 0x0F0F);
    write_oam_word_bytes(bus.oam.bytes_mut(), 19, 1, 0xAAAA);
    write_oam_word_bytes(bus.oam.bytes_mut(), 19, 2, 0xBBBB);
    write_oam_word_bytes(bus.oam.bytes_mut(), 19, 3, 0xCCCC);

    let state = BusArbitrationState::default().with_ppu(ppu.bus_state());
    assert_eq!(state.ppu.mode(), PpuAccessMode::Drawing);
    assert_eq!(ppu.snapshot().current_oam_scan_row, Some(19));

    bus.route_cpu_address_event(
        CpuAddressEvent {
            kind: CpuAddressEventKind::IncDec,
            access_address: None,
            idu_address: Some(0xFE99),
            update_direction: Some(CpuAddressUpdateDirection::Increment),
        },
        &state,
        &mut ppu,
    );

    let expected_first = ((0x0F0F_u16 ^ 0x00FF) & (0x1234 ^ 0x00FF)) ^ 0x00FF;
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 19, 0), expected_first);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 19, 1), 0x1111);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 19, 2), 0x00FF);
    assert_eq!(read_oam_word_bytes(bus.oam.bytes(), 19, 3), 0x2222);
}

#[test]
fn route_cpu_address_event_does_not_widen_plain_oam_reads_in_the_last_mode2_slot() {
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let mut ppu = prepare_mode2_ppu_at_row(ConsoleModel::GameBoy, 19);
    write_oam_word_bytes(bus.oam.bytes_mut(), 18, 0, 0x1234);
    write_oam_word_bytes(bus.oam.bytes_mut(), 18, 1, 0x1111);
    write_oam_word_bytes(bus.oam.bytes_mut(), 18, 2, 0x00FF);
    write_oam_word_bytes(bus.oam.bytes_mut(), 18, 3, 0x2222);
    write_oam_word_bytes(bus.oam.bytes_mut(), 19, 0, 0x0F0F);
    write_oam_word_bytes(bus.oam.bytes_mut(), 19, 1, 0xAAAA);
    write_oam_word_bytes(bus.oam.bytes_mut(), 19, 2, 0xBBBB);
    write_oam_word_bytes(bus.oam.bytes_mut(), 19, 3, 0xCCCC);

    let before = bus.oam.clone();
    let state = BusArbitrationState::default().with_ppu(ppu.bus_state());
    assert_eq!(state.ppu.mode(), PpuAccessMode::Drawing);
    assert_eq!(ppu.snapshot().current_oam_scan_row, Some(19));

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
fn route_cpu_address_event_uses_write_with_incdec_when_the_idu_edge_reaches_oam() {
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let mut ppu = prepare_mode2_ppu_at_row(ConsoleModel::GameBoy, 2);
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
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let mut ppu = prepare_mode3_ppu(ConsoleModel::GameBoy);
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
fn route_cpu_address_event_does_not_turn_dma_video_bus_unusable_reads_into_corruption() {
    let mut bus = Bus::new(ConsoleModel::GameBoy);
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    seed_oam_corruption_rows(bus.oam.bytes_mut());
    let before = bus.oam.clone();

    let state = BusArbitrationState::default().with_dma(DmaBusState::video_bus_blocked(Some(
        DmaMemoryRegionImpact::Oam,
    )));
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

    assert_eq!(bus.oam, before);
}
