use super::*;
use crate::scheduler::TCycle;

fn tick_apu_with_edges(apu: &mut Apu, t_cycle: u64, edges: &[DerivedEdge]) {
    let mut context = CycleContext::for_cycle(TCycle::new(t_cycle));
    for &edge in edges {
        context.push_derived_edge(edge);
    }
    apu.tick_t_cycle(&context);
}

const fn pulse_length_load(counter: u8) -> u8 {
    0xC0 | ((PULSE_LENGTH_COUNTER_RELOAD - counter) & PULSE_LENGTH_LOAD_MASK)
}

fn prime_channel_2_trigger_test(apu: &mut Apu, length_counter: u8) {
    apu.write_register(0xFF17, 0x08);
    apu.write_register(0xFF16, pulse_length_load(PULSE_LENGTH_COUNTER_RELOAD));
    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT);
    apu.write_register(0xFF16, pulse_length_load(length_counter));
}

fn prime_channel_1_trigger_test(apu: &mut Apu, length_counter: u8) {
    apu.write_register(0xFF12, 0x08);
    apu.write_register(0xFF11, pulse_length_load(PULSE_LENGTH_COUNTER_RELOAD));
    apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT);
    apu.write_register(0xFF11, pulse_length_load(length_counter));
}

#[test]
fn nr52_tracks_channel_active_state_separately_from_dac_state() {
    let mut apu = Apu::new(ConsoleModel::Dmg);

    apu.write_register(0xFF26, 0x80);

    assert_eq!(apu.read_register(0xFF26), 0xF0);
    assert_eq!(apu.snapshot().channel_dac_mask, 0x00);

    apu.write_register(0xFF12, 0xF3);
    assert_eq!(apu.read_register(0xFF26), 0xF0);
    assert_eq!(apu.snapshot().channel_dac_mask, CHANNEL_ACTIVE_CH1);
    assert_eq!(apu.snapshot().channel_active_mask, 0x00);

    apu.write_register(0xFF14, 0x80);
    assert_eq!(apu.read_register(0xFF26), 0xF1);
    assert_eq!(apu.snapshot().channel_dac_mask, CHANNEL_ACTIVE_CH1);
    assert_eq!(apu.snapshot().channel_active_mask, CHANNEL_ACTIVE_CH1);

    apu.write_register(0xFF12, 0x00);
    assert_eq!(apu.read_register(0xFF26), 0xF0);
    assert_eq!(apu.snapshot().channel_dac_mask, 0x00);
    assert_eq!(apu.snapshot().channel_active_mask, 0x00);
}

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
    assert_eq!(routed.master_output.left, ANALOG_ONE);
    assert_eq!(routed.master_output.right, ANALOG_ONE / 2);
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

#[test]
fn audio_register_readback_keeps_write_only_and_mixed_fields_explicit() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);

    apu.write_register(0xFF10, 0x00);
    apu.write_register(0xFF11, 0x00);
    apu.write_register(0xFF13, 0x12);
    apu.write_register(0xFF14, 0x40);
    apu.write_register(0xFF1C, 0x00);
    apu.write_register(0xFF20, 0x34);
    apu.write_register(0xFF23, 0x00);

    assert_eq!(apu.read_register(0xFF10), 0x80);
    assert_eq!(apu.read_register(0xFF11), 0x3F);
    assert_eq!(apu.read_register(0xFF13), 0xFF);
    assert_eq!(apu.read_register(0xFF14), 0xFF);
    assert_eq!(apu.read_register(0xFF1C), 0x9F);
    assert_eq!(apu.read_register(0xFF20), 0xFF);
    assert_eq!(apu.read_register(0xFF23), 0xBF);
    assert_eq!(apu.read_register(0xFF15), 0xFF);
}

#[test]
fn nr52_power_off_clears_audio_registers_but_preserves_wave_ram() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF12, 0xF3);
    apu.write_register(0xFF14, 0x80);
    apu.write_register(0xFF24, 0x77);
    apu.write_register(0xFF25, 0xF3);
    apu.write_register(0xFF30, 0x12);
    apu.write_register(0xFF31, 0x34);

    apu.write_register(0xFF26, 0x00);

    assert_eq!(apu.read_register(0xFF26), 0x70);
    assert_eq!(apu.read_register(0xFF12), 0x00);
    assert_eq!(apu.read_register(0xFF14), 0xBF);
    assert_eq!(apu.read_register(0xFF24), 0x00);
    assert_eq!(apu.read_register(0xFF25), 0x00);
    assert_eq!(apu.read_register(0xFF30), 0x12);
    assert_eq!(apu.read_register(0xFF31), 0x34);

    apu.write_register(0xFF12, 0xF3);
    apu.write_register(0xFF24, 0x77);
    apu.write_register(0xFF25, 0xF3);
    assert_eq!(apu.read_register(0xFF12), 0x00);
    assert_eq!(apu.read_register(0xFF24), 0x00);
    assert_eq!(apu.read_register(0xFF25), 0x00);
}

#[test]
fn frame_sequencer_advances_only_on_the_shared_div_apu_edge() {
    let mut apu = Apu::new(ConsoleModel::Dmg);

    tick_apu_with_edges(&mut apu, 0, &[]);
    assert_eq!(apu.snapshot().div_apu, 0x00);

    tick_apu_with_edges(&mut apu, 1, &[DerivedEdge::DividerTick]);
    assert_eq!(apu.snapshot().div_apu, 0x00);

    tick_apu_with_edges(&mut apu, 2, &[DerivedEdge::ApuFrameSequencerEdge]);
    assert_eq!(apu.snapshot().div_apu, 0x01);
    assert_eq!(apu.frame_sequencer.length_clock_count, 1);
    assert_eq!(apu.frame_sequencer.sweep_clock_count, 0);
    assert_eq!(apu.frame_sequencer.envelope_clock_count, 0);
}

#[test]
fn powering_on_with_the_div_apu_source_high_keeps_waiting_for_the_next_live_edge() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.apply_startup_state(ApuStartupState {
        powered: false,
        nr10: 0x00,
        nr11: 0x00,
        nr12: 0x00,
        nr13: 0x00,
        nr14: 0x00,
        nr21: 0x00,
        nr22: 0x00,
        nr23: 0x00,
        nr24: 0x00,
        nr30: 0x00,
        nr31: 0x00,
        nr32: 0x00,
        nr33: 0x00,
        nr34: 0x00,
        nr41: 0x00,
        nr42: 0x00,
        nr43: 0x00,
        nr44: 0x00,
        nr50: 0x00,
        nr51: 0x00,
        channel_active_mask: 0x00,
        div_apu: 0x05,
        wave_ram_startup_policy: WaveRamStartupPolicy::DeterministicZeroed,
    });

    apu.write_register_with_div_apu_source(0xFF26, 0x80, true);
    assert!(apu.snapshot().powered);
    assert_eq!(apu.snapshot().div_apu, 0x00);

    tick_apu_with_edges(&mut apu, 0, &[DerivedEdge::ApuFrameSequencerEdge]);
    assert_eq!(apu.snapshot().div_apu, 0x01);
}

#[test]
fn frame_sequencer_emits_length_sweep_and_envelope_clocks_on_the_documented_steps() {
    let mut apu = Apu::new(ConsoleModel::Dmg);

    for t_cycle in 0..8 {
        tick_apu_with_edges(&mut apu, t_cycle, &[DerivedEdge::ApuFrameSequencerEdge]);
    }

    assert_eq!(apu.snapshot().div_apu, 0x00);
    assert_eq!(apu.frame_sequencer.length_clock_count, 4);
    assert_eq!(apu.frame_sequencer.sweep_clock_count, 2);
    assert_eq!(apu.frame_sequencer.envelope_clock_count, 1);
}

#[test]
fn channel_1_trigger_reloads_period_envelope_and_sweep_without_resetting_duty_step() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x11);
    apu.write_register(0xFF11, 0xBF);
    apu.write_register(0xFF12, 0xA2);
    apu.write_register(0xFF13, 0xAB);
    apu.channel_1.pulse.duty_step = 5;

    apu.write_register(0xFF14, 0xC4);

    assert!(apu.channel_1.pulse.runtime.active);
    assert_eq!(apu.channel_1.pulse.duty_step, 5);
    assert_eq!(apu.channel_1.pulse.length_counter, 1);
    assert_eq!(apu.channel_1.pulse.current_volume, 0x0A);
    assert_eq!(apu.channel_1.pulse.envelope_timer, 0x02);
    assert_eq!(apu.channel_1.pulse.period_timer, pulse_timer_reload(0x04AB));
    assert_eq!(apu.channel_1.sweep.shadow_period, 0x04AB);
    assert_eq!(apu.channel_1.sweep.timer, 0x01);
    assert!(apu.channel_1.sweep.enabled);
}

