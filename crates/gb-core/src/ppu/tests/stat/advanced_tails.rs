use super::super::*;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
struct TerminalTailConfig {
    stat: u8,
    ly: u8,
    blank_frame_active: bool,
    mode0_start_dot: u16,
    current_transfer_x: u8,
    visible_pixels_output: u8,
    startup_fifo_placeholders: u8,
    fifo_len: usize,
    fetcher_stage: PpuBgFetcherStage,
    fetcher_stage_dot: u8,
    line_dot: u16,
    selected_sprite_count: u8,
    selected_sprite_x: u8,
}

impl Default for TerminalTailConfig {
    fn default() -> Self {
        Self {
            stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
            ly: 68,
            blank_frame_active: false,
            mode0_start_dot: MODE0_START_DOT + 60,
            current_transfer_x: 163,
            visible_pixels_output: 155,
            startup_fifo_placeholders: 4,
            fifo_len: 5,
            fetcher_stage: PpuBgFetcherStage::Push,
            fetcher_stage_dot: 0,
            line_dot: MODE0_START_DOT + 60,
            selected_sprite_count: MAX_SELECTED_SPRITES_PER_LINE as u8,
            selected_sprite_x: 2,
        }
    }
}

fn terminal_tail_rig(config: TerminalTailConfig) -> PpuTestRig {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: config.stat,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = config.ly;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = config.blank_frame_active;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = config.mode0_start_dot;
    ppu.bg_pipeline_state.startup_source_state = Mode3StartupSourceState::FifoBacked;
    ppu.bg_pipeline_state.current_transfer_x = config.current_transfer_x;
    ppu.bg_pipeline_state.visible_pixels_output = config.visible_pixels_output;
    ppu.bg_pipeline_state.transfer_phase = Mode3TransferPhase::Output;
    ppu.bg_pipeline_state.startup_fifo_placeholders = config.startup_fifo_placeholders;
    ppu.bg_pipeline_state
        .fifo
        .extend(std::iter::repeat_n(0, config.fifo_len));
    ppu.bg_pipeline_state.fetcher.stage = config.fetcher_stage;
    ppu.bg_pipeline_state.fetcher.stage_dot = config.fetcher_stage_dot;
    ppu.line_dot = config.line_dot;

    for oam_index in 0..config.selected_sprite_count {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: config.selected_sprite_x,
            tile_index: oam_index,
            attributes: 0,
        });
    }

    ppu
}

fn cpu_visible_stat_mode(ppu: &Ppu) -> u8 {
    ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03
}

#[test]
#[ignore = "diagnostic terminal x162 placeholder-backed tail with blank_frame_active on saturated sprite lines"]
fn cpu_stat_read_logs_terminal_x162_placeholder_backed_tail_with_blank_frame_active_on_saturated_sprite_lines()
 {
    let ppu = terminal_tail_rig(TerminalTailConfig {
        blank_frame_active: true,
        current_transfer_x: 162,
        visible_pixels_output: 154,
        fifo_len: 6,
        ..TerminalTailConfig::default()
    });

    println!(
        "blank_frame_active_case read={:#04X} mode0_start_dot={} current_transfer_x={} fifo_len={} placeholders={}",
        cpu_visible_stat_mode(&ppu),
        ppu.current_mode0_start_dot(),
        ppu.bg_pipeline_state.current_transfer_x,
        ppu.bg_pipeline_state.fifo.len(),
        ppu.bg_pipeline_state.startup_fifo_placeholders
    );
}

