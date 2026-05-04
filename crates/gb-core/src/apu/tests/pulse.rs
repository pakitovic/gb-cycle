use super::*;
use crate::speed::CgbSpeedMode;

#[test]
fn channel_1_trigger_reloads_period_envelope_and_sweep_without_resetting_duty_step() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x11);
    apu.write_register(0xFF11, 0xBF);
    apu.write_register(0xFF12, 0xA2);
    apu.write_register(0xFF13, 0xAB);
    apu.channels.channel_1.pulse.duty_step = 5;

    apu.write_register(0xFF14, 0xC4);

    assert!(apu.channels.channel_1.pulse.runtime.active);
    assert_eq!(apu.channels.channel_1.pulse.duty_step, 5);
    assert_eq!(apu.channels.channel_1.pulse.length_counter, 1);
    assert_eq!(apu.channels.channel_1.pulse.envelope.current_volume, 0x0A);
    assert_eq!(apu.channels.channel_1.pulse.envelope.timer, 0x02);
    assert_eq!(
        apu.channels.channel_1.pulse.period_timer,
        pulse_timer_reload(0x04AB)
    );
    assert_eq!(apu.channels.channel_1.sweep.shadow_period, 0x04AB);
    assert_eq!(apu.channels.channel_1.sweep.timer, 0x01);
    assert!(apu.channels.channel_1.sweep.enabled);
}

#[test]
fn pulse_trigger_reloads_state_but_does_not_activate_while_the_dac_is_off() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);

    apu.write_register(0xFF10, 0x11);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0x00);
    apu.write_register(0xFF13, 0xAB);
    apu.channels.channel_1.pulse.period_timer = 0x0123;
    apu.channels.channel_1.pulse.envelope.timer = 5;
    apu.channels.channel_1.pulse.envelope.current_volume = 7;
    apu.channels.channel_1.sweep.shadow_period = 0x0456;
    apu.channels.channel_1.sweep.timer = 3;
    apu.channels.channel_1.sweep.enabled = true;

    apu.write_register(0xFF14, 0x80);

    assert!(!apu.channels.channel_1.pulse.runtime.active);
    assert_eq!(
        apu.channels.channel_1.pulse.period_timer,
        pulse_timer_reload_preserving_trigger_phase(0x00AB, 0x0123)
    );
    assert_eq!(
        apu.channels.channel_1.pulse.envelope.timer,
        envelope_timer_reload(0)
    );
    assert_eq!(apu.channels.channel_1.pulse.envelope.current_volume, 0);
    assert_eq!(apu.channels.channel_1.sweep.shadow_period, 0x00AB);
    assert_eq!(apu.channels.channel_1.sweep.timer, 1);
    assert!(apu.channels.channel_1.sweep.enabled);

    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0x00);
    apu.write_register(0xFF18, 0xCD);
    apu.channels.channel_2.pulse.period_timer = 0x0235;
    apu.channels.channel_2.pulse.envelope.timer = 6;
    apu.channels.channel_2.pulse.envelope.current_volume = 9;

    apu.write_register(0xFF19, 0x80);

    assert!(!apu.channels.channel_2.pulse.runtime.active);
    assert_eq!(
        apu.channels.channel_2.pulse.period_timer,
        pulse_timer_reload_preserving_trigger_phase(0x00CD, 0x0235)
    );
    assert_eq!(
        apu.channels.channel_2.pulse.envelope.timer,
        envelope_timer_reload(0)
    );
    assert_eq!(apu.channels.channel_2.pulse.envelope.current_volume, 0);
}

#[test]
fn channel_1_first_trigger_after_power_on_suppresses_the_initial_high_duty_output() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF11, 0x40);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0xFF);

    apu.write_register(0xFF14, 0x87);

    assert!(apu.channels.channel_1.pulse.runtime.active);
    assert_eq!(apu.channels.channel_1.pulse.duty_step, 0);
    assert!(pulse_waveform_high(
        apu.channels.channel_1.pulse.duty,
        apu.channels.channel_1.pulse.duty_step,
    ));
    assert!(apu.channels.channel_1.pulse.suppress_initial_trigger_output);
    assert_eq!(apu.channels.channel_1.pulse.current_digital_output(), 0);

    for _ in 0..4 {
        apu.channels.channel_1.tick_fast_timer();
    }

    assert_eq!(apu.channels.channel_1.pulse.duty_step, 1);
    assert!(!apu.channels.channel_1.pulse.suppress_initial_trigger_output);
}

#[test]
fn live_pulse_duty_write_waits_for_the_next_duty_step_boundary() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0xC0);
    apu.write_register(0xFF17, 0x80);
    apu.write_register(0xFF18, 0xFF);
    apu.write_register(0xFF19, 0x87);

    for _ in 0..4 {
        apu.channels.channel_2.tick_fast_timer();
    }
    assert_eq!(apu.channels.channel_2.pulse.duty_step, 1);
    assert_eq!(apu.channels.channel_2.pulse.duty, 3);
    assert_eq!(apu.channels.channel_2.pulse.current_digital_output(), 0x08);

    apu.write_register(0xFF16, 0x01);

    assert_eq!(apu.channels.channel_2.pulse.length_counter, 63);
    assert_eq!(apu.channels.channel_2.pulse.duty, 3);
    assert_eq!(apu.channels.channel_2.pulse.pending_duty, Some(0));
    assert_eq!(apu.channels.channel_2.pulse.current_digital_output(), 0x08);

    for _ in 0..4 {
        apu.channels.channel_2.tick_fast_timer();
    }

    assert_eq!(apu.channels.channel_2.pulse.duty_step, 2);
    assert_eq!(apu.channels.channel_2.pulse.duty, 0);
    assert_eq!(apu.channels.channel_2.pulse.pending_duty, None);
    assert_eq!(apu.channels.channel_2.pulse.current_digital_output(), 0);
}

#[test]
fn inactive_pulse_duty_write_updates_the_waveform_immediately() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);

    apu.write_register(0xFF16, 0x01);
    assert_eq!(apu.channels.channel_2.pulse.duty, 0);
    assert_eq!(apu.channels.channel_2.pulse.pending_duty, None);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 63);

    apu.write_register(0xFF16, 0xC0);

    assert_eq!(apu.channels.channel_2.pulse.duty, 3);
    assert_eq!(apu.channels.channel_2.pulse.pending_duty, None);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 64);
}

#[test]
fn save_state_preserves_live_pulse_pending_duty_write() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0xC0);
    apu.write_register(0xFF17, 0x80);
    apu.write_register(0xFF18, 0xFF);
    apu.write_register(0xFF19, 0x87);

    for _ in 0..4 {
        apu.channels.channel_2.tick_fast_timer();
    }
    apu.write_register(0xFF16, 0x01);
    assert_eq!(apu.channels.channel_2.pulse.pending_duty, Some(0));

    let mut uninterrupted = apu.clone();
    let saved = apu.capture_save_state();
    let mut restored = Apu::new(ConsoleModel::GameBoy);
    restored.restore_save_state(&saved);

    for _ in 0..4 {
        uninterrupted.channels.channel_2.tick_fast_timer();
        restored.channels.channel_2.tick_fast_timer();
    }

    assert_eq!(
        restored.capture_save_state(),
        uninterrupted.capture_save_state()
    );
    assert_eq!(restored.channels.channel_2.pulse.duty, 0);
    assert_eq!(restored.channels.channel_2.pulse.pending_duty, None);
}

