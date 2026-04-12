use super::*;

#[test]
fn stop_does_not_wake_when_no_joyp_rows_are_selected() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x10, 0x00, 0xFB, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF0F, 0x00);
    machine.write_bus(0xFF00, 0x30);
    step_machine_t_cycles(&mut machine, 8);

    assert_eq!(machine.cpu().registers().pc, 0x0102);
    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Stopped);

    machine.set_joypad_button_pressed(JoypadButton::A, true);
    step_machine_t_cycles(&mut machine, 8);

    assert_eq!(machine.read_bus(0xFF0F), 0xE0);
    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Stopped);
    assert_eq!(machine.cpu().registers().pc, 0x0102);
}

#[test]
fn stop_resets_div_and_keeps_it_frozen_until_a_later_wake_event() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x10, 0x00, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF00, 0x10);
    assert_eq!(machine.read_bus(0xFF04), 0xAB);

    step_machine_t_cycles(&mut machine, 8);

    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Stopped);
    assert_eq!(machine.cpu().registers().pc, 0x0102);
    assert_eq!(machine.read_bus(0xFF04), 0x00);

    step_machine_t_cycles(&mut machine, 64);

    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Stopped);
    assert_eq!(machine.read_bus(0xFF04), 0x00);

    machine.set_joypad_button_pressed(JoypadButton::A, true);
    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(machine.read_bus(0xFF04), 0x00);

    step_machine_t_cycles(&mut machine, 256);

    assert_eq!(machine.read_bus(0xFF04), 0x01);
}

#[test]
fn stop_drops_external_serial_clocks_instead_of_replaying_them_after_wake() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x10, 0x00, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF00, 0x10);
    machine.write_bus(0xFF01, 0x81);
    machine.write_bus(0xFF02, 0x80);

    step_machine_t_cycles(&mut machine, 8);

    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Stopped);
    assert_eq!(machine.read_bus(0xFF01), 0x81);
    assert_eq!(
        machine.serial().transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 0 }
    );

    machine.queue_external_serial_clock();
    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Stopped);
    assert_eq!(machine.read_bus(0xFF01), 0x81);
    assert_eq!(
        machine.serial().transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 0 }
    );

    machine.set_joypad_button_pressed(JoypadButton::A, true);
    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(machine.read_bus(0xFF01), 0x81);
    assert_eq!(
        machine.serial().transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 0 }
    );

    machine.queue_external_serial_clock();
    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(machine.read_bus(0xFF01), 0x03);
    assert_eq!(
        machine.serial().transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 1 }
    );
}

#[test]
fn stop_wakes_from_the_selected_joypad_line_and_services_irq_later() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x10, 0x00, 0xFB, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x01);
    machine.write_bus(0xFF0F, 0x00);
    machine.write_bus(0xFF00, 0x10);
    step_machine_t_cycles(&mut machine, 8);

    assert_eq!(machine.cpu().registers().pc, 0x0102);
    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Stopped);

    machine.set_joypad_button_pressed(JoypadButton::A, true);
    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(machine.read_bus(0xFF0F), 0xF0);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(machine.cpu().registers().pc, 0x0102);

    step_machine_t_cycles(&mut machine, 4);

    assert!(machine.cpu().delayed_ime_enable());
    assert!(!machine.cpu().ime());
    assert_eq!(machine.cpu().registers().pc, 0x0103);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    machine.write_bus(0xFF0F, 0x01);
    step_machine_t_cycles(&mut machine, 4);

    assert_eq!(machine.read_bus(0xFF0F), 0xE0);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: gb_core::InterruptSource::VBlank,
            step: 0,
            t_cycle: 0,
        }
    );
}