#[test]
fn pulse_trigger_reloads_state_but_does_not_activate_while_the_dac_is_off() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);

    apu.write_register(0xFF10, 0x11);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0x00);
    apu.write_register(0xFF13, 0xAB);
    apu.channel_1.pulse.period_timer = 0x0123;
    apu.channel_1.pulse.envelope_timer = 5;
    apu.channel_1.pulse.current_volume = 7;
    apu.channel_1.sweep.shadow_period = 0x0456;
    apu.channel_1.sweep.timer = 3;
    apu.channel_1.sweep.enabled = true;

    apu.write_register(0xFF14, 0x80);

    assert!(!apu.channel_1.pulse.runtime.active);
    assert_eq!(
        apu.channel_1.pulse.period_timer,
        pulse_timer_reload(0x00AB) | (0x0123 & 0x03)
    );
    assert_eq!(apu.channel_1.pulse.envelope_timer, envelope_timer_reload(0));
    assert_eq!(apu.channel_1.pulse.current_volume, 0);
    assert_eq!(apu.channel_1.sweep.shadow_period, 0x00AB);
    assert_eq!(apu.channel_1.sweep.timer, 1);
    assert!(apu.channel_1.sweep.enabled);

    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0x00);
    apu.write_register(0xFF18, 0xCD);
    apu.channel_2.pulse.period_timer = 0x0235;
    apu.channel_2.pulse.envelope_timer = 6;
    apu.channel_2.pulse.current_volume = 9;

    apu.write_register(0xFF19, 0x80);

    assert!(!apu.channel_2.pulse.runtime.active);
    assert_eq!(
        apu.channel_2.pulse.period_timer,
        pulse_timer_reload(0x00CD) | (0x0235 & 0x03)
    );
    assert_eq!(apu.channel_2.pulse.envelope_timer, envelope_timer_reload(0));
    assert_eq!(apu.channel_2.pulse.current_volume, 0);
}

#[test]
fn channel_1_first_trigger_after_power_on_suppresses_the_initial_high_duty_output() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF11, 0x40);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0xFF);

    apu.write_register(0xFF14, 0x87);

    assert!(apu.channel_1.pulse.runtime.active);
    assert_eq!(apu.channel_1.pulse.duty_step, 0);
    assert!(pulse_waveform_high(
        apu.channel_1.pulse.duty,
        apu.channel_1.pulse.duty_step,
    ));
    assert!(apu.channel_1.pulse.suppress_initial_trigger_output);
    assert_eq!(apu.channel_1.pulse.current_digital_output(), 0);

    for _ in 0..4 {
        apu.channel_1.tick_fast_timer();
    }

    assert_eq!(apu.channel_1.pulse.duty_step, 1);
    assert!(!apu.channel_1.pulse.suppress_initial_trigger_output);
}

#[test]
fn channel_2_retrigger_after_the_first_post_power_on_trigger_does_not_resuppress_output() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x40);
    apu.write_register(0xFF17, 0xF0);
    apu.write_register(0xFF18, 0xFF);

    apu.write_register(0xFF19, 0x87);
    assert!(apu.channel_2.pulse.suppress_initial_trigger_output);

    for _ in 0..4 {
        apu.channel_2.tick_fast_timer();
    }

    assert!(!apu.channel_2.pulse.suppress_initial_trigger_output);

    apu.channel_2.pulse.duty_step = 0;
    apu.write_register(0xFF19, 0x87);

    assert!(!apu.channel_2.pulse.suppress_initial_trigger_output);
    assert_eq!(apu.channel_2.pulse.current_digital_output(), 0x0F);
}

#[test]
fn pulse_fast_timer_stays_frozen_until_the_first_trigger_after_power_on() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0xF0);
    apu.write_register(0xFF18, 0xFF);

    apu.channel_2.pulse.duty_step = 3;
    apu.channel_2.pulse.period_timer = 1;

    apu.channel_2.tick_fast_timer();

    assert_eq!(apu.channel_2.pulse.duty_step, 3);
    assert_eq!(apu.channel_2.pulse.period_timer, 1);
    assert!(apu.channel_2.pulse.first_trigger_after_power_on_pending);
}

#[test]
fn nr52_power_cycle_rearms_the_first_trigger_after_power_on_pulse_suppression() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF11, 0x40);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0xFF);
    apu.write_register(0xFF14, 0x87);

    assert!(apu.channel_1.pulse.suppress_initial_trigger_output);

    for _ in 0..4 {
        apu.channel_1.tick_fast_timer();
    }

    assert!(!apu.channel_1.pulse.suppress_initial_trigger_output);

    apu.write_register(0xFF26, 0x00);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF11, 0x40);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0xFF);
    apu.write_register(0xFF14, 0x87);

    assert!(apu.channel_1.pulse.suppress_initial_trigger_output);
    assert_eq!(apu.channel_1.pulse.current_digital_output(), 0);
}

#[test]
fn triggering_a_pulse_channel_preserves_the_low_two_bits_of_the_frequency_timer() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0xF0);
    apu.write_register(0xFF18, 0xFF);
    apu.channel_2.pulse.period_timer = 0x0003;

    apu.write_register(0xFF19, 0x87);

    assert_eq!(
        apu.channel_2.pulse.period_timer,
        pulse_timer_reload(0x07FF) | 0x0003
    );
}

#[test]
fn triggering_a_pulse_channel_just_before_an_envelope_step_reloads_the_timer_with_plus_one() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0xF2);

    apu.write_register(0xFF19, 0x80);

    assert_eq!(
        apu.channel_2.pulse.envelope_timer,
        envelope_timer_reload(0x02) + 1
    );
}

#[test]
fn enabling_pulse_length_on_a_non_length_step_clocks_it_immediately() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(1);
    apu.write_register(0xFF11, 0xBF);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF14, 0x80);

    assert!(apu.channel_1.pulse.runtime.active);
    assert!(!apu.channel_1.pulse.length_enabled);
    assert_eq!(apu.channel_1.pulse.length_counter, 1);

    apu.write_register(0xFF14, LENGTH_ENABLE_BIT);

    assert!(apu.channel_1.pulse.length_enabled);
    assert_eq!(apu.channel_1.pulse.length_counter, 0);
    assert!(!apu.channel_1.pulse.runtime.active);
}

#[test]
fn enabling_pulse_length_on_a_length_step_does_not_clock_it() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(0);
    apu.write_register(0xFF11, 0xBF);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF14, 0x80);

    assert!(apu.channel_1.pulse.runtime.active);
    assert!(!apu.channel_1.pulse.length_enabled);
    assert_eq!(apu.channel_1.pulse.length_counter, 1);

    apu.write_register(0xFF14, LENGTH_ENABLE_BIT);

    assert!(apu.channel_1.pulse.length_enabled);
    assert_eq!(apu.channel_1.pulse.length_counter, 1);
    assert!(apu.channel_1.pulse.runtime.active);
}

#[test]
fn pulse_trigger_rom_second_half_enable_keeps_length_unchanged_before_retrigger() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(6);
    prime_channel_2_trigger_test(&mut apu, 2);

    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channel_2.pulse.length_counter, 2);
    assert!(apu.channel_2.pulse.runtime.active);

    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channel_2.pulse.length_counter, 2);
    assert!(apu.channel_2.pulse.runtime.active);
}

#[test]
fn pulse_trigger_rom_first_half_enable_clocks_once_and_survives_the_intervening_non_length_edge() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_channel_2_trigger_test(&mut apu, 2);

    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channel_2.pulse.length_counter, 1);
    assert!(apu.channel_2.pulse.runtime.active);

    tick_apu_with_edges(&mut apu, 0, &[DerivedEdge::ApuFrameSequencerEdge]);
    assert_eq!(apu.snapshot().div_apu, 0x00);
    assert_eq!(apu.channel_2.pulse.length_counter, 1);
    assert!(apu.channel_2.pulse.runtime.active);

    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channel_2.pulse.length_counter, 1);
    assert!(apu.channel_2.pulse.runtime.active);
}

#[test]
fn triggering_a_zero_length_pulse_with_length_enabled_reloads_and_clocks_it() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_channel_2_trigger_test(&mut apu, 1);

    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channel_2.pulse.length_counter, 0);
    assert!(!apu.channel_2.pulse.runtime.active);

    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channel_2.pulse.length_counter, 63);
    assert!(apu.channel_2.pulse.runtime.active);
}

#[test]
fn triggering_a_length_one_pulse_with_enable_on_the_same_first_half_write_matches_the_unfrozen_case()
 {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_channel_2_trigger_test(&mut apu, 1);

    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channel_2.pulse.length_counter, 63);
    assert!(apu.channel_2.pulse.runtime.active);
}

#[test]
fn triggering_a_nonzero_length_pulse_does_not_change_its_length_counter() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(6);
    prime_channel_2_trigger_test(&mut apu, 2);
    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);

    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channel_2.pulse.length_counter, 2);
    assert!(apu.channel_2.pulse.runtime.active);
}

#[test]
fn writes_other_than_disabling_to_enabled_do_not_extra_clock_pulse_length() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_channel_2_trigger_test(&mut apu, 2);

    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channel_2.pulse.length_counter, 1);

    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channel_2.pulse.length_counter, 1);

    apu.write_register(0xFF19, 0x00);
    assert_eq!(apu.channel_2.pulse.length_counter, 1);

    apu.write_register(0xFF19, 0x00);
    assert_eq!(apu.channel_2.pulse.length_counter, 1);
}