#[test]
fn cgb_normal_speed_inactive_pulse_trigger_uses_fixed_startup_delay_before_first_duty_step() {
    let mut apu = Apu::new(ConsoleModel::GameBoyColor);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0x80);
    apu.write_register(0xFF18, 0xFF);

    for _ in 0..4 {
        apu.channels.channel_2.tick_fast_timer();
    }
    assert_eq!(apu.channels.channel_2.pulse.power_on_phase, 4);

    apu.write_register(0xFF19, 0x87);

    assert!(apu.channels.channel_2.pulse.runtime.active);
    assert_eq!(apu.channels.channel_2.pulse.trigger_delay_t_cycles, 8);
    assert!(apu.channels.channel_2.pulse.suppress_initial_trigger_output);

    for _ in 0..8 {
        apu.channels.channel_2.tick_fast_timer();
    }
    assert_eq!(apu.channels.channel_2.pulse.trigger_delay_t_cycles, 0);
    assert_eq!(apu.channels.channel_2.pulse.duty_step, 0);
    assert!(apu.channels.channel_2.pulse.suppress_initial_trigger_output);

    for _ in 0..4 {
        apu.channels.channel_2.tick_fast_timer();
    }
    assert_eq!(apu.channels.channel_2.pulse.duty_step, 1);
    assert!(!apu.channels.channel_2.pulse.suppress_initial_trigger_output);
}

#[test]
fn cgb_normal_speed_pulse_power_on_phase_does_not_change_the_fixed_startup_delay() {
    let mut apu = Apu::new(ConsoleModel::GameBoyColor);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0x80);
    apu.write_register(0xFF18, 0xFF);

    for _ in 0..12 {
        apu.channels.channel_2.tick_fast_timer();
    }
    assert_eq!(apu.channels.channel_2.pulse.power_on_phase, 4);

    apu.write_register(0xFF19, 0x87);

    assert_eq!(apu.channels.channel_2.pulse.trigger_delay_t_cycles, 8);
}

#[test]
fn cgb_double_speed_inactive_pulse_trigger_uses_cpu_visible_startup_delay() {
    let mut apu = Apu::new(ConsoleModel::GameBoyColor);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0x80);
    apu.write_register(0xFF18, 0xFF);

    for _ in 0..4 {
        apu.channels.channel_2.tick_fast_timer();
    }
    assert_eq!(apu.channels.channel_2.pulse.power_on_phase, 4);

    apu.write_register_for_speed(0xFF19, 0x87, CgbSpeedMode::Double);

    assert_eq!(apu.channels.channel_2.pulse.trigger_delay_t_cycles, 20);
}

#[test]
fn cgb_normal_speed_active_pulse_retrigger_uses_short_restart_delay_before_next_duty_step() {
    let mut apu = Apu::new(ConsoleModel::GameBoyColor);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0x80);
    apu.write_register(0xFF18, 0xFF);
    apu.write_register(0xFF19, 0x87);

    apu.channels.channel_2.pulse.suppress_initial_trigger_output = false;
    apu.channels.channel_2.pulse.duty_step = 4;
    apu.channels.channel_2.pulse.period_timer = pulse_timer_reload(0x07FF);

    apu.write_register(0xFF19, 0x87);

    assert!(apu.channels.channel_2.pulse.runtime.active);
    assert_eq!(apu.channels.channel_2.pulse.trigger_delay_t_cycles, 4);
    assert!(!apu.channels.channel_2.pulse.suppress_initial_trigger_output);
    assert_eq!(apu.channels.channel_2.pulse.duty_step, 4);
    assert_eq!(apu.channels.channel_2.pulse.current_digital_output(), 0);

    for _ in 0..4 {
        apu.channels.channel_2.tick_fast_timer();
    }

    assert_eq!(apu.channels.channel_2.pulse.trigger_delay_t_cycles, 0);
    assert_eq!(apu.channels.channel_2.pulse.period_timer, 4);
    assert_eq!(apu.channels.channel_2.pulse.duty_step, 4);
    assert_eq!(apu.channels.channel_2.pulse.current_digital_output(), 0);

    for _ in 0..4 {
        apu.channels.channel_2.tick_fast_timer();
    }

    assert_eq!(apu.channels.channel_2.pulse.duty_step, 5);
    assert_eq!(apu.channels.channel_2.pulse.current_digital_output(), 0x08);
}

#[test]
fn cgb_double_speed_active_pulse_retrigger_scales_the_restart_delay_in_cpu_visible_cycles() {
    let mut apu = Apu::new(ConsoleModel::GameBoyColor);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0x80);
    apu.write_register(0xFF18, 0xFF);
    apu.write_register(0xFF19, 0x87);
    apu.channels.channel_2.pulse.suppress_initial_trigger_output = false;

    apu.write_register_for_speed(0xFF19, 0x87, CgbSpeedMode::Double);

    assert_eq!(apu.channels.channel_2.pulse.trigger_delay_t_cycles, 8);
    assert!(!apu.channels.channel_2.pulse.suppress_initial_trigger_output);
}

#[test]
fn cgb_pulse_dac_disable_freezes_the_generation_timer_until_the_next_real_trigger() {
    let mut apu = Apu::new(ConsoleModel::GameBoyColor);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0x80);
    apu.write_register(0xFF18, 0xFC);
    apu.write_register(0xFF19, 0x87);
    apu.channels.channel_2.pulse.suppress_initial_trigger_output = false;
    apu.channels.channel_2.pulse.duty_step = 4;
    apu.channels.channel_2.pulse.period_timer = 1;

    apu.write_register(0xFF17, 0x00);

    assert!(!apu.channels.channel_2.pulse.runtime.active);
    assert!(!apu.channels.channel_2.pulse.runtime.dac_enabled);
    assert!(apu.channels.channel_2.pulse.timer_stopped_by_dac_disable);

    for _ in 0..8 {
        apu.channels.channel_2.tick_fast_timer();
    }

    assert_eq!(apu.channels.channel_2.pulse.duty_step, 4);
    assert_eq!(apu.channels.channel_2.pulse.period_timer, 1);

    apu.write_register(0xFF17, 0x80);

    assert!(!apu.channels.channel_2.pulse.runtime.active);
    assert!(apu.channels.channel_2.pulse.runtime.dac_enabled);
    assert!(apu.channels.channel_2.pulse.timer_stopped_by_dac_disable);

    for _ in 0..8 {
        apu.channels.channel_2.tick_fast_timer();
    }

    assert_eq!(apu.channels.channel_2.pulse.duty_step, 4);
    assert_eq!(apu.channels.channel_2.pulse.period_timer, 1);

    apu.write_register(0xFF19, 0x87);

    assert!(apu.channels.channel_2.pulse.runtime.active);
    assert!(!apu.channels.channel_2.pulse.timer_stopped_by_dac_disable);
    assert_eq!(apu.channels.channel_2.pulse.duty_step, 4);
    assert_eq!(
        apu.channels.channel_2.pulse.period_timer,
        pulse_timer_reload_preserving_trigger_phase(0x07FC, 1)
    );
    assert_eq!(apu.channels.channel_2.pulse.trigger_delay_t_cycles, 8);
}

