mod common;

use common::machine_driver::step_machine_t_cycles;
use common::synthetic_cartridge::build_nom_bc_test_rom;
use gb_core::{
    BootRomAssets, BootRomKind, ConsoleModel, CpuAddressEvent, CpuAddressEventKind,
    CpuAddressUpdateDirection, CpuDiagnosticTrap, CpuExecutionState, JoypadButton, Machine,
    MachineConfig, SerialTransferState, StartupMode,
};

const BOOT_ROM_LEN: usize = 0x0100;

fn build_test_rom(program: &[u8], boot_opcode: u8) -> Vec<u8> {
    build_nom_bc_test_rom(program, boot_opcode, &[])
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

#[path = "cpu/cpu_bus_address_events.rs"]
mod cpu_bus_address_events;
#[path = "cpu/cpu_fetch_decode.rs"]
mod cpu_fetch_decode;
#[path = "cpu/cpu_halt_stop.rs"]
mod cpu_halt_stop;
#[path = "cpu/cpu_interrupts_ime.rs"]
mod cpu_interrupts_ime;
