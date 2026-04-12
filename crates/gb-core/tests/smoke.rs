mod common;

use gb_core::{ConsoleModel, Machine, MachineConfig, TCycle};

#[test]
fn public_api_smoke_test_constructs_a_machine() {
    let machine = Machine::new(MachineConfig::new(ConsoleModel::Dmg));

    assert_eq!(machine.next_t_cycle(), TCycle::ZERO);
}

#[test]
fn phase0_test_layout_exposes_reserved_fixture_directories() {
    common::paths::assert_directory_exists(&common::paths::tests_dir());
    common::paths::assert_directory_exists(&common::paths::fixtures_dir());
    common::paths::assert_directory_exists(&common::paths::rom_fixtures_dir());
    common::paths::assert_directory_exists(&common::paths::trace_fixtures_dir());
}
