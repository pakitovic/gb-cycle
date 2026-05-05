mod common;

use common::synthetic_cartridge::{BankedCartridgeBuilder, ProgramBuilder};
use gb_core::{ConsoleModel, Machine, MachineConfig, StartupMode};

const FIXTURE_ACCEPT_ENV: &str = common::fixture_env::PHASE6;
const SENTINEL_VALUE: u8 = 0xA5;

fn load_fixture_machine(rom_name: &str, expected_rom: &[u8]) -> Machine {
    load_fixture_machine_with_model(rom_name, expected_rom, ConsoleModel::GameBoy)
}

fn load_fixture_machine_with_model(
    rom_name: &str,
    expected_rom: &[u8],
    console_model: ConsoleModel,
) -> Machine {
    let rom_fixture = common::fixtures::ensure_suite_binary_fixture(
        "phase6",
        rom_name,
        expected_rom,
        FIXTURE_ACCEPT_ENV,
    );
    let mut machine =
        Machine::new(MachineConfig::new(console_model).with_startup_mode(StartupMode::SkipBoot));
    machine
        .load_cartridge(rom_fixture)
        .expect("synthetic cartridge test ROM should load");
    machine
}

fn step_until_wram_sentinel(machine: &mut Machine, address: u16) {
    common::machine_driver::step_until_wram_sentinel(machine, address, SENTINEL_VALUE, 80_000);
}

fn assert_serial_output(machine: &mut Machine, expected: &[u8]) {
    let mut serial = Vec::new();
    for _ in 0..80_000 {
        serial.extend(machine.take_serial_output_bytes());
        if serial.len() >= expected.len() {
            break;
        }
        machine.step_t_cycle();
    }

    assert_eq!(serial, expected);
}

#[path = "phase6/phase6_mbc1.rs"]
mod phase6_mbc1;
#[path = "phase6/phase6_mbc2.rs"]
mod phase6_mbc2;
#[path = "phase6/phase6_mbc3.rs"]
mod phase6_mbc3;
#[path = "phase6/phase6_mbc5.rs"]
mod phase6_mbc5;
#[path = "phase6/phase6_mbc6.rs"]
mod phase6_mbc6;
