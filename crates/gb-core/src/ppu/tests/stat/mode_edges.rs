use super::super::*;

#[test]
fn cpu_stat_read_switches_to_mode3_on_the_exact_mode2_end_dot() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x08,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 1;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;

    ppu.line_dot = MODE2_DOTS - 1;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x02
    );

    ppu.line_dot = MODE2_DOTS;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );

    ppu.line_dot = MODE2_DOTS + 1;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );
}

#[test]
fn cpu_stat_read_switches_to_hblank_on_the_exact_mode0_start_dot() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x08,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 1;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;

    ppu.line_dot = MODE0_START_DOT - 1;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x03
    );

    ppu.line_dot = MODE0_START_DOT;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );

    ppu.line_dot = MODE0_START_DOT + 1;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

fn dmg_mode0_stat_ppu(scx: u8) -> Ppu {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x08,
        scy: 0x00,
        scx,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.ly = 1;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.startup_mode_latch = None;
    ppu.stat_state.irq_line = false;
    ppu
}

fn dmg_mode2_stat_ppu() -> Ppu {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x20,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.ly = 1;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.startup_mode_latch = None;
    ppu.stat_state.irq_line = false;
    ppu
}

fn cgb_compat_mode2_stat_ppu() -> Ppu {
    let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);
    ppu.apply_operating_mode_state(OperatingMode::GbCompatible);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x20,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.ly = 1;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.startup_mode_latch = None;
    ppu.stat_state.irq_line = false;
    ppu
}

#[test]
fn ordinary_mode0_stat_pretrigger_is_hidden_from_same_cycle_cpu_if_reads() {
    let mut ppu = dmg_mode0_stat_ppu(0);

    ppu.line_dot = MODE0_START_DOT - 4;
    ppu.refresh_stat_irq_line(false);

    assert_eq!(
        ppu.pending_interrupt_request_mask(),
        InterruptSource::LcdStat.mask()
    );
    assert_eq!(ppu.cpu_visible_pending_interrupt_request_mask(), 0);
}

#[test]
fn mode0_hblank_halt_wake_defers_only_the_model_scx_pretrigger_aperture() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoyColor);
    ppu.apply_operating_mode_state(OperatingMode::Cgb);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE0_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.ly = 1;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.startup_mode_latch = None;

    let mode0_start_dot = ppu.current_mode0_start_dot();
    ppu.line_dot = mode0_start_dot - 5;
    assert!(!ppu.mode0_hblank_halt_wake_deferred());

    ppu.line_dot = mode0_start_dot - 4;
    assert!(ppu.mode0_hblank_halt_wake_deferred());

    ppu.line_dot = mode0_start_dot - 1;
    assert!(ppu.mode0_hblank_halt_wake_deferred());

    ppu.line_dot = mode0_start_dot;
    assert!(!ppu.mode0_hblank_halt_wake_deferred());

    ppu.stat_interrupt_enable = STAT_MODE2_INTERRUPT_ENABLE_BIT;
    ppu.line_dot = mode0_start_dot - 4;
    assert!(!ppu.mode0_hblank_halt_wake_deferred());

    for scx in 0..=7 {
        let mut dmg = dmg_mode0_stat_ppu(scx);
        dmg.line_dot = dmg.current_mode0_start_dot() - 4;
        assert_eq!(
            dmg.mode0_hblank_halt_wake_deferred(),
            matches!(scx, 1 | 2 | 5 | 6),
            "SCX low bits {scx}"
        );
    }
}

#[test]
fn ordinary_mode2_stat_pretrigger_is_hidden_from_same_cycle_cpu_if_reads() {
    let mut ppu = dmg_mode2_stat_ppu();

    ppu.line_dot = ppu.current_scanline_length() - 4;
    ppu.refresh_stat_irq_line(false);

    assert_eq!(
        ppu.pending_interrupt_request_mask(),
        InterruptSource::LcdStat.mask()
    );
    assert_eq!(ppu.cpu_visible_pending_interrupt_request_mask(), 0);
}

