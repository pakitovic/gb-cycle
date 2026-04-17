//! Diagnostic-only PPU probes.
//!
//! Policy:
//! - stable, cheap oracles stay active in the owning family module
//! - ignored ad-hoc probes live here and use `#[ignore = "diag: ..."]`
//! - stale probes should be deleted instead of preserved as historical noise

use super::*;
use gb_core::ppu::PpuBgCachedSliceOriginSnapshot;

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

fn load_mealybug_m3_lcdc_bg_en_change_machine() -> Machine<gb_core::TraceSummaryBuffer> {
    let rom_path = resolve_test_rom_path("mealybug-tearoom-tests/ppu/m3_lcdc_bg_en_change.gb");
    let rom =
        std::fs::read(&rom_path).expect("mealybug m3_lcdc_bg_en_change ROM should be present");
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("diagnostic ROM should load");
    machine
}

fn load_mealybug_m3_lcdc_bg_map_change_machine() -> Machine<gb_core::TraceSummaryBuffer> {
    let rom_path = resolve_test_rom_path("mealybug-tearoom-tests/ppu/m3_lcdc_bg_map_change.gb");
    let rom =
        std::fs::read(&rom_path).expect("mealybug m3_lcdc_bg_map_change ROM should be present");
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("diagnostic ROM should load");
    machine
}

fn load_mealybug_m3_lcdc_obj_en_change_machine() -> Machine<gb_core::TraceSummaryBuffer> {
    let rom_path = resolve_test_rom_path("mealybug-tearoom-tests/ppu/m3_lcdc_obj_en_change.gb");
    let rom =
        std::fs::read(&rom_path).expect("mealybug m3_lcdc_obj_en_change ROM should be present");
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("diagnostic ROM should load");
    machine
}

fn load_mealybug_m3_lcdc_obj_en_change_variant_machine() -> Machine<gb_core::TraceSummaryBuffer> {
    let rom_path =
        resolve_test_rom_path("mealybug-tearoom-tests/ppu/m3_lcdc_obj_en_change_variant.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mealybug m3_lcdc_obj_en_change_variant ROM should be present");
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("diagnostic ROM should load");
    machine
}

fn load_mealybug_m3_lcdc_obj_size_change_machine() -> Machine<gb_core::TraceSummaryBuffer> {
    let rom_path = resolve_test_rom_path("mealybug-tearoom-tests/ppu/m3_lcdc_obj_size_change.gb");
    let rom =
        std::fs::read(&rom_path).expect("mealybug m3_lcdc_obj_size_change ROM should be present");
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("diagnostic ROM should load");
    machine
}

fn load_mealybug_m3_lcdc_obj_size_change_scx_machine() -> Machine<gb_core::TraceSummaryBuffer> {
    let rom_path =
        resolve_test_rom_path("mealybug-tearoom-tests/ppu/m3_lcdc_obj_size_change_scx.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mealybug m3_lcdc_obj_size_change_scx ROM should be present");
    let mut machine = Machine::new_summary(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("diagnostic ROM should load");
    machine
}

fn seed_dmg_boot_trademark_tile(machine: &mut Machine<gb_core::TraceSummaryBuffer>) {
    const DMG_BOOT_TRADEMARK_TILE_BYTES: [u8; 16] = [
        0x3C, 0x00, 0x42, 0x00, 0xB9, 0x00, 0xA5, 0x00, 0xB9, 0x00, 0xA5, 0x00, 0x42, 0x00, 0x3C,
        0x00,
    ];

    for (index, byte) in DMG_BOOT_TRADEMARK_TILE_BYTES.iter().copied().enumerate() {
        machine.write_bus(0x8190 + index as u16, byte);
    }
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

fn log_mealybug_m3_lcdc_bg_en_change_after_ff40_write_window(target_ly: u8, stop_vpo: u8) {
    let mut machine = load_mealybug_m3_lcdc_bg_en_change_machine();
    let mut armed = false;
    let mut write_index = 0usize;
    let mut last_vpo = 0u8;

    for _ in 0..20_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();

        if before.ly == target_ly
            && before.mode == PpuAccessMode::Drawing
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF40)
        {
            let activity = before_cpu
                .last_bus_activity
                .expect("FF40 write should expose a bus activity snapshot");
            println!(
                "write#{} ly={} line_dot={} vpo={} x={} scx={} visible_scx={} value={:#04X} stage={:?} stage_dot={} startup={:?} placeholders={} push_pending={} fill_pending={} front_cached={:?}",
                write_index,
                before.ly,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                before.scx,
                before.visible_scx,
                activity.value,
                before.bg_fetcher_stage,
                before.bg_fetcher_stage_dot,
                before.bg_startup_fetch_seam,
                before.bg_startup_fifo_placeholders,
                before.bg_push_pending,
                before.bg_fill_pending,
                before.bg_fifo_cached_pixels.first(),
            );
            write_index += 1;
            if !armed {
                armed = true;
                last_vpo = before.visible_pixels_output;
            }
        }

        machine.step_t_cycle();

        let after = machine.ppu().snapshot();
        if after.ly != target_ly {
            if armed {
                break;
            }
            continue;
        }

        if after.visible_pixels_output != last_vpo {
            let visible_x = last_vpo as usize;
            let lcdc = machine.read_bus(0xFF40);
            let panel = machine.ppu().framebuffer()[after.ly as usize * 160 + visible_x];
            println!(
                "emit line_dot={} vpo={} -> {} x={} scx={} visible_scx={} mixed={} panel={} lcdc={:#04X} stage={:?} stage_dot={} startup={:?} placeholders={} push_pending={} fill_pending={} front_cached={:?}",
                after.line_dot,
                last_vpo,
                after.visible_pixels_output,
                visible_x,
                after.scx,
                after.visible_scx,
                after.current_scanline_pixels[visible_x],
                panel,
                lcdc,
                after.bg_fetcher_stage,
                after.bg_fetcher_stage_dot,
                after.bg_startup_fetch_seam,
                after.bg_startup_fifo_placeholders,
                after.bg_push_pending,
                after.bg_fill_pending,
                after.bg_fifo_cached_pixels.first(),
            );
            last_vpo = after.visible_pixels_output;
            if after.visible_pixels_output >= stop_vpo {
                return;
            }
        }
    }

    panic!("timed out before logging the target LY output window after FF40 writes");
}

fn log_mealybug_m3_lcdc_obj_en_change_after_ff40_write_window(target_ly: u8, stop_vpo: u8) {
    let mut machine = load_mealybug_m3_lcdc_obj_en_change_machine();
    let mut armed = false;
    let mut write_index = 0usize;
    let mut last_vpo = 0u8;
    let mut recent_emits = std::collections::VecDeque::new();
    let mut recent_states = std::collections::VecDeque::new();

    for _ in 0..20_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();
        if before.ly == target_ly && before.mode == PpuAccessMode::Drawing {
            let tile25_row0 = (
                machine.read_bus(0x8000 + 25 * 16),
                machine.read_bus(0x8000 + 25 * 16 + 1),
            );
            recent_states.push_back(format!(
                "state line_dot={} vpo={} x={} visible_lcdc={:#04X} pipeline_lcdc={:#04X} obj_stage={:?} obj_stage_dot={} obj_req={:?} obj_resolved={:?} obj_tile={:?} obj_row={:?} obj_addr={:?}/{:?} obj_low={:#04X} obj_high={:#04X} pending_match_x={:?} pending_len={} obj_fifo={:?} tile25_row0={:?}",
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                before.visible_lcdc,
                before.pipeline_lcdc,
                before.obj_fetcher_stage,
                before.obj_fetcher_stage_dot,
                before.obj_fetcher_requested_sprite,
                before.obj_fetcher_resolved_sprite,
                before.obj_fetcher_resolved_tile_index,
                before.obj_fetcher_resolved_tile_row,
                before.obj_fetcher_tile_low_address,
                before.obj_fetcher_tile_high_address,
                before.obj_fetcher_tile_low,
                before.obj_fetcher_tile_high,
                before.obj_pending_hit_match_x,
                before.obj_pending_hit_len,
                before.obj_fifo_pixels,
                tile25_row0,
            ));
            while recent_states.len() > 20 {
                recent_states.pop_front();
            }
        }

        if before.ly == target_ly
            && before.mode == PpuAccessMode::Drawing
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF40)
        {
            let activity = before_cpu
                .last_bus_activity
                .expect("FF40 write should expose a bus activity snapshot");
            let tile25_row0 = (
                machine.read_bus(0x8000 + 25 * 16),
                machine.read_bus(0x8000 + 25 * 16 + 1),
            );
            println!(
                "write#{} ly={} line_dot={} vpo={} x={} value={:#04X} visible_lcdc={:#04X} pipeline_lcdc={:#04X} obj_stage={:?} obj_stage_dot={} obj_req={:?} obj_resolved={:?} obj_tile={:?} obj_row={:?} obj_addr={:?}/{:?} obj_low={:#04X} obj_high={:#04X} pending_match_x={:?} pending_len={} obj_fifo={:?} tile25_row0={:?} selected={:?} startup={:?} push_pending={} fill_pending={} front_cached={:?}",
                write_index,
                before.ly,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                activity.value,
                before.visible_lcdc,
                before.pipeline_lcdc,
                before.obj_fetcher_stage,
                before.obj_fetcher_stage_dot,
                before.obj_fetcher_requested_sprite,
                before.obj_fetcher_resolved_sprite,
                before.obj_fetcher_resolved_tile_index,
                before.obj_fetcher_resolved_tile_row,
                before.obj_fetcher_tile_low_address,
                before.obj_fetcher_tile_high_address,
                before.obj_fetcher_tile_low,
                before.obj_fetcher_tile_high,
                before.obj_pending_hit_match_x,
                before.obj_pending_hit_len,
                before.obj_fifo_pixels,
                tile25_row0,
                before.selected_sprites,
                before.bg_startup_fetch_seam,
                before.bg_push_pending,
                before.bg_fill_pending,
                before.bg_fifo_cached_pixels.first(),
            );
            if !recent_states.is_empty() {
                println!("recent_states_before_write=");
                for state in &recent_states {
                    println!("  {state}");
                }
            }
            if !recent_emits.is_empty() {
                println!("recent_emits_before_write={recent_emits:?}");
            }
            write_index += 1;
            if !armed {
                armed = true;
                last_vpo = before.visible_pixels_output;
            }
        }

        machine.step_t_cycle();

        if !armed {
            continue;
        }

        let after = machine.ppu().snapshot();
        if after.ly != target_ly {
            break;
        }

        if after.visible_pixels_output != last_vpo {
            let visible_x = last_vpo as usize;
            let lcdc = machine.read_bus(0xFF40);
            let panel = machine.ppu().framebuffer()[after.ly as usize * 160 + visible_x];
            recent_emits.push_back((
                after.line_dot,
                last_vpo,
                after.visible_pixels_output,
                visible_x,
                after.current_scanline_pixels[visible_x],
                panel,
                lcdc,
            ));
            while recent_emits.len() > 16 {
                recent_emits.pop_front();
            }
            if armed {
                println!(
                    "emit line_dot={} vpo={} -> {} x={} mixed={} panel={} lcdc={:#04X} visible_lcdc={:#04X} pipeline_lcdc={:#04X} obj_stage={:?} obj_stage_dot={} obj_req={:?} obj_resolved={:?} obj_tile={:?} obj_row={:?} obj_addr={:?}/{:?} obj_low={:#04X} obj_high={:#04X} pending_match_x={:?} pending_len={} obj_fifo={:?} selected={:?}",
                    after.line_dot,
                    last_vpo,
                    after.visible_pixels_output,
                    visible_x,
                    after.current_scanline_pixels[visible_x],
                    panel,
                    lcdc,
                    after.visible_lcdc,
                    after.pipeline_lcdc,
                    after.obj_fetcher_stage,
                    after.obj_fetcher_stage_dot,
                    after.obj_fetcher_requested_sprite,
                    after.obj_fetcher_resolved_sprite,
                    after.obj_fetcher_resolved_tile_index,
                    after.obj_fetcher_resolved_tile_row,
                    after.obj_fetcher_tile_low_address,
                    after.obj_fetcher_tile_high_address,
                    after.obj_fetcher_tile_low,
                    after.obj_fetcher_tile_high,
                    after.obj_pending_hit_match_x,
                    after.obj_pending_hit_len,
                    after.obj_fifo_pixels,
                    after.selected_sprites,
                );
            }
            last_vpo = after.visible_pixels_output;
            if armed && after.visible_pixels_output >= stop_vpo {
                return;
            }
        }
    }

    panic!("timed out before logging the target LY output window after FF40 writes");
}

