use gb_core::{
    BootRomAssets, BootRomKind, ConsoleModel, CpuAddressEvent, CpuAddressEventKind,
    CpuAddressUpdateDirection, CpuDiagnosticTrap, CpuExecutionState, JoypadButton, Machine,
    MachineConfig, SerialTransferState, StartupMode,
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
        MachineConfig::new(ConsoleModel::Dmg)
            .with_startup_mode(StartupMode::RealBoot)
            .with_boot_rom_assets(
                BootRomAssets::none()
                    .with_bytes(BootRomKind::Dmg, build_boot_rom_image(0xD3))
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
fn machine_executes_add_hl_register_pair_and_preserves_zero_flag() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
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
fn machine_executes_ldh_a8_reads_and_writes_through_ff00_offset() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(
            &[0x3E, 0x34, 0xE0, 0x01, 0x3E, 0x00, 0xF0, 0x01],
            0x12,
        ))
        .expect("NoMBC test ROM should load");

    step_machine_t_cycles(&mut machine, 40);

    assert_eq!(machine.read_bus(0xFF01), 0x34);
    assert_eq!(machine.cpu().registers().a, 0x34);
    assert_eq!(machine.cpu().registers().pc, 0x0108);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
}

#[test]
fn machine_executes_ld_ff00_plus_c_reads_and_writes_through_the_same_bus_path() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(
            &[0x0E, 0x01, 0x3E, 0x56, 0xE2, 0x3E, 0x00, 0xF2],
            0x12,
        ))
        .expect("NoMBC test ROM should load");

    step_machine_t_cycles(&mut machine, 40);

    assert_eq!(machine.read_bus(0xFF01), 0x56);
    assert_eq!(machine.cpu().registers().a, 0x56);
    assert_eq!(machine.cpu().registers().pc, 0x0108);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
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
fn machine_exposes_hli_hld_and_incdec_address_events_through_the_public_cpu_api() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(
            &[
                0x21, 0x00, 0xC0, 0x2A, 0x21, 0x01, 0xC0, 0x32, 0x21, 0xFF, 0xFE, 0x23,
            ],
            0x12,
        ))
        .expect("NoMBC test ROM should load");
    machine.write_bus(0xC000, 0x77);

    step_machine_t_cycles(&mut machine, 20);

    assert_eq!(machine.cpu().registers().a, 0x77);
    assert_eq!(
        machine.cpu().last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::ReadWithIncDec,
            access_address: Some(0xC000),
            idu_address: Some(0xC001),
            update_direction: Some(CpuAddressUpdateDirection::Increment),
        })
    );

    step_machine_t_cycles(&mut machine, 20);

    assert_eq!(machine.read_bus(0xC001), 0x77);
    assert_eq!(
        machine.cpu().last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::WriteWithIncDec,
            access_address: Some(0xC001),
            idu_address: Some(0xC000),
            update_direction: Some(CpuAddressUpdateDirection::Decrement),
        })
    );

    step_machine_t_cycles(&mut machine, 20);

    assert_eq!(machine.cpu().registers().h, 0xFF);
    assert_eq!(machine.cpu().registers().l, 0x00);
    assert_eq!(
        machine.cpu().last_address_event(),
        Some(CpuAddressEvent {
            kind: CpuAddressEventKind::IncDec,
            access_address: None,
            idu_address: Some(0xFF00),
            update_direction: Some(CpuAddressUpdateDirection::Increment),
        })
    );
}

#[test]
fn machine_executes_alu_register_hl_and_immediate_families() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
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
fn cpu_trace_mentions_the_last_address_event_next_to_the_last_bus_activity() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x00], 0x12))
        .expect("NoMBC test ROM should load");

    step_machine_t_cycles(&mut machine, 4);

    let trace = machine.tracer().sink().render_text();

    assert!(trace.contains("last_bus_activity=opcode_fetch@0x0100=0x00"));
    assert!(trace.contains("last_address_event=read+inc@0x0100->0x0101"));
}

#[test]
fn invalid_opcode_hole_enters_a_visible_machine_level_diagnostic_trap() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
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
fn ei_halt_with_a_pending_irq_services_once_and_returns_to_halt() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom_with_patches(
            &[0xFB, 0x76, 0x3C, 0x00],
            0x12,
            &[(0x0040, 0xD9)],
        ))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x01);
    machine.write_bus(0xFF0F, 0x01);

    step_machine_t_cycles(&mut machine, 8);

    assert!(!machine.cpu().ime());
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
    assert_eq!(machine.read_bus(0xFFFC), 0x01);

    step_machine_t_cycles(&mut machine, 16);

    assert_eq!(machine.cpu().registers().pc, 0x0101);
    assert!(machine.cpu().ime());
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    step_machine_t_cycles(&mut machine, 4);

    assert_eq!(machine.cpu().registers().pc, 0x0102);
    assert_eq!(machine.cpu().registers().a, 0x01);
    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Halted);
}

#[test]
fn ei_halt_followed_by_rst_still_returns_to_halt_before_executing_rst() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom_with_patches(
            &[0xFB, 0x76, 0xFF, 0x00],
            0x12,
            &[(0x0040, 0xD9)],
        ))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFFFF, 0x01);
    machine.write_bus(0xFF0F, 0x01);

    step_machine_t_cycles(&mut machine, 8);

    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: gb_core::InterruptSource::VBlank,
            step: 0,
            t_cycle: 0,
        }
    );
    assert_eq!(machine.cpu().registers().pc, 0x0101);
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);

    step_machine_t_cycles(&mut machine, 20);

    assert_eq!(machine.cpu().registers().pc, 0x0040);
    assert_eq!(machine.cpu().registers().sp, 0xFFFC);
    assert_eq!(machine.read_bus(0xFFFD), 0x01);
    assert_eq!(machine.read_bus(0xFFFC), 0x01);

    step_machine_t_cycles(&mut machine, 16);

    assert_eq!(machine.cpu().registers().pc, 0x0101);
    assert!(machine.cpu().ime());
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );

    step_machine_t_cycles(&mut machine, 4);

    assert_eq!(machine.cpu().registers().pc, 0x0102);
    assert_eq!(machine.cpu().registers().sp, 0xFFFE);
    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Halted);
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
