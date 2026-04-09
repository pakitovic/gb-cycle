use super::*;

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
fn powering_on_keeps_waiting_for_the_next_live_div_apu_edge() {
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

    apu.write_register(0xFF26, 0x80);
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
