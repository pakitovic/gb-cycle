use super::*;

#[test]
fn mode2_selection_on_the_live_machine_preserves_oam_order_and_caps_at_ten_entries() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    for index in 0..12 {
        let x = match index {
            0 => 0,
            1 => 168,
            _ => 8 + index,
        };
        seed_oam_entry(&mut machine, index, 16, x, 0x40 + index, 0);
    }
    seed_oam_entry(&mut machine, 20, 8, 24, 0x99, 0);

    for _ in 0..80 {
        machine.step_t_cycle();
    }

    let snapshot = machine.ppu().snapshot();
    assert_eq!(snapshot.mode, PpuAccessMode::Drawing);
    assert_eq!(snapshot.mode2_scanned_entries, 40);
    assert_eq!(snapshot.selected_sprites.len(), 10);
    assert_eq!(
        snapshot
            .selected_sprites
            .iter()
            .map(|sprite| sprite.oam_index)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
    );
    assert_eq!(snapshot.selected_sprites[0].x, 0);
    assert_eq!(snapshot.selected_sprites[1].x, 168);
}

#[test]
fn mode2_selection_uses_live_lcdc2_on_the_machine_timeline() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_oam_entry(&mut machine, 0, 0, 24, 0x10, 0);
    seed_oam_entry(&mut machine, 1, 1, 32, 0x11, 0);

    machine.step_t_cycle();
    machine.step_t_cycle();
    assert!(machine.ppu().snapshot().selected_sprites.is_empty());

    machine.write_bus(0xFF40, 0x95);
    machine.step_t_cycle();
    machine.step_t_cycle();

    let snapshot = machine.ppu().snapshot();
    assert_eq!(snapshot.mode2_scanned_entries, 2);
    assert_eq!(snapshot.selected_sprites.len(), 1);
    assert_eq!(snapshot.selected_sprites[0].oam_index, 1);
    assert_eq!(snapshot.selected_sprites[0].y, 1);
}

#[test]
fn direct_mode2_oam_write_corrupts_the_live_row_without_storing_the_cpu_byte() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    let rows = build_oam_corruption_fixture();

    machine.write_bus(0xFF40, 0x00);
    seed_oam_corruption_fixture(&mut machine, &rows);
    machine.write_bus(0xFF40, 0x80);

    while machine.ppu().snapshot().current_oam_scan_row != Some(1) {
        machine.step_t_cycle();
    }

    let row = machine.ppu().snapshot().current_oam_scan_row.unwrap();
    machine.write_bus(0xFE20, 0x99);
    machine.write_bus(0xFF40, 0x00);

    assert_eq!(
        read_oam_corruption_row(&mut machine, row),
        expected_write_corruption(&rows, row)
    );
}

#[test]
fn direct_mode2_fea0_read_uses_blocked_readback_and_the_same_read_corruption_path() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    let rows = build_oam_corruption_fixture();

    machine.write_bus(0xFF40, 0x00);
    seed_oam_corruption_fixture(&mut machine, &rows);
    machine.write_bus(0xFF40, 0x80);

    while machine.ppu().snapshot().current_oam_scan_row != Some(2) {
        machine.step_t_cycle();
    }

    let row = machine.ppu().snapshot().current_oam_scan_row.unwrap();
    assert_eq!(machine.read_bus(0xFEA0), 0xFF);
    machine.write_bus(0xFF40, 0x00);

    assert_eq!(
        read_oam_corruption_row(&mut machine, row),
        expected_read_corruption(&rows, row)
    );
}

#[test]
fn cpu_inc_hl_inside_fe_range_reaches_the_same_mode2_corruption_controller() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    let rows = build_oam_corruption_fixture();
    let mut program = vec![0x21, 0x08, 0xFE];
    program.extend(std::iter::repeat_n(0x00, 119));
    program.extend([0x23, 0x00]);

    machine
        .load_cartridge(build_test_rom(&program, 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF40, 0x00);
    seed_oam_corruption_fixture(&mut machine, &rows);

    for _ in 0..12 {
        machine.step_t_cycle();
    }

    machine.write_bus(0xFF40, 0x80);

    let mut triggered_row = None;
    for _ in 0..1_024 {
        machine.step_t_cycle();
        if let Some(event) = machine.cpu().last_address_event()
            && event.kind == CpuAddressEventKind::IncDec
            && event.idu_address == Some(0xFE09)
            && event.update_direction == Some(CpuAddressUpdateDirection::Increment)
        {
            triggered_row = machine.ppu().snapshot().current_oam_scan_row;
            break;
        }
    }

    let row = triggered_row.expect("INC HL should trigger during Mode 2");
    assert!(row > 0);
    machine.write_bus(0xFF40, 0x00);

    assert_eq!(
        read_oam_corruption_row(&mut machine, row),
        expected_write_corruption(&rows, row)
    );
}
