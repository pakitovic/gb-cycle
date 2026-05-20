use super::*;

#[test]
fn sc_forces_reserved_bits_high_and_tracks_control_fields() {
    let mut serial = Serial::new(ConsoleModel::GameBoy);

    serial.write_sc(0x81);

    assert_eq!(serial.read_sc(), 0xFF);
    assert_eq!(serial.clock_mode, SerialClockMode::Internal);
    assert_eq!(
        serial.transfer_state,
        SerialTransferState::TransferRequested { bits_shifted: 0 }
    );
}

#[test]
fn startup_state_recreates_the_documented_post_boot_sb_and_sc_snapshot() {
    let mut serial = Serial::new(ConsoleModel::GameBoy);

    serial.apply_startup_state(SerialStartupState::from_registers(0x00, 0x7E));

    assert_eq!(serial.read_sb(), 0x00);
    assert_eq!(serial.read_sc(), 0x7E);
    assert_eq!(serial.clock_mode, SerialClockMode::External);
    assert_eq!(serial.transfer_state, SerialTransferState::Idle);
}

#[test]
fn startup_state_and_sc_writes_cover_internal_and_external_transfer_modes() {
    let startup_state = SerialStartupState::from_registers(0xA5, 0x81);
    assert_eq!(startup_state.sb, 0xA5);
    assert_eq!(startup_state.clock_mode, SerialClockMode::Internal);
    assert!(!startup_state.cgb_high_speed_clock);
    assert_eq!(
        startup_state.transfer_state,
        SerialTransferState::TransferRequested { bits_shifted: 0 }
    );
    assert_eq!(startup_state.clock_counter, 0);

    let mut serial = Serial::new(ConsoleModel::GameBoy);
    serial.apply_startup_state(startup_state);
    assert_eq!(serial.read_sc(), 0xFF);

    serial.write_sc(0x00);
    assert_eq!(serial.read_sc(), 0x7E);
    assert_eq!(serial.clock_mode, SerialClockMode::External);
    assert_eq!(serial.transfer_state, SerialTransferState::Idle);
}

#[test]
fn scheduler_trace_message_reports_cycle_phase_and_console_model() {
    let serial = Serial::new(ConsoleModel::GameBoy);
    let context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);
    let trace = serial.scheduler_trace_message(&context);

    assert_eq!(
        trace,
        "t_cycle=0 phase=external_event_ingress console_model=GameBoy status=Ready sb=0x00 clock_mode=External transfer_state=Idle peer=Disconnected"
    );
}

#[test]
fn cgb_native_and_dmg_ext_modes_latch_sc1_while_compatibility_keeps_it_forced_high() {
    let mut cgb = Serial::new_with_operating_mode(ConsoleModel::GameBoyColor, OperatingMode::Cgb);

    cgb.write_sc(0x81);
    assert_eq!(cgb.read_sc(), 0xFD);
    assert!(!cgb.cgb_high_speed_clock());

    cgb.write_sc(0x83);
    assert_eq!(cgb.read_sc(), 0xFF);
    assert!(cgb.cgb_high_speed_clock());

    cgb.apply_operating_mode_state(OperatingMode::GbCompatible);
    assert_eq!(cgb.read_sc(), 0xFF);
    assert!(!cgb.cgb_high_speed_clock());

    cgb.write_sc(0x83);
    assert_eq!(cgb.read_sc(), 0xFF);
    assert!(!cgb.cgb_high_speed_clock());

    cgb.apply_operating_mode_state(OperatingMode::CgbDmgExt);
    cgb.write_sc(0x83);
    assert_eq!(cgb.read_sc(), 0xFF);
    assert!(cgb.cgb_high_speed_clock());
    cgb.write_sc(0x81);
    assert_eq!(cgb.read_sc(), 0xFD);
    assert!(!cgb.cgb_high_speed_clock());

    let mut dmg = Serial::new(ConsoleModel::GameBoy);
    dmg.write_sc(0x83);
    assert_eq!(dmg.read_sc(), 0xFF);
    assert!(!dmg.cgb_high_speed_clock());
}

