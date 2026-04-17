use super::*;

#[test]
fn channel_3_trigger_preserves_the_buffered_sample_until_the_next_wave_fetch() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.channels.channel_3.wave_ram[0] = 0x12;
    apu.channels.channel_3.wave_ram[1] = 0x34;
    apu.channels.channel_3.sample_buffer = 0x0E;

    apu.write_register(0xFF1A, 0x80);
    apu.write_register(0xFF1C, 0x20);
    apu.write_register(0xFF1D, 0xFF);
    apu.write_register(0xFF1E, 0x87);

    assert!(apu.channels.channel_3.runtime.active);
    assert_eq!(apu.channels.channel_3.sample_index, 0);
    assert_eq!(apu.channels.channel_3.sample_buffer, 0x0E);
    assert_eq!(
        apu.channels.channel_3.period_timer,
        2 + WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES
    );

    for expected_timer in (1..=1 + WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES).rev() {
        apu.channels.channel_3.tick_fast_timer();
        assert_eq!(apu.channels.channel_3.sample_buffer, 0x0E);
        assert_eq!(apu.channels.channel_3.period_timer, expected_timer);
    }

    apu.channels.channel_3.tick_fast_timer();
    assert_eq!(apu.channels.channel_3.sample_index, 1);
    assert_eq!(apu.channels.channel_3.sample_buffer, 0x02);
    assert_eq!(apu.channels.channel_3.period_timer, 2);
}

#[test]
fn channel_3_period_writes_take_effect_only_after_the_next_wave_fetch_boundary() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.channels.channel_3.wave_ram[0] = 0x12;

    apu.write_register(0xFF1A, 0x80);
    apu.write_register(0xFF1C, 0x20);
    apu.write_register(0xFF1D, 0xFF);
    apu.write_register(0xFF1E, 0x87);

    apu.channels.channel_3.tick_fast_timer();
    assert_eq!(
        apu.channels.channel_3.period_timer,
        1 + WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES
    );

    apu.write_register(0xFF1D, 0xFE);
    apu.write_register(0xFF1E, 0x07);

    assert_eq!(apu.channels.channel_3.period_value(), 0x07FE);
    assert_eq!(
        apu.channels.channel_3.period_timer,
        1 + WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES
    );
    assert_eq!(apu.channels.channel_3.sample_buffer, 0);

    for expected_timer in (1..=WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES).rev() {
        apu.channels.channel_3.tick_fast_timer();
        assert_eq!(apu.channels.channel_3.sample_index, 0);
        assert_eq!(apu.channels.channel_3.sample_buffer, 0);
        assert_eq!(apu.channels.channel_3.period_timer, expected_timer);
    }

    apu.channels.channel_3.tick_fast_timer();
    assert_eq!(apu.channels.channel_3.sample_index, 1);
    assert_eq!(apu.channels.channel_3.sample_buffer, 0x02);
    assert_eq!(apu.channels.channel_3.period_timer, 4);
}

#[test]
fn channel_3_output_level_applies_immediate_digital_attenuation_without_disabling_it() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF1A, 0x80);
    apu.channels.channel_3.runtime.active = true;
    apu.channels.channel_3.sample_buffer = 0x0C;

    apu.write_register(0xFF1C, 0x00);
    assert_eq!(apu.channels.channel_3.current_digital_output(), 0);
    assert!(apu.channels.channel_3.runtime.active);

    apu.write_register(0xFF1C, 0x20);
    assert_eq!(apu.channels.channel_3.current_digital_output(), 0x0C);

    apu.write_register(0xFF1C, 0x40);
    assert_eq!(apu.channels.channel_3.current_digital_output(), 0x06);

    apu.write_register(0xFF1C, 0x60);
    assert_eq!(apu.channels.channel_3.current_digital_output(), 0x03);
    assert!(apu.channels.channel_3.runtime.active);
}

#[test]
fn channel_3_fast_timer_advances_sample_state_while_the_channel_is_inactive() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.channels.channel_3.wave_ram[0] = 0x12;
    apu.write_register(0xFF1A, 0x80);
    apu.write_register(0xFF1C, 0x20);
    apu.write_register(0xFF1D, 0xFF);
    apu.write_register(0xFF1E, 0x87);

    apu.channels.channel_3.runtime.active = false;
    apu.channels.channel_3.sample_index = 0;
    apu.channels.channel_3.sample_buffer = 0x0E;
    apu.channels.channel_3.period_timer = 1;

    apu.channels.channel_3.tick_fast_timer();

    assert_eq!(apu.channels.channel_3.sample_index, 1);
    assert_eq!(apu.channels.channel_3.sample_buffer, 0x02);
    assert_eq!(apu.channels.channel_3.period_timer, 2);
    assert!(!apu.channels.channel_3.runtime.active);
    assert_eq!(apu.channels.channel_3.current_digital_output(), 0);
}