#[test]
fn save_state_preserves_the_pulse_timer_stopped_by_dac_disable_latch() {
    let mut apu = Apu::new(ConsoleModel::GameBoyColor);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0x80);
    apu.write_register(0xFF18, 0xFC);
    apu.write_register(0xFF19, 0x87);
    apu.channels.channel_2.pulse.suppress_initial_trigger_output = false;
    apu.channels.channel_2.pulse.duty_step = 4;
    apu.channels.channel_2.pulse.period_timer = 1;
    apu.write_register(0xFF17, 0x00);

    let mut uninterrupted = apu.clone();
    let saved = apu.capture_save_state();
    let mut restored = Apu::new(ConsoleModel::GameBoyColor);
    restored.restore_save_state(&saved);

    for _ in 0..8 {
        uninterrupted.channels.channel_2.tick_fast_timer();
        restored.channels.channel_2.tick_fast_timer();
    }

    assert_eq!(
        restored.capture_save_state(),
        uninterrupted.capture_save_state()
    );
    assert!(
        restored
            .channels
            .channel_2
            .pulse
            .timer_stopped_by_dac_disable
    );
    assert_eq!(restored.channels.channel_2.pulse.duty_step, 4);
    assert_eq!(restored.channels.channel_2.pulse.period_timer, 1);
}

#[test]
fn cgb_double_speed_pulse_generation_timers_tick_on_the_normal_speed_domain() {
    let mut apu = Apu::new(ConsoleModel::GameBoyColor);
    apu.write_register(0xFF26, 0x80);
    apu.channels
        .channel_2
        .pulse
        .first_trigger_after_power_on_pending = false;
    apu.channels.channel_2.pulse.timer_stopped_by_dac_disable = false;
    apu.channels.channel_2.pulse.period_timer = 1;

    let odd_context = CycleContext::for_cycle(TCycle::new(1));
    apu.tick_t_cycle_for_speed(&odd_context, CgbSpeedMode::Double);

    assert_eq!(apu.channels.channel_2.pulse.period_timer, 1);
    assert_eq!(apu.channels.channel_2.pulse.duty_step, 0);

    let even_context = CycleContext::for_cycle(TCycle::new(2));
    apu.tick_t_cycle_for_speed(&even_context, CgbSpeedMode::Double);

    assert_eq!(
        apu.channels.channel_2.pulse.period_timer,
        pulse_timer_reload(0)
    );
    assert_eq!(apu.channels.channel_2.pulse.duty_step, 1);
}

#[test]
fn channel_2_retrigger_after_the_first_post_power_on_trigger_does_not_resuppress_output() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x40);
    apu.write_register(0xFF17, 0xF0);
    apu.write_register(0xFF18, 0xFF);

    apu.write_register(0xFF19, 0x87);
    assert!(apu.channels.channel_2.pulse.suppress_initial_trigger_output);

    for _ in 0..4 {
        apu.channels.channel_2.tick_fast_timer();
    }

    assert!(!apu.channels.channel_2.pulse.suppress_initial_trigger_output);

    apu.channels.channel_2.pulse.duty_step = 0;
    apu.write_register(0xFF19, 0x87);

    assert_eq!(apu.channels.channel_2.pulse.trigger_delay_t_cycles, 0);
    assert!(!apu.channels.channel_2.pulse.suppress_initial_trigger_output);
    assert_eq!(apu.channels.channel_2.pulse.current_digital_output(), 0x0F);
}

#[test]
fn triggering_a_pulse_channel_from_inactive_state_resuppresses_output_until_the_next_duty_step() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x40);
    apu.write_register(0xFF17, 0xF0);
    apu.write_register(0xFF18, 0xFF);

    apu.write_register(0xFF19, 0x87);
    for _ in 0..4 {
        apu.channels.channel_2.tick_fast_timer();
    }

    assert!(!apu.channels.channel_2.pulse.suppress_initial_trigger_output);

    apu.channels.channel_2.pulse.runtime.active = false;
    apu.channels.channel_2.pulse.duty_step = 0;
    apu.channels.channel_2.pulse.suppress_initial_trigger_output = false;

    apu.write_register(0xFF19, 0x87);

    assert!(apu.channels.channel_2.pulse.runtime.active);
    assert!(apu.channels.channel_2.pulse.suppress_initial_trigger_output);
    assert_eq!(apu.channels.channel_2.pulse.current_digital_output(), 0);

    for _ in 0..4 {
        apu.channels.channel_2.tick_fast_timer();
    }

    assert!(!apu.channels.channel_2.pulse.suppress_initial_trigger_output);
    assert_eq!(apu.channels.channel_2.pulse.duty_step, 1);
}

#[test]
fn pulse_fast_timer_stays_frozen_until_the_first_trigger_after_power_on() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0xF0);
    apu.write_register(0xFF18, 0xFF);

    apu.channels.channel_2.pulse.duty_step = 3;
    apu.channels.channel_2.pulse.period_timer = 1;

    apu.channels.channel_2.tick_fast_timer();

    assert_eq!(apu.channels.channel_2.pulse.duty_step, 3);
    assert_eq!(apu.channels.channel_2.pulse.period_timer, 1);
    assert!(
        apu.channels
            .channel_2
            .pulse
            .first_trigger_after_power_on_pending
    );
}

#[test]
fn nr52_power_cycle_rearms_the_first_trigger_after_power_on_pulse_suppression() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF11, 0x40);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0xFF);
    apu.write_register(0xFF14, 0x87);

    assert!(apu.channels.channel_1.pulse.suppress_initial_trigger_output);

    for _ in 0..4 {
        apu.channels.channel_1.tick_fast_timer();
    }

    assert!(!apu.channels.channel_1.pulse.suppress_initial_trigger_output);

    apu.write_register(0xFF26, 0x00);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF11, 0x40);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0xFF);
    apu.write_register(0xFF14, 0x87);

    assert!(apu.channels.channel_1.pulse.suppress_initial_trigger_output);
    assert_eq!(apu.channels.channel_1.pulse.current_digital_output(), 0);
}

#[test]
fn triggering_a_pulse_channel_preserves_the_underlying_timer_phase() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0xF0);
    apu.write_register(0xFF18, 0xFE);
    apu.channels.channel_2.pulse.period_timer = 0x0001;

    apu.write_register(0xFF19, 0x87);

    assert_eq!(
        apu.channels.channel_2.pulse.period_timer,
        pulse_timer_reload_preserving_trigger_phase(0x07FE, 0x0001)
    );
}

#[test]
fn pulse_trigger_phase_preservation_matches_all_four_t_cycle_subphases() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0xF0);
    apu.write_register(0xFF18, 0xFE);

    for (current_period_timer, expected_period_timer) in [(4, 8), (3, 7), (2, 6), (1, 5)] {
        apu.channels.channel_2.pulse.period_timer = current_period_timer;
        apu.write_register(0xFF19, 0x87);
        assert_eq!(
            apu.channels.channel_2.pulse.period_timer, expected_period_timer,
            "current_period_timer={current_period_timer}"
        );
    }
}

#[test]
fn triggering_a_pulse_channel_just_before_an_envelope_step_reloads_the_timer_with_plus_one() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0xF2);

    apu.write_register(0xFF19, 0x80);

    assert_eq!(
        apu.channels.channel_2.pulse.envelope.timer,
        envelope_timer_reload(0x02) + 1
    );
}

#[test]
fn enabling_pulse_length_on_a_non_length_step_clocks_it_immediately() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(1);
    apu.write_register(0xFF11, 0xBF);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF14, 0x80);

    assert!(apu.channels.channel_1.pulse.runtime.active);
    assert!(!apu.channels.channel_1.pulse.length_enabled);
    assert_eq!(apu.channels.channel_1.pulse.length_counter, 1);

    apu.write_register(0xFF14, LENGTH_ENABLE_BIT);

    assert!(apu.channels.channel_1.pulse.length_enabled);
    assert_eq!(apu.channels.channel_1.pulse.length_counter, 0);
    assert!(!apu.channels.channel_1.pulse.runtime.active);
}