#[test]
fn writing_length_after_enabling_it_matches_the_trigger_rom_sequence() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    apu.write_register(0xFF17, 0x08);
    apu.write_register(0xFF16, pulse_length_load(PULSE_LENGTH_COUNTER_RELOAD));
    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT);

    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    apu.write_register(0xFF16, pulse_length_load(2));
    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    apu.write_register(0xFF19, 0x00);
    apu.write_register(0xFF19, 0x00);

    assert_eq!(apu.channel_2.pulse.length_counter, 2);
    assert!(!apu.channel_2.pulse.length_enabled);
    assert!(apu.channel_2.pulse.runtime.active);
}

#[test]
fn extra_length_clocking_to_zero_disables_the_pulse_channel() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_channel_2_trigger_test(&mut apu, 1);

    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);

    assert_eq!(apu.channel_2.pulse.length_counter, 0);
    assert!(!apu.channel_2.pulse.runtime.active);
}

#[test]
fn enabling_length_again_after_it_reached_zero_does_not_clock_or_unfreeze_it() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_channel_2_trigger_test(&mut apu, 1);

    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channel_2.pulse.length_counter, 0);

    apu.write_register(0xFF19, 0x00);
    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channel_2.pulse.length_counter, 0);

    apu.write_register(0xFF19, 0x00);
    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channel_2.pulse.length_counter, 0);
}

#[test]
fn triggering_a_zero_length_pulse_with_length_disabled_unfreezes_it_to_the_full_reload() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_channel_2_trigger_test(&mut apu, 1);

    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    apu.write_register(0xFF19, 0x00);
    assert_eq!(apu.channel_2.pulse.length_counter, 0);
    assert!(!apu.channel_2.pulse.length_enabled);

    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT);

    assert_eq!(apu.channel_2.pulse.length_counter, 64);
    assert!(apu.channel_2.pulse.runtime.active);
}

#[test]
fn disabled_dac_still_allows_trigger_to_reload_and_clock_pulse_length() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_channel_2_trigger_test(&mut apu, 1);

    apu.write_register(0xFF17, 0x00);
    assert!(!apu.channel_2.pulse.runtime.dac_enabled);
    assert!(!apu.channel_2.pulse.runtime.active);

    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channel_2.pulse.length_counter, 63);
    assert!(!apu.channel_2.pulse.runtime.active);

    apu.write_register(0xFF17, 0x08);
    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT);

    assert_eq!(apu.channel_2.pulse.length_counter, 63);
    assert!(apu.channel_2.pulse.runtime.active);
}

#[test]
fn channel_1_first_half_enable_clocks_length_once_before_retrigger() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_channel_1_trigger_test(&mut apu, 2);

    apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channel_1.pulse.length_counter, 1);
    assert!(apu.channel_1.pulse.runtime.active);

    tick_apu_with_edges(&mut apu, 0, &[DerivedEdge::ApuFrameSequencerEdge]);
    apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channel_1.pulse.length_counter, 1);
    assert!(apu.channel_1.pulse.runtime.active);
}

#[test]
fn channel_1_trigger_with_zero_length_enabled_reloads_and_clocks_it() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_channel_1_trigger_test(&mut apu, 1);

    apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channel_1.pulse.length_counter, 0);
    assert!(!apu.channel_1.pulse.runtime.active);

    apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channel_1.pulse.length_counter, 63);
    assert!(apu.channel_1.pulse.runtime.active);
}

#[test]
fn channel_1_trigger_unfreezes_zero_length_and_clocks_it_after_disabling_length() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_channel_1_trigger_test(&mut apu, 1);

    apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channel_1.pulse.length_counter, 0);
    assert!(!apu.channel_1.pulse.runtime.active);

    apu.write_register(0xFF14, 0x00);
    assert_eq!(apu.channel_1.pulse.length_counter, 0);
    assert!(!apu.channel_1.pulse.length_enabled);

    apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channel_1.pulse.length_counter, 63);
    assert!(apu.channel_1.pulse.runtime.active);
}

#[test]
fn channel_1_retrigger_after_unfreezing_zero_length_does_not_extra_clock_again() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_channel_1_trigger_test(&mut apu, 1);

    apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channel_1.pulse.length_counter, 0);
    assert!(!apu.channel_1.pulse.runtime.active);

    apu.write_register(0xFF14, 0x00);
    apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);
    assert_eq!(apu.channel_1.pulse.length_counter, 63);
    assert!(apu.channel_1.pulse.runtime.active);

    apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channel_1.pulse.length_counter, 63);
    assert!(apu.channel_1.pulse.runtime.active);
}

#[test]
fn trigger_unfreezes_zero_length_then_a_later_enable_allows_normal_length_clocks() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_channel_2_trigger_test(&mut apu, 1);

    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channel_2.pulse.length_counter, 0);
    assert!(!apu.channel_2.pulse.runtime.active);

    apu.write_register(0xFF19, 0x00);
    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT);
    assert_eq!(apu.channel_2.pulse.length_counter, 64);
    assert!(apu.channel_2.pulse.runtime.active);

    tick_apu_with_edges(&mut apu, 0, &[DerivedEdge::ApuFrameSequencerEdge]);
    assert_eq!(apu.snapshot().div_apu, 0x00);
    assert_eq!(apu.channel_2.pulse.length_counter, 64);

    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channel_2.pulse.length_counter, 64);

    tick_apu_with_edges(&mut apu, 1, &[DerivedEdge::ApuFrameSequencerEdge]);
    tick_apu_with_edges(&mut apu, 2, &[DerivedEdge::ApuFrameSequencerEdge]);
    assert_eq!(apu.channel_2.pulse.length_counter, 63);
    assert!(apu.channel_2.pulse.runtime.active);

    tick_apu_with_edges(&mut apu, 3, &[DerivedEdge::ApuFrameSequencerEdge]);
    tick_apu_with_edges(&mut apu, 4, &[DerivedEdge::ApuFrameSequencerEdge]);
    assert_eq!(apu.channel_2.pulse.length_counter, 62);
    assert!(apu.channel_2.pulse.runtime.active);
}

#[test]
fn channel_1_retrigger_after_two_zero_length_freezes_only_extra_clocks_on_real_unfreeze_points() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_channel_1_trigger_test(&mut apu, 1);

    apu.write_register(0xFF14, 0x00);
    apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channel_1.pulse.length_counter, 0);
    assert!(!apu.channel_1.pulse.runtime.active);

    apu.write_register(0xFF14, 0x00);
    apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channel_1.pulse.length_counter, 0);
    assert!(!apu.channel_1.pulse.runtime.active);

    apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT);
    assert_eq!(apu.channel_1.pulse.length_counter, 64);
    assert!(apu.channel_1.pulse.runtime.active);
    assert!(!apu.channel_1.pulse.length_enabled);

    apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channel_1.pulse.length_counter, 63);
    assert!(apu.channel_1.pulse.runtime.active);

    apu.write_register(0xFF14, 0x00);
    assert_eq!(apu.channel_1.pulse.length_counter, 63);
    assert!(apu.channel_1.pulse.runtime.active);
    assert!(!apu.channel_1.pulse.length_enabled);

    apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channel_1.pulse.length_counter, 62);
    assert!(apu.channel_1.pulse.runtime.active);

    apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);
    assert_eq!(apu.channel_1.pulse.length_counter, 62);
    assert!(apu.channel_1.pulse.runtime.active);
}

#[test]
fn triggering_a_zero_length_pulse_on_a_non_length_step_reloads_it_to_63() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(1);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0xF0);
    apu.channel_2.pulse.length_counter = 0;

    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert!(apu.channel_2.pulse.runtime.active);
    assert!(apu.channel_2.pulse.length_enabled);
    assert_eq!(apu.channel_2.pulse.length_counter, 63);
}

#[test]
fn pulse_period_writes_take_effect_only_after_the_current_sample_finishes() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0xF0);
    apu.write_register(0xFF18, 0xFF);
    apu.write_register(0xFF19, 0x87);

    assert_eq!(apu.channel_2.pulse.period_timer, 4);

    apu.channel_2.tick_fast_timer();
    apu.channel_2.tick_fast_timer();
    assert_eq!(apu.channel_2.pulse.period_timer, 2);

    apu.write_register(0xFF18, 0xFE);
    apu.write_register(0xFF19, 0x07);
    assert_eq!(apu.channel_2.period_value(), 0x07FE);
    assert_eq!(apu.channel_2.pulse.period_timer, 2);

    apu.channel_2.tick_fast_timer();
    assert_eq!(apu.channel_2.pulse.period_timer, 1);
    apu.channel_2.tick_fast_timer();

    assert_eq!(apu.channel_2.pulse.period_timer, 8);
    assert_eq!(apu.channel_2.pulse.duty_step, 1);
    assert_eq!(apu.channel_2.pulse.current_digital_output(), 0);
}

