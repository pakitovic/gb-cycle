#![allow(dead_code)]

use gb_core::{CpuExecutionState, Machine};

pub fn step_machine_t_cycles(machine: &mut Machine, steps: usize) {
    for _ in 0..steps {
        machine.step_t_cycle();
    }
}

pub fn step_machine_until<F>(machine: &mut Machine, max_steps: usize, mut predicate: F)
where
    F: FnMut(&Machine) -> bool,
{
    for _ in 0..max_steps {
        if predicate(machine) {
            return;
        }
        machine.step_t_cycle();
    }

    assert!(
        predicate(machine),
        "predicate was not satisfied within {max_steps} T-cycles"
    );
}

pub fn step_until_wram_sentinel(machine: &mut Machine, address: u16, value: u8, max_steps: usize) {
    step_until_wram_sentinel_with_driver(machine, address, value, max_steps, |_| {});
}

pub fn step_until_wram_sentinel_with_driver<F>(
    machine: &mut Machine,
    address: u16,
    value: u8,
    max_steps: usize,
    mut driver: F,
) where
    F: FnMut(&mut Machine),
{
    for _ in 0..max_steps {
        if machine.read_bus(address) == value {
            return;
        }
        driver(machine);
        if machine.read_bus(address) == value {
            return;
        }
        machine.step_t_cycle();
    }

    panic!(
        "sentinel was not reached: observed={:#04X} pc={:#06X} state={:?} opcode={:?}",
        machine.read_bus(address),
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.cpu().current_opcode()
    );
}

pub fn run_until_halted(machine: &mut Machine, max_t_cycles: usize) -> u8 {
    for _ in 0..max_t_cycles {
        machine.step_t_cycle();
        if machine.cpu().execution_state() == CpuExecutionState::Halted {
            return machine.cpu().registers().b;
        }
    }

    panic!(
        "probe ROM did not halt; pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}