#[test]
fn enabling_pulse_length_on_a_length_step_does_not_clock_it() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(0);
    apu.write_register(0xFF11, 0xBF);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF14, 0x80);

    assert!(apu.channels.channel_1.pulse.runtime.active);
    assert!(!apu.channels.channel_1.pulse.length_enabled);
    assert_eq!(apu.channels.channel_1.pulse.length_counter, 1);

    apu.write_register(0xFF14, LENGTH_ENABLE_BIT);

    assert!(apu.channels.channel_1.pulse.length_enabled);
    assert_eq!(apu.channels.channel_1.pulse.length_counter, 1);
    assert!(apu.channels.channel_1.pulse.runtime.active);
}

#[test]
fn pulse_trigger_rom_second_half_enable_keeps_length_unchanged_before_retrigger() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(6);
    prime_pulse_trigger_test(&mut apu, &CHANNEL_2, 2);

    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 2);
    assert!(apu.channels.channel_2.pulse.runtime.active);

    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channels.channel_2.pulse.length_counter, 2);
    assert!(apu.channels.channel_2.pulse.runtime.active);
}

#[test]
fn pulse_trigger_rom_first_half_enable_clocks_once_and_survives_the_intervening_non_length_edge() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_pulse_trigger_test(&mut apu, &CHANNEL_2, 2);

    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 1);
    assert!(apu.channels.channel_2.pulse.runtime.active);

    tick_apu_with_edges(&mut apu, 0, &[DerivedEdge::ApuFrameSequencerEdge]);
    assert_eq!(apu.snapshot().div_apu, 0x00);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 1);
    assert!(apu.channels.channel_2.pulse.runtime.active);

    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channels.channel_2.pulse.length_counter, 1);
    assert!(apu.channels.channel_2.pulse.runtime.active);
}

#[test]
fn triggering_a_zero_length_pulse_with_length_enabled_reloads_and_clocks_it() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_pulse_trigger_test(&mut apu, &CHANNEL_2, 1);

    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 0);
    assert!(!apu.channels.channel_2.pulse.runtime.active);

    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channels.channel_2.pulse.length_counter, 63);
    assert!(apu.channels.channel_2.pulse.runtime.active);
}

#[test]
fn triggering_a_length_one_pulse_with_enable_on_the_same_first_half_write_matches_the_unfrozen_case()
 {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_pulse_trigger_test(&mut apu, &CHANNEL_2, 1);

    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channels.channel_2.pulse.length_counter, 63);
    assert!(apu.channels.channel_2.pulse.runtime.active);
}

#[test]
fn triggering_a_nonzero_length_pulse_does_not_change_its_length_counter() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(6);
    prime_pulse_trigger_test(&mut apu, &CHANNEL_2, 2);
    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);

    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channels.channel_2.pulse.length_counter, 2);
    assert!(apu.channels.channel_2.pulse.runtime.active);
}

#[test]
fn writes_other_than_disabling_to_enabled_do_not_extra_clock_pulse_length() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_pulse_trigger_test(&mut apu, &CHANNEL_2, 2);

    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 1);

    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 1);

    apu.write_register(0xFF19, 0x00);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 1);

    apu.write_register(0xFF19, 0x00);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 1);
}

#[test]
fn writing_length_after_enabling_it_matches_the_trigger_rom_sequence() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
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

    assert_eq!(apu.channels.channel_2.pulse.length_counter, 2);
    assert!(!apu.channels.channel_2.pulse.length_enabled);
    assert!(apu.channels.channel_2.pulse.runtime.active);
}

#[test]
fn extra_length_clocking_to_zero_disables_the_pulse_channel() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_pulse_trigger_test(&mut apu, &CHANNEL_2, 1);

    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);

    assert_eq!(apu.channels.channel_2.pulse.length_counter, 0);
    assert!(!apu.channels.channel_2.pulse.runtime.active);
}

#[test]
fn enabling_length_again_after_it_reached_zero_does_not_clock_or_unfreeze_it() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_pulse_trigger_test(&mut apu, &CHANNEL_2, 1);

    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 0);

    apu.write_register(0xFF19, 0x00);
    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 0);

    apu.write_register(0xFF19, 0x00);
    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 0);
}

#[test]
fn triggering_a_zero_length_pulse_with_length_disabled_unfreezes_it_to_the_full_reload() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_pulse_trigger_test(&mut apu, &CHANNEL_2, 1);

    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    apu.write_register(0xFF19, 0x00);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 0);
    assert!(!apu.channels.channel_2.pulse.length_enabled);

    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT);

    assert_eq!(apu.channels.channel_2.pulse.length_counter, 64);
    assert!(apu.channels.channel_2.pulse.runtime.active);
}

#[test]
fn disabled_dac_still_allows_trigger_to_reload_and_clock_pulse_length() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_pulse_trigger_test(&mut apu, &CHANNEL_2, 1);

    apu.write_register(0xFF17, 0x00);
    assert!(!apu.channels.channel_2.pulse.runtime.dac_enabled);
    assert!(!apu.channels.channel_2.pulse.runtime.active);

    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channels.channel_2.pulse.length_counter, 63);
    assert!(!apu.channels.channel_2.pulse.runtime.active);

    apu.write_register(0xFF17, 0x08);
    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT);

    assert_eq!(apu.channels.channel_2.pulse.length_counter, 63);
    assert!(apu.channels.channel_2.pulse.runtime.active);
}

#[test]
fn channel_1_first_half_enable_clocks_length_once_before_retrigger() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_pulse_trigger_test(&mut apu, &CHANNEL_1, 2);

    apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_1.pulse.length_counter, 1);
    assert!(apu.channels.channel_1.pulse.runtime.active);

    tick_apu_with_edges(&mut apu, 0, &[DerivedEdge::ApuFrameSequencerEdge]);
    apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channels.channel_1.pulse.length_counter, 1);
    assert!(apu.channels.channel_1.pulse.runtime.active);
}

#[test]
fn channel_1_trigger_with_zero_length_enabled_reloads_and_clocks_it() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_pulse_trigger_test(&mut apu, &CHANNEL_1, 1);

    apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_1.pulse.length_counter, 0);
    assert!(!apu.channels.channel_1.pulse.runtime.active);

    apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channels.channel_1.pulse.length_counter, 63);
    assert!(apu.channels.channel_1.pulse.runtime.active);
}

#[test]
fn channel_1_trigger_unfreezes_zero_length_and_clocks_it_after_disabling_length() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_pulse_trigger_test(&mut apu, &CHANNEL_1, 1);

    apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_1.pulse.length_counter, 0);
    assert!(!apu.channels.channel_1.pulse.runtime.active);

    apu.write_register(0xFF14, 0x00);
    assert_eq!(apu.channels.channel_1.pulse.length_counter, 0);
    assert!(!apu.channels.channel_1.pulse.length_enabled);

    apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channels.channel_1.pulse.length_counter, 63);
    assert!(apu.channels.channel_1.pulse.runtime.active);
}

#[test]
fn channel_1_retrigger_after_unfreezing_zero_length_does_not_extra_clock_again() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_pulse_trigger_test(&mut apu, &CHANNEL_1, 1);

    apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_1.pulse.length_counter, 0);
    assert!(!apu.channels.channel_1.pulse.runtime.active);

    apu.write_register(0xFF14, 0x00);
    apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_1.pulse.length_counter, 63);
    assert!(apu.channels.channel_1.pulse.runtime.active);

    apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channels.channel_1.pulse.length_counter, 63);
    assert!(apu.channels.channel_1.pulse.runtime.active);
}

