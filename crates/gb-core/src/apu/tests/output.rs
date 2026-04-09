use super::*;

#[test]
fn enabled_dac_output_remains_distinct_from_dac_off_even_when_the_channel_is_inactive() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);

    apu.write_register(0xFF12, 0x08);
    let enabled_snapshot = apu.snapshot();

    assert_eq!(enabled_snapshot.output.channel_digital_outputs[0], 0);
    assert_eq!(enabled_snapshot.output.channel_dac_outputs[0], ANALOG_ONE);
    assert_eq!(enabled_snapshot.channel_dac_mask, CHANNEL_ACTIVE_CH1);
    assert_eq!(enabled_snapshot.channel_active_mask, 0x00);

    apu.write_register(0xFF12, 0x00);
    let disabled_snapshot = apu.snapshot();

    assert_eq!(disabled_snapshot.output.channel_digital_outputs[0], 0);
    assert_eq!(disabled_snapshot.channel_dac_mask, 0x00);
    assert_eq!(disabled_snapshot.channel_active_mask, 0x00);
}

#[test]
fn disabling_the_last_dac_disconnects_the_output_immediately() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF12, 0x08);
    apu.write_register(0xFF24, 0x00);
    apu.write_register(0xFF25, 0x11);

    tick_apu_with_edges(&mut apu, 0, &[]);
    let charged = apu.snapshot().output;
    assert!(charged.hpf_capacitor.left > 0);
    assert!(charged.hpf_capacitor.right > 0);

    apu.write_register(0xFF12, 0x00);
    let dac_off = apu.snapshot().output;

    assert_eq!(dac_off.channel_dac_outputs[0], 0);
    assert_eq!(dac_off.hpf_output.left, 0);
    assert_eq!(dac_off.hpf_output.right, 0);
    assert_eq!(dac_off.hpf_capacitor, charged.hpf_capacitor);
}

#[test]
fn hpf_capacitor_freezes_while_all_dacs_are_off() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF12, 0x08);
    apu.write_register(0xFF24, 0x00);
    apu.write_register(0xFF25, 0x11);

    tick_apu_with_edges(&mut apu, 0, &[]);
    apu.write_register(0xFF12, 0x00);
    let after_write = apu.snapshot().output;

    tick_apu_with_edges(&mut apu, 1, &[]);
    let after_first_dac_off_tick = apu.snapshot().output;
    tick_apu_with_edges(&mut apu, 2, &[]);
    let after_second_dac_off_tick = apu.snapshot().output;

    assert_eq!(after_write.hpf_output.left, 0);
    assert_eq!(after_write.hpf_output.right, 0);
    assert_eq!(after_first_dac_off_tick.hpf_output.left, 0);
    assert_eq!(after_first_dac_off_tick.hpf_output.right, 0);
    assert_eq!(after_second_dac_off_tick.hpf_output.left, 0);
    assert_eq!(after_second_dac_off_tick.hpf_output.right, 0);
    assert_eq!(
        after_first_dac_off_tick.hpf_capacitor,
        after_write.hpf_capacitor
    );
    assert_eq!(
        after_second_dac_off_tick.hpf_capacitor,
        after_write.hpf_capacitor
    );
}

#[test]
fn routed_nonzero_vin_does_not_keep_the_output_path_connected_without_channel_dacs() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.master.vin_input = ApuStereoOutputSnapshot::new(ANALOG_ONE, ANALOG_ONE / 2);
    apu.write_register(0xFF24, NR50_VIN_LEFT_BIT | NR50_VIN_RIGHT_BIT);

    let routed = apu.snapshot().output;
    assert_eq!(routed.channel_dac_outputs, [0, 0, 0, 0]);
    assert_eq!(
        routed.vin_analog_output,
        ApuStereoOutputSnapshot::new(ANALOG_ONE, ANALOG_ONE / 2)
    );
    assert_eq!(routed.mixer_output.left, ANALOG_ONE);
    assert_eq!(routed.mixer_output.right, ANALOG_ONE / 2);
    assert_eq!(routed.master_output.left, 0);
    assert_eq!(routed.master_output.right, 0);
    assert_eq!(routed.hpf_output.left, 0);
    assert_eq!(routed.hpf_output.right, 0);
    assert_eq!(routed.hpf_capacitor.left, 0);
    assert_eq!(routed.hpf_capacitor.right, 0);

    tick_apu_with_edges(&mut apu, 0, &[]);
    let settled = apu.snapshot().output;
    assert_eq!(settled.hpf_output.left, 0);
    assert_eq!(settled.hpf_output.right, 0);
    assert_eq!(settled.hpf_capacitor.left, 0);
    assert_eq!(settled.hpf_capacitor.right, 0);
}

