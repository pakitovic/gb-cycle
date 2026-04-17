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

fn scanline_prefix(snapshot: &PpuSnapshot, len: usize) -> String {
    let produced = usize::from(snapshot.visible_pixels_output);
    let limit = produced.min(len);
    snapshot.current_scanline_pixels[..limit]
        .iter()
        .map(|pixel| char::from(b'0' + *pixel))
        .collect()
}

fn mixed_color_prefix(snapshot: &PpuSnapshot, len: usize) -> String {
    let produced = usize::from(snapshot.visible_pixels_output);
    let limit = produced.min(len);
    snapshot.current_scanline_mixed_colors[..limit]
        .iter()
        .map(|pixel| char::from(b'0' + *pixel))
        .collect()
}

fn framebuffer_row_prefix(ppu: &gb_core::Ppu, row: usize, len: usize) -> String {
    let start = row * 160;
    let end = start + len;
    ppu.framebuffer()[start..end]
        .iter()
        .map(|pixel| char::from(b'0' + *pixel))
        .collect()
}

fn format_window_trace_snapshot(snapshot: &PpuSnapshot) -> String {
    format!(
        concat!(
            "ly={} dot={} mode={:?} vis={} transfer_x={} started={} ",
            "fetcher={:?} stage={:?}/{} transfer_kind={:?} wx(vis/pipeline)={:#04X}/{:#04X} ",
            "bgp(vis/pipeline)={:#04X}/{:#04X} bgp_override={:?}/{} pixels={} mixed={}",
        ),
        snapshot.ly,
        snapshot.line_dot,
        snapshot.mode,
        snapshot.visible_pixels_output,
        snapshot.bg_current_transfer_x,
        snapshot.window_started_this_line,
        snapshot.bg_fetcher_source,
        snapshot.bg_fetcher_stage,
        snapshot.bg_fetcher_stage_dot,
        snapshot.bg_current_transfer_kind,
        snapshot.visible_wx,
        snapshot.pipeline_wx,
        snapshot.visible_bgp,
        snapshot.pipeline_bgp,
        snapshot.dmg_bgp_cpu_commit_output_palette_override,
        snapshot.dmg_bgp_cpu_commit_output_delay_pixels_remaining,
        scanline_prefix(snapshot, 24),
        mixed_color_prefix(snapshot, 24),
    )
}

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_window_timing_line0_left_edge_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_window_timing");
    const RUNNER_CAPTURE_T_CYCLES: u64 = 2_106_720;

    let mut stepped_t_cycles = 0_u64;
    let mut frame_index = 0_u32;
    let mut previous_at_frame_origin = true;
    let mut previous_ly = machine.ppu().snapshot().ly;

    let mut current_line0_writes = Vec::new();
    let mut current_line0_trace = Vec::new();
    let mut current_line0_summary = None;

    let mut last_completed_line0_writes = Vec::new();
    let mut last_completed_line0_trace = Vec::new();
    let mut last_completed_line0_summary = None;

    while stepped_t_cycles < RUNNER_CAPTURE_T_CYCLES {
        machine.step_t_cycle();
        stepped_t_cycles += 1;
        let snapshot = machine.ppu().snapshot();
        let at_frame_origin = snapshot.ly == 0 && snapshot.line_dot == 0;
        if at_frame_origin && !previous_at_frame_origin {
            frame_index = frame_index.wrapping_add(1);
        }
        previous_at_frame_origin = at_frame_origin;

        if let Some(event) = machine.cpu().last_address_event()
            && event.kind == CpuAddressEventKind::Write
            && matches!(
                event.access_address,
                Some(0xFF40 | 0xFF47 | 0xFF4A | 0xFF4B)
            )
            && snapshot.ly == 0
        {
            let address = event
                .access_address
                .expect("filtered MMIO write should have address");
            current_line0_writes.push(format!(
                "frame={} write {:04X}={:02X} at ly={} dot={} mode={:?} vis={}",
                frame_index,
                address,
                machine.read_bus(address),
                snapshot.ly,
                snapshot.line_dot,
                snapshot.mode,
                snapshot.visible_pixels_output,
            ));
        }

        if snapshot.ly == 0 {
            if snapshot.mode == PpuAccessMode::Drawing
                && snapshot.line_dot >= 80
                && snapshot.visible_pixels_output <= 16
            {
                current_line0_trace.push(format!(
                    "frame={} {}",
                    frame_index,
                    format_window_trace_snapshot(&snapshot)
                ));
            }

            if snapshot.mode == PpuAccessMode::HBlank {
                let pixel_prefix = scanline_prefix(&snapshot, 24);
                let mixed_prefix = mixed_color_prefix(&snapshot, 24);
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 0, 24);
                let first_nonzero_pixel = machine.ppu().framebuffer()[..160]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line0_summary = Some(format!(
                    "frame={} line0_pixels={} line0_mixed={} line0_framebuffer={} first_nonzero_framebuffer={:?} window_started={} wx(vis/pipeline)={:#04X}/{:#04X}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_wx,
                    snapshot.pipeline_wx,
                ));
            }
        }

        if previous_ly == 0 && snapshot.ly != 0 {
            last_completed_line0_summary = current_line0_summary.take();
            last_completed_line0_writes.clone_from(&current_line0_writes);
            last_completed_line0_trace.clone_from(&current_line0_trace);
            current_line0_writes.clear();
            current_line0_trace.clear();
        }

        previous_ly = snapshot.ly;
    }

    let final_snapshot = machine.ppu().snapshot();

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    eprintln!(
        "final_snapshot: ly={} dot={} mode={:?} vis={} frame={}",
        final_snapshot.ly,
        final_snapshot.line_dot,
        final_snapshot.mode,
        final_snapshot.visible_pixels_output,
        frame_index,
    );
    eprintln!("last_completed_line0_summary: {last_completed_line0_summary:?}");
    eprintln!("last_completed_line0_register_writes:");
    for write in &last_completed_line0_writes {
        eprintln!("  {write}");
    }
    eprintln!("last_completed_line0_trace:");
    for entry in &last_completed_line0_trace {
        eprintln!("  {entry}");
    }

    assert!(
        last_completed_line0_summary.is_some(),
        "probe should capture the latest completed line 0 before the runner-equivalent stop point"
    );
}

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_window_timing_first_band_summaries() {
    let mut machine = load_mealybug_window_diag_machine("m3_window_timing");
    const RUNNER_CAPTURE_T_CYCLES: u64 = 2_106_720;

    let mut stepped_t_cycles = 0_u64;
    let mut frame_index = 0_u32;
    let mut previous_at_frame_origin = true;
    let mut previous_ly = machine.ppu().snapshot().ly;

    let mut current_line_writes = Vec::new();
    let mut completed_lines = Vec::new();

    while stepped_t_cycles < RUNNER_CAPTURE_T_CYCLES {
        machine.step_t_cycle();
        stepped_t_cycles += 1;
        let snapshot = machine.ppu().snapshot();
        let at_frame_origin = snapshot.ly == 0 && snapshot.line_dot == 0;
        if at_frame_origin && !previous_at_frame_origin {
            frame_index = frame_index.wrapping_add(1);
        }
        previous_at_frame_origin = at_frame_origin;

        if let Some(event) = machine.cpu().last_address_event()
            && event.kind == CpuAddressEventKind::Write
            && matches!(
                event.access_address,
                Some(0xFF40 | 0xFF47 | 0xFF4A | 0xFF4B)
            )
            && snapshot.ly < 16
        {
            let address = event
                .access_address
                .expect("filtered MMIO write should have address");
            current_line_writes.push(format!(
                "frame={} line={} write {:04X}={:02X} at dot={} mode={:?} vis={}",
                frame_index,
                snapshot.ly,
                address,
                machine.read_bus(address),
                snapshot.line_dot,
                snapshot.mode,
                snapshot.visible_pixels_output,
            ));
        }

        if previous_ly < 16 && snapshot.ly != previous_ly {
            let row = usize::from(previous_ly);
            let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), row, 24);
            completed_lines.push(format!(
                "frame={} line={} framebuffer={} window_started={} wx(vis/pipeline)={:#04X}/{:#04X}",
                frame_index,
                previous_ly,
                framebuffer_prefix,
                machine.ppu().snapshot().window_started_this_line,
                machine.ppu().snapshot().visible_wx,
                machine.ppu().snapshot().pipeline_wx,
            ));
            completed_lines.extend(
                current_line_writes
                    .drain(..)
                    .map(|write| format!("  {write}")),
            );
        }

        previous_ly = snapshot.ly;
    }

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    for line in completed_lines {
        eprintln!("{line}");
    }
}

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_window_timing_line8_left_edge_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_window_timing");
    const RUNNER_CAPTURE_T_CYCLES: u64 = 2_106_720;

    let mut stepped_t_cycles = 0_u64;
    let mut frame_index = 0_u32;
    let mut previous_at_frame_origin = true;
    let mut previous_ly = machine.ppu().snapshot().ly;

    let mut current_line8_writes = Vec::new();
    let mut current_line8_trace = Vec::new();
    let mut current_line8_summary = None;

    let mut last_completed_line8_writes = Vec::new();
    let mut last_completed_line8_trace = Vec::new();
    let mut last_completed_line8_summary = None;

    while stepped_t_cycles < RUNNER_CAPTURE_T_CYCLES {
        machine.step_t_cycle();
        stepped_t_cycles += 1;
        let snapshot = machine.ppu().snapshot();
        let at_frame_origin = snapshot.ly == 0 && snapshot.line_dot == 0;
        if at_frame_origin && !previous_at_frame_origin {
            frame_index = frame_index.wrapping_add(1);
        }
        previous_at_frame_origin = at_frame_origin;

        if let Some(event) = machine.cpu().last_address_event()
            && event.kind == CpuAddressEventKind::Write
            && matches!(
                event.access_address,
                Some(0xFF40 | 0xFF47 | 0xFF4A | 0xFF4B)
            )
            && snapshot.ly == 8
        {
            let address = event
                .access_address
                .expect("filtered MMIO write should have address");
            current_line8_writes.push(format!(
                "frame={} write {:04X}={:02X} at ly={} dot={} mode={:?} vis={}",
                frame_index,
                address,
                machine.read_bus(address),
                snapshot.ly,
                snapshot.line_dot,
                snapshot.mode,
                snapshot.visible_pixels_output,
            ));
        }

        if snapshot.ly == 8 {
            if snapshot.mode == PpuAccessMode::Drawing
                && snapshot.line_dot >= 80
                && snapshot.visible_pixels_output <= 16
            {
                current_line8_trace.push(format!(
                    "frame={} {}",
                    frame_index,
                    format_window_trace_snapshot(&snapshot)
                ));
            }

            if snapshot.mode == PpuAccessMode::HBlank {
                let pixel_prefix = scanline_prefix(&snapshot, 24);
                let mixed_prefix = mixed_color_prefix(&snapshot, 24);
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 8, 24);
                let first_nonzero_pixel = machine.ppu().framebuffer()[8 * 160..9 * 160]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line8_summary = Some(format!(
                    "frame={} line8_pixels={} line8_mixed={} line8_framebuffer={} first_nonzero_framebuffer={:?} window_started={} wx(vis/pipeline)={:#04X}/{:#04X}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_wx,
                    snapshot.pipeline_wx,
                ));
            }
        }

        if previous_ly == 8 && snapshot.ly != 8 {
            last_completed_line8_summary = current_line8_summary.take();
            last_completed_line8_writes.clone_from(&current_line8_writes);
            last_completed_line8_trace.clone_from(&current_line8_trace);
            current_line8_writes.clear();
            current_line8_trace.clear();
        }

        previous_ly = snapshot.ly;
    }

    let final_snapshot = machine.ppu().snapshot();

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    eprintln!(
        "final_snapshot: ly={} dot={} mode={:?} vis={} frame={}",
        final_snapshot.ly,
        final_snapshot.line_dot,
        final_snapshot.mode,
        final_snapshot.visible_pixels_output,
        frame_index,
    );
    eprintln!("last_completed_line8_summary: {last_completed_line8_summary:?}");
    eprintln!("last_completed_line8_register_writes:");
    for write in &last_completed_line8_writes {
        eprintln!("  {write}");
    }
    eprintln!("last_completed_line8_trace:");
    for entry in &last_completed_line8_trace {
        eprintln!("  {entry}");
    }
}

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_window_timing_line9_left_edge_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_window_timing");
    const RUNNER_CAPTURE_T_CYCLES: u64 = 2_106_720;

    let mut stepped_t_cycles = 0_u64;
    let mut frame_index = 0_u32;
    let mut previous_at_frame_origin = true;
    let mut previous_ly = machine.ppu().snapshot().ly;

    let mut current_line_writes = Vec::new();
    let mut current_line_trace = Vec::new();
    let mut current_line_summary = None;

    let mut last_completed_line_writes = Vec::new();
    let mut last_completed_line_trace = Vec::new();
    let mut last_completed_line_summary = None;

    while stepped_t_cycles < RUNNER_CAPTURE_T_CYCLES {
        machine.step_t_cycle();
        stepped_t_cycles += 1;
        let snapshot = machine.ppu().snapshot();
        let at_frame_origin = snapshot.ly == 0 && snapshot.line_dot == 0;
        if at_frame_origin && !previous_at_frame_origin {
            frame_index = frame_index.wrapping_add(1);
        }
        previous_at_frame_origin = at_frame_origin;

        if let Some(event) = machine.cpu().last_address_event()
            && event.kind == CpuAddressEventKind::Write
            && matches!(
                event.access_address,
                Some(0xFF40 | 0xFF47 | 0xFF4A | 0xFF4B)
            )
            && snapshot.ly == 9
        {
            let address = event
                .access_address
                .expect("filtered MMIO write should have address");
            current_line_writes.push(format!(
                "frame={} write {:04X}={:02X} at ly={} dot={} mode={:?} vis={}",
                frame_index,
                address,
                machine.read_bus(address),
                snapshot.ly,
                snapshot.line_dot,
                snapshot.mode,
                snapshot.visible_pixels_output,
            ));
        }

        if snapshot.ly == 9 {
            if snapshot.mode == PpuAccessMode::Drawing
                && snapshot.line_dot >= 80
                && snapshot.visible_pixels_output <= 16
            {
                current_line_trace.push(format!(
                    "frame={} {}",
                    frame_index,
                    format_window_trace_snapshot(&snapshot)
                ));
            }

            if snapshot.mode == PpuAccessMode::HBlank {
                let pixel_prefix = scanline_prefix(&snapshot, 24);
                let mixed_prefix = mixed_color_prefix(&snapshot, 24);
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 9, 24);
                let first_nonzero_pixel = machine.ppu().framebuffer()[9 * 160..10 * 160]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line_summary = Some(format!(
                    "frame={} line9_pixels={} line9_mixed={} line9_framebuffer={} first_nonzero_framebuffer={:?} window_started={} wx(vis/pipeline)={:#04X}/{:#04X}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_wx,
                    snapshot.pipeline_wx,
                ));
            }
        }

        if previous_ly == 9 && snapshot.ly != 9 {
            last_completed_line_summary = current_line_summary.take();
            last_completed_line_writes.clone_from(&current_line_writes);
            last_completed_line_trace.clone_from(&current_line_trace);
            current_line_writes.clear();
            current_line_trace.clear();
        }

        previous_ly = snapshot.ly;
    }

    let final_snapshot = machine.ppu().snapshot();

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    eprintln!(
        "final_snapshot: ly={} dot={} mode={:?} vis={} frame={}",
        final_snapshot.ly,
        final_snapshot.line_dot,
        final_snapshot.mode,
        final_snapshot.visible_pixels_output,
        frame_index,
    );
    eprintln!("last_completed_line9_summary: {last_completed_line_summary:?}");
    eprintln!("last_completed_line9_register_writes:");
    for write in &last_completed_line_writes {
        eprintln!("  {write}");
    }
    eprintln!("last_completed_line9_trace:");
    for entry in &last_completed_line_trace {
        eprintln!("  {entry}");
    }
}

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_window_timing_line18_left_edge_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_window_timing");
    const RUNNER_CAPTURE_T_CYCLES: u64 = 2_106_720;

    let mut stepped_t_cycles = 0_u64;
    let mut frame_index = 0_u32;
    let mut previous_at_frame_origin = true;
    let mut previous_ly = machine.ppu().snapshot().ly;

    let mut current_line_writes = Vec::new();
    let mut current_line_trace = Vec::new();
    let mut current_line_summary = None;

    let mut last_completed_line_writes = Vec::new();
    let mut last_completed_line_trace = Vec::new();
    let mut last_completed_line_summary = None;

    while stepped_t_cycles < RUNNER_CAPTURE_T_CYCLES {
        machine.step_t_cycle();
        stepped_t_cycles += 1;
        let snapshot = machine.ppu().snapshot();
        let at_frame_origin = snapshot.ly == 0 && snapshot.line_dot == 0;
        if at_frame_origin && !previous_at_frame_origin {
            frame_index = frame_index.wrapping_add(1);
        }
        previous_at_frame_origin = at_frame_origin;

        if let Some(event) = machine.cpu().last_address_event()
            && event.kind == CpuAddressEventKind::Write
            && matches!(
                event.access_address,
                Some(0xFF40 | 0xFF47 | 0xFF4A | 0xFF4B)
            )
            && snapshot.ly == 18
        {
            let address = event
                .access_address
                .expect("filtered MMIO write should have address");
            current_line_writes.push(format!(
                "frame={} write {:04X}={:02X} at ly={} dot={} mode={:?} vis={}",
                frame_index,
                address,
                machine.read_bus(address),
                snapshot.ly,
                snapshot.line_dot,
                snapshot.mode,
                snapshot.visible_pixels_output,
            ));
        }

        if snapshot.ly == 18 {
            if snapshot.mode == PpuAccessMode::Drawing
                && snapshot.line_dot >= 80
                && snapshot.visible_pixels_output <= 20
            {
                current_line_trace.push(format!(
                    "frame={} {}",
                    frame_index,
                    format_window_trace_snapshot(&snapshot)
                ));
            }

            if snapshot.mode == PpuAccessMode::HBlank {
                let pixel_prefix = scanline_prefix(&snapshot, 24);
                let mixed_prefix = mixed_color_prefix(&snapshot, 24);
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 18, 24);
                let first_nonzero_pixel = machine.ppu().framebuffer()[18 * 160..19 * 160]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line_summary = Some(format!(
                    "frame={} line18_pixels={} line18_mixed={} line18_framebuffer={} first_nonzero_framebuffer={:?} window_started={} wx(vis/pipeline)={:#04X}/{:#04X}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_wx,
                    snapshot.pipeline_wx,
                ));
            }
        }

        if previous_ly == 18 && snapshot.ly != 18 {
            last_completed_line_summary = current_line_summary.take();
            last_completed_line_writes.clone_from(&current_line_writes);
            last_completed_line_trace.clone_from(&current_line_trace);
            current_line_writes.clear();
            current_line_trace.clear();
        }

        previous_ly = snapshot.ly;
    }

    let final_snapshot = machine.ppu().snapshot();

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    eprintln!(
        "final_snapshot: ly={} dot={} mode={:?} vis={} frame={}",
        final_snapshot.ly,
        final_snapshot.line_dot,
        final_snapshot.mode,
        final_snapshot.visible_pixels_output,
        frame_index,
    );
    eprintln!("last_completed_line18_summary: {last_completed_line_summary:?}");
    eprintln!("last_completed_line18_register_writes:");
    for write in &last_completed_line_writes {
        eprintln!("  {write}");
    }
    eprintln!("last_completed_line18_trace:");
    for entry in &last_completed_line_trace {
        eprintln!("  {entry}");
    }
}

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_window_timing_line21_left_edge_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_window_timing");
    const RUNNER_CAPTURE_T_CYCLES: u64 = 2_106_720;

    let mut stepped_t_cycles = 0_u64;
    let mut frame_index = 0_u32;
    let mut previous_at_frame_origin = true;
    let mut previous_ly = machine.ppu().snapshot().ly;

    let mut current_line_writes = Vec::new();
    let mut current_line_trace = Vec::new();
    let mut current_line_summary = None;

    let mut last_completed_line_writes = Vec::new();
    let mut last_completed_line_trace = Vec::new();
    let mut last_completed_line_summary = None;

    while stepped_t_cycles < RUNNER_CAPTURE_T_CYCLES {
        machine.step_t_cycle();
        stepped_t_cycles += 1;
        let snapshot = machine.ppu().snapshot();
        let at_frame_origin = snapshot.ly == 0 && snapshot.line_dot == 0;
        if at_frame_origin && !previous_at_frame_origin {
            frame_index = frame_index.wrapping_add(1);
        }
        previous_at_frame_origin = at_frame_origin;

        if let Some(event) = machine.cpu().last_address_event()
            && event.kind == CpuAddressEventKind::Write
            && matches!(
                event.access_address,
                Some(0xFF40 | 0xFF47 | 0xFF4A | 0xFF4B)
            )
            && snapshot.ly == 21
        {
            let address = event
                .access_address
                .expect("filtered MMIO write should have address");
            current_line_writes.push(format!(
                "frame={} write {:04X}={:02X} at ly={} dot={} mode={:?} vis={}",
                frame_index,
                address,
                machine.read_bus(address),
                snapshot.ly,
                snapshot.line_dot,
                snapshot.mode,
                snapshot.visible_pixels_output,
            ));
        }

        if snapshot.ly == 21 {
            if snapshot.mode == PpuAccessMode::Drawing
                && snapshot.line_dot >= 80
                && snapshot.visible_pixels_output <= 24
            {
                current_line_trace.push(format!(
                    "frame={} {}",
                    frame_index,
                    format_window_trace_snapshot(&snapshot)
                ));
            }

            if snapshot.mode == PpuAccessMode::HBlank {
                let pixel_prefix = scanline_prefix(&snapshot, 32);
                let mixed_prefix = mixed_color_prefix(&snapshot, 32);
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 21, 32);
                let first_nonzero_pixel = machine.ppu().framebuffer()[21 * 160..22 * 160]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line_summary = Some(format!(
                    "frame={} line21_pixels={} line21_mixed={} line21_framebuffer={} first_nonzero_framebuffer={:?} window_started={} wx(vis/pipeline)={:#04X}/{:#04X}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_wx,
                    snapshot.pipeline_wx,
                ));
            }
        }

        if previous_ly == 21 && snapshot.ly != 21 {
            last_completed_line_summary = current_line_summary.take();
            last_completed_line_writes.clone_from(&current_line_writes);
            last_completed_line_trace.clone_from(&current_line_trace);
            current_line_writes.clear();
            current_line_trace.clear();
        }

        previous_ly = snapshot.ly;
    }

    let final_snapshot = machine.ppu().snapshot();

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    eprintln!(
        "final_snapshot: ly={} dot={} mode={:?} vis={} frame={}",
        final_snapshot.ly,
        final_snapshot.line_dot,
        final_snapshot.mode,
        final_snapshot.visible_pixels_output,
        frame_index,
    );
    eprintln!("last_completed_line21_summary: {last_completed_line_summary:?}");
    eprintln!("last_completed_line21_register_writes:");
    for write in &last_completed_line_writes {
        eprintln!("  {write}");
    }
    eprintln!("last_completed_line21_trace:");
    for entry in &last_completed_line_trace {
        eprintln!("  {entry}");
    }
}

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_window_timing_wx0_first_band_summaries() {
    let mut machine = load_mealybug_window_diag_machine("m3_window_timing_wx_0");
    const RUNNER_CAPTURE_T_CYCLES: u64 = 2_106_720;

    let mut stepped_t_cycles = 0_u64;
    let mut frame_index = 0_u32;
    let mut previous_at_frame_origin = true;
    let mut previous_ly = machine.ppu().snapshot().ly;

    let mut current_line_writes = Vec::new();
    let mut completed_lines = Vec::new();

    while stepped_t_cycles < RUNNER_CAPTURE_T_CYCLES {
        machine.step_t_cycle();
        stepped_t_cycles += 1;
        let snapshot = machine.ppu().snapshot();
        let at_frame_origin = snapshot.ly == 0 && snapshot.line_dot == 0;
        if at_frame_origin && !previous_at_frame_origin {
            frame_index = frame_index.wrapping_add(1);
        }
        previous_at_frame_origin = at_frame_origin;

        if let Some(event) = machine.cpu().last_address_event()
            && event.kind == CpuAddressEventKind::Write
            && matches!(
                event.access_address,
                Some(0xFF40 | 0xFF47 | 0xFF4A | 0xFF4B)
            )
            && snapshot.ly < 16
        {
            let address = event
                .access_address
                .expect("filtered MMIO write should have address");
            current_line_writes.push(format!(
                "frame={} line={} write {:04X}={:02X} at dot={} mode={:?} vis={}",
                frame_index,
                snapshot.ly,
                address,
                machine.read_bus(address),
                snapshot.line_dot,
                snapshot.mode,
                snapshot.visible_pixels_output,
            ));
        }

        if previous_ly < 16 && snapshot.ly != previous_ly {
            let row = usize::from(previous_ly);
            let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), row, 24);
            completed_lines.push(format!(
                "frame={} line={} framebuffer={} window_started={} wx(vis/pipeline)={:#04X}/{:#04X}",
                frame_index,
                previous_ly,
                framebuffer_prefix,
                machine.ppu().snapshot().window_started_this_line,
                machine.ppu().snapshot().visible_wx,
                machine.ppu().snapshot().pipeline_wx,
            ));
            completed_lines.extend(
                current_line_writes
                    .drain(..)
                    .map(|write| format!("  {write}")),
            );
        }

        previous_ly = snapshot.ly;
    }

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    for line in completed_lines {
        eprintln!("{line}");
    }
}

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_window_timing_wx0_line0_left_edge_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_window_timing_wx_0");
    const RUNNER_CAPTURE_T_CYCLES: u64 = 2_106_720;

    let mut stepped_t_cycles = 0_u64;
    let mut frame_index = 0_u32;
    let mut previous_at_frame_origin = true;
    let mut previous_ly = machine.ppu().snapshot().ly;

    let mut current_line_writes = Vec::new();
    let mut current_line_trace = Vec::new();
    let mut current_line_summary = None;

    let mut last_completed_line_writes = Vec::new();
    let mut last_completed_line_trace = Vec::new();
    let mut last_completed_line_summary = None;

    while stepped_t_cycles < RUNNER_CAPTURE_T_CYCLES {
        machine.step_t_cycle();
        stepped_t_cycles += 1;
        let snapshot = machine.ppu().snapshot();
        let at_frame_origin = snapshot.ly == 0 && snapshot.line_dot == 0;
        if at_frame_origin && !previous_at_frame_origin {
            frame_index = frame_index.wrapping_add(1);
        }
        previous_at_frame_origin = at_frame_origin;

        if let Some(event) = machine.cpu().last_address_event()
            && event.kind == CpuAddressEventKind::Write
            && matches!(
                event.access_address,
                Some(0xFF40 | 0xFF47 | 0xFF4A | 0xFF4B)
            )
            && snapshot.ly == 0
        {
            let address = event
                .access_address
                .expect("filtered MMIO write should have address");
            current_line_writes.push(format!(
                "frame={} write {:04X}={:02X} at ly={} dot={} mode={:?} vis={}",
                frame_index,
                address,
                machine.read_bus(address),
                snapshot.ly,
                snapshot.line_dot,
                snapshot.mode,
                snapshot.visible_pixels_output,
            ));
        }

        if snapshot.ly == 0 {
            if snapshot.mode == PpuAccessMode::Drawing
                && snapshot.line_dot >= 80
                && snapshot.visible_pixels_output <= 24
            {
                current_line_trace.push(format!(
                    "frame={} {}",
                    frame_index,
                    format_window_trace_snapshot(&snapshot)
                ));
            }

            if snapshot.mode == PpuAccessMode::HBlank {
                let pixel_prefix = scanline_prefix(&snapshot, 24);
                let mixed_prefix = mixed_color_prefix(&snapshot, 24);
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 0, 24);
                let first_nonzero_pixel = machine.ppu().framebuffer()[..160]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line_summary = Some(format!(
                    "frame={} line0_pixels={} line0_mixed={} line0_framebuffer={} first_nonzero_framebuffer={:?} window_started={} wx(vis/pipeline)={:#04X}/{:#04X}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_wx,
                    snapshot.pipeline_wx,
                ));
            }
        }

        if previous_ly == 0 && snapshot.ly != 0 {
            last_completed_line_summary = current_line_summary.take();
            last_completed_line_writes.clone_from(&current_line_writes);
            last_completed_line_trace.clone_from(&current_line_trace);
            current_line_writes.clear();
            current_line_trace.clear();
        }

        previous_ly = snapshot.ly;
    }

    let final_snapshot = machine.ppu().snapshot();

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    eprintln!(
        "final_snapshot: ly={} dot={} mode={:?} vis={} frame={}",
        final_snapshot.ly,
        final_snapshot.line_dot,
        final_snapshot.mode,
        final_snapshot.visible_pixels_output,
        frame_index,
    );
    eprintln!("last_completed_line0_summary: {last_completed_line_summary:?}");
    eprintln!("last_completed_line0_register_writes:");
    for write in &last_completed_line_writes {
        eprintln!("  {write}");
    }
    eprintln!("last_completed_line0_trace:");
    for entry in &last_completed_line_trace {
        eprintln!("  {entry}");
    }
}
