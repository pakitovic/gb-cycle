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

fn bg_fifo_cached_prefix(snapshot: &PpuSnapshot, len: usize) -> String {
    snapshot
        .bg_fifo_cached_pixels
        .iter()
        .take(len)
        .map(|cached| match cached {
            None => "-".to_string(),
            Some(cached) => format!(
                "{:?}:{:?}:fx{}:px{}:tm{}:tda{:#06X}",
                cached.source,
                cached.origin,
                cached.fetch_x,
                cached.pixel_index,
                u8::from(cached.needs_live_tilemap_refetch),
                cached.tile_data_address,
            ),
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn format_window_trace_snapshot(snapshot: &PpuSnapshot) -> String {
    format!(
        concat!(
            "ly={} dot={} mode={:?} vis={} transfer_x={} started={} ",
            "fetcher={:?} stage={:?}/{} transfer={:?}/{:?}/{:?}/{:?} ",
            "wx(vis/pipeline)={:#04X}/{:#04X} ",
            "lcdc(vis/pipeline)={:#04X}/{:#04X} bg_map(win/bg)={}/{} ",
            "tilemap={:#06X} tiledata={:#06X} tile_index={:#04X} tile_low={:#04X} tile_high={:#04X} ",
            "bgp(vis/pipeline)={:#04X}/{:#04X} bgp_override={:?}/{} ",
            "fifo_len={} fifo_cached={} pixels={} mixed={}",
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
        snapshot.bg_current_transfer_lane,
        snapshot.bg_current_transfer_source_window,
        snapshot.bg_current_transfer_backing,
        snapshot.bg_current_transfer_kind,
        snapshot.visible_wx,
        snapshot.pipeline_wx,
        snapshot.visible_lcdc,
        snapshot.pipeline_lcdc,
        (snapshot.visible_lcdc & 0x40) != 0,
        (snapshot.visible_lcdc & 0x08) != 0,
        snapshot.bg_fetcher_tile_map_address,
        snapshot.bg_fetcher_tile_data_address,
        snapshot.bg_fetcher_tile_index,
        snapshot.bg_fetcher_tile_low,
        snapshot.bg_fetcher_tile_high,
        snapshot.visible_bgp,
        snapshot.pipeline_bgp,
        snapshot.dmg_bgp_cpu_commit_output_palette_override,
        snapshot.dmg_bgp_cpu_commit_output_delay_pixels_remaining,
        snapshot.bg_fifo_pixels.len(),
        bg_fifo_cached_prefix(snapshot, 8),
        scanline_prefix(snapshot, 24),
        mixed_color_prefix(snapshot, 24),
    )
}

fn format_window_map_samples(machine: &mut Machine<gb_core::TraceSummaryBuffer>) -> String {
    let mut bus = |address| machine.read_bus(address);
    format!(
        "9800=[{:02X},{:02X},{:02X},{:02X}] 9C00=[{:02X},{:02X},{:02X},{:02X}] tile0=[{:02X},{:02X},{:02X},{:02X},{:02X},{:02X},{:02X},{:02X}] tile1=[{:02X},{:02X},{:02X},{:02X},{:02X},{:02X},{:02X},{:02X}]",
        bus(0x9860),
        bus(0x9861),
        bus(0x9862),
        bus(0x9863),
        bus(0x9C60),
        bus(0x9C61),
        bus(0x9C62),
        bus(0x9C63),
        bus(0x9000),
        bus(0x9001),
        bus(0x9002),
        bus(0x9003),
        bus(0x9004),
        bus(0x9005),
        bus(0x9006),
        bus(0x9007),
        bus(0x9010),
        bus(0x9011),
        bus(0x9012),
        bus(0x9013),
        bus(0x9014),
        bus(0x9015),
        bus(0x9016),
        bus(0x9017),
    )
}

fn format_window_map_row_samples(
    machine: &mut Machine<gb_core::TraceSummaryBuffer>,
    map_row: u8,
) -> String {
    let row_offset = u16::from(map_row) * 32;
    let mut bus = |address| machine.read_bus(address);
    format!(
        "row={} 9800=[{:02X},{:02X},{:02X},{:02X}] 9C00=[{:02X},{:02X},{:02X},{:02X}]",
        map_row,
        bus(0x9800 + row_offset),
        bus(0x9801 + row_offset),
        bus(0x9802 + row_offset),
        bus(0x9803 + row_offset),
        bus(0x9C00 + row_offset),
        bus(0x9C01 + row_offset),
        bus(0x9C02 + row_offset),
        bus(0x9C03 + row_offset),
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

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_lcdc_win_map_change_first_band_summaries() {
    let mut machine = load_mealybug_window_diag_machine("m3_lcdc_win_map_change");
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
                "frame={} line={} write {:04X}={:02X} at dot={} mode={:?} vis={} lcdc(vis/pipeline)={:#04X}/{:#04X}",
                frame_index,
                snapshot.ly,
                address,
                machine.read_bus(address),
                snapshot.line_dot,
                snapshot.mode,
                snapshot.visible_pixels_output,
                snapshot.visible_lcdc,
                snapshot.pipeline_lcdc,
            ));
        }

        if previous_ly < 16 && snapshot.ly != previous_ly {
            let row = usize::from(previous_ly);
            let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), row, 24);
            completed_lines.push(format!(
                "frame={} line={} framebuffer={} window_started={} lcdc(vis/pipeline)={:#04X}/{:#04X}",
                frame_index,
                previous_ly,
                framebuffer_prefix,
                machine.ppu().snapshot().window_started_this_line,
                machine.ppu().snapshot().visible_lcdc,
                machine.ppu().snapshot().pipeline_lcdc,
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
fn diag_m3_lcdc_win_map_change_line0_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_lcdc_win_map_change");
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
                    "frame={} line0_pixels={} line0_mixed={} line0_framebuffer={} first_nonzero_framebuffer={:?} window_started={} lcdc(vis/pipeline)={:#04X}/{:#04X}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_lcdc,
                    snapshot.pipeline_lcdc,
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

    eprintln!("capture_t_cycles={stepped_t_cycles}");
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

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_lcdc_win_map_change_line8_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_lcdc_win_map_change");
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
            && snapshot.ly == 8
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

        if snapshot.ly == 8 {
            if snapshot.mode == PpuAccessMode::Drawing
                && snapshot.line_dot >= 80
                && snapshot.visible_pixels_output <= 32
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
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 8, 32);
                let first_nonzero_pixel = machine.ppu().framebuffer()[8 * 160..9 * 160]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line_summary = Some(format!(
                    "frame={} line8_pixels={} line8_mixed={} line8_framebuffer={} first_nonzero_framebuffer={:?} window_started={} lcdc(vis/pipeline)={:#04X}/{:#04X}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_lcdc,
                    snapshot.pipeline_lcdc,
                ));
            }
        }

        if previous_ly == 8 && snapshot.ly != 8 {
            last_completed_line_summary = current_line_summary.take();
            last_completed_line_writes.clone_from(&current_line_writes);
            last_completed_line_trace.clone_from(&current_line_trace);
            current_line_writes.clear();
            current_line_trace.clear();
        }

        previous_ly = snapshot.ly;
    }

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    eprintln!("last_completed_line8_summary: {last_completed_line_summary:?}");
    eprintln!("last_completed_line8_register_writes:");
    for write in &last_completed_line_writes {
        eprintln!("  {write}");
    }
    eprintln!("last_completed_line8_trace:");
    for entry in &last_completed_line_trace {
        eprintln!("  {entry}");
    }
}

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_lcdc_win_map_change_line16_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_lcdc_win_map_change");
    const RUNNER_CAPTURE_T_CYCLES: u64 = 2_106_720;

    let mut stepped_t_cycles = 0_u64;
    let mut frame_index = 0_u32;
    let mut previous_at_frame_origin = true;
    let mut previous_ly = machine.ppu().snapshot().ly;

    let mut current_line_writes = Vec::new();
    let mut current_line_trace = Vec::new();
    let mut current_line_summary = None;

    let mut last_completed_line_summary = None;
    let mut last_completed_line_writes = Vec::new();
    let mut last_completed_line_trace = Vec::new();

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
            && snapshot.ly == 16
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

        if snapshot.ly == 16 {
            if snapshot.mode == PpuAccessMode::Drawing
                && snapshot.line_dot >= 80
                && snapshot.visible_pixels_output <= 32
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
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 16, 32);
                let row_start = 16 * 160;
                let row_end = row_start + 160;
                let first_nonzero_pixel = machine.ppu().framebuffer()[row_start..row_end]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line_summary = Some(format!(
                    "frame={} line16_pixels={} line16_mixed={} line16_framebuffer={} first_nonzero_framebuffer={:?} window_started={} lcdc(vis/pipeline)={:#04X}/{:#04X}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_lcdc,
                    snapshot.pipeline_lcdc,
                ));
            }
        }

        if previous_ly == 16 && snapshot.ly != 16 {
            last_completed_line_summary = current_line_summary.take();
            last_completed_line_writes.clone_from(&current_line_writes);
            last_completed_line_trace.clone_from(&current_line_trace);
            current_line_writes.clear();
            current_line_trace.clear();
        }

        previous_ly = snapshot.ly;
    }

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    eprintln!("last_completed_line16_summary: {last_completed_line_summary:?}");
    eprintln!("last_completed_line16_register_writes:");
    for write in &last_completed_line_writes {
        eprintln!("  {write}");
    }
    eprintln!("last_completed_line16_trace:");
    for entry in &last_completed_line_trace {
        eprintln!("  {entry}");
    }
}

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_lcdc_win_map_change_line24_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_lcdc_win_map_change");
    const RUNNER_CAPTURE_T_CYCLES: u64 = 2_106_720;

    let mut stepped_t_cycles = 0_u64;
    let mut frame_index = 0_u32;
    let mut previous_at_frame_origin = true;
    let mut previous_ly = machine.ppu().snapshot().ly;

    let mut current_line_writes = Vec::new();
    let mut current_line_trace = Vec::new();
    let mut current_line_summary = None;

    let mut last_completed_line_summary = None;
    let mut last_completed_line_writes = Vec::new();
    let mut last_completed_line_trace = Vec::new();

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
            && snapshot.ly == 24
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

        if snapshot.ly == 24 {
            if snapshot.mode == PpuAccessMode::Drawing
                && snapshot.line_dot >= 80
                && snapshot.visible_pixels_output <= 32
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
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 24, 32);
                let row_start = 24 * 160;
                let row_end = row_start + 160;
                let first_nonzero_pixel = machine.ppu().framebuffer()[row_start..row_end]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line_summary = Some(format!(
                    "frame={} line24_pixels={} line24_mixed={} line24_framebuffer={} first_nonzero_framebuffer={:?} window_started={} lcdc(vis/pipeline)={:#04X}/{:#04X} {}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_lcdc,
                    snapshot.pipeline_lcdc,
                    format_window_map_samples(&mut machine),
                ));
            }
        }

        if previous_ly == 24 && snapshot.ly != 24 {
            last_completed_line_summary = current_line_summary.take();
            last_completed_line_writes.clone_from(&current_line_writes);
            last_completed_line_trace.clone_from(&current_line_trace);
            current_line_writes.clear();
            current_line_trace.clear();
        }

        previous_ly = snapshot.ly;
    }

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    eprintln!("last_completed_line24_summary: {last_completed_line_summary:?}");
    eprintln!("last_completed_line24_register_writes:");
    for write in &last_completed_line_writes {
        eprintln!("  {write}");
    }
    eprintln!("last_completed_line24_trace:");
    for entry in &last_completed_line_trace {
        eprintln!("  {entry}");
    }
}

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_lcdc_win_map_change_line25_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_lcdc_win_map_change");
    const RUNNER_CAPTURE_T_CYCLES: u64 = 2_106_720;

    let mut stepped_t_cycles = 0_u64;
    let mut frame_index = 0_u32;
    let mut previous_at_frame_origin = true;
    let mut previous_ly = machine.ppu().snapshot().ly;

    let mut current_line_writes = Vec::new();
    let mut current_line_trace = Vec::new();
    let mut current_line_summary = None;

    let mut last_completed_line_summary = None;
    let mut last_completed_line_writes = Vec::new();
    let mut last_completed_line_trace = Vec::new();

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
            && snapshot.ly == 25
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

        if snapshot.ly == 25 {
            if snapshot.mode == PpuAccessMode::Drawing
                && snapshot.line_dot >= 80
                && snapshot.visible_pixels_output <= 32
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
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 25, 32);
                let row_start = 25 * 160;
                let row_end = row_start + 160;
                let first_nonzero_pixel = machine.ppu().framebuffer()[row_start..row_end]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line_summary = Some(format!(
                    "frame={} line25_pixels={} line25_mixed={} line25_framebuffer={} first_nonzero_framebuffer={:?} window_started={} lcdc(vis/pipeline)={:#04X}/{:#04X} {}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_lcdc,
                    snapshot.pipeline_lcdc,
                    format_window_map_samples(&mut machine),
                ));
            }
        }

        if previous_ly == 25 && snapshot.ly != 25 {
            last_completed_line_summary = current_line_summary.take();
            last_completed_line_writes.clone_from(&current_line_writes);
            last_completed_line_trace.clone_from(&current_line_trace);
            current_line_writes.clear();
            current_line_trace.clear();
        }

        previous_ly = snapshot.ly;
    }

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    eprintln!("last_completed_line25_summary: {last_completed_line_summary:?}");
    eprintln!("last_completed_line25_register_writes:");
    for write in &last_completed_line_writes {
        eprintln!("  {write}");
    }
    eprintln!("last_completed_line25_trace:");
    for entry in &last_completed_line_trace {
        eprintln!("  {entry}");
    }
}

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_lcdc_win_map_change_line32_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_lcdc_win_map_change");
    const RUNNER_CAPTURE_T_CYCLES: u64 = 2_106_720;

    let mut stepped_t_cycles = 0_u64;
    let mut frame_index = 0_u32;
    let mut previous_at_frame_origin = true;
    let mut previous_ly = machine.ppu().snapshot().ly;

    let mut current_line_writes = Vec::new();
    let mut current_line_trace = Vec::new();
    let mut current_line_summary = None;

    let mut last_completed_line_summary = None;
    let mut last_completed_line_writes = Vec::new();
    let mut last_completed_line_trace = Vec::new();

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
            && snapshot.ly == 32
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

        if snapshot.ly == 32 {
            if snapshot.mode == PpuAccessMode::Drawing
                && snapshot.line_dot >= 80
                && snapshot.visible_pixels_output <= 32
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
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 32, 32);
                let row_start = 32 * 160;
                let row_end = row_start + 160;
                let first_nonzero_pixel = machine.ppu().framebuffer()[row_start..row_end]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line_summary = Some(format!(
                    "frame={} line32_pixels={} line32_mixed={} line32_framebuffer={} first_nonzero_framebuffer={:?} window_started={} lcdc(vis/pipeline)={:#04X}/{:#04X} {}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_lcdc,
                    snapshot.pipeline_lcdc,
                    format_window_map_row_samples(&mut machine, 4),
                ));
            }
        }

        if previous_ly == 32 && snapshot.ly != 32 {
            last_completed_line_summary = current_line_summary.take();
            last_completed_line_writes.clone_from(&current_line_writes);
            last_completed_line_trace.clone_from(&current_line_trace);
            current_line_writes.clear();
            current_line_trace.clear();
        }

        previous_ly = snapshot.ly;
    }

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    eprintln!("last_completed_line32_summary: {last_completed_line_summary:?}");
    eprintln!("last_completed_line32_register_writes:");
    for write in &last_completed_line_writes {
        eprintln!("  {write}");
    }
    eprintln!("last_completed_line32_trace:");
    for entry in &last_completed_line_trace {
        eprintln!("  {entry}");
    }
}

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_lcdc_win_map_change_line40_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_lcdc_win_map_change");
    const RUNNER_CAPTURE_T_CYCLES: u64 = 2_106_720;

    let mut stepped_t_cycles = 0_u64;
    let mut frame_index = 0_u32;
    let mut previous_at_frame_origin = true;
    let mut previous_ly = machine.ppu().snapshot().ly;

    let mut current_line_writes = Vec::new();
    let mut current_line_trace = Vec::new();
    let mut current_line_summary = None;

    let mut last_completed_line_summary = None;
    let mut last_completed_line_writes = Vec::new();
    let mut last_completed_line_trace = Vec::new();

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
            && snapshot.ly == 40
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

        if snapshot.ly == 40 {
            if snapshot.mode == PpuAccessMode::Drawing
                && snapshot.line_dot >= 80
                && snapshot.visible_pixels_output <= 32
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
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 40, 32);
                let row_start = 40 * 160;
                let row_end = row_start + 160;
                let first_nonzero_pixel = machine.ppu().framebuffer()[row_start..row_end]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line_summary = Some(format!(
                    "frame={} line40_pixels={} line40_mixed={} line40_framebuffer={} first_nonzero_framebuffer={:?} window_started={} lcdc(vis/pipeline)={:#04X}/{:#04X} {}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_lcdc,
                    snapshot.pipeline_lcdc,
                    format_window_map_row_samples(&mut machine, 5),
                ));
            }
        }

        if previous_ly == 40 && snapshot.ly != 40 {
            last_completed_line_summary = current_line_summary.take();
            last_completed_line_writes.clone_from(&current_line_writes);
            last_completed_line_trace.clone_from(&current_line_trace);
            current_line_writes.clear();
            current_line_trace.clear();
        }

        previous_ly = snapshot.ly;
    }

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    eprintln!("last_completed_line40_summary: {last_completed_line_summary:?}");
    eprintln!("last_completed_line40_register_writes:");
    for write in &last_completed_line_writes {
        eprintln!("  {write}");
    }
    eprintln!("last_completed_line40_trace:");
    for entry in &last_completed_line_trace {
        eprintln!("  {entry}");
    }
}

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_lcdc_win_map_change_line64_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_lcdc_win_map_change");
    const RUNNER_CAPTURE_T_CYCLES: u64 = 2_106_720;

    let mut stepped_t_cycles = 0_u64;
    let mut frame_index = 0_u32;
    let mut previous_at_frame_origin = true;
    let mut previous_ly = machine.ppu().snapshot().ly;

    let mut current_line_writes = Vec::new();
    let mut current_line_trace = Vec::new();
    let mut current_line_summary = None;

    let mut last_completed_line_summary = None;
    let mut last_completed_line_writes = Vec::new();
    let mut last_completed_line_trace = Vec::new();

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
            && snapshot.ly == 64
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

        if snapshot.ly == 64 {
            if snapshot.mode == PpuAccessMode::Drawing
                && snapshot.line_dot >= 80
                && snapshot.visible_pixels_output <= 32
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
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 64, 32);
                let row_start = 64 * 160;
                let row_end = row_start + 160;
                let first_nonzero_pixel = machine.ppu().framebuffer()[row_start..row_end]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line_summary = Some(format!(
                    "frame={} line64_pixels={} line64_mixed={} line64_framebuffer={} first_nonzero_framebuffer={:?} window_started={} lcdc(vis/pipeline)={:#04X}/{:#04X} {}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_lcdc,
                    snapshot.pipeline_lcdc,
                    format_window_map_row_samples(&mut machine, 8),
                ));
            }
        }

        if previous_ly == 64 && snapshot.ly != 64 {
            last_completed_line_summary = current_line_summary.take();
            last_completed_line_writes.clone_from(&current_line_writes);
            last_completed_line_trace.clone_from(&current_line_trace);
            current_line_writes.clear();
            current_line_trace.clear();
        }

        previous_ly = snapshot.ly;
    }

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    eprintln!("last_completed_line64_summary: {last_completed_line_summary:?}");
    eprintln!("last_completed_line64_register_writes:");
    for write in &last_completed_line_writes {
        eprintln!("  {write}");
    }
    eprintln!("last_completed_line64_trace:");
    for entry in &last_completed_line_trace {
        eprintln!("  {entry}");
    }
}

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_lcdc_win_en_change_multiple_line0_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_lcdc_win_en_change_multiple");
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
                && snapshot.visible_pixels_output <= 96
            {
                current_line_trace.push(format!(
                    "frame={} {}",
                    frame_index,
                    format_window_trace_snapshot(&snapshot)
                ));
            }

            if snapshot.mode == PpuAccessMode::HBlank {
                let pixel_prefix = scanline_prefix(&snapshot, 96);
                let mixed_prefix = mixed_color_prefix(&snapshot, 96);
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 0, 96);
                let first_nonzero_pixel = machine.ppu().framebuffer()[..160]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line_summary = Some(format!(
                    "frame={} line0_pixels={} line0_mixed={} line0_framebuffer={} first_nonzero_framebuffer={:?} window_started={} lcdc(vis/pipeline)={:#04X}/{:#04X}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_lcdc,
                    snapshot.pipeline_lcdc,
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

    eprintln!("capture_t_cycles={stepped_t_cycles}");
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

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_lcdc_win_en_change_multiple_line1_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_lcdc_win_en_change_multiple");
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
            && snapshot.ly == 1
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

        if snapshot.ly == 1 {
            if snapshot.mode == PpuAccessMode::Drawing
                && snapshot.line_dot >= 80
                && snapshot.visible_pixels_output <= 96
            {
                current_line_trace.push(format!(
                    "frame={} {}",
                    frame_index,
                    format_window_trace_snapshot(&snapshot)
                ));
            }

            if snapshot.mode == PpuAccessMode::HBlank {
                let pixel_prefix = scanline_prefix(&snapshot, 96);
                let mixed_prefix = mixed_color_prefix(&snapshot, 96);
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 1, 96);
                let first_nonzero_pixel = machine.ppu().framebuffer()[160..320]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line_summary = Some(format!(
                    "frame={} line1_pixels={} line1_mixed={} line1_framebuffer={} first_nonzero_framebuffer={:?} window_started={} lcdc(vis/pipeline)={:#04X}/{:#04X}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_lcdc,
                    snapshot.pipeline_lcdc,
                ));
            }
        }

        if previous_ly == 1 && snapshot.ly != 1 {
            last_completed_line_summary = current_line_summary.take();
            last_completed_line_writes.clone_from(&current_line_writes);
            last_completed_line_trace.clone_from(&current_line_trace);
            current_line_writes.clear();
            current_line_trace.clear();
        }

        previous_ly = snapshot.ly;
    }

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    eprintln!("last_completed_line1_summary: {last_completed_line_summary:?}");
    eprintln!("last_completed_line1_register_writes:");
    for write in &last_completed_line_writes {
        eprintln!("  {write}");
    }
    eprintln!("last_completed_line1_trace:");
    for entry in &last_completed_line_trace {
        eprintln!("  {entry}");
    }
}

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_lcdc_win_en_change_multiple_line2_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_lcdc_win_en_change_multiple");
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
            && snapshot.ly == 2
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

        if snapshot.ly == 2 {
            if snapshot.mode == PpuAccessMode::Drawing
                && snapshot.line_dot >= 80
                && snapshot.visible_pixels_output <= 96
            {
                current_line_trace.push(format!(
                    "frame={} {}",
                    frame_index,
                    format_window_trace_snapshot(&snapshot)
                ));
            }

            if snapshot.mode == PpuAccessMode::HBlank {
                let pixel_prefix = scanline_prefix(&snapshot, 96);
                let mixed_prefix = mixed_color_prefix(&snapshot, 96);
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 2, 96);
                let first_nonzero_pixel = machine.ppu().framebuffer()[320..480]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line_summary = Some(format!(
                    "frame={} line2_pixels={} line2_mixed={} line2_framebuffer={} first_nonzero_framebuffer={:?} window_started={} lcdc(vis/pipeline)={:#04X}/{:#04X}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_lcdc,
                    snapshot.pipeline_lcdc,
                ));
            }
        }

        if previous_ly == 2 && snapshot.ly != 2 {
            last_completed_line_summary = current_line_summary.take();
            last_completed_line_writes.clone_from(&current_line_writes);
            last_completed_line_trace.clone_from(&current_line_trace);
            current_line_writes.clear();
            current_line_trace.clear();
        }

        previous_ly = snapshot.ly;
    }

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    eprintln!("last_completed_line2_summary: {last_completed_line_summary:?}");
    eprintln!("last_completed_line2_register_writes:");
    for write in &last_completed_line_writes {
        eprintln!("  {write}");
    }
    eprintln!("last_completed_line2_trace:");
    for entry in &last_completed_line_trace {
        eprintln!("  {entry}");
    }
}

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_lcdc_win_en_change_multiple_wx_line0_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_lcdc_win_en_change_multiple_wx");
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
                && snapshot.visible_pixels_output <= 96
            {
                current_line_trace.push(format!(
                    "frame={} {}",
                    frame_index,
                    format_window_trace_snapshot(&snapshot)
                ));
            }

            if snapshot.mode == PpuAccessMode::HBlank {
                let pixel_prefix = scanline_prefix(&snapshot, 96);
                let mixed_prefix = mixed_color_prefix(&snapshot, 96);
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 0, 96);
                let first_nonzero_pixel = machine.ppu().framebuffer()[..160]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line_summary = Some(format!(
                    "frame={} line0_pixels={} line0_mixed={} line0_framebuffer={} first_nonzero_framebuffer={:?} window_started={} lcdc(vis/pipeline)={:#04X}/{:#04X}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_lcdc,
                    snapshot.pipeline_lcdc,
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

    eprintln!("capture_t_cycles={stepped_t_cycles}");
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

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_lcdc_tile_sel_win_change_line16_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_lcdc_tile_sel_win_change");
    const RUNNER_CAPTURE_T_CYCLES: u64 = 2_106_720;

    let mut stepped_t_cycles = 0_u64;
    let mut frame_index = 0_u32;
    let mut previous_at_frame_origin = true;
    let mut previous_ly = machine.ppu().snapshot().ly;

    let mut current_line_writes = Vec::new();
    let mut current_line_trace = Vec::new();
    let mut current_line_summary = None;

    let mut last_completed_line_summary = None;
    let mut last_completed_line_writes = Vec::new();
    let mut last_completed_line_trace = Vec::new();

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
            && snapshot.ly == 16
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

        if snapshot.ly == 16 {
            if snapshot.mode == PpuAccessMode::Drawing
                && snapshot.line_dot >= 80
                && snapshot.visible_pixels_output <= 32
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
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 16, 32);
                let row_start = 16 * 160;
                let row_end = row_start + 160;
                let first_nonzero_pixel = machine.ppu().framebuffer()[row_start..row_end]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line_summary = Some(format!(
                    "frame={} line16_pixels={} line16_mixed={} line16_framebuffer={} first_nonzero_framebuffer={:?} window_started={} lcdc(vis/pipeline)={:#04X}/{:#04X} {} {} {}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_lcdc,
                    snapshot.pipeline_lcdc,
                    format_window_map_samples(&mut machine),
                    format_window_map_row_samples(&mut machine, 0),
                    format_window_map_row_samples(&mut machine, 1),
                ));
            }
        }

        if previous_ly == 16 && snapshot.ly != 16 {
            last_completed_line_summary = current_line_summary.take();
            last_completed_line_writes.clone_from(&current_line_writes);
            last_completed_line_trace.clone_from(&current_line_trace);
            current_line_writes.clear();
            current_line_trace.clear();
        }

        previous_ly = snapshot.ly;
    }

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    eprintln!("last_completed_line16_summary: {last_completed_line_summary:?}");
    eprintln!("last_completed_line16_register_writes:");
    for write in &last_completed_line_writes {
        eprintln!("  {write}");
    }
    eprintln!("last_completed_line16_trace:");
    for entry in &last_completed_line_trace {
        eprintln!("  {entry}");
    }
}

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_lcdc_tile_sel_win_change_line8_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_lcdc_tile_sel_win_change");
    const RUNNER_CAPTURE_T_CYCLES: u64 = 2_106_720;

    let mut stepped_t_cycles = 0_u64;
    let mut frame_index = 0_u32;
    let mut previous_at_frame_origin = true;
    let mut previous_ly = machine.ppu().snapshot().ly;

    let mut current_line_writes = Vec::new();
    let mut current_line_trace = Vec::new();
    let mut current_line_summary = None;

    let mut last_completed_line_summary = None;
    let mut last_completed_line_writes = Vec::new();
    let mut last_completed_line_trace = Vec::new();

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

        if snapshot.ly == 8 {
            if snapshot.mode == PpuAccessMode::Drawing
                && snapshot.line_dot >= 80
                && snapshot.visible_pixels_output <= 32
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
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 8, 32);
                let row_start = 8 * 160;
                let row_end = row_start + 160;
                let first_nonzero_pixel = machine.ppu().framebuffer()[row_start..row_end]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line_summary = Some(format!(
                    "frame={} line8_pixels={} line8_mixed={} line8_framebuffer={} first_nonzero_framebuffer={:?} window_started={} lcdc(vis/pipeline)={:#04X}/{:#04X} {} {} {}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_lcdc,
                    snapshot.pipeline_lcdc,
                    format_window_map_samples(&mut machine),
                    format_window_map_row_samples(&mut machine, 0),
                    format_window_map_row_samples(&mut machine, 1),
                ));
            }
        }

        if previous_ly == 8 && snapshot.ly != 8 {
            last_completed_line_summary = current_line_summary.take();
            last_completed_line_writes.clone_from(&current_line_writes);
            last_completed_line_trace.clone_from(&current_line_trace);
            current_line_writes.clear();
            current_line_trace.clear();
        }

        previous_ly = snapshot.ly;
    }

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    eprintln!("last_completed_line8_summary: {last_completed_line_summary:?}");
    eprintln!("last_completed_line8_register_writes:");
    for write in &last_completed_line_writes {
        eprintln!("  {write}");
    }
    eprintln!("last_completed_line8_trace:");
    for entry in &last_completed_line_trace {
        eprintln!("  {entry}");
    }
}

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_lcdc_tile_sel_win_change_line24_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_lcdc_tile_sel_win_change");
    const RUNNER_CAPTURE_T_CYCLES: u64 = 2_106_720;

    let mut stepped_t_cycles = 0_u64;
    let mut frame_index = 0_u32;
    let mut previous_at_frame_origin = true;
    let mut previous_ly = machine.ppu().snapshot().ly;

    let mut current_line_writes = Vec::new();
    let mut current_line_trace = Vec::new();
    let mut current_line_summary = None;

    let mut last_completed_line_summary = None;
    let mut last_completed_line_writes = Vec::new();
    let mut last_completed_line_trace = Vec::new();

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
            && snapshot.ly == 24
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

        if snapshot.ly == 24 {
            if snapshot.mode == PpuAccessMode::Drawing
                && snapshot.line_dot >= 80
                && snapshot.visible_pixels_output <= 32
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
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 24, 32);
                let row_start = 24 * 160;
                let row_end = row_start + 160;
                let first_nonzero_pixel = machine.ppu().framebuffer()[row_start..row_end]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line_summary = Some(format!(
                    "frame={} line24_pixels={} line24_mixed={} line24_framebuffer={} first_nonzero_framebuffer={:?} window_started={} lcdc(vis/pipeline)={:#04X}/{:#04X} {} {} {}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_lcdc,
                    snapshot.pipeline_lcdc,
                    format_window_map_samples(&mut machine),
                    format_window_map_row_samples(&mut machine, 0),
                    format_window_map_row_samples(&mut machine, 1),
                ));
            }
        }

        if previous_ly == 24 && snapshot.ly != 24 {
            last_completed_line_summary = current_line_summary.take();
            last_completed_line_writes.clone_from(&current_line_writes);
            last_completed_line_trace.clone_from(&current_line_trace);
            current_line_writes.clear();
            current_line_trace.clear();
        }

        previous_ly = snapshot.ly;
    }

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    eprintln!("last_completed_line24_summary: {last_completed_line_summary:?}");
    eprintln!("last_completed_line24_register_writes:");
    for write in &last_completed_line_writes {
        eprintln!("  {write}");
    }
    eprintln!("last_completed_line24_trace:");
    for entry in &last_completed_line_trace {
        eprintln!("  {entry}");
    }
}

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_lcdc_tile_sel_win_change_line32_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_lcdc_tile_sel_win_change");
    const RUNNER_CAPTURE_T_CYCLES: u64 = 2_106_720;

    let mut stepped_t_cycles = 0_u64;
    let mut frame_index = 0_u32;
    let mut previous_at_frame_origin = true;
    let mut previous_ly = machine.ppu().snapshot().ly;

    let mut current_line_writes = Vec::new();
    let mut current_line_trace = Vec::new();
    let mut current_line_summary = None;

    let mut last_completed_line_summary = None;
    let mut last_completed_line_writes = Vec::new();
    let mut last_completed_line_trace = Vec::new();

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
            && snapshot.ly == 32
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

        if snapshot.ly == 32 {
            if snapshot.mode == PpuAccessMode::Drawing
                && snapshot.line_dot >= 80
                && snapshot.visible_pixels_output <= 32
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
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 32, 32);
                let row_start = 32 * 160;
                let row_end = row_start + 160;
                let first_nonzero_pixel = machine.ppu().framebuffer()[row_start..row_end]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line_summary = Some(format!(
                    "frame={} line32_pixels={} line32_mixed={} line32_framebuffer={} first_nonzero_framebuffer={:?} window_started={} lcdc(vis/pipeline)={:#04X}/{:#04X} {} {} {}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_lcdc,
                    snapshot.pipeline_lcdc,
                    format_window_map_samples(&mut machine),
                    format_window_map_row_samples(&mut machine, 0),
                    format_window_map_row_samples(&mut machine, 1),
                ));
            }
        }

        if previous_ly == 32 && snapshot.ly != 32 {
            last_completed_line_summary = current_line_summary.take();
            last_completed_line_writes.clone_from(&current_line_writes);
            last_completed_line_trace.clone_from(&current_line_trace);
            current_line_writes.clear();
            current_line_trace.clear();
        }

        previous_ly = snapshot.ly;
    }

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    eprintln!("last_completed_line32_summary: {last_completed_line_summary:?}");
    eprintln!("last_completed_line32_register_writes:");
    for write in &last_completed_line_writes {
        eprintln!("  {write}");
    }
    eprintln!("last_completed_line32_trace:");
    for entry in &last_completed_line_trace {
        eprintln!("  {entry}");
    }
}