#[test]
fn frame_sequencer_length_and_envelope_clocks_drive_pulse_channel_state() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF11, 0xBF);
    apu.write_register(0xFF12, 0x11);
    apu.write_register(0xFF14, 0xC0);
    apu.write_register(0xFF16, 0x3F);
    apu.write_register(0xFF17, 0x21);
    apu.write_register(0xFF19, 0xC0);

    apu.frame_sequencer.apply_startup_phase(7);
    tick_apu_with_edges(&mut apu, 0, &[DerivedEdge::ApuFrameSequencerEdge]);

    assert_eq!(apu.channel_1.pulse.current_volume, 0);
    assert_eq!(apu.channel_2.pulse.current_volume, 1);
    assert!(apu.channel_1.pulse.runtime.active);
    assert!(apu.channel_2.pulse.runtime.active);

    apu.frame_sequencer.apply_startup_phase(0);
    tick_apu_with_edges(&mut apu, 1, &[DerivedEdge::ApuFrameSequencerEdge]);

    assert!(!apu.channel_1.pulse.runtime.active);
    assert!(!apu.channel_2.pulse.runtime.active);
}

#[test]
fn channel_1_sweep_clock_writes_back_shadow_period_and_runs_the_second_overflow_check() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x11);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0x00);
    apu.write_register(0xFF14, 0x85);

    assert_eq!(apu.channel_1.period_value(), 0x0500);
    assert!(apu.channel_1.pulse.runtime.active);

    apu.channel_1.clock_sweep();

    assert_eq!(apu.channel_1.period_value(), 0x0780);
    assert_eq!(apu.channel_1.sweep.shadow_period, 0x0780);
    assert!(!apu.channel_1.pulse.runtime.active);
}

#[test]
fn channel_1_sweep_clock_can_update_the_shadow_period_while_inactive() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x11);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0x00);
    apu.write_register(0xFF14, 0x85);

    assert_eq!(apu.channel_1.period_value(), 0x0500);
    assert!(apu.channel_1.sweep.enabled);

    apu.channel_1.pulse.runtime.active = false;
    apu.channel_1.clock_sweep();

    assert_eq!(apu.channel_1.period_value(), 0x0780);
    assert_eq!(apu.channel_1.sweep.shadow_period, 0x0780);
    assert!(!apu.channel_1.pulse.runtime.active);
}

#[test]
fn channel_1_shift_zero_sweep_does_not_calculate_on_trigger_but_can_overflow_on_sweep_clock() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x10);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0x00);
    apu.write_register(0xFF14, 0x86);

    assert_eq!(apu.channel_1.period_value(), 0x0600);
    assert!(apu.channel_1.pulse.runtime.active);

    apu.channel_1.clock_sweep();

    assert_eq!(apu.channel_1.period_value(), 0x0600);
    assert!(!apu.channel_1.pulse.runtime.active);
}

#[test]
fn channel_1_zero_sweep_pace_reloads_to_eight_and_rearms_on_a_non_zero_write() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x11);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0x00);
    apu.write_register(0xFF14, 0x82);

    assert_eq!(apu.channel_1.period_value(), 0x0200);
    apu.channel_1.clock_sweep();
    assert_eq!(apu.channel_1.period_value(), 0x0300);

    apu.write_register(0xFF10, 0x01);
    for _ in 0..8 {
        apu.channel_1.clock_sweep();
        assert_eq!(apu.channel_1.period_value(), 0x0300);
        assert!(apu.channel_1.pulse.runtime.active);
    }

    assert_eq!(apu.channel_1.sweep.shadow_period, 0x0300);
    assert_eq!(apu.channel_1.sweep.timer, 1);

    apu.write_register(0xFF10, 0x11);
    apu.channel_1.clock_sweep();

    assert_eq!(apu.channel_1.period_value(), 0x0480);
    assert_eq!(apu.channel_1.sweep.shadow_period, 0x0480);
    assert!(apu.channel_1.pulse.runtime.active);
}

#[test]
fn clearing_negate_after_a_negate_calculation_disables_channel_1() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x09);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0x00);
    apu.write_register(0xFF14, 0x84);

    assert!(apu.channel_1.pulse.runtime.active);
    assert!(apu.channel_1.sweep.negate_calculated_since_trigger);

    apu.write_register(0xFF10, 0x10);

    assert!(!apu.channel_1.pulse.runtime.active);
}

#[test]
fn clearing_negate_after_an_in_range_negate_calculation_still_disables_channel_1() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x19);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0x00);
    apu.write_register(0xFF14, 0x84);

    assert!(apu.channel_1.pulse.runtime.active);
    assert!(apu.channel_1.sweep.negate_calculated_since_trigger);
    assert_eq!(apu.channel_1.period_value(), 0x0400);

    apu.write_register(0xFF10, 0x11);

    assert!(!apu.channel_1.pulse.runtime.active);
    assert_eq!(apu.channel_1.period_value(), 0x0400);
}

#[test]
fn clearing_negate_without_a_negate_calculation_keeps_channel_1_active() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x08);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0x00);
    apu.write_register(0xFF14, 0x84);

    assert!(apu.channel_1.pulse.runtime.active);
    assert!(!apu.channel_1.sweep.negate_calculated_since_trigger);

    apu.write_register(0xFF10, 0x10);

    assert!(apu.channel_1.pulse.runtime.active);
}

#[test]
fn channel_1_negate_sweep_uses_eleven_bit_twos_complement_subtraction() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x1C);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0xB0);
    apu.write_register(0xFF14, 0x85);

    apu.channel_1.clock_sweep();

    assert_eq!(apu.channel_1.period_value(), 0x0555);
    assert_eq!(apu.channel_1.sweep.shadow_period, 0x0555);
    assert!(apu.channel_1.pulse.runtime.active);
}

#[test]
fn envelope_reaching_zero_does_not_disable_the_pulse_channel() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0x11);
    apu.write_register(0xFF19, 0x80);

    apu.frame_sequencer.apply_startup_phase(7);
    tick_apu_with_edges(&mut apu, 0, &[DerivedEdge::ApuFrameSequencerEdge]);

    assert_eq!(apu.channel_2.pulse.current_volume, 0);
    assert!(apu.channel_2.pulse.runtime.active);
}

#[test]
fn pulse_envelope_stops_automatic_updates_after_saturating_at_fifteen() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0xFA);
    apu.write_register(0xFF19, 0x80);

    apu.channel_2.pulse.envelope_timer = 1;

    assert!(apu.channel_2.pulse.envelope_automatic_updates_enabled);
    assert_eq!(apu.channel_2.pulse.current_volume, 0x0F);

    apu.channel_2.clock_envelope();

    assert_eq!(apu.channel_2.pulse.current_volume, 0x0F);
    assert_eq!(apu.channel_2.pulse.envelope_timer, 2);
    assert!(apu.channel_2.pulse.runtime.active);
    assert!(!apu.channel_2.pulse.envelope_automatic_updates_enabled);

    apu.channel_2.clock_envelope();

    assert_eq!(apu.channel_2.pulse.current_volume, 0x0F);
    assert_eq!(apu.channel_2.pulse.envelope_timer, 2);
    assert!(apu.channel_2.pulse.runtime.active);

    apu.write_register(0xFF19, 0x80);

    assert!(apu.channel_2.pulse.envelope_automatic_updates_enabled);
    assert_eq!(apu.channel_2.pulse.current_volume, 0x0F);
    assert_eq!(apu.channel_2.pulse.envelope_timer, 2);
}

#[test]
fn pulse_fast_timer_advances_duty_step_while_the_channel_is_inactive() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0xF0);
    apu.write_register(0xFF18, 0xFF);
    apu.write_register(0xFF19, 0x87);

    apu.channel_2.pulse.runtime.active = false;
    apu.channel_2.pulse.duty_step = 6;
    apu.channel_2.pulse.period_timer = 1;

    apu.channel_2.tick_fast_timer();

    assert_eq!(apu.channel_2.pulse.duty_step, 7);
    assert_eq!(apu.channel_2.pulse.period_timer, 4);
    assert!(!apu.channel_2.pulse.runtime.active);
    assert_eq!(apu.channel_2.pulse.current_digital_output(), 0);
}

#[test]
fn pulse_envelope_clock_advances_while_the_channel_is_inactive() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0x11);
    apu.write_register(0xFF19, 0x80);

    apu.channel_2.pulse.runtime.active = false;
    apu.channel_2.pulse.envelope_timer = 1;
    apu.channel_2.pulse.current_volume = 1;

    apu.channel_2.clock_envelope();

    assert_eq!(apu.channel_2.pulse.envelope_timer, 1);
    assert_eq!(apu.channel_2.pulse.current_volume, 0);
    assert!(!apu.channel_2.pulse.runtime.active);
    assert_eq!(apu.channel_2.pulse.current_digital_output(), 0);
}

#[test]
fn live_nrx2_write_with_increase_and_zero_pace_increments_active_pulse_channels() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0x08);
    apu.write_register(0xFF14, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0x08);
    apu.write_register(0xFF19, 0x80);

    assert!(apu.channel_1.pulse.runtime.active);
    assert!(apu.channel_2.pulse.runtime.active);
    assert_eq!(apu.channel_1.pulse.current_volume, 0);
    assert_eq!(apu.channel_2.pulse.current_volume, 0);

    apu.write_register(0xFF12, 0x08);
    apu.write_register(0xFF17, 0x08);

    assert_eq!(apu.channel_1.pulse.current_volume, 1);
    assert_eq!(apu.channel_2.pulse.current_volume, 1);

    apu.channel_1.pulse.current_volume = 0x0F;
    apu.write_register(0xFF12, 0x08);
    assert_eq!(apu.channel_1.pulse.current_volume, 0);

    apu.channel_2.pulse.current_volume = 7;
    apu.write_register(0xFF17, 0x09);
    assert_eq!(apu.channel_2.pulse.current_volume, 7);
}

