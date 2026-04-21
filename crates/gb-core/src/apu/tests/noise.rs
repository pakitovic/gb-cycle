use super::*;

#[test]
fn channel_4_trigger_with_zero_length_enabled_reloads_and_clocks_it() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    apu.write_register(0xFF21, 0x08);
    apu.write_register(0xFF20, pulse_length_load(1));
    apu.write_register(0xFF23, LENGTH_ENABLE_BIT);

    assert_eq!(apu.channels.channel_4.length_counter, 0);
    assert!(!apu.channels.channel_4.runtime.active);

    apu.write_register(0xFF23, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channels.channel_4.length_counter, 63);
    assert!(apu.channels.channel_4.runtime.active);
}

#[test]
fn channel_4_retrigger_after_unfreezing_zero_length_does_not_extra_clock_again() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    apu.write_register(0xFF21, 0x08);
    apu.write_register(0xFF20, pulse_length_load(1));
    apu.write_register(0xFF23, LENGTH_ENABLE_BIT);

    assert_eq!(apu.channels.channel_4.length_counter, 0);
    assert!(!apu.channels.channel_4.runtime.active);

    apu.write_register(0xFF23, 0x00);
    apu.write_register(0xFF23, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_4.length_counter, 63);
    assert!(apu.channels.channel_4.runtime.active);

    apu.write_register(0xFF23, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channels.channel_4.length_counter, 63);
    assert!(apu.channels.channel_4.runtime.active);
}

#[test]
fn channel_4_trigger_reloads_envelope_lfsr_and_noise_timer() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF21, 0xF2);
    apu.write_register(0xFF22, 0x15);
    apu.write_register(0xFF23, 0x80);

    assert!(apu.channels.channel_4.runtime.active);
    assert_eq!(apu.channels.channel_4.envelope.current_volume, 0x0F);
    assert_eq!(apu.channels.channel_4.envelope.timer, 2);
    assert_eq!(
        apu.channels.channel_4.noise.lfsr_state,
        NOISE_LFSR_INITIAL_STATE
    );
    assert_eq!(apu.channels.channel_4.current_digital_output(), 0x0F);
    assert_eq!(apu.channels.channel_4.noise.period_timer, 160);
}

#[test]
fn channel_4_trigger_reloads_state_but_does_not_activate_while_the_dac_is_off() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF21, 0x00);
    apu.write_register(0xFF22, 0x15);
    apu.channels.channel_4.noise.period_timer = 77;
    apu.channels.channel_4.envelope.timer = 4;
    apu.channels.channel_4.envelope.current_volume = 6;
    apu.channels.channel_4.noise.lfsr_state = 0x0123;

    apu.write_register(0xFF23, 0x80);

    assert!(!apu.channels.channel_4.runtime.active);
    assert_eq!(apu.channels.channel_4.noise.period_timer, 160);
    assert_eq!(
        apu.channels.channel_4.envelope.timer,
        envelope_timer_reload(0)
    );
    assert_eq!(apu.channels.channel_4.envelope.current_volume, 0);
    assert_eq!(
        apu.channels.channel_4.noise.lfsr_state,
        NOISE_LFSR_INITIAL_STATE
    );
}

#[test]
fn channel_4_noise_timer_steps_the_lfsr_and_short_width_mode_copies_feedback_into_bit_six() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.runtime.active = true;
    channel.noise.short_width_mode = true;
    channel.noise.clock_shift = 0;
    channel.noise.clock_divider_code = 0;
    channel.noise.period_timer = 1;
    channel.envelope.current_volume = 0x0F;
    channel.noise.lfsr_state = NOISE_LFSR_INITIAL_STATE;

    channel.tick_fast_timer();

    assert_eq!(channel.noise.period_timer, 8);
    assert_eq!(channel.noise.lfsr_state, 0x4040);
    assert_eq!(channel.current_digital_output(), 0x0F);
}