#[test]
fn nr51_routes_channel_dac_outputs_independently_to_left_and_right_buses() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF12, 0x08);
    apu.write_register(0xFF17, 0x08);
    apu.write_register(0xFF25, 0x12);

    let snapshot = apu.snapshot();

    assert_eq!(
        snapshot.output.channel_dac_outputs,
        [ANALOG_ONE, ANALOG_ONE, 0, 0]
    );
    assert_eq!(snapshot.output.vin_analog_output.left, 0);
    assert_eq!(snapshot.output.vin_analog_output.right, 0);
    assert_eq!(snapshot.output.mixer_output.left, ANALOG_ONE);
    assert_eq!(snapshot.output.mixer_output.right, ANALOG_ONE);
    assert_eq!(snapshot.output.master_output.left, ANALOG_ONE);
    assert_eq!(snapshot.output.master_output.right, ANALOG_ONE);
}

#[test]
fn nr50_vin_bits_route_the_explicit_neutral_vin_lane_without_altering_channel_mix() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF12, 0x08);
    apu.write_register(0xFF25, 0x11);
    apu.write_register(0xFF24, 0x00);

    let baseline = apu.snapshot().output;
    assert_eq!(baseline.vin_analog_output.left, 0);
    assert_eq!(baseline.vin_analog_output.right, 0);
    assert_eq!(baseline.master_output.left, ANALOG_ONE);
    assert_eq!(baseline.master_output.right, ANALOG_ONE);

    apu.write_register(0xFF24, NR50_VIN_LEFT_BIT | NR50_VIN_RIGHT_BIT);
    let vin_routed = apu.snapshot().output;

    assert_eq!(vin_routed.vin_analog_output.left, 0);
    assert_eq!(vin_routed.vin_analog_output.right, 0);
    assert_eq!(vin_routed.mixer_output.left, baseline.mixer_output.left);
    assert_eq!(vin_routed.mixer_output.right, baseline.mixer_output.right);
    assert_eq!(vin_routed.master_output.left, baseline.master_output.left);
    assert_eq!(vin_routed.master_output.right, baseline.master_output.right);
}

#[test]
fn nr50_volume_zero_still_scales_by_one_and_seven_scales_by_eight() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF12, 0x08);
    apu.write_register(0xFF25, 0x11);

    apu.write_register(0xFF24, 0x00);
    let quiet_snapshot = apu.snapshot();
    assert_eq!(quiet_snapshot.output.master_output.left, ANALOG_ONE);
    assert_eq!(quiet_snapshot.output.master_output.right, ANALOG_ONE);

    apu.write_register(0xFF24, 0x77);
    let loud_snapshot = apu.snapshot();
    assert_eq!(loud_snapshot.output.master_output.left, ANALOG_ONE * 8);
    assert_eq!(loud_snapshot.output.master_output.right, ANALOG_ONE * 8);
}

