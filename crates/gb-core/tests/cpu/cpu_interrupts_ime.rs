use super::*;

#[test]
fn pending_irq_stays_visible_when_ime_is_disabled() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x01);
    machine.write_bus(0xFF0F, 0x01);
    step_machine_t_cycles(&mut machine, 4);

    assert_eq!(machine.read_bus(0xFF0F), 0xE1);
    assert_eq!(machine.read_bus(0xFFFF), 0x01);
    assert!(!machine.cpu().ime());
    assert_eq!(machine.cpu().registers().pc, 0x0101);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn machine_accepts_the_highest_priority_pending_irq_after_ei_nop() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0xFB, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x05);
    machine.write_bus(0xFF0F, 0x05);

    step_machine_t_cycles(&mut machine, 4);

    assert!(!machine.cpu().ime());
    assert!(machine.cpu().delayed_ime_enable());
    assert_eq!(machine.read_bus(0xFF0F), 0xE5);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    step_machine_t_cycles(&mut machine, 4);

    assert!(!machine.cpu().ime());
    assert!(!machine.cpu().delayed_ime_enable());
    assert_eq!(machine.read_bus(0xFF0F), 0xE4);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: gb_core::InterruptSource::VBlank,
            step: 0,
            t_cycle: 0,
        }
    );

    step_machine_t_cycles(&mut machine, 20);

    assert_eq!(machine.cpu().registers().pc, 0x0040);
    assert_eq!(machine.cpu().registers().sp, 0xFFFC);
    assert_eq!(machine.read_bus(0xFFFD), 0x01);
    assert_eq!(machine.read_bus(0xFFFC), 0x02);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn machine_accepts_a_pending_irq_after_ei_followed_by_ei() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0xFB, 0xFB, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x08);
    machine.write_bus(0xFF0F, 0x08);

    step_machine_t_cycles(&mut machine, 4);

    assert!(!machine.cpu().ime());
    assert!(machine.cpu().delayed_ime_enable());
    assert_eq!(machine.read_bus(0xFF0F), 0xE8);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    step_machine_t_cycles(&mut machine, 4);

    assert!(!machine.cpu().ime());
    assert!(!machine.cpu().delayed_ime_enable());
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: gb_core::InterruptSource::Serial,
            step: 0,
            t_cycle: 0,
        }
    );

    step_machine_t_cycles(&mut machine, 20);

    assert_eq!(machine.cpu().registers().pc, 0x0058);
    assert_eq!(machine.cpu().registers().sp, 0xFFFC);
    assert_eq!(machine.read_bus(0xFFFD), 0x01);
    assert_eq!(machine.read_bus(0xFFFC), 0x02);
}

#[test]
fn machine_keeps_interrupts_blocked_for_ei_di() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0xFB, 0xF3, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x01);
    machine.write_bus(0xFF0F, 0x01);

    step_machine_t_cycles(&mut machine, 8);

    assert!(!machine.cpu().ime());
    assert!(!machine.cpu().delayed_ime_enable());
    assert_eq!(machine.read_bus(0xFF0F), 0xE1);
    assert_eq!(machine.cpu().registers().pc, 0x0102);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    step_machine_t_cycles(&mut machine, 4);

    assert_eq!(machine.cpu().registers().pc, 0x0103);
    assert_eq!(machine.read_bus(0xFF0F), 0xE1);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn reti_reenables_interrupts_and_allows_a_remaining_pending_source_to_service() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom_with_patches(
            &[0xFB, 0x00, 0x00],
            0x12,
            &[(0x0040, 0xD9)],
        ))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x03);
    machine.write_bus(0xFF0F, 0x03);

    step_machine_t_cycles(&mut machine, 8);

    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: gb_core::InterruptSource::VBlank,
            step: 0,
            t_cycle: 0,
        }
    );
    assert_eq!(machine.read_bus(0xFF0F), 0xE2);

    step_machine_t_cycles(&mut machine, 20);

    assert_eq!(machine.cpu().registers().pc, 0x0040);
    assert_eq!(machine.read_bus(0xFF0F), 0xE2);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    step_machine_t_cycles(&mut machine, 16);

    assert_eq!(machine.cpu().registers().pc, 0x0102);
    assert!(!machine.cpu().ime());
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: gb_core::InterruptSource::LcdStat,
            step: 0,
            t_cycle: 0,
        }
    );
}