fn log_mealybug_m3_lcdc_obj_en_change_write_signatures(target_lys: &[u8]) {
    let mut machine = load_mealybug_m3_lcdc_obj_en_change_machine();
    let targets = target_lys
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut printed = std::collections::BTreeSet::new();

    for _ in 0..20_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();
        if before.mode == PpuAccessMode::Drawing
            && targets.contains(&before.ly)
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF40)
            && printed.insert(before.ly)
        {
            let activity = before_cpu
                .last_bus_activity
                .expect("FF40 write should expose a bus activity snapshot");
            println!(
                "ly={} line_dot={} vpo={} x={} value={:#04X} visible_lcdc={:#04X} pipeline_lcdc={:#04X} selected={:?} obj_stage={:?} obj_stage_dot={} obj_fifo={:?}",
                before.ly,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                activity.value,
                before.visible_lcdc,
                before.pipeline_lcdc,
                before.selected_sprites,
                before.obj_fetcher_stage,
                before.obj_fetcher_stage_dot,
                before.obj_fifo_pixels,
            );

            if printed.len() == targets.len() {
                return;
            }
        }

        machine.step_t_cycle();
    }

    panic!("timed out before logging all target LY signatures");
}

fn log_mealybug_m3_lcdc_obj_en_change_hblank_row(target_ly: u8) {
    let mut machine = load_mealybug_m3_lcdc_obj_en_change_machine();

    for _ in 0..20_000_000 {
        machine.step_t_cycle();

        let after = machine.ppu().snapshot();
        if after.ly != target_ly {
            continue;
        }

        if after.mode == PpuAccessMode::HBlank {
            let row_start = after.ly as usize * 160;
            let tile25_row0 = (
                machine.read_bus(0x8000 + 25 * 16),
                machine.read_bus(0x8000 + 25 * 16 + 1),
            );
            let sprite_bytes = after.selected_sprites.first().map(|sprite| {
                let sprite_top = sprite.y.wrapping_sub(16);
                let row = target_ly.wrapping_sub(sprite_top);
                let tile = sprite.tile_index;
                let base = u16::from(tile) * 16 + u16::from(row) * 2;
                (
                    tile,
                    row,
                    machine.read_bus(0x8000 + base),
                    machine.read_bus(0x8000 + base + 1),
                )
            });
            println!(
                "hblank ly={} mode0={} selected={:?} tile25_row0={:?} sprite_bytes={:?} raw_0_15={:?} panel_0_15={:?}",
                after.ly,
                after.mode0_start_dot,
                after.selected_sprites,
                tile25_row0,
                sprite_bytes,
                &after.current_scanline_pixels[..16],
                &machine.ppu().framebuffer()[row_start..row_start + 16],
            );
            return;
        }
    }

    panic!("timed out before sampling HBlank row");
}

fn log_mealybug_m3_lcdc_obj_en_change_line_timeline(
    target_ly: u8,
    line_dot_start: u16,
    line_dot_end: u16,
) {
    let mut machine = load_mealybug_m3_lcdc_obj_en_change_machine();

    for _ in 0..20_000_000 {
        machine.step_t_cycle();

        let after = machine.ppu().snapshot();
        if after.ly != target_ly || after.line_dot < line_dot_start || after.line_dot > line_dot_end
        {
            continue;
        }

        let cpu = machine.cpu().snapshot();
        let last_event = cpu.last_address_event.and_then(|event| {
            event.access_address.map(|address| {
                (
                    event.kind,
                    address,
                    cpu.last_bus_activity.map(|activity| activity.value),
                )
            })
        });
        let tile25_row0 = (
            machine.read_bus(0x8000 + 25 * 16),
            machine.read_bus(0x8000 + 25 * 16 + 1),
        );
        println!(
            "ly={} line_dot={} vpo={} x={} visible_lcdc={:#04X} pipeline_lcdc={:#04X} obj_stage={:?} obj_stage_dot={} obj_fifo={:?} tile25_row0={:?} last_event={:?}",
            after.ly,
            after.line_dot,
            after.visible_pixels_output,
            after.bg_current_transfer_x,
            after.visible_lcdc,
            after.pipeline_lcdc,
            after.obj_fetcher_stage,
            after.obj_fetcher_stage_dot,
            after.obj_fifo_pixels,
            tile25_row0,
            last_event,
        );

        if after.line_dot == line_dot_end {
            return;
        }
    }

    panic!("timed out before logging the target line timeline");
}

fn log_mealybug_m3_lcdc_obj_en_change_tile25_activity(target_lys: &[u8]) {
    let mut machine = load_mealybug_m3_lcdc_obj_en_change_machine();
    let targets = target_lys
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut printed_hblank = std::collections::BTreeSet::new();

    for _ in 0..20_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();
        if let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && let Some(activity) = before_cpu.last_bus_activity
            && (0x8190..=0x819F).contains(&activity.address)
        {
            println!(
                "tile25_write ly={} mode={:?} line_dot={} vpo={} x={} address={:#06X} value={:#04X}",
                before.ly,
                before.mode,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                activity.address,
                activity.value,
            );
        }

        machine.step_t_cycle();

        let after = machine.ppu().snapshot();
        if after.mode == PpuAccessMode::HBlank
            && targets.contains(&after.ly)
            && printed_hblank.insert(after.ly)
        {
            let rows = (0..8)
                .map(|row| {
                    let base = 0x8000 + 25 * 16 + row * 2;
                    (row, machine.read_bus(base), machine.read_bus(base + 1))
                })
                .collect::<Vec<_>>();
            println!(
                "tile25_hblank ly={} mode0={} rows={:?} selected={:?} raw_0_15={:?}",
                after.ly,
                after.mode0_start_dot,
                rows,
                after.selected_sprites,
                &after.current_scanline_pixels[..16],
            );
            if printed_hblank.len() == targets.len() {
                return;
            }
        }
    }

    panic!("timed out before logging tile25 activity");
}