#[test]
fn live_nrx2_write_requires_retrigger_before_reprogramming_pulse_envelopes() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);

    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0x52);
    apu.write_register(0xFF14, 0x80);
    apu.channel_1.pulse.envelope_timer = 1;

    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0x52);
    apu.write_register(0xFF19, 0x80);
    apu.channel_2.pulse.envelope_timer = 1;

    assert_eq!(apu.channel_1.pulse.current_volume, 5);
    assert_eq!(apu.channel_2.pulse.current_volume, 5);

    apu.write_register(0xFF12, 0x69);
    apu.write_register(0xFF17, 0x69);

    assert_eq!(apu.channel_1.pulse.current_volume, 5);
    assert_eq!(apu.channel_2.pulse.current_volume, 5);

    apu.channel_1.clock_envelope();
    apu.channel_2.clock_envelope();

    assert_eq!(apu.channel_1.pulse.current_volume, 4);
    assert_eq!(apu.channel_1.pulse.envelope_timer, 2);
    assert_eq!(apu.channel_2.pulse.current_volume, 4);
    assert_eq!(apu.channel_2.pulse.envelope_timer, 2);

    apu.write_register(0xFF14, 0x80);
    apu.write_register(0xFF19, 0x80);

    assert_eq!(apu.channel_1.pulse.current_volume, 6);
    assert_eq!(apu.channel_1.pulse.envelope_timer, 1);
    assert_eq!(apu.channel_2.pulse.current_volume, 6);
    assert_eq!(apu.channel_2.pulse.envelope_timer, 1);
}

#[test]
fn channel_3_trigger_preserves_the_buffered_sample_until_the_next_wave_fetch() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.channel_3.wave_ram[0] = 0x12;
    apu.channel_3.wave_ram[1] = 0x34;
    apu.channel_3.sample_buffer = 0x0E;

    apu.write_register(0xFF1A, 0x80);
    apu.write_register(0xFF1C, 0x20);
    apu.write_register(0xFF1D, 0xFF);
    apu.write_register(0xFF1E, 0x87);

    assert!(apu.channel_3.runtime.active);
    assert_eq!(apu.channel_3.sample_index, 0);
    assert_eq!(apu.channel_3.sample_buffer, 0x0E);
    assert_eq!(
        apu.channel_3.period_timer,
        2 + WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES
    );

    for expected_timer in (1..=1 + WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES).rev() {
        apu.channel_3.tick_fast_timer();
        assert_eq!(apu.channel_3.sample_buffer, 0x0E);
        assert_eq!(apu.channel_3.period_timer, expected_timer);
    }

    apu.channel_3.tick_fast_timer();
    assert_eq!(apu.channel_3.sample_index, 1);
    assert_eq!(apu.channel_3.sample_buffer, 0x02);
    assert_eq!(apu.channel_3.period_timer, 2);
}

#[test]
fn channel_3_period_writes_take_effect_only_after_the_next_wave_fetch_boundary() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.channel_3.wave_ram[0] = 0x12;

    apu.write_register(0xFF1A, 0x80);
    apu.write_register(0xFF1C, 0x20);
    apu.write_register(0xFF1D, 0xFF);
    apu.write_register(0xFF1E, 0x87);

    apu.channel_3.tick_fast_timer();
    assert_eq!(
        apu.channel_3.period_timer,
        1 + WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES
    );

    apu.write_register(0xFF1D, 0xFE);
    apu.write_register(0xFF1E, 0x07);

    assert_eq!(apu.channel_3.period_value(), 0x07FE);
    assert_eq!(
        apu.channel_3.period_timer,
        1 + WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES
    );
    assert_eq!(apu.channel_3.sample_buffer, 0);

    for expected_timer in (1..=WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES).rev() {
        apu.channel_3.tick_fast_timer();
        assert_eq!(apu.channel_3.sample_index, 0);
        assert_eq!(apu.channel_3.sample_buffer, 0);
        assert_eq!(apu.channel_3.period_timer, expected_timer);
    }

    apu.channel_3.tick_fast_timer();
    assert_eq!(apu.channel_3.sample_index, 1);
    assert_eq!(apu.channel_3.sample_buffer, 0x02);
    assert_eq!(apu.channel_3.period_timer, 4);
}

#[test]
fn channel_3_output_level_applies_immediate_digital_attenuation_without_disabling_it() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF1A, 0x80);
    apu.channel_3.runtime.active = true;
    apu.channel_3.sample_buffer = 0x0C;

    apu.write_register(0xFF1C, 0x00);
    assert_eq!(apu.channel_3.current_digital_output(), 0);
    assert!(apu.channel_3.runtime.active);

    apu.write_register(0xFF1C, 0x20);
    assert_eq!(apu.channel_3.current_digital_output(), 0x0C);

    apu.write_register(0xFF1C, 0x40);
    assert_eq!(apu.channel_3.current_digital_output(), 0x06);

    apu.write_register(0xFF1C, 0x60);
    assert_eq!(apu.channel_3.current_digital_output(), 0x03);
    assert!(apu.channel_3.runtime.active);
}

#[test]
fn channel_3_fast_timer_advances_sample_state_while_the_channel_is_inactive() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.channel_3.wave_ram[0] = 0x12;
    apu.write_register(0xFF1A, 0x80);
    apu.write_register(0xFF1C, 0x20);
    apu.write_register(0xFF1D, 0xFF);
    apu.write_register(0xFF1E, 0x87);

    apu.channel_3.runtime.active = false;
    apu.channel_3.sample_index = 0;
    apu.channel_3.sample_buffer = 0x0E;
    apu.channel_3.period_timer = 1;

    apu.channel_3.tick_fast_timer();

    assert_eq!(apu.channel_3.sample_index, 1);
    assert_eq!(apu.channel_3.sample_buffer, 0x02);
    assert_eq!(apu.channel_3.period_timer, 2);
    assert!(!apu.channel_3.runtime.active);
    assert_eq!(apu.channel_3.current_digital_output(), 0);
}

#[test]
fn channel_3_trigger_reloads_timer_and_index_but_does_not_activate_while_the_dac_is_off() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF1A, 0x00);
    apu.write_register(0xFF1D, 0xAB);
    apu.channel_3.sample_index = 17;
    apu.channel_3.sample_buffer = 0x0E;
    apu.channel_3.period_timer = 99;
    apu.channel_3.length_counter = 7;

    apu.write_register(0xFF1E, 0x80);

    assert!(!apu.channel_3.runtime.active);
    assert_eq!(apu.channel_3.sample_index, 0);
    assert_eq!(apu.channel_3.sample_buffer, 0x0E);
    assert_eq!(
        apu.channel_3.period_timer,
        wave_timer_reload(0x00AB) + WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES
    );
    assert_eq!(apu.channel_3.length_counter, 7);
}

#[test]
fn active_channel_3_wave_ram_reads_return_ff_outside_the_dmg_fetch_window() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF1A, 0x80);
    apu.channel_3.runtime.active = true;
    apu.channel_3.wave_ram[0] = 0x12;
    apu.channel_3.wave_ram[1] = 0x34;

    assert_eq!(apu.read_register(0xFF30), WAVE_RAM_INACCESSIBLE_READ_VALUE);
    assert_eq!(apu.read_register(0xFF3F), WAVE_RAM_INACCESSIBLE_READ_VALUE);
}

#[test]
fn active_channel_3_wave_ram_writes_are_ignored_outside_the_dmg_fetch_window() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF1A, 0x80);
    apu.channel_3.runtime.active = true;
    apu.channel_3.wave_ram[0] = 0x12;
    apu.channel_3.wave_ram[1] = 0x34;

    apu.write_register(0xFF30, 0xAB);

    assert_eq!(apu.channel_3.wave_ram[0], 0x12);
    assert_eq!(apu.channel_3.wave_ram[1], 0x34);
}

#[test]
fn dmg_channel_3_wave_ram_access_uses_the_internal_byte_only_during_the_fetch_window() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF1A, 0x80);
    apu.write_register(0xFF1C, 0x20);
    apu.write_register(0xFF1D, 0xFF);
    apu.write_register(0xFF1E, 0x87);
    apu.channel_3.wave_ram[0] = 0x12;
    apu.channel_3.wave_ram[1] = 0x34;

    for _ in 0..2 + WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES {
        apu.channel_3.begin_t_cycle();
        apu.channel_3.tick_fast_timer();
    }

    assert_eq!(apu.channel_3.sample_index, 1);
    assert_eq!(apu.read_register(0xFF30), 0x12);
    assert_eq!(apu.read_register(0xFF3F), 0x12);

    apu.write_register(0xFF30, 0xAB);

    assert_eq!(apu.channel_3.wave_ram[0], 0xAB);
    assert_eq!(apu.channel_3.wave_ram[1], 0x34);

    apu.channel_3.begin_t_cycle();

    assert_eq!(apu.read_register(0xFF30), WAVE_RAM_INACCESSIBLE_READ_VALUE);
}

