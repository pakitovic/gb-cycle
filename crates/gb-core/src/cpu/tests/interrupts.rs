use super::*;
use crate::joypad::JoypadButton;

#[test]
fn interrupt_service_uses_a_five_machine_cycle_sequence_and_pushes_pc_bytewise() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[]);
    let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0150,
        sp: 0xFFFE,
        ..CpuStartupState::power_on_reset()
    });
    cpu.set_ime_enabled();
    interrupts.write_ie(0x01);
    interrupts.write_if(0x01);

    cpu.evaluate_wake_and_interrupts(&mut interrupts, &mut joypad);

    assert!(!cpu.ime());
    assert_eq!(interrupts.read_if(), 0xE0);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: InterruptSource::VBlank,
            step: 0,
            t_cycle: 0,
        }
    );

    tick_cpu_n_with_interrupts(&mut cpu, &mut bus, &mut cartridge, &mut interrupts, 4);

    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: InterruptSource::VBlank,
            step: 1,
            t_cycle: 0,
        }
    );
    assert_eq!(cpu.registers().sp, 0xFFFE);

    tick_cpu_n_with_interrupts(&mut cpu, &mut bus, &mut cartridge, &mut interrupts, 4);

    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: InterruptSource::VBlank,
            step: 2,
            t_cycle: 0,
        }
    );
    assert_eq!(cpu.registers().sp, 0xFFFE);

    tick_cpu_n_with_interrupts(&mut cpu, &mut bus, &mut cartridge, &mut interrupts, 4);

    assert_eq!(cpu.registers().sp, 0xFFFD);
    assert_eq!(bus.read(0xFFFD), 0x00);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: InterruptSource::VBlank,
            step: 3,
            t_cycle: 0,
        }
    );
    assert_eq!(
        cpu.last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::IncDec,
            access_address: None,
            idu_address: Some(0xFFFD),
            update_direction: Some(CpuAddressUpdateDirection::Decrement),
        })
    );

    tick_cpu_n_with_interrupts(&mut cpu, &mut bus, &mut cartridge, &mut interrupts, 4);

    assert_eq!(cpu.registers().sp, 0xFFFD);
    assert_eq!(bus.read(0xFFFD), 0x01);
    assert_eq!(bus.read(0xFFFC), 0x00);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: InterruptSource::VBlank,
            step: 4,
            t_cycle: 0,
        }
    );
    assert_eq!(
        cpu.last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::Write,
            access_address: Some(0xFFFD),
            idu_address: None,
            update_direction: None,
        })
    );

    tick_cpu_n_with_interrupts(&mut cpu, &mut bus, &mut cartridge, &mut interrupts, 4);

    assert_eq!(cpu.registers().sp, 0xFFFC);
    assert_eq!(bus.read(0xFFFC), 0x50);
    assert_eq!(cpu.registers().pc, 0x0040);
    assert_eq!(interrupts.read_if(), 0xE0);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn pending_interrupt_can_preempt_the_in_flight_fetch_before_the_opcode_latches() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0150,
        ..CpuStartupState::power_on_reset()
    });
    cpu.set_ime_enabled();
    cpu.execution_state = CpuExecutionState::FetchOpcode { t_cycle: 3 };
    interrupts.write_ie(0x04);
    interrupts.write_if(0x04);

    cpu.evaluate_wake_and_interrupts(&mut interrupts, &mut joypad);

    assert!(!cpu.ime());
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: InterruptSource::Timer,
            step: 0,
            t_cycle: 0,
        }
    );
}