fn log_mealybug_m3_lcdc_obj_en_change_after_ff40_write_window_in_completed_frame(
    target_completed_frames: u32,
    target_ly: u8,
    stop_vpo: u8,
) {
    let mut machine = load_mealybug_m3_lcdc_obj_en_change_machine();
    let mut completed_frames = 0_u32;
    let mut at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;
    let mut armed = false;
    let mut write_index = 0usize;
    let mut last_vpo = 0u8;
    let mut recent_states = std::collections::VecDeque::new();

    for _ in 0..80_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();
        if completed_frames == target_completed_frames
            && before.ly == target_ly
            && before.mode == PpuAccessMode::Drawing
        {
            recent_states.push_back(format!(
                "frame={} state line_dot={} vpo={} x={} bg_stage={:?}/{} bg_addr={:#06X} bg_tile={} bg_low={:#04X} bg_high={:#04X} last_unsigned={:#04X}/{:#04X} obj_stage={:?} obj_stage_dot={} obj_req={:?} obj_resolved={:?} obj_tile={:?} obj_row={:?} obj_addr={:?}/{:?} obj_low={:#04X} obj_high={:#04X} obj_fifo={:?}",
                completed_frames,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                before.bg_fetcher_stage,
                before.bg_fetcher_stage_dot,
                before.bg_fetcher_tile_data_address,
                before.bg_fetcher_tile_index,
                before.bg_fetcher_tile_low,
                before.bg_fetcher_tile_high,
                before.last_unsigned_tile_data_low_fetch,
                before.last_unsigned_tile_data_high_fetch,
                before.obj_fetcher_stage,
                before.obj_fetcher_stage_dot,
                before.obj_fetcher_requested_sprite,
                before.obj_fetcher_resolved_sprite,
                before.obj_fetcher_resolved_tile_index,
                before.obj_fetcher_resolved_tile_row,
                before.obj_fetcher_tile_low_address,
                before.obj_fetcher_tile_high_address,
                before.obj_fetcher_tile_low,
                before.obj_fetcher_tile_high,
                before.obj_fifo_pixels,
            ));
            while recent_states.len() > 20 {
                recent_states.pop_front();
            }
        }

        if completed_frames == target_completed_frames
            && before.ly == target_ly
            && before.mode == PpuAccessMode::Drawing
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF40)
        {
            let activity = before_cpu
                .last_bus_activity
                .expect("FF40 write should expose a bus activity snapshot");
            println!(
                "target_frame_write#{} completed_frames={} ly={} line_dot={} vpo={} x={} value={:#04X} visible_lcdc={:#04X} pipeline_lcdc={:#04X} bg_stage={:?}/{} bg_addr={:#06X} bg_tile={} bg_low={:#04X} bg_high={:#04X} last_unsigned={:#04X}/{:#04X} obj_stage={:?} obj_stage_dot={} obj_req={:?} obj_resolved={:?} obj_tile={:?} obj_row={:?} obj_addr={:?}/{:?} obj_low={:#04X} obj_high={:#04X} obj_fifo={:?}",
                write_index,
                completed_frames,
                before.ly,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                activity.value,
                before.visible_lcdc,
                before.pipeline_lcdc,
                before.bg_fetcher_stage,
                before.bg_fetcher_stage_dot,
                before.bg_fetcher_tile_data_address,
                before.bg_fetcher_tile_index,
                before.bg_fetcher_tile_low,
                before.bg_fetcher_tile_high,
                before.last_unsigned_tile_data_low_fetch,
                before.last_unsigned_tile_data_high_fetch,
                before.obj_fetcher_stage,
                before.obj_fetcher_stage_dot,
                before.obj_fetcher_requested_sprite,
                before.obj_fetcher_resolved_sprite,
                before.obj_fetcher_resolved_tile_index,
                before.obj_fetcher_resolved_tile_row,
                before.obj_fetcher_tile_low_address,
                before.obj_fetcher_tile_high_address,
                before.obj_fetcher_tile_low,
                before.obj_fetcher_tile_high,
                before.obj_fifo_pixels,
            );
            println!("recent_target_frame_states_before_write=");
            for state in &recent_states {
                println!("  {state}");
            }
            write_index += 1;
            armed = true;
            last_vpo = before.visible_pixels_output;
        }

        machine.step_t_cycle();

        let now_at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;
        if now_at_frame_origin && !at_frame_origin {
            completed_frames += 1;
        }
        at_frame_origin = now_at_frame_origin;

        if !armed {
            continue;
        }

        let after = machine.ppu().snapshot();
        if completed_frames != target_completed_frames || after.ly != target_ly {
            continue;
        }

        if after.visible_pixels_output != last_vpo {
            let visible_x = last_vpo as usize;
            let panel = machine.ppu().framebuffer()[after.ly as usize * 160 + visible_x];
            println!(
                "target_frame_emit completed_frames={} line_dot={} vpo={} -> {} x={} mixed={} panel={} lcdc={:#04X} obj_stage={:?} obj_stage_dot={} obj_low={:#04X} obj_high={:#04X} obj_fifo={:?}",
                completed_frames,
                after.line_dot,
                last_vpo,
                after.visible_pixels_output,
                visible_x,
                after.current_scanline_pixels[visible_x],
                panel,
                machine.read_bus(0xFF40),
                after.obj_fetcher_stage,
                after.obj_fetcher_stage_dot,
                after.obj_fetcher_tile_low,
                after.obj_fetcher_tile_high,
                after.obj_fifo_pixels,
            );
            last_vpo = after.visible_pixels_output;
            if after.visible_pixels_output >= stop_vpo {
                return;
            }
        }
    }

    panic!("timed out before logging the target completed frame write window");
}

fn log_mealybug_m3_lcdc_obj_en_change_video_writes_until_hblank(
    target_completed_frames: u32,
    target_ly: u8,
) {
    let mut machine = load_mealybug_m3_lcdc_obj_en_change_machine();
    let mut completed_frames = 0_u32;
    let mut at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;

    for _ in 0..80_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();
        if completed_frames == target_completed_frames
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && let Some(activity) = before_cpu.last_bus_activity
            && ((0x8000..=0x9FFF).contains(&activity.address)
                || (0xFE00..=0xFE9F).contains(&activity.address)
                || activity.address == 0xFF40)
        {
            println!(
                "frame={} write mode={:?} ly={} line_dot={} vpo={} x={} address={:#06X} value={:#04X}",
                completed_frames,
                before.mode,
                before.ly,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                activity.address,
                activity.value,
            );
        }

        machine.step_t_cycle();

        let now_at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;
        if now_at_frame_origin && !at_frame_origin {
            completed_frames += 1;
        }
        at_frame_origin = now_at_frame_origin;

        let after = machine.ppu().snapshot();
        if completed_frames == target_completed_frames
            && after.ly == target_ly
            && after.mode == PpuAccessMode::HBlank
        {
            let tile25_rows = (0..8)
                .map(|row| {
                    let base = 0x8000 + 25 * 16 + row * 2;
                    (row, machine.read_bus(base), machine.read_bus(base + 1))
                })
                .collect::<Vec<_>>();
            let oam_entry3 = (0..4)
                .map(|offset| machine.read_bus(0xFE00 + 3 * 4 + offset))
                .collect::<Vec<_>>();
            println!(
                "frame={} hblank ly={} tile25_rows={:?} oam_entry3={:?}",
                completed_frames, after.ly, tile25_rows, oam_entry3,
            );
            return;
        }
    }

    panic!("timed out before logging target-frame video writes");
}

fn log_mealybug_m3_lcdc_obj_en_change_with_seeded_trademark_tile() {
    const DMG_BOOT_TRADEMARK_TILE_BYTES: [u8; 16] = [
        0x3C, 0x00, 0x42, 0x00, 0xB9, 0x00, 0xA5, 0x00, 0xB9, 0x00, 0xA5, 0x00, 0x42, 0x00, 0x3C,
        0x00,
    ];

    let mut machine = load_mealybug_m3_lcdc_obj_en_change_machine();
    for (index, byte) in DMG_BOOT_TRADEMARK_TILE_BYTES.iter().copied().enumerate() {
        machine.write_bus(0x8190 + index as u16, byte);
    }
    let seeded_rows = (0..8)
        .map(|row| {
            let base = 0x8190 + row * 2;
            (row, machine.read_bus(base), machine.read_bus(base + 1))
        })
        .collect::<Vec<_>>();
    println!("seeded_tile25_rows={seeded_rows:?}");

    let mut completed_frames = 0_u32;
    let mut at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;

    while completed_frames < 30 {
        machine.step_t_cycle();
        let now_at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;
        if now_at_frame_origin && !at_frame_origin {
            completed_frames += 1;
        }
        at_frame_origin = now_at_frame_origin;
    }

    for y in 24..32 {
        let row_start = y * 160;
        println!(
            "seeded_frame row{} left8={:?}",
            y,
            &machine.ppu().framebuffer()[row_start..row_start + 8]
        );
    }
    let final_rows = (0..8)
        .map(|row| {
            let base = 0x8190 + row * 2;
            (row, machine.read_bus(base), machine.read_bus(base + 1))
        })
        .collect::<Vec<_>>();
    println!("final_tile25_rows={final_rows:?}");
}