#[test]
fn trigger_unfreezes_zero_length_then_a_later_enable_allows_normal_length_clocks() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_pulse_trigger_test(&mut apu, &CHANNEL_2, 1);

    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 0);
    assert!(!apu.channels.channel_2.pulse.runtime.active);

    apu.write_register(0xFF19, 0x00);
    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 64);
    assert!(apu.channels.channel_2.pulse.runtime.active);

    tick_apu_with_edges(&mut apu, 0, &[DerivedEdge::ApuFrameSequencerEdge]);
    assert_eq!(apu.snapshot().div_apu, 0x00);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 64);

    apu.write_register(0xFF19, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 64);

    tick_apu_with_edges(&mut apu, 1, &[DerivedEdge::ApuFrameSequencerEdge]);
    tick_apu_with_edges(&mut apu, 2, &[DerivedEdge::ApuFrameSequencerEdge]);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 63);
    assert!(apu.channels.channel_2.pulse.runtime.active);

    tick_apu_with_edges(&mut apu, 3, &[DerivedEdge::ApuFrameSequencerEdge]);
    tick_apu_with_edges(&mut apu, 4, &[DerivedEdge::ApuFrameSequencerEdge]);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 62);
    assert!(apu.channels.channel_2.pulse.runtime.active);
}

#[test]
fn channel_1_retrigger_after_two_zero_length_freezes_only_extra_clocks_on_real_unfreeze_points() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    prime_pulse_trigger_test(&mut apu, &CHANNEL_1, 1);

    apu.write_register(0xFF14, 0x00);
    apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_1.pulse.length_counter, 0);
    assert!(!apu.channels.channel_1.pulse.runtime.active);

    apu.write_register(0xFF14, 0x00);
    apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_1.pulse.length_counter, 0);
    assert!(!apu.channels.channel_1.pulse.runtime.active);

    apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT);
    assert_eq!(apu.channels.channel_1.pulse.length_counter, 64);
    assert!(apu.channels.channel_1.pulse.runtime.active);
    assert!(!apu.channels.channel_1.pulse.length_enabled);

    apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_1.pulse.length_counter, 63);
    assert!(apu.channels.channel_1.pulse.runtime.active);

    apu.write_register(0xFF14, 0x00);
    assert_eq!(apu.channels.channel_1.pulse.length_counter, 63);
    assert!(apu.channels.channel_1.pulse.runtime.active);
    assert!(!apu.channels.channel_1.pulse.length_enabled);

    apu.write_register(0xFF14, LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_1.pulse.length_counter, 62);
    assert!(apu.channels.channel_1.pulse.runtime.active);

    apu.write_register(0xFF14, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_1.pulse.length_counter, 62);
    assert!(apu.channels.channel_1.pulse.runtime.active);
}

#[test]
fn triggering_a_zero_length_pulse_on_a_non_length_step_reloads_it_to_63() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(1);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0xF0);
    apu.channels.channel_2.pulse.length_counter = 0;

    apu.write_register(0xFF19, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert!(apu.channels.channel_2.pulse.runtime.active);
    assert!(apu.channels.channel_2.pulse.length_enabled);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 63);
}

#[test]
fn pulse_period_writes_take_effect_only_after_the_current_sample_finishes() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0xF0);
    apu.write_register(0xFF18, 0xFF);
    apu.write_register(0xFF19, 0x87);

    assert_eq!(apu.channels.channel_2.pulse.period_timer, 4);

    apu.channels.channel_2.tick_fast_timer();
    apu.channels.channel_2.tick_fast_timer();
    assert_eq!(apu.channels.channel_2.pulse.period_timer, 2);

    apu.write_register(0xFF18, 0xFE);
    apu.write_register(0xFF19, 0x07);
    assert_eq!(apu.channels.channel_2.period_value(), 0x07FE);
    assert_eq!(apu.channels.channel_2.pulse.period_timer, 2);

    apu.channels.channel_2.tick_fast_timer();
    assert_eq!(apu.channels.channel_2.pulse.period_timer, 1);
    apu.channels.channel_2.tick_fast_timer();

    assert_eq!(apu.channels.channel_2.pulse.period_timer, 8);
    assert_eq!(apu.channels.channel_2.pulse.duty_step, 1);
    assert_eq!(apu.channels.channel_2.pulse.current_digital_output(), 0);
}

#[test]
fn frame_sequencer_length_and_envelope_clocks_drive_pulse_channel_state() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF11, 0xBF);
    apu.write_register(0xFF12, 0x11);
    apu.write_register(0xFF14, 0xC0);
    apu.write_register(0xFF16, 0x3F);
    apu.write_register(0xFF17, 0x21);
    apu.write_register(0xFF19, 0xC0);

    apu.frame_sequencer.apply_startup_phase(7);
    tick_apu_with_edges(&mut apu, 0, &[DerivedEdge::ApuFrameSequencerEdge]);

    assert_eq!(apu.channels.channel_1.pulse.envelope.current_volume, 0);
    assert_eq!(apu.channels.channel_2.pulse.envelope.current_volume, 1);
    assert!(apu.channels.channel_1.pulse.runtime.active);
    assert!(apu.channels.channel_2.pulse.runtime.active);

    apu.frame_sequencer.apply_startup_phase(0);
    tick_apu_with_edges(&mut apu, 1, &[DerivedEdge::ApuFrameSequencerEdge]);

    assert!(!apu.channels.channel_1.pulse.runtime.active);
    assert!(!apu.channels.channel_2.pulse.runtime.active);
}

#[test]
fn channel_1_sweep_clock_writes_back_shadow_period_and_runs_the_second_overflow_check() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x11);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0x00);
    apu.write_register(0xFF14, 0x85);

    assert_eq!(apu.channels.channel_1.period_value(), 0x0500);
    assert!(apu.channels.channel_1.pulse.runtime.active);

    apu.channels.channel_1.clock_sweep(ConsoleModel::GameBoy);

    assert_eq!(apu.channels.channel_1.period_value(), 0x0780);
    assert_eq!(apu.channels.channel_1.sweep.shadow_period, 0x0780);
    assert!(!apu.channels.channel_1.pulse.runtime.active);
}

fn prime_cgb_shift_zero_sweep_restart_overflow() -> Apu {
    let mut apu = Apu::new(ConsoleModel::GameBoyColor);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x10);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0xFF);
    apu.write_register(0xFF14, 0x83);
    apu.write_register(0xFF14, 0x87);
    apu
}

#[test]
fn cgb_channel_1_sweep_second_overflow_check_is_delayed_after_period_writeback() {
    let mut apu = Apu::new(ConsoleModel::GameBoyColor);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x27);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0xF0);
    apu.write_register(0xFF14, 0x87);

    assert_eq!(apu.channels.channel_1.period_value(), 0x07F0);
    assert!(apu.channels.channel_1.pulse.runtime.active);

    apu.channels
        .channel_1
        .clock_sweep(ConsoleModel::GameBoyColor);
    assert_eq!(apu.channels.channel_1.period_value(), 0x07F0);

    apu.channels
        .channel_1
        .clock_sweep(ConsoleModel::GameBoyColor);

    assert_eq!(apu.channels.channel_1.period_value(), 0x07FF);
    assert_eq!(apu.channels.channel_1.sweep.shadow_period, 0x07FF);
    assert!(apu.channels.channel_1.pulse.runtime.active);

    for _ in 0..31 {
        apu.channels.channel_1.tick_fast_timer();
        assert!(apu.channels.channel_1.pulse.runtime.active);
    }

    apu.channels.channel_1.tick_fast_timer();

    assert!(!apu.channels.channel_1.pulse.runtime.active);
}

