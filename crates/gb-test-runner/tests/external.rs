use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use gb_core::{ConsoleModel, Machine, MachineConfig, StartupMode};
use gb_test_runner::{
    RomRunner, RomSuite, acid_dmg_curated_suite, blargg_dmg_curated_suite, daid_dmg_curated_suite,
    discover_test_rom_store_root, hacktix_dmg_curated_suite, mealybug_tearoom_dmg_curated_suite,
    mooneye_acceptance_dmg_curated_suite, update_curated_test_report,
};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should be two levels above gb-test-runner")
        .to_path_buf()
}

fn run_curated_suite_and_update_report(
    suite: &RomSuite,
    suite_label: &str,
) -> Option<gb_test_runner::RomSuiteReport> {
    let workspace_root = workspace_root();

    let Some(store_root) = discover_test_rom_store_root(&workspace_root) else {
        eprintln!(
            "skipping ignored test because neither GB_CYCLE_TEST_ROM_ROOT nor the default curated test ROM store is configured"
        );
        return None;
    };
    let Some(family) = suite.family.as_deref() else {
        panic!("{suite_label} should declare its curated family");
    };
    if !store_root.join(family).exists() {
        eprintln!(
            "skipping ignored test because curated family {family} is not materialized under {}",
            store_root.display()
        );
        return None;
    }

    let report = RomRunner::new()
        .run_suite(suite)
        .unwrap_or_else(|_| panic!("{suite_label} should execute"));
    update_curated_test_report(&workspace_root, &report)
        .expect("curated report should update after a repo-managed suite run");
    Some(report)
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn blargg_curated_suite_passes_from_repo_store() {
    let Some(report) =
        run_curated_suite_and_update_report(&blargg_dmg_curated_suite(), "curated blargg suite")
    else {
        return;
    };
    assert!(report.all_passed(), "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn acid_curated_suite_passes_from_repo_store() {
    let Some(report) =
        run_curated_suite_and_update_report(&acid_dmg_curated_suite(), "curated acid suite")
    else {
        return;
    };
    assert!(report.all_non_failing(), "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn mealybug_curated_suite_updates_report_from_repo_store() {
    let Some(report) = run_curated_suite_and_update_report(
        &mealybug_tearoom_dmg_curated_suite(),
        "curated mealybug suite",
    ) else {
        return;
    };
    assert_eq!(
        report.family.as_deref(),
        Some("mealybug-tearoom-tests"),
        "{report:#?}"
    );
    assert_eq!(report.cases.len(), 10, "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn mooneye_curated_suite_updates_report_from_repo_store() {
    let Some(report) = run_curated_suite_and_update_report(
        &mooneye_acceptance_dmg_curated_suite(),
        "curated mooneye suite",
    ) else {
        return;
    };
    assert_eq!(report.family.as_deref(), Some("mooneye"), "{report:#?}");
    assert_eq!(report.cases.len(), 66, "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn daid_curated_suite_updates_report_from_repo_store() {
    let Some(report) =
        run_curated_suite_and_update_report(&daid_dmg_curated_suite(), "curated daid suite")
    else {
        return;
    };
    assert_eq!(report.family.as_deref(), Some("daid"), "{report:#?}");
    assert_eq!(report.cases.len(), 3, "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn hacktix_curated_suite_updates_report_from_repo_store() {
    let Some(report) =
        run_curated_suite_and_update_report(&hacktix_dmg_curated_suite(), "curated hacktix suite")
    else {
        return;
    };
    assert_eq!(report.family.as_deref(), Some("hacktix"), "{report:#?}");
    assert_eq!(report.cases.len(), 2, "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn blargg_01_special_copies_bank1_payload_to_wram_before_running() {
    let workspace_root = workspace_root();

    let Some(root) = discover_test_rom_store_root(&workspace_root) else {
        eprintln!(
            "skipping ignored test because neither GB_CYCLE_TEST_ROM_ROOT nor the default curated test ROM store is configured"
        );
        return;
    };

    let rom_path = root.join("blargg/cpu_instrs/01-special.gb");
    let rom = fs::read(&rom_path).expect("curated blargg ROM should be readable");

    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom.clone())
        .expect("curated blargg ROM should load");

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
        "expected bank 1 payload to execute and emit serial output"
    );
}
