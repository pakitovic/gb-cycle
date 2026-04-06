use super::*;

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
    cpu.ime = true;
    interrupts.write_ie(0x01);
    interrupts.write_if(0x01);

    cpu.evaluate_wake_and_interrupts(&mut interrupts, &mut joypad);

    assert!(!cpu.ime());
    assert_eq!(interrupts.read_if(), 0xE1);
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
    cpu.ime = true;
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
    cpu.ime = true;
    interrupts.write_ie(0x05);
    interrupts.write_if(0x04);

    cpu.evaluate_wake_and_interrupts(&mut interrupts, &mut joypad);

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
    cpu.ime = true;
    cpu.finish_and_request_halt();
    interrupts.write_ie(0x01);
    interrupts.write_if(0x01);

    cpu.evaluate_wake_and_interrupts(&mut interrupts, &mut joypad);

    assert_eq!(interrupts.read_if(), 0xE1);
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
    assert_eq!(interrupts.read_if(), 0xE1);
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
