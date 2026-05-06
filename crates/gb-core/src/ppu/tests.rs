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