#[test]
fn cgb_channel_3_wave_ram_remains_addressable_while_inactive() {
    let mut apu = Apu::new(ConsoleModel::Cgb);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF1A, 0x80);
    apu.channel_3.wave_ram[0] = 0x12;
    apu.channel_3.wave_ram[1] = 0x34;

    assert_eq!(apu.read_register(0xFF30), 0x12);
    assert_eq!(apu.read_register(0xFF31), 0x34);

    apu.write_register(0xFF30, 0xAB);

    assert_eq!(apu.channel_3.wave_ram[0], 0xAB);
    assert_eq!(apu.channel_3.wave_ram[1], 0x34);
}

#[test]
fn observed_register_write_captures_channel_3_dac_disable_before_and_after_state() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF1A, NR30_DAC_POWER_BIT);
    apu.write_register(0xFF1C, 0x20);
    apu.write_register(0xFF1D, 0x00);
    apu.write_register(0xFF1E, CHANNEL_TRIGGER_BIT);

    let before_disable = apu.snapshot();
    assert_eq!(
        before_disable.channel_active_mask & CHANNEL_ACTIVE_CH3,
        CHANNEL_ACTIVE_CH3
    );
    assert_eq!(
        before_disable.channel_dac_mask & CHANNEL_ACTIVE_CH3,
        CHANNEL_ACTIVE_CH3
    );

    apu.write_register(0xFF1A, 0x00);

    let observation = apu
        .snapshot()
        .last_register_write
        .expect("FF1A write should be observed");
    assert_eq!(observation.address, 0xFF1A);
    assert_eq!(observation.value, 0x00);
    assert_eq!(
        observation.before.channel_active_mask & CHANNEL_ACTIVE_CH3,
        CHANNEL_ACTIVE_CH3
    );
    assert_eq!(
        observation.before.channel_dac_mask & CHANNEL_ACTIVE_CH3,
        CHANNEL_ACTIVE_CH3
    );
    assert_eq!(
        observation.after.channel_active_mask & CHANNEL_ACTIVE_CH3,
        0x00
    );
    assert_eq!(
        observation.after.channel_dac_mask & CHANNEL_ACTIVE_CH3,
        0x00
    );
    assert_ne!(observation.before.nr52, observation.after.nr52);

    tick_apu_with_edges(&mut apu, 0, &[]);
    assert!(apu.snapshot().last_register_write.is_none());
}

fn active_channel_3_test_state() -> Channel3State {
    Channel3State {
        nr30: NR30_DAC_POWER_BIT,
        runtime: ChannelRuntimeState {
            dac_enabled: true,
            active: true,
        },
        ..Channel3State::default()
    }
}

#[test]
fn dmg_channel_3_retrigger_corrupts_wave_ram_byte_zero_two_t_cycles_before_the_next_fetch() {
    let mut channel = active_channel_3_test_state();
    channel.wave_ram = [
        0x10, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
        0xFF,
    ];
    channel.sample_index = 1;
    channel.period_timer = 2;

    channel.write_nr34(CHANNEL_TRIGGER_BIT, ConsoleModel::Dmg, 0);

    assert_eq!(channel.wave_ram[0], 0x11);
    assert_eq!(channel.wave_ram[1], 0x11);
    assert_eq!(channel.wave_ram[2], 0x22);
    assert_eq!(channel.wave_ram[3], 0x33);
}

#[test]
fn dmg_channel_3_retrigger_corrupts_wave_ram_from_the_next_aligned_four_byte_block() {
    let mut channel = active_channel_3_test_state();
    channel.wave_ram = [
        0x10, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
        0xFF,
    ];
    channel.sample_index = 7;
    channel.period_timer = 2;

    channel.write_nr34(CHANNEL_TRIGGER_BIT, ConsoleModel::Dmg, 0);

    assert_eq!(channel.wave_ram[0], 0x44);
    assert_eq!(channel.wave_ram[1], 0x55);
    assert_eq!(channel.wave_ram[2], 0x66);
    assert_eq!(channel.wave_ram[3], 0x77);
}

#[test]
fn channel_3_retrigger_corruption_is_gated_to_dmg_family_behavior() {
    let mut channel = active_channel_3_test_state();
    channel.wave_ram = [
        0x10, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
        0xFF,
    ];
    channel.sample_index = 7;
    channel.period_timer = 2;

    channel.write_nr34(CHANNEL_TRIGGER_BIT, ConsoleModel::Cgb, 0);

    assert_eq!(
        channel.wave_ram,
        [
            0x10, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
            0xEE, 0xFF,
        ]
    );
}

#[test]
fn dmg_channel_3_retrigger_corruption_requires_the_two_t_cycle_window() {
    let mut channel = active_channel_3_test_state();
    channel.wave_ram = [
        0x10, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE,
        0xFF,
    ];
    channel.sample_index = 7;
    channel.period_timer = 1;

    channel.write_nr34(CHANNEL_TRIGGER_BIT, ConsoleModel::Dmg, 0);

    assert_eq!(
        channel.wave_ram,
        [
            0x10, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
            0xEE, 0xFF,
        ]
    );
}

#[test]
fn channel_3_trigger_with_zero_length_enabled_reloads_and_clocks_it() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    apu.write_register(0xFF1A, 0x80);
    apu.write_register(0xFF1B, 0xFF);
    apu.write_register(0xFF1E, LENGTH_ENABLE_BIT);

    assert_eq!(apu.channel_3.length_counter, 0);
    assert!(!apu.channel_3.runtime.active);

    apu.write_register(0xFF1E, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channel_3.length_counter, 255);
    assert!(apu.channel_3.runtime.active);
}

#[test]
fn channel_4_trigger_with_zero_length_enabled_reloads_and_clocks_it() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    apu.write_register(0xFF21, 0x08);
    apu.write_register(0xFF20, pulse_length_load(1));
    apu.write_register(0xFF23, LENGTH_ENABLE_BIT);

    assert_eq!(apu.channel_4.length_counter, 0);
    assert!(!apu.channel_4.runtime.active);

    apu.write_register(0xFF23, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channel_4.length_counter, 63);
    assert!(apu.channel_4.runtime.active);
}

#[test]
fn channel_4_retrigger_after_unfreezing_zero_length_does_not_extra_clock_again() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    apu.write_register(0xFF21, 0x08);
    apu.write_register(0xFF20, pulse_length_load(1));
    apu.write_register(0xFF23, LENGTH_ENABLE_BIT);

    assert_eq!(apu.channel_4.length_counter, 0);
    assert!(!apu.channel_4.runtime.active);

    apu.write_register(0xFF23, 0x00);
    apu.write_register(0xFF23, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);
    assert_eq!(apu.channel_4.length_counter, 63);
    assert!(apu.channel_4.runtime.active);

    apu.write_register(0xFF23, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channel_4.length_counter, 63);
    assert!(apu.channel_4.runtime.active);
}

#[test]
fn channel_4_trigger_reloads_envelope_lfsr_and_noise_timer() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF21, 0xF2);
    apu.write_register(0xFF22, 0x15);
    apu.write_register(0xFF23, 0x80);

    assert!(apu.channel_4.runtime.active);
    assert_eq!(apu.channel_4.current_volume, 0x0F);
    assert_eq!(apu.channel_4.envelope_timer, 2);
    assert_eq!(apu.channel_4.lfsr_state, NOISE_LFSR_INITIAL_STATE);
    assert_eq!(apu.channel_4.current_digital_output(), 0x0F);
    assert_eq!(apu.channel_4.period_timer, 160);
}

#[test]
fn channel_4_trigger_reloads_state_but_does_not_activate_while_the_dac_is_off() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF21, 0x00);
    apu.write_register(0xFF22, 0x15);
    apu.channel_4.period_timer = 77;
    apu.channel_4.envelope_timer = 4;
    apu.channel_4.current_volume = 6;
    apu.channel_4.lfsr_state = 0x0123;

    apu.write_register(0xFF23, 0x80);

    assert!(!apu.channel_4.runtime.active);
    assert_eq!(apu.channel_4.period_timer, 160);
    assert_eq!(apu.channel_4.envelope_timer, envelope_timer_reload(0));
    assert_eq!(apu.channel_4.current_volume, 0);
    assert_eq!(apu.channel_4.lfsr_state, NOISE_LFSR_INITIAL_STATE);
}

#[test]
fn channel_4_noise_timer_steps_the_lfsr_and_short_width_mode_copies_feedback_into_bit_six() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.runtime.active = true;
    channel.short_width_mode = true;
    channel.clock_shift = 0;
    channel.clock_divider_code = 0;
    channel.period_timer = 1;
    channel.current_volume = 0x0F;
    channel.lfsr_state = NOISE_LFSR_INITIAL_STATE;

    channel.tick_fast_timer();

    assert_eq!(channel.period_timer, 8);
    assert_eq!(channel.lfsr_state, 0x4040);
    assert_eq!(channel.current_digital_output(), 0x0F);
}