#[test]
fn pending_interrupt_does_not_preempt_after_the_opcode_has_latched() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[0xCB, 0x11]);
    let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });
    cpu.set_ime_enabled();

    tick_cpu_n(&mut cpu, &mut bus, &mut cartridge, 4);

    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 0,
            t_cycle: 0,
        }
    );
    assert_eq!(cpu.current_opcode(), Some(0xCB));

    interrupts.write_ie(0x04);
    interrupts.write_if(0x04);

    cpu.evaluate_wake_and_interrupts(&mut interrupts, &mut joypad);

    assert!(cpu.ime());
    assert_eq!(interrupts.read_if(), 0xE4);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 0,
            t_cycle: 0,
        }
    );
    assert_eq!(cpu.current_opcode(), Some(0xCB));
}

#[test]
fn interrupt_service_keeps_the_accepted_source_latched_until_vector_commit() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[]);
    let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0150,
        sp: 0xFFFE,
        ..CpuStartupState::power_on_reset()
    });
    cpu.set_ime_enabled();
    interrupts.write_ie(0x05);
    interrupts.write_if(0x04);

    cpu.evaluate_wake_and_interrupts(&mut interrupts, &mut joypad);

    assert_eq!(interrupts.read_if(), 0xE0);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: InterruptSource::Timer,
            step: 0,
            t_cycle: 0,
        }
    );

    tick_cpu_n_with_interrupts(&mut cpu, &mut bus, &mut cartridge, &mut interrupts, 12);
    interrupts.request(InterruptSource::VBlank);
    tick_cpu_n_with_interrupts(&mut cpu, &mut bus, &mut cartridge, &mut interrupts, 8);

    assert_eq!(cpu.registers().pc, 0x0050);
    assert_eq!(interrupts.read_if(), 0xE1);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn halt_with_a_pending_interrupt_and_ime_enabled_enters_service_without_staying_halted() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0150,
        ..CpuStartupState::power_on_reset()
    });
    cpu.set_ime_enabled();
    cpu.finish_and_request_halt();
    interrupts.write_ie(0x01);
    interrupts.write_if(0x01);

    cpu.evaluate_wake_and_interrupts(&mut interrupts, &mut joypad);

    assert_eq!(interrupts.read_if(), 0xE0);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: InterruptSource::VBlank,
            step: 0,
            t_cycle: 0,
        }
    );
}

#[test]
fn pending_interrupt_wakes_a_halted_cpu_with_ime_disabled_without_entering_service() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[0x00, 0x00]);
    let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });
    cpu.execution_state = CpuExecutionState::Halted;
    cpu.set_ime_disabled();
    interrupts.write_ie(0x04);
    interrupts.write_if(0x04);

    cpu.evaluate_wake_and_interrupts(&mut interrupts, &mut joypad);

    assert!(!cpu.ime());
    assert_eq!(interrupts.read_if(), 0xE4);
    assert_eq!(cpu.registers().pc, 0x0100);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    tick_cpu_n_with_scheduler_interrupt_evaluation(
        &mut cpu,
        &mut bus,
        &mut cartridge,
        &mut interrupts,
        &mut joypad,
        4,
    );

    assert!(!cpu.ime());
    assert_eq!(interrupts.read_if(), 0xE4);
    assert_eq!(cpu.registers().pc, 0x0101);
    assert_eq!(cpu.current_opcode(), None);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(
        cpu.last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::ReadWithIncDec,
            access_address: Some(0x0100),
            idu_address: Some(0x0101),
            update_direction: Some(CpuAddressUpdateDirection::Increment),
        })
    );
}

#[test]
fn ei_halt_with_a_pending_interrupt_services_and_preserves_the_halt_return_address() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0151,
        ..CpuStartupState::power_on_reset()
    });
    cpu.schedule_delayed_ime_enable();
    cpu.advance_delayed_ime_enable();
    cpu.finish_and_request_halt();
    interrupts.write_ie(0x01);
    interrupts.write_if(0x01);

    cpu.evaluate_wake_and_interrupts(&mut interrupts, &mut joypad);

    assert!(!cpu.ime());
    assert_eq!(interrupts.read_if(), 0xE0);
    assert_eq!(cpu.registers().pc, 0x0150);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: InterruptSource::VBlank,
            step: 0,
            t_cycle: 0,
        }
    );
}

