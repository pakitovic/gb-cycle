use super::*;

#[test]
fn skip_boot_fetches_the_entry_opcode_from_the_cartridge_bus_path() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0xCB], 0x12))
        .expect("NoMBC test ROM should load");

    step_machine_t_cycles(&mut machine, 4);

    assert_eq!(machine.cpu().registers().pc, 0x0101);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::Execute {
            step: 0,
            t_cycle: 0
        }
    );
    assert_eq!(machine.cpu().current_opcode(), Some(0xCB));
    assert_eq!(
        machine.cpu().last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::ReadWithIncDec,
            access_address: Some(0x0100),
            idu_address: Some(0x0101),
            update_direction: Some(CpuAddressUpdateDirection::Increment),
        })
    );
}

#[test]
fn real_boot_fetches_from_boot_rom_while_the_overlay_is_mapped() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy)
            .with_startup_mode(StartupMode::RealBoot)
            .with_boot_rom_assets(
                BootRomAssets::none()
                    .with_bytes(HardwareRevision::DmgCpuC, build_boot_rom_image(0xD3))
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
        CpuExecutionState::DiagnosticTrap {
            trap: CpuDiagnosticTrap::InvalidOpcode {
                opcode: 0xD3,
                address: 0x0000,
            },
        }
    );
    assert_eq!(machine.cpu().current_opcode(), Some(0xD3));
}

#[test]
fn machine_executes_imm16_load_and_immediate_alu_with_real_fetches() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
fn machine_executes_add_hl_register_pair_and_preserves_zero_flag() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(
            &[0x3E, 0x01, 0xFE, 0x01, 0x21, 0x34, 0x12, 0x29],
            0x12,
        ))
        .expect("NoMBC test ROM should load");

    step_machine_t_cycles(&mut machine, 36);

    assert_eq!(machine.cpu().registers().h, 0x24);
    assert_eq!(machine.cpu().registers().l, 0x68);
    assert_eq!(machine.cpu().registers().f, 0x80);
    assert_eq!(machine.cpu().registers().pc, 0x0108);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn machine_executes_control_flow_stack_and_cb_prefix_program() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
fn machine_executes_alu_register_hl_and_immediate_families() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(
            &[
                0x21, 0x00, 0xC0, 0x3E, 0xF0, 0x06, 0x0F, 0xB0, 0xE6, 0x3C, 0xAE,
            ],
            0x12,
        ))
        .expect("NoMBC test ROM should load");
    machine.write_bus(0xC000, 0x3C);

    step_machine_t_cycles(&mut machine, 48);

    assert_eq!(machine.cpu().registers().a, 0x00);
    assert_eq!(machine.cpu().registers().f, 0x80);
    assert_eq!(machine.cpu().registers().pc, 0x010B);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn machine_executes_register_to_register_loads_from_a_into_dehl() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0xAF, 0x57, 0x5F, 0x67, 0x6F, 0x00], 0x12))
        .expect("NoMBC test ROM should load");

    step_machine_t_cycles(&mut machine, 24);

    assert_eq!(machine.cpu().registers().a, 0x00);
    assert_eq!(machine.cpu().registers().d, 0x00);
    assert_eq!(machine.cpu().registers().e, 0x00);
    assert_eq!(machine.cpu().registers().h, 0x00);
    assert_eq!(machine.cpu().registers().l, 0x00);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn machine_increments_a_b_c_d_e_h_l_consistently_inside_a_short_loop_body() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(
            &[
                0xAF, 0x47, 0x4F, 0x57, 0x5F, 0x67, 0x6F, 0x3C, 0x04, 0x0C, 0x14, 0x1C, 0x24, 0x2C,
            ],
            0x12,
        ))
        .expect("NoMBC test ROM should load");

    step_machine_t_cycles(&mut machine, 56);

    assert_eq!(machine.cpu().registers().a, 0x01);
    assert_eq!(machine.cpu().registers().b, 0x01);
    assert_eq!(machine.cpu().registers().c, 0x01);
    assert_eq!(machine.cpu().registers().d, 0x01);
    assert_eq!(machine.cpu().registers().e, 0x01);
    assert_eq!(machine.cpu().registers().h, 0x01);
    assert_eq!(machine.cpu().registers().l, 0x01);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn machine_executes_decimal_adjust_accumulator_flag_ops_accumulator_rotates_and_jp_hl() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(
            &[
                0x3E, 0x09, 0xC6, 0x09, 0x27, 0x37, 0x1F, 0x2F, 0x3F, 0x21, 0x0D, 0x01, 0xE9, 0x00,
            ],
            0x12,
        ))
        .expect("NoMBC test ROM should load");

    step_machine_t_cycles(&mut machine, 56);

    assert_eq!(machine.cpu().registers().a, 0x73);
    assert_eq!(machine.cpu().registers().f, 0x10);
    assert_eq!(machine.cpu().registers().pc, 0x010E);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn invalid_opcode_hole_enters_a_visible_machine_level_diagnostic_trap() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0xD3], 0x12))
        .expect("NoMBC test ROM should load");

    step_machine_t_cycles(&mut machine, 4);

    assert_eq!(machine.cpu().registers().pc, 0x0101);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::DiagnosticTrap {
            trap: CpuDiagnosticTrap::InvalidOpcode {
                opcode: 0xD3,
                address: 0x0100,
            },
        }
    );

    let trapped_cycle = machine.next_t_cycle();
    step_machine_t_cycles(&mut machine, 4);

    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::DiagnosticTrap {
            trap: CpuDiagnosticTrap::InvalidOpcode {
                opcode: 0xD3,
                address: 0x0100,
            },
        }
    );
    assert_eq!(machine.cpu().registers().pc, 0x0101);
    assert_eq!(machine.next_t_cycle().get(), trapped_cycle.get() + 4);
}

#[test]
fn supported_cb_set_opcode_completes_and_keeps_machine_running() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0xCB, 0xFF], 0x12))
        .expect("NoMBC test ROM should load");

    step_machine_t_cycles(&mut machine, 8);

    assert_eq!(machine.cpu().registers().pc, 0x0102);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(machine.cpu().current_opcode(), None);
}