#[test]
fn interrupt_service_cancels_when_the_high_pc_byte_push_disables_the_last_pending_interrupt() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom_with_patches(
            &[0x31, 0x00, 0x00, 0xC3, 0x00, 0x02],
            0x12,
            &[(0x0200, 0xFB), (0x0201, 0x00), (0x0202, 0x00)],
        ))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x04);
    machine.write_bus(0xFF0F, 0x04);

    step_machine_t_cycles(&mut machine, 36);

    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: gb_core::InterruptSource::Timer,
            step: 0,
            t_cycle: 0,
        }
    );
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);
    assert_eq!(machine.read_bus(0xFFFF), 0x04);

    step_machine_t_cycles(&mut machine, 16);

    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(machine.cpu().registers().pc, 0x0000);
    assert_eq!(machine.cpu().registers().sp, 0xFFFF);
    assert_eq!(machine.read_bus(0xFFFF), 0x02);
    assert_eq!(machine.read_bus(0xFF0F), 0xE4);
}

#[test]
fn interrupt_service_retargets_when_the_high_pc_byte_push_changes_ie_priority() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom_with_patches(
            &[0x31, 0x00, 0x00, 0xC3, 0x00, 0x02],
            0x12,
            &[(0x0200, 0xFB), (0x0201, 0x00), (0x0202, 0x00)],
        ))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x03);
    machine.write_bus(0xFF0F, 0x03);

    step_machine_t_cycles(&mut machine, 36);

    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: gb_core::InterruptSource::VBlank,
            step: 0,
            t_cycle: 0,
        }
    );
    assert_eq!(machine.read_bus(0xFF0F), 0xE2);
    assert_eq!(machine.read_bus(0xFFFF), 0x03);

    step_machine_t_cycles(&mut machine, 16);

    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: gb_core::InterruptSource::LcdStat,
            step: 4,
            t_cycle: 0,
        }
    );
    assert_eq!(machine.read_bus(0xFFFF), 0x02);
    assert_eq!(machine.read_bus(0xFF0F), 0xE1);

    step_machine_t_cycles(&mut machine, 4);

    assert_eq!(machine.cpu().registers().pc, 0x0048);
    assert_eq!(machine.cpu().registers().sp, 0xFFFE);
    assert_eq!(machine.read_bus(0xFFFF), 0x02);
    assert_eq!(machine.read_bus(0xFF0F), 0xE1);
}

#[test]
fn interrupt_service_does_not_cancel_when_the_low_pc_byte_push_disables_ie_too_late() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom_with_patches(
            &[0x31, 0x01, 0x00, 0xC3, 0x35, 0x02],
            0x12,
            &[(0x0235, 0xFB), (0x0236, 0x00), (0x0237, 0x00)],
        ))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x08);
    machine.write_bus(0xFF0F, 0x08);

    step_machine_t_cycles(&mut machine, 36);

    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: gb_core::InterruptSource::Serial,
            step: 0,
            t_cycle: 0,
        }
    );
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);
    assert_eq!(machine.read_bus(0xFFFF), 0x08);

    step_machine_t_cycles(&mut machine, 16);

    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: gb_core::InterruptSource::Serial,
            step: 4,
            t_cycle: 0,
        }
    );
    assert_eq!(machine.read_bus(0xFFFF), 0x08);
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);

    step_machine_t_cycles(&mut machine, 4);

    assert_eq!(machine.cpu().registers().pc, 0x0058);
    assert_eq!(machine.cpu().registers().sp, 0xFFFF);
    assert_eq!(machine.read_bus(0xFFFF), 0x37);
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);
}