#[test]
fn cgb_channel_1_sweep_delayed_overflow_check_survives_save_state() {
    let mut uninterrupted = Apu::new(ConsoleModel::GameBoyColor);
    uninterrupted.write_register(0xFF26, 0x80);
    uninterrupted.write_register(0xFF10, 0x27);
    uninterrupted.write_register(0xFF11, 0x80);
    uninterrupted.write_register(0xFF12, 0xF0);
    uninterrupted.write_register(0xFF13, 0xF0);
    uninterrupted.write_register(0xFF14, 0x87);
    uninterrupted
        .channels
        .channel_1
        .clock_sweep(ConsoleModel::GameBoyColor);
    uninterrupted
        .channels
        .channel_1
        .clock_sweep(ConsoleModel::GameBoyColor);

    for _ in 0..12 {
        uninterrupted.channels.channel_1.tick_fast_timer();
    }

    let saved = uninterrupted.capture_save_state();
    let mut restored = Apu::new(ConsoleModel::GameBoyColor);
    restored.restore_save_state(&saved);

    for _ in 0..20 {
        uninterrupted.channels.channel_1.tick_fast_timer();
        restored.channels.channel_1.tick_fast_timer();
    }

    assert_eq!(
        restored.capture_save_state(),
        uninterrupted.capture_save_state()
    );
    assert!(!restored.channels.channel_1.pulse.runtime.active);
}

#[test]
fn cgb_channel_1_shift_zero_sweep_restart_hold_defers_boundary_overflow() {
    let mut apu = prime_cgb_shift_zero_sweep_restart_overflow();

    assert_eq!(apu.channels.channel_1.period_value(), 0x07FF);
    assert!(apu.channels.channel_1.pulse.runtime.active);
    assert_eq!(
        apu.channels.channel_1.sweep.restart_hold_t_cycles,
        CGB_CH1_SWEEP_RESTART_HOLD_T_CYCLES
    );

    apu.channels
        .channel_1
        .clock_sweep(ConsoleModel::GameBoyColor);

    assert!(apu.channels.channel_1.pulse.runtime.active);
    assert_eq!(apu.channels.channel_1.sweep.delayed_calculation_t_cycles, 0);

    for _ in 0..(CGB_CH1_SWEEP_RESTART_HOLD_T_CYCLES - 1) {
        apu.channels.channel_1.tick_fast_timer();
    }
    apu.channels
        .channel_1
        .clock_sweep(ConsoleModel::GameBoyColor);

    assert!(apu.channels.channel_1.pulse.runtime.active);
    assert_eq!(apu.channels.channel_1.sweep.delayed_calculation_t_cycles, 0);

    apu.channels.channel_1.tick_fast_timer();
    apu.channels
        .channel_1
        .clock_sweep(ConsoleModel::GameBoyColor);

    assert!(apu.channels.channel_1.pulse.runtime.active);
    assert_eq!(
        apu.channels.channel_1.sweep.delayed_calculation_t_cycles,
        CGB_SWEEP_UNSHIFTED_DELAYED_CALCULATION_T_CYCLES
    );

    for _ in 0..(CGB_SWEEP_UNSHIFTED_DELAYED_CALCULATION_T_CYCLES - 1) {
        apu.channels.channel_1.tick_fast_timer();
        assert!(apu.channels.channel_1.pulse.runtime.active);
    }

    apu.channels.channel_1.tick_fast_timer();

    assert!(!apu.channels.channel_1.pulse.runtime.active);
}

#[test]
fn cgb_channel_1_shift_zero_sweep_restart_hold_survives_save_state() {
    let mut uninterrupted = prime_cgb_shift_zero_sweep_restart_overflow();
    for _ in 0..4 {
        uninterrupted.channels.channel_1.tick_fast_timer();
    }

    let saved = uninterrupted.capture_save_state();
    let mut restored = Apu::new(ConsoleModel::GameBoyColor);
    restored.restore_save_state(&saved);

    uninterrupted
        .channels
        .channel_1
        .clock_sweep(ConsoleModel::GameBoyColor);
    restored
        .channels
        .channel_1
        .clock_sweep(ConsoleModel::GameBoyColor);
    for _ in 0..CGB_SWEEP_UNSHIFTED_DELAYED_CALCULATION_T_CYCLES {
        uninterrupted.channels.channel_1.tick_fast_timer();
        restored.channels.channel_1.tick_fast_timer();
    }

    assert!(uninterrupted.channels.channel_1.pulse.runtime.active);
    assert!(restored.channels.channel_1.pulse.runtime.active);
    assert_eq!(
        restored.capture_save_state(),
        uninterrupted.capture_save_state()
    );

    for _ in 0..(CGB_CH1_SWEEP_RESTART_HOLD_T_CYCLES
        - 4
        - CGB_SWEEP_UNSHIFTED_DELAYED_CALCULATION_T_CYCLES)
    {
        uninterrupted.channels.channel_1.tick_fast_timer();
        restored.channels.channel_1.tick_fast_timer();
    }
    uninterrupted
        .channels
        .channel_1
        .clock_sweep(ConsoleModel::GameBoyColor);
    restored
        .channels
        .channel_1
        .clock_sweep(ConsoleModel::GameBoyColor);
    for _ in 0..CGB_SWEEP_UNSHIFTED_DELAYED_CALCULATION_T_CYCLES {
        uninterrupted.channels.channel_1.tick_fast_timer();
        restored.channels.channel_1.tick_fast_timer();
    }

    assert_eq!(
        restored.capture_save_state(),
        uninterrupted.capture_save_state()
    );
    assert!(!restored.channels.channel_1.pulse.runtime.active);
}

#[test]
fn cgb_channel_1_trigger_sweep_overflow_check_is_delayed() {
    let mut apu = Apu::new(ConsoleModel::GameBoyColor);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x17);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0xFF);
    apu.write_register(0xFF14, 0x87);

    apu.write_register(0xFF14, 0x87);

    assert!(apu.channels.channel_1.pulse.runtime.active);

    for _ in 0..35 {
        apu.channels.channel_1.tick_fast_timer();
        assert!(apu.channels.channel_1.pulse.runtime.active);
    }

    apu.channels.channel_1.tick_fast_timer();

    assert!(!apu.channels.channel_1.pulse.runtime.active);
}

#[test]
fn cgb_channel_1_decreasing_sweep_writeback_extends_active_retrigger_hold() {
    let mut apu = Apu::new(ConsoleModel::GameBoyColor);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x1F);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0xFF);
    apu.write_register(0xFF14, 0x87);
    apu.channels.channel_1.pulse.trigger_delay_t_cycles = 0;
    apu.channels
        .channel_1
        .clock_sweep(ConsoleModel::GameBoyColor);

    assert_eq!(apu.channels.channel_1.period_value(), 0x07F0);

    apu.write_register(0xFF14, 0x87);

    assert_eq!(apu.channels.channel_1.pulse.trigger_delay_t_cycles, 8);
}

#[test]
fn cgb_channel_1_increasing_sweep_writeback_keeps_base_active_retrigger_hold() {
    let mut apu = Apu::new(ConsoleModel::GameBoyColor);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x17);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0xF0);
    apu.write_register(0xFF14, 0x87);
    apu.channels.channel_1.pulse.trigger_delay_t_cycles = 0;
    apu.channels
        .channel_1
        .clock_sweep(ConsoleModel::GameBoyColor);

    assert_eq!(apu.channels.channel_1.period_value(), 0x07FF);

    apu.write_register(0xFF14, 0x87);

    assert_eq!(apu.channels.channel_1.pulse.trigger_delay_t_cycles, 4);
}

