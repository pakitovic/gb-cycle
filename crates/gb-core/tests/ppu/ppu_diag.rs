//! Diagnostic-only PPU probes.
//!
//! Policy:
//! - stable, cheap oracles stay active in the owning family module
//! - ignored ad-hoc probes live here and use `#[ignore = "diag: ..."]`
//! - stale probes should be deleted instead of preserved as historical noise

use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
struct DaidPpuScanlineBgpLineObservation {
    raw_pixels_prefix: [u8; 32],
    panel_pixels_prefix: [u8; 32],
    visible_bgp_writes: Vec<(u16, u8, u8)>,
}

const DAID_SCANLINE_BGP_ACCEPTED_LY23_PANEL_PREFIXES: [[u8; 32]; 3] = [
    [
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
        1, 1,
    ],
    [
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 3, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3,
        1, 1,
    ],
    [
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1,
        1, 1,
    ],
];

fn resolve_test_rom_path(relative: &str) -> std::path::PathBuf {
    if let Some(root) = std::env::var_os("GB_CYCLE_TEST_ROM_ROOT") {
        return std::path::PathBuf::from(root).join(relative);
    }

    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test")
        .join(relative)
}

fn load_daid_ppu_scanline_bgp_machine() -> Machine<gb_core::TraceSummaryBuffer> {
    let rom_path = resolve_test_rom_path("daid/ppu_scanline_bgp.gb");
    let rom = std::fs::read(&rom_path).expect("daid ppu_scanline_bgp ROM should be present");
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("diagnostic ROM should load");
    machine
}

fn load_mealybug_m3_scx_low_3_bits_machine() -> Machine<gb_core::TraceSummaryBuffer> {
    let rom_path = resolve_test_rom_path("mealybug-tearoom-tests/ppu/m3_scx_low_3_bits.gb");
    let rom = std::fs::read(&rom_path).expect("mealybug m3_scx_low_3_bits ROM should be present");
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("diagnostic ROM should load");
    machine
}

fn load_mealybug_m3_scx_high_5_bits_machine() -> Machine<gb_core::TraceSummaryBuffer> {
    let rom_path = resolve_test_rom_path("mealybug-tearoom-tests/ppu/m3_scx_high_5_bits.gb");
    let rom = std::fs::read(&rom_path).expect("mealybug m3_scx_high_5_bits ROM should be present");
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("diagnostic ROM should load");
    machine
}

fn log_mealybug_m3_scx_high_5_bits_hblank_row_after_target_write(target_ly: u8) {
    let mut machine = load_mealybug_m3_scx_high_5_bits_machine();
    let mut armed = false;

    for _ in 0..20_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();
        if !armed
            && before.ly == target_ly
            && before.mode == PpuAccessMode::Drawing
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF43)
        {
            let activity = before_cpu
                .last_bus_activity
                .expect("FF43 write should expose a bus activity snapshot");
            println!(
                "arm ly={} line_dot={} x={} vpo={} scx={:#04X} visible_scx={:#04X} stage={:?} stage_dot={} startup={:?}",
                before.ly,
                before.line_dot,
                before.bg_current_transfer_x,
                before.visible_pixels_output,
                activity.value,
                before.visible_scx,
                before.bg_fetcher_stage,
                before.bg_fetcher_stage_dot,
                before.bg_startup_fetch_seam,
            );
            armed = true;
        }

        machine.step_t_cycle();

        if !armed {
            continue;
        }

        let after = machine.ppu().snapshot();
        if after.ly != target_ly {
            break;
        }

        if after.mode == PpuAccessMode::HBlank {
            let scy = machine.read_bus(0xFF42);
            let bg_row = after.ly.wrapping_add(scy) & 0x07;
            let row_start = after.ly as usize * 160;
            let cols = [
                2_u16, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
            ];
            let entries = cols
                .into_iter()
                .map(|col| {
                    let tile = machine.read_bus(0x9800 + col);
                    let low =
                        machine.read_bus(0x8000 + u16::from(tile) * 16 + u16::from(bg_row) * 2);
                    let high =
                        machine.read_bus(0x8000 + u16::from(tile) * 16 + u16::from(bg_row) * 2 + 1);
                    (col, tile, low, high)
                })
                .collect::<Vec<_>>();
            println!(
                "hblank ly={} mode0={} bg_row={} entries={:?} raw_16_31={:?} panel_16_31={:?}",
                after.ly,
                after.mode0_start_dot,
                bg_row,
                entries,
                &after.current_scanline_pixels[16..32],
                &machine.ppu().framebuffer()[row_start + 16..row_start + 32],
            );
            return;
        }
    }

    panic!("timed out before sampling the HBlank row after the target FF43 write");
}

fn run_live_scx_previsible_probe(start_scx: u8, live_write: Option<(u16, u8)>) -> PpuSnapshot {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.write_bus(0xFF40, 0x91);
    machine.write_bus(0xFF43, start_scx);

    seed_bg_tile_row(&mut machine, 0, 0, 0x55, 0x33);
    seed_bg_tile_row(&mut machine, 1, 0, 0xAA, 0xCC);
    seed_bg_tile_row(&mut machine, 2, 0, 0xF0, 0x00);
    seed_bg_tile_row(&mut machine, 3, 0, 0x00, 0xF0);
    for tile_x in 0..32 {
        seed_bg_tilemap_entry(&mut machine, tile_x, 0, tile_x % 4);
    }

    if let Some((target_line_dot, value)) = live_write {
        step_until_line_dot(&mut machine, target_line_dot);
        machine.write_bus(0xFF43, value);
    }

    step_until_hblank(&mut machine);
    machine.ppu().snapshot()
}

fn run_scx_last_column_probe(start_scx: u8) -> PpuSnapshot {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.write_bus(0xFF40, 0x91);
    machine.write_bus(0xFF43, start_scx);

    seed_bg_tile_row(&mut machine, 0, 0, 0x00, 0x00);
    seed_bg_tile_row(&mut machine, 0x19, 0, 0xFF, 0xFF);
    for tile_x in 0..32 {
        seed_bg_tilemap_entry(
            &mut machine,
            tile_x,
            0,
            if tile_x == 19 { 0x19 } else { 0x00 },
        );
    }

    step_until_hblank(&mut machine);
    machine.ppu().snapshot()
}

fn sample_daid_ppu_scanline_bgp_line(target_ly: u8) -> DaidPpuScanlineBgpLineObservation {
    sample_daid_ppu_scanline_bgp_lines(&[target_ly])
        .remove(&target_ly)
        .expect("target line should be sampled")
}

fn sample_daid_ppu_scanline_bgp_lines(
    target_lys: &[u8],
) -> std::collections::BTreeMap<u8, DaidPpuScanlineBgpLineObservation> {
    let mut machine = load_daid_ppu_scanline_bgp_machine();
    let targets = target_lys
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut visible_bgp_writes = std::collections::BTreeMap::<u8, Vec<(u16, u8, u8)>>::new();
    let mut observations = std::collections::BTreeMap::new();
    let mut saw_progress = false;
    let mut wraps = 0usize;

    for _ in 0..5_000_000 {
        machine.step_t_cycle();

        let snapshot = machine.ppu().snapshot();

        if snapshot.ly != 0 || snapshot.line_dot != 0 {
            saw_progress = true;
        } else if saw_progress {
            wraps += 1;
        }

        if let Some(event) = machine.cpu().last_address_event()
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF47)
            && wraps == 1
            && targets.contains(&snapshot.ly)
        {
            visible_bgp_writes.entry(snapshot.ly).or_default().push((
                snapshot.line_dot,
                snapshot.visible_pixels_output,
                machine.read_bus(0xFF47),
            ));
        }

        if wraps == 1
            && targets.contains(&snapshot.ly)
            && snapshot.mode == PpuAccessMode::HBlank
            && !observations.contains_key(&snapshot.ly)
        {
            let mut raw_pixels_prefix = [0_u8; 32];
            raw_pixels_prefix.copy_from_slice(&snapshot.current_scanline_pixels[..32]);

            let framebuffer_row_start = snapshot.ly as usize * 160;
            let mut panel_pixels_prefix = [0_u8; 32];
            panel_pixels_prefix.copy_from_slice(
                &machine.ppu().framebuffer()[framebuffer_row_start..framebuffer_row_start + 32],
            );

            observations.insert(
                snapshot.ly,
                DaidPpuScanlineBgpLineObservation {
                    raw_pixels_prefix,
                    panel_pixels_prefix,
                    visible_bgp_writes: visible_bgp_writes.remove(&snapshot.ly).unwrap_or_default(),
                },
            );
            if observations.len() == targets.len() {
                return observations;
            }
        }
    }

    panic!("timed out before sampling requested daid ppu_scanline_bgp lines");
}

