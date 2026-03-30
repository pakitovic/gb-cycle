use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use gb_core::{
    BootRomAssets, ConsoleModel, CpuDiagnosticTrap, CpuExecutionState, Machine, MachineConfig,
    StartupMode, TraceSummaryBuffer,
};
use gb_test_runner::{
    RomRunner, RomSuite, acid_dmg_curated_suite, blargg_dmg_repo_gated_suite, boot_rom_image_path,
    boot_rom_kind_for_console_model, cpp_dmg_curated_suite, daid_dmg_curated_suite,
    discover_boot_rom_store_root, discover_test_rom_store_root, hacktix_dmg_curated_suite,
    mealybug_tearoom_dmg_curated_suite, mooneye_acceptance_dmg_curated_suite,
    update_curated_test_report, verify_boot_rom_file,
};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
const REAL_BOOT_HANDOFF_T_CYCLE_LIMIT: usize = 25_000_000;
const CARTRIDGE_ENTRY_OPCODE: u8 = 0xD3;
const NINTENDO_LOGO: [u8; 48] = [
    0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B, 0x03, 0x73, 0x00, 0x83, 0x00, 0x0C, 0x00, 0x0D,
    0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E, 0xDC, 0xCC, 0x6E, 0xE6, 0xDD, 0xDD, 0xD9, 0x99,
    0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC, 0xDD, 0xDC, 0x99, 0x9F, 0xBB, 0xB9, 0x33, 0x3E,
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should be two levels above gb-test-runner")
        .to_path_buf()
}

fn run_curated_suite(
    suite: &RomSuite,
    suite_label: &str,
    update_report: bool,
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
    if update_report {
        update_curated_test_report(&workspace_root, &report)
            .expect("curated report should update after a repo-managed suite run");
    }
    Some(report)
}

fn build_real_boot_validation_rom() -> Vec<u8> {
    let mut rom = vec![0x00; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    rom[0x0000] = 0x12;
    rom[0x0100] = CARTRIDGE_ENTRY_OPCODE;
    rom[0x0104..0x0134].copy_from_slice(&NINTENDO_LOGO);
    rom[0x0134..0x013C].copy_from_slice(b"BOOTREAL");
    rom[0x0143] = 0x00;
    rom[0x0146] = 0x00;
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom[0x014D] = compute_header_checksum(&rom);
    rom
}

fn compute_header_checksum(rom: &[u8]) -> u8 {
    let mut checksum = 0_u8;
    for byte in &rom[0x0134..=0x014C] {
        checksum = checksum.wrapping_sub(*byte).wrapping_sub(1);
    }
    checksum
}

fn load_verified_boot_rom_assets(console_model: ConsoleModel) -> Option<BootRomAssets> {
    let workspace_root = workspace_root();
    let Some(root) = discover_boot_rom_store_root(&workspace_root) else {
        eprintln!(
            "skipping ignored test because neither GB_CYCLE_BOOT_ROM_ROOT nor the default boot ROM store is configured"
        );
        return None;
    };
    let Some(kind) = boot_rom_kind_for_console_model(console_model) else {
        panic!("expected a DMG-family console model, got {console_model:?}");
    };
    let image_path = boot_rom_image_path(&root, kind);
    verify_boot_rom_file(&image_path, kind).unwrap_or_else(|_| {
        panic!(
            "verified boot ROM asset should be readable: {}",
            image_path.display()
        )
    });
    Some(
        BootRomAssets::from_directory(&root)
            .unwrap_or_else(|_| panic!("boot ROM assets should load from {}", root.display())),
    )
}

fn step_until_real_boot_handoff(machine: &mut Machine<TraceSummaryBuffer>) {
    for _ in 0..REAL_BOOT_HANDOFF_T_CYCLE_LIMIT {
        if !machine.boot().is_boot_rom_mapped() {
            return;
        }

        machine.step_t_cycle();

        if let CpuExecutionState::DiagnosticTrap { trap } = machine.cpu().execution_state() {
            panic!(
                "real boot trapped before FF50 handoff: {trap:?}\n{}",
                machine.snapshot().render_text()
            );
        }
    }

    let ly = machine.ppu().ly();
    let line_dot = machine.ppu().line_dot();
    let lcdc = machine.read_bus(0xFF40);
    let stat = machine.read_bus(0xFF41);
    let scy = machine.read_bus(0xFF42);
    let ly_readback = machine.read_bus(0xFF44);

    panic!(
        "real boot did not reach the FF50 handoff within {REAL_BOOT_HANDOFF_T_CYCLE_LIMIT} T-cycles\nppu.ly={ly} ppu.line_dot={line_dot} lcdc=0x{lcdc:02X} stat=0x{stat:02X} scy=0x{scy:02X} ly_readback=0x{ly_readback:02X}\n{}",
        machine.snapshot().render_text()
    );
}

fn run_real_boot_validation(console_model: ConsoleModel) {
    let Some(boot_rom_assets) = load_verified_boot_rom_assets(console_model) else {
        return;
    };

    let mut machine = Machine::new_summary(
        MachineConfig::new(console_model)
            .with_startup_mode(StartupMode::RealBoot)
            .with_boot_rom_assets(boot_rom_assets),
    );
    machine
        .load_cartridge(build_real_boot_validation_rom())
        .expect("validation cartridge should load as NoMBC");

    assert!(machine.boot().is_boot_rom_mapped());
    assert_eq!(
        machine.read_bus(0x0000),
        machine.boot().read_boot_rom(0x0000)
    );

    step_until_real_boot_handoff(&mut machine);

    assert!(!machine.boot().is_boot_rom_mapped());
    assert_eq!(machine.cpu().registers().pc, 0x0100);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::FetchOpcode { t_cycle: 0 }
    );
    assert_eq!(machine.cpu().current_opcode(), None);
    assert_eq!(machine.read_bus(0x0000), 0x12);
    assert_eq!(machine.read_bus(0x0100), CARTRIDGE_ENTRY_OPCODE);

    for _ in 0..4 {
        machine.step_t_cycle();
    }

    assert_eq!(machine.cpu().registers().pc, 0x0101);
    assert_eq!(
        machine.cpu().execution_state(),
        CpuExecutionState::DiagnosticTrap {
            trap: CpuDiagnosticTrap::UnsupportedOpcode {
                opcode: CARTRIDGE_ENTRY_OPCODE,
                address: 0x0100,
            },
        }
    );
}