#[test]
fn cgb_channel_1_decreasing_sweep_restart_hold_survives_save_state() {
    let mut uninterrupted = Apu::new(ConsoleModel::GameBoyColor);
    uninterrupted.write_register(0xFF26, 0x80);
    uninterrupted.write_register(0xFF10, 0x1F);
    uninterrupted.write_register(0xFF11, 0x80);
    uninterrupted.write_register(0xFF12, 0xF0);
    uninterrupted.write_register(0xFF13, 0xFF);
    uninterrupted.write_register(0xFF14, 0x87);
    uninterrupted
        .channels
        .channel_1
        .pulse
        .trigger_delay_t_cycles = 0;
    uninterrupted
        .channels
        .channel_1
        .clock_sweep(ConsoleModel::GameBoyColor);

    let saved = uninterrupted.capture_save_state();
    let mut restored = Apu::new(ConsoleModel::GameBoyColor);
    restored.restore_save_state(&saved);

    uninterrupted.write_register(0xFF14, 0x87);
    restored.write_register(0xFF14, 0x87);

    assert_eq!(
        restored.capture_save_state(),
        uninterrupted.capture_save_state()
    );
    assert_eq!(restored.channels.channel_1.pulse.trigger_delay_t_cycles, 8);
}

#[test]
fn cgb_channel_1_nr10_zero_write_cancels_pending_sweep_overflow_check() {
    let mut apu = Apu::new(ConsoleModel::GameBoyColor);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x27);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0xF0);
    apu.write_register(0xFF14, 0x87);
    apu.channels
        .channel_1
        .clock_sweep(ConsoleModel::GameBoyColor);
    apu.channels
        .channel_1
        .clock_sweep(ConsoleModel::GameBoyColor);

    apu.write_register(0xFF10, 0x00);

    for _ in 0..40 {
        apu.channels.channel_1.tick_fast_timer();
    }

    assert!(apu.channels.channel_1.pulse.runtime.active);
    assert_eq!(apu.channels.channel_1.period_value(), 0x07FF);
}

#[test]
fn channel_1_sweep_clock_can_update_the_shadow_period_while_inactive() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x11);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0x00);
    apu.write_register(0xFF14, 0x85);

    assert_eq!(apu.channels.channel_1.period_value(), 0x0500);
    assert!(apu.channels.channel_1.sweep.enabled);

    apu.channels.channel_1.pulse.runtime.active = false;
    apu.channels.channel_1.clock_sweep(ConsoleModel::GameBoy);

    assert_eq!(apu.channels.channel_1.period_value(), 0x0780);
    assert_eq!(apu.channels.channel_1.sweep.shadow_period, 0x0780);
    assert!(!apu.channels.channel_1.pulse.runtime.active);
}

#[test]
fn channel_1_shift_zero_sweep_does_not_calculate_on_trigger_but_can_overflow_on_sweep_clock() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x10);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0x00);
    apu.write_register(0xFF14, 0x86);

    assert_eq!(apu.channels.channel_1.period_value(), 0x0600);
    assert!(apu.channels.channel_1.pulse.runtime.active);

    apu.channels.channel_1.clock_sweep(ConsoleModel::GameBoy);

    assert_eq!(apu.channels.channel_1.period_value(), 0x0600);
    assert!(!apu.channels.channel_1.pulse.runtime.active);
}

#[test]
fn channel_1_zero_sweep_pace_reloads_to_eight_and_rearms_on_a_non_zero_write() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x11);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0x00);
    apu.write_register(0xFF14, 0x82);

    assert_eq!(apu.channels.channel_1.period_value(), 0x0200);
    apu.channels.channel_1.clock_sweep(ConsoleModel::GameBoy);
    assert_eq!(apu.channels.channel_1.period_value(), 0x0300);

    apu.write_register(0xFF10, 0x01);
    for _ in 0..8 {
        apu.channels.channel_1.clock_sweep(ConsoleModel::GameBoy);
        assert_eq!(apu.channels.channel_1.period_value(), 0x0300);
        assert!(apu.channels.channel_1.pulse.runtime.active);
    }

    assert_eq!(apu.channels.channel_1.sweep.shadow_period, 0x0300);
    assert_eq!(apu.channels.channel_1.sweep.timer, 1);

    apu.write_register(0xFF10, 0x11);
    apu.channels.channel_1.clock_sweep(ConsoleModel::GameBoy);

    assert_eq!(apu.channels.channel_1.period_value(), 0x0480);
    assert_eq!(apu.channels.channel_1.sweep.shadow_period, 0x0480);
    assert!(apu.channels.channel_1.pulse.runtime.active);
}

#[test]
fn clearing_negate_after_a_negate_calculation_disables_channel_1() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x09);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0x00);
    apu.write_register(0xFF14, 0x84);

    assert!(apu.channels.channel_1.pulse.runtime.active);
    assert!(apu.channels.channel_1.sweep.negate_calculated_since_trigger);

    apu.write_register(0xFF10, 0x10);

    assert!(!apu.channels.channel_1.pulse.runtime.active);
}

#[test]
fn clearing_negate_after_an_in_range_negate_calculation_still_disables_channel_1() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x19);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0x00);
    apu.write_register(0xFF14, 0x84);

    assert!(apu.channels.channel_1.pulse.runtime.active);
    assert!(apu.channels.channel_1.sweep.negate_calculated_since_trigger);
    assert_eq!(apu.channels.channel_1.period_value(), 0x0400);

    apu.write_register(0xFF10, 0x11);

    assert!(!apu.channels.channel_1.pulse.runtime.active);
    assert_eq!(apu.channels.channel_1.period_value(), 0x0400);
}

#[test]
fn clearing_negate_without_a_negate_calculation_keeps_channel_1_active() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x08);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0x00);
    apu.write_register(0xFF14, 0x84);

    assert!(apu.channels.channel_1.pulse.runtime.active);
    assert!(!apu.channels.channel_1.sweep.negate_calculated_since_trigger);

    apu.write_register(0xFF10, 0x10);

    assert!(apu.channels.channel_1.pulse.runtime.active);
}

#[test]
fn channel_1_negate_sweep_uses_eleven_bit_twos_complement_subtraction() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF10, 0x1C);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0xF0);
    apu.write_register(0xFF13, 0xB0);
    apu.write_register(0xFF14, 0x85);

    apu.channels.channel_1.clock_sweep(ConsoleModel::GameBoy);

    assert_eq!(apu.channels.channel_1.period_value(), 0x0555);
    assert_eq!(apu.channels.channel_1.sweep.shadow_period, 0x0555);
    assert!(apu.channels.channel_1.pulse.runtime.active);
}

#[test]
fn envelope_reaching_zero_does_not_disable_the_pulse_channel() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0x11);
    apu.write_register(0xFF19, 0x80);

    apu.frame_sequencer.apply_startup_phase(7);
    tick_apu_with_edges(&mut apu, 0, &[DerivedEdge::ApuFrameSequencerEdge]);

    assert_eq!(apu.channels.channel_2.pulse.envelope.current_volume, 0);
    assert!(apu.channels.channel_2.pulse.runtime.active);
}