fn log_mealybug_m3_lcdc_obj_en_change_seeded_trademark_final_frame_window() {
    const DMG_BOOT_TRADEMARK_TILE_BYTES: [u8; 16] = [
        0x3C, 0x00, 0x42, 0x00, 0xB9, 0x00, 0xA5, 0x00, 0xB9, 0x00, 0xA5, 0x00, 0x42, 0x00, 0x3C,
        0x00,
    ];

    let mut machine = load_mealybug_m3_lcdc_obj_en_change_machine();
    for (index, byte) in DMG_BOOT_TRADEMARK_TILE_BYTES.iter().copied().enumerate() {
        machine.write_bus(0x8190 + index as u16, byte);
    }

    let mut completed_frames = 0_u32;
    let mut at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;

    for _ in 0..80_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();
        if completed_frames == 29
            && before.ly == 24
            && before.mode == PpuAccessMode::Drawing
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF40)
        {
            let activity = before_cpu
                .last_bus_activity
                .expect("FF40 write should expose a bus activity snapshot");
            println!(
                "seeded_target_write value={:#04X} line_dot={} vpo={} x={} obj_low={:#04X} obj_high={:#04X} obj_fifo={:?} tile25_rows={:?}",
                activity.value,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                before.obj_fetcher_tile_low,
                before.obj_fetcher_tile_high,
                before.obj_fifo_pixels,
                (0..8)
                    .map(|row| {
                        let base = 0x8190 + row * 2;
                        (row, machine.read_bus(base), machine.read_bus(base + 1))
                    })
                    .collect::<Vec<_>>(),
            );
        }

        machine.step_t_cycle();

        let now_at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;
        if now_at_frame_origin && !at_frame_origin {
            completed_frames += 1;
        }
        at_frame_origin = now_at_frame_origin;

        let after = machine.ppu().snapshot();
        if completed_frames == 29 && after.ly == 24 && (92..=120).contains(&after.line_dot) {
            println!(
                "seeded_state line_dot={} vpo={} x={} obj_stage={:?} obj_stage_dot={} obj_tile={:?} obj_row={:?} obj_addr={:?}/{:?} obj_low={:#04X} obj_high={:#04X} obj_fifo={:?}",
                after.line_dot,
                after.visible_pixels_output,
                after.bg_current_transfer_x,
                after.obj_fetcher_stage,
                after.obj_fetcher_stage_dot,
                after.obj_fetcher_resolved_tile_index,
                after.obj_fetcher_resolved_tile_row,
                after.obj_fetcher_tile_low_address,
                after.obj_fetcher_tile_high_address,
                after.obj_fetcher_tile_low,
                after.obj_fetcher_tile_high,
                after.obj_fifo_pixels,
            );
            if after.line_dot == 120 {
                return;
            }
        }
    }

    panic!("timed out before logging seeded trademark final-frame window");
}

fn log_mealybug_m3_lcdc_obj_en_change_seeded_trademark_write_fifos(ly_start: u8, ly_end: u8) {
    const DMG_BOOT_TRADEMARK_TILE_BYTES: [u8; 16] = [
        0x3C, 0x00, 0x42, 0x00, 0xB9, 0x00, 0xA5, 0x00, 0xB9, 0x00, 0xA5, 0x00, 0x42, 0x00, 0x3C,
        0x00,
    ];

    let mut machine = load_mealybug_m3_lcdc_obj_en_change_machine();
    for (index, byte) in DMG_BOOT_TRADEMARK_TILE_BYTES.iter().copied().enumerate() {
        machine.write_bus(0x8190 + index as u16, byte);
    }

    let mut completed_frames = 0_u32;
    let mut at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;

    for _ in 0..80_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();
        if completed_frames == 29
            && (ly_start..=ly_end).contains(&before.ly)
            && before.mode == PpuAccessMode::Drawing
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF40)
        {
            let activity = before_cpu
                .last_bus_activity
                .expect("FF40 write should expose a bus activity snapshot");
            println!(
                "seeded_write ly={} value={:#04X} line_dot={} vpo={} x={} fifo={:?} selected={:?}",
                before.ly,
                activity.value,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                before.obj_fifo_pixels,
                before.selected_sprites,
            );
        }

        machine.step_t_cycle();

        let now_at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;
        if now_at_frame_origin && !at_frame_origin {
            completed_frames += 1;
        }
        at_frame_origin = now_at_frame_origin;

        if completed_frames > 29 {
            return;
        }
    }

    panic!("timed out before logging seeded trademark write fifos");
}

fn log_mealybug_m3_lcdc_obj_en_change_variant_seeded_bgp_writes(target_lys: &[u8]) {
    let mut machine = load_mealybug_m3_lcdc_obj_en_change_variant_machine();
    seed_dmg_boot_trademark_tile(&mut machine);
    let targets = target_lys
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut completed_frames = 0_u32;
    let mut at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;

    for _ in 0..80_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();
        if completed_frames == 29
            && targets.contains(&before.ly)
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF47)
        {
            let activity = before_cpu
                .last_bus_activity
                .expect("FF47 write should expose a bus activity snapshot");
            println!(
                "variant_bgp ly={} mode={:?} line_dot={} vpo={} x={} value={:#04X} visible_bgp={:#04X} pipeline_bgp={:#04X} output_override={:?} output_delay={} selected={:?} fifo_prefix={:?}",
                before.ly,
                before.mode,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                activity.value,
                before.visible_bgp,
                before.pipeline_bgp,
                before.dmg_bgp_cpu_commit_output_palette_override,
                before.dmg_bgp_cpu_commit_output_delay_pixels_remaining,
                before.selected_sprites,
                before.obj_fifo_pixels,
            );
        }

        machine.step_t_cycle();

        let now_at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;
        if now_at_frame_origin && !at_frame_origin {
            completed_frames += 1;
        }
        at_frame_origin = now_at_frame_origin;

        if completed_frames > 29 {
            return;
        }
    }

    panic!("timed out before logging variant seeded BGP writes");
}

fn log_mealybug_m3_lcdc_obj_en_change_variant_seeded_hblank_row(target_ly: u8) {
    let mut machine = load_mealybug_m3_lcdc_obj_en_change_variant_machine();
    seed_dmg_boot_trademark_tile(&mut machine);
    let mut completed_frames = 0_u32;
    let mut at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;

    for _ in 0..80_000_000 {
        let before = machine.ppu().snapshot();
        if completed_frames == 29 && before.ly == target_ly && before.mode == PpuAccessMode::HBlank
        {
            println!(
                "variant_row ly={} pixels={:?} panel={:?}",
                target_ly,
                &before.current_scanline_pixels[..24],
                &machine.ppu().framebuffer()
                    [target_ly as usize * 160..target_ly as usize * 160 + 24],
            );
            return;
        }

        machine.step_t_cycle();

        let now_at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;
        if now_at_frame_origin && !at_frame_origin {
            completed_frames += 1;
        }
        at_frame_origin = now_at_frame_origin;
    }

    panic!("timed out before logging variant seeded hblank row");
}