#[test]
fn ordinary_lyc_stat_edge_rises_visible_on_the_second_dot_of_a_new_line() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_LYC_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0,
        lyc: 1,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.stat_state.irq_line = false;
    ppu.line_dot = DOTS_PER_SCANLINE - 1;

    ppu.tick();

    assert_eq!(ppu.snapshot().ly, 1);
    assert_eq!(ppu.snapshot().line_dot, 0);
    assert!(!ppu.snapshot().lyc_coincidence);
    assert_eq!(ppu.pending_interrupt_request_mask(), 0);

    ppu.tick();

    assert_eq!(ppu.snapshot().line_dot, 1);
    assert!(ppu.snapshot().lyc_coincidence);
    assert_eq!(
        ppu.pending_interrupt_request_mask(),
        InterruptSource::LcdStat.mask()
    );
    assert_eq!(
        ppu.cpu_visible_pending_interrupt_request_mask(),
        InterruptSource::LcdStat.mask()
    );
}

#[test]
fn dmg_line153_lyc0_stat_pretrigger_can_be_cancelled_by_same_dot_lyc_write() {
    let mut pretrigger = PpuTestRig::dmg();
    pretrigger.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_LYC_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: TOTAL_SCANLINES - 1,
        lyc: 0,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    pretrigger.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    pretrigger.blank_frame_active = false;
    pretrigger.startup_mode_latch = None;
    pretrigger.stat_state.irq_line = false;
    pretrigger.line_dot = LINE_153_LYC0_STAT_IRQ_PRETRIGGER_DOT - 1;

    pretrigger.tick();

    assert_eq!(
        pretrigger.snapshot().line_dot,
        LINE_153_LYC0_STAT_IRQ_PRETRIGGER_DOT
    );
    assert!(!pretrigger.snapshot().lyc_coincidence);
    assert_eq!(
        pretrigger.read_register(0xFF41) & 0x04,
        0,
        "the internal pretrigger must not publish STAT.2 before the visible LYC=0 window"
    );
    assert_eq!(
        pretrigger.pending_interrupt_request_mask(),
        InterruptSource::LcdStat.mask()
    );
    assert_eq!(pretrigger.cpu_visible_pending_interrupt_request_mask(), 0);

    let mut cancelled = PpuTestRig::dmg();
    cancelled.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_LYC_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: TOTAL_SCANLINES - 1,
        lyc: 0,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    cancelled.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    cancelled.blank_frame_active = false;
    cancelled.startup_mode_latch = None;
    cancelled.stat_state.irq_line = false;
    cancelled.line_dot = LINE_153_LYC0_STAT_IRQ_PRETRIGGER_DOT - 1;

    cancelled.tick();
    cancelled.write_register(0xFF45, 0xFF);

    assert_eq!(cancelled.pending_interrupt_request_mask(), 0);
    assert!(!cancelled.snapshot().stat_irq_line);
}

#[test]
fn dmg_line153_lyc0_stat_pretrigger_bridges_to_visible_coincidence_without_retrigger() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_LYC_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: TOTAL_SCANLINES - 1,
        lyc: 0,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.startup_mode_latch = None;
    ppu.stat_state.irq_line = false;
    ppu.line_dot = LINE_153_LYC0_STAT_IRQ_PRETRIGGER_DOT - 1;

    ppu.tick();

    assert_eq!(
        ppu.snapshot().line_dot,
        LINE_153_LYC0_STAT_IRQ_PRETRIGGER_DOT
    );
    assert!(ppu.snapshot().stat_irq_line);
    assert!(!ppu.snapshot().lyc_coincidence);
    assert_eq!(
        drain_ppu_interrupts(&mut ppu.ppu),
        vec![InterruptSource::LcdStat]
    );

    ppu.tick_n(u64::from(
        LINE_153_LYC0_COMPARE_START_DOT - LINE_153_LYC0_STAT_IRQ_PRETRIGGER_DOT,
    ));

    assert_eq!(ppu.snapshot().line_dot, LINE_153_LYC0_COMPARE_START_DOT);
    assert!(ppu.snapshot().stat_irq_line);
    assert!(ppu.snapshot().lyc_coincidence);
    assert!(
        drain_ppu_interrupts(&mut ppu.ppu).is_empty(),
        "the visible LYC=0 seam must continue the pretriggered STAT line instead of creating a second IRQ edge"
    );
}