#[test]
fn cpu_stat_read_keeps_mode3_for_terminal_x161_placeholder_backed_tail_on_saturated_sprite_lines() {
    let ppu = terminal_tail_rig(TerminalTailConfig {
        current_transfer_x: 161,
        visible_pixels_output: 153,
        fifo_len: 7,
        fetcher_stage: PpuBgFetcherStage::TileDataHigh,
        fetcher_stage_dot: 1,
        ..TerminalTailConfig::default()
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 61,
        "internal raster still stretches one more dot from the live transfer"
    );
    assert_eq!(cpu_visible_stat_mode(&ppu), 0x03);
}

#[test]
fn cpu_stat_read_keeps_mode3_for_terminal_x165_placeholder_backed_tail_on_saturated_sprite_lines() {
    let ppu = terminal_tail_rig(TerminalTailConfig {
        ly: 70,
        mode0_start_dot: MODE0_START_DOT + 63,
        current_transfer_x: 165,
        visible_pixels_output: 157,
        fifo_len: 11,
        fetcher_stage: PpuBgFetcherStage::TileIndex,
        fetcher_stage_dot: 1,
        line_dot: MODE0_START_DOT + 64,
        selected_sprite_x: 0,
        ..TerminalTailConfig::default()
    });

    assert_eq!(ppu.current_mode0_start_dot(), MODE0_START_DOT + 65);
    assert_eq!(cpu_visible_stat_mode(&ppu), 0x03);
}

#[test]
fn cpu_stat_read_keeps_mode3_for_terminal_x159_ready_tail_on_shorter_saturated_sprite_lines() {
    let ppu = terminal_tail_rig(TerminalTailConfig {
        stat: STAT_MODE2_INTERRUPT_ENABLE_BIT,
        current_transfer_x: 159,
        visible_pixels_output: 151,
        startup_fifo_placeholders: 0,
        fifo_len: 1,
        selected_sprite_x: 17,
        ..TerminalTailConfig::default()
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 61,
        "live transfer still stretches one more dot from the shorter ready tail"
    );
    assert_eq!(cpu_visible_stat_mode(&ppu), 0x03);
}

#[test]
fn cpu_stat_read_keeps_mode3_for_terminal_x151_ready_tail_on_unsaturated_sprite_lines() {
    let ppu = terminal_tail_rig(TerminalTailConfig {
        current_transfer_x: 151,
        visible_pixels_output: 143,
        startup_fifo_placeholders: 0,
        fifo_len: 1,
        selected_sprite_count: 5,
        selected_sprite_x: 24,
        ..TerminalTailConfig::default()
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 61,
        "live transfer still stretches one more dot from the ready tail"
    );
    assert!(matches!(
        ppu.current_transfer().map(|transfer| transfer.readiness),
        Some(Mode3TransferReadiness::Ready(_))
    ));
    assert_eq!(cpu_visible_stat_mode(&ppu), 0x03);
}

#[test]
fn cpu_stat_read_keeps_mode3_for_terminal_x158_ready_tail_on_saturated_sprite_lines() {
    let ppu = terminal_tail_rig(TerminalTailConfig {
        current_transfer_x: 158,
        visible_pixels_output: 150,
        startup_fifo_placeholders: 0,
        fifo_len: 1,
        selected_sprite_x: 17,
        ..TerminalTailConfig::default()
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 61,
        "live transfer still stretches one more dot from the ready tail"
    );
    assert!(matches!(
        ppu.current_transfer().map(|transfer| transfer.readiness),
        Some(Mode3TransferReadiness::Ready(_))
    ));
    assert_eq!(cpu_visible_stat_mode(&ppu), 0x03);
}

#[test]
fn cpu_stat_read_keeps_mode3_for_terminal_waiting_for_fifo_tail_on_unsaturated_sprite_lines() {
    let ppu = terminal_tail_rig(TerminalTailConfig {
        current_transfer_x: 152,
        visible_pixels_output: 144,
        startup_fifo_placeholders: 0,
        fifo_len: 0,
        fetcher_stage: PpuBgFetcherStage::TileIndex,
        selected_sprite_count: 5,
        selected_sprite_x: 19,
        ..TerminalTailConfig::default()
    });

    assert_eq!(
        ppu.current_mode0_start_dot(),
        MODE0_START_DOT + 61,
        "live transfer still stretches one more dot while the FIFO is refilling"
    );
    assert!(matches!(
        ppu.current_transfer().map(|transfer| transfer.readiness),
        Some(Mode3TransferReadiness::WaitingForFifo(_))
    ));
    assert_eq!(cpu_visible_stat_mode(&ppu), 0x03);
}
