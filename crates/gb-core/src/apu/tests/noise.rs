use super::*;
use crate::apu::registers::Channel4Register;

#[test]
fn channel_4_trigger_with_zero_length_enabled_reloads_and_clocks_it() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
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
    let mut apu = Apu::new(ConsoleModel::GameBoy);
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
fn channel_4_delayed_trigger_with_zero_length_enabled_reloads_and_clocks_it() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    apu.write_register(0xFF21, 0x08);
    apu.write_register(0xFF20, pulse_length_load(1));
    apu.write_register(0xFF23, LENGTH_ENABLE_BIT);
    apu.channels.channel_4.nr43_live_write.alignment = 1;

    assert_eq!(apu.channels.channel_4.length_counter, 0);
    assert!(!apu.channels.channel_4.runtime.active);

    apu.write_register(0xFF23, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channels.channel_4.length_counter, 63);
    assert!(!apu.channels.channel_4.runtime.active);
    assert_eq!(apu.channels.channel_4.debug_snapshot().dmg_delayed_start, 6);
}

#[test]
fn channel_4_delayed_retrigger_after_unfreezing_zero_length_does_not_extra_clock_again() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.frame_sequencer.apply_startup_phase(7);
    apu.write_register(0xFF21, 0x08);
    apu.write_register(0xFF20, pulse_length_load(1));
    apu.write_register(0xFF23, LENGTH_ENABLE_BIT);
    apu.channels.channel_4.nr43_live_write.alignment = 1;

    apu.write_register(0xFF23, 0x00);
    apu.write_register(0xFF23, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);
    assert_eq!(apu.channels.channel_4.length_counter, 63);
    assert!(!apu.channels.channel_4.runtime.active);

    apu.write_register(0xFF23, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channels.channel_4.length_counter, 63);
    assert!(!apu.channels.channel_4.runtime.active);
}

#[test]
fn channel_4_delayed_trigger_still_fires_when_noise_clocking_is_suppressed() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF21, 0xF2);
    apu.write_register(0xFF22, 0xE0);
    apu.channels.channel_4.nr43_live_write.alignment = 1;

    apu.write_register(0xFF23, CHANNEL_TRIGGER_BIT);

    assert_eq!(apu.channels.channel_4.debug_snapshot().dmg_delayed_start, 6);
    assert!(!apu.channels.channel_4.runtime.active);

    for t_cycle in 0..12 {
        tick_apu_with_edges(&mut apu, t_cycle, &[]);
    }

    let snapshot = apu.channels.channel_4.debug_snapshot();
    assert_eq!(snapshot.dmg_delayed_start, 0);
    assert!(snapshot.runtime_active);
    assert_eq!(snapshot.period_timer, noise_timer_reload(0x0E, 0));
}

#[test]
fn channel_4_trigger_reloads_envelope_lfsr_and_noise_timer() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
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
    assert_eq!(apu.channels.channel_4.current_digital_output(), 0);
    assert_eq!(apu.channels.channel_4.noise.period_timer, 160);
}

#[test]
fn channel_4_trigger_reloads_state_but_does_not_activate_while_the_dac_is_off() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
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
fn channel_4_hidden_counter_stays_idle_until_the_channel_has_started() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.write_nr43(0x4C);
    let counter_timer_after_write = channel.nr43_live_write.counter_timer;

    for _ in 0..16 {
        channel.tick_fast_timer();
    }

    assert_eq!(channel.nr43_live_write.noise_counter, 0);
    assert_eq!(
        channel.nr43_live_write.counter_timer,
        counter_timer_after_write
    );
    assert!(!channel.nr43_live_write.background_counting);
    assert!(!channel.nr43_live_write.counter_active);
}

#[test]
fn channel_4_trigger_uses_the_sameboy_guided_hidden_counter_start_state() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.nr43_live_write.alignment = 0;
    channel.nr43_live_write.noise_counter = 0x1234;
    channel.nr43_live_write.counter_timer = 0;
    channel.write_register(Channel4Register::Nr42, 0xF0, ConsoleModel::GameBoy, 0);
    channel.write_register(Channel4Register::Nr43, 0x4C, ConsoleModel::GameBoy, 0);
    channel.write_register(Channel4Register::Nr44, 0x80, ConsoleModel::GameBoy, 0);

    assert_eq!(channel.nr43_live_write.alignment, 0);
    assert_eq!(channel.nr43_live_write.noise_counter, 0x1234);
    assert!(channel.runtime.active);
    assert!(channel.nr43_live_write.counter_timer > 0);
    assert!(!channel.nr43_live_write.countdown_reloaded);
    assert!(channel.nr43_live_write.counter_active);
    assert!(channel.nr43_live_write.background_counting);
}