#[test]
fn cgb_line153_lyc_edges_follow_the_cgb_compare_schedule() {
    let mut lyc153 = PpuTestRig::with_model(ConsoleModel::GameBoyColor);
    lyc153.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_LYC_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: TOTAL_SCANLINES - 2,
        lyc: TOTAL_SCANLINES - 1,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    lyc153.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    lyc153.blank_frame_active = false;
    lyc153.stat_state.irq_line = false;
    lyc153.line_dot = DOTS_PER_SCANLINE - 1;

    lyc153.tick();

    assert_eq!(lyc153.snapshot().ly, TOTAL_SCANLINES - 1);
    assert_eq!(lyc153.snapshot().line_dot, 0);
    assert!(!lyc153.snapshot().lyc_coincidence);
    assert_eq!(
        lyc153.pending_interrupt_request_mask() & InterruptSource::LcdStat.mask(),
        0
    );

    lyc153.tick();

    assert_eq!(
        lyc153.snapshot().line_dot,
        CGB_LINE_153_LYC153_COMPARE_START_DOT
    );
    assert!(lyc153.snapshot().lyc_coincidence);
    assert_ne!(
        lyc153.pending_interrupt_request_mask() & InterruptSource::LcdStat.mask(),
        0
    );
    assert_ne!(
        lyc153.cpu_visible_pending_interrupt_request_mask() & InterruptSource::LcdStat.mask(),
        0
    );

    let mut lyc0 = PpuTestRig::with_model(ConsoleModel::GameBoyColor);
    lyc0.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_LYC_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: TOTAL_SCANLINES - 1,
        lyc: 0,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    lyc0.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    lyc0.blank_frame_active = false;
    lyc0.startup_mode_latch = None;
    lyc0.stat_state.irq_line = false;
    lyc0.line_dot = CGB_LINE_153_LY_READ_ZERO_DOT - 1;

    lyc0.tick();

    assert_eq!(lyc0.snapshot().line_dot, CGB_LINE_153_LY_READ_ZERO_DOT);
    assert_eq!(lyc0.read_register(0xFF44), 0);
    assert!(!lyc0.snapshot().lyc_coincidence);
    assert_eq!(lyc0.pending_interrupt_request_mask(), 0);

    lyc0.tick();

    assert_eq!(
        lyc0.snapshot().line_dot,
        CGB_LINE_153_LYC0_COMPARE_START_DOT
    );
    assert!(lyc0.snapshot().lyc_coincidence);
    assert_eq!(
        lyc0.pending_interrupt_request_mask(),
        InterruptSource::LcdStat.mask()
    );
    assert_eq!(
        lyc0.cpu_visible_pending_interrupt_request_mask(),
        InterruptSource::LcdStat.mask()
    );
}

#[test]
fn ordinary_mode1_stat_edge_is_hidden_from_same_cycle_cpu_if_reads() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_MODE1_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: VISIBLE_SCANLINES - 1,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.stat_state.irq_line = false;
    ppu.line_dot = DOTS_PER_SCANLINE - 1;

    ppu.tick();

    assert_eq!(ppu.snapshot().ly, VISIBLE_SCANLINES);
    assert_eq!(ppu.snapshot().line_dot, 0);
    assert_eq!(
        ppu.pending_interrupt_request_mask(),
        InterruptSource::VBlank.mask() | InterruptSource::LcdStat.mask()
    );
    assert_eq!(ppu.cpu_visible_pending_interrupt_request_mask(), 0);
}

