use super::*;
use crate::scheduler::TCycle;

#[test]
fn div_is_derived_from_the_internal_counter_and_reset_by_any_write() {
    let mut timer = Timer::new(ConsoleModel::GameBoy);

    timer.system_counter = 0xABCD;

    assert_eq!(timer.read_div(), 0xAB);

    let effects = timer.write_div_with_effects(0xFF);

    assert_eq!(timer.read_div(), 0x00);
    assert_eq!(timer.system_counter, 0);
    assert!(!effects.apu_frame_sequencer_edge);
}

#[test]
fn tac_forces_unused_bits_high_and_masks_writes_to_control_bits() {
    let mut timer = Timer::new(ConsoleModel::GameBoy);

    timer.write_tac(0xFF);

    assert_eq!(timer.read_tac(), 0xFF);
    assert_eq!(timer.tac, 0x07);
}

#[test]
fn startup_state_applies_the_visible_post_boot_timer_snapshot() {
    let mut timer = Timer::new(ConsoleModel::GameBoy);

    timer.apply_startup_state(TimerStartupState {
        system_counter: 0xAB00,
        tima: 0x00,
        tma: 0x00,
        tac: 0x00,
    });

    assert_eq!(timer.read_div(), 0xAB);
    assert_eq!(timer.read_tima(), 0x00);
    assert_eq!(timer.read_tma(), 0x00);
    assert_eq!(timer.read_tac(), 0xF8);
}

fn tick_timer(timer: &mut Timer, t_cycle: u64) -> CycleContext {
    let mut context = CycleContext::for_cycle(TCycle::new(t_cycle));
    timer.tick_t_cycle(&mut context);
    context
}

fn tick_timer_for_speed(
    timer: &mut Timer,
    t_cycle: u64,
    speed_mode: crate::speed::CgbSpeedMode,
) -> CycleContext {
    let mut context = CycleContext::for_cycle(TCycle::new(t_cycle));
    timer.tick_t_cycle_for_speed(&mut context, speed_mode);
    context
}

#[test]
fn bit_3_timer_frequency_increments_tima_on_falling_edges() {
    let mut timer = Timer::new(ConsoleModel::GameBoy);
    timer.write_tac(0x05);

    for t_cycle in 0..15 {
        tick_timer(&mut timer, t_cycle);
    }

    assert_eq!(timer.read_tima(), 0x00);

    tick_timer(&mut timer, 15);
    assert_eq!(timer.read_tima(), 0x01);

    for t_cycle in 16..31 {
        tick_timer(&mut timer, t_cycle);
    }

    assert_eq!(timer.read_tima(), 0x01);

    tick_timer(&mut timer, 31);
    assert_eq!(timer.read_tima(), 0x02);
}

#[test]
fn div_write_falling_edge_glitch_can_increment_tima() {
    let mut timer = Timer::new(ConsoleModel::GameBoy);
    timer.apply_startup_state(TimerStartupState {
        system_counter: 0x0008,
        tima: 0x0F,
        tma: 0x00,
        tac: 0x05,
    });

    let effects = timer.write_div_with_effects(0x00);

    assert_eq!(timer.read_tima(), 0x10);
    assert_eq!(timer.read_div(), 0x00);
    assert!(!effects.apu_frame_sequencer_edge);
}

#[test]
fn div_write_reports_when_the_apu_frame_sequencer_edge_occurs() {
    let mut timer = Timer::new(ConsoleModel::GameBoy);
    timer.apply_startup_state(TimerStartupState {
        system_counter: 0x1000,
        tima: 0x00,
        tma: 0x00,
        tac: 0x00,
    });

    let effects = timer.write_div_with_effects(0x00);

    assert!(effects.apu_frame_sequencer_edge);
    assert_eq!(timer.snapshot().system_counter, 0x0000);
}

#[test]
fn tac_write_falling_edge_glitch_can_increment_tima() {
    let mut timer = Timer::new(ConsoleModel::GameBoy);
    timer.apply_startup_state(TimerStartupState {
        system_counter: 0x0020,
        tima: 0x2A,
        tma: 0x00,
        tac: 0x06,
    });

    timer.write_tac(0x04);

    assert_eq!(timer.read_tima(), 0x2B);
}