#[test]
fn channel_4_hidden_counter_tick_steps_the_lfsr_and_short_width_mode_copies_feedback_into_bit_six()
{
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.runtime.active = true;
    channel.nr43_live_write.counter_active = true;
    channel.nr43_live_write.background_counting = true;
    channel.nr43_live_write.counter_timer = 1;
    channel.nr43_live_write.noise_counter = 0;
    channel.noise.short_width_mode = true;
    channel.noise.clock_shift = 0;
    channel.noise.clock_divider_code = 0;
    channel.noise.period_timer = 1;
    channel.envelope.current_volume = 0x0F;
    channel.noise.lfsr_state = NOISE_LFSR_INITIAL_STATE;

    channel.tick_fast_timer();

    assert_eq!(channel.noise.period_timer, 8);
    assert_eq!(channel.noise.lfsr_state, 0x4040);
    assert_eq!(channel.current_digital_output(), 0);
}

#[test]
fn channel_4_hidden_counter_does_not_step_the_lfsr_while_the_channel_is_inactive() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.runtime.active = false;
    channel.nr43_live_write.background_counting = true;
    channel.nr43_live_write.counter_timer = 1;
    channel.nr43_live_write.noise_counter = 0;
    channel.noise.short_width_mode = true;
    channel.noise.clock_shift = 0;
    channel.noise.clock_divider_code = 0;
    channel.noise.period_timer = 1;
    channel.envelope.current_volume = 0x0F;
    channel.noise.lfsr_state = NOISE_LFSR_INITIAL_STATE;

    channel.tick_fast_timer();

    assert_eq!(channel.noise.period_timer, 8);
    assert_eq!(channel.noise.lfsr_state, NOISE_LFSR_INITIAL_STATE);
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
    assert_eq!(channel.current_digital_output(), 0);
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
    assert_eq!(channel.noise.lfsr_state, lfsr_before);
}

#[test]
fn channel_4_live_nr43_write_between_clocked_rates_can_use_the_explicit_old_to_ff_pass() {
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
    let trace = last_channel_4_nr43_trace(&channel);
    assert_eq!(
        require_nr43_pass(trace.old_to_ff, "old_to_ff").category,
        ApuCh4Nr43LiveWriteCategory::None
    );
    assert_eq!(
        require_nr43_pass(trace.old_to_ff, "old_to_ff").action,
        ApuCh4Nr43LfsrAction::None
    );
    assert_eq!(
        require_nr43_pass(trace.ff_to_glitch_1, "ff_to_glitch_1").action,
        ApuCh4Nr43LfsrAction::None
    );
    assert!(trace.low_shift_followup.is_none());
    let stepped_lfsr = channel.noise.lfsr_state;
    assert_eq!(channel.current_digital_output(), 0);

    for expected_timer in (1..noise_timer_reload(0, 0)).rev() {
        channel.tick_fast_timer();
        assert_eq!(channel.noise.period_timer, expected_timer);
        assert_eq!(channel.noise.lfsr_state, stepped_lfsr);
    }

    channel.tick_fast_timer();

    assert_eq!(channel.noise.period_timer, noise_timer_reload(0, 0));
    assert_eq!(channel.noise.lfsr_state, stepped_lfsr);
}

fn last_channel_4_nr43_trace(channel: &Channel4State) -> ApuCh4Nr43LiveWriteTrace {
    channel
        .nr43_live_write
        .last_trace
        .expect("trace should record the live NR43 write")
}

fn require_nr43_pass(trace: Option<ApuCh4Nr43PassTrace>, label: &str) -> ApuCh4Nr43PassTrace {
    trace.unwrap_or_else(|| panic!("{label} pass should be present"))
}

