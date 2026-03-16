use gb_core::{
    BootRomAssets, BootRomKind, ConsoleModel, CpuExecutionState, JoypadButton, Machine,
    MachineConfig, StartupMode,
};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
const BOOT_ROM_LEN: usize = 0x0100;

fn build_test_rom(program: &[u8], boot_opcode: u8) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    rom[0x0000] = boot_opcode;
    for (offset, byte) in program.iter().copied().enumerate() {
        rom[0x0100 + offset] = byte;
    }
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom
}

fn build_test_rom_with_patches(
    program: &[u8],
    boot_opcode: u8,
    patches: &[(usize, u8)],
) -> Vec<u8> {
    let mut rom = build_test_rom(program, boot_opcode);
    for &(address, value) in patches {
        rom[address] = value;
    }
    rom
}

fn build_boot_rom_image(first_opcode: u8) -> Vec<u8> {
    let mut rom = vec![0xFF; BOOT_ROM_LEN];
    rom[0x0000] = first_opcode;
    rom
}

fn step_machine_t_cycles(machine: &mut Machine, steps: usize) {
    for _ in 0..steps {
        machine.step_t_cycle();
    }
}

#[test]
fn skip_boot_fetches_the_entry_opcode_from_the_cartridge_bus_path() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0xCB], 0x12))
        .expect("NoMBC test ROM should load");

    step_machine_t_cycles(&mut machine, 4);

    assert_eq!(machine.cpu().registers().pc, 0x0101);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::Execute {
            opcode: 0xCB,
            step: 0,
            t_cycle: 0,
        }
    );
    assert_eq!(machine.cpu().current_opcode(), Some(0xCB));
}

#[test]
fn real_boot_fetches_from_boot_rom_while_the_overlay_is_mapped() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg)
            .with_startup_mode(StartupMode::RealBoot)
            .with_boot_rom_assets(
                BootRomAssets::none()
                    .with_bytes(BootRomKind::Dmg, build_boot_rom_image(0x99))
                    .expect("DMG boot ROM image should validate"),
            ),
    );

    machine
        .load_cartridge(build_test_rom(&[0xCB], 0x12))
        .expect("NoMBC test ROM should load");

    step_machine_t_cycles(&mut machine, 4);

    assert!(machine.boot().is_boot_rom_mapped());
    assert_eq!(machine.cpu().registers().pc, 0x0001);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::Execute {
            opcode: 0x99,
            step: 0,
            t_cycle: 0,
        }
    );
    assert_eq!(machine.cpu().current_opcode(), Some(0x99));
}

#[test]
fn machine_executes_imm16_load_and_immediate_alu_with_real_fetches() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(
            &[0x31, 0x34, 0x12, 0x3E, 0x0F, 0xC6, 0x01],
            0x12,
        ))
        .expect("NoMBC test ROM should load");

    step_machine_t_cycles(&mut machine, 28);

    assert_eq!(machine.cpu().registers().sp, 0x1234);
    assert_eq!(machine.cpu().registers().a, 0x10);
    assert_eq!(machine.cpu().registers().f, 0x20);
    assert_eq!(machine.cpu().registers().pc, 0x0107);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(machine.cpu().current_opcode(), None);
}

#[test]
fn machine_executes_control_flow_stack_and_cb_prefix_program() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(
            &[
                0x18, 0x02, 0x00, 0x00, 0xCD, 0x09, 0x01, 0x00, 0x00, 0xCB, 0x11, 0xC9,
            ],
            0x12,
        ))
        .expect("NoMBC test ROM should load");

    step_machine_t_cycles(&mut machine, 60);

    assert_eq!(machine.cpu().registers().pc, 0x0107);
    assert_eq!(machine.cpu().registers().sp, 0xFFFE);
    assert_eq!(machine.cpu().registers().c, 0x27);
    assert_eq!(machine.cpu().registers().f, 0x00);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(machine.cpu().current_opcode(), None);
}

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
fn halt_with_ime_enabled_wakes_on_a_later_irq_and_services_it() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0xFB, 0x00, 0x76, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x01);
    machine.write_bus(0xFF0F, 0x00);
    step_machine_t_cycles(&mut machine, 12);

    assert!(machine.cpu().ime());
    assert_eq!(machine.cpu().registers().pc, 0x0103);
    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Halted);

    machine.write_bus(0xFF0F, 0x01);
    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(machine.read_bus(0xFF0F), 0xE0);
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
    assert_eq!(machine.read_bus(0xFFFC), 0x03);
}

#[test]
fn halt_with_ime_disabled_wakes_without_servicing_the_pending_irq() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x76, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x01);
    machine.write_bus(0xFF0F, 0x00);
    step_machine_t_cycles(&mut machine, 4);

    assert!(!machine.cpu().ime());
    assert_eq!(machine.cpu().registers().pc, 0x0101);
    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Halted);

    machine.write_bus(0xFF0F, 0x01);
    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(machine.read_bus(0xFF0F), 0xE1);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    step_machine_t_cycles(&mut machine, 4);

    assert_eq!(machine.cpu().registers().pc, 0x0102);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn halt_bug_suppresses_the_next_pc_increment_without_servicing_the_irq() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x76, 0x3C, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x01);
    machine.write_bus(0xFF0F, 0x01);

    step_machine_t_cycles(&mut machine, 8);

    assert_eq!(machine.read_bus(0xFF0F), 0xE1);
    assert_eq!(machine.cpu().registers().a, 0x02);
    assert_eq!(machine.cpu().registers().pc, 0x0101);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    step_machine_t_cycles(&mut machine, 4);

    assert_eq!(machine.cpu().registers().a, 0x03);
    assert_eq!(machine.cpu().registers().pc, 0x0102);
}

#[test]
fn stop_wakes_from_the_joypad_path_independently_of_selection_and_services_irq_later() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x10, 0x00, 0xFB, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x01);
    machine.write_bus(0xFF0F, 0x01);
    machine.write_bus(0xFF00, 0x30);
    step_machine_t_cycles(&mut machine, 8);

    assert_eq!(machine.cpu().registers().pc, 0x0102);
    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Stopped);

    machine.set_joypad_button_pressed(JoypadButton::A, true);
    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(machine.read_bus(0xFF0F), 0xE1);
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