#[test]
fn pulse_envelope_stops_automatic_updates_after_saturating_at_fifteen() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0xFA);
    apu.write_register(0xFF19, 0x80);

    apu.channels.channel_2.pulse.envelope.timer = 1;

    assert!(
        apu.channels
            .channel_2
            .pulse
            .envelope
            .automatic_updates_enabled
    );
    assert_eq!(apu.channels.channel_2.pulse.envelope.current_volume, 0x0F);

    apu.channels.channel_2.clock_envelope();

    assert_eq!(apu.channels.channel_2.pulse.envelope.current_volume, 0x0F);
    assert_eq!(apu.channels.channel_2.pulse.envelope.timer, 2);
    assert!(apu.channels.channel_2.pulse.runtime.active);
    assert!(
        !apu.channels
            .channel_2
            .pulse
            .envelope
            .automatic_updates_enabled
    );

    apu.channels.channel_2.clock_envelope();

    assert_eq!(apu.channels.channel_2.pulse.envelope.current_volume, 0x0F);
    assert_eq!(apu.channels.channel_2.pulse.envelope.timer, 2);
    assert!(apu.channels.channel_2.pulse.runtime.active);

    apu.write_register(0xFF19, 0x80);

    assert!(
        apu.channels
            .channel_2
            .pulse
            .envelope
            .automatic_updates_enabled
    );
    assert_eq!(apu.channels.channel_2.pulse.envelope.current_volume, 0x0F);
    assert_eq!(apu.channels.channel_2.pulse.envelope.timer, 2);
}

#[test]
fn pulse_fast_timer_advances_duty_step_while_the_channel_is_inactive_with_dac_enabled() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0xF0);
    apu.write_register(0xFF18, 0xFF);
    apu.write_register(0xFF19, 0x87);

    apu.channels.channel_2.pulse.runtime.active = false;
    apu.channels.channel_2.pulse.duty_step = 6;
    apu.channels.channel_2.pulse.period_timer = 1;

    apu.channels.channel_2.tick_fast_timer();

    assert_eq!(apu.channels.channel_2.pulse.duty_step, 7);
    assert_eq!(apu.channels.channel_2.pulse.period_timer, 4);
    assert!(!apu.channels.channel_2.pulse.runtime.active);
    assert_eq!(apu.channels.channel_2.pulse.current_digital_output(), 0);
}

#[test]
fn pulse_envelope_clock_advances_while_the_channel_is_inactive() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0x11);
    apu.write_register(0xFF19, 0x80);

    apu.channels.channel_2.pulse.runtime.active = false;
    apu.channels.channel_2.pulse.envelope.timer = 1;
    apu.channels.channel_2.pulse.envelope.current_volume = 1;

    apu.channels.channel_2.clock_envelope();

    assert_eq!(apu.channels.channel_2.pulse.envelope.timer, 1);
    assert_eq!(apu.channels.channel_2.pulse.envelope.current_volume, 0);
    assert!(!apu.channels.channel_2.pulse.runtime.active);
    assert_eq!(apu.channels.channel_2.pulse.current_digital_output(), 0);
}

#[test]
fn live_nrx2_write_with_increase_and_zero_pace_increments_active_pulse_channels() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0x08);
    apu.write_register(0xFF14, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0x08);
    apu.write_register(0xFF19, 0x80);

    assert!(apu.channels.channel_1.pulse.runtime.active);
    assert!(apu.channels.channel_2.pulse.runtime.active);
    assert_eq!(apu.channels.channel_1.pulse.envelope.current_volume, 0);
    assert_eq!(apu.channels.channel_2.pulse.envelope.current_volume, 0);

    apu.write_register(0xFF12, 0x08);
    apu.write_register(0xFF17, 0x08);

    assert_eq!(apu.channels.channel_1.pulse.envelope.current_volume, 1);
    assert_eq!(apu.channels.channel_2.pulse.envelope.current_volume, 1);

    apu.channels.channel_1.pulse.envelope.current_volume = 0x0F;
    apu.write_register(0xFF12, 0x08);
    assert_eq!(apu.channels.channel_1.pulse.envelope.current_volume, 0);

    apu.channels.channel_2.pulse.envelope.current_volume = 7;
    apu.write_register(0xFF17, 0x09);
    assert_eq!(apu.channels.channel_2.pulse.envelope.current_volume, 7);
}

fn cgb_channel_1_volume_after_live_nrx2_write(old_value: u8, new_value: u8) -> u8 {
    let mut apu = Apu::new(ConsoleModel::GameBoyColor);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, old_value);
    apu.write_register(0xFF14, 0x80);
    apu.write_register(0xFF12, new_value);
    apu.channels.channel_1.pulse.envelope.current_volume
}

fn cgb_channel_2_volume_after_live_nrx2_write(old_value: u8, new_value: u8) -> u8 {
    let mut apu = Apu::new(ConsoleModel::GameBoyColor);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, old_value);
    apu.write_register(0xFF19, 0x80);
    apu.write_register(0xFF17, new_value);
    apu.channels.channel_2.pulse.envelope.current_volume
}

#[test]
fn cgb_live_nrx2_writes_use_the_shared_zombie_volume_matrix_for_pulse_channels() {
    let cases = [
        (0x50, 0xF1, 0x04),
        (0x51, 0xF8, 0x09),
        (0x58, 0xF0, 0x0B),
        (0x58, 0xF1, 0x0A),
        (0x58, 0xF8, 0x06),
        (0x59, 0xF8, 0x05),
    ];

    for (old_value, new_value, expected_volume) in cases {
        assert_eq!(
            cgb_channel_1_volume_after_live_nrx2_write(old_value, new_value),
            expected_volume,
            "CH1 old={old_value:#04X} new={new_value:#04X}",
        );
        assert_eq!(
            cgb_channel_2_volume_after_live_nrx2_write(old_value, new_value),
            expected_volume,
            "CH2 old={old_value:#04X} new={new_value:#04X}",
        );
    }
}

#[test]
fn live_nrx2_write_requires_retrigger_before_reprogramming_pulse_envelopes() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);

    apu.write_register(0xFF11, 0x80);
    apu.write_register(0xFF12, 0x52);
    apu.write_register(0xFF14, 0x80);
    apu.channels.channel_1.pulse.envelope.timer = 1;

    apu.write_register(0xFF16, 0x80);
    apu.write_register(0xFF17, 0x52);
    apu.write_register(0xFF19, 0x80);
    apu.channels.channel_2.pulse.envelope.timer = 1;

    assert_eq!(apu.channels.channel_1.pulse.envelope.current_volume, 5);
    assert_eq!(apu.channels.channel_2.pulse.envelope.current_volume, 5);

    apu.write_register(0xFF12, 0x69);
    apu.write_register(0xFF17, 0x69);

    assert_eq!(apu.channels.channel_1.pulse.envelope.current_volume, 5);
    assert_eq!(apu.channels.channel_2.pulse.envelope.current_volume, 5);

    apu.channels.channel_1.clock_envelope();
    apu.channels.channel_2.clock_envelope();

    assert_eq!(apu.channels.channel_1.pulse.envelope.current_volume, 4);
    assert_eq!(apu.channels.channel_1.pulse.envelope.timer, 2);
    assert_eq!(apu.channels.channel_2.pulse.envelope.current_volume, 4);
    assert_eq!(apu.channels.channel_2.pulse.envelope.timer, 2);

    apu.write_register(0xFF14, 0x80);
    apu.write_register(0xFF19, 0x80);

    assert_eq!(apu.channels.channel_1.pulse.envelope.current_volume, 6);
    assert_eq!(apu.channels.channel_1.pulse.envelope.timer, 1);
    assert_eq!(apu.channels.channel_2.pulse.envelope.current_volume, 6);
    assert_eq!(apu.channels.channel_2.pulse.envelope.timer, 1);
}