#[test]
fn channel_4_noise_timer_keeps_running_while_the_channel_is_inactive() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.runtime.active = false;
    channel.noise.short_width_mode = true;
    channel.noise.clock_shift = 0;
    channel.noise.clock_divider_code = 0;
    channel.noise.period_timer = 1;
    channel.envelope.current_volume = 0x0F;
    channel.noise.lfsr_state = NOISE_LFSR_INITIAL_STATE;

    channel.tick_fast_timer();

    assert_eq!(channel.noise.period_timer, 8);
    assert_eq!(channel.noise.lfsr_state, 0x4040);
    assert!(!channel.runtime.active);
    assert_eq!(channel.current_digital_output(), 0);
}

#[test]
fn channel_4_live_nr43_write_into_shift_14_reloads_the_suppressed_noise_timer() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.runtime.active = true;
    channel.envelope.current_volume = 0x0F;
    channel.noise.lfsr_state = 0x1234;
    channel.write_nr43(0x00);
    channel.noise.period_timer = 1;

    channel.write_nr43(0xE0);

    assert_eq!(channel.noise.period_timer, noise_timer_reload(14, 0));

    let lfsr_before = channel.noise.lfsr_state;
    channel.tick_fast_timer();

    assert_eq!(channel.noise.period_timer, noise_timer_reload(14, 0));
    assert_eq!(channel.noise.lfsr_state, lfsr_before);
    assert_eq!(channel.current_digital_output(), 0x0F);
}

#[test]
fn channel_4_live_nr43_write_out_of_shift_14_reloads_from_the_new_clocked_timer_base() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.runtime.active = true;
    channel.envelope.current_volume = 0x0F;
    channel.noise.lfsr_state = 0x1234;
    channel.write_nr43(0xE0);

    assert_eq!(channel.noise.period_timer, noise_timer_reload(14, 0));

    channel.write_nr43(0x00);

    assert_eq!(channel.noise.period_timer, noise_timer_reload(0, 0));
    let lfsr_before = channel.noise.lfsr_state;

    for expected_timer in (1..noise_timer_reload(0, 0)).rev() {
        channel.tick_fast_timer();
        assert_eq!(channel.noise.period_timer, expected_timer);
        assert_eq!(channel.noise.lfsr_state, lfsr_before);
    }

    channel.tick_fast_timer();

    assert_eq!(channel.noise.period_timer, noise_timer_reload(0, 0));
    assert_ne!(channel.noise.lfsr_state, lfsr_before);
}

#[test]
fn channel_4_live_nr43_write_between_clocked_rates_reloads_the_new_timer_without_stepping_lfsr() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.runtime.active = true;
    channel.envelope.current_volume = 0x0F;
    channel.noise.lfsr_state = 0x1234;
    channel.write_nr43(0x15);

    assert_eq!(channel.noise.period_timer, noise_timer_reload(1, 5));

    channel.noise.period_timer = 1;
    channel.nr43_live_write.noise_counter = 0;
    channel.write_nr43(0x00);

    assert_eq!(channel.noise.period_timer, noise_timer_reload(0, 0));
    assert_eq!(channel.noise.lfsr_state, 0x1234);
    assert_eq!(channel.current_digital_output(), 0x0F);

    for expected_timer in (1..noise_timer_reload(0, 0)).rev() {
        channel.tick_fast_timer();
        assert_eq!(channel.noise.period_timer, expected_timer);
        assert_eq!(channel.noise.lfsr_state, 0x1234);
    }

    channel.tick_fast_timer();

    assert_eq!(channel.noise.period_timer, noise_timer_reload(0, 0));
    assert_ne!(channel.noise.lfsr_state, 0x1234);
}