#[test]
fn channel_4_noise_timer_keeps_running_while_the_channel_is_inactive() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.runtime.active = false;
    channel.short_width_mode = true;
    channel.clock_shift = 0;
    channel.clock_divider_code = 0;
    channel.period_timer = 1;
    channel.current_volume = 0x0F;
    channel.lfsr_state = NOISE_LFSR_INITIAL_STATE;

    channel.tick_fast_timer();

    assert_eq!(channel.period_timer, 8);
    assert_eq!(channel.lfsr_state, 0x4040);
    assert!(!channel.runtime.active);
    assert_eq!(channel.current_digital_output(), 0);
}

#[test]
fn channel_4_live_nr43_write_into_shift_14_reloads_the_suppressed_noise_timer() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.runtime.active = true;
    channel.current_volume = 0x0F;
    channel.lfsr_state = 0x1234;
    channel.write_nr43(0x00);
    channel.period_timer = 1;

    channel.write_nr43(0xE0);

    assert_eq!(channel.period_timer, noise_timer_reload(14, 0));

    let lfsr_before = channel.lfsr_state;
    channel.tick_fast_timer();

    assert_eq!(channel.period_timer, noise_timer_reload(14, 0));
    assert_eq!(channel.lfsr_state, lfsr_before);
    assert_eq!(channel.current_digital_output(), 0x0F);
}

#[test]
fn channel_4_live_nr43_write_out_of_shift_14_reloads_from_the_new_clocked_timer_base() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.runtime.active = true;
    channel.current_volume = 0x0F;
    channel.lfsr_state = 0x1234;
    channel.write_nr43(0xE0);

    assert_eq!(channel.period_timer, noise_timer_reload(14, 0));

    channel.write_nr43(0x00);

    assert_eq!(channel.period_timer, noise_timer_reload(0, 0));
    let lfsr_before = channel.lfsr_state;

    for expected_timer in (1..noise_timer_reload(0, 0)).rev() {
        channel.tick_fast_timer();
        assert_eq!(channel.period_timer, expected_timer);
        assert_eq!(channel.lfsr_state, lfsr_before);
    }

    channel.tick_fast_timer();

    assert_eq!(channel.period_timer, noise_timer_reload(0, 0));
    assert_ne!(channel.lfsr_state, lfsr_before);
}

#[test]
fn channel_4_envelope_reaching_zero_does_not_disable_the_channel() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF21, 0x12);
    apu.write_register(0xFF22, 0x00);
    apu.write_register(0xFF23, 0x80);
    apu.channel_4.envelope_timer = 1;

    assert!(apu.channel_4.runtime.active);
    assert_eq!(apu.channel_4.current_volume, 1);
    assert!(apu.channel_4.envelope_automatic_updates_enabled);

    apu.channel_4.clock_envelope();

    assert_eq!(apu.channel_4.current_volume, 0);
    assert_eq!(apu.channel_4.envelope_timer, 2);
    assert!(apu.channel_4.runtime.active);
    assert!(apu.channel_4.envelope_automatic_updates_enabled);

    apu.channel_4.clock_envelope();

    assert_eq!(apu.channel_4.current_volume, 0);
    assert_eq!(apu.channel_4.envelope_timer, 1);
    assert!(apu.channel_4.runtime.active);
    assert!(apu.channel_4.envelope_automatic_updates_enabled);

    apu.channel_4.clock_envelope();

    assert_eq!(apu.channel_4.current_volume, 0);
    assert_eq!(apu.channel_4.envelope_timer, 2);
    assert!(apu.channel_4.runtime.active);
    assert!(!apu.channel_4.envelope_automatic_updates_enabled);

    apu.channel_4.clock_envelope();

    assert_eq!(apu.channel_4.current_volume, 0);
    assert_eq!(apu.channel_4.envelope_timer, 2);
    assert!(apu.channel_4.runtime.active);

    apu.write_register(0xFF23, 0x80);

    assert!(apu.channel_4.envelope_automatic_updates_enabled);
    assert_eq!(apu.channel_4.current_volume, 1);
    assert_eq!(apu.channel_4.envelope_timer, 2);
}

#[test]
fn channel_4_envelope_clock_advances_while_the_channel_is_inactive() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF21, 0x11);
    apu.write_register(0xFF23, 0x80);

    apu.channel_4.runtime.active = false;
    apu.channel_4.envelope_timer = 1;
    apu.channel_4.current_volume = 1;

    apu.channel_4.clock_envelope();

    assert_eq!(apu.channel_4.envelope_timer, 1);
    assert_eq!(apu.channel_4.current_volume, 0);
    assert!(!apu.channel_4.runtime.active);
    assert_eq!(apu.channel_4.current_digital_output(), 0);
}

#[test]
fn live_nr42_write_with_increase_and_zero_pace_increments_active_noise_channel() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF21, 0x08);
    apu.write_register(0xFF23, 0x80);

    assert!(apu.channel_4.runtime.active);
    assert_eq!(apu.channel_4.current_volume, 0);

    apu.write_register(0xFF21, 0x08);
    assert_eq!(apu.channel_4.current_volume, 1);

    apu.channel_4.current_volume = 0x0F;
    apu.write_register(0xFF21, 0x08);
    assert_eq!(apu.channel_4.current_volume, 0);
}

#[test]
fn live_nr42_write_requires_retrigger_before_reprogramming_the_noise_envelope() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF21, 0x52);
    apu.write_register(0xFF23, 0x80);
    apu.channel_4.envelope_timer = 1;

    assert_eq!(apu.channel_4.current_volume, 5);

    apu.write_register(0xFF21, 0x69);

    assert_eq!(apu.channel_4.current_volume, 5);

    apu.channel_4.clock_envelope();

    assert_eq!(apu.channel_4.current_volume, 4);
    assert_eq!(apu.channel_4.envelope_timer, 2);

    apu.write_register(0xFF23, 0x80);

    assert_eq!(apu.channel_4.current_volume, 6);
    assert_eq!(apu.channel_4.envelope_timer, 1);
}

#[test]
fn channel_4_live_15_bit_to_7_bit_switch_can_lock_the_active_lfsr_window_silently() {
    let mut wide = Channel4State::default();
    wide.runtime.dac_enabled = true;
    wide.runtime.active = true;
    wide.write_nr43(0x00);
    wide.period_timer = 1;
    wide.current_volume = 0x0F;
    wide.lfsr_state = 0x007F;

    let mut narrow = wide.clone();
    narrow.write_nr43(0x08);

    wide.tick_fast_timer();
    narrow.tick_fast_timer();

    assert_eq!(wide.lfsr_state & 0x7F, 0x3F);
    assert_eq!(narrow.lfsr_state & 0x7F, 0x7F);
    assert_eq!(narrow.current_digital_output(), 0);
    assert!(narrow.runtime.active);

    narrow.period_timer = 1;
    narrow.tick_fast_timer();

    assert_eq!(narrow.lfsr_state & 0x7F, 0x7F);
    assert_eq!(narrow.current_digital_output(), 0);
    assert!(narrow.runtime.active);
}

#[test]
fn channel_4_retrigger_recovers_from_short_width_lockup_without_clearing_activity() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF21, 0xF0);
    apu.write_register(0xFF22, 0x08);

    apu.channel_4.runtime.active = true;
    apu.channel_4.lfsr_state = 0x007F;
    apu.channel_4.current_volume = 0x0F;

    assert_eq!(apu.read_register(0xFF26) & 0x08, 0x08);
    assert_eq!(apu.channel_4.current_digital_output(), 0);

    apu.write_register(0xFF23, 0x80);

    assert_eq!(apu.channel_4.lfsr_state, NOISE_LFSR_INITIAL_STATE);
    assert!(apu.channel_4.runtime.active);
    assert_eq!(apu.read_register(0xFF26) & 0x08, 0x08);
    assert_eq!(apu.channel_4.current_digital_output(), 0x0F);

    apu.channel_4.period_timer = 1;
    apu.channel_4.tick_fast_timer();

    assert_ne!(apu.channel_4.lfsr_state & 0x7F, 0x7F);
}

#[test]
fn dmg_powered_off_length_writes_preserve_internal_length_counters_without_restoring_register_state()
 {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF26, 0x00);

    apu.write_register(0xFF11, 0xD5);
    apu.write_register(0xFF16, 0xEA);
    apu.write_register(0xFF1B, 0x44);
    apu.write_register(0xFF20, 0xCD);

    assert_eq!(
        apu.channel_1.pulse.length_counter,
        pulse_length_counter_from_load(0xD5)
    );
    assert_eq!(
        apu.channel_2.pulse.length_counter,
        pulse_length_counter_from_load(0xEA)
    );
    assert_eq!(
        apu.channel_3.length_counter,
        wave_length_counter_from_load(0x44)
    );
    assert_eq!(
        apu.channel_4.length_counter,
        pulse_length_counter_from_load(0xCD)
    );
    assert_eq!(apu.read_register(0xFF11), 0x3F);
    assert_eq!(apu.read_register(0xFF16), 0x3F);
    assert_eq!(apu.read_register(0xFF1B), 0xFF);
    assert_eq!(apu.read_register(0xFF20), 0xFF);
}