#[test]
fn channel_4_live_nr43_write_records_explicit_pre_cgb_d_passes() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.runtime.active = true;
    channel.envelope.current_volume = 0x0F;
    channel.noise.lfsr_state = NOISE_LFSR_INITIAL_STATE;
    channel.write_nr43(0x00);
    channel.nr43_live_write.noise_counter = 0x0002;

    channel.write_nr43(0x10);

    assert_eq!(channel.noise.period_timer, noise_timer_reload(1, 0));
    let trace = last_channel_4_nr43_trace(&channel);
    assert_eq!(trace.ff_value, 0xFF);
    assert_eq!(trace.glitch_1_value, 0x7F);
    assert_eq!(trace.glitch_2_value, None);
    assert_eq!(trace.old_shift, 0);
    assert_eq!(trace.ff_shift, 15);
    assert_eq!(trace.glitch_1_shift, 7);
    assert_eq!(trace.glitch_2_shift, None);
    assert!(!trace.old_bit);
    assert!(!trace.ff_bit);
    assert!(!trace.glitch_1_bit);
    assert_eq!(trace.glitch_2_bit, None);
    assert!(trace.new_bit);

    let old_to_ff = require_nr43_pass(trace.old_to_ff, "old_to_ff");
    assert_eq!(old_to_ff.kind, ApuCh4Nr43PassKind::OldToFf);
    assert_eq!(old_to_ff.category, ApuCh4Nr43LiveWriteCategory::None);
    assert_eq!(old_to_ff.action, ApuCh4Nr43LfsrAction::None);

    let ff_to_glitch_1 = require_nr43_pass(trace.ff_to_glitch_1, "ff_to_glitch_1");
    assert_eq!(ff_to_glitch_1.kind, ApuCh4Nr43PassKind::FfToGlitch1);
    assert_eq!(
        ff_to_glitch_1.category,
        ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort
    );
    assert_eq!(ff_to_glitch_1.action, ApuCh4Nr43LfsrAction::ForcedShortStep);

    assert!(trace.glitch_1_to_glitch_2.is_none());

    let glitch_to_new = require_nr43_pass(trace.glitch_to_new, "glitch_to_new");
    assert_eq!(glitch_to_new.kind, ApuCh4Nr43PassKind::GlitchToNew);
    assert_eq!(
        glitch_to_new.category,
        ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort
    );
    assert_eq!(glitch_to_new.action, ApuCh4Nr43LfsrAction::None);
    assert!(trace.low_shift_followup.is_none());
    assert_eq!(
        trace.decision_category,
        ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort
    );
}

#[test]
fn channel_4_explicit_old_to_ff_pass_can_resolve_category_2() {
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
    let trace = last_channel_4_nr43_trace(&channel);
    let old_to_ff = require_nr43_pass(trace.old_to_ff, "old_to_ff");
    assert_eq!(old_to_ff.value_from, 0x00);
    assert_eq!(old_to_ff.value_to, 0xFF);
    assert!(!old_to_ff.bit_from);
    assert!(!old_to_ff.bit_to);
    assert_eq!(old_to_ff.category, ApuCh4Nr43LiveWriteCategory::Category2);
    assert_eq!(old_to_ff.action, ApuCh4Nr43LfsrAction::PlainStep);
    let ff_to_glitch_1 = require_nr43_pass(trace.ff_to_glitch_1, "ff_to_glitch_1");
    assert_eq!(ff_to_glitch_1.category, ApuCh4Nr43LiveWriteCategory::None);
    assert_eq!(ff_to_glitch_1.action, ApuCh4Nr43LfsrAction::None);
    assert_eq!(
        trace.decision_category,
        ApuCh4Nr43LiveWriteCategory::Category2
    );
    assert_eq!(trace.lfsr_action, ApuCh4Nr43LfsrAction::PlainStep);
}