fn log_mealybug_m3_lcdc_obj_en_change_variant_seeded_line_window(
    target_ly: u8,
    visible_x_start: u8,
    visible_x_end: u8,
) {
    let mut machine = load_mealybug_m3_lcdc_obj_en_change_variant_machine();
    seed_dmg_boot_trademark_tile(&mut machine);
    let mut completed_frames = 0_u32;
    let mut at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;

    for _ in 0..80_000_000 {
        let before = machine.ppu().snapshot();
        if completed_frames == 29
            && before.ly == target_ly
            && before.mode == PpuAccessMode::Drawing
            && (visible_x_start..=visible_x_end).contains(&before.visible_pixels_output)
        {
            println!(
                "variant_line ly={} line_dot={} vpo={} x={} visible_bgp={:#04X} pipeline_bgp={:#04X} output_override={:?} output_delay={} current_pixels={:?} panel_prefix={:?}",
                before.ly,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                before.visible_bgp,
                before.pipeline_bgp,
                before.dmg_bgp_cpu_commit_output_palette_override,
                before.dmg_bgp_cpu_commit_output_delay_pixels_remaining,
                &before.current_scanline_pixels
                    [..usize::from(before.visible_pixels_output.min(16))],
                &machine.ppu().framebuffer()[target_ly as usize * 160
                    ..target_ly as usize * 160 + usize::from(before.visible_pixels_output.min(16))],
            );

            if before.visible_pixels_output == visible_x_end {
                return;
            }
        }

        machine.step_t_cycle();

        let now_at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;
        if now_at_frame_origin && !at_frame_origin {
            completed_frames += 1;
        }
        at_frame_origin = now_at_frame_origin;
    }

    panic!("timed out before logging variant seeded line window");
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_obj_en_change ly24 post-write output window"]
fn real_mealybug_m3_lcdc_obj_en_change_logs_ly24_after_ff40_write_window() {
    log_mealybug_m3_lcdc_obj_en_change_after_ff40_write_window(24, 16);
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_obj_en_change ly80 post-write output window"]
fn real_mealybug_m3_lcdc_obj_en_change_logs_ly80_after_ff40_write_window() {
    log_mealybug_m3_lcdc_obj_en_change_after_ff40_write_window(80, 16);
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_obj_en_change representative write signatures"]
fn real_mealybug_m3_lcdc_obj_en_change_logs_representative_write_signatures() {
    log_mealybug_m3_lcdc_obj_en_change_write_signatures(&[
        24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 104, 112, 120,
    ]);
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_obj_en_change ly64 hblank row"]
fn real_mealybug_m3_lcdc_obj_en_change_logs_ly64_hblank_row() {
    log_mealybug_m3_lcdc_obj_en_change_hblank_row(64);
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_obj_en_change ly24 timeline"]
fn real_mealybug_m3_lcdc_obj_en_change_logs_ly24_timeline() {
    log_mealybug_m3_lcdc_obj_en_change_line_timeline(24, 92, 108);
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_obj_en_change tile25 activity"]
fn real_mealybug_m3_lcdc_obj_en_change_logs_tile25_activity() {
    log_mealybug_m3_lcdc_obj_en_change_tile25_activity(&[24, 32]);
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_obj_en_change ly24 final-frame window"]
fn real_mealybug_m3_lcdc_obj_en_change_logs_ly24_final_frame_window() {
    log_mealybug_m3_lcdc_obj_en_change_after_ff40_write_window_in_completed_frame(29, 24, 16);
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_obj_en_change frame29 video writes to ly24"]
fn real_mealybug_m3_lcdc_obj_en_change_logs_frame29_video_writes_to_ly24() {
    log_mealybug_m3_lcdc_obj_en_change_video_writes_until_hblank(29, 24);
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_obj_en_change seeded trademark tile"]
fn real_mealybug_m3_lcdc_obj_en_change_logs_seeded_trademark_tile() {
    log_mealybug_m3_lcdc_obj_en_change_with_seeded_trademark_tile();
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_obj_en_change seeded trademark final-frame window"]
fn real_mealybug_m3_lcdc_obj_en_change_logs_seeded_trademark_final_frame_window() {
    log_mealybug_m3_lcdc_obj_en_change_seeded_trademark_final_frame_window();
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_obj_en_change seeded trademark write fifos"]
fn real_mealybug_m3_lcdc_obj_en_change_logs_seeded_trademark_write_fifos() {
    log_mealybug_m3_lcdc_obj_en_change_seeded_trademark_write_fifos(0, 23);
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_obj_en_change_variant seeded BGP writes"]
fn real_mealybug_m3_lcdc_obj_en_change_variant_logs_seeded_bgp_writes() {
    log_mealybug_m3_lcdc_obj_en_change_variant_seeded_bgp_writes(&[
        0, 1, 7, 24, 40, 64, 88, 112, 120,
    ]);
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_obj_en_change_variant seeded hblank row"]
fn real_mealybug_m3_lcdc_obj_en_change_variant_logs_seeded_hblank_row() {
    log_mealybug_m3_lcdc_obj_en_change_variant_seeded_hblank_row(24);
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_obj_en_change_variant seeded line window"]
fn real_mealybug_m3_lcdc_obj_en_change_variant_logs_seeded_line_window() {
    log_mealybug_m3_lcdc_obj_en_change_variant_seeded_line_window(24, 0, 8);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MealybugLcdc0WriteSignature {
    line_dot: u16,
    visible_pixels_output: u8,
    current_transfer_x: u8,
    startup_fifo_placeholders: u8,
    origin: Option<PpuBgCachedSliceOriginSnapshot>,
    pixel_index: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MealybugLcdc3WriteSignature {
    line_dot: u16,
    visible_pixels_output: u8,
    current_transfer_x: u8,
    startup_fifo_placeholders: u8,
    origin: Option<PpuBgCachedSliceOriginSnapshot>,
    pixel_index: Option<u8>,
    tile_map_address: Option<u16>,
    tile_index: Option<u8>,
}

fn log_mealybug_m3_lcdc_bg_en_change_representative_row_signatures(target_lys: &[u8]) {
    let mut machine = load_mealybug_m3_lcdc_bg_en_change_machine();
    let targets = target_lys
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut write_signatures =
        std::collections::BTreeMap::<u8, Vec<MealybugLcdc0WriteSignature>>::new();
    let mut printed = std::collections::BTreeSet::new();
    let mut saw_progress = false;
    let mut wraps = 0usize;

    for _ in 0..20_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();

        if before.ly != 0 || before.line_dot != 0 {
            saw_progress = true;
        } else if saw_progress {
            wraps += 1;
        }

        if wraps >= 8
            && targets.contains(&before.ly)
            && before.mode == PpuAccessMode::Drawing
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF40)
        {
            let front = before.bg_fifo_cached_pixels.first().copied().flatten();
            write_signatures
                .entry(before.ly)
                .or_default()
                .push(MealybugLcdc0WriteSignature {
                    line_dot: before.line_dot,
                    visible_pixels_output: before.visible_pixels_output,
                    current_transfer_x: before.bg_current_transfer_x,
                    startup_fifo_placeholders: before.bg_startup_fifo_placeholders,
                    origin: front.map(|cached| cached.origin),
                    pixel_index: front.map(|cached| cached.pixel_index),
                });
        }

        machine.step_t_cycle();

        let after = machine.ppu().snapshot();
        if wraps < 8
            || !targets.contains(&after.ly)
            || after.mode != PpuAccessMode::HBlank
            || printed.contains(&after.ly)
        {
            continue;
        }

        let row_start = after.ly as usize * 160;
        let row = &machine.ppu().framebuffer()[row_start..row_start + 40];
        let sprite_xs = after
            .selected_sprites
            .iter()
            .map(|sprite| sprite.x)
            .collect::<Vec<_>>();
        println!(
            "ly={} sprite_xs={:?} row40={:?} writes={:?}",
            after.ly,
            sprite_xs,
            row,
            write_signatures.get(&after.ly).cloned().unwrap_or_default(),
        );
        printed.insert(after.ly);

        if printed.len() == targets.len() {
            return;
        }
    }

    panic!("timed out before logging all target LY signatures");
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_bg_en_change ly1 post-write output window"]
fn real_mealybug_m3_lcdc_bg_en_change_logs_ly1_after_ff40_write_window() {
    log_mealybug_m3_lcdc_bg_en_change_after_ff40_write_window(1, 40);
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_bg_en_change ly67 post-write output window"]
fn real_mealybug_m3_lcdc_bg_en_change_logs_ly67_after_ff40_write_window() {
    log_mealybug_m3_lcdc_bg_en_change_after_ff40_write_window(67, 40);
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_bg_en_change representative row signatures"]
fn real_mealybug_m3_lcdc_bg_en_change_logs_representative_row_signatures() {
    log_mealybug_m3_lcdc_bg_en_change_representative_row_signatures(&[
        0, 8, 16, 24, 32, 40, 49, 57, 64, 72, 80, 88, 96, 105, 113, 121, 128, 136,
    ]);
}

fn log_mealybug_m3_lcdc_bg_map_change_representative_row_signatures(target_lys: &[u8]) {
    let mut machine = load_mealybug_m3_lcdc_bg_map_change_machine();
    let targets = target_lys
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut write_signatures =
        std::collections::BTreeMap::<u8, Vec<MealybugLcdc3WriteSignature>>::new();
    let mut printed = std::collections::BTreeSet::new();
    let mut saw_progress = false;
    let mut wraps = 0usize;

    for _ in 0..20_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();

        if before.ly != 0 || before.line_dot != 0 {
            saw_progress = true;
        } else if saw_progress {
            wraps += 1;
        }

        if wraps >= 8
            && targets.contains(&before.ly)
            && before.mode == PpuAccessMode::Drawing
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF40)
        {
            let front = before.bg_fifo_cached_pixels.first().copied().flatten();
            write_signatures
                .entry(before.ly)
                .or_default()
                .push(MealybugLcdc3WriteSignature {
                    line_dot: before.line_dot,
                    visible_pixels_output: before.visible_pixels_output,
                    current_transfer_x: before.bg_current_transfer_x,
                    startup_fifo_placeholders: before.bg_startup_fifo_placeholders,
                    origin: front.map(|cached| cached.origin),
                    pixel_index: front.map(|cached| cached.pixel_index),
                    tile_map_address: front.map(|cached| cached.tile_map_address),
                    tile_index: front.map(|cached| cached.tile_index),
                });
        }

        machine.step_t_cycle();

        let after = machine.ppu().snapshot();
        if wraps < 8
            || !targets.contains(&after.ly)
            || after.mode != PpuAccessMode::HBlank
            || printed.contains(&after.ly)
        {
            continue;
        }

        let row_start = after.ly as usize * 160;
        let row = &machine.ppu().framebuffer()[row_start..row_start + 40];
        let sprite_xs = after
            .selected_sprites
            .iter()
            .map(|sprite| sprite.x)
            .collect::<Vec<_>>();
        println!(
            "ly={} sprite_xs={:?} row40={:?} writes={:?}",
            after.ly,
            sprite_xs,
            row,
            write_signatures.get(&after.ly).cloned().unwrap_or_default(),
        );
        printed.insert(after.ly);

        if printed.len() == targets.len() {
            return;
        }
    }

    panic!("timed out before logging all target LY signatures");
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_bg_map_change representative row signatures"]
fn real_mealybug_m3_lcdc_bg_map_change_logs_representative_row_signatures() {
    log_mealybug_m3_lcdc_bg_map_change_representative_row_signatures(&[
        0, 8, 16, 24, 32, 40, 49, 57, 64, 72, 80, 88, 96, 105, 113, 121, 128, 136,
    ]);
}

fn log_mealybug_m3_lcdc_bg_map_change_after_ff40_write_window(target_ly: u8, stop_vpo: u8) {
    let mut machine = load_mealybug_m3_lcdc_bg_map_change_machine();
    let mut armed = false;
    let mut write_index = 0usize;
    let mut last_vpo = 0u8;

    for _ in 0..20_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();

        if before.ly == target_ly
            && before.mode == PpuAccessMode::Drawing
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF40)
        {
            let activity = before_cpu
                .last_bus_activity
                .expect("FF40 write should expose a bus activity snapshot");
            println!(
                "write#{} ly={} line_dot={} vpo={} x={} value={:#04X} stage={:?} stage_dot={} fetch_map={:#06X} fetch_data={:#06X} fetch_tile={} fetch_low={:#04X} fetch_high={:#04X} startup={:?} placeholders={} push_pending={} push_cached={:?} fill_pending={} fill_cached={:?} obj_fifo={:?} front_cached={:?}",
                write_index,
                before.ly,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                activity.value,
                before.bg_fetcher_stage,
                before.bg_fetcher_stage_dot,
                before.bg_fetcher_tile_map_address,
                before.bg_fetcher_tile_data_address,
                before.bg_fetcher_tile_index,
                before.bg_fetcher_tile_low,
                before.bg_fetcher_tile_high,
                before.bg_startup_fetch_seam,
                before.bg_startup_fifo_placeholders,
                before.bg_push_pending,
                before.bg_push_cached,
                before.bg_fill_pending,
                before.bg_fill_cached,
                before.obj_fifo_pixels,
                before.bg_fifo_cached_pixels.first(),
            );
            write_index += 1;
            if !armed {
                armed = true;
                last_vpo = before.visible_pixels_output;
            }
        }

        machine.step_t_cycle();

        if !armed {
            continue;
        }

        let after = machine.ppu().snapshot();
        if after.ly != target_ly {
            break;
        }

        if after.visible_pixels_output != last_vpo {
            let visible_x = last_vpo as usize;
            let lcdc = machine.read_bus(0xFF40);
            let panel = machine.ppu().framebuffer()[after.ly as usize * 160 + visible_x];
            println!(
                "emit line_dot={} vpo={} -> {} x={} mixed={} panel={} lcdc={:#04X} stage={:?} stage_dot={} fetch_map={:#06X} fetch_data={:#06X} fetch_tile={} fetch_low={:#04X} fetch_high={:#04X} startup={:?} placeholders={} push_pending={} push_cached={:?} fill_pending={} fill_cached={:?} obj_fifo={:?} front_cached={:?}",
                after.line_dot,
                last_vpo,
                after.visible_pixels_output,
                visible_x,
                after.current_scanline_pixels[visible_x],
                panel,
                lcdc,
                after.bg_fetcher_stage,
                after.bg_fetcher_stage_dot,
                after.bg_fetcher_tile_map_address,
                after.bg_fetcher_tile_data_address,
                after.bg_fetcher_tile_index,
                after.bg_fetcher_tile_low,
                after.bg_fetcher_tile_high,
                after.bg_startup_fetch_seam,
                after.bg_startup_fifo_placeholders,
                after.bg_push_pending,
                after.bg_push_cached,
                after.bg_fill_pending,
                after.bg_fill_cached,
                after.obj_fifo_pixels,
                after.bg_fifo_cached_pixels.first(),
            );
            last_vpo = after.visible_pixels_output;
            if after.visible_pixels_output >= stop_vpo {
                return;
            }
        }
    }

    panic!("timed out before logging the target LY output window after FF40 writes");
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_bg_map_change ly8 post-write output window"]
fn real_mealybug_m3_lcdc_bg_map_change_logs_ly8_after_ff40_write_window() {
    log_mealybug_m3_lcdc_bg_map_change_after_ff40_write_window(8, 24);
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_bg_map_change ly24 post-write output window"]
fn real_mealybug_m3_lcdc_bg_map_change_logs_ly24_after_ff40_write_window() {
    log_mealybug_m3_lcdc_bg_map_change_after_ff40_write_window(24, 24);
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_bg_map_change ly128 post-write output window"]
fn real_mealybug_m3_lcdc_bg_map_change_logs_ly128_after_ff40_write_window() {
    log_mealybug_m3_lcdc_bg_map_change_after_ff40_write_window(128, 24);
}

fn log_mealybug_m3_lcdc_bg_map_change_high_band_tile_sources(target_lys: &[u8]) {
    let mut machine = load_mealybug_m3_lcdc_bg_map_change_machine();
    let targets = target_lys
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut printed = std::collections::BTreeSet::new();

    for _ in 0..20_000_000 {
        machine.step_t_cycle();

        let after = machine.ppu().snapshot();
        if after.mode != PpuAccessMode::HBlank
            || !targets.contains(&after.ly)
            || printed.contains(&after.ly)
        {
            continue;
        }

        let bg_row = after.ly & 0x07;
        let map0 = (0_u16..=2)
            .map(|col| {
                let tile = machine.read_bus(0x9800 + (u16::from(after.ly / 8) * 32) + col);
                let low = machine.read_bus(0x8000 + u16::from(tile) * 16 + u16::from(bg_row) * 2);
                let high =
                    machine.read_bus(0x8000 + u16::from(tile) * 16 + u16::from(bg_row) * 2 + 1);
                (col, tile, low, high)
            })
            .collect::<Vec<_>>();
        let map1 = (0_u16..=2)
            .map(|col| {
                let tile = machine.read_bus(0x9C00 + (u16::from(after.ly / 8) * 32) + col);
                let low = machine.read_bus(0x8000 + u16::from(tile) * 16 + u16::from(bg_row) * 2);
                let high =
                    machine.read_bus(0x8000 + u16::from(tile) * 16 + u16::from(bg_row) * 2 + 1);
                (col, tile, low, high)
            })
            .collect::<Vec<_>>();
        println!(
            "ly={} bg_row={} map0={:?} map1={:?} row0_23={:?}",
            after.ly,
            bg_row,
            map0,
            map1,
            &machine.ppu().framebuffer()[after.ly as usize * 160..after.ly as usize * 160 + 24],
        );
        printed.insert(after.ly);
        if printed.len() == targets.len() {
            return;
        }
    }

    panic!("timed out before logging high-band tile sources");
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_bg_map_change high-band tile sources"]
fn real_mealybug_m3_lcdc_bg_map_change_logs_high_band_tile_sources() {
    log_mealybug_m3_lcdc_bg_map_change_high_band_tile_sources(&[
        128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143,
    ]);
}

fn log_mealybug_m3_lcdc_bg_map_change_high_band_obj_rows(target_lys: &[u8]) {
    let mut machine = load_mealybug_m3_lcdc_bg_map_change_machine();
    let targets = target_lys
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut printed = std::collections::BTreeSet::new();
    let mut saw_progress = false;
    let mut wraps = 0usize;

    for _ in 0..20_000_000 {
        machine.step_t_cycle();

        let after = machine.ppu().snapshot();
        if after.ly != 0 || after.line_dot != 0 {
            saw_progress = true;
        } else if saw_progress {
            wraps += 1;
        }

        if after.mode != PpuAccessMode::HBlank
            || wraps < 8
            || !targets.contains(&after.ly)
            || printed.contains(&after.ly)
        {
            continue;
        }

        let sprite = after
            .selected_sprites
            .first()
            .copied()
            .expect("target rows should keep one selected sprite");
        let obj_height = if after.lcdc & 0x04 != 0 { 16 } else { 8 };
        let sprite_top = sprite.y.wrapping_sub(16);
        let mut row = after.ly.wrapping_sub(sprite_top);
        if sprite.attributes & 0x40 != 0 {
            row = obj_height - 1 - row;
        }
        let tile_index = if obj_height == 16 {
            let base = sprite.tile_index & !0x01;
            if row < 8 { base } else { base + 1 }
        } else {
            sprite.tile_index
        };
        let tile_row = if obj_height == 16 && row >= 8 {
            row - 8
        } else {
            row
        };
        let low = machine.read_bus(0x8000 + u16::from(tile_index) * 16 + u16::from(tile_row) * 2);
        let high =
            machine.read_bus(0x8000 + u16::from(tile_index) * 16 + u16::from(tile_row) * 2 + 1);
        let bg0_low = machine.read_bus(0x8000 + u16::from(tile_row) * 2);
        let bg0_high = machine.read_bus(0x8000 + u16::from(tile_row) * 2 + 1);
        let bg1_low = machine.read_bus(0x8010 + u16::from(tile_row) * 2);
        let bg1_high = machine.read_bus(0x8010 + u16::from(tile_row) * 2 + 1);
        println!(
            "ly={} sprite={:?} obj_height={} row={} tile_index={:#04X} low={:#04X} high={:#04X} bg0=({:#04X},{:#04X}) bg1=({:#04X},{:#04X})",
            after.ly,
            sprite,
            obj_height,
            tile_row,
            tile_index,
            low,
            high,
            bg0_low,
            bg0_high,
            bg1_low,
            bg1_high,
        );
        printed.insert(after.ly);
        if printed.len() == targets.len() {
            return;
        }
    }

    panic!("timed out before logging high-band object rows");
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_bg_map_change high-band object rows"]
fn real_mealybug_m3_lcdc_bg_map_change_logs_high_band_obj_rows() {
    log_mealybug_m3_lcdc_bg_map_change_high_band_obj_rows(&[
        128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143,
    ]);
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_bg_map_change dump vram at ly128 hblank"]
fn real_mealybug_m3_lcdc_bg_map_change_dump_vram_at_ly128_hblank() {
    let mut machine = load_mealybug_m3_lcdc_bg_map_change_machine();
    let mut saw_progress = false;
    let mut wraps = 0usize;

    for _ in 0..20_000_000 {
        machine.step_t_cycle();

        let after = machine.ppu().snapshot();
        if after.ly != 0 || after.line_dot != 0 {
            saw_progress = true;
        } else if saw_progress {
            wraps += 1;
        }

        if wraps < 8 || after.mode != PpuAccessMode::HBlank || after.ly != 128 {
            continue;
        }

        let debug = format!("{:?}", machine.bus());
        let vram_start = debug
            .find("vram: VramDomain { bytes: [")
            .expect("bus debug should expose VRAM bytes");
        let bytes_start = vram_start + "vram: VramDomain { bytes: [".len();
        let bytes_end = debug[bytes_start..]
            .find("], acquired_by:")
            .map(|offset| bytes_start + offset)
            .expect("VRAM bytes list should terminate");
        let vram_bytes = debug[bytes_start..bytes_end]
            .split(',')
            .map(|value| value.trim().parse::<u8>().expect("VRAM byte should parse"))
            .collect::<Vec<_>>();
        println!(
            "tiledata_0190_01B0={:?} tiledata_1000_1020={:?} map0_1A00_1A10={:?} map1_1E00_1E10={:?}",
            &vram_bytes[0x0190..0x01B0],
            &vram_bytes[0x1000..0x1020],
            &vram_bytes[0x1A00..0x1A10],
            &vram_bytes[0x1E00..0x1E10],
        );
        return;
    }

    panic!("timed out before dumping VRAM at ly128 hblank");
}

#[test]
#[ignore = "diag: real mealybug m3_lcdc_bg_map_change search blob tile in vram"]
fn real_mealybug_m3_lcdc_bg_map_change_search_blob_tile_in_vram() {
    let mut machine = load_mealybug_m3_lcdc_bg_map_change_machine();
    let mut saw_progress = false;
    let mut wraps = 0usize;
    let blob_rows = [0x3C_u8, 0x42, 0xB9, 0xA5, 0xB9, 0xA5, 0x42, 0x3C];

    for _ in 0..20_000_000 {
        machine.step_t_cycle();

        let after = machine.ppu().snapshot();
        if after.ly != 0 || after.line_dot != 0 {
            saw_progress = true;
        } else if saw_progress {
            wraps += 1;
        }

        if wraps < 8 || after.mode != PpuAccessMode::HBlank || after.ly != 128 {
            continue;
        }

        let debug = format!("{:?}", machine.bus());
        let vram_start = debug
            .find("vram: VramDomain { bytes: [")
            .expect("bus debug should expose VRAM bytes");
        let bytes_start = vram_start + "vram: VramDomain { bytes: [".len();
        let bytes_end = debug[bytes_start..]
            .find("], acquired_by:")
            .map(|offset| bytes_start + offset)
            .expect("VRAM bytes list should terminate");
        let vram_bytes = debug[bytes_start..bytes_end]
            .split(',')
            .map(|value| value.trim().parse::<u8>().expect("VRAM byte should parse"))
            .collect::<Vec<_>>();

        let mut exact_matches = Vec::new();
        let mut or_matches = Vec::new();
        let mut low_matches = Vec::new();
        let mut high_matches = Vec::new();
        let mut xor_matches = Vec::new();
        for tile_base in (0..0x1800).step_by(16) {
            let mut exact_matched = true;
            let mut or_matched = true;
            let mut low_matched = true;
            let mut high_matched = true;
            let mut xor_matched = true;
            for (row, expected) in blob_rows.iter().copied().enumerate() {
                let low = vram_bytes[tile_base + row * 2];
                let high = vram_bytes[tile_base + row * 2 + 1];
                if low != expected || high != expected {
                    exact_matched = false;
                }
                if low | high != expected {
                    or_matched = false;
                }
                if low != expected {
                    low_matched = false;
                }
                if high != expected {
                    high_matched = false;
                }
                if low ^ high != expected {
                    xor_matched = false;
                }
            }
            if exact_matched {
                exact_matches.push(tile_base / 16);
            }
            if or_matched {
                or_matches.push(tile_base / 16);
            }
            if low_matched {
                low_matches.push(tile_base / 16);
            }
            if high_matched {
                high_matches.push(tile_base / 16);
            }
            if xor_matched {
                xor_matches.push(tile_base / 16);
            }
        }

        println!(
            "blob_like_tiles exact={:?} or={:?} low={:?} high={:?} xor={:?}",
            exact_matches, or_matches, low_matches, high_matches, xor_matches
        );
        return;
    }

    panic!("timed out before searching VRAM for blob tile");
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

#[test]
#[ignore = "diag: logs FF40 writes and OBJ fetch states for m3_lcdc_obj_size_change"]
fn real_mealybug_m3_lcdc_obj_size_change_logs_target_lines() {
    let mut machine = load_mealybug_m3_lcdc_obj_size_change_machine();
    let target_lys = [8_u8, 24, 40];
    let mut completed_frames = 0_u32;
    let mut at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;
    let mut last_signature = None;

    for _ in 0..80_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();
        if completed_frames == 29
            && target_lys.contains(&before.ly)
            && before.mode == PpuAccessMode::Drawing
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF40)
        {
            let activity = before_cpu
                .last_bus_activity
                .expect("FF40 write should expose a bus activity snapshot");
            println!(
                "size_change_write ly={} value={:#04X} line_dot={} vpo={} x={} obj_stage={:?} obj_stage_dot={} sprite={:?} resolved={:?} tile={:?}/{:?}",
                before.ly,
                activity.value,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                before.obj_fetcher_stage,
                before.obj_fetcher_stage_dot,
                before.obj_fetcher_requested_sprite.map(|sprite| sprite.x),
                before.obj_fetcher_resolved_sprite.map(|sprite| sprite.x),
                before.obj_fetcher_resolved_tile_index,
                before.obj_fetcher_resolved_tile_row,
            );
        }

        machine.step_t_cycle();

        let now_at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;
        if now_at_frame_origin && !at_frame_origin {
            completed_frames += 1;
        }
        at_frame_origin = now_at_frame_origin;

        let after = machine.ppu().snapshot();
        if completed_frames != 29 || !target_lys.contains(&after.ly) {
            continue;
        }

        let signature = (
            after.ly,
            after.line_dot,
            after.obj_fetcher_stage,
            after.obj_fetcher_stage_dot,
            after.obj_fetcher_requested_sprite.map(|sprite| sprite.x),
            after.obj_fetcher_resolved_sprite.map(|sprite| sprite.x),
            after.obj_fetcher_resolved_tile_index,
            after.obj_fetcher_resolved_tile_row,
            after.obj_fetcher_tile_low,
            after.obj_fetcher_tile_high,
            machine.read_bus(0xFF40),
        );
        if !matches!(after.obj_fetcher_stage, PpuObjFetcherStage::Idle)
            && last_signature != Some(signature)
        {
            println!(
                "size_change_state ly={} line_dot={} vpo={} x={} lcdc={:#04X} obj_stage={:?} obj_stage_dot={} sprite={:?} resolved={:?} tile={:?}/{:?} low={:#04X} high={:#04X}",
                after.ly,
                after.line_dot,
                after.visible_pixels_output,
                after.bg_current_transfer_x,
                machine.read_bus(0xFF40),
                after.obj_fetcher_stage,
                after.obj_fetcher_stage_dot,
                after.obj_fetcher_requested_sprite.map(|sprite| sprite.x),
                after.obj_fetcher_resolved_sprite.map(|sprite| sprite.x),
                after.obj_fetcher_resolved_tile_index,
                after.obj_fetcher_resolved_tile_row,
                after.obj_fetcher_tile_low,
                after.obj_fetcher_tile_high,
            );
            last_signature = Some(signature);
        }

        if completed_frames == 29 && after.ly > 40 {
            return;
        }
    }

    panic!("timed out before logging the target lines");
}

#[test]
#[ignore = "diag: focused FF40/object windows for m3_lcdc_obj_size_change residual seams"]
fn real_mealybug_m3_lcdc_obj_size_change_logs_focus_lines() {
    let mut machine = load_mealybug_m3_lcdc_obj_size_change_machine();
    let target_lys = [2_u8, 4, 8, 20, 24, 34, 130, 136];
    let targets = target_lys
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut completed_frames = 0_u32;
    let mut at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;
    let mut printed_hblank = std::collections::BTreeSet::new();
    let mut line_write_counts = [0_u8; 256];

    for _ in 0..80_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();
        if completed_frames == 29
            && targets.contains(&before.ly)
            && before.mode == PpuAccessMode::Drawing
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF40)
        {
            let activity = before_cpu
                .last_bus_activity
                .expect("FF40 write should expose a bus activity snapshot");
            let line_write_index = &mut line_write_counts[before.ly as usize];
            *line_write_index += 1;
            println!(
                "focus_write ly={} pulse={} value={:#04X} line_dot={} vpo={} x={} obj_stage={:?} obj_stage_dot={} obj_heights={}/{} line_start_height={} sprite={:?} resolved={:?} tile={:?}/{:?} low={:#04X} high={:#04X}",
                before.ly,
                *line_write_index,
                activity.value,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                before.obj_fetcher_stage,
                before.obj_fetcher_stage_dot,
                before.obj_fetcher_selected_obj_height,
                before.obj_fetcher_latched_obj_height,
                before.obj_mode3_line_start_obj_height,
                before.obj_fetcher_requested_sprite.map(|sprite| sprite.x),
                before.obj_fetcher_resolved_sprite.map(|sprite| sprite.x),
                before.obj_fetcher_resolved_tile_index,
                before.obj_fetcher_resolved_tile_row,
                before.obj_fetcher_tile_low,
                before.obj_fetcher_tile_high,
            );
        }

        machine.step_t_cycle();

        let now_at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;
        if now_at_frame_origin && !at_frame_origin {
            completed_frames += 1;
        }
        at_frame_origin = now_at_frame_origin;

        if completed_frames != 29 {
            continue;
        }

        let after = machine.ppu().snapshot();
        if after.line_dot == 0 {
            line_write_counts[after.ly as usize] = 0;
        }
        if targets.contains(&after.ly)
            && after.mode == PpuAccessMode::Drawing
            && (92..=150).contains(&after.line_dot)
            && !matches!(after.obj_fetcher_stage, PpuObjFetcherStage::Idle)
        {
            println!(
                "focus_state ly={} line_dot={} vpo={} x={} lcdc={:#04X} obj_stage={:?} obj_stage_dot={} obj_heights={}/{} line_start_height={} sprite={:?} resolved={:?} tile={:?}/{:?} low={:#04X} high={:#04X}",
                after.ly,
                after.line_dot,
                after.visible_pixels_output,
                after.bg_current_transfer_x,
                machine.read_bus(0xFF40),
                after.obj_fetcher_stage,
                after.obj_fetcher_stage_dot,
                after.obj_fetcher_selected_obj_height,
                after.obj_fetcher_latched_obj_height,
                after.obj_mode3_line_start_obj_height,
                after.obj_fetcher_requested_sprite.map(|sprite| sprite.x),
                after.obj_fetcher_resolved_sprite.map(|sprite| sprite.x),
                after.obj_fetcher_resolved_tile_index,
                after.obj_fetcher_resolved_tile_row,
                after.obj_fetcher_tile_low,
                after.obj_fetcher_tile_high,
            );
        }

        if after.mode == PpuAccessMode::HBlank
            && targets.contains(&after.ly)
            && printed_hblank.insert(after.ly)
        {
            let row_start = after.ly as usize * 160;
            let sprite_bytes = after
                .selected_sprites
                .iter()
                .copied()
                .map(|sprite| {
                    let sprite_top = sprite.y.wrapping_sub(16);
                    let raw_row = after.ly.wrapping_sub(sprite_top);
                    let mut line_start16_row = raw_row;
                    if sprite.attributes & 0x40 != 0 {
                        line_start16_row = 15 - line_start16_row;
                    }
                    let line_start16_tile = (sprite.tile_index & !0x01)
                        + u8::from(line_start16_row >= 8);
                    let line_start16_tile_row = if line_start16_row >= 8 {
                        line_start16_row - 8
                    } else {
                        line_start16_row
                    };
                    let line_start16_low = machine.read_bus(
                        0x8000
                            + u16::from(line_start16_tile) * 16
                            + u16::from(line_start16_tile_row) * 2,
                    );
                    let line_start16_high = machine.read_bus(
                        0x8000
                            + u16::from(line_start16_tile) * 16
                            + u16::from(line_start16_tile_row) * 2
                            + 1,
                    );

                    let mut live8_row = raw_row & 0x07;
                    if sprite.attributes & 0x40 != 0 {
                        live8_row = 7 - live8_row;
                    }
                    let live8_low = machine.read_bus(
                        0x8000 + u16::from(sprite.tile_index) * 16 + u16::from(live8_row) * 2,
                    );
                    let live8_high = machine.read_bus(
                        0x8000 + u16::from(sprite.tile_index) * 16 + u16::from(live8_row) * 2 + 1,
                    );

                    format!(
                        "x={} y={} tile={:#04X} attr={:#04X} raw_row={} line16=({:#04X},{:#04X}; tile={:#04X} row={}) live8=({:#04X},{:#04X}; row={})",
                        sprite.x,
                        sprite.y,
                        sprite.tile_index,
                        sprite.attributes,
                        raw_row,
                        line_start16_low,
                        line_start16_high,
                        line_start16_tile,
                        line_start16_tile_row,
                        live8_low,
                        live8_high,
                        live8_row,
                    )
                })
                .collect::<Vec<_>>();
            println!(
                "focus_hblank ly={} mode0_start_dot={} sprites={:?} row0_39={:?}",
                after.ly,
                after.mode0_start_dot,
                sprite_bytes,
                &machine.ppu().framebuffer()[row_start..row_start + 40],
            );
            if printed_hblank.len() == targets.len() {
                return;
            }
        }
    }

    panic!("timed out before logging the focus lines");
}

#[test]
#[ignore = "diag: focused FF40/object windows for m3_lcdc_obj_size_change_scx residual seams"]
fn real_mealybug_m3_lcdc_obj_size_change_scx_logs_focus_lines() {
    let mut machine = load_mealybug_m3_lcdc_obj_size_change_scx_machine();
    let target_lys = [2_u8, 4, 18, 34, 66, 74, 76, 130, 132];
    let targets = target_lys
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let mut completed_frames = 0_u32;
    let mut at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;
    let mut printed_hblank = std::collections::BTreeSet::new();
    let mut line_write_counts = [0_u8; 256];

    for _ in 0..80_000_000 {
        let before = machine.ppu().snapshot();
        let before_cpu = machine.cpu().snapshot();
        if completed_frames == 29
            && targets.contains(&before.ly)
            && before.mode == PpuAccessMode::Drawing
            && let Some(event) = before_cpu.last_address_event
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF40)
        {
            let activity = before_cpu
                .last_bus_activity
                .expect("FF40 write should expose a bus activity snapshot");
            let line_write_index = &mut line_write_counts[before.ly as usize];
            *line_write_index += 1;
            println!(
                "scx_focus_write ly={} scx={} pulse={} value={:#04X} line_dot={} vpo={} x={} obj_stage={:?} obj_stage_dot={} obj_heights={}/{} line_start_height={} sprite={:?} resolved={:?} tile={:?}/{:?} low={:#04X} high={:#04X}",
                before.ly,
                machine.read_bus(0xFF43),
                *line_write_index,
                activity.value,
                before.line_dot,
                before.visible_pixels_output,
                before.bg_current_transfer_x,
                before.obj_fetcher_stage,
                before.obj_fetcher_stage_dot,
                before.obj_fetcher_selected_obj_height,
                before.obj_fetcher_latched_obj_height,
                before.obj_mode3_line_start_obj_height,
                before.obj_fetcher_requested_sprite.map(|sprite| sprite.x),
                before.obj_fetcher_resolved_sprite.map(|sprite| sprite.x),
                before.obj_fetcher_resolved_tile_index,
                before.obj_fetcher_resolved_tile_row,
                before.obj_fetcher_tile_low,
                before.obj_fetcher_tile_high,
            );
        }

        machine.step_t_cycle();

        let now_at_frame_origin = machine.ppu().ly() == 0 && machine.ppu().line_dot() == 0;
        if now_at_frame_origin && !at_frame_origin {
            completed_frames += 1;
        }
        at_frame_origin = now_at_frame_origin;

        if completed_frames != 29 {
            continue;
        }

        let after = machine.ppu().snapshot();
        if after.line_dot == 0 {
            line_write_counts[after.ly as usize] = 0;
        }
        if targets.contains(&after.ly)
            && after.mode == PpuAccessMode::Drawing
            && (88..=150).contains(&after.line_dot)
            && !matches!(after.obj_fetcher_stage, PpuObjFetcherStage::Idle)
        {
            println!(
                "scx_focus_state ly={} scx={} line_dot={} vpo={} x={} lcdc={:#04X} obj_stage={:?} obj_stage_dot={} obj_heights={}/{} line_start_height={} sprite={:?} resolved={:?} tile={:?}/{:?} low={:#04X} high={:#04X}",
                after.ly,
                machine.read_bus(0xFF43),
                after.line_dot,
                after.visible_pixels_output,
                after.bg_current_transfer_x,
                machine.read_bus(0xFF40),
                after.obj_fetcher_stage,
                after.obj_fetcher_stage_dot,
                after.obj_fetcher_selected_obj_height,
                after.obj_fetcher_latched_obj_height,
                after.obj_mode3_line_start_obj_height,
                after.obj_fetcher_requested_sprite.map(|sprite| sprite.x),
                after.obj_fetcher_resolved_sprite.map(|sprite| sprite.x),
                after.obj_fetcher_resolved_tile_index,
                after.obj_fetcher_resolved_tile_row,
                after.obj_fetcher_tile_low,
                after.obj_fetcher_tile_high,
            );
        }

        if after.mode == PpuAccessMode::HBlank
            && targets.contains(&after.ly)
            && printed_hblank.insert(after.ly)
        {
            let row_start = after.ly as usize * 160;
            let sprite_bytes = after
                .selected_sprites
                .iter()
                .copied()
                .map(|sprite| {
                    let sprite_top = sprite.y.wrapping_sub(16);
                    let raw_row = after.ly.wrapping_sub(sprite_top);
                    let mut line_start16_row = raw_row;
                    if sprite.attributes & 0x40 != 0 {
                        line_start16_row = 15 - line_start16_row;
                    }
                    let line_start16_tile = (sprite.tile_index & !0x01)
                        + u8::from(line_start16_row >= 8);
                    let line_start16_tile_row = if line_start16_row >= 8 {
                        line_start16_row - 8
                    } else {
                        line_start16_row
                    };
                    let line_start16_low = machine.read_bus(
                        0x8000
                            + u16::from(line_start16_tile) * 16
                            + u16::from(line_start16_tile_row) * 2,
                    );
                    let line_start16_high = machine.read_bus(
                        0x8000
                            + u16::from(line_start16_tile) * 16
                            + u16::from(line_start16_tile_row) * 2
                            + 1,
                    );

                    let mut live8_row = raw_row & 0x07;
                    if sprite.attributes & 0x40 != 0 {
                        live8_row = 7 - live8_row;
                    }
                    let live8_low = machine.read_bus(
                        0x8000 + u16::from(sprite.tile_index) * 16 + u16::from(live8_row) * 2,
                    );
                    let live8_high = machine.read_bus(
                        0x8000 + u16::from(sprite.tile_index) * 16 + u16::from(live8_row) * 2 + 1,
                    );

                    format!(
                        "x={} y={} tile={:#04X} attr={:#04X} raw_row={} line16=({:#04X},{:#04X}; tile={:#04X} row={}) live8=({:#04X},{:#04X}; row={})",
                        sprite.x,
                        sprite.y,
                        sprite.tile_index,
                        sprite.attributes,
                        raw_row,
                        line_start16_low,
                        line_start16_high,
                        line_start16_tile,
                        line_start16_tile_row,
                        live8_low,
                        live8_high,
                        live8_row,
                    )
                })
                .collect::<Vec<_>>();
            println!(
                "scx_focus_hblank ly={} scx={} mode0_start_dot={} sprites={:?} row0_39={:?}",
                after.ly,
                machine.read_bus(0xFF43),
                after.mode0_start_dot,
                sprite_bytes,
                &machine.ppu().framebuffer()[row_start..row_start + 40],
            );
            if printed_hblank.len() == targets.len() {
                return;
            }
        }
    }

    panic!("timed out before logging the SCX focus lines");
}
