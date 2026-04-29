use super::*;

#[test]
fn nr51_bus_writes_retarget_the_live_analog_mix_immediately() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
fn nr50_vin_bus_bits_route_a_neutral_lane_without_perturbing_the_live_mix() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF26, 0x00);
    machine.write_bus(0xFF26, 0x80);
    machine.write_bus(0xFF12, 0x08);
    machine.write_bus(0xFF25, 0x11);
    machine.write_bus(0xFF24, 0x00);

    let baseline = machine.apu().snapshot().output;
    assert_eq!(baseline.vin_analog_output.left, 0);
    assert_eq!(baseline.vin_analog_output.right, 0);
    assert_eq!(baseline.master_output.left, 15_000_000);
    assert_eq!(baseline.master_output.right, 15_000_000);

    machine.write_bus(0xFF24, 0x88);

    let vin_routed = machine.apu().snapshot().output;
    assert_eq!(vin_routed.vin_analog_output.left, 0);
    assert_eq!(vin_routed.vin_analog_output.right, 0);
    assert_eq!(vin_routed.master_output.left, baseline.master_output.left);
    assert_eq!(vin_routed.master_output.right, baseline.master_output.right);
    assert_eq!(vin_routed.hpf_output.left, baseline.hpf_output.left);
    assert_eq!(vin_routed.hpf_output.right, baseline.hpf_output.right);
}

#[test]
fn host_side_snapshot_capture_cadence_does_not_feed_back_into_apu_state() {
    let mut baseline = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    let mut observed = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