#[test]
fn internal_clock_shifts_sb_bit_by_bit_and_requests_irq_on_completion() {
    let mut serial = Serial::new(ConsoleModel::GameBoy);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    serial.write_sb(0x81);
    serial.write_sc(0x81);

    for _ in 0..511 {
        serial.tick_t_cycle(&mut context);
        assert!(context.interrupt_requests().is_empty());
        assert_eq!(
            serial.transfer_state(),
            SerialTransferState::TransferRequested { bits_shifted: 0 }
        );
    }

    serial.tick_t_cycle(&mut context);
    assert_eq!(serial.read_sb(), 0x03);
    assert_eq!(
        serial.transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 1 }
    );
    assert!(context.interrupt_requests().is_empty());

    for _ in 0..(7 * 512) {
        serial.tick_t_cycle(&mut context);
    }

    assert_eq!(serial.read_sb(), 0xFF);
    assert_eq!(serial.read_sc(), 0x7F);
    assert_eq!(serial.transfer_state(), SerialTransferState::Idle);
    assert_eq!(context.interrupt_requests(), &[InterruptSource::Serial]);
    assert_eq!(serial.take_completed_output_bytes(), vec![0x81]);
    assert!(serial.take_completed_output_bytes().is_empty());
}

#[test]
fn idle_tick_matches_full_tick_without_transfer_and_reports_no_active_serial_work() {
    let mut idle = Serial::new(ConsoleModel::GameBoyColor);
    idle.apply_startup_state(
        SerialStartupState::from_registers(0x42, 0x7E).with_clock_counter(0x1234),
    );
    let mut full = idle.clone();
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    idle.tick_idle_t_cycle();
    let telemetry = full.tick_t_cycle_for_speed(&mut context, CgbSpeedMode::Double);

    assert_eq!(full, idle);
    assert_eq!(telemetry, SerialTickTelemetry::default());
    assert!(context.interrupt_requests().is_empty());
}

#[test]
fn slave_mode_waits_for_external_clocks() {
    let mut serial = Serial::new(ConsoleModel::GameBoy);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    serial.write_sb(0xA5);
    serial.write_sc(0x80);

    for _ in 0..2048 {
        serial.tick_t_cycle(&mut context);
    }

    assert_eq!(serial.read_sb(), 0xA5);
    assert_eq!(serial.read_sc(), 0xFE);
    assert_eq!(
        serial.transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 0 }
    );
    assert!(context.interrupt_requests().is_empty());
    assert!(serial.take_completed_output_bytes().is_empty());
}

#[test]
fn external_transfer_without_pending_pulse_reports_wait_tick_without_shift() {
    let mut serial = Serial::new(ConsoleModel::GameBoy);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    serial.write_sb(0xA5);
    serial.write_sc(0x80);
    let previous_counter = serial.clock_counter;

    let telemetry = serial.tick_t_cycle(&mut context);

    assert_eq!(
        telemetry,
        SerialTickTelemetry {
            active_t_cycles: 1,
            external_ticks: 1,
            external_wait_ticks: 1,
            ..Default::default()
        }
    );
    assert_eq!(serial.clock_counter, previous_counter.wrapping_add(1));
    assert_eq!(serial.read_sb(), 0xA5);
    assert_eq!(
        serial.transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 0 }
    );
    assert!(context.interrupt_requests().is_empty());
}

#[test]
fn external_wait_fast_path_matches_full_tick_for_long_windows() {
    for ticks in [1_u64, 512, 140_448] {
        let mut fast = Serial::new(ConsoleModel::GameBoyColor);
        fast.apply_startup_state(
            SerialStartupState::from_registers(0xA5, 0x80).with_clock_counter(0xFF00),
        );
        fast.set_peer(SerialPeer::StagedIncomingByte { byte: 0x5A });
        let mut full = fast.clone();
        let mut full_context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);
        let mut fast_telemetry = SerialTickTelemetry::default();
        let mut full_telemetry = SerialTickTelemetry::default();

        for _ in 0..ticks {
            assert!(fast.external_wait_without_pending_clock());
            fast_telemetry.accumulate(fast.tick_external_wait_t_cycle());
            full_telemetry
                .accumulate(full.tick_t_cycle_for_speed(&mut full_context, CgbSpeedMode::Double));
        }

        assert_eq!(
            fast, full,
            "external wait fast path diverged after {ticks} T-cycles"
        );
        assert_eq!(fast_telemetry, full_telemetry);
        assert_eq!(fast_telemetry.active_t_cycles, ticks);
        assert_eq!(fast_telemetry.external_ticks, ticks);
        assert_eq!(fast_telemetry.external_wait_ticks, ticks);
        assert_eq!(fast_telemetry.shift_edges, 0);
        assert!(full_context.interrupt_requests().is_empty());
    }
}