#[test]
fn channel_4_live_nr43_write_steps_the_lfsr_when_the_new_selected_counter_bit_is_high() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.runtime.active = true;
    channel.envelope.current_volume = 0x0F;
    channel.noise.lfsr_state = NOISE_LFSR_INITIAL_STATE;
    channel.write_nr43(0x00);
    channel.nr43_live_write.noise_counter = 0x0002;

    channel.write_nr43(0x10);

    assert_eq!(channel.noise.lfsr_state, 0x4040);
    assert_eq!(channel.noise.period_timer, noise_timer_reload(1, 0));
    let trace = channel
        .nr43_live_write
        .last_trace
        .expect("trace should record the live NR43 write");
    assert_eq!(
        trace.decision_category,
        ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort
    );
    assert_eq!(trace.lfsr_action, ApuCh4Nr43LfsrAction::ForcedShortStep);
    assert_eq!(trace.glitch_value, 0x00);
    assert_eq!(trace.second_glitch_value, 0x10);
    assert_eq!(trace.glitch_shift, 0);
    assert_eq!(trace.second_glitch_shift, 1);
    assert!(!trace.old_bit);
    assert!(!trace.glitch_bit);
    assert!(trace.second_glitch_bit);
    assert!(trace.new_bit);
    assert!(trace.runtime_active);
    assert!(trace.ff_to_new_step);
    assert!(!trace.old_to_ff_step);
    assert!(!trace.old_to_ff_forced_short_width);
    assert!(trace.ff_to_new_forced_short_width);
    assert!(!trace.reload_seam_step);
}

#[test]
fn channel_4_single_intermediate_category_2_is_a_plain_step_in_the_current_subset() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.runtime.active = true;
    channel.envelope.current_volume = 0x0F;
    channel.noise.lfsr_state = 0x642E;
    channel.write_nr43(0x00);
    channel.nr43_live_write.noise_counter = 0x0100;
    channel.nr43_live_write.countdown_reloaded = false;

    channel.write_nr43(0x90);

    assert_eq!(channel.noise.lfsr_state, 0x3217);
    let trace = channel
        .nr43_live_write
        .last_trace
        .expect("trace should record the category-2 write");
    assert_eq!(
        trace.decision_category,
        ApuCh4Nr43LiveWriteCategory::Category2
    );
    assert_eq!(trace.lfsr_action, ApuCh4Nr43LfsrAction::PlainStep);
    assert_eq!(trace.glitch_value, 0x80);
    assert_eq!(trace.second_glitch_value, 0x80);
    assert_eq!(trace.glitch_shift, 8);
    assert_eq!(trace.second_glitch_shift, 8);
    assert!(!trace.old_bit);
    assert!(trace.glitch_bit);
    assert!(trace.second_glitch_bit);
    assert!(!trace.new_bit);
    assert!(trace.ff_to_new_step);
    assert!(!trace.ff_to_new_forced_short_width);
}

#[test]
fn channel_4_single_intermediate_category_1_is_currently_inert_in_the_dmg_subset() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.runtime.active = true;
    channel.envelope.current_volume = 0x0F;
    channel.noise.lfsr_state = 0x642E;
    channel.write_nr43(0x00);
    channel.nr43_live_write.noise_counter = 0x0201;
    channel.nr43_live_write.countdown_reloaded = false;

    channel.write_nr43(0x90);

    assert_eq!(channel.noise.lfsr_state, 0x642E);
    let trace = channel
        .nr43_live_write
        .last_trace
        .expect("trace should record the category-1 write");
    assert_eq!(
        trace.decision_category,
        ApuCh4Nr43LiveWriteCategory::Category1
    );
    assert_eq!(trace.lfsr_action, ApuCh4Nr43LfsrAction::None);
    assert_eq!(trace.glitch_value, 0x80);
    assert_eq!(trace.second_glitch_value, 0x80);
    assert_eq!(trace.glitch_shift, 8);
    assert_eq!(trace.second_glitch_shift, 8);
    assert!(trace.old_bit);
    assert!(!trace.glitch_bit);
    assert!(!trace.second_glitch_bit);
    assert!(trace.new_bit);
    assert!(!trace.ff_to_new_step);
    assert!(!trace.ff_to_new_forced_short_width);
}

#[test]
fn channel_4_live_nr43_write_does_not_step_when_the_new_selected_counter_bit_stays_low() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.runtime.active = true;
    channel.envelope.current_volume = 0x0F;
    channel.noise.lfsr_state = 0x1234;
    channel.write_nr43(0x00);
    channel.nr43_live_write.noise_counter = 0x0001;

    channel.write_nr43(0x10);

    assert_eq!(channel.noise.lfsr_state, 0x1234);
    assert_eq!(channel.noise.period_timer, noise_timer_reload(1, 0));
    let trace = channel
        .nr43_live_write
        .last_trace
        .expect("trace should record the live NR43 write");
    assert_eq!(trace.decision_category, ApuCh4Nr43LiveWriteCategory::None);
    assert_eq!(trace.lfsr_action, ApuCh4Nr43LfsrAction::None);
    assert_eq!(trace.glitch_value, 0x00);
    assert_eq!(trace.second_glitch_value, 0x10);
    assert_eq!(trace.glitch_shift, 0);
    assert_eq!(trace.second_glitch_shift, 1);
    assert!(trace.old_bit);
    assert!(trace.glitch_bit);
    assert!(!trace.second_glitch_bit);
    assert!(!trace.new_bit);
    assert!(!trace.reload_seam_step);
    assert!(!trace.old_to_ff_step);
    assert!(!trace.ff_to_new_step);
}