#[test]
#[ignore = "diag: compare startup scx=2 against a previsible live FF43=2 write"]
fn live_scx_previsible_probe_logs_scanline_alignment() {
    let baseline = run_live_scx_previsible_probe(0x00, None);
    let live = run_live_scx_previsible_probe(0x00, Some((88, 0x02)));
    let startup = run_live_scx_previsible_probe(0x02, None);

    println!(
        "baseline first16={:?} tail16={:?} mode0={} vpo={}",
        &baseline.current_scanline_pixels[..16],
        &baseline.current_scanline_pixels[144..160],
        baseline.mode0_start_dot,
        baseline.visible_pixels_output,
    );
    println!(
        "live     first16={:?} tail16={:?} mode0={} vpo={}",
        &live.current_scanline_pixels[..16],
        &live.current_scanline_pixels[144..160],
        live.mode0_start_dot,
        live.visible_pixels_output,
    );
    println!(
        "startup  first16={:?} tail16={:?} mode0={} vpo={}",
        &startup.current_scanline_pixels[..16],
        &startup.current_scanline_pixels[144..160],
        startup.mode0_start_dot,
        startup.visible_pixels_output,
    );
}

#[test]
#[ignore = "diag: compare last-column visibility for startup scx low-bit offsets"]
fn scx_last_column_probe_logs_tail_visibility() {
    let scx0 = run_scx_last_column_probe(0x00);
    let scx2 = run_scx_last_column_probe(0x02);

    println!("scx0 tail16={:?}", &scx0.current_scanline_pixels[144..160]);
    println!("scx2 tail16={:?}", &scx2.current_scanline_pixels[144..160]);
}

#[test]
#[ignore = "diag: real mealybug m3_scx_low_3_bits FF43 write trace"]
fn real_mealybug_m3_scx_low_3_bits_logs_ff43_writes() {
    let mut machine = load_mealybug_m3_scx_low_3_bits_machine();
    let mut saw_progress = false;
    let mut wraps = 0usize;
    let mut writes_logged = 0usize;

    for _ in 0..15_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        if ppu.ly != 0 || ppu.line_dot != 0 {
            saw_progress = true;
        } else if saw_progress {
            wraps += 1;
        }

        let cpu = machine.cpu().snapshot();
        if let Some(event) = cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF43)
        {
            let activity = cpu
                .last_bus_activity
                .expect("FF43 write should expose a bus activity snapshot");
            println!(
                "wrap={} ly={} line_dot={} mode={:?} mode0_start_dot={} x={} vpo={} scx={:#04X} visible_scx={:#04X} stage={:?} fifo_len={} placeholders={} pc={:#06X}",
                wraps,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_current_transfer_x,
                ppu.visible_pixels_output,
                activity.value,
                ppu.visible_scx,
                ppu.bg_fetcher_stage,
                ppu.bg_fifo_pixels.len(),
                ppu.bg_startup_fifo_placeholders,
                cpu.registers.pc,
            );
            writes_logged += 1;
            if writes_logged >= 160 {
                return;
            }
        }
    }

    panic!("timed out before logging enough FF43 writes");
}

#[test]
#[ignore = "diag: real mealybug m3_scx_low_3_bits FF43 writes around the LY=0x48 cutoff"]
fn real_mealybug_m3_scx_low_3_bits_logs_ff43_writes_near_cutoff() {
    let mut machine = load_mealybug_m3_scx_low_3_bits_machine();

    for _ in 0..15_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        if !(68..=75).contains(&ppu.ly) {
            continue;
        }

        let cpu = machine.cpu().snapshot();
        if let Some(event) = cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF43)
        {
            let activity = cpu
                .last_bus_activity
                .expect("FF43 write should expose a bus activity snapshot");
            println!(
                "ly={} line_dot={} mode={:?} mode0={} x={} vpo={} scx={:#04X} visible_scx={:#04X} stage={:?} stage_dot={} fifo_len={} placeholders={} startup={:?} pc={:#06X}",
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_current_transfer_x,
                ppu.visible_pixels_output,
                activity.value,
                ppu.visible_scx,
                ppu.bg_fetcher_stage,
                ppu.bg_fetcher_stage_dot,
                ppu.bg_fifo_pixels.len(),
                ppu.bg_startup_fifo_placeholders,
                ppu.bg_startup_fetch_seam,
                cpu.registers.pc,
            );

            let _ = machine.step_t_cycle();
            let post_commit = machine.ppu().snapshot();
            println!(
                "  post+1 ly={} line_dot={} mode={:?} mode0={} x={} vpo={} scx={:#04X} visible_scx={:#04X} stage={:?} stage_dot={} fifo_len={} placeholders={} startup={:?}",
                post_commit.ly,
                post_commit.line_dot,
                post_commit.mode,
                post_commit.mode0_start_dot,
                post_commit.bg_current_transfer_x,
                post_commit.visible_pixels_output,
                post_commit.scx,
                post_commit.visible_scx,
                post_commit.bg_fetcher_stage,
                post_commit.bg_fetcher_stage_dot,
                post_commit.bg_fifo_pixels.len(),
                post_commit.bg_startup_fifo_placeholders,
                post_commit.bg_startup_fetch_seam,
            );
        }
    }

    panic!("timed out before sampling the LY cutoff window");
}

#[test]
#[ignore = "diag: real mealybug m3_scx_low_3_bits tail pixels near the cutoff"]
fn real_mealybug_m3_scx_low_3_bits_logs_tail_pixels_near_cutoff() {
    let mut machine = load_mealybug_m3_scx_low_3_bits_machine();
    let mut sampled = std::collections::BTreeSet::new();

    for _ in 0..15_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        if !(68..=75).contains(&ppu.ly) || ppu.mode != PpuAccessMode::HBlank {
            continue;
        }
        if !sampled.insert(ppu.ly) {
            continue;
        }

        let row_start = ppu.ly as usize * 160;
        println!(
            "ly={} mode0={} raw_tail={:?} panel_tail={:?}",
            ppu.ly,
            ppu.mode0_start_dot,
            &ppu.current_scanline_pixels[148..160],
            &machine.ppu().framebuffer()[row_start + 148..row_start + 160],
        );

        if sampled.len() == 8 {
            return;
        }
    }

    panic!("timed out before sampling HBlank tails near the cutoff");
}

