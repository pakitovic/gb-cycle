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