#[test]
fn stop_with_ime_disabled_and_a_pending_interrupt_enters_zombie_mode_as_a_one_byte_stop() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0xF3, 0x10, 0x04, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF00, 0x10);
    machine.write_bus(0xFFFF, 0x01);
    machine.write_bus(0xFF0F, 0x01);
    step_machine_t_cycles(&mut machine, 12);

    assert!(!machine.cpu().ime());
    assert_eq!(machine.cpu().registers().pc, 0x0102);
    assert_eq!(machine.read_bus(0xFF0F), 0xE1);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::ZombieStopped
    );

    step_machine_t_cycles(&mut machine, 16);

    assert_eq!(machine.read_bus(0xFF0F), 0xE1);
    assert_eq!(machine.cpu().registers().pc, 0x0102);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::ZombieStopped
    );
    machine.set_joypad_button_pressed(JoypadButton::A, true);
    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(machine.cpu().registers().pc, 0x0102);
    assert_eq!(machine.read_bus(0xFF0F), 0xF1);

    step_machine_t_cycles(&mut machine, 8);

    assert_eq!(machine.cpu().registers().b, 0x01);
    assert_eq!(machine.cpu().registers().pc, 0x0104);
    assert_eq!(machine.read_bus(0xFF0F), 0xF1);
}

#[test]
fn stop_with_ime_disabled_and_a_selected_held_button_behaves_like_halt_with_two_byte_visibility() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0xF3, 0x10, 0x00, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF00, 0x10);
    machine.set_joypad_button_pressed(JoypadButton::A, true);

    for _ in 0..16 {
        machine.step_t_cycle();
        if matches!(machine.cpu().execution_state(), CpuExecutionState::Halted) {
            break;
        }
    }

    assert!(!machine.cpu().ime());
    assert_eq!(machine.cpu().registers().pc, 0x0103);
    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Halted);
}

#[test]
fn stop_with_ime_disabled_and_a_selected_held_button_plus_pending_interrupt_behaves_like_a_one_byte_nop()
 {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0xF3, 0x10, 0x04, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF00, 0x10);
    machine.set_joypad_button_pressed(JoypadButton::A, true);
    machine.write_bus(0xFFFF, 0x01);
    machine.write_bus(0xFF0F, 0x01);
    step_machine_t_cycles(&mut machine, 8);

    assert!(!machine.cpu().ime());
    assert_eq!(machine.cpu().registers().pc, 0x0102);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    step_machine_t_cycles(&mut machine, 4);

    assert_eq!(machine.cpu().registers().b, 0x01);
    assert_eq!(machine.cpu().registers().pc, 0x0103);
    assert_eq!(machine.read_bus(0xFF0F), 0xF1);
}

#[test]
fn stop_with_ime_enabled_and_a_selected_held_button_behaves_like_a_one_byte_nop() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0xFB, 0x00, 0x10, 0x04, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF00, 0x10);
    machine.set_joypad_button_pressed(JoypadButton::A, true);
    step_machine_t_cycles(&mut machine, 12);

    assert!(machine.cpu().ime());
    assert_eq!(machine.cpu().registers().pc, 0x0103);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    step_machine_t_cycles(&mut machine, 4);

    assert_eq!(machine.cpu().registers().b, 0x01);
    assert_eq!(machine.cpu().registers().pc, 0x0104);
}

#[test]
fn stop_nop_like_entry_still_resets_div_before_running_immediately_again() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0xFB, 0x00, 0x10, 0x04, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF00, 0x10);
    machine.set_joypad_button_pressed(JoypadButton::A, true);
    assert_eq!(machine.read_bus(0xFF04), 0xAB);

    step_machine_t_cycles(&mut machine, 12);

    assert!(machine.cpu().ime());
    assert_eq!(machine.cpu().registers().pc, 0x0103);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(machine.read_bus(0xFF04), 0x00);

    step_machine_t_cycles(&mut machine, 256);

    assert_eq!(machine.read_bus(0xFF04), 0x01);
}

