mod common;

use common::machine_driver::run_until_halted;
use common::synthetic_cartridge::{HEADER_MINIMUM_ROM_LEN, build_nom_bc_test_rom};
use gb_core::{
    ConsoleModel, CpuAddressEventKind, CpuAddressUpdateDirection, CpuBusAccessKind, Machine,
    MachineConfig, OperatingMode, PpuAccessMode, PpuBgFetcherSource, PpuLcdState,
    PpuObjFetcherStage, PpuSnapshot, PpuVisibleOutputState, StartupMode,
};

include!("ppu/ppu_setup.rs");
include!("ppu/ppu_probe_lcd.rs");
include!("ppu/ppu_probe_intr20.rs");
include!("ppu/ppu_probe_stat_mode.rs");
include!("ppu/ppu_probe_hblank_scx.rs");

#[path = "ppu/ppu_diag.rs"]
mod ppu_diag;
#[path = "ppu/ppu_lcd_restart.rs"]
mod ppu_lcd_restart;
#[path = "ppu/ppu_mode_edges.rs"]
mod ppu_mode_edges;
#[path = "ppu/ppu_oam_dma.rs"]
mod ppu_oam_dma;

fn step_until_visible_pixels_output_on_line(
    machine: &mut Machine,
    target_ly: u8,
    min_visible_pixels_output: u8,
) {
    let mut stepped_t_cycles = 0u32;
    loop {
        let snapshot = machine.ppu().snapshot();
        if snapshot.ly == target_ly
            && snapshot.mode == PpuAccessMode::Drawing
            && snapshot.visible_pixels_output >= min_visible_pixels_output
        {
            return;
        }

        machine.step_t_cycle();
        stepped_t_cycles += 1;
        assert!(
            stepped_t_cycles < 2_000,
            "did not reach ly={} drawing with {} visible pixels in time; last snapshot={:?}",
            target_ly,
            min_visible_pixels_output,
            machine.ppu().snapshot()
        );
    }
}

fn run_live_bgp_write_prefix(config: MachineConfig) -> (Vec<u8>, Vec<u8>) {
    let mut machine = Machine::new(config);
    machine.write_bus(0xFF40, 0x91);
    machine.write_bus(0xFF42, 0x00);
    machine.write_bus(0xFF43, 0x00);
    machine.write_bus(0xFF47, 0x01);

    seed_bg_tile_row(&mut machine, 0, 0, 0x00, 0x00);
    for tile_x in 0..4 {
        seed_bg_tilemap_entry(&mut machine, tile_x, 0, 0);
    }

    step_until_visible_pixels_output_on_line(&mut machine, 0, 8);

    let before = machine.ppu().framebuffer()[..8].to_vec();
    machine.write_bus(0xFF47, 0x12);
    let after = machine.ppu().framebuffer()[..8].to_vec();
    (before, after)
}

#[test]
fn live_machine_bus_access_uses_the_current_ppu_mode_from_the_raster_state() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.step_t_cycle();
    assert_eq!(machine.ppu().snapshot().mode, PpuAccessMode::OamScan);
    assert_eq!(machine.read_bus(0xFE00), 0xFF);

    for _ in 1..80 {
        machine.step_t_cycle();
    }

    let drawing = machine.ppu().snapshot();
    assert_eq!(drawing.mode, PpuAccessMode::Drawing);
    assert_eq!(drawing.line_dot, 80);
    assert_eq!(machine.read_bus(0x8000), 0xFF);
}

#[test]
fn skip_boot_ppu_state_continues_from_the_published_snapshot_on_the_shared_timeline() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    let startup = machine.ppu().snapshot();
    assert_eq!(startup.mode, PpuAccessMode::VBlank);
    assert_eq!(startup.ly, 0);
    assert_eq!(startup.line_dot, 0);
    assert_eq!(startup.lcd_state, PpuLcdState::Enabled);
    assert_eq!(startup.visible_output, PpuVisibleOutputState::Driving);

    machine.step_t_cycle();

    let after_first_dot = machine.ppu().snapshot();
    assert_eq!(after_first_dot.mode, PpuAccessMode::OamScan);
    assert_eq!(after_first_dot.ly, 0);
    assert_eq!(after_first_dot.line_dot, 1);
    assert_eq!(after_first_dot.mode_dot, 1);
}

