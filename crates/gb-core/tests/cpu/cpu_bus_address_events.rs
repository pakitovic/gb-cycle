use super::*;

#[test]
fn machine_executes_ldh_a8_reads_and_writes_through_ff00_offset() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(
            &[0x3E, 0x34, 0xE0, 0x01, 0x3E, 0x00, 0xF0, 0x01],
            0x12,
        ))
        .expect("NoMBC test ROM should load");

    step_machine_t_cycles(&mut machine, 40);

    assert_eq!(machine.read_bus(0xFF01), 0x34);
    assert_eq!(machine.cpu().registers().a, 0x34);
    assert_eq!(machine.cpu().registers().pc, 0x0108);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn machine_executes_ld_ff00_plus_c_reads_and_writes_through_the_same_bus_path() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(
            &[0x0E, 0x01, 0x3E, 0x56, 0xE2, 0x3E, 0x00, 0xF2],
            0x12,
        ))
        .expect("NoMBC test ROM should load");

    step_machine_t_cycles(&mut machine, 40);

    assert_eq!(machine.read_bus(0xFF01), 0x56);
    assert_eq!(machine.cpu().registers().a, 0x56);
    assert_eq!(machine.cpu().registers().pc, 0x0108);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn machine_exposes_hli_hld_and_incdec_address_events_through_the_public_cpu_api() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(
            &[
                0x21, 0x00, 0xC0, 0x2A, 0x21, 0x01, 0xC0, 0x32, 0x21, 0xFF, 0xFE, 0x23,
            ],
            0x12,
        ))
        .expect("NoMBC test ROM should load");
    machine.write_bus(0xC000, 0x77);

    step_machine_t_cycles(&mut machine, 20);

    assert_eq!(machine.cpu().registers().a, 0x77);
    assert_eq!(
        machine.cpu().last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::ReadWithIncDec,
            access_address: Some(0xC000),
            idu_address: Some(0xC001),
            update_direction: Some(CpuAddressUpdateDirection::Increment),
        })
    );

    step_machine_t_cycles(&mut machine, 20);

    assert_eq!(machine.read_bus(0xC001), 0x77);
    assert_eq!(
        machine.cpu().last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::WriteWithIncDec,
            access_address: Some(0xC001),
            idu_address: Some(0xC000),
            update_direction: Some(CpuAddressUpdateDirection::Decrement),
        })
    );

    step_machine_t_cycles(&mut machine, 20);

    assert_eq!(machine.cpu().registers().h, 0xFF);
    assert_eq!(machine.cpu().registers().l, 0x00);
    assert_eq!(
        machine.cpu().last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::IncDec,
            access_address: None,
            idu_address: Some(0xFF00),
            update_direction: Some(CpuAddressUpdateDirection::Increment),
        })
    );
}

#[test]
fn cpu_trace_mentions_the_last_address_event_next_to_the_last_bus_activity() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x00], 0x12))
        .expect("NoMBC test ROM should load");

    step_machine_t_cycles(&mut machine, 4);

    let trace = machine.tracer().sink().render_text();

    assert!(trace.contains("last_bus_activity=opcode_fetch@0x0100=0x00"));
    assert!(trace.contains("last_address_event=read+inc@0x0100->0x0101"));
}