#[test]
#[ignore = "requires verified local dmg0 boot ROM asset under .roms/bootrom or GB_CYCLE_BOOT_ROM_ROOT"]
fn real_boot_with_verified_dmg0_boot_rom_reaches_cartridge_entry_via_ff50_handoff() {
    run_real_boot_validation(ConsoleModel::Dmg0);
}

#[test]
#[ignore = "requires verified local dmg boot ROM asset under .roms/bootrom or GB_CYCLE_BOOT_ROM_ROOT"]
fn real_boot_with_verified_dmg_boot_rom_reaches_cartridge_entry_via_ff50_handoff() {
    run_real_boot_validation(ConsoleModel::Dmg);
}

#[test]
#[ignore = "requires verified local mgb boot ROM asset under .roms/bootrom or GB_CYCLE_BOOT_ROM_ROOT"]
fn real_boot_with_verified_mgb_boot_rom_reaches_cartridge_entry_via_ff50_handoff() {
    run_real_boot_validation(ConsoleModel::Mgb);
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn blargg_curated_suite_passes_from_repo_store() {
    let Some(report) = run_curated_suite(
        &blargg_dmg_repo_gated_suite(),
        "repo-gated blargg suite",
        true,
    ) else {
        return;
    };
    assert!(report.all_passed(), "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn acid_curated_suite_passes_from_repo_store() {
    let Some(report) = run_curated_suite(&acid_dmg_curated_suite(), "curated acid suite", true)
    else {
        return;
    };
    assert!(report.all_non_failing(), "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn mealybug_curated_suite_updates_report_from_repo_store() {
    let suite = mealybug_tearoom_dmg_curated_suite();
    let expected_case_count = suite.cases.len();
    let Some(report) = run_curated_suite(&suite, "curated mealybug suite", true) else {
        return;
    };
    assert_eq!(
        report.family.as_deref(),
        Some("mealybug-tearoom-tests"),
        "{report:#?}"
    );
    assert_eq!(report.cases.len(), expected_case_count, "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn mooneye_curated_suite_updates_report_from_repo_store() {
    let suite = mooneye_acceptance_dmg_curated_suite();
    let expected_case_count = suite.cases.len();
    let Some(report) = run_curated_suite(&suite, "curated mooneye suite", true) else {
        return;
    };
    assert_eq!(report.family.as_deref(), Some("mooneye"), "{report:#?}");
    assert_eq!(report.cases.len(), expected_case_count, "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn daid_curated_suite_updates_report_from_repo_store() {
    let Some(report) = run_curated_suite(&daid_dmg_curated_suite(), "curated daid suite", true)
    else {
        return;
    };
    assert_eq!(report.family.as_deref(), Some("daid"), "{report:#?}");
    assert_eq!(report.cases.len(), 3, "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn cpp_curated_suite_updates_report_from_repo_store() {
    let Some(report) = run_curated_suite(&cpp_dmg_curated_suite(), "curated cpp suite", true)
    else {
        return;
    };
    assert_eq!(report.family.as_deref(), Some("cpp"), "{report:#?}");
    assert_eq!(report.cases.len(), 3, "{report:#?}");
}

#[test]
#[ignore = "requires curated test ROM assets under .roms/test or GB_CYCLE_TEST_ROM_ROOT"]
fn hacktix_curated_suite_updates_report_from_repo_store() {
    let Some(report) =
        run_curated_suite(&hacktix_dmg_curated_suite(), "curated hacktix suite", true)
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