#[test]
fn external_transfer_with_pending_pulse_reports_shift_edge_not_wait_tick() {
    let mut serial = Serial::new(ConsoleModel::GameBoy);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    serial.write_sb(0x81);
    serial.write_sc(0x80);

    assert!(serial.queue_external_clock_pulse());
    let telemetry = serial.tick_t_cycle(&mut context);

    assert_eq!(
        telemetry,
        SerialTickTelemetry {
            active_t_cycles: 1,
            external_ticks: 1,
            shift_edges: 1,
            ..Default::default()
        }
    );
    assert_eq!(serial.read_sb(), 0x03);
    assert_eq!(
        serial.transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 1 }
    );
    assert!(context.interrupt_requests().is_empty());
}

#[test]
fn slave_mode_discards_external_clock_pulses_queued_before_transfer_start() {
    let mut serial = Serial::new(ConsoleModel::GameBoy);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    assert!(!serial.queue_external_clock_pulse());

    serial.write_sb(0x81);
    serial.write_sc(0x80);
    serial.tick_t_cycle(&mut context);

    assert_eq!(serial.read_sb(), 0x81);
    assert_eq!(
        serial.transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 0 }
    );

    assert!(serial.queue_external_clock_pulse());
    serial.tick_t_cycle(&mut context);

    assert_eq!(serial.read_sb(), 0x03);
    assert_eq!(
        serial.transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 1 }
    );
}

#[test]
fn loopback_peer_returns_the_original_byte_after_eight_shifts() {
    let mut serial = Serial::new(ConsoleModel::GameBoy);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    serial.set_peer(SerialPeer::Loopback);
    serial.write_sb(0x96);
    serial.write_sc(0x81);

    for _ in 0..(8 * 512) {
        serial.tick_t_cycle(&mut context);
    }

    assert_eq!(serial.read_sb(), 0x96);
    assert_eq!(serial.transfer_state(), SerialTransferState::Idle);
    assert_eq!(context.interrupt_requests(), &[InterruptSource::Serial]);
    assert_eq!(serial.take_completed_output_bytes(), vec![0x96]);
}

#[test]
fn staged_incoming_byte_is_shifted_in_bit_by_bit_across_the_transfer() {
    let mut serial = Serial::new(ConsoleModel::GameBoy);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    serial.set_peer(SerialPeer::StagedIncomingByte { byte: 0x81 });
    serial.write_sb(0x00);
    serial.write_sc(0x81);

    for _ in 0..(8 * 512) {
        serial.tick_t_cycle(&mut context);
    }

    assert_eq!(serial.read_sb(), 0x81);
    assert_eq!(serial.transfer_state(), SerialTransferState::Idle);
    assert_eq!(context.interrupt_requests(), &[InterruptSource::Serial]);
    assert_eq!(serial.take_completed_output_bytes(), vec![0x00]);
}

#[test]
fn internal_clock_phase_stays_aligned_to_the_free_running_counter_when_transfer_starts() {
    let mut serial = Serial::new(ConsoleModel::GameBoy);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    serial.apply_startup_state(
        SerialStartupState::from_registers(0x80, 0x7E).with_clock_counter(0x01FC),
    );
    serial.write_sc(0x81);

    for _ in 0..3 {
        serial.tick_t_cycle(&mut context);
        assert_eq!(
            serial.transfer_state(),
            SerialTransferState::TransferRequested { bits_shifted: 0 }
        );
    }

    serial.tick_t_cycle(&mut context);

    assert_eq!(serial.read_sb(), 0x01);
    assert_eq!(
        serial.transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 1 }
    );
}

#[test]
fn internal_transfer_reports_non_edge_and_edge_ticks_without_changing_edge_timing() {
    let mut serial = Serial::new(ConsoleModel::GameBoy);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    serial.apply_startup_state(
        SerialStartupState::from_registers(0x80, 0x7E).with_clock_counter(0x01FC),
    );
    serial.write_sc(0x81);

    for _ in 0..3 {
        assert_eq!(
            serial.tick_t_cycle(&mut context),
            SerialTickTelemetry {
                active_t_cycles: 1,
                internal_ticks: 1,
                ..Default::default()
            }
        );
        assert_eq!(
            serial.transfer_state(),
            SerialTransferState::TransferRequested { bits_shifted: 0 }
        );
    }

    assert_eq!(
        serial.tick_t_cycle(&mut context),
        SerialTickTelemetry {
            active_t_cycles: 1,
            internal_ticks: 1,
            shift_edges: 1,
            ..Default::default()
        }
    );
    assert_eq!(serial.read_sb(), 0x01);
    assert_eq!(
        serial.transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 1 }
    );
}