#[test]
fn halt_bug_turns_the_next_fetch_into_a_non_incrementing_pc_read() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[0x76, 0x00, 0x00]);
    let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });
    cpu.set_ime_disabled();
    interrupts.write_ie(0x01);
    interrupts.write_if(0x01);

    tick_cpu_n_with_scheduler_interrupt_evaluation(
        &mut cpu,
        &mut bus,
        &mut cartridge,
        &mut interrupts,
        &mut joypad,
        4,
    );

    assert_eq!(cpu.registers().pc, 0x0101);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(interrupts.read_if(), 0xE1);

    tick_cpu_n_with_scheduler_interrupt_evaluation(
        &mut cpu,
        &mut bus,
        &mut cartridge,
        &mut interrupts,
        &mut joypad,
        4,
    );

    assert_eq!(cpu.registers().pc, 0x0101);
    assert_eq!(interrupts.read_if(), 0xE1);
    assert_eq!(
        cpu.last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::Read,
            access_address: Some(0x0101),
            idu_address: None,
            update_direction: None,
        })
    );
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    tick_cpu_n_with_scheduler_interrupt_evaluation(
        &mut cpu,
        &mut bus,
        &mut cartridge,
        &mut interrupts,
        &mut joypad,
        4,
    );

    assert_eq!(cpu.registers().pc, 0x0102);
    assert_eq!(
        cpu.last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::ReadWithIncDec,
            access_address: Some(0x0101),
            idu_address: Some(0x0102),
            update_direction: Some(CpuAddressUpdateDirection::Increment),
        })
    );
    assert_eq!(interrupts.read_if(), 0xE1);
}

#[test]
fn ei_then_nop_accepts_a_pending_interrupt_before_the_third_opcode_starts() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[0xFB, 0x00, 0x00]);
    let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });
    interrupts.write_ie(0x01);
    interrupts.write_if(0x01);

    tick_cpu_n_with_scheduler_interrupt_evaluation(
        &mut cpu,
        &mut bus,
        &mut cartridge,
        &mut interrupts,
        &mut joypad,
        4,
    );

    assert!(!cpu.ime());
    assert!(cpu.delayed_ime_enable());
    assert_eq!(interrupts.read_if(), 0xE1);
    assert_eq!(cpu.registers().pc, 0x0101);

    tick_cpu_n_with_scheduler_interrupt_evaluation(
        &mut cpu,
        &mut bus,
        &mut cartridge,
        &mut interrupts,
        &mut joypad,
        4,
    );

    assert!(!cpu.ime());
    assert_eq!(interrupts.read_if(), 0xE0);
    assert_eq!(cpu.registers().pc, 0x0102);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: InterruptSource::VBlank,
            step: 0,
            t_cycle: 0,
        }
    );
}

#[test]
fn ei_then_di_does_not_leave_an_interrupt_acceptance_window() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[0xFB, 0xF3, 0x00]);
    let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });
    interrupts.write_ie(0x01);
    interrupts.write_if(0x01);

    tick_cpu_n_with_scheduler_interrupt_evaluation(
        &mut cpu,
        &mut bus,
        &mut cartridge,
        &mut interrupts,
        &mut joypad,
        8,
    );

    assert!(!cpu.ime());
    assert!(!cpu.delayed_ime_enable());
    assert_eq!(interrupts.read_if(), 0xE1);
    assert_eq!(cpu.registers().pc, 0x0102);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    tick_cpu_n_with_scheduler_interrupt_evaluation(
        &mut cpu,
        &mut bus,
        &mut cartridge,
        &mut interrupts,
        &mut joypad,
        4,
    );

    assert!(!cpu.ime());
    assert_eq!(interrupts.read_if(), 0xE1);
    assert_eq!(cpu.registers().pc, 0x0103);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn di_ei_nop_accepts_a_pending_interrupt_after_the_following_instruction() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[0xF3, 0xFB, 0x00, 0x00]);
    let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });
    cpu.set_ime_enabled();
    interrupts.write_ie(0x01);

    tick_cpu_n_with_scheduler_interrupt_evaluation(
        &mut cpu,
        &mut bus,
        &mut cartridge,
        &mut interrupts,
        &mut joypad,
        4,
    );

    assert!(!cpu.ime());
    assert!(!cpu.delayed_ime_enable());
    assert_eq!(interrupts.read_if(), 0xE0);
    assert_eq!(cpu.registers().pc, 0x0101);

    interrupts.write_if(0x01);

    tick_cpu_n_with_scheduler_interrupt_evaluation(
        &mut cpu,
        &mut bus,
        &mut cartridge,
        &mut interrupts,
        &mut joypad,
        8,
    );

    assert!(!cpu.ime());
    assert_eq!(interrupts.read_if(), 0xE0);
    assert_eq!(cpu.registers().pc, 0x0103);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: InterruptSource::VBlank,
            step: 0,
            t_cycle: 0,
        }
    );
}