#[test]
fn overflow_reloads_tima_and_requests_timer_interrupt_four_t_cycles_later() {
    let mut timer = Timer::new(ConsoleModel::GameBoy);
    timer.write_tima(0xFF);
    timer.write_tma(0x77);
    timer.write_tac(0x05);

    for t_cycle in 0..15 {
        let context = tick_timer(&mut timer, t_cycle);
        assert!(context.interrupt_requests().is_empty());
    }

    let overflow_cycle = tick_timer(&mut timer, 15);
    assert!(overflow_cycle.interrupt_requests().is_empty());
    assert_eq!(timer.read_tima(), 0x00);

    for t_cycle in 16..19 {
        let context = tick_timer(&mut timer, t_cycle);
        assert!(context.interrupt_requests().is_empty());
        assert_eq!(timer.read_tima(), 0x00);
    }

    let reload_cycle = tick_timer(&mut timer, 19);
    assert_eq!(timer.read_tima(), 0x77);
    assert_eq!(reload_cycle.interrupt_requests(), &[InterruptSource::Timer]);
}

#[test]
fn tac_glitch_overflow_reloads_without_slipping_an_extra_t_cycle() {
    let mut timer = Timer::new(ConsoleModel::GameBoy);
    timer.apply_startup_state(TimerStartupState {
        system_counter: 0x0200,
        tima: 0xFF,
        tma: 0x77,
        tac: 0x04,
    });

    timer.write_tac(0x00);
    assert_eq!(timer.read_tima(), 0x00);

    for t_cycle in 0..2 {
        let context = tick_timer(&mut timer, t_cycle);
        assert!(context.interrupt_requests().is_empty());
        assert_eq!(timer.read_tima(), 0x00);
    }

    let reload_cycle = tick_timer(&mut timer, 2);
    assert_eq!(timer.read_tima(), 0x77);
    assert_eq!(reload_cycle.interrupt_requests(), &[InterruptSource::Timer]);
}

#[test]
fn tima_write_during_pending_reload_cancels_the_reload_and_irq_request() {
    let mut timer = Timer::new(ConsoleModel::GameBoy);
    timer.write_tima(0xFF);
    timer.write_tma(0x99);
    timer.write_tac(0x05);

    for t_cycle in 0..16 {
        tick_timer(&mut timer, t_cycle);
    }

    assert_eq!(timer.read_tima(), 0x00);
    timer.write_tima(0x44);

    for t_cycle in 16..20 {
        let context = tick_timer(&mut timer, t_cycle);
        assert!(context.interrupt_requests().is_empty());
    }

    assert_eq!(timer.read_tima(), 0x44);
}

#[test]
fn tma_write_before_reload_changes_the_value_copied_into_tima() {
    let mut timer = Timer::new(ConsoleModel::GameBoy);
    timer.write_tima(0xFF);
    timer.write_tma(0x12);
    timer.write_tac(0x05);

    for t_cycle in 0..16 {
        tick_timer(&mut timer, t_cycle);
    }

    timer.write_tma(0x34);

    for t_cycle in 16..19 {
        tick_timer(&mut timer, t_cycle);
    }

    let context = tick_timer(&mut timer, 19);

    assert_eq!(timer.read_tima(), 0x34);
    assert_eq!(context.interrupt_requests(), &[InterruptSource::Timer]);
}

#[test]
fn tima_write_on_the_reload_cycle_is_ignored() {
    let mut timer = Timer::new(ConsoleModel::GameBoy);
    timer.write_tima(0xFF);
    timer.write_tma(0x55);
    timer.write_tac(0x05);

    for t_cycle in 0..20 {
        tick_timer(&mut timer, t_cycle);
    }

    assert_eq!(timer.read_tima(), 0x55);

    timer.write_tima(0x77);

    assert_eq!(timer.read_tima(), 0x55);
}