#[test]
fn bg_only_mode3_produces_visible_pixels_from_vram_on_the_machine_timeline() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_bg_tile_row(&mut machine, 0, 0, 0x55, 0x33);
    seed_bg_tile_row(&mut machine, 1, 0, 0xAA, 0xCC);
    seed_bg_tilemap_entry(&mut machine, 0, 0, 0);
    seed_bg_tilemap_entry(&mut machine, 1, 0, 1);

    step_until_line_dot(&mut machine, 252);

    let snapshot = machine.ppu().snapshot();
    assert_eq!(snapshot.mode, PpuAccessMode::HBlank);
    assert_eq!(snapshot.mode0_start_dot, 252);
    assert_eq!(snapshot.visible_pixels_output, 160);
    assert_eq!(
        &snapshot.current_scanline_pixels[..16],
        &[0, 1, 2, 3, 0, 1, 2, 3, 3, 2, 1, 0, 3, 2, 1, 0]
    );
}

#[test]
fn cgb_compatibility_machine_keeps_bgp_palette_conflict_quirks_disabled() {
    let dmg_config = MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot);
    let cgb_compat_config = MachineConfig::new(ConsoleModel::Cgb)
        .with_operating_mode(OperatingMode::CgbCompatibility)
        .with_startup_mode(StartupMode::SkipBoot);

    let (dmg_before, dmg_after) = run_live_bgp_write_prefix(dmg_config);
    let (cgb_before, cgb_after) = run_live_bgp_write_prefix(cgb_compat_config.clone());

    let capabilities = cgb_compat_config.capability_set();
    assert!(capabilities.dmg_software_contract());
    assert!(!capabilities.dmg_family_quirks_enabled());

    assert_eq!(dmg_before, vec![1; 8]);
    assert_ne!(dmg_after, dmg_before);
    assert_eq!(cgb_before, vec![1; 8]);
    assert_eq!(cgb_after, cgb_before);
}

#[test]
fn scx_discard_keeps_vram_blocked_until_the_variable_mode3_end() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0x8000, 0x12);
    machine.write_bus(0xFF43, 0x07);

    step_until_line_dot(&mut machine, 252);

    let extended_drawing = machine.ppu().snapshot();
    assert_eq!(extended_drawing.mode, PpuAccessMode::Drawing);
    assert_eq!(extended_drawing.mode0_start_dot, 259);
    assert_eq!(machine.read_bus(0x8000), 0xFF);

    step_until_line_dot(&mut machine, 259);

    let hblank = machine.ppu().snapshot();
    assert_eq!(hblank.mode, PpuAccessMode::HBlank);
    assert_eq!(hblank.mode0_start_dot, 259);
    assert_eq!(machine.read_bus(0x8000), 0x12);
}

#[test]
fn window_starts_mid_scanline_on_the_live_machine_without_recomputing_the_bg_prefix() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF40, 0xF1);
    machine.write_bus(0xFF4A, 0x00);
    machine.write_bus(0xFF4B, 0x0F);

    seed_bg_tile_row(&mut machine, 0, 0, 0x55, 0x33);
    seed_bg_tile_row(&mut machine, 1, 0, 0xCC, 0xF0);
    seed_bg_tilemap_entry(&mut machine, 0, 0, 0);
    seed_window_tilemap_entry(&mut machine, 0, 0, 1);

    step_until_line_dot(&mut machine, 270);

    let snapshot = machine.ppu().snapshot();
    assert_eq!(snapshot.mode, PpuAccessMode::HBlank);
    assert_eq!(snapshot.bg_fetcher_source, PpuBgFetcherSource::Window);
    assert!(snapshot.window_started_this_line);
    assert_eq!(
        &snapshot.current_scanline_pixels[..16],
        &[0, 1, 2, 3, 0, 1, 2, 3, 3, 3, 2, 2, 1, 1, 0, 0]
    );
}

#[test]
fn window_status_bar_style_activation_uses_the_internal_line_counter_on_later_lines() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF40, 0xF1);
    machine.write_bus(0xFF4A, 0x01);
    machine.write_bus(0xFF4B, 0x07);

    seed_bg_tile_row(&mut machine, 0, 0, 0x55, 0x33);
    seed_bg_tile_row(&mut machine, 1, 0, 0xCC, 0xF0);
    seed_bg_tilemap_entry(&mut machine, 0, 0, 0);
    seed_window_tilemap_entry(&mut machine, 0, 0, 1);

    while !(machine.ppu().snapshot().ly == 1
        && machine.ppu().snapshot().mode == PpuAccessMode::HBlank)
    {
        machine.step_t_cycle();
    }

    let snapshot = machine.ppu().snapshot();
    assert_eq!(snapshot.window_line_counter, 0);
    assert_eq!(snapshot.bg_fetcher_source, PpuBgFetcherSource::Window);
    assert_eq!(
        &snapshot.current_scanline_pixels[..8],
        &[3, 3, 2, 2, 1, 1, 0, 0]
    );

    while !(machine.ppu().snapshot().ly == 2 && machine.ppu().snapshot().line_dot == 0) {
        machine.step_t_cycle();
    }

    assert_eq!(machine.ppu().snapshot().window_line_counter, 1);
}