#[test]
fn channel_4_live_nr43_write_can_take_the_reload_seam_without_forcing_an_extra_step() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.runtime.active = true;
    channel.envelope.current_volume = 0x0F;
    channel.noise.lfsr_state = NOISE_LFSR_INITIAL_STATE;
    channel.write_nr43(0x40);
    channel.nr43_live_write.alignment = 2;
    channel.nr43_live_write.noise_counter = 0x01A0;
    channel.nr43_live_write.countdown_reloaded = true;

    channel.write_nr43(0x50);

    assert_eq!(channel.noise.lfsr_state, 0x4000);
    assert_eq!(channel.noise.period_timer, noise_timer_reload(5, 0));
    assert_eq!(channel.nr43_live_write.counter_timer, 2);
    assert!(!channel.nr43_live_write.countdown_reloaded);
    let trace = channel
        .nr43_live_write
        .last_trace
        .expect("trace should record the seam write");
    assert!(trace.reload_seam_step);
    assert!(!trace.ff_to_new_step);
    assert!(!trace.feedback_corruption);
}

#[test]
fn channel_4_live_nr43_write_reloads_the_hidden_counter_timer_with_alignment_after_a_reload_seam() {
    let mut channel = Channel4State::default();
    channel.write_nr43(0x00);
    channel.nr43_live_write.alignment = 3;
    channel.nr43_live_write.countdown_reloaded = true;

    channel.write_nr43(0x01);

    assert_eq!(channel.nr43_live_write.counter_timer, 7);
    assert!(!channel.nr43_live_write.countdown_reloaded);
}

#[test]
fn channel_4_live_nr43_write_preserves_the_hidden_counter_timer_outside_the_reload_seam() {
    let mut channel = Channel4State::default();
    channel.write_nr43(0x4C);
    channel.nr43_live_write.counter_timer = 11;
    channel.nr43_live_write.alignment = 2;
    channel.nr43_live_write.countdown_reloaded = false;

    channel.write_nr43(0x5C);

    assert_eq!(channel.nr43_live_write.counter_timer, 11);
    assert!(!channel.nr43_live_write.countdown_reloaded);
}

#[test]
fn channel_4_live_nr43_write_can_step_twice_with_feedback_corruption_in_low_shift_cases() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.runtime.active = true;
    channel.envelope.current_volume = 0x0F;
    channel.noise.lfsr_state = 0x642E;
    channel.write_nr43(0x80);
    channel.nr43_live_write.noise_counter = 0x0003;
    channel.nr43_live_write.countdown_reloaded = false;

    channel.write_nr43(0x10);

    assert_eq!(channel.noise.lfsr_state, 0x190B);
    assert_eq!(channel.noise.period_timer, noise_timer_reload(1, 0));
    let trace = channel
        .nr43_live_write
        .last_trace
        .expect("trace should record the low-shift write");
    assert_eq!(
        trace.decision_category,
        ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort
    );
    assert_eq!(
        trace.lfsr_action,
        ApuCh4Nr43LfsrAction::ForcedShortStepThenLowShiftCorruption
    );
    assert_eq!(trace.glitch_value, 0x00);
    assert_eq!(trace.second_glitch_value, 0x00);
    assert_eq!(trace.glitch_shift, 0);
    assert_eq!(trace.second_glitch_shift, 0);
    assert!(!trace.old_bit);
    assert!(trace.glitch_bit);
    assert!(trace.second_glitch_bit);
    assert!(trace.new_bit);
    assert!(!trace.old_to_ff_step);
    assert!(!trace.old_to_ff_forced_short_width);
    assert!(trace.ff_to_new_step);
    assert!(trace.ff_to_new_forced_short_width);
    assert!(trace.low_shift_extra_step);
    assert!(trace.feedback_corruption);
}

