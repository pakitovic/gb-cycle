mod common;

use common::machine_driver::step_machine_t_cycles;
use common::synthetic_cartridge::build_nom_bc_test_rom;
use gb_core::{ConsoleModel, CpuExecutionState, Machine, MachineConfig, StartupMode};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;

fn build_test_rom(program: &[u8]) -> Vec<u8> {
    build_nom_bc_test_rom(program, 0xFF, &[])
}

fn build_boot_div_probe_rom() -> Vec<u8> {
    let mut program = Vec::new();

    let read_and_push_div = |program: &mut Vec<u8>| {
        program.extend_from_slice(&[0xF0, 0x04, 0xF5]);
    };

    program.extend(std::iter::repeat_n(0x00, 6));
    read_and_push_div(&mut program);
    program.extend(std::iter::repeat_n(0x00, 57));
    read_and_push_div(&mut program);
    program.extend(std::iter::repeat_n(0x00, 56));
    read_and_push_div(&mut program);
    program.extend(std::iter::repeat_n(0x00, 57));
    read_and_push_div(&mut program);
    program.extend(std::iter::repeat_n(0x00, 57));
    read_and_push_div(&mut program);
    program.extend(std::iter::repeat_n(0x00, 58));
    read_and_push_div(&mut program);

    // Spin once the stack samples have been captured.
    program.extend_from_slice(&[0x18, 0xFE]);
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    rom[0x0100] = 0x00;
    rom[0x0101..0x0104].copy_from_slice(&[0xC3, 0x50, 0x01]);
    for (offset, byte) in program.iter().copied().enumerate() {
        rom[0x0150 + offset] = byte;
    }
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom
}

fn build_header_jump_rom(program: &[u8]) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    rom[0x0100] = 0x00;
    rom[0x0101..0x0104].copy_from_slice(&[0xC3, 0x50, 0x01]);
    for (offset, byte) in program.iter().copied().enumerate() {
        rom[0x0150 + offset] = byte;
    }
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom
}

fn build_timer_rapid_toggle_probe_rom() -> Vec<u8> {
    let mut program = vec![
        0x3E, 0x04, // ld a, $04
        0xE0, 0xFF, // ldh ($FF), a
        0xAF, // xor a
        0xE0, 0x0F, // ldh ($0F), a
        0xE0, 0x04, // ldh ($04), a
        0x3E, 0xF0, // ld a, $F0
        0xE0, 0x05, // ldh ($05), a
        0x3E, 0x04, // ld a, %00000100
        0xE0, 0x07, // ldh ($07), a
        0x01, 0xFF, 0xFF, // ld bc, $FFFF
        0xFB, // ei
    ];

    let loop_start = 0x0150 + program.len();
    program.extend_from_slice(&[
        0x3E, 0x04, // ld a, %00000100
        0xE0, 0x07, // ldh ($07), a
        0x3E, 0x00, // ld a, $00
        0xE0, 0x07, // ldh ($07), a
        0x0B, // dec bc
        0x79, // ld a, c
        0xB0, // or b
        0x20, 0xF3, // jr nz, loop_start
    ]);
    debug_assert_eq!(loop_start, 0x0165);

    build_header_jump_rom(&program)
}

#[test]
fn skip_boot_div_continues_from_the_documented_hidden_counter_state() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x00]))
        .expect("NoMBC test ROM should load");

    assert_eq!(machine.read_bus(0xFF04), 0xAB);

    step_machine_t_cycles(&mut machine, 256);

    assert_eq!(machine.read_bus(0xFF04), 0xAC);
}

#[test]
fn skip_boot_div_phase_matches_mooneye_boot_div_probe_on_dmg() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_boot_div_probe_rom())
        .expect("NoMBC test ROM should load");

    step_machine_t_cycles(&mut machine, 1_500);

    let observed = [
        machine.read_bus(0xFFFD),
        machine.read_bus(0xFFFB),
        machine.read_bus(0xFFF9),
        machine.read_bus(0xFFF7),
        machine.read_bus(0xFFF5),
        machine.read_bus(0xFFF3),
    ];

    assert_eq!(observed, [0xAC, 0xAD, 0xAD, 0xAE, 0xAF, 0xB1]);
}