#[test]
#[ignore = "diag: real mealybug m3_scx_low_3_bits tail on the same line as a successful FF43 write"]
fn real_mealybug_m3_scx_low_3_bits_logs_same_line_tail_after_successful_write() {
    let mut machine = load_mealybug_m3_scx_low_3_bits_machine();
    let mut armed_lys = std::collections::BTreeSet::new();
    let mut sampled_lys = std::collections::BTreeSet::new();

    for _ in 0..15_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        let cpu = machine.cpu().snapshot();

        if let Some(event) = cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF43)
            && (72..=75).contains(&ppu.ly)
            && ppu.line_dot == 84
        {
            armed_lys.insert(ppu.ly);
        }

        if ppu.mode == PpuAccessMode::HBlank
            && armed_lys.remove(&ppu.ly)
            && sampled_lys.insert(ppu.ly)
        {
            let row_start = ppu.ly as usize * 160;
            let lcdc = machine.read_bus(0xFF40);
            let bg_row = ppu.ly.wrapping_add(machine.read_bus(0xFF42)) & 0x07;
            let tile19 = machine.read_bus(0x9800 + 19);
            let tile20 = machine.read_bus(0x9800 + 20);
            let tile19_low =
                machine.read_bus(0x8000 + u16::from(tile19) * 16 + u16::from(bg_row) * 2);
            let tile19_high =
                machine.read_bus(0x8000 + u16::from(tile19) * 16 + u16::from(bg_row) * 2 + 1);
            let tile20_low =
                machine.read_bus(0x8000 + u16::from(tile20) * 16 + u16::from(bg_row) * 2);
            let tile20_high =
                machine.read_bus(0x8000 + u16::from(tile20) * 16 + u16::from(bg_row) * 2 + 1);
            let tile19_signed_base =
                0x9000u16.wrapping_add((tile19 as i8 as i16 as u16).wrapping_mul(16));
            let tile20_signed_base =
                0x9000u16.wrapping_add((tile20 as i8 as i16 as u16).wrapping_mul(16));
            let tile19_signed_low = machine.read_bus(tile19_signed_base + u16::from(bg_row) * 2);
            let tile19_signed_high =
                machine.read_bus(tile19_signed_base + u16::from(bg_row) * 2 + 1);
            let tile20_signed_low = machine.read_bus(tile20_signed_base + u16::from(bg_row) * 2);
            let tile20_signed_high =
                machine.read_bus(tile20_signed_base + u16::from(bg_row) * 2 + 1);
            println!(
                "ly={} lcdc={:#04X} mode0={} scy={} bg_row={} tile19={:#04X} tile19_row=({:#04X},{:#04X}) tile19_signed_row=({:#04X},{:#04X}) tile20={:#04X} tile20_row=({:#04X},{:#04X}) tile20_signed_row=({:#04X},{:#04X}) raw_tail={:?} panel_tail={:?}",
                ppu.ly,
                lcdc,
                ppu.mode0_start_dot,
                machine.read_bus(0xFF42),
                bg_row,
                tile19,
                tile19_low,
                tile19_high,
                tile19_signed_low,
                tile19_signed_high,
                tile20,
                tile20_low,
                tile20_high,
                tile20_signed_low,
                tile20_signed_high,
                &ppu.current_scanline_pixels[148..160],
                &machine.ppu().framebuffer()[row_start + 148..row_start + 160],
            );
            if ppu.ly == 72 {
                let tile0_rows: Vec<_> = (0u8..8)
                    .map(|row| {
                        (
                            machine.read_bus(0x8000 + u16::from(row) * 2),
                            machine.read_bus(0x8000 + u16::from(row) * 2 + 1),
                        )
                    })
                    .collect();
                let tile19_rows: Vec<_> = (0u8..8)
                    .map(|row| {
                        let base = 0x8000 + 0x19 * 16 + u16::from(row) * 2;
                        (machine.read_bus(base), machine.read_bus(base + 1))
                    })
                    .collect();
                println!("tile0_rows={tile0_rows:?} tile19_rows={tile19_rows:?}");
            }

            if sampled_lys.len() == 4 {
                return;
            }
        }
    }

    panic!("timed out before sampling same-line HBlank tails after the successful writes");
}