#[test]
fn channel_4_high_shift_narrow_staircase_uses_the_sameboy_style_single_intermediate_bits() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.runtime.active = true;
    channel.envelope.current_volume = 0x0F;

    channel.noise.lfsr_state = 0x6189;
    channel.write_nr43(0x03);
    channel.nr43_live_write.noise_counter = 0x3CEA;
    channel.nr43_live_write.countdown_reloaded = false;
    channel.write_nr43(0x2C);
    assert_eq!(channel.noise.lfsr_state, 0x3084);
    let trace = channel
        .nr43_live_write
        .last_trace
        .expect("trace should record the 03 -> 2C write");
    assert!(trace.ff_to_new_step);
    assert!(!trace.ff_to_new_forced_short_width);

    channel.noise.lfsr_state = 0x3F3F;
    channel.write_nr43(0x4C);
    channel.nr43_live_write.noise_counter = 0x19BC;
    channel.nr43_live_write.countdown_reloaded = false;
    channel.write_nr43(0x5C);
    assert_eq!(channel.noise.lfsr_state, 0x3F3F);
    let trace = channel
        .nr43_live_write
        .last_trace
        .expect("trace should record the 4C -> 5C write");
    assert!(!trace.old_to_ff_step);
    assert!(!trace.ff_to_new_step);

    channel.noise.lfsr_state = 0x3F3F;
    channel.write_nr43(0x5C);
    channel.nr43_live_write.noise_counter = 0x224E;
    channel.nr43_live_write.countdown_reloaded = false;
    channel.write_nr43(0x6C);
    assert_eq!(channel.noise.lfsr_state, 0x3F3F);
    let trace = channel
        .nr43_live_write
        .last_trace
        .expect("trace should record the 5C -> 6C write");
    assert_eq!(trace.glitch_value, 0x5C);
    assert_eq!(trace.second_glitch_value, 0x6C);
    assert!(!trace.glitch_bit);
    assert!(trace.second_glitch_bit);
    assert!(!trace.ff_to_new_step);
    assert_eq!(trace.lfsr_action, ApuCh4Nr43LfsrAction::None);

    channel.noise.lfsr_state = 0x3ABA;
    channel.write_nr43(0x6C);
    channel.nr43_live_write.noise_counter = 0x2AE1;
    channel.nr43_live_write.countdown_reloaded = true;
    channel.write_nr43(0x7C);
    assert_eq!(channel.noise.lfsr_state, 0x3ABA);
    let trace = channel
        .nr43_live_write
        .last_trace
        .expect("trace should record the 6C -> 7C write");
    assert!(!trace.reload_seam_step);
    assert!(!trace.ff_to_new_step);

    channel.noise.lfsr_state = 0x7676;
    channel.write_nr43(0x7C);
    channel.nr43_live_write.noise_counter = 0x3374;
    channel.nr43_live_write.countdown_reloaded = false;
    channel.write_nr43(0x6C);
    assert_eq!(channel.noise.lfsr_state, 0x3B3B);
    let trace = channel
        .nr43_live_write
        .last_trace
        .expect("trace should record the 7C -> 6C write");
    assert_eq!(trace.glitch_value, 0x7C);
    assert_eq!(trace.second_glitch_value, 0x6C);
    assert!(!trace.glitch_bit);
    assert!(trace.second_glitch_bit);
    assert!(trace.ff_to_new_step);
    assert!(trace.ff_to_new_forced_short_width);
    assert_eq!(trace.lfsr_action, ApuCh4Nr43LfsrAction::ForcedShortStep);

    channel.noise.lfsr_state = 0x0909;
    channel.write_nr43(0x5C);
    channel.nr43_live_write.noise_counter = 0x0499;
    channel.nr43_live_write.countdown_reloaded = false;
    channel.write_nr43(0x4C);
    assert_eq!(channel.noise.lfsr_state, 0x0909);
    let trace = channel
        .nr43_live_write
        .last_trace
        .expect("trace should record the 5C -> 4C write");
    assert_eq!(trace.glitch_value, 0x5C);
    assert_eq!(trace.second_glitch_value, 0x4C);
    assert!(!trace.glitch_bit);
    assert!(trace.second_glitch_bit);
    assert!(!trace.ff_to_new_step);

    channel.noise.lfsr_state = 0x0707;
    channel.write_nr43(0x4C);
    channel.nr43_live_write.noise_counter = 0x0D2B;
    channel.nr43_live_write.countdown_reloaded = false;
    channel.write_nr43(0x3C);
    assert_eq!(channel.noise.lfsr_state, 0x0707);
    let trace = channel
        .nr43_live_write
        .last_trace
        .expect("trace should record the 4C -> 3C write");
    assert_eq!(trace.glitch_value, 0x4C);
    assert_eq!(trace.second_glitch_value, 0x7C);
    assert!(!trace.glitch_bit);
    assert!(!trace.second_glitch_bit);
    assert!(!trace.ff_to_new_step);
    assert!(!trace.ff_to_new_forced_short_width);
    assert_eq!(trace.lfsr_action, ApuCh4Nr43LfsrAction::None);

    channel.noise.lfsr_state = 0x4BCB;
    channel.write_nr43(0x3C);
    channel.nr43_live_write.noise_counter = 0x15BE;
    channel.nr43_live_write.countdown_reloaded = false;
    channel.write_nr43(0x09);
    assert_eq!(channel.noise.lfsr_state, 0x4BCB);
    let trace = channel
        .nr43_live_write
        .last_trace
        .expect("trace should record the 3C -> 09 write");
    assert!(!trace.ff_to_new_step);
}

