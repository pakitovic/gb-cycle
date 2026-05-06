use super::*;
pub(super) use crate::bus::BusMaster;
pub(super) use crate::scheduler::TCycle;
pub(super) use crate::{
    ConsoleModel, Machine, MachineConfig, OperatingMode, StartupMode, TraceSummaryBuffer,
};

mod fixtures;
mod lcd;
mod mode2;
mod mode3;
mod obj;
mod oracle;
mod palette;
mod save_state;
mod stat;
mod window;

use self::fixtures::*;

#[derive(Default)]
struct PpuRegionTrace {
    events: Vec<(bool, PpuStepRegion)>,
}

impl PpuStepObserver for PpuRegionTrace {
    fn begin_ppu_region(&mut self, region: PpuStepRegion) {
        self.events.push((true, region));
    }

    fn end_ppu_region(&mut self, region: PpuStepRegion) {
        self.events.push((false, region));
    }
}

#[test]
fn ppu_region_observer_wraps_work_only_when_regions_are_recorded() {
    let mut observer = PpuRegionTrace::default();
    assert!(observer.records_ppu_regions());

    let result = observe_ppu_step_region(&mut observer, PpuStepRegion::Mode3PixelTransfer, || 7);

    assert_eq!(result, 7);
    assert_eq!(
        observer.events,
        vec![
            (true, PpuStepRegion::Mode3PixelTransfer),
            (false, PpuStepRegion::Mode3PixelTransfer),
        ]
    );

    let mut observer = NoopPpuStepObserver;
    assert!(!observer.records_ppu_regions());
    PpuStepObserver::begin_ppu_region(&mut observer, PpuStepRegion::Other);
    PpuStepObserver::end_ppu_region(&mut observer, PpuStepRegion::Other);

    let mut ran = false;
    observe_ppu_step_region(&mut observer, PpuStepRegion::Mode3PixelTransfer, || {
        ran = true;
    });

    assert!(ran);
}

#[test]
fn ppu_operating_mode_normalization_keeps_legacy_save_states_safe() {
    assert_eq!(
        normalize_ppu_operating_mode(ConsoleModel::GameBoy, OperatingMode::Cgb),
        OperatingMode::Dmg
    );
    assert_eq!(
        normalize_ppu_operating_mode(ConsoleModel::GameBoyColor, OperatingMode::GbCompatible),
        OperatingMode::GbCompatible
    );
    assert_eq!(
        normalize_saved_ppu_operating_mode(
            ConsoleModel::GameBoyColor,
            OperatingMode::Dmg,
            CgbObjPriorityMode::DmgXCoordinate,
        ),
        OperatingMode::GbCompatible
    );
    assert_eq!(
        normalize_saved_ppu_operating_mode(
            ConsoleModel::GameBoyColor,
            OperatingMode::Dmg,
            CgbObjPriorityMode::CgbOamOrder,
        ),
        OperatingMode::Cgb
    );
}

#[test]
fn default_rgb555_framebuffer_matches_cgb_white_backdrop_contract() {
    let framebuffer = default_framebuffer_rgb555();

    assert_eq!(framebuffer.len(), FRAMEBUFFER_PIXELS);
    assert!(framebuffer.iter().all(|&pixel| pixel == RGB555_WHITE));
}

#[test]
fn blank_frame_cpu_visible_mode_uses_the_previous_lcd_restart_dot() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.lcdc = LCDC_ENABLE_BIT;
    ppu.lcd_state = PpuLcdState::Enabled;
    ppu.blank_frame_active = true;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::first_line_after_enable();
    ppu.ly = 0;
    ppu.line_dot = LCD_REENABLE_LINE0_MODE3_START_DOT + 1;

    assert_eq!(
        ppu.current_cpu_visible_access_mode(),
        PpuAccessMode::Drawing
    );
}

#[test]
fn mode0_start_dot_fast_path_tracks_live_mode3_mutations() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);
    ppu.lcdc = LCDC_ENABLE_BIT | LCDC_BG_ENABLE_BIT | LCDC_OBJ_ENABLE_BIT;
    ppu.lcd_state = PpuLcdState::Enabled;
    ppu.reload_mode3_register_latches_from_mmio();
    ppu.ly = 12;
    ppu.line_dot = MODE0_START_DOT - 4;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 8;
    let sprite_y = ppu.ly + 16;
    ppu.mode2_scan_state.push(PpuSelectedSprite {
        oam_index: 0,
        y: sprite_y,
        x: 32,
        tile_index: 0,
        attributes: 0,
    });

    assert_eq!(ppu.current_mode0_start_dot(), MODE0_START_DOT + 8);
    assert_eq!(ppu.current_mode0_start_dot(), MODE0_START_DOT + 8);

    ppu.bg_pipeline_state.mode0_start_dot += 1;
    assert_eq!(ppu.current_mode0_start_dot(), MODE0_START_DOT + 9);

    ppu.mode2_scan_state.selected_sprites[0] = Some(PpuSelectedSprite {
        oam_index: 0,
        y: sprite_y,
        x: 168,
        tile_index: 0,
        attributes: 0,
    });
    ppu.bg_pipeline_state.mode0_start_dot = ppu.baseline_mode0_start_dot();
    assert_eq!(ppu.current_mode0_start_dot(), MODE0_START_DOT - 1);
}

