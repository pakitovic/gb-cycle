use super::*;

#[test]
fn dmg_powered_off_nr41_length_write_survives_power_cycle_on_the_machine_bus() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF26, 0x00);
    machine.write_bus(0xFF20, 0x3E);
    machine.write_bus(0xFF26, 0x80);
    machine.write_bus(0xFF21, 0x08);
    machine.write_bus(0xFF23, 0xC0);

    assert_eq!(machine.read_bus(0xFF26) & 0x08, 0x08);

    step_until_next_div_apu_edge(&mut machine);
    assert_eq!(machine.read_bus(0xFF26) & 0x08, 0x08);

    step_until_next_div_apu_edge(&mut machine);
    assert_eq!(machine.read_bus(0xFF26) & 0x08, 0x08);

    step_until_next_div_apu_edge(&mut machine);
    assert_eq!(machine.read_bus(0xFF26) & 0x08, 0x00);
}

#[test]
fn skip_boot_div_apu_phase_matches_the_shared_divider_entry_and_next_edge() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    let initial_div_apu = machine.apu().snapshot().div_apu;
    let initial_system_counter = machine.timer().snapshot().system_counter;
    let remaining_until_next_edge = 0x2000 - (initial_system_counter & 0x1FFF);

    assert_eq!(initial_div_apu, 0x05);
    assert_eq!(remaining_until_next_edge, 0x1438);

    for _ in 0..remaining_until_next_edge - 1 {
        machine.step_t_cycle();
    }

    assert_eq!(machine.apu().snapshot().div_apu, initial_div_apu);

    machine.step_t_cycle();

    assert_eq!(
        machine.apu().snapshot().div_apu,
        (initial_div_apu + 1) & 0x07
    );
}

#[test]
fn div_write_can_advance_div_apu_immediately_when_it_resets_a_high_source_bit() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    while machine.timer().snapshot().system_counter & 0x1000 == 0 {
        machine.step_t_cycle();
    }

    let before = machine.apu().snapshot().div_apu;

    machine.write_bus(0xFF04, 0x00);

    assert_eq!(machine.timer().snapshot().system_counter, 0x0000);
    assert_eq!(machine.apu().snapshot().div_apu, (before + 1) & 0x07);
}

#[test]
fn powering_on_apu_keeps_the_next_live_frame_edge() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF26, 0x00);
    while machine.timer().snapshot().system_counter & 0x1000 == 0 {
        machine.step_t_cycle();
    }

    let remaining_until_next_edge = 0x2000 - (machine.timer().snapshot().system_counter & 0x1FFF);

    assert_ne!(machine.timer().snapshot().system_counter & 0x1000, 0);

    machine.write_bus(0xFF26, 0x80);
    assert_eq!(machine.apu().snapshot().div_apu, 0x00);

    for _ in 0..remaining_until_next_edge {
        machine.step_t_cycle();
    }

    assert_eq!(machine.apu().snapshot().div_apu, 0x01);

    for _ in 0..0x2000 {
        machine.step_t_cycle();
    }

    assert_eq!(machine.apu().snapshot().div_apu, 0x02);
}

#[test]
fn pulse_channel_length_expiry_clears_nr52_bits_on_the_next_length_clock() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF26, 0x00);
    machine.write_bus(0xFF26, 0x80);

    machine.write_bus(0xFF11, 0xBE);
    machine.write_bus(0xFF12, 0xF1);
    machine.write_bus(0xFF14, 0x40);
    machine.write_bus(0xFF14, 0xC0);
    machine.write_bus(0xFF16, 0x3E);
    machine.write_bus(0xFF17, 0xF1);
    machine.write_bus(0xFF19, 0x40);
    machine.write_bus(0xFF19, 0xC0);

    assert_eq!(machine.apu().snapshot().div_apu, 0x00);
    assert_eq!(machine.read_bus(0xFF26) & 0x03, 0x03);

    step_until_next_div_apu_edge(&mut machine);
    assert_eq!(machine.read_bus(0xFF26) & 0x03, 0x03);

    step_until_next_div_apu_edge(&mut machine);
    assert_eq!(machine.read_bus(0xFF26) & 0x03, 0x03);

    step_until_next_div_apu_edge(&mut machine);
    assert_eq!(machine.read_bus(0xFF26) & 0x03, 0x00);
}

#[test]
fn channel_1_sweep_overflow_clears_nr52_on_the_frame_sequencer_timeline() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF26, 0x00);
    machine.write_bus(0xFF26, 0x80);

    machine.write_bus(0xFF10, 0x11);
    machine.write_bus(0xFF11, 0x80);
    machine.write_bus(0xFF12, 0xF0);
    machine.write_bus(0xFF13, 0x00);
    machine.write_bus(0xFF14, 0x85);

    assert_eq!(machine.apu().snapshot().div_apu, 0x00);
    assert_eq!(machine.read_bus(0xFF26) & 0x01, 0x01);

    step_until_next_div_apu_edge(&mut machine);
    assert_eq!(machine.read_bus(0xFF26) & 0x01, 0x01);

    step_until_next_div_apu_edge(&mut machine);
    assert_eq!(machine.read_bus(0xFF26) & 0x01, 0x01);

    step_until_next_div_apu_edge(&mut machine);
    assert_eq!(machine.read_bus(0xFF26) & 0x01, 0x00);
}

#[test]
fn channel_3_length_expiry_clears_nr52_on_the_shared_length_clock() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF26, 0x00);
    machine.write_bus(0xFF26, 0x80);

    machine.write_bus(0xFF1A, 0x80);
    machine.write_bus(0xFF1B, 0xFF);
    machine.write_bus(0xFF1C, 0x20);
    machine.write_bus(0xFF1E, 0xC0);

    assert_eq!(machine.apu().snapshot().div_apu, 0x00);
    assert_eq!(machine.read_bus(0xFF26) & 0x04, 0x04);

    step_until_next_div_apu_edge(&mut machine);
    assert_eq!(machine.read_bus(0xFF26) & 0x04, 0x00);
}