#[test]
fn channel_4_envelope_reaching_zero_does_not_disable_the_channel() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF21, 0x12);
    apu.write_register(0xFF22, 0x00);
    apu.write_register(0xFF23, 0x80);
    apu.channels.channel_4.envelope.timer = 1;

    assert!(apu.channels.channel_4.runtime.active);
    assert_eq!(apu.channels.channel_4.envelope.current_volume, 1);
    assert!(apu.channels.channel_4.envelope.automatic_updates_enabled);

    apu.channels.channel_4.clock_envelope();

    assert_eq!(apu.channels.channel_4.envelope.current_volume, 0);
    assert_eq!(apu.channels.channel_4.envelope.timer, 2);
    assert!(apu.channels.channel_4.runtime.active);
    assert!(apu.channels.channel_4.envelope.automatic_updates_enabled);

    apu.channels.channel_4.clock_envelope();

    assert_eq!(apu.channels.channel_4.envelope.current_volume, 0);
    assert_eq!(apu.channels.channel_4.envelope.timer, 1);
    assert!(apu.channels.channel_4.runtime.active);
    assert!(apu.channels.channel_4.envelope.automatic_updates_enabled);

    apu.channels.channel_4.clock_envelope();

    assert_eq!(apu.channels.channel_4.envelope.current_volume, 0);
    assert_eq!(apu.channels.channel_4.envelope.timer, 2);
    assert!(apu.channels.channel_4.runtime.active);
    assert!(!apu.channels.channel_4.envelope.automatic_updates_enabled);

    apu.channels.channel_4.clock_envelope();

    assert_eq!(apu.channels.channel_4.envelope.current_volume, 0);
    assert_eq!(apu.channels.channel_4.envelope.timer, 2);
    assert!(apu.channels.channel_4.runtime.active);

    apu.write_register(0xFF23, 0x80);

    assert!(apu.channels.channel_4.envelope.automatic_updates_enabled);
    assert_eq!(apu.channels.channel_4.envelope.current_volume, 1);
    assert_eq!(apu.channels.channel_4.envelope.timer, 2);
}

