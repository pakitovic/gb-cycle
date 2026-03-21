use std::env;
use std::fs;
use std::path::Path;

use gb_core::{ConsoleModel, Machine, MachineConfig, StartupMode};
use gb_test_runner::{
    RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR, RomRunner, discover_external_rom_root_for_key,
    retrio_blargg_cpu_instrs_full_suite, retrio_blargg_cpu_smoke_suite,
    retrio_blargg_halt_bug_suite, retrio_blargg_instr_timing_suite,
    retrio_blargg_mem_timing_individual_suite, retrio_blargg_mem_timing_suite,
    retrio_blargg_oam_bug_suite,
};

#[test]
#[ignore = "requires retrio/gb-test-roms assets under GB_CYCLE_RETRIO_GB_TEST_ROMS_ROOT"]
fn retrio_blargg_cpu_smoke_suite_runs_against_real_external_assets() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should be two levels above gb-test-runner");

    if discover_external_rom_root_for_key(workspace_root, RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR)
        .expect("external ROM source manifest should be readable")
        .is_none()
    {
        eprintln!(
            "skipping ignored test because neither {} nor the default external ROM store is configured",
            RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR
        );
        return;
    }

    let report = RomRunner::new()
        .run_suite(&retrio_blargg_cpu_smoke_suite())
        .expect("external retrio/blargg suite should execute");

    assert!(report.all_passed(), "{report:#?}");
}

#[test]
#[ignore = "requires retrio/gb-test-roms assets under GB_CYCLE_RETRIO_GB_TEST_ROMS_ROOT"]
fn retrio_blargg_cpu_instrs_full_suite_runs_against_real_external_assets() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should be two levels above gb-test-runner");

    if discover_external_rom_root_for_key(workspace_root, RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR)
        .expect("external ROM source manifest should be readable")
        .is_none()
    {
        eprintln!(
            "skipping ignored test because neither {} nor the default external ROM store is configured",
            RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR
        );
        return;
    }

    let report = RomRunner::new()
        .run_suite(&retrio_blargg_cpu_instrs_full_suite())
        .expect("external retrio/blargg cpu_instrs full suite should execute");

    assert!(report.all_passed(), "{report:#?}");
}

#[test]
#[ignore = "requires retrio/gb-test-roms assets under GB_CYCLE_RETRIO_GB_TEST_ROMS_ROOT"]
fn retrio_blargg_instr_timing_suite_runs_against_real_external_assets() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should be two levels above gb-test-runner");

    if discover_external_rom_root_for_key(workspace_root, RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR)
        .expect("external ROM source manifest should be readable")
        .is_none()
    {
        eprintln!(
            "skipping ignored test because neither {} nor the default external ROM store is configured",
            RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR
        );
        return;
    }

    let report = RomRunner::new()
        .run_suite(&retrio_blargg_instr_timing_suite())
        .expect("external retrio/blargg instr_timing suite should execute");

    assert!(report.all_passed(), "{report:#?}");
}

#[test]
#[ignore = "requires retrio/gb-test-roms assets under GB_CYCLE_RETRIO_GB_TEST_ROMS_ROOT"]
fn retrio_blargg_halt_bug_suite_runs_against_real_external_assets() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should be two levels above gb-test-runner");

    if discover_external_rom_root_for_key(workspace_root, RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR)
        .expect("external ROM source manifest should be readable")
        .is_none()
    {
        eprintln!(
            "skipping ignored test because neither {} nor the default external ROM store is configured",
            RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR
        );
        return;
    }

    let report = RomRunner::new()
        .run_suite(&retrio_blargg_halt_bug_suite())
        .expect("external retrio/blargg halt_bug suite should execute");

    assert!(report.all_passed(), "{report:#?}");
}

#[test]
#[ignore = "requires retrio/gb-test-roms assets under GB_CYCLE_RETRIO_GB_TEST_ROMS_ROOT"]
fn retrio_blargg_mem_timing_suite_runs_against_real_external_assets() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should be two levels above gb-test-runner");

    if discover_external_rom_root_for_key(workspace_root, RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR)
        .expect("external ROM source manifest should be readable")
        .is_none()
    {
        eprintln!(
            "skipping ignored test because neither {} nor the default external ROM store is configured",
            RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR
        );
        return;
    }

    let report = RomRunner::new()
        .run_suite(&retrio_blargg_mem_timing_suite())
        .expect("external retrio/blargg mem_timing suite should execute");

    assert!(report.all_passed(), "{report:#?}");
}

#[test]
#[ignore = "requires retrio/gb-test-roms assets under GB_CYCLE_RETRIO_GB_TEST_ROMS_ROOT"]
fn retrio_blargg_mem_timing_individual_suite_runs_against_real_external_assets() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should be two levels above gb-test-runner");

    if discover_external_rom_root_for_key(workspace_root, RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR)
        .expect("external ROM source manifest should be readable")
        .is_none()
    {
        eprintln!(
            "skipping ignored test because neither {} nor the default external ROM store is configured",
            RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR
        );
        return;
    }

    let report = RomRunner::new()
        .run_suite(&retrio_blargg_mem_timing_individual_suite())
        .expect("external retrio/blargg mem_timing individual suite should execute");

    assert!(report.all_passed(), "{report:#?}");
}

#[test]
#[ignore = "requires retrio/gb-test-roms assets under GB_CYCLE_RETRIO_GB_TEST_ROMS_ROOT"]
fn retrio_blargg_oam_bug_suite_runs_against_real_external_assets() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should be two levels above gb-test-runner");

    if discover_external_rom_root_for_key(workspace_root, RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR)
        .expect("external ROM source manifest should be readable")
        .is_none()
    {
        eprintln!(
            "skipping ignored test because neither {} nor the default external ROM store is configured",
            RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR
        );
        return;
    }

    let report = RomRunner::new()
        .run_suite(&retrio_blargg_oam_bug_suite())
        .expect("external retrio/blargg oam_bug suite should execute");

    assert!(report.all_passed(), "{report:#?}");
}

#[test]
#[ignore = "requires retrio/gb-test-roms assets under GB_CYCLE_RETRIO_GB_TEST_ROMS_ROOT"]
fn retrio_blargg_01_special_copies_bank1_payload_to_wram_before_running() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should be two levels above gb-test-runner");

    let Some(root) =
        discover_external_rom_root_for_key(workspace_root, RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR)
            .expect("external ROM source manifest should be readable")
    else {
        eprintln!(
            "skipping ignored test because neither {} nor the default external ROM store is configured",
            RETRIO_GB_TEST_ROMS_ROOT_ENV_VAR
        );
        return;
    };

    let rom_path = root.join("cpu_instrs/individual/01-special.gb");
    let rom = fs::read(&rom_path).expect("external ROM should be readable");

    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom.clone())
        .expect("external ROM should load");

    let mut saw_serial_output = false;
    for _ in 0..2_000_000_u64 {
        machine.step_t_cycle();
        if !machine.take_serial_output_bytes().is_empty() {
            saw_serial_output = true;
            break;
        }
    }

    assert!(
        saw_serial_output,
        "expected startup title output before comparison"
    );

    for offset in 0..0x1000_u16 {
        let expected = rom[0x4000 + usize::from(offset)];
        let actual = machine.read_bus(0xC000 + offset);
        assert_eq!(
            actual,
            expected,
            "copied byte mismatch at WRAM {:#06X} from ROM offset {:#06X}",
            0xC000 + offset,
            0x4000 + offset
        );
    }
}