#[test]
fn stop_wake_with_ime_enabled_takes_the_bugged_0x0000_isr_and_corrupts_the_return_stack() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0xFB, 0x00, 0x10, 0x00, 0x00], 0xD9))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x10);
    machine.write_bus(0xFF0F, 0x00);
    machine.write_bus(0xFF00, 0x10);
    machine.write_bus(0xFFFE, 0xAA);
    step_machine_t_cycles(&mut machine, 16);

    assert_eq!(machine.cpu().registers().pc, 0x0104);
    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Stopped);

    machine.set_joypad_button_pressed(JoypadButton::A, true);
    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::ServiceStopWakeBuggedInterrupt {
            step: 0,
            t_cycle: 0,
        }
    );
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);

    step_machine_t_cycles(&mut machine, 20);

    assert!(!machine.cpu().ime());
    assert_eq!(machine.cpu().registers().pc, 0x0000);
    assert_eq!(machine.cpu().registers().sp, 0xFFFD);
    assert_eq!(machine.read_bus(0xFFFD), 0x04);

    step_machine_t_cycles(&mut machine, 16);

    assert!(machine.cpu().ime());
    assert_eq!(machine.cpu().registers().pc, 0xAA04);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn stop_wake_events_do_not_survive_while_the_cpu_is_not_stopped() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x00, 0x10, 0x00, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF00, 0x10);
    machine.set_joypad_button_pressed(JoypadButton::Start, true);
    step_machine_t_cycles(&mut machine, 4);
    machine.set_joypad_button_pressed(JoypadButton::Start, false);
    step_machine_t_cycles(&mut machine, 8);

    assert_eq!(machine.cpu().registers().pc, 0x0103);
    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Stopped);
}

#[test]
fn stop_wake_and_joypad_irq_remain_separate_ordered_events_on_the_same_input_change() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x10, 0x00, 0xFB, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x10);
    machine.write_bus(0xFF0F, 0x00);
    machine.write_bus(0xFF00, 0x10);
    step_machine_t_cycles(&mut machine, 8);

    assert_eq!(machine.cpu().registers().pc, 0x0102);
    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Stopped);
    assert_eq!(machine.read_bus(0xFF0F) & 0x10, 0x00);

    machine.set_joypad_button_pressed(JoypadButton::A, true);
    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(machine.read_bus(0xFF0F) & 0x10, 0x10);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(machine.cpu().registers().pc, 0x0102);

    step_machine_t_cycles(&mut machine, 4);
    assert!(machine.cpu().delayed_ime_enable());
    assert!(!machine.cpu().ime());
    assert_eq!(machine.cpu().registers().pc, 0x0103);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(machine.read_bus(0xFF0F) & 0x10, 0x10);

    step_machine_t_cycles(&mut machine, 4);
    assert_eq!(machine.read_bus(0xFF0F) & 0x10, 0x00);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: gb_core::InterruptSource::Joypad,
            step: 0,
            t_cycle: 0,
        }
    );
}

#[test]
fn second_stop_with_the_same_button_still_held_takes_the_halt_like_branch() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x10, 0x00, 0x10, 0x00, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF00, 0x10);
    step_machine_t_cycles(&mut machine, 8);

    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Stopped);
    assert_eq!(machine.cpu().registers().pc, 0x0102);

    machine.set_joypad_button_pressed(JoypadButton::Start, true);
    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(machine.cpu().registers().pc, 0x0102);

    step_machine_t_cycles(&mut machine, 8);

    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Halted);
    assert_eq!(machine.cpu().registers().pc, 0x0104);

    machine.set_joypad_button_pressed(JoypadButton::Start, true);
    step_machine_t_cycles(&mut machine, 2);

    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Halted);
    assert_eq!(machine.cpu().registers().pc, 0x0104);
}

#[test]
fn stop_can_wake_again_after_the_button_is_released_and_pressed_again() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x10, 0x00, 0x10, 0x00, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF00, 0x10);
    step_machine_t_cycles(&mut machine, 8);
    machine.set_joypad_button_pressed(JoypadButton::Start, true);
    step_machine_t_cycles(&mut machine, 1);
    machine.set_joypad_button_pressed(JoypadButton::Start, false);
    step_machine_t_cycles(&mut machine, 8);

    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Stopped);
    assert_eq!(machine.cpu().registers().pc, 0x0104);

    machine.set_joypad_button_pressed(JoypadButton::Start, true);
    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(machine.cpu().registers().pc, 0x0104);
}