#[test]
fn chained_ei_still_accepts_a_pending_interrupt_after_the_second_instruction() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[0xFB, 0xFB, 0x00]);
    let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        ..CpuStartupState::power_on_reset()
    });
    interrupts.write_ie(0x01);
    interrupts.write_if(0x01);

    tick_cpu_n_with_scheduler_interrupt_evaluation(
        &mut cpu,
        &mut bus,
        &mut cartridge,
        &mut interrupts,
        &mut joypad,
        4,
    );

    assert!(!cpu.ime());
    assert!(cpu.delayed_ime_enable());
    assert_eq!(interrupts.read_if(), 0xE1);
    assert_eq!(cpu.registers().pc, 0x0101);

    tick_cpu_n_with_scheduler_interrupt_evaluation(
        &mut cpu,
        &mut bus,
        &mut cartridge,
        &mut interrupts,
        &mut joypad,
        4,
    );

    assert!(!cpu.ime());
    assert_eq!(interrupts.read_if(), 0xE0);
    assert_eq!(cpu.registers().pc, 0x0102);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: InterruptSource::VBlank,
            step: 0,
            t_cycle: 0,
        }
    );
}

#[test]
fn interrupt_rerequest_from_the_same_source_remains_pending_after_service() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[]);
    let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0150,
        sp: 0xFFFE,
        ..CpuStartupState::power_on_reset()
    });
    cpu.set_ime_enabled();
    interrupts.write_ie(0x04);
    interrupts.write_if(0x04);

    cpu.evaluate_wake_and_interrupts(&mut interrupts, &mut joypad);

    assert_eq!(interrupts.read_if(), 0xE0);
    tick_cpu_n_with_interrupts(&mut cpu, &mut bus, &mut cartridge, &mut interrupts, 12);
    interrupts.request(InterruptSource::Timer);
    tick_cpu_n_with_interrupts(&mut cpu, &mut bus, &mut cartridge, &mut interrupts, 8);

    assert_eq!(cpu.registers().pc, 0x0050);
    assert_eq!(interrupts.read_if(), 0xE4);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn reti_restores_pc_reenables_ime_and_allows_immediate_interrupt_acceptance() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[0xD9]);
    let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0100,
        sp: 0xFFFC,
        ..CpuStartupState::power_on_reset()
    });
    bus.write(0xFFFC, 0x34);
    bus.write(0xFFFD, 0x12);
    interrupts.write_ie(0x04);
    interrupts.write_if(0x04);

    tick_cpu_n_with_interrupts(&mut cpu, &mut bus, &mut cartridge, &mut interrupts, 12);

    assert!(!cpu.ime());
    assert_eq!(interrupts.read_if(), 0xE4);
    assert_eq!(cpu.registers().pc, 0x0101);
    assert_eq!(cpu.registers().sp, 0xFFFE);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::Execute {
            step: 2,
            t_cycle: 0,
        }
    );

    tick_cpu_n_with_interrupts(&mut cpu, &mut bus, &mut cartridge, &mut interrupts, 4);

    assert!(cpu.ime());
    assert!(!cpu.delayed_ime_enable());
    assert_eq!(interrupts.read_if(), 0xE4);
    assert_eq!(cpu.registers().pc, 0x1234);
    assert_eq!(cpu.registers().sp, 0xFFFE);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    cpu.evaluate_wake_and_interrupts(&mut interrupts, &mut joypad);

    assert!(!cpu.ime());
    assert_eq!(interrupts.read_if(), 0xE0);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: InterruptSource::Timer,
            step: 0,
            t_cycle: 0,
        }
    );
}