#[test]
fn live_machine_obj_fetch_stretches_mode3_and_keeps_vram_blocked_until_hblank() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_oam_entry(&mut machine, 0, 16, 8, 0, 0);
    seed_bg_tile_row(&mut machine, 0, 0, 0x00, 0xFF);
    machine.write_bus(0xFF40, 0x82);

    step_until_line_dot(&mut machine, 252);

    let drawing = machine.ppu().snapshot();
    assert_eq!(drawing.mode, PpuAccessMode::Drawing);
    assert!(drawing.mode0_start_dot > 252);
    assert_eq!(machine.read_bus(0x8000), 0xFF);

    let mode0_start_dot = drawing.mode0_start_dot;
    step_until_line_dot(&mut machine, mode0_start_dot);

    let hblank = machine.ppu().snapshot();
    assert_eq!(hblank.mode, PpuAccessMode::HBlank);
    assert_eq!(hblank.mode0_start_dot, mode0_start_dot);
    assert_eq!(&hblank.current_scanline_pixels[..8], &[2; 8]);
    assert_eq!(machine.read_bus(0x8000), 0x00);
}

#[test]
fn disabling_lcdc1_during_live_object_fetch_keeps_the_timing_cost_but_drops_pixels() {
    fn run_case(disable_obj_during_fetch: bool) -> PpuSnapshot {
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );

        seed_oam_entry(&mut machine, 0, 16, 8, 0, 0);
        seed_bg_tile_row(&mut machine, 0, 0, 0x00, 0xFF);
        machine.write_bus(0xFF40, 0x82);

        let mut waited_t_cycles = 0;
        loop {
            let fetching = machine.ppu().snapshot();
            if fetching.obj_fetcher_stage != PpuObjFetcherStage::Idle {
                assert_eq!(fetching.mode, PpuAccessMode::Drawing);
                assert!(fetching.visible_pixels_output <= 1);
                break;
            }

            machine.step_t_cycle();
            waited_t_cycles += 1;
            assert!(
                waited_t_cycles < 160,
                "left-edge OBJ fetch should begin during the first visible scanline"
            );
        }

        let fetching = machine.ppu().snapshot();
        assert_eq!(fetching.mode, PpuAccessMode::Drawing);
        assert_ne!(fetching.obj_fetcher_stage, PpuObjFetcherStage::Idle);

        if disable_obj_during_fetch {
            machine.write_bus(0xFF40, 0x80);
        }

        step_until_hblank(&mut machine);

        let hblank = machine.ppu().snapshot();
        assert_eq!(hblank.mode, PpuAccessMode::HBlank);
        hblank
    }

    let enabled = run_case(false);
    let disabled = run_case(true);

    assert_eq!(disabled.mode0_start_dot, enabled.mode0_start_dot);
    assert_ne!(enabled.current_scanline_pixels[0], 0);
    assert_eq!(&disabled.current_scanline_pixels[..8], &[0; 8]);
}

#[test]
fn window_start_keeps_the_obj_fifo_alive_for_final_mixing_on_the_live_machine() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_oam_entry(&mut machine, 0, 16, 28, 1, 0x80);
    seed_bg_tile_row(&mut machine, 0, 0, 0x00, 0x00);
    seed_bg_tile_row(&mut machine, 1, 0, 0x00, 0xFF);
    seed_bg_tile_row(&mut machine, 2, 0, 0xA0, 0x00);
    seed_window_tilemap_entry(&mut machine, 0, 0, 2);
    machine.write_bus(0xFF40, 0xF3);
    machine.write_bus(0xFF4A, 0x00);
    machine.write_bus(0xFF4B, 0x1F);

    step_until_hblank(&mut machine);

    let snapshot = machine.ppu().snapshot();
    assert_eq!(snapshot.mode, PpuAccessMode::HBlank);
    assert!(snapshot.window_started_this_line);
    assert_eq!(snapshot.bg_fetcher_source, PpuBgFetcherSource::Window);
    assert_eq!(
        &snapshot.current_scanline_pixels[20..28],
        &[2, 2, 2, 2, 1, 2, 1, 2]
    );
}