#[test]
fn cgb_double_speed_internal_clock_uses_the_faster_edge_bit() {
    let mut serial = Serial::new(ConsoleModel::GameBoyColor);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    serial.apply_startup_state(
        SerialStartupState::from_registers(0x80, 0x7E).with_clock_counter(0x00FC),
    );
    serial.write_sc(0x81);

    for _ in 0..3 {
        serial.tick_t_cycle_for_speed(&mut context, crate::speed::CgbSpeedMode::Double);
        assert_eq!(
            serial.transfer_state(),
            SerialTransferState::TransferRequested { bits_shifted: 0 }
        );
    }

    serial.tick_t_cycle_for_speed(&mut context, crate::speed::CgbSpeedMode::Double);

    assert_eq!(serial.read_sb(), 0x01);
    assert_eq!(
        serial.transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 1 }
    );
}

#[test]
fn cgb_sc1_high_speed_internal_clock_uses_fast_edge_bits() {
    let mut normal =
        Serial::new_with_operating_mode(ConsoleModel::GameBoyColor, OperatingMode::Cgb);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    normal.write_sb(0x80);
    normal.write_sc(0x83);

    for _ in 0..15 {
        normal.tick_t_cycle_for_speed(&mut context, crate::speed::CgbSpeedMode::Normal);
        assert_eq!(
            normal.transfer_state(),
            SerialTransferState::TransferRequested { bits_shifted: 0 }
        );
    }

    normal.tick_t_cycle_for_speed(&mut context, crate::speed::CgbSpeedMode::Normal);
    assert_eq!(normal.read_sb(), 0x01);
    assert_eq!(
        normal.transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 1 }
    );

    let mut double =
        Serial::new_with_operating_mode(ConsoleModel::GameBoyColor, OperatingMode::Cgb);
    double.write_sb(0x80);
    double.write_sc(0x83);

    for _ in 0..7 {
        double.tick_t_cycle_for_speed(&mut context, crate::speed::CgbSpeedMode::Double);
        assert_eq!(
            double.transfer_state(),
            SerialTransferState::TransferRequested { bits_shifted: 0 }
        );
    }

    double.tick_t_cycle_for_speed(&mut context, crate::speed::CgbSpeedMode::Double);
    assert_eq!(double.read_sb(), 0x01);
    assert_eq!(
        double.transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 1 }
    );
}

#[test]
fn completed_byte_is_reported_for_one_tick_and_then_cleared_without_losing_history() {
    let mut serial = Serial::new(ConsoleModel::GameBoy);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    serial.write_sb(0x81);
    serial.write_sc(0x81);

    for _ in 0..(8 * 512 - 1) {
        let telemetry = serial.tick_t_cycle(&mut context);
        assert_eq!(telemetry.completed_bytes, 0);
    }

    let completion = serial.tick_t_cycle(&mut context);
    assert_eq!(
        completion,
        SerialTickTelemetry {
            active_t_cycles: 1,
            internal_ticks: 1,
            shift_edges: 1,
            completed_bytes: 1,
            ..Default::default()
        }
    );
    assert_eq!(serial.latest_completed_output_byte(), Some(0x81));

    let clear = serial.tick_t_cycle(&mut context);
    assert_eq!(clear, SerialTickTelemetry::default());
    assert_eq!(serial.latest_completed_output_byte(), None);
    assert_eq!(serial.take_completed_output_bytes(), vec![0x81]);
}

#[test]
fn transfer_reuses_the_last_staged_outgoing_byte_until_sb_is_rewritten() {
    let mut serial = Serial::new(ConsoleModel::GameBoy);
    let mut context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);

    serial.set_peer(SerialPeer::StagedIncomingByte { byte: 0x12 });
    serial.write_sb(0xA5);
    serial.write_sc(0x81);

    for _ in 0..(8 * 512) {
        serial.tick_t_cycle(&mut context);
    }

    assert_eq!(serial.read_sb(), 0x12);
    assert_eq!(serial.take_completed_output_bytes(), vec![0xA5]);

    serial.set_peer(SerialPeer::StagedIncomingByte { byte: 0x34 });
    serial.write_sc(0x81);

    for _ in 0..(8 * 512) {
        serial.tick_t_cycle(&mut context);
    }

    assert_eq!(serial.read_sb(), 0x34);
    assert_eq!(serial.take_completed_output_bytes(), vec![0xA5]);
}
