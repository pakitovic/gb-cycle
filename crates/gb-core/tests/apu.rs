use gb_core::{ConsoleModel, Machine, MachineConfig, StartupMode, WaveRamStartupPolicy};

fn step_until_next_div_apu_edge(machine: &mut Machine) {
    let starting_phase = machine.apu().snapshot().div_apu;

    for _ in 0..=0x2000 {
        machine.step_t_cycle();
        if machine.apu().snapshot().div_apu != starting_phase {
            return;
        }
    }

    panic!("expected the shared divider to reach the next APU frame-sequencer edge");
}

#[test]
fn skip_boot_exposes_the_published_post_boot_audio_readback() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    let cases = [
        (0xFF10, 0x80),
        (0xFF11, 0xBF),
        (0xFF12, 0xF3),
        (0xFF13, 0xFF),
        (0xFF14, 0xBF),
        (0xFF16, 0x3F),
        (0xFF17, 0x00),
        (0xFF18, 0xFF),
        (0xFF19, 0xBF),
        (0xFF1A, 0x7F),
        (0xFF1B, 0xFF),
        (0xFF1C, 0x9F),
        (0xFF1D, 0xFF),
        (0xFF1E, 0xBF),
        (0xFF20, 0xFF),
        (0xFF21, 0x00),
        (0xFF22, 0x00),
        (0xFF23, 0xBF),
        (0xFF24, 0x77),
        (0xFF25, 0xF3),
        (0xFF26, 0xF1),
        (0xFF30, 0x00),
    ];

    for (address, expected) in cases {
        assert_eq!(
            machine.read_bus(address),
            expected,
            "unexpected value at {address:#06X}"
        );
    }

    let snapshot = machine.apu().snapshot();
    assert!(snapshot.powered);
    assert_eq!(snapshot.channel_active_mask, 0x01);
    assert_eq!(snapshot.channel_dac_mask, 0x01);
    assert_eq!(snapshot.div_apu, 0x05);
    assert_eq!(
        snapshot.wave_ram_startup_policy,
        WaveRamStartupPolicy::DeterministicZeroed
    );
}

#[test]
fn nr52_power_off_clears_audio_registers_but_preserves_wave_ram_through_the_machine_bus() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF30, 0x12);
    machine.write_bus(0xFF31, 0x34);
    machine.write_bus(0xFF12, 0xF3);
    machine.write_bus(0xFF14, 0x80);
    machine.write_bus(0xFF24, 0x77);
    machine.write_bus(0xFF25, 0xF3);

    machine.write_bus(0xFF26, 0x00);

    assert_eq!(machine.read_bus(0xFF26), 0x70);
    assert_eq!(machine.read_bus(0xFF12), 0x00);
    assert_eq!(machine.read_bus(0xFF14), 0xBF);
    assert_eq!(machine.read_bus(0xFF24), 0x00);
    assert_eq!(machine.read_bus(0xFF25), 0x00);
    assert_eq!(machine.read_bus(0xFF30), 0x12);
    assert_eq!(machine.read_bus(0xFF31), 0x34);

    machine.write_bus(0xFF12, 0xF3);
    machine.write_bus(0xFF24, 0x77);
    machine.write_bus(0xFF25, 0xF3);
    assert_eq!(machine.read_bus(0xFF12), 0x00);
    assert_eq!(machine.read_bus(0xFF24), 0x00);
    assert_eq!(machine.read_bus(0xFF25), 0x00);

    machine.write_bus(0xFF26, 0x80);
    machine.write_bus(0xFF12, 0xF3);
    assert_eq!(machine.read_bus(0xFF12), 0xF3);
}