#[test]
fn upper_pc_push_into_ie_can_cancel_service_and_restore_the_accepted_if_bit() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[]);
    let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0200,
        sp: 0x0000,
        ..CpuStartupState::power_on_reset()
    });
    cpu.set_ime_enabled();
    interrupts.write_ie(0x04);
    interrupts.write_if(0x04);

    cpu.evaluate_wake_and_interrupts(&mut interrupts, &mut joypad);

    assert_eq!(interrupts.read_if(), 0xE0);
    tick_cpu_n_with_interrupts(&mut cpu, &mut bus, &mut cartridge, &mut interrupts, 16);

    assert_eq!(cpu.registers().pc, 0x0000);
    assert_eq!(interrupts.read_if(), 0xE4);
    assert!(!cpu.ime());
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn upper_pc_push_into_ie_can_retarget_service_and_restore_the_unserved_if_bit() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[]);
    let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0200,
        sp: 0x0000,
        ..CpuStartupState::power_on_reset()
    });
    cpu.set_ime_enabled();
    interrupts.write_ie(0x03);
    interrupts.write_if(0x03);

    cpu.evaluate_wake_and_interrupts(&mut interrupts, &mut joypad);

    assert_eq!(interrupts.read_if(), 0xE2);
    tick_cpu_n_with_interrupts(&mut cpu, &mut bus, &mut cartridge, &mut interrupts, 20);

    assert_eq!(cpu.registers().pc, 0x0048);
    assert_eq!(interrupts.read_if(), 0xE1);
    assert!(!cpu.ime());
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn stop_wake_with_ime_enabled_and_a_pending_joypad_irq_enters_bugged_interrupt_service() {
    let mut cpu = CpuCore::new(ConsoleModel::Dmg);
    let mut bus = Bus::new(ConsoleModel::Dmg);
    let mut cartridge = build_test_cartridge(&[]);
    let mut interrupts = InterruptController::new(ConsoleModel::Dmg);
    let mut joypad = Joypad::new(ConsoleModel::Dmg);

    cpu.apply_startup_state(CpuStartupState {
        pc: 0x0104,
        sp: 0xFFFE,
        ..CpuStartupState::power_on_reset()
    });
    cpu.set_ime_enabled();
    cpu.execution_state = CpuExecutionState::Stopped;
    interrupts.write_ie(0x10);
    interrupts.write_if(0x10);
    joypad.write_p1(0x10);
    joypad.set_button_pressed(JoypadButton::A, true);

    cpu.evaluate_wake_and_interrupts(&mut interrupts, &mut joypad);

    assert!(!cpu.ime());
    assert_eq!(interrupts.read_if(), 0xE0);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::ServiceStopWakeBuggedInterrupt {
            step: 0,
            t_cycle: 0,
        }
    );

    tick_cpu_n_with_interrupts(&mut cpu, &mut bus, &mut cartridge, &mut interrupts, 20);

    assert_eq!(cpu.registers().pc, 0x0000);
    assert_eq!(cpu.registers().sp, 0xFFFD);
    assert_eq!(bus.read(0xFFFD), 0x04);
    assert_eq!(
        cpu.execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}
