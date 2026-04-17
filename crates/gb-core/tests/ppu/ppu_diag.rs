//! Diagnostic-only PPU probes reserved for the remaining window-mechanics work.
//!
//! Policy:
//! - stale diagnostics should be deleted instead of archived here
//! - only add short-lived ignored probes for the remaining window-family blockers
//! - keep this module empty until that final window stage needs ad-hoc instrumentation

#![allow(dead_code)]

use super::*;

const WINDOW_DIAG_TIMEOUT_T_CYCLES: u32 = 5_000_000;

fn resolve_test_rom_path(relative: &str) -> std::path::PathBuf {
    if let Some(root) = std::env::var_os("GB_CYCLE_TEST_ROM_ROOT") {
        return std::path::PathBuf::from(root).join(relative);
    }

    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test")
        .join(relative)
}

fn load_diag_machine(relative_rom_path: &str) -> Machine<gb_core::TraceSummaryBuffer> {
    let rom_path = resolve_test_rom_path(relative_rom_path);
    let rom = std::fs::read(&rom_path).expect("diagnostic ROM should be present");
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("diagnostic ROM should load");
    machine
}

fn load_mealybug_window_diag_machine(rom_name: &str) -> Machine<gb_core::TraceSummaryBuffer> {
    load_diag_machine(&format!("mealybug-tearoom-tests/ppu/{rom_name}.gb"))
}

fn step_until_diag_condition(
    machine: &mut Machine<gb_core::TraceSummaryBuffer>,
    context: &str,
    mut predicate: impl FnMut(&PpuSnapshot) -> bool,
) -> PpuSnapshot {
    for _ in 0..WINDOW_DIAG_TIMEOUT_T_CYCLES {
        let snapshot = machine.ppu().snapshot();
        if predicate(&snapshot) {
            return snapshot;
        }
        machine.step_t_cycle();
    }

    panic!(
        "timed out waiting for {context}; last snapshot={:?}",
        machine.ppu().snapshot()
    );
}

fn step_until_window_diag_point(
    machine: &mut Machine<gb_core::TraceSummaryBuffer>,
    target_ly: u8,
    min_visible_pixels_output: u8,
) -> PpuSnapshot {
    step_until_diag_condition(
        machine,
        &format!("LY={target_ly} with at least {min_visible_pixels_output} visible pixels"),
        |snapshot| {
            snapshot.ly == target_ly
                && snapshot.mode == PpuAccessMode::Drawing
                && snapshot.visible_pixels_output >= min_visible_pixels_output
        },
    )
}