#[test]
fn tma_write_on_the_reload_cycle_updates_the_reloaded_tima_value() {
    let mut timer = Timer::new(ConsoleModel::GameBoy);
    timer.write_tima(0xFF);
    timer.write_tma(0x12);
    timer.write_tac(0x05);

    for t_cycle in 0..20 {
        tick_timer(&mut timer, t_cycle);
    }

    timer.write_tma(0x34);

    assert_eq!(timer.read_tma(), 0x34);
    assert_eq!(timer.read_tima(), 0x34);
}

#[test]
fn shared_divider_tick_publishes_timer_and_apu_edges_into_the_cycle_context() {
    let mut timer = Timer::new(ConsoleModel::GameBoy);
    timer.apply_startup_state(TimerStartupState {
        system_counter: 0x1FFF,
        tima: 0x0F,
        tma: 0x00,
        tac: 0x05,
    });

    let context = tick_timer(&mut timer, 0);

    assert_eq!(
        context.derived_edges(),
        &[
            DerivedEdge::DividerTick,
            DerivedEdge::TimerInputFallingEdge,
            DerivedEdge::ApuFrameSequencerEdge,
        ]
    );
    assert_eq!(timer.snapshot().system_counter, 0x2000);
    assert_eq!(timer.read_tima(), 0x10);
}

#[test]
fn cgb_double_speed_keeps_divider_cpu_visible_cadence() {
    let mut timer = Timer::new(ConsoleModel::GameBoyColor);
    timer.apply_startup_state(TimerStartupState {
        system_counter: 0x00FF,
        tima: 0x00,
        tma: 0x00,
        tac: 0x00,
    });

    let context = tick_timer_for_speed(&mut timer, 0, crate::speed::CgbSpeedMode::Double);

    assert_eq!(timer.snapshot().system_counter, 0x0100);
    assert_eq!(timer.read_div(), 0x01);
    assert_eq!(context.derived_edges(), &[DerivedEdge::DividerTick]);
}

#[test]
fn cgb_double_speed_keeps_apu_frame_sequencer_on_undoubled_divider_domain() {
    let mut timer = Timer::new(ConsoleModel::GameBoyColor);
    timer.apply_startup_state(TimerStartupState {
        system_counter: 0x1FFF,
        tima: 0x00,
        tma: 0x00,
        tac: 0x00,
    });

    let normal_context = tick_timer_for_speed(&mut timer, 0, crate::speed::CgbSpeedMode::Double);
    assert_eq!(timer.snapshot().system_counter, 0x2000);
    assert_eq!(normal_context.derived_edges(), &[DerivedEdge::DividerTick]);

    timer.apply_startup_state(TimerStartupState {
        system_counter: 0x3FFF,
        tima: 0x00,
        tma: 0x00,
        tac: 0x00,
    });
    let apu_context = tick_timer_for_speed(&mut timer, 1, crate::speed::CgbSpeedMode::Double);

    assert_eq!(timer.snapshot().system_counter, 0x4000);
    assert_eq!(
        apu_context.derived_edges(),
        &[DerivedEdge::DividerTick, DerivedEdge::ApuFrameSequencerEdge]
    );
}

#[test]
fn cgb_double_speed_div_write_uses_double_speed_apu_divider_bit() {
    let mut timer = Timer::new(ConsoleModel::GameBoyColor);
    timer.apply_startup_state(TimerStartupState {
        system_counter: 0x1000,
        tima: 0x00,
        tma: 0x00,
        tac: 0x00,
    });

    let normal_effects =
        timer.write_div_with_effects_for_speed(0x00, crate::speed::CgbSpeedMode::Double);
    assert!(!normal_effects.apu_frame_sequencer_edge);

    timer.apply_startup_state(TimerStartupState {
        system_counter: 0x2000,
        tima: 0x00,
        tma: 0x00,
        tac: 0x00,
    });
    let double_effects =
        timer.write_div_with_effects_for_speed(0x00, crate::speed::CgbSpeedMode::Double);
    assert!(double_effects.apu_frame_sequencer_edge);
}