#[test]
fn hpf_state_persists_across_t_cycles_and_pulls_the_output_towards_zero() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF12, 0x08);
    apu.write_register(0xFF24, 0x00);
    apu.write_register(0xFF25, 0x11);

    let before = apu.snapshot().output;
    assert_eq!(before.hpf_output.left, ANALOG_ONE);
    assert_eq!(before.hpf_output.right, ANALOG_ONE);
    assert_eq!(before.hpf_capacitor.left, 0);
    assert_eq!(before.hpf_capacitor.right, 0);

    tick_apu_with_edges(&mut apu, 0, &[]);
    let after_first_tick = apu.snapshot().output;
    assert_eq!(after_first_tick.hpf_output.left, ANALOG_ONE);
    assert_eq!(after_first_tick.hpf_output.right, ANALOG_ONE);
    assert!(after_first_tick.hpf_capacitor.left > 0);
    assert!(after_first_tick.hpf_capacitor.right > 0);

    tick_apu_with_edges(&mut apu, 1, &[]);
    let after_second_tick = apu.snapshot().output;
    assert!(after_second_tick.hpf_output.left < after_first_tick.hpf_output.left);
    assert!(after_second_tick.hpf_output.right < after_first_tick.hpf_output.right);
    assert!(after_second_tick.hpf_capacitor.left > after_first_tick.hpf_capacitor.left);
    assert!(after_second_tick.hpf_capacitor.right > after_first_tick.hpf_capacitor.right);
}

#[test]
fn output_path_selects_the_expected_hpf_charge_model_per_console_model() {
    assert_eq!(
        Apu::new(ConsoleModel::Dmg0).output_path.hpf_charge_model,
        HpfChargeModel::Dmg0Dmg
    );
    assert_eq!(
        Apu::new(ConsoleModel::Dmg).output_path.hpf_charge_model,
        HpfChargeModel::Dmg0Dmg
    );
    assert_eq!(
        Apu::new(ConsoleModel::Mgb).output_path.hpf_charge_model,
        HpfChargeModel::MgbCgb
    );
    assert_eq!(
        Apu::new(ConsoleModel::Cgb).output_path.hpf_charge_model,
        HpfChargeModel::MgbCgb
    );
}

#[test]
fn extra_length_clocking_policy_is_explicitly_revision_deferred_for_cgb() {
    assert_eq!(
        extra_length_clocking_policy(ConsoleModel::Dmg0),
        ExtraLengthClockingPolicy::CurrentDmgBaseline
    );
    assert_eq!(
        extra_length_clocking_policy(ConsoleModel::Dmg),
        ExtraLengthClockingPolicy::CurrentDmgBaseline
    );
    assert_eq!(
        extra_length_clocking_policy(ConsoleModel::Mgb),
        ExtraLengthClockingPolicy::CurrentDmgBaseline
    );
    assert_eq!(
        extra_length_clocking_policy(ConsoleModel::Cgb),
        ExtraLengthClockingPolicy::DeferredCgbRevisionBehavior
    );
}

#[test]
fn cgb_hpf_settles_more_aggressively_than_dmg() {
    let mut dmg = Apu::new(ConsoleModel::Dmg);
    let mut cgb = Apu::new(ConsoleModel::Cgb);

    for apu in [&mut dmg, &mut cgb] {
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF12, 0x08);
        apu.write_register(0xFF24, 0x00);
        apu.write_register(0xFF25, 0x11);
    }

    tick_apu_with_edges(&mut dmg, 0, &[]);
    tick_apu_with_edges(&mut cgb, 0, &[]);

    let dmg_after_first_tick = dmg.snapshot().output;
    let cgb_after_first_tick = cgb.snapshot().output;
    assert!(cgb_after_first_tick.hpf_capacitor.left > dmg_after_first_tick.hpf_capacitor.left);
    assert!(cgb_after_first_tick.hpf_capacitor.right > dmg_after_first_tick.hpf_capacitor.right);

    tick_apu_with_edges(&mut dmg, 1, &[]);
    tick_apu_with_edges(&mut cgb, 1, &[]);

    let dmg_after_second_tick = dmg.snapshot().output;
    let cgb_after_second_tick = cgb.snapshot().output;
    assert!(cgb_after_second_tick.hpf_output.left < dmg_after_second_tick.hpf_output.left);
    assert!(cgb_after_second_tick.hpf_output.right < dmg_after_second_tick.hpf_output.right);
}