#[test]
fn channel_4_explicit_ff_to_glitch_1_pass_can_resolve_category_1() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.runtime.active = true;
    channel.envelope.current_volume = 0x0F;
    channel.noise.lfsr_state = 0x642E;
    channel.write_nr43(0x00);
    channel.nr43_live_write.noise_counter = 0x0201;
    channel.nr43_live_write.countdown_reloaded = false;

    channel.write_nr43(0x90);

    let trace = last_channel_4_nr43_trace(&channel);
    let ff_to_glitch_1 = require_nr43_pass(trace.ff_to_glitch_1, "ff_to_glitch_1");
    assert_eq!(ff_to_glitch_1.value_from, 0xFF);
    assert_eq!(ff_to_glitch_1.value_to, 0xFF);
    assert_eq!(
        ff_to_glitch_1.category,
        ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort
    );
    assert_eq!(ff_to_glitch_1.action, ApuCh4Nr43LfsrAction::ForcedShortStep);
    assert_eq!(
        trace.decision_category,
        ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort
    );
    assert_eq!(trace.lfsr_action, ApuCh4Nr43LfsrAction::ForcedShortStep);
}

#[test]
fn channel_4_live_nr43_write_can_remain_inert_across_all_primary_passes() {
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
    let trace = last_channel_4_nr43_trace(&channel);
    assert_eq!(trace.decision_category, ApuCh4Nr43LiveWriteCategory::None);
    assert_eq!(trace.lfsr_action, ApuCh4Nr43LfsrAction::None);
    assert_eq!(
        require_nr43_pass(trace.old_to_ff, "old_to_ff").action,
        ApuCh4Nr43LfsrAction::None
    );
    assert_eq!(
        require_nr43_pass(trace.ff_to_glitch_1, "ff_to_glitch_1").action,
        ApuCh4Nr43LfsrAction::None
    );
    assert!(trace.glitch_1_to_glitch_2.is_none());
    assert_eq!(
        require_nr43_pass(trace.glitch_to_new, "glitch_to_new").action,
        ApuCh4Nr43LfsrAction::None
    );
    assert!(trace.low_shift_followup.is_none());
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

    assert_eq!(channel.noise.period_timer, noise_timer_reload(5, 0));
    assert_eq!(channel.nr43_live_write.counter_timer, 2);
    assert!(!channel.nr43_live_write.countdown_reloaded);
    let trace = last_channel_4_nr43_trace(&channel);
    let seam = require_nr43_pass(trace.reload_seam, "reload_seam");
    assert_eq!(seam.kind, ApuCh4Nr43PassKind::ReloadSeam);
    assert_eq!(seam.action, ApuCh4Nr43LfsrAction::PlainStep);
    assert_eq!(trace.lfsr_before, NOISE_LFSR_INITIAL_STATE);
    assert_eq!(trace.lfsr_after, channel.noise.lfsr_state);
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
    channel.nr43_live_write.noise_counter = 0x0082;
    channel.nr43_live_write.countdown_reloaded = false;

    channel.write_nr43(0x10);

    assert_eq!(channel.noise.period_timer, noise_timer_reload(1, 0));
    let trace = last_channel_4_nr43_trace(&channel);
    let old_to_ff = require_nr43_pass(trace.old_to_ff, "old_to_ff");
    assert_eq!(old_to_ff.category, ApuCh4Nr43LiveWriteCategory::None);
    assert_eq!(old_to_ff.action, ApuCh4Nr43LfsrAction::None);
    let ff_to_glitch_1 = require_nr43_pass(trace.ff_to_glitch_1, "ff_to_glitch_1");
    assert_eq!(
        ff_to_glitch_1.category,
        ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort
    );
    assert_eq!(
        ff_to_glitch_1.action,
        ApuCh4Nr43LfsrAction::ForcedShortStepThenLowShiftCorruption
    );
    assert_eq!(
        trace.decision_category,
        ApuCh4Nr43LiveWriteCategory::RisingEdgeForcedShort
    );
    assert_eq!(
        trace.lfsr_action,
        ApuCh4Nr43LfsrAction::ForcedShortStepThenLowShiftCorruption
    );
    assert_eq!(
        require_nr43_pass(trace.glitch_to_new, "glitch_to_new").action,
        ApuCh4Nr43LfsrAction::None
    );
    assert!(trace.low_shift_followup.is_none());
}

