use super::*;
use crate::apu::registers::{UNUSED_NR1F_ADDRESS, UNUSED_NR15_ADDRESS, WAVE_RAM_START_ADDRESS};

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
fn nr52_power_on_restarts_the_ch4_hidden_startup_phase_from_zero() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF26, 0x00);

    for t_cycle in 0..5 {
        tick_apu_with_edges(&mut apu, t_cycle, &[]);
    }
    assert_ne!(apu.channel_4_debug_snapshot().alignment, 0);

    apu.write_register(0xFF26, 0x80);

    let snapshot = apu.channel_4_debug_snapshot();
    assert_eq!(snapshot.alignment, 0);
    assert!(!snapshot.countdown_reloaded);
    assert_eq!(snapshot.counter_timer, 0);
    assert_eq!(snapshot.noise_counter, 0);
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
        apu.channels.channel_1.pulse.length_counter,
        pulse_length_counter_from_load(0xD5)
    );
    assert_eq!(
        apu.channels.channel_2.pulse.length_counter,
        pulse_length_counter_from_load(0xEA)
    );
    assert_eq!(
        apu.channels.channel_3.length_counter,
        wave_length_counter_from_load(0x44)
    );
    assert_eq!(
        apu.channels.channel_4.length_counter,
        pulse_length_counter_from_load(0xCD)
    );
    assert_eq!(apu.read_register(0xFF11), 0x3F);
    assert_eq!(apu.read_register(0xFF16), 0x3F);
    assert_eq!(apu.read_register(0xFF1B), 0xFF);
    assert_eq!(apu.read_register(0xFF20), 0xFF);
}

#[test]
fn dmg_powered_off_non_length_writes_leave_preserved_length_counters_unchanged() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF26, 0x00);

    apu.write_register(0xFF11, 0xD5);
    apu.write_register(0xFF16, 0xEA);
    apu.write_register(0xFF1B, 0x44);
    apu.write_register(0xFF20, 0xCD);

    let expected_length_counters = (
        apu.channels.channel_1.pulse.length_counter,
        apu.channels.channel_2.pulse.length_counter,
        apu.channels.channel_3.length_counter,
        apu.channels.channel_4.length_counter,
    );

    for (address, value) in [
        (0xFF10, 0x7F),
        (0xFF12, 0xF3),
        (0xFF14, 0xC7),
        (0xFF17, 0xF3),
        (0xFF19, 0xC7),
        (0xFF1A, 0x80),
        (0xFF1C, 0x60),
        (0xFF1E, 0xC7),
        (0xFF21, 0xF3),
        (0xFF22, 0x35),
        (0xFF23, 0xC7),
        (0xFF24, 0x77),
        (0xFF25, 0xF3),
    ] {
        apu.write_register(address, value);
    }

    assert_eq!(
        (
            apu.channels.channel_1.pulse.length_counter,
            apu.channels.channel_2.pulse.length_counter,
            apu.channels.channel_3.length_counter,
            apu.channels.channel_4.length_counter,
        ),
        expected_length_counters
    );
    assert_eq!(apu.read_register(0xFF12), 0x00);
    assert_eq!(apu.read_register(0xFF17), 0x00);
    assert_eq!(apu.read_register(0xFF1A), 0x7F);
    assert_eq!(apu.read_register(0xFF21), 0x00);
    assert_eq!(apu.read_register(0xFF24), 0x00);
    assert_eq!(apu.read_register(0xFF25), 0x00);
}

#[test]
fn cgb_powered_off_length_writes_are_ignored_for_all_channels() {
    let mut apu = Apu::new(ConsoleModel::Cgb);

    for (address, value) in [
        (0xFF11, 0xD5),
        (0xFF16, 0xEA),
        (0xFF1B, 0x44),
        (0xFF20, 0xCD),
    ] {
        apu.write_register(address, value);
    }

    assert_eq!(apu.channels.channel_1.pulse.length_counter, 0);
    assert_eq!(apu.channels.channel_2.pulse.length_counter, 0);
    assert_eq!(apu.channels.channel_3.length_counter, 0);
    assert_eq!(apu.channels.channel_4.length_counter, 0);
    assert_eq!(apu.read_register(0xFF11), 0x3F);
    assert_eq!(apu.read_register(0xFF16), 0x3F);
    assert_eq!(apu.read_register(0xFF1B), 0xFF);
    assert_eq!(apu.read_register(0xFF20), 0xFF);
}

#[test]
fn register_write_observation_is_recorded_only_for_decoded_apu_registers() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);

    for (address, value, expected_observation) in [
        (UNUSED_NR15_ADDRESS, 0x34, true),
        (UNUSED_NR1F_ADDRESS, 0x12, true),
        (WAVE_RAM_START_ADDRESS, 0xAB, false),
        (0xFF27, 0x56, false),
        (0xFF40, 0x78, false),
    ] {
        apu.write_register(address, value);
        let observation = apu.snapshot().last_register_write;

        if expected_observation {
            let observation = observation.expect("decoded APU register write should be observed");
            assert_eq!(observation.address, address);
            assert_eq!(observation.value, value);
            assert_eq!(observation.before, observation.after);
        } else {
            assert!(
                observation.is_none(),
                "{address:#06X} should not be observed"
            );
        }
    }

    assert_eq!(apu.read_register(WAVE_RAM_START_ADDRESS), 0xAB);
}

#[test]
fn observed_nr52_power_off_write_captures_the_before_and_after_power_transition() {
    let mut apu = Apu::new(ConsoleModel::Dmg);
    apu.write_register(0xFF26, 0x80);
    apu.write_register(0xFF12, 0xF3);
    apu.write_register(0xFF14, 0x80);
    apu.write_register(0xFF24, 0x77);
    apu.write_register(0xFF25, 0x11);

    apu.write_register(0xFF26, 0x00);

    let observation = apu
        .snapshot()
        .last_register_write
        .expect("NR52 power-off write should be observed");
    assert_eq!(observation.address, 0xFF26);
    assert_eq!(observation.value, 0x00);
    assert!(observation.before.powered);
    assert!(!observation.after.powered);
    assert_eq!(observation.before.nr52, 0xF1);
    assert_eq!(observation.after.nr52, 0x70);
    assert_eq!(observation.before.channel_active_mask, CHANNEL_ACTIVE_CH1);
    assert_eq!(observation.after.channel_active_mask, 0x00);
    assert_eq!(observation.before.channel_dac_mask, CHANNEL_ACTIVE_CH1);
    assert_eq!(observation.after.channel_dac_mask, 0x00);
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