#[test]
fn access_mode_fast_path_and_scanline_length_cache_track_raster_keys() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);
    ppu.lcdc = LCDC_ENABLE_BIT | LCDC_BG_ENABLE_BIT;
    ppu.lcd_state = PpuLcdState::Enabled;
    ppu.reload_mode3_register_latches_from_mmio();
    ppu.ly = 0;
    ppu.line_dot = MODE2_DOTS + 4;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 3;

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);
    assert_eq!(ppu.current_access_mode(), PpuAccessMode::Drawing);
    assert_eq!(ppu.current_scanline_length(), DOTS_PER_SCANLINE);

    ppu.bg_pipeline_state.mode3_started = false;
    ppu.line_dot = ppu.current_mode0_start_dot();
    assert_eq!(ppu.current_access_mode(), PpuAccessMode::HBlank);

    ppu.lcd_restart_phase = PpuLcdRestartPhase::first_line_after_enable();
    ppu.ly = 0;
    assert_eq!(ppu.current_scanline_length(), LCD_REENABLE_LINE0_TOTAL_DOTS);
}

fn stable_bus_snapshot_fixture() -> Ppu {
    let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);
    ppu.lcdc = LCDC_ENABLE_BIT | LCDC_BG_ENABLE_BIT;
    ppu.lcd_state = PpuLcdState::Enabled;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.startup_mode_latch = None;
    ppu.reload_mode3_register_latches_from_mmio();
    ppu.ly = 42;
    ppu.line_dot = MODE2_DOTS + 4;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT + 8;
    ppu
}

fn direct_bus_state_snapshot(ppu: &Ppu) -> PpuBusStateSnapshot {
    PpuBusStateSnapshot {
        owner: ppu.owner_bus_state(),
        cpu_read: ppu.cpu_bus_state(),
        cpu_write: ppu.cpu_write_bus_state(),
    }
}

#[test]
fn stable_bus_state_snapshot_matches_direct_helpers_on_stable_dots() {
    let mut ppu = stable_bus_snapshot_fixture();

    ppu.line_dot = MODE2_DOTS + 4;
    assert_eq!(
        ppu.stable_bus_state_snapshot(),
        Some(direct_bus_state_snapshot(&ppu))
    );

    ppu.line_dot = ppu.current_mode0_start_dot() + 8;
    assert_eq!(
        ppu.stable_bus_state_snapshot(),
        Some(direct_bus_state_snapshot(&ppu))
    );
}

#[test]
fn stable_bus_state_snapshot_falls_back_on_publication_seams() {
    let mut ppu = stable_bus_snapshot_fixture();

    for line_dot in [
        0,
        MODE2_DOTS,
        ppu.current_scanline_length() - 4,
        ppu.current_scanline_length() - 1,
    ] {
        ppu.ly = 42;
        ppu.line_dot = line_dot;
        assert_eq!(ppu.stable_bus_state_snapshot(), None, "line_dot={line_dot}");
        assert_eq!(ppu.bus_state_snapshot(), direct_bus_state_snapshot(&ppu));
    }

    ppu = stable_bus_snapshot_fixture();
    ppu.bg_pipeline_state.mode3_started = false;
    ppu.line_dot = ppu.current_mode0_start_dot();
    assert_eq!(ppu.stable_bus_state_snapshot(), None);
    assert_eq!(ppu.bus_state_snapshot(), direct_bus_state_snapshot(&ppu));

    ppu.line_dot = MODE2_DOTS + 4;
    ppu.ly = VISIBLE_SCANLINES;
    assert_eq!(ppu.stable_bus_state_snapshot(), None);
    assert_eq!(ppu.bus_state_snapshot(), direct_bus_state_snapshot(&ppu));

    ppu = stable_bus_snapshot_fixture();
    ppu.lcd_restart_phase = PpuLcdRestartPhase::first_line_after_enable();
    assert_eq!(ppu.stable_bus_state_snapshot(), None);
    assert_eq!(ppu.bus_state_snapshot(), direct_bus_state_snapshot(&ppu));

    ppu = stable_bus_snapshot_fixture();
    ppu.blank_frame_active = true;
    assert_eq!(ppu.stable_bus_state_snapshot(), None);
    assert_eq!(ppu.bus_state_snapshot(), direct_bus_state_snapshot(&ppu));

    ppu = stable_bus_snapshot_fixture();
    ppu.startup_mode_latch = Some(PpuAccessMode::Drawing);
    assert_eq!(ppu.stable_bus_state_snapshot(), None);
    assert_eq!(ppu.bus_state_snapshot(), direct_bus_state_snapshot(&ppu));
}