#[test]
fn channel_4_high_shift_narrow_staircase_runs_through_explicit_pre_cgb_d_passes() {
    let mut channel = Channel4State::default();
    channel.runtime.dac_enabled = true;
    channel.runtime.active = true;
    channel.envelope.current_volume = 0x0F;

    channel.noise.lfsr_state = 0x6189;
    channel.write_nr43(0x03);
    channel.nr43_live_write.noise_counter = 0x2078;
    channel.nr43_live_write.countdown_reloaded = false;
    channel.write_nr43(0x2C);
    let trace = last_channel_4_nr43_trace(&channel);
    assert_eq!(
        require_nr43_pass(trace.old_to_ff, "old_to_ff").action,
        ApuCh4Nr43LfsrAction::None
    );
    assert_eq!(
        require_nr43_pass(trace.ff_to_glitch_1, "ff_to_glitch_1").action,
        ApuCh4Nr43LfsrAction::None
    );
    assert_eq!(
        require_nr43_pass(trace.low_shift_followup, "low_shift_followup").action,
        ApuCh4Nr43LfsrAction::PlainStep
    );
    assert!(trace.glitch_1_to_glitch_2.is_none());

    channel.noise.lfsr_state = 0x3F3F;
    channel.write_nr43(0x4C);
    channel.nr43_live_write.noise_counter = 0x3A31;
    channel.nr43_live_write.countdown_reloaded = false;
    channel.write_nr43(0x5C);
    let trace = last_channel_4_nr43_trace(&channel);
    assert_eq!(
        require_nr43_pass(trace.ff_to_glitch_1, "ff_to_glitch_1").action,
        ApuCh4Nr43LfsrAction::ForcedShortStep
    );

    channel.noise.lfsr_state = 0x3F3F;
    channel.write_nr43(0x5C);
    channel.nr43_live_write.noise_counter = 0x02C3;
    channel.nr43_live_write.countdown_reloaded = false;
    channel.write_nr43(0x6C);
    let trace = last_channel_4_nr43_trace(&channel);
    assert_eq!(trace.glitch_1_value, 0x7F);
    assert_eq!(trace.glitch_2_value, None);
    assert_eq!(
        require_nr43_pass(trace.ff_to_glitch_1, "ff_to_glitch_1").action,
        ApuCh4Nr43LfsrAction::ForcedShortStep
    );

    channel.noise.lfsr_state = 0x3ABA;
    channel.write_nr43(0x6C);
    channel.nr43_live_write.noise_counter = 0x0B55;
    channel.nr43_live_write.countdown_reloaded = false;
    channel.write_nr43(0x7C);
    let trace = last_channel_4_nr43_trace(&channel);
    assert_eq!(
        require_nr43_pass(trace.ff_to_glitch_1, "ff_to_glitch_1").action,
        ApuCh4Nr43LfsrAction::None
    );

    channel.noise.lfsr_state = 0x7676;
    channel.write_nr43(0x7C);
    channel.nr43_live_write.noise_counter = 0x13E8;
    channel.nr43_live_write.countdown_reloaded = false;
    channel.write_nr43(0x6C);
    let trace = last_channel_4_nr43_trace(&channel);
    assert_eq!(
        require_nr43_pass(trace.ff_to_glitch_1, "ff_to_glitch_1").action,
        ApuCh4Nr43LfsrAction::ForcedShortStep
    );

    channel.noise.lfsr_state = 0x0909;
    channel.write_nr43(0x5C);
    channel.nr43_live_write.noise_counter = 0x250D;
    channel.nr43_live_write.countdown_reloaded = false;
    channel.write_nr43(0x4C);
    let trace = last_channel_4_nr43_trace(&channel);
    assert_eq!(
        require_nr43_pass(trace.old_to_ff, "old_to_ff").action,
        ApuCh4Nr43LfsrAction::PlainStep
    );
    assert_eq!(
        require_nr43_pass(trace.ff_to_glitch_1, "ff_to_glitch_1").action,
        ApuCh4Nr43LfsrAction::None
    );

    channel.noise.lfsr_state = 0x0707;
    channel.write_nr43(0x4C);
    channel.nr43_live_write.noise_counter = 0x2D9F;
    channel.nr43_live_write.countdown_reloaded = false;
    channel.write_nr43(0x3C);
    let trace = last_channel_4_nr43_trace(&channel);
    assert_eq!(
        require_nr43_pass(trace.ff_to_glitch_1, "ff_to_glitch_1").action,
        ApuCh4Nr43LfsrAction::ForcedShortStep
    );

    channel.noise.lfsr_state = 0x4BCB;
    channel.write_nr43(0x3C);
    channel.nr43_live_write.noise_counter = 0x3632;
    channel.nr43_live_write.countdown_reloaded = false;
    channel.write_nr43(0x09);
    let trace = last_channel_4_nr43_trace(&channel);
    assert_eq!(
        require_nr43_pass(trace.ff_to_glitch_1, "ff_to_glitch_1").action,
        ApuCh4Nr43LfsrAction::None
    );
    assert_eq!(
        require_nr43_pass(trace.glitch_to_new, "glitch_to_new").action,
        ApuCh4Nr43LfsrAction::None
    );
}