#[test]
fn channel_3_trigger_reloads_timer_and_index_but_does_not_activate_while_the_dac_is_off() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF1A, 0x00);
    apu.write_register(0xFF1D, 0xAB);
    apu.channels.channel_3.sample_index = 17;
    apu.channels.channel_3.sample_buffer = 0x0E;
    apu.channels.channel_3.period_timer = 99;
    apu.channels.channel_3.length_counter = 7;

    apu.write_register(0xFF1E, 0x80);

    assert!(!apu.channels.channel_3.runtime.active);
    assert_eq!(apu.channels.channel_3.sample_index, 0);
    assert_eq!(apu.channels.channel_3.sample_buffer, 0x0E);
    assert_eq!(
        apu.channels.channel_3.period_timer,
        wave_timer_reload(0x00AB) + WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES
    );
    assert_eq!(apu.channels.channel_3.length_counter, 7);
}

#[test]
fn active_channel_3_wave_ram_reads_return_ff_outside_the_dmg_fetch_window() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF1A, 0x80);
    apu.channels.channel_3.runtime.active = true;
    apu.channels.channel_3.wave_ram[0] = 0x12;
    apu.channels.channel_3.wave_ram[1] = 0x34;

    assert_eq!(apu.read_register(0xFF30), WAVE_RAM_INACCESSIBLE_READ_VALUE);
    assert_eq!(apu.read_register(0xFF3F), WAVE_RAM_INACCESSIBLE_READ_VALUE);
}

#[test]
fn active_channel_3_wave_ram_writes_are_ignored_outside_the_dmg_fetch_window() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF1A, 0x80);
    apu.channels.channel_3.runtime.active = true;
    apu.channels.channel_3.wave_ram[0] = 0x12;
    apu.channels.channel_3.wave_ram[1] = 0x34;

    apu.write_register(0xFF30, 0xAB);

    assert_eq!(apu.channels.channel_3.wave_ram[0], 0x12);
    assert_eq!(apu.channels.channel_3.wave_ram[1], 0x34);
}

#[test]
fn dmg_channel_3_wave_ram_access_uses_the_internal_byte_only_during_the_fetch_window() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF1A, 0x80);
    apu.write_register(0xFF1C, 0x20);
    apu.write_register(0xFF1D, 0xFF);
    apu.write_register(0xFF1E, 0x87);
    apu.channels.channel_3.wave_ram[0] = 0x12;
    apu.channels.channel_3.wave_ram[1] = 0x34;

    for _ in 0..2 + WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES {
        apu.channels.channel_3.begin_t_cycle();
        apu.channels.channel_3.tick_fast_timer();
    }

    assert_eq!(apu.channels.channel_3.sample_index, 1);
    assert_eq!(apu.read_register(0xFF30), 0x12);
    assert_eq!(apu.read_register(0xFF3F), 0x12);

    apu.write_register(0xFF30, 0xAB);

    assert_eq!(apu.channels.channel_3.wave_ram[0], 0xAB);
    assert_eq!(apu.channels.channel_3.wave_ram[1], 0x34);

    apu.channels.channel_3.begin_t_cycle();

    assert_eq!(apu.read_register(0xFF30), WAVE_RAM_INACCESSIBLE_READ_VALUE);
}

#[test]
fn cgb_channel_3_wave_ram_remains_addressable_while_inactive() {
    let mut apu = Apu::new(ConsoleModel::Cgb);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF1A, 0x80);
    apu.channels.channel_3.wave_ram[0] = 0x12;
    apu.channels.channel_3.wave_ram[1] = 0x34;

    assert_eq!(apu.read_register(0xFF30), 0x12);
    assert_eq!(apu.read_register(0xFF31), 0x34);

    apu.write_register(0xFF30, 0xAB);

    assert_eq!(apu.channels.channel_3.wave_ram[0], 0xAB);
    assert_eq!(apu.channels.channel_3.wave_ram[1], 0x34);
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
    let mut channel = Channel3State::default();
    channel.apply_powered_startup(NR30_DAC_POWER_BIT, 0, 0, 0, 0, true);
    channel.runtime = ChannelRuntimeState {
        dac_enabled: true,
        active: true,
    };
    channel
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

    assert_eq!(apu.channels.channel_3.length_counter, 0);
    assert!(!apu.channels.channel_3.runtime.active);

    apu.write_register(0xFF1E, CHANNEL_TRIGGER_BIT | LENGTH_ENABLE_BIT);

    assert_eq!(apu.channels.channel_3.length_counter, 255);
    assert!(apu.channels.channel_3.runtime.active);
}