#[test]
fn mode2_stat_pretrigger_defers_halt_wake_only_during_reenable_blank_frame() {
    let mut steady_state = dmg_mode2_stat_ppu();
    steady_state.line_dot = steady_state.current_scanline_length() - 4;
    assert!(!steady_state.dmg_mode2_oam_halt_wake_deferred());

    let mut first_blank_frame = dmg_mode2_stat_ppu();
    first_blank_frame.blank_frame_active = true;
    first_blank_frame.line_dot = first_blank_frame.current_scanline_length() - 4;
    assert!(first_blank_frame.dmg_mode2_oam_halt_wake_deferred());
}

#[test]
fn dmg_line144_mode2_stat_source_is_hidden_and_does_not_lock_oam() {
    let mut ppu = dmg_mode2_stat_ppu();
    ppu.ly = VISIBLE_SCANLINES - 1;
    ppu.line_dot = ppu.current_scanline_length() - 4;
    ppu.refresh_stat_irq_line(false);

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::HBlank);
    assert_eq!(ppu.current_bus_access_mode(), PpuAccessMode::HBlank);
    assert_eq!(
        ppu.pending_interrupt_request_mask(),
        InterruptSource::LcdStat.mask()
    );
    assert_eq!(ppu.cpu_visible_pending_interrupt_request_mask(), 0);
}

#[test]
fn cgb_compat_line144_mode2_stat_source_is_hidden_without_dmg_service_deferral() {
    let mut ppu = cgb_compat_mode2_stat_ppu();
    ppu.ly = VISIBLE_SCANLINES - 1;
    ppu.line_dot = ppu.current_scanline_length() - 4;
    ppu.refresh_stat_irq_line(false);

    assert_eq!(ppu.current_access_mode(), PpuAccessMode::HBlank);
    assert_eq!(ppu.current_bus_access_mode(), PpuAccessMode::HBlank);
    assert_eq!(
        ppu.pending_interrupt_request_mask(),
        InterruptSource::LcdStat.mask()
    );
    assert_eq!(ppu.cpu_visible_pending_interrupt_request_mask(), 0);
    assert!(!ppu.dmg_mode2_vblank_entry_interrupt_service_deferred());
}

#[test]
fn dmg_line144_mode2_stat_service_defers_only_until_vblank_entry() {
    let mut ppu = dmg_mode2_stat_ppu();
    ppu.ly = VISIBLE_SCANLINES - 1;
    ppu.line_dot = ppu.current_scanline_length() - 4;

    assert!(ppu.dmg_mode2_vblank_entry_interrupt_service_deferred());

    ppu.line_dot = ppu.current_scanline_length() - 1;
    assert!(ppu.dmg_mode2_vblank_entry_interrupt_service_deferred());

    ppu.ly = VISIBLE_SCANLINES;
    ppu.line_dot = 0;

    assert!(!ppu.dmg_mode2_vblank_entry_interrupt_service_deferred());
}

#[test]
fn mode2_stat_write_requests_only_on_the_oam_start_dot() {
    let mut oam_start = dmg_mode2_stat_ppu();
    oam_start.stat_interrupt_enable = 0;
    oam_start.stat_state.irq_line = false;
    oam_start.ly = 2;
    oam_start.line_dot = 0;
    oam_start.write_stat(STAT_MODE2_INTERRUPT_ENABLE_BIT);
    assert_eq!(
        oam_start.pending_interrupt_request_mask(),
        InterruptSource::LcdStat.mask()
    );

    let mut already_in_oam = dmg_mode2_stat_ppu();
    already_in_oam.stat_interrupt_enable = 0;
    already_in_oam.stat_state.irq_line = false;
    already_in_oam.ly = 2;
    already_in_oam.line_dot = 4;
    already_in_oam.write_stat(STAT_MODE2_INTERRUPT_ENABLE_BIT);
    assert_eq!(already_in_oam.pending_interrupt_request_mask(), 0);
    assert!(already_in_oam.stat_state.irq_line);
}