#[test]
fn channel_4_envelope_reaching_zero_does_not_disable_the_channel() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
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
    let mut apu = Apu::new(ConsoleModel::GameBoy);
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
    let mut apu = Apu::new(ConsoleModel::GameBoy);
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

fn cgb_channel_4_volume_after_live_nr42_write(old_value: u8, new_value: u8) -> u8 {
    let mut apu = Apu::new(ConsoleModel::GameBoyColor);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF21, old_value);
    apu.write_register(0xFF23, 0x80);
    apu.write_register(0xFF21, new_value);
    apu.channels.channel_4.envelope.current_volume
}

#[test]
fn cgb_live_nr42_writes_use_the_shared_zombie_volume_matrix_for_noise() {
    let cases = [
        (0x50, 0xF1, 0x04),
        (0x51, 0xF8, 0x09),
        (0x58, 0xF0, 0x0B),
        (0x58, 0xF8, 0x06),
        (0x59, 0xF8, 0x05),
    ];

    for (old_value, new_value, expected_volume) in cases {
        assert_eq!(
            cgb_channel_4_volume_after_live_nr42_write(old_value, new_value),
            expected_volume,
            "CH4 old={old_value:#04X} new={new_value:#04X}",
        );
    }
}

#[test]
fn live_nr42_write_requires_retrigger_before_reprogramming_the_noise_envelope() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
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
fn channel_4_live_15_bit_to_7_bit_switch_can_lock_the_active_lfsr_window_at_constant_volume() {
    let mut wide = Channel4State::default();
    wide.runtime.dac_enabled = true;
    wide.runtime.active = true;
    wide.nr43_live_write.counter_active = true;
    wide.nr43_live_write.background_counting = true;
    wide.nr43_live_write.counter_timer = 1;
    wide.nr43_live_write.noise_counter = 0;
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
    assert_eq!(narrow.current_digital_output(), 0x0F);
    assert!(narrow.runtime.active);

    narrow.nr43_live_write.counter_timer = 1;
    narrow.noise.period_timer = 1;
    narrow.tick_fast_timer();

    assert_eq!(narrow.noise.lfsr_state & 0x7F, 0x7F);
    assert_eq!(narrow.current_digital_output(), 0x0F);
    assert!(narrow.runtime.active);
}

#[test]
fn channel_4_retrigger_recovers_from_short_width_lockup_without_clearing_activity() {
    let mut apu = Apu::new(ConsoleModel::GameBoy);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF21, 0xF0);
    apu.write_register(0xFF22, 0x08);

    apu.channels.channel_4.runtime.active = true;
    apu.channels.channel_4.noise.lfsr_state = 0x007F;
    apu.channels.channel_4.envelope.current_volume = 0x0F;

    assert_eq!(apu.read_register(0xFF26) & 0x08, 0x08);
    assert_eq!(apu.channels.channel_4.current_digital_output(), 0x0F);

    apu.write_register(0xFF23, 0x80);

    assert_eq!(
        apu.channels.channel_4.noise.lfsr_state,
        NOISE_LFSR_INITIAL_STATE
    );
    assert!(apu.channels.channel_4.runtime.active);
    assert_eq!(apu.read_register(0xFF26) & 0x08, 0x08);
    assert_eq!(apu.channels.channel_4.current_digital_output(), 0);

    apu.channels.channel_4.noise.period_timer = 1;
    apu.channels.channel_4.tick_fast_timer();

    assert_ne!(apu.channels.channel_4.noise.lfsr_state & 0x7F, 0x7F);
}