#[test]
#[ignore = "diag: real mealybug m3_scx_low_3_bits host-forced SCX=2 early in the line"]
fn real_mealybug_m3_scx_low_3_bits_logs_host_forced_scx2_tail() {
    let mut machine = load_mealybug_m3_scx_low_3_bits_machine();
    let mut forced = false;

    for _ in 0..15_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        if !forced && ppu.ly == 72 && ppu.line_dot == 33 {
            machine.write_bus(0xFF43, 0x02);
            forced = true;
        }

        if forced && ppu.ly == 72 && ppu.mode == PpuAccessMode::HBlank {
            let row_start = ppu.ly as usize * 160;
            println!(
                "ly={} mode0={} raw_tail={:?} panel_tail={:?}",
                ppu.ly,
                ppu.mode0_start_dot,
                &ppu.current_scanline_pixels[148..160],
                &machine.ppu().framebuffer()[row_start + 148..row_start + 160],
            );
            return;
        }
    }

    panic!("timed out before sampling the host-forced SCX=2 line");
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits FF43 writes and x16..31 output"]
fn real_mealybug_m3_scx_high_5_bits_logs_ff43_writes_and_tail() {
    let mut machine = load_mealybug_m3_scx_high_5_bits_machine();
    let mut sampled_lys = std::collections::BTreeSet::new();

    for _ in 0..20_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        let cpu = machine.cpu().snapshot();

        if let Some(event) = cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF43)
            && (24..=38).contains(&ppu.ly)
        {
            let activity = cpu
                .last_bus_activity
                .expect("FF43 write should expose a bus activity snapshot");
            println!(
                "write ly={} line_dot={} mode={:?} mode0={} x={} vpo={} scx={:#04X} visible_scx={:#04X} stage={:?} stage_dot={} fifo_len={} placeholders={} startup={:?} push_pending={} fill_pending={} front_cached={:?} push_cached={:?} fill_cached={:?} pc={:#06X}",
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_current_transfer_x,
                ppu.visible_pixels_output,
                activity.value,
                ppu.visible_scx,
                ppu.bg_fetcher_stage,
                ppu.bg_fetcher_stage_dot,
                ppu.bg_fifo_pixels.len(),
                ppu.bg_startup_fifo_placeholders,
                ppu.bg_startup_fetch_seam,
                ppu.bg_push_pending,
                ppu.bg_fill_pending,
                ppu.bg_fifo_cached_pixels.first(),
                ppu.bg_fifo_cached_pixels
                    .get(ppu.bg_fifo_pixels.is_empty() as usize),
                ppu.bg_fill_pending
                    .then(|| ppu.bg_fifo_cached_pixels.last()),
                cpu.registers.pc,
            );

            let _ = machine.step_t_cycle();
            let post_commit = machine.ppu().snapshot();
            println!(
                "  post+1 ly={} line_dot={} mode={:?} mode0={} x={} vpo={} scx={:#04X} visible_scx={:#04X} stage={:?} stage_dot={} fifo_len={} placeholders={} startup={:?} push_pending={} fill_pending={} front_cached={:?}",
                post_commit.ly,
                post_commit.line_dot,
                post_commit.mode,
                post_commit.mode0_start_dot,
                post_commit.bg_current_transfer_x,
                post_commit.visible_pixels_output,
                post_commit.scx,
                post_commit.visible_scx,
                post_commit.bg_fetcher_stage,
                post_commit.bg_fetcher_stage_dot,
                post_commit.bg_fifo_pixels.len(),
                post_commit.bg_startup_fifo_placeholders,
                post_commit.bg_startup_fetch_seam,
                post_commit.bg_push_pending,
                post_commit.bg_fill_pending,
                post_commit.bg_fifo_cached_pixels.first(),
            );
        }

        if (24..=38).contains(&ppu.ly)
            && ppu.mode == PpuAccessMode::HBlank
            && sampled_lys.insert(ppu.ly)
        {
            let row_start = ppu.ly as usize * 160;
            if ppu.ly == 24 {
                let scy = machine.read_bus(0xFF42);
                let bg_row = ppu.ly.wrapping_add(scy) & 0x07;
                let tile4 = machine.read_bus(0x9864);
                let tile5 = machine.read_bus(0x9865);
                let tile6 = machine.read_bus(0x9866);
                let tile4_low =
                    machine.read_bus(0x8000 + u16::from(tile4) * 16 + u16::from(bg_row) * 2);
                let tile4_high =
                    machine.read_bus(0x8000 + u16::from(tile4) * 16 + u16::from(bg_row) * 2 + 1);
                let tile5_low =
                    machine.read_bus(0x8000 + u16::from(tile5) * 16 + u16::from(bg_row) * 2);
                let tile5_high =
                    machine.read_bus(0x8000 + u16::from(tile5) * 16 + u16::from(bg_row) * 2 + 1);
                let tile6_low =
                    machine.read_bus(0x8000 + u16::from(tile6) * 16 + u16::from(bg_row) * 2);
                let tile6_high =
                    machine.read_bus(0x8000 + u16::from(tile6) * 16 + u16::from(bg_row) * 2 + 1);
                let tile70_low = machine.read_bus(0x8000 + 0x46 * 16 + u16::from(bg_row) * 2);
                let tile70_high = machine.read_bus(0x8000 + 0x46 * 16 + u16::from(bg_row) * 2 + 1);
                let tile71_low = machine.read_bus(0x8000 + 0x47 * 16 + u16::from(bg_row) * 2);
                let tile71_high = machine.read_bus(0x8000 + 0x47 * 16 + u16::from(bg_row) * 2 + 1);
                println!(
                    "line ly={} mode0={} bg_row={} tile4={:#04X} row4=({:#04X},{:#04X}) tile5={:#04X} row5=({:#04X},{:#04X}) tile6={:#04X} row6=({:#04X},{:#04X}) tile70=({:#04X},{:#04X}) tile71=({:#04X},{:#04X}) raw_16_31={:?} panel_16_31={:?}",
                    ppu.ly,
                    ppu.mode0_start_dot,
                    bg_row,
                    tile4,
                    tile4_low,
                    tile4_high,
                    tile5,
                    tile5_low,
                    tile5_high,
                    tile6,
                    tile6_low,
                    tile6_high,
                    tile70_low,
                    tile70_high,
                    tile71_low,
                    tile71_high,
                    &ppu.current_scanline_pixels[16..32],
                    &machine.ppu().framebuffer()[row_start + 16..row_start + 32],
                );
            } else {
                println!(
                    "line ly={} mode0={} raw_16_31={:?} panel_16_31={:?}",
                    ppu.ly,
                    ppu.mode0_start_dot,
                    &ppu.current_scanline_pixels[16..32],
                    &machine.ppu().framebuffer()[row_start + 16..row_start + 32],
                );
            }

            if sampled_lys.len() == 15 {
                return;
            }
        }
    }

    panic!("timed out before sampling the target FF43 writes / HBlank rows");
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits FF43 write chronology"]
fn real_mealybug_m3_scx_high_5_bits_logs_ff43_write_chronology() {
    let mut machine = load_mealybug_m3_scx_high_5_bits_machine();
    let mut writes_logged = 0usize;

    for _ in 0..20_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        let cpu = machine.cpu().snapshot();
        if let Some(event) = cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF43)
        {
            let activity = cpu
                .last_bus_activity
                .expect("FF43 write should expose a bus activity snapshot");
            println!(
                "write#{} ly={} line_dot={} mode={:?} mode0={} x={} vpo={} scx={:#04X} visible_scx={:#04X} stage={:?} stage_dot={} fifo_len={} placeholders={} startup={:?} front_cached={:?} pc={:#06X}",
                writes_logged,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_current_transfer_x,
                ppu.visible_pixels_output,
                activity.value,
                ppu.visible_scx,
                ppu.bg_fetcher_stage,
                ppu.bg_fetcher_stage_dot,
                ppu.bg_fifo_pixels.len(),
                ppu.bg_startup_fifo_placeholders,
                ppu.bg_startup_fetch_seam,
                ppu.bg_fifo_cached_pixels.first(),
                cpu.registers.pc,
            );
            writes_logged += 1;
            if writes_logged >= 260 {
                return;
            }
        }
    }

    panic!("timed out before logging enough FF43 writes");
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly24 after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly24_after_write_window() {
    let mut machine = load_mealybug_m3_scx_high_5_bits_machine();
    let mut armed = false;
    let mut last_vpo = 0u8;

    for _ in 0..20_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();

        if !armed
            && before.ly == 24
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF43)
        {
            armed = true;
            last_vpo = before.visible_pixels_output;
            let activity = before_cpu
                .last_bus_activity
                .expect("FF43 write should expose a bus activity snapshot");
            println!(
                "arm ly={} line_dot={} vpo={} x={} scx={:#04X} visible_scx={:#04X} stage={:?} stage_dot={} front_cached={:?}",
                before.ly,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                activity.value,
                before.visible_scx,
                before.bg_fetcher_stage,
                before.bg_fetcher_stage_dot,
                before.bg_fifo_cached_pixels.first(),
            );
        }

        machine.step_t_cycle();

        if !armed {
            continue;
        }

        let after = machine.ppu().snapshot();
        if after.ly != 24 {
            break;
        }

        if after.visible_pixels_output != last_vpo {
            let visible_x = last_vpo as usize;
            println!(
                "emit line_dot={} vpo={} -> {} x={} mixed={} panel={} stage={:?} stage_dot={} fetch_map={:#06X} fetch_data={:#06X} push_pending={} push_front={:?} fill_pending={} front_cached={:?}",
                after.line_dot,
                last_vpo,
                after.visible_pixels_output,
                visible_x,
                after.current_scanline_pixels[visible_x],
                machine.ppu().framebuffer()[after.ly as usize * 160 + visible_x],
                after.bg_fetcher_stage,
                after.bg_fetcher_stage_dot,
                after.bg_fetcher_tile_map_address,
                after.bg_fetcher_tile_data_address,
                after.bg_push_pending,
                after.bg_push_pending.then_some({
                    (
                        after.bg_fetcher_tile_map_address,
                        after.bg_fetcher_tile_data_address,
                    )
                }),
                after.bg_fill_pending,
                after.bg_fifo_cached_pixels.first(),
            );
            last_vpo = after.visible_pixels_output;
            if after.visible_pixels_output >= 24 {
                return;
            }
        }
    }

    panic!("timed out before logging the LY=24 post-write output window");
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly9 after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly9_after_write_window() {
    let mut machine = load_mealybug_m3_scx_high_5_bits_machine();
    let mut armed = false;
    let mut last_vpo = 0u8;

    for _ in 0..20_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();

        if !armed
            && before.ly == 9
            && before.mode == PpuAccessMode::Drawing
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF43)
        {
            armed = true;
            last_vpo = before.visible_pixels_output;
            let activity = before_cpu
                .last_bus_activity
                .expect("FF43 write should expose a bus activity snapshot");
            println!(
                "arm ly={} line_dot={} vpo={} x={} scx={:#04X} visible_scx={:#04X} stage={:?} stage_dot={} front_cached={:?}",
                before.ly,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                activity.value,
                before.visible_scx,
                before.bg_fetcher_stage,
                before.bg_fetcher_stage_dot,
                before.bg_fifo_cached_pixels.first(),
            );
        }

        machine.step_t_cycle();

        if !armed {
            continue;
        }

        let after = machine.ppu().snapshot();
        if after.ly != 9 {
            break;
        }

        if after.visible_pixels_output != last_vpo {
            let visible_x = last_vpo as usize;
            println!(
                "emit line_dot={} vpo={} -> {} x={} mixed={} panel={} stage={:?} stage_dot={} fetch_map={:#06X} fetch_data={:#06X} push_pending={} fill_pending={} front_cached={:?}",
                after.line_dot,
                last_vpo,
                after.visible_pixels_output,
                visible_x,
                after.current_scanline_pixels[visible_x],
                machine.ppu().framebuffer()[after.ly as usize * 160 + visible_x],
                after.bg_fetcher_stage,
                after.bg_fetcher_stage_dot,
                after.bg_fetcher_tile_map_address,
                after.bg_fetcher_tile_data_address,
                after.bg_push_pending,
                after.bg_fill_pending,
                after.bg_fifo_cached_pixels.first(),
            );
            last_vpo = after.visible_pixels_output;
            if after.visible_pixels_output >= 24 {
                return;
            }
        }
    }

    panic!("timed out before logging the LY=9 post-write output window");
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly11 after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly11_after_write_window() {
    let mut machine = load_mealybug_m3_scx_high_5_bits_machine();
    let mut armed = false;
    let mut last_vpo = 0u8;

    for _ in 0..20_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();

        if !armed
            && before.ly == 11
            && before.mode == PpuAccessMode::Drawing
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF43)
        {
            armed = true;
            last_vpo = before.visible_pixels_output;
            let activity = before_cpu
                .last_bus_activity
                .expect("FF43 write should expose a bus activity snapshot");
            println!(
                "arm ly={} line_dot={} vpo={} x={} scx={:#04X} visible_scx={:#04X} stage={:?} stage_dot={} front_cached={:?}",
                before.ly,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                activity.value,
                before.visible_scx,
                before.bg_fetcher_stage,
                before.bg_fetcher_stage_dot,
                before.bg_fifo_cached_pixels.first(),
            );
        }

        machine.step_t_cycle();

        if !armed {
            continue;
        }

        let after = machine.ppu().snapshot();
        if after.ly != 11 {
            break;
        }

        if after.visible_pixels_output != last_vpo {
            let visible_x = last_vpo as usize;
            println!(
                "emit line_dot={} vpo={} -> {} x={} mixed={} panel={} stage={:?} stage_dot={} fetch_map={:#06X} fetch_data={:#06X} push_pending={} fill_pending={} front_cached={:?}",
                after.line_dot,
                last_vpo,
                after.visible_pixels_output,
                visible_x,
                after.current_scanline_pixels[visible_x],
                machine.ppu().framebuffer()[after.ly as usize * 160 + visible_x],
                after.bg_fetcher_stage,
                after.bg_fetcher_stage_dot,
                after.bg_fetcher_tile_map_address,
                after.bg_fetcher_tile_data_address,
                after.bg_push_pending,
                after.bg_fill_pending,
                after.bg_fifo_cached_pixels.first(),
            );
            last_vpo = after.visible_pixels_output;
            if after.visible_pixels_output >= 24 {
                return;
            }
        }
    }

    panic!("timed out before logging the LY=11 post-write output window");
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly33 after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly33_after_write_window() {
    let mut machine = load_mealybug_m3_scx_high_5_bits_machine();
    let mut armed = false;
    let mut last_vpo = 0u8;

    for _ in 0..20_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();

        if !armed
            && before.ly == 33
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF43)
        {
            armed = true;
            last_vpo = before.visible_pixels_output;
            let activity = before_cpu
                .last_bus_activity
                .expect("FF43 write should expose a bus activity snapshot");
            println!(
                "arm ly={} line_dot={} vpo={} x={} scx={:#04X} visible_scx={:#04X} stage={:?} stage_dot={} front_cached={:?}",
                before.ly,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                activity.value,
                before.visible_scx,
                before.bg_fetcher_stage,
                before.bg_fetcher_stage_dot,
                before.bg_fifo_cached_pixels.first(),
            );
        }

        machine.step_t_cycle();

        if !armed {
            continue;
        }

        let after = machine.ppu().snapshot();
        if after.ly != 33 {
            break;
        }

        if after.visible_pixels_output != last_vpo {
            let visible_x = last_vpo as usize;
            println!(
                "emit line_dot={} vpo={} -> {} x={} mixed={} panel={} stage={:?} stage_dot={} fetch_map={:#06X} fetch_data={:#06X} push_pending={} fill_pending={} front_cached={:?}",
                after.line_dot,
                last_vpo,
                after.visible_pixels_output,
                visible_x,
                after.current_scanline_pixels[visible_x],
                machine.ppu().framebuffer()[after.ly as usize * 160 + visible_x],
                after.bg_fetcher_stage,
                after.bg_fetcher_stage_dot,
                after.bg_fetcher_tile_map_address,
                after.bg_fetcher_tile_data_address,
                after.bg_push_pending,
                after.bg_fill_pending,
                after.bg_fifo_cached_pixels.first(),
            );
            last_vpo = after.visible_pixels_output;
            if after.visible_pixels_output >= 28 {
                return;
            }
        }
    }

    panic!("timed out before logging the LY=33 post-write output window");
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly72 after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly72_after_write_window() {
    let mut machine = load_mealybug_m3_scx_high_5_bits_machine();
    let mut armed = false;
    let mut last_vpo = 0u8;

    for _ in 0..20_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();

        if !armed
            && before.ly == 72
            && before.mode == PpuAccessMode::Drawing
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF43)
        {
            armed = true;
            last_vpo = before.visible_pixels_output;
            let activity = before_cpu
                .last_bus_activity
                .expect("FF43 write should expose a bus activity snapshot");
            println!(
                "arm ly={} line_dot={} vpo={} x={} scx={:#04X} visible_scx={:#04X} stage={:?} stage_dot={} front_cached={:?}",
                before.ly,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                activity.value,
                before.visible_scx,
                before.bg_fetcher_stage,
                before.bg_fetcher_stage_dot,
                before.bg_fifo_cached_pixels.first(),
            );
        }

        machine.step_t_cycle();

        if !armed {
            continue;
        }

        let after = machine.ppu().snapshot();
        if after.ly != 72 {
            break;
        }

        if after.visible_pixels_output != last_vpo {
            let visible_x = last_vpo as usize;
            println!(
                "emit line_dot={} vpo={} -> {} x={} mixed={} panel={} stage={:?} stage_dot={} fetch_map={:#06X} fetch_data={:#06X} push_pending={} fill_pending={} front_cached={:?}",
                after.line_dot,
                last_vpo,
                after.visible_pixels_output,
                visible_x,
                after.current_scanline_pixels[visible_x],
                machine.ppu().framebuffer()[after.ly as usize * 160 + visible_x],
                after.bg_fetcher_stage,
                after.bg_fetcher_stage_dot,
                after.bg_fetcher_tile_map_address,
                after.bg_fetcher_tile_data_address,
                after.bg_push_pending,
                after.bg_fill_pending,
                after.bg_fifo_cached_pixels.first(),
            );
            last_vpo = after.visible_pixels_output;
            if after.visible_pixels_output >= 24 {
                return;
            }
        }
    }

    panic!("timed out before logging the LY=72 post-write output window");
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly30 after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly30_after_write_window() {
    let mut machine = load_mealybug_m3_scx_high_5_bits_machine();
    let mut armed = false;
    let mut last_vpo = 0u8;

    for _ in 0..20_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();

        if !armed
            && before.ly == 30
            && before.mode == PpuAccessMode::Drawing
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF43)
        {
            armed = true;
            last_vpo = before.visible_pixels_output;
            let activity = before_cpu
                .last_bus_activity
                .expect("FF43 write should expose a bus activity snapshot");
            println!(
                "arm ly={} line_dot={} vpo={} x={} scx={:#04X} visible_scx={:#04X} stage={:?} stage_dot={} front_cached={:?}",
                before.ly,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                activity.value,
                before.visible_scx,
                before.bg_fetcher_stage,
                before.bg_fetcher_stage_dot,
                before.bg_fifo_cached_pixels.first(),
            );
        }

        machine.step_t_cycle();

        if !armed {
            continue;
        }

        let after = machine.ppu().snapshot();
        if after.ly != 30 {
            break;
        }

        if after.visible_pixels_output != last_vpo {
            let visible_x = last_vpo as usize;
            println!(
                "emit line_dot={} vpo={} -> {} x={} mixed={} panel={} stage={:?} stage_dot={} fetch_map={:#06X} fetch_data={:#06X} push_pending={} fill_pending={} front_cached={:?}",
                after.line_dot,
                last_vpo,
                after.visible_pixels_output,
                visible_x,
                after.current_scanline_pixels[visible_x],
                machine.ppu().framebuffer()[after.ly as usize * 160 + visible_x],
                after.bg_fetcher_stage,
                after.bg_fetcher_stage_dot,
                after.bg_fetcher_tile_map_address,
                after.bg_fetcher_tile_data_address,
                after.bg_push_pending,
                after.bg_fill_pending,
                after.bg_fifo_cached_pixels.first(),
            );
            last_vpo = after.visible_pixels_output;
            if after.visible_pixels_output >= 28 {
                return;
            }
        }
    }

    panic!("timed out before logging the LY=30 post-write output window");
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly89 after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly89_after_write_window() {
    let mut machine = load_mealybug_m3_scx_high_5_bits_machine();
    let mut armed = false;
    let mut last_vpo = 0u8;

    for _ in 0..20_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();

        if !armed
            && before.ly == 89
            && before.mode == PpuAccessMode::Drawing
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF43)
        {
            armed = true;
            last_vpo = before.visible_pixels_output;
            let activity = before_cpu
                .last_bus_activity
                .expect("FF43 write should expose a bus activity snapshot");
            println!(
                "arm ly={} line_dot={} vpo={} x={} scx={:#04X} visible_scx={:#04X} stage={:?} stage_dot={} front_cached={:?}",
                before.ly,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                activity.value,
                before.visible_scx,
                before.bg_fetcher_stage,
                before.bg_fetcher_stage_dot,
                before.bg_fifo_cached_pixels.first(),
            );
        }

        machine.step_t_cycle();

        if !armed {
            continue;
        }

        let after = machine.ppu().snapshot();
        if after.ly != 89 {
            break;
        }

        if after.visible_pixels_output != last_vpo {
            let visible_x = last_vpo as usize;
            println!(
                "emit line_dot={} vpo={} -> {} x={} mixed={} panel={} stage={:?} stage_dot={} fetch_map={:#06X} fetch_data={:#06X} push_pending={} fill_pending={} front_cached={:?}",
                after.line_dot,
                last_vpo,
                after.visible_pixels_output,
                visible_x,
                after.current_scanline_pixels[visible_x],
                machine.ppu().framebuffer()[after.ly as usize * 160 + visible_x],
                after.bg_fetcher_stage,
                after.bg_fetcher_stage_dot,
                after.bg_fetcher_tile_map_address,
                after.bg_fetcher_tile_data_address,
                after.bg_push_pending,
                after.bg_fill_pending,
                after.bg_fifo_cached_pixels.first(),
            );
            last_vpo = after.visible_pixels_output;
            if after.visible_pixels_output >= 24 {
                return;
            }
        }
    }

    panic!("timed out before logging the LY=89 post-write output window");
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly91 after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly91_after_write_window() {
    let mut machine = load_mealybug_m3_scx_high_5_bits_machine();
    let mut armed = false;
    let mut last_vpo = 0u8;

    for _ in 0..20_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();

        if !armed
            && before.ly == 91
            && before.mode == PpuAccessMode::Drawing
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF43)
        {
            armed = true;
            last_vpo = before.visible_pixels_output;
            let activity = before_cpu
                .last_bus_activity
                .expect("FF43 write should expose a bus activity snapshot");
            println!(
                "arm ly={} line_dot={} vpo={} x={} scx={:#04X} visible_scx={:#04X} stage={:?} stage_dot={} front_cached={:?}",
                before.ly,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                activity.value,
                before.visible_scx,
                before.bg_fetcher_stage,
                before.bg_fetcher_stage_dot,
                before.bg_fifo_cached_pixels.first(),
            );
        }

        machine.step_t_cycle();

        if !armed {
            continue;
        }

        let after = machine.ppu().snapshot();
        if after.ly != 91 {
            break;
        }

        if after.visible_pixels_output != last_vpo {
            let visible_x = last_vpo as usize;
            println!(
                "emit line_dot={} vpo={} -> {} x={} mixed={} panel={} stage={:?} stage_dot={} fetch_map={:#06X} fetch_data={:#06X} push_pending={} fill_pending={} front_cached={:?}",
                after.line_dot,
                last_vpo,
                after.visible_pixels_output,
                visible_x,
                after.current_scanline_pixels[visible_x],
                machine.ppu().framebuffer()[after.ly as usize * 160 + visible_x],
                after.bg_fetcher_stage,
                after.bg_fetcher_stage_dot,
                after.bg_fetcher_tile_map_address,
                after.bg_fetcher_tile_data_address,
                after.bg_push_pending,
                after.bg_fill_pending,
                after.bg_fifo_cached_pixels.first(),
            );
            last_vpo = after.visible_pixels_output;
            if after.visible_pixels_output >= 24 {
                return;
            }
        }
    }

    panic!("timed out before logging the LY=91 post-write output window");
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly120 after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly120_after_write_window() {
    let mut machine = load_mealybug_m3_scx_high_5_bits_machine();
    let mut armed = false;
    let mut last_vpo = 0u8;

    for _ in 0..20_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();

        if !armed
            && before.ly == 120
            && before.mode == PpuAccessMode::Drawing
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF43)
        {
            armed = true;
            last_vpo = before.visible_pixels_output;
            let activity = before_cpu
                .last_bus_activity
                .expect("FF43 write should expose a bus activity snapshot");
            println!(
                "arm ly={} line_dot={} vpo={} x={} scx={:#04X} visible_scx={:#04X} stage={:?} stage_dot={} front_cached={:?}",
                before.ly,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                activity.value,
                before.visible_scx,
                before.bg_fetcher_stage,
                before.bg_fetcher_stage_dot,
                before.bg_fifo_cached_pixels.first(),
            );
        }

        machine.step_t_cycle();

        if !armed {
            continue;
        }

        let after = machine.ppu().snapshot();
        if after.ly != 120 {
            break;
        }

        if after.visible_pixels_output != last_vpo {
            let visible_x = last_vpo as usize;
            println!(
                "emit line_dot={} vpo={} -> {} x={} mixed={} panel={} stage={:?} stage_dot={} fetch_map={:#06X} fetch_data={:#06X} push_pending={} fill_pending={} front_cached={:?}",
                after.line_dot,
                last_vpo,
                after.visible_pixels_output,
                visible_x,
                after.current_scanline_pixels[visible_x],
                machine.ppu().framebuffer()[after.ly as usize * 160 + visible_x],
                after.bg_fetcher_stage,
                after.bg_fetcher_stage_dot,
                after.bg_fetcher_tile_map_address,
                after.bg_fetcher_tile_data_address,
                after.bg_push_pending,
                after.bg_fill_pending,
                after.bg_fifo_cached_pixels.first(),
            );
            last_vpo = after.visible_pixels_output;
            if after.visible_pixels_output >= 24 {
                return;
            }
        }
    }

    panic!("timed out before logging the LY=120 post-write output window");
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly121 after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly121_after_write_window() {
    let mut machine = load_mealybug_m3_scx_high_5_bits_machine();
    let mut armed = false;
    let mut last_vpo = 0u8;

    for _ in 0..20_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();

        if !armed
            && before.ly == 121
            && before.mode == PpuAccessMode::Drawing
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF43)
        {
            armed = true;
            last_vpo = before.visible_pixels_output;
            let activity = before_cpu
                .last_bus_activity
                .expect("FF43 write should expose a bus activity snapshot");
            println!(
                "arm ly={} line_dot={} vpo={} x={} scx={:#04X} visible_scx={:#04X} stage={:?} stage_dot={} front_cached={:?}",
                before.ly,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                activity.value,
                before.visible_scx,
                before.bg_fetcher_stage,
                before.bg_fetcher_stage_dot,
                before.bg_fifo_cached_pixels.first(),
            );
        }

        machine.step_t_cycle();

        if !armed {
            continue;
        }

        let after = machine.ppu().snapshot();
        if after.ly != 121 {
            break;
        }

        if after.visible_pixels_output != last_vpo {
            let visible_x = last_vpo as usize;
            println!(
                "emit line_dot={} vpo={} -> {} x={} mixed={} panel={} stage={:?} stage_dot={} fetch_map={:#06X} fetch_data={:#06X} push_pending={} fill_pending={} front_cached={:?}",
                after.line_dot,
                last_vpo,
                after.visible_pixels_output,
                visible_x,
                after.current_scanline_pixels[visible_x],
                machine.ppu().framebuffer()[after.ly as usize * 160 + visible_x],
                after.bg_fetcher_stage,
                after.bg_fetcher_stage_dot,
                after.bg_fetcher_tile_map_address,
                after.bg_fetcher_tile_data_address,
                after.bg_push_pending,
                after.bg_fill_pending,
                after.bg_fifo_cached_pixels.first(),
            );
            last_vpo = after.visible_pixels_output;
            if after.visible_pixels_output >= 24 {
                return;
            }
        }
    }

    panic!("timed out before logging the LY=121 post-write output window");
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly122 after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly122_after_write_window() {
    let mut machine = load_mealybug_m3_scx_high_5_bits_machine();
    let mut armed = false;
    let mut last_vpo = 0u8;

    for _ in 0..20_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();

        if !armed
            && before.ly == 122
            && before.mode == PpuAccessMode::Drawing
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF43)
        {
            armed = true;
            last_vpo = before.visible_pixels_output;
            let activity = before_cpu
                .last_bus_activity
                .expect("FF43 write should expose a bus activity snapshot");
            println!(
                "arm ly={} line_dot={} vpo={} x={} scx={:#04X} visible_scx={:#04X} stage={:?} stage_dot={} front_cached={:?}",
                before.ly,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                activity.value,
                before.visible_scx,
                before.bg_fetcher_stage,
                before.bg_fetcher_stage_dot,
                before.bg_fifo_cached_pixels.first(),
            );
        }

        machine.step_t_cycle();

        if !armed {
            continue;
        }

        let after = machine.ppu().snapshot();
        if after.ly != 122 {
            break;
        }

        if after.visible_pixels_output != last_vpo {
            let visible_x = last_vpo as usize;
            println!(
                "emit line_dot={} vpo={} -> {} x={} mixed={} panel={} stage={:?} stage_dot={} fetch_map={:#06X} fetch_data={:#06X} push_pending={} fill_pending={} front_cached={:?}",
                after.line_dot,
                last_vpo,
                after.visible_pixels_output,
                visible_x,
                after.current_scanline_pixels[visible_x],
                machine.ppu().framebuffer()[after.ly as usize * 160 + visible_x],
                after.bg_fetcher_stage,
                after.bg_fetcher_stage_dot,
                after.bg_fetcher_tile_map_address,
                after.bg_fetcher_tile_data_address,
                after.bg_push_pending,
                after.bg_fill_pending,
                after.bg_fifo_cached_pixels.first(),
            );
            last_vpo = after.visible_pixels_output;
            if after.visible_pixels_output >= 24 {
                return;
            }
        }
    }

    panic!("timed out before logging the LY=122 post-write output window");
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly26 hblank row after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly26_hblank_row_after_write() {
    log_mealybug_m3_scx_high_5_bits_hblank_row_after_target_write(26);
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly9 hblank row after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly9_hblank_row_after_write() {
    log_mealybug_m3_scx_high_5_bits_hblank_row_after_target_write(9);
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly27 hblank row after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly27_hblank_row_after_write() {
    log_mealybug_m3_scx_high_5_bits_hblank_row_after_target_write(27);
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly30 hblank row after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly30_hblank_row_after_write() {
    log_mealybug_m3_scx_high_5_bits_hblank_row_after_target_write(30);
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly40 hblank row after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly40_hblank_row_after_write() {
    log_mealybug_m3_scx_high_5_bits_hblank_row_after_target_write(40);
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly41 hblank row after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly41_hblank_row_after_write() {
    log_mealybug_m3_scx_high_5_bits_hblank_row_after_target_write(41);
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly50 hblank row after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly50_hblank_row_after_write() {
    log_mealybug_m3_scx_high_5_bits_hblank_row_after_target_write(50);
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly89 hblank row after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly89_hblank_row_after_write() {
    log_mealybug_m3_scx_high_5_bits_hblank_row_after_target_write(89);
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly56 hblank row after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly56_hblank_row_after_write() {
    log_mealybug_m3_scx_high_5_bits_hblank_row_after_target_write(56);
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly64 hblank row after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly64_hblank_row_after_write() {
    log_mealybug_m3_scx_high_5_bits_hblank_row_after_target_write(64);
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly72 hblank row after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly72_hblank_row_after_write() {
    log_mealybug_m3_scx_high_5_bits_hblank_row_after_target_write(72);
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly80 hblank row after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly80_hblank_row_after_write() {
    log_mealybug_m3_scx_high_5_bits_hblank_row_after_target_write(80);
}

#[test]
#[ignore = "diag: real mealybug m3_scx_high_5_bits ly35 hblank row after FF43 write"]
fn real_mealybug_m3_scx_high_5_bits_logs_ly35_hblank_row_after_write() {
    log_mealybug_m3_scx_high_5_bits_hblank_row_after_target_write(35);
}

#[test]
#[ignore = "diag: mooneye intr_2_oam_ok_timing seam"]
fn mode2_to_oam_release_probe_matches_mooneye_counts() {
    let delay46 = run_intr_2_oam_ok_probe(46);
    let delay45 = run_intr_2_oam_ok_probe(45);

    assert_eq!(
        (delay46.count, delay45.count),
        (0x01, 0x02),
        "delay46={delay46:?} delay45={delay45:?}"
    );
}

#[test]
#[ignore = "diag: first FE00 reads after intr_2_oam_ok_timing wake"]
fn mode2_to_oam_release_probe_logs_first_reads() {
    let delay46 = sample_intr_2_oam_ok_reads(46, 3);
    let delay45 = sample_intr_2_oam_ok_reads(45, 3);
    println!("delay46={delay46:?}");
    println!("delay45={delay45:?}");
}

#[test]
#[ignore = "diag: FE00 reads in the real mooneye intr_2_oam_ok_timing ROM"]
fn real_mooneye_oam_ok_logs_first_reads() {
    let reads = sample_real_mooneye_oam_ok_reads(4);
    println!("reads={reads:?}");
}

#[test]
#[ignore = "diag: real mooneye stat_lyc_onoff access trace"]
fn real_mooneye_stat_lyc_onoff_logs_first_accesses() {
    let accesses = sample_real_mooneye_stat_lyc_onoff_accesses(48);
    println!("accesses={accesses:?}");
}

#[test]
#[ignore = "diag: reduced caller matrix for ten-sprite staggered mooneye cases"]
fn intr_2_mode0_timing_sprites_ten_sprite_staggered_real_caller_matrix() {
    for (label, sprite_xs, delay_a, delay_b) in [
        (
            "x00_to_x48_68_67",
            [0x00, 0x08, 0x10, 0x18, 0x20, 0x28, 0x30, 0x38, 0x40, 0x48],
            0x44_u8,
            0x43_u8,
        ),
        (
            "x01_to_x49_66_65",
            [0x01, 0x09, 0x11, 0x19, 0x21, 0x29, 0x31, 0x39, 0x41, 0x49],
            0x42_u8,
            0x41_u8,
        ),
        (
            "x02_to_x4A_63_62",
            [0x02, 0x0A, 0x12, 0x1A, 0x22, 0x2A, 0x32, 0x3A, 0x42, 0x4A],
            0x3F_u8,
            0x3E_u8,
        ),
        (
            "x03_to_x4B_61_60",
            [0x03, 0x0B, 0x13, 0x1B, 0x23, 0x2B, 0x33, 0x3B, 0x43, 0x4B],
            0x3D_u8,
            0x3C_u8,
        ),
        (
            "x04_to_x4C_58_57",
            [0x04, 0x0C, 0x14, 0x1C, 0x24, 0x2C, 0x34, 0x3C, 0x44, 0x4C],
            0x3A_u8,
            0x39_u8,
        ),
        (
            "x05_to_x4D_56_55",
            [0x05, 0x0D, 0x15, 0x1D, 0x25, 0x2D, 0x35, 0x3D, 0x45, 0x4D],
            0x38_u8,
            0x37_u8,
        ),
        (
            "x06_to_x4E_56_55",
            [0x06, 0x0E, 0x16, 0x1E, 0x26, 0x2E, 0x36, 0x3E, 0x46, 0x4E],
            0x38_u8,
            0x37_u8,
        ),
        (
            "x07_to_x4F_56_55",
            [0x07, 0x0F, 0x17, 0x1F, 0x27, 0x2F, 0x37, 0x3F, 0x47, 0x4F],
            0x38_u8,
            0x37_u8,
        ),
        (
            "x48_to_x00_68_67",
            [0x48, 0x40, 0x38, 0x30, 0x28, 0x20, 0x18, 0x10, 0x08, 0x00],
            0x44_u8,
            0x43_u8,
        ),
        (
            "x49_to_x01_66_65",
            [0x49, 0x41, 0x39, 0x31, 0x29, 0x21, 0x19, 0x11, 0x09, 0x01],
            0x42_u8,
            0x41_u8,
        ),
    ] {
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );
        machine
            .load_cartridge(
                build_intr_2_mode0_timing_sprites_real_caller_probe_rom_with_specs(
                    &sprite_xs, delay_a, delay_b,
                ),
            )
            .expect("probe ROM should load");

        let mut outcome = None;
        for _ in 0..5_000_000 {
            machine.step_t_cycle();

            let cpu = machine.cpu().snapshot();
            if machine.read_bus(0xC20A) == 0x01 {
                let ppu = machine.ppu().snapshot();
                outcome = Some(format!(
                    "{label}: success b={:#04X} c={:#04X} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                    cpu.registers.b,
                    cpu.registers.c,
                    ppu.ly,
                    ppu.line_dot,
                    ppu.mode,
                    ppu.mode0_start_dot,
                ));
                break;
            }

            if matches!(cpu.registers.pc, 0x486E | 0x486F | 0x4870 | 0x0C06) {
                let ppu = machine.ppu().snapshot();
                outcome = Some(format!(
                    "{label}: failure pc={:#06X} a={:#04X} b={:#04X} c={:#04X} d={:#04X} e={:#04X} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                    cpu.registers.pc,
                    cpu.registers.a,
                    cpu.registers.b,
                    cpu.registers.c,
                    cpu.registers.d,
                    cpu.registers.e,
                    ppu.ly,
                    ppu.line_dot,
                    ppu.mode,
                    ppu.mode0_start_dot,
                ));
                break;
            }
        }

        println!("{}", outcome.unwrap_or_else(|| format!("{label}: timeout")));
    }
}

#[test]
#[ignore = "diag: copied ROM-path probe for case1 arm/read signature"]
fn mode2_to_mode0_sprites_case1_rom_path_probe_logs_arm_and_first_read() {
    let sample = sample_intr_2_mode0_sprites_case1_rom_path_probe_reads_after_arm(2);
    let Intr2Mode0SpritesCase1RomPathArmObservation {
        ly,
        line_dot,
        mode,
        pc,
    } = sample.arm;
    println!("case1_rom_path_arm ly={ly} line_dot={line_dot} mode={mode:?} pc={pc:#06X}");
    println!("case1_rom_path_reads={:?}", sample.reads);
    println!("case1_rom_path_terminal={:?}", sample.terminal);
}

#[test]
#[ignore = "diag: stale stat-mode probe that no longer matches the external mode0 oracle"]
fn mode2_to_mode0_stat_probe_matches_mooneye_counts() {
    let delay46 = run_intr_2_stat_mode_probe(46, 0x00);
    let delay45 = run_intr_2_stat_mode_probe(45, 0x00);

    assert_eq!(
        (delay46.count, delay45.count),
        (0x01, 0x02),
        "delay46={delay46:?} delay45={delay45:?}"
    );
}

#[test]
#[ignore = "diag: local ly23 BGP phase oracle for daid ppu_scanline_bgp"]
fn daid_ppu_scanline_bgp_ly23_phase_oracle() {
    let line15 = sample_daid_ppu_scanline_bgp_line(15);
    let line23 = sample_daid_ppu_scanline_bgp_line(23);

    assert_eq!(
        line15.raw_pixels_prefix, line23.raw_pixels_prefix,
        "ly15 raw={:?} ly23 raw={:?}",
        line15.raw_pixels_prefix, line23.raw_pixels_prefix
    );
    assert_eq!(
        line15.visible_bgp_writes, line23.visible_bgp_writes,
        "ly15 writes={:?} ly23 writes={:?}",
        line15.visible_bgp_writes, line23.visible_bgp_writes
    );
    assert_ne!(
        line15.panel_pixels_prefix, line23.panel_pixels_prefix,
        "ly15 panel={:?} ly23 panel={:?}",
        line15.panel_pixels_prefix, line23.panel_pixels_prefix
    );
    assert!(
        DAID_SCANLINE_BGP_ACCEPTED_LY23_PANEL_PREFIXES
            .iter()
            .any(|expected| expected == &line23.panel_pixels_prefix),
        "ly23 panel={:?} accepted={:?}",
        line23.panel_pixels_prefix,
        DAID_SCANLINE_BGP_ACCEPTED_LY23_PANEL_PREFIXES
    );
}

#[test]
#[ignore = "diag: block-end family summary for daid ppu_scanline_bgp"]
fn daid_ppu_scanline_bgp_logs_block_end_family() {
    let target_lys = [
        6_u8, 7, 8, 22, 23, 24, 38, 39, 40, 54, 55, 56, 78, 79, 80, 86, 87, 88, 94, 95, 96, 102,
        103, 104, 118, 119, 120, 134, 135, 136,
    ];
    let observations = sample_daid_ppu_scanline_bgp_lines(&target_lys);
    let mut summary = Vec::new();

    for &ly in &[7_u8, 23, 39, 55, 79, 87, 95, 103, 119, 135] {
        let previous = observations
            .get(&(ly - 1))
            .expect("previous line should be sampled");
        let current = observations
            .get(&ly)
            .expect("current line should be sampled");
        let next = observations
            .get(&(ly + 1))
            .expect("next line should be sampled");
        let panel_matches_previous = current.panel_pixels_prefix == previous.panel_pixels_prefix;
        let panel_matches_next = current.panel_pixels_prefix == next.panel_pixels_prefix;
        let raw_matches_previous = current.raw_pixels_prefix == previous.raw_pixels_prefix;
        let raw_matches_next = current.raw_pixels_prefix == next.raw_pixels_prefix;
        let writes_match_previous = current.visible_bgp_writes == previous.visible_bgp_writes;
        let writes_match_next = current.visible_bgp_writes == next.visible_bgp_writes;
        summary.push((
            ly,
            panel_matches_previous,
            panel_matches_next,
            raw_matches_previous,
            raw_matches_next,
            writes_match_previous,
            writes_match_next,
            current.panel_pixels_prefix,
        ));
    }

    println!("block_end_family={summary:#?}");
}

#[test]
#[ignore = "diag: lcd enable write chronology boundary snapshots"]
fn cpu_path_lcd_enable_write_probe_logs_boundary_snapshots() {
    for delay in [111_u16, 112, 131, 132, 225, 226, 245, 246] {
        let oam = run_lcd_enable_write_probe_observation(0xFE00, delay);
        let vram = run_lcd_enable_write_probe_observation(0x8000, delay);
        println!("delay={delay} oam={oam:?} vram={vram:?}");
    }
}