#[test]
#[ignore = "diagnostic-only probe for the remaining window blocker"]
fn diag_m3_lcdc_tile_sel_win_change_line40_trace() {
    let mut machine = load_mealybug_window_diag_machine("m3_lcdc_tile_sel_win_change");
    const RUNNER_CAPTURE_T_CYCLES: u64 = 2_106_720;

    let mut stepped_t_cycles = 0_u64;
    let mut frame_index = 0_u32;
    let mut previous_at_frame_origin = true;
    let mut previous_ly = machine.ppu().snapshot().ly;

    let mut current_line_writes = Vec::new();
    let mut current_line_trace = Vec::new();
    let mut current_line_summary = None;

    let mut last_completed_line_summary = None;
    let mut last_completed_line_writes = Vec::new();
    let mut last_completed_line_trace = Vec::new();

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
            && snapshot.ly == 40
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

        if snapshot.ly == 40 {
            if snapshot.mode == PpuAccessMode::Drawing
                && snapshot.line_dot >= 80
                && snapshot.visible_pixels_output <= 32
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
                let framebuffer_prefix = framebuffer_row_prefix(machine.ppu(), 40, 32);
                let row_start = 40 * 160;
                let row_end = row_start + 160;
                let first_nonzero_pixel = machine.ppu().framebuffer()[row_start..row_end]
                    .iter()
                    .position(|pixel| *pixel != 0);
                current_line_summary = Some(format!(
                    "frame={} line40_pixels={} line40_mixed={} line40_framebuffer={} first_nonzero_framebuffer={:?} window_started={} lcdc(vis/pipeline)={:#04X}/{:#04X} {} {} {}",
                    frame_index,
                    pixel_prefix,
                    mixed_prefix,
                    framebuffer_prefix,
                    first_nonzero_pixel,
                    snapshot.window_started_this_line,
                    snapshot.visible_lcdc,
                    snapshot.pipeline_lcdc,
                    format_window_map_samples(&mut machine),
                    format_window_map_row_samples(&mut machine, 0),
                    format_window_map_row_samples(&mut machine, 1),
                ));
            }
        }

        if previous_ly == 40 && snapshot.ly != 40 {
            last_completed_line_summary = current_line_summary.take();
            last_completed_line_writes.clone_from(&current_line_writes);
            last_completed_line_trace.clone_from(&current_line_trace);
            current_line_writes.clear();
            current_line_trace.clear();
        }

        previous_ly = snapshot.ly;
    }

    eprintln!("capture_t_cycles={stepped_t_cycles}");
    eprintln!("last_completed_line40_summary: {last_completed_line_summary:?}");
    eprintln!("last_completed_line40_register_writes:");
    for write in &last_completed_line_writes {
        eprintln!("  {write}");
    }
    eprintln!("last_completed_line40_trace:");
    for entry in &last_completed_line_trace {
        eprintln!("  {entry}");
    }
}