#[test]
fn real_boot_handoff_scx_seam_suppresses_only_scx3_and_scx7_mode0_pretrigger() {
    let mut ordinary = dmg_mode0_stat_ppu(0);
    ordinary.apply_dmg_real_boot_handoff_stat_irq_phase();
    ordinary.line_dot = MODE0_START_DOT - 4;
    ordinary.refresh_stat_irq_line(false);
    assert_eq!(
        ordinary.pending_interrupt_request_mask(),
        InterruptSource::LcdStat.mask()
    );

    for scx in [3, 7] {
        let mut seam = dmg_mode0_stat_ppu(scx);
        seam.apply_dmg_real_boot_handoff_stat_irq_phase();
        seam.line_dot = seam.current_mode0_start_dot() - 4;
        seam.refresh_stat_irq_line(false);
        assert_eq!(seam.pending_interrupt_request_mask(), 0);

        seam.line_dot = seam.current_mode0_start_dot();
        seam.refresh_stat_irq_line(false);
        assert_eq!(
            seam.pending_interrupt_request_mask(),
            InterruptSource::LcdStat.mask()
        );
    }
}

#[test]
fn dmg_real_boot_power_on_first_lcd_enable_starts_from_the_observed_dot_phase() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_dmg_real_boot_power_on_lcd_enable_phase();

    ppu.enter_lcd_enabled_restart_state();

    assert_eq!(
        ppu.line_dot,
        DMG_REAL_BOOT_POWER_ON_LCD_ENABLE_INITIAL_LINE_DOT
    );
    assert!(!ppu.dmg_real_boot_power_on_lcd_enable_phase_active);
}

#[test]
fn dmg0_direct_boot_handoff_phase_clears_when_vblank_starts() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x83,
        scy: 0x00,
        scx: 0x00,
        ly: 0x01,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.apply_dmg0_direct_boot_handoff_stat_phase();
    assert!(ppu.stat_state.boot_power_on_ppu_phase_active);
    assert!(ppu.stat_state.boot_power_on_ppu_phase_extends_until_vblank);

    ppu.ly = VISIBLE_SCANLINES - 1;
    ppu.line_dot = DOTS_PER_SCANLINE - 1;
    ppu.tick();

    assert_eq!(ppu.ly, VISIBLE_SCANLINES);
    assert_eq!(ppu.line_dot, 0);
    assert!(!ppu.stat_state.boot_power_on_ppu_phase_active);
    assert_eq!(ppu.stat_state.boot_power_on_ppu_phase_base_dot, 0);
    assert!(!ppu.stat_state.boot_power_on_ppu_phase_extends_until_vblank);
}

#[test]
#[ignore = "diagnostic direct-read experiment for offscreen-right mode0 publication"]
fn cpu_stat_read_switches_to_hblank_one_dot_before_mode0_start_for_offscreen_right_sprites() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x85,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 68;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.bg_pipeline_state.mode3_started = true;
    ppu.bg_pipeline_state.mode0_start_dot = MODE0_START_DOT;
    for oam_index in 0..10 {
        ppu.mode2_scan_state.push(PpuSelectedSprite {
            oam_index,
            y: 16,
            x: 168,
            tile_index: 0,
            attributes: 0,
        });
    }

    ppu.line_dot = MODE0_START_DOT - 1;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x03,
        0x00
    );
}

#[test]
fn cpu_stat_read_drops_lyc_coincidence_on_the_first_dot_of_a_new_line() {
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x85,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.ly = 1;
    ppu.lyc = 1;
    ppu.line_dot = 0;
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;

    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x07,
        0x00
    );

    ppu.line_dot = 4;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x07,
        0x06
    );
}