#[test]
fn host_output_sample_matches_the_live_post_hpf_output_snapshot() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF12, 0x08);
    apu.write_register(0xFF24, 0x77);
    apu.write_register(0xFF25, 0x11);

    let output_snapshot = apu.snapshot().output.hpf_output;

    assert_eq!(
        apu.host_output_sample(),
        ApuHostSample {
            left: output_snapshot.left,
            right: output_snapshot.right,
        }
    );
}

#[test]
fn sample_capture_rejects_zero_sample_rate() {
    assert_eq!(
        ApuSampleCapture::new(0).expect_err("zero sample rate must fail"),
        ApuSampleCaptureError::OutputSampleRateZero
    );
}

#[test]
fn sample_capture_can_emit_one_sample_per_t_cycle() {
    let mut capture =
        ApuSampleCapture::new(DMG_FAMILY_APU_CAPTURE_CLOCK_HZ).expect("valid sample rate");

    capture.record_output_t_cycle(ApuHostSample { left: 1, right: -1 });
    capture.record_output_t_cycle(ApuHostSample { left: 2, right: -2 });
    capture.record_output_t_cycle(ApuHostSample { left: 3, right: -3 });

    assert_eq!(
        capture.drain_samples(),
        vec![
            ApuHostSample { left: 1, right: -1 },
            ApuHostSample { left: 2, right: -2 },
            ApuHostSample { left: 3, right: -3 },
        ]
    );
}

#[test]
fn sample_capture_emits_samples_at_the_requested_fractional_rate() {
    let mut capture =
        ApuSampleCapture::new(DMG_FAMILY_APU_CAPTURE_CLOCK_HZ / 4).expect("valid sample rate");

    for sample_index in 0..8 {
        capture.record_output_t_cycle(ApuHostSample {
            left: sample_index,
            right: -sample_index,
        });
    }

    assert_eq!(
        capture.drain_samples(),
        vec![
            ApuHostSample { left: 3, right: -3 },
            ApuHostSample { left: 7, right: -7 },
        ]
    );
}

#[test]
fn sample_capture_produces_the_exact_requested_sample_count_over_one_second() {
    let mut capture = ApuSampleCapture::new(48_000).expect("valid sample rate");

    for _ in 0..DMG_FAMILY_APU_CAPTURE_CLOCK_HZ {
        capture.record_output_t_cycle(ApuHostSample::default());
    }

    assert_eq!(capture.pending_sample_count(), 48_000);
    assert_eq!(capture.drain_samples().len(), 48_000);
}

#[test]
fn sample_capture_can_drain_into_a_reusable_buffer() {
    let mut capture =
        ApuSampleCapture::new(DMG_FAMILY_APU_CAPTURE_CLOCK_HZ).expect("valid sample rate");
    let mut reusable_buffer = vec![ApuHostSample {
        left: 99,
        right: -99,
    }];

    capture.record_output_t_cycle(ApuHostSample { left: 7, right: -7 });
    capture.record_output_t_cycle(ApuHostSample { left: 8, right: -8 });

    capture.drain_samples_into(&mut reusable_buffer);

    assert_eq!(
        reusable_buffer,
        vec![
            ApuHostSample { left: 7, right: -7 },
            ApuHostSample { left: 8, right: -8 },
        ]
    );
    assert_eq!(capture.pending_sample_count(), 0);
}

#[test]
fn mixer_and_hpf_output_change_immediately_when_routing_changes() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF12, 0x08);
    apu.write_register(0xFF24, 0x00);
    apu.write_register(0xFF25, 0x01);

    let right_only = apu.snapshot().output;
    assert_eq!(right_only.master_output.left, 0);
    assert_eq!(right_only.master_output.right, ANALOG_ONE);
    assert_eq!(right_only.hpf_output.left, 0);
    assert_eq!(right_only.hpf_output.right, ANALOG_ONE);

    apu.write_register(0xFF25, 0x10);
    let left_only = apu.snapshot().output;
    assert_eq!(left_only.master_output.left, ANALOG_ONE);
    assert_eq!(left_only.master_output.right, 0);
    assert_eq!(left_only.hpf_output.left, ANALOG_ONE);
    assert_eq!(left_only.hpf_output.right, 0);
}