#[test]
fn channel_3_wave_ram_survives_nr52_power_off_after_the_channel_has_been_started() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF30, 0x12);
    machine.write_bus(0xFF31, 0x34);
    machine.write_bus(0xFF1A, 0x80);
    machine.write_bus(0xFF1C, 0x20);
    machine.write_bus(0xFF1E, 0x80);

    assert_eq!(machine.read_bus(0xFF26) & 0x04, 0x04);

    machine.write_bus(0xFF26, 0x00);

    assert_eq!(machine.read_bus(0xFF1A), 0x7F);
    assert_eq!(machine.read_bus(0xFF1C), 0x9F);
    assert_eq!(machine.read_bus(0xFF1E), 0xBF);
    assert_eq!(machine.read_bus(0xFF26) & 0x04, 0x00);
    assert_eq!(machine.read_bus(0xFF30), 0x12);
    assert_eq!(machine.read_bus(0xFF31), 0x34);
}

#[test]
fn dmg_powered_off_nr41_length_write_survives_power_cycle_on_the_machine_bus() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
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
fn nr52_channel_status_follows_trigger_and_dac_disable_through_the_machine_bus() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF26, 0x00);
    machine.write_bus(0xFF26, 0x80);

    assert_eq!(machine.read_bus(0xFF26), 0xF0);
    assert_eq!(machine.apu().snapshot().channel_dac_mask, 0x00);

    machine.write_bus(0xFF12, 0xF3);
    assert_eq!(machine.read_bus(0xFF26), 0xF0);
    assert_eq!(machine.apu().snapshot().channel_dac_mask, 0x01);
    assert_eq!(machine.apu().snapshot().channel_active_mask, 0x00);

    machine.write_bus(0xFF14, 0x80);
    assert_eq!(machine.read_bus(0xFF26), 0xF1);
    assert_eq!(machine.apu().snapshot().channel_dac_mask, 0x01);
    assert_eq!(machine.apu().snapshot().channel_active_mask, 0x01);

    machine.write_bus(0xFF12, 0x00);
    assert_eq!(machine.read_bus(0xFF26), 0xF0);
    assert_eq!(machine.apu().snapshot().channel_dac_mask, 0x00);
    assert_eq!(machine.apu().snapshot().channel_active_mask, 0x00);
}

#[test]
fn nr51_bus_writes_retarget_the_live_analog_mix_immediately() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF26, 0x00);
    machine.write_bus(0xFF26, 0x80);
    machine.write_bus(0xFF12, 0x08);
    machine.write_bus(0xFF24, 0x00);
    machine.write_bus(0xFF25, 0x01);

    let right_only = machine.apu().snapshot().output;
    assert_eq!(right_only.master_output.left, 0);
    assert_eq!(right_only.master_output.right, 15_000_000);
    assert_eq!(right_only.hpf_output.left, 0);
    assert_eq!(right_only.hpf_output.right, 15_000_000);

    machine.write_bus(0xFF25, 0x10);

    let left_only = machine.apu().snapshot().output;
    assert_eq!(left_only.master_output.left, 15_000_000);
    assert_eq!(left_only.master_output.right, 0);
    assert_eq!(left_only.hpf_output.left, 15_000_000);
    assert_eq!(left_only.hpf_output.right, 0);
}

#[test]
fn host_side_snapshot_capture_cadence_does_not_feed_back_into_apu_state() {
    let mut baseline = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    let mut observed = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    for machine in [&mut baseline, &mut observed] {
        machine.write_bus(0xFF26, 0x00);
        machine.write_bus(0xFF26, 0x80);
        machine.write_bus(0xFF12, 0x08);
        machine.write_bus(0xFF17, 0x08);
        machine.write_bus(0xFF24, 0x77);
        machine.write_bus(0xFF25, 0x33);
    }

    for step in 0..128 {
        baseline.step_t_cycle();

        if step % 3 == 0 {
            let _ = observed.apu().snapshot();
        }
        if step % 5 == 0 {
            let _ = observed.snapshot();
        }

        observed.step_t_cycle();

        if step % 7 == 0 {
            let _ = observed.apu().snapshot();
        }
    }

    assert_eq!(baseline.apu().snapshot(), observed.apu().snapshot());
}

#[test]
fn skip_boot_div_apu_phase_matches_the_shared_divider_entry_and_next_edge() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
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
fn powering_on_apu_while_the_div_apu_source_bit_is_high_keeps_the_next_live_frame_edge() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
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