#[test]
fn channel_4_envelope_clock_advances_while_the_channel_is_inactive() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF21, 0x11);
    apu.write_register(0xFF23, 0x80);

    apu.channels.channel_4.runtime.active = false;
    apu.channels.channel_4.envelope.timer = 1;
    apu.channels.channel_4.envelope.current_volume = 1;

    apu.channels.channel_4.clock_envelope();

    assert_eq!(apu.channels.channel_4.envelope.timer, 1);
    assert_eq!(apu.channels.channel_4.envelope.current_volume, 0);
    assert!(!apu.channels.channel_4.runtime.active);
    assert_eq!(apu.channels.channel_4.current_digital_output(), 0);
}

#[test]
fn live_nr42_write_with_increase_and_zero_pace_increments_active_noise_channel() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF21, 0x08);
    apu.write_register(0xFF23, 0x80);

    assert!(apu.channels.channel_4.runtime.active);
    assert_eq!(apu.channels.channel_4.envelope.current_volume, 0);

    apu.write_register(0xFF21, 0x08);
    assert_eq!(apu.channels.channel_4.envelope.current_volume, 1);

    apu.channels.channel_4.envelope.current_volume = 0x0F;
    apu.write_register(0xFF21, 0x08);
    assert_eq!(apu.channels.channel_4.envelope.current_volume, 0);
}

#[test]
fn live_nr42_write_requires_retrigger_before_reprogramming_the_noise_envelope() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF21, 0x52);
    apu.write_register(0xFF23, 0x80);
    apu.channels.channel_4.envelope.timer = 1;

    assert_eq!(apu.channels.channel_4.envelope.current_volume, 5);

    apu.write_register(0xFF21, 0x69);

    assert_eq!(apu.channels.channel_4.envelope.current_volume, 5);

    apu.channels.channel_4.clock_envelope();

    assert_eq!(apu.channels.channel_4.envelope.current_volume, 4);
    assert_eq!(apu.channels.channel_4.envelope.timer, 2);

    apu.write_register(0xFF23, 0x80);

    assert_eq!(apu.channels.channel_4.envelope.current_volume, 6);
    assert_eq!(apu.channels.channel_4.envelope.timer, 1);
}

#[test]
fn channel_4_live_15_bit_to_7_bit_switch_can_lock_the_active_lfsr_window_silently() {
    let mut wide = Channel4State::default();
    wide.runtime.dac_enabled = true;
    wide.runtime.active = true;
    wide.write_nr43(0x00);
    wide.noise.period_timer = 1;
    wide.envelope.current_volume = 0x0F;
    wide.noise.lfsr_state = 0x007F;

    let mut narrow = wide.clone();
    narrow.write_nr43(0x08);

    wide.tick_fast_timer();
    narrow.tick_fast_timer();

    assert_eq!(wide.noise.lfsr_state & 0x7F, 0x3F);
    assert_eq!(narrow.noise.lfsr_state & 0x7F, 0x7F);
    assert_eq!(narrow.current_digital_output(), 0);
    assert!(narrow.runtime.active);

    narrow.noise.period_timer = 1;
    narrow.tick_fast_timer();

    assert_eq!(narrow.noise.lfsr_state & 0x7F, 0x7F);
    assert_eq!(narrow.current_digital_output(), 0);
    assert!(narrow.runtime.active);
}

#[test]
fn channel_4_retrigger_recovers_from_short_width_lockup_without_clearing_activity() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF21, 0xF0);
    apu.write_register(0xFF22, 0x08);

    apu.channels.channel_4.runtime.active = true;
    apu.channels.channel_4.noise.lfsr_state = 0x007F;
    apu.channels.channel_4.envelope.current_volume = 0x0F;

    assert_eq!(apu.read_register(0xFF26) & 0x08, 0x08);
    assert_eq!(apu.channels.channel_4.current_digital_output(), 0);

    apu.write_register(0xFF23, 0x80);

    assert_eq!(
        apu.channels.channel_4.noise.lfsr_state,
        NOISE_LFSR_INITIAL_STATE
    );
    assert!(apu.channels.channel_4.runtime.active);
    assert_eq!(apu.read_register(0xFF26) & 0x08, 0x08);
    assert_eq!(apu.channels.channel_4.current_digital_output(), 0x0F);

    apu.channels.channel_4.noise.period_timer = 1;
    apu.channels.channel_4.tick_fast_timer();

    assert_ne!(apu.channels.channel_4.noise.lfsr_state & 0x7F, 0x7F);
}