#[test]
fn startup_state_recreates_the_published_post_boot_audio_snapshot() {
    let mut apu = Apu::new(ConsoleModel::Dmg);

    apu.apply_startup_state(ApuStartupState {
        powered: true,
        nr10: 0x00,
        nr11: 0x80,
        nr12: 0xF3,
        nr13: 0x00,
        nr14: 0x00,
        nr21: 0x00,
        nr22: 0x00,
        nr23: 0x00,
        nr24: 0x00,
        nr30: 0x00,
        nr31: 0x00,
        nr32: 0x00,
        nr33: 0x00,
        nr34: 0x00,
        nr41: 0x00,
        nr42: 0x00,
        nr43: 0x00,
        nr44: 0x00,
        nr50: 0x77,
        nr51: 0xF3,
        channel_active_mask: CHANNEL_ACTIVE_CH1,
        div_apu: 0,
        wave_ram_startup_policy: WaveRamStartupPolicy::DeterministicZeroed,
    });

    assert_eq!(apu.read_register(0xFF10), 0x80);
    assert_eq!(apu.read_register(0xFF11), 0xBF);
    assert_eq!(apu.read_register(0xFF12), 0xF3);
    assert_eq!(apu.read_register(0xFF13), 0xFF);
    assert_eq!(apu.read_register(0xFF14), 0xBF);
    assert_eq!(apu.read_register(0xFF16), 0x3F);
    assert_eq!(apu.read_register(0xFF17), 0x00);
    assert_eq!(apu.read_register(0xFF18), 0xFF);
    assert_eq!(apu.read_register(0xFF19), 0xBF);
    assert_eq!(apu.read_register(0xFF1A), 0x7F);
    assert_eq!(apu.read_register(0xFF1B), 0xFF);
    assert_eq!(apu.read_register(0xFF1C), 0x9F);
    assert_eq!(apu.read_register(0xFF1D), 0xFF);
    assert_eq!(apu.read_register(0xFF1E), 0xBF);
    assert_eq!(apu.read_register(0xFF20), 0xFF);
    assert_eq!(apu.read_register(0xFF21), 0x00);
    assert_eq!(apu.read_register(0xFF22), 0x00);
    assert_eq!(apu.read_register(0xFF23), 0xBF);
    assert_eq!(apu.read_register(0xFF24), 0x77);
    assert_eq!(apu.read_register(0xFF25), 0xF3);
    assert_eq!(apu.read_register(0xFF26), 0xF1);
    assert_eq!(apu.read_register(0xFF30), 0x00);

    let snapshot = apu.snapshot();
    assert_eq!(snapshot.channel_active_mask, CHANNEL_ACTIVE_CH1);
    assert_eq!(snapshot.channel_dac_mask, CHANNEL_ACTIVE_CH1);
    assert_eq!(snapshot.div_apu, 0);
    assert_eq!(
        snapshot.wave_ram_startup_policy,
        WaveRamStartupPolicy::DeterministicZeroed
    );
}

#[test]
fn channel_2_3_and_4_register_paths_keep_dac_enable_and_trigger_distinct() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);

    apu.write_register(0xFF16, 0xC7);
    apu.write_register(0xFF17, 0x00);
    apu.write_register(0xFF18, 0x12);
    apu.write_register(0xFF19, 0x80);
    assert_eq!(apu.read_register(0xFF26), 0xF0);

    apu.write_register(0xFF17, 0xF3);
    apu.write_register(0xFF19, 0x80);
    assert_eq!(apu.read_register(0xFF26), 0xF2);

    apu.write_register(0xFF1A, 0x80);
    apu.write_register(0xFF1B, 0x55);
    apu.write_register(0xFF1D, 0x34);
    apu.write_register(0xFF1E, 0x80);
    assert_eq!(apu.read_register(0xFF26), 0xF6);

    apu.write_register(0xFF21, 0xF3);
    apu.write_register(0xFF22, 0x20);
    apu.write_register(0xFF23, 0x80);
    assert_eq!(apu.read_register(0xFF26), 0xFE);

    let snapshot = apu.snapshot();
    assert_eq!(snapshot.channel_active_mask, 0x0E);
    assert_eq!(snapshot.channel_dac_mask, 0x0E);

    apu.write_register(0xFF24, 0x77);
    apu.write_register(0xFF25, 0xF3);
    apu.write_register(0xFF15, 0x34);
    apu.write_register(0xFF1F, 0x12);
    apu.write_register(0xFF27, 0x56);
    apu.write_register(0xFF40, 0x78);

    assert_eq!(apu.read_register(0xFF16), 0xFF);
    assert_eq!(apu.read_register(0xFF17), 0xF3);
    assert_eq!(apu.read_register(0xFF18), 0xFF);
    assert_eq!(apu.read_register(0xFF19), 0xBF);
    assert_eq!(apu.read_register(0xFF1A), 0xFF);
    assert_eq!(apu.read_register(0xFF1B), 0xFF);
    assert_eq!(apu.read_register(0xFF1D), 0xFF);
    assert_eq!(apu.read_register(0xFF1E), 0xBF);
    assert_eq!(apu.read_register(0xFF21), 0xF3);
    assert_eq!(apu.read_register(0xFF22), 0x20);
    assert_eq!(apu.read_register(0xFF24), 0x77);
    assert_eq!(apu.read_register(0xFF25), 0xF3);
    assert_eq!(apu.read_register(0xFF1F), 0xFF);
    assert_eq!(apu.read_register(0xFF40), 0xFF);
}

#[test]
fn powered_off_startup_state_matches_the_nr52_power_off_contract() {
    let mut apu = Apu::new(ConsoleModel::Dmg);

    apu.apply_startup_state(ApuStartupState {
        powered: false,
        nr10: 0x7F,
        nr11: 0xFF,
        nr12: 0xF3,
        nr13: 0x12,
        nr14: 0xFF,
        nr21: 0xFF,
        nr22: 0xF3,
        nr23: 0x34,
        nr24: 0xFF,
        nr30: 0xFF,
        nr31: 0x56,
        nr32: 0xFF,
        nr33: 0x78,
        nr34: 0xFF,
        nr41: 0x9A,
        nr42: 0xF3,
        nr43: 0xBC,
        nr44: 0xFF,
        nr50: 0x77,
        nr51: 0xF3,
        channel_active_mask: 0x0F,
        div_apu: 0xFF,
        wave_ram_startup_policy: WaveRamStartupPolicy::DeterministicZeroed,
    });

    assert_eq!(apu.read_register(0xFF10), 0x80);
    assert_eq!(apu.read_register(0xFF12), 0x00);
    assert_eq!(apu.read_register(0xFF1A), 0x7F);
    assert_eq!(apu.read_register(0xFF24), 0x00);
    assert_eq!(apu.read_register(0xFF25), 0x00);
    assert_eq!(apu.read_register(0xFF26), 0x70);

    let snapshot = apu.snapshot();
    assert!(!snapshot.powered);
    assert_eq!(snapshot.channel_active_mask, 0x00);
    assert_eq!(snapshot.channel_dac_mask, 0x00);
    assert_eq!(snapshot.div_apu, 0x07);

    let context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);
    let trace = apu.scheduler_trace_message(&context);
    assert_eq!(
        trace,
        "t_cycle=0 phase=external_event_ingress console_model=Dmg status=Ready powered=false nr50=0x00 nr51=0x00 nr52=0x70 div_apu=7 active_mask=0x00 dac_mask=0x00 channel_digital_outputs=[0, 0, 0, 0] mixer=(0, 0) hpf=(0, 0)"
    );
}

#[test]
fn powered_off_apu_keeps_div_apu_phase_in_sync_with_shared_edges() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.apply_startup_state(ApuStartupState {
        powered: false,
        nr10: 0x00,
        nr11: 0x00,
        nr12: 0x00,
        nr13: 0x00,
        nr14: 0x00,
        nr21: 0x00,
        nr22: 0x00,
        nr23: 0x00,
        nr24: 0x00,
        nr30: 0x00,
        nr31: 0x00,
        nr32: 0x00,
        nr33: 0x00,
        nr34: 0x00,
        nr41: 0x00,
        nr42: 0x00,
        nr43: 0x00,
        nr44: 0x00,
        nr50: 0x00,
        nr51: 0x00,
        channel_active_mask: 0x00,
        div_apu: 0x05,
        wave_ram_startup_policy: WaveRamStartupPolicy::DeterministicZeroed,
    });

    tick_apu_with_edges(&mut apu, 0, &[DerivedEdge::ApuFrameSequencerEdge]);

    assert!(!apu.snapshot().powered);
    assert_eq!(apu.snapshot().div_apu, 0x06);
}

#[test]
fn div_apu_phase_can_be_derived_from_the_shared_system_counter() {
    assert_eq!(div_apu_phase_from_system_counter(0x0000), 0x00);
    assert_eq!(div_apu_phase_from_system_counter(0x2000), 0x01);
    assert_eq!(div_apu_phase_from_system_counter(0xABC8), 0x05);
}