#[test]
fn timer_request_becomes_visible_only_after_the_reload_delay() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x00]))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF0F, 0x00);
    machine.write_bus(0xFF04, 0x00);
    machine.write_bus(0xFF05, 0xFF);
    machine.write_bus(0xFF06, 0x42);
    machine.write_bus(0xFF07, 0x05);

    step_machine_t_cycles(&mut machine, 15);

    assert_eq!(machine.read_bus(0xFF05), 0xFF);
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);

    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(machine.read_bus(0xFF05), 0x00);
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);

    step_machine_t_cycles(&mut machine, 3);

    assert_eq!(machine.read_bus(0xFF05), 0x00);
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);

    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(machine.read_bus(0xFF05), 0x42);
    assert_eq!(machine.read_bus(0xFF0F), 0xE4);
}

#[test]
fn halted_cpu_services_timer_irq_only_after_the_reload_delay() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0xFB, 0x00, 0x76]))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF0F, 0x00);
    machine.write_bus(0xFFFF, 0x04);

    step_machine_t_cycles(&mut machine, 12);

    assert!(machine.cpu().ime());
    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Halted);

    machine.write_bus(0xFF04, 0x00);
    machine.write_bus(0xFF05, 0xFF);
    machine.write_bus(0xFF06, 0x66);
    machine.write_bus(0xFF07, 0x05);

    step_machine_t_cycles(&mut machine, 19);

    assert_eq!(machine.read_bus(0xFF05), 0x00);
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);
    assert_eq!(machine.cpu().execution_state(), CpuExecutionState::Halted);

    step_machine_t_cycles(&mut machine, 1);

    assert_eq!(machine.read_bus(0xFF0F), 0xE0);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::ServiceInterrupt {
            source: gb_core::InterruptSource::Timer,
            step: 0,
            t_cycle: 0,
        }
    );
}

#[test]
fn same_cycle_if_read_observes_a_timer_request_before_interrupt_aggregation() {
    let mut program = vec![
        0xAF, // xor a
        0xE0, 0x06, // ldh ($06), a ; TMA = 0
        0x3E, 0x05, // ld a, $05
        0xE0, 0x07, // ldh ($07), a ; TAC = 262144 Hz
        0xAF, // xor a
        0xE0, 0x0F, // ldh ($0F), a ; IF = 0
        0x3E, 0xEC, // ld a, $EC
        0xE0, 0x05, // ldh ($05), a ; TIMA = -20
    ];
    program.extend(std::iter::repeat_n(0x00, 70));
    program.extend_from_slice(&[
        0xF0, 0x0F, // ldh a, ($0F)
        0xE6, 0x04, // and $04
        0xC2, 0x6B, 0x01, // jp nz, fail
        0xF0, 0x0F, // ldh a, ($0F)
        0xE6, 0x04, // and $04
        0xCA, 0x6B, 0x01, // jp z, fail
        0x3E, 0x01, // ld a, $01
        0xEA, 0x00, 0xC0, // ld ($C000), a
        0x18, 0xFE, // jr -2
        0x3E, 0x02, // fail: ld a, $02
        0xEA, 0x00, 0xC0, // ld ($C000), a
        0x18, 0xFE, // jr -2
    ]);

    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_header_jump_rom(&program))
        .expect("NoMBC test ROM should load");
    machine.write_bus(0xC000, 0x00);

    for _ in 0..20_000 {
        machine.step_t_cycle();
        let result = machine.read_bus(0xC000);
        if result != 0 {
            assert_eq!(result, 0x01);
            return;
        }
    }

    panic!("IF visibility probe did not finish");
}

#[test]
fn rapid_timer_toggle_matches_the_mooneye_interrupt_window() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_timer_rapid_toggle_probe_rom())
        .expect("NoMBC test ROM should load");

    for _ in 0..20_000 {
        machine.step_t_cycle();
        if let CpuExecutionState::ServiceInterrupt {
            source: gb_core::InterruptSource::Timer,
            ..
        } = machine.cpu().execution_state()
        {
            let registers = machine.cpu().registers();
            assert_eq!(registers.b, 0xFF);
            assert_eq!(
                registers.c,
                0xD9,
                "service_bc={:#04X}{:#04X} service_state={:?}",
                registers.b,
                registers.c,
                machine.cpu().execution_state()
            );
            return;
        }
    }

    panic!("timer interrupt was not accepted within the rapid-toggle probe window");
}
