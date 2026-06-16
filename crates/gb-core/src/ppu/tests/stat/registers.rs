use super::super::*;

fn dmg_skip_boot_power_on_rig() -> PpuTestRig {
    let mut ppu = PpuTestRig::dmg();
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
    ppu.apply_dmg_skip_boot_stat_irq_startup_phase();
    ppu
}

fn set_dmg_skip_boot_power_on_delay(ppu: &mut PpuTestRig, delay_mcycles: u16) {
    let absolute_dot = 36 + u32::from(delay_mcycles) * 4;
    ppu.ly = (absolute_dot / u32::from(DOTS_PER_SCANLINE)) as u8;
    ppu.line_dot = (absolute_dot % u32::from(DOTS_PER_SCANLINE)) as u16;
}

fn dmg_real_boot_handoff_power_on_rig() -> PpuTestRig {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x85,
        scy: 0x00,
        scx: 0x00,
        ly: 153,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = 392;
    ppu.apply_dmg_real_boot_handoff_stat_irq_phase();
    ppu
}

fn set_dmg_real_boot_handoff_power_on_delay(ppu: &mut PpuTestRig, delay_mcycles: u16) {
    let frame_dots = u32::from(DOTS_PER_SCANLINE) * u32::from(TOTAL_SCANLINES);
    let base_dot = (153 * u32::from(DOTS_PER_SCANLINE) + 428) % frame_dots;
    let absolute_dot = (base_dot + u32::from(delay_mcycles) * 4) % frame_dots;
    ppu.ly = (absolute_dot / u32::from(DOTS_PER_SCANLINE)) as u8;
    ppu.line_dot = (absolute_dot % u32::from(DOTS_PER_SCANLINE)) as u16;
}

#[test]
fn stat_keeps_live_mode_and_coincidence_bits_outside_the_writable_mask() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x81,
        scy: 0x00,
        scx: 0x00,
        ly: 0x12,
        lyc: 0x12,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = 4;

    ppu.write_register(0xFF41, 0xFF);

    assert_eq!(ppu.read_register(0xFF41), 0xFD);
}

#[test]
fn lyc_writes_reevaluate_coincidence_immediately_and_can_raise_lcd_stat() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x42,
        scy: 0x00,
        scx: 0x00,
        ly: 0x12,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = 4;

    assert!(!ppu.snapshot().lyc_coincidence);
    assert!(!ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());

    ppu.write_register(0xFF45, 0x12);

    assert_eq!(ppu.read_register(0xFF41), 0xC6);
    assert!(ppu.snapshot().lyc_coincidence);
    assert!(ppu.snapshot().stat_irq_line);
    assert_eq!(
        drain_ppu_interrupts(&mut ppu),
        vec![InterruptSource::LcdStat]
    );
}

#[test]
fn stat_line_blocks_new_requests_while_an_enabled_source_keeps_it_high() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x62,
        scy: 0x00,
        scx: 0x00,
        ly: 0x21,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = 4;

    assert!(ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());

    ppu.write_register(0xFF45, 0x21);

    assert!(ppu.snapshot().lyc_coincidence);
    assert!(ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());
}

#[test]
fn dmg_mode2_enable_requests_lcd_stat_at_line144_pretrigger_only() {
    let mut ppu = PpuTestRig::dmg();

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: STAT_MODE2_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 143,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = DOTS_PER_SCANLINE - 5;
    ppu.refresh_stat_irq_line(false);
    assert!(!ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());

    ppu.tick();

    assert_eq!(ppu.snapshot().ly, 143);
    assert_eq!(ppu.snapshot().line_dot, DOTS_PER_SCANLINE - 4);
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::HBlank);
    assert!(ppu.snapshot().stat_irq_line);
    assert_eq!(
        drain_ppu_interrupts(&mut ppu),
        vec![InterruptSource::LcdStat]
    );

    ppu.tick();

    assert_eq!(ppu.snapshot().ly, 143);
    assert_eq!(ppu.snapshot().line_dot, DOTS_PER_SCANLINE - 3);
    assert!(!ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());

    for _ in 0..3 {
        ppu.tick();
    }

    assert_eq!(ppu.snapshot().ly, 144);
    assert_eq!(ppu.snapshot().line_dot, 0);
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::VBlank);
    assert!(!ppu.snapshot().stat_irq_line);
    assert_eq!(
        drain_ppu_interrupts(&mut ppu),
        vec![InterruptSource::VBlank]
    );
}

#[test]
fn cgb_compat_mode2_enable_requests_lcd_stat_at_line144_pretrigger_only() {
    let mut ppu = PpuTestRig::with_model(ConsoleModel::GameBoyColor);
    ppu.apply_operating_mode_state(OperatingMode::GbCompatible);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: STAT_MODE2_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 143,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = DOTS_PER_SCANLINE - 5;
    ppu.refresh_stat_irq_line(false);
    assert!(!ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());

    ppu.tick();

    assert_eq!(ppu.snapshot().ly, 143);
    assert_eq!(ppu.snapshot().line_dot, DOTS_PER_SCANLINE - 4);
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::HBlank);
    assert!(ppu.snapshot().stat_irq_line);
    assert_eq!(
        drain_ppu_interrupts(&mut ppu),
        vec![InterruptSource::LcdStat]
    );

    ppu.tick();

    assert_eq!(ppu.snapshot().ly, 143);
    assert_eq!(ppu.snapshot().line_dot, DOTS_PER_SCANLINE - 3);
    assert!(!ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());

    for _ in 0..3 {
        ppu.tick();
    }

    assert_eq!(ppu.snapshot().ly, 144);
    assert_eq!(ppu.snapshot().line_dot, 0);
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::VBlank);
    assert!(!ppu.snapshot().stat_irq_line);
    assert_eq!(
        drain_ppu_interrupts(&mut ppu),
        vec![InterruptSource::VBlank]
    );
}

#[test]
fn mode2_enable_alone_does_not_hold_stat_high_past_vblank_entry() {
    let mut ppu = PpuTestRig::dmg();

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: STAT_MODE2_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 144,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.line_dot = 8;
    ppu.refresh_stat_irq_line(false);
    assert!(!ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());
}

#[test]
fn stat_write_quirk_requests_in_mode1_mode2_and_coincidence_but_not_plain_mode3() {
    let mut mode2 = PpuTestRig::dmg();

    mode2.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x20,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    assert!(drain_ppu_interrupts(&mut mode2).is_empty());

    mode2.write_register(0xFF41, 0x00);

    assert!(mode2.snapshot().stat_irq_line);
    assert_eq!(
        drain_ppu_interrupts(&mut mode2),
        vec![InterruptSource::LcdStat]
    );

    let mut mode1 = PpuTestRig::dmg();
    mode1.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x01,
        scy: 0x00,
        scx: 0x00,
        ly: 144,
        lyc: 0x20,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    mode1.line_dot = 8;
    assert_eq!(mode1.snapshot().mode, PpuAccessMode::VBlank);
    assert!(drain_ppu_interrupts(&mut mode1).is_empty());

    mode1.write_register(0xFF41, 0x00);

    assert!(mode1.snapshot().stat_irq_line);
    assert_eq!(
        drain_ppu_interrupts(&mut mode1),
        vec![InterruptSource::LcdStat]
    );

    let mut mode3 = PpuTestRig::dmg();
    mode3.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x20,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    mode3.tick_n(80);
    assert_eq!(mode3.snapshot().mode, PpuAccessMode::Drawing);
    assert!(drain_ppu_interrupts(&mut mode3).is_empty());

    mode3.write_register(0xFF41, 0x00);

    assert!(!mode3.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut mode3).is_empty());

    let mut coincidence = PpuTestRig::dmg();
    coincidence.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 0x00,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    coincidence.tick_n(80);
    assert_eq!(coincidence.snapshot().mode, PpuAccessMode::Drawing);
    assert!(drain_ppu_interrupts(&mut coincidence).is_empty());

    coincidence.write_register(0xFF41, 0x00);

    assert!(coincidence.snapshot().stat_irq_line);
    assert_eq!(
        drain_ppu_interrupts(&mut coincidence),
        vec![InterruptSource::LcdStat]
    );
}

#[test]
fn dmg_stat_write_quirk_requests_for_nonzero_enable_writes_in_vblank() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x00,
        scy: 0x00,
        scx: 0x00,
        ly: 144,
        lyc: 0x20,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.startup_mode_latch = None;
    ppu.line_dot = 8;
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::VBlank);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());

    ppu.write_register(0xFF41, STAT_LYC_INTERRUPT_ENABLE_BIT);

    assert!(ppu.snapshot().stat_irq_line);
    assert_eq!(
        drain_ppu_interrupts(&mut ppu),
        vec![InterruptSource::LcdStat]
    );
}

#[test]
fn dmg_lcd_restart_nonzero_stat_write_does_not_spuriously_request_before_lyc1() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x00,
        scy: 0x00,
        scx: 0x00,
        ly: 0,
        lyc: 1,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.lcd_restart_phase = PpuLcdRestartPhase::first_line_after_enable();
    ppu.blank_frame_active = true;
    ppu.startup_mode_latch = None;
    ppu.line_dot = 36;
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::HBlank);
    assert!(!ppu.snapshot().lyc_coincidence);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());

    ppu.write_register(0xFF41, STAT_LYC_INTERRUPT_ENABLE_BIT);

    assert!(!ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());

    ppu.ly = 1;
    ppu.line_dot = 1;
    ppu.refresh_stat_irq_line(false);

    assert!(ppu.snapshot().lyc_coincidence);
    assert!(ppu.snapshot().stat_irq_line);
    assert_eq!(
        drain_ppu_interrupts(&mut ppu),
        vec![InterruptSource::LcdStat]
    );
}

#[test]
fn dmg_vblank_stat_write_quirk_blocks_the_repeated_line153_lyc0_source() {
    let mut blocked = PpuTestRig::dmg();
    blocked.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x00,
        scy: 0x00,
        scx: 0x00,
        ly: 144,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    blocked.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    blocked.blank_frame_active = false;
    blocked.startup_mode_latch = None;
    blocked.line_dot = 8;
    blocked.write_register(0xFF41, STAT_LYC_INTERRUPT_ENABLE_BIT);
    assert_eq!(
        drain_ppu_interrupts(&mut blocked),
        vec![InterruptSource::LcdStat]
    );

    blocked.stat_state.irq_line = false;
    blocked.ly = TOTAL_SCANLINES - 1;
    // The pretrigger source reads the 1-T-cycle delayed window membership (deferred edge);
    // arm it so the test exercises the quirk-block against an otherwise-firing source.
    blocked.line_dot = LINE_153_LYC0_STAT_IRQ_PRETRIGGER_DOT + 1;
    blocked.stat_state.last_line_153_lyc0_pretrigger_window = true;
    blocked.refresh_stat_irq_line(false);

    assert!(!blocked.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut blocked).is_empty());

    let mut ordinary = PpuTestRig::dmg();
    ordinary.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: STAT_LYC_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: TOTAL_SCANLINES - 1,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ordinary.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ordinary.blank_frame_active = false;
    ordinary.startup_mode_latch = None;
    ordinary.stat_state.irq_line = false;
    ordinary.line_dot = LINE_153_LYC0_STAT_IRQ_PRETRIGGER_DOT + 1;
    ordinary.stat_state.last_line_153_lyc0_pretrigger_window = true;
    ordinary.refresh_stat_irq_line(false);

    assert!(ordinary.snapshot().stat_irq_line);
    assert_eq!(
        drain_ppu_interrupts(&mut ordinary),
        vec![InterruptSource::LcdStat]
    );
}

#[test]
fn dmg_lcd_restart_line1_cpu_stat_read_delays_lyc_coincidence_publication() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x00,
        scy: 0x00,
        scx: 0x00,
        ly: 1,
        lyc: 1,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.lcd_restart_phase = PpuLcdRestartPhase::first_line_after_enable();
    ppu.blank_frame_active = true;
    ppu.startup_mode_latch = None;
    ppu.line_dot = 1;

    assert!(ppu.snapshot().lyc_coincidence);
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::Immediate) & 0x04,
        0x04
    );
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x04,
        0x00
    );

    ppu.line_dot = LINE0_VBLANK_WRAP_STAT_READBACK_DELAY_DOTS;
    assert_eq!(
        ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation) & 0x04,
        0x04
    );
}

#[test]
fn dmg_stat_write_quirk_uses_explicit_line_dot_windows() {
    let startup = PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 1,
        lyc: 0x20,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    };

    let mut hblank_start = PpuTestRig::dmg();
    hblank_start.apply_startup_state(startup);
    hblank_start.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    hblank_start.blank_frame_active = false;
    hblank_start.startup_mode_latch = None;
    hblank_start.line_dot = hblank_start.current_mode0_start_dot();

    hblank_start.write_register(0xFF41, 0x00);

    assert!(!hblank_start.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut hblank_start).is_empty());

    let mut hblank_quirk = PpuTestRig::dmg();
    hblank_quirk.apply_startup_state(startup);
    hblank_quirk.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    hblank_quirk.blank_frame_active = false;
    hblank_quirk.startup_mode_latch = None;
    hblank_quirk.line_dot = hblank_quirk.current_mode0_start_dot() + 4;

    hblank_quirk.write_register(0xFF41, 0x00);

    assert!(hblank_quirk.snapshot().stat_irq_line);
    assert_eq!(
        drain_ppu_interrupts(&mut hblank_quirk),
        vec![InterruptSource::LcdStat]
    );

    let mut oam_start = PpuTestRig::dmg();
    oam_start.apply_startup_state(PpuStartupState { ly: 2, ..startup });
    oam_start.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    oam_start.blank_frame_active = false;
    oam_start.startup_mode_latch = None;
    oam_start.line_dot = 0;

    oam_start.write_register(0xFF41, 0x00);

    assert!(oam_start.snapshot().stat_irq_line);
    assert_eq!(
        drain_ppu_interrupts(&mut oam_start),
        vec![InterruptSource::LcdStat]
    );

    let mut oam_late = PpuTestRig::dmg();
    oam_late.apply_startup_state(PpuStartupState { ly: 2, ..startup });
    oam_late.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    oam_late.blank_frame_active = false;
    oam_late.startup_mode_latch = None;
    oam_late.line_dot = 4;

    oam_late.write_register(0xFF41, 0x00);

    assert!(!oam_late.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut oam_late).is_empty());

    let mut frame_start = PpuTestRig::dmg();
    frame_start.apply_startup_state(PpuStartupState { ly: 0, ..startup });
    frame_start.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    frame_start.blank_frame_active = false;
    frame_start.startup_mode_latch = None;
    frame_start.line_dot = 12;

    frame_start.write_register(0xFF41, 0x00);

    assert!(frame_start.snapshot().stat_irq_line);
    assert_eq!(
        drain_ppu_interrupts(&mut frame_start),
        vec![InterruptSource::LcdStat]
    );
}

#[test]
fn cgb_stat_write_does_not_inherit_the_dmg_spurious_interrupt_quirk() {
    for operating_mode in [OperatingMode::Cgb, OperatingMode::GbCompatible] {
        let mut ppu = PpuTestRig::with_model(ConsoleModel::GameBoyColor);
        ppu.apply_startup_state(PpuStartupState {
            lcdc: 0x80,
            stat: 0x80,
            scy: 0x00,
            scx: 0x00,
            ly: 1,
            lyc: 0x20,
            bgp: 0x00,
            wy: 0x00,
            wx: 0x00,
            obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
        });
        ppu.apply_operating_mode_state(operating_mode);
        ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
        ppu.blank_frame_active = false;
        ppu.startup_mode_latch = None;
        ppu.line_dot = ppu.current_mode0_start_dot() + 4;

        ppu.write_register(0xFF41, 0x00);

        assert!(
            !ppu.snapshot().stat_irq_line,
            "{operating_mode:?} must not reuse the DMG STAT write quirk"
        );
        assert!(drain_ppu_interrupts(&mut ppu).is_empty());
    }
}

#[test]
fn stat_write_arming_the_current_mode_source_latches_without_requesting_immediately() {
    let mut ppu = PpuTestRig::dmg();

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x00,
        scy: 0x00,
        scx: 0x00,
        ly: 1,
        lyc: 0x20,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    ppu.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    ppu.blank_frame_active = false;
    ppu.startup_mode_latch = None;
    ppu.line_dot = 24;
    ppu.refresh_stat_irq_line(false);
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::OamScan);
    assert!(!ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());

    ppu.write_register(0xFF41, STAT_MODE2_INTERRUPT_ENABLE_BIT);

    assert!(ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());

    ppu.tick_n(u64::from(MODE2_DOTS - 24));

    assert_eq!(ppu.snapshot().mode, PpuAccessMode::Drawing);
    assert!(!ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());
}

#[test]
fn lyc_coincidence_tracks_vblank_lines_and_the_line_153_ly0_window() {
    let mut ppu = PpuTestRig::dmg();

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 143,
        lyc: 144,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.advance_until_line_start(144);
    assert_eq!(ppu.snapshot().ly, 144);
    assert!(!ppu.snapshot().lyc_coincidence);

    ppu.tick();
    assert!(ppu.snapshot().lyc_coincidence);

    ppu.write_register(0xFF45, 153);
    assert!(!ppu.snapshot().lyc_coincidence);

    ppu.advance_until_line_start(153);
    assert_eq!(ppu.snapshot().ly, 153);
    assert!(!ppu.snapshot().lyc_coincidence);

    ppu.tick_n(u64::from(LINE_153_LYC153_COMPARE_START_DOT));
    assert!(ppu.snapshot().lyc_coincidence);

    ppu.tick_n(u64::from(
        LINE_153_LYC153_COMPARE_END_DOT - LINE_153_LYC153_COMPARE_START_DOT,
    ));
    assert!(!ppu.snapshot().lyc_coincidence);

    ppu.write_register(0xFF45, 0);
    assert!(!ppu.snapshot().lyc_coincidence);

    ppu.tick_n(u64::from(
        LINE_153_LYC0_COMPARE_START_DOT - LINE_153_LYC153_COMPARE_END_DOT,
    ));
    assert_eq!(ppu.snapshot().ly, 153);
    assert_eq!(ppu.read_register(0xFF44), 0);
    assert!(ppu.snapshot().lyc_coincidence);

    ppu.advance_until_line_start(0);
    assert_eq!(ppu.snapshot().ly, 0);
    assert!(ppu.snapshot().lyc_coincidence);
}

#[test]
fn cgb_lyc_zero_coincidence_rises_during_the_line_153_ly0_window() {
    let mut ppu = PpuTestRig::with_model(ConsoleModel::GameBoyColor);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x82,
        scy: 0x00,
        scx: 0x00,
        ly: 153,
        lyc: 0,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    assert_eq!(ppu.snapshot().ly, 153);
    assert!(!ppu.snapshot().lyc_coincidence);
    assert_eq!(ppu.read_register(0xFF44), 153);

    ppu.tick_n(u64::from(CGB_LINE_153_LY_READ_ZERO_DOT - 1));

    assert_eq!(ppu.snapshot().ly, 153);
    assert_eq!(ppu.read_register(0xFF44), 153);
    assert!(!ppu.snapshot().lyc_coincidence);

    ppu.tick();

    assert_eq!(ppu.snapshot().ly, 153);
    assert_eq!(ppu.read_register(0xFF44), 0);
    assert!(!ppu.snapshot().lyc_coincidence);

    ppu.tick();

    assert_eq!(ppu.read_register(0xFF44), 0);
    assert!(ppu.snapshot().lyc_coincidence);
}

#[test]
fn ly_read_advances_early_only_on_visible_hblank_lines() {
    let mut visible = PpuTestRig::dmg();
    visible.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x00,
        scy: 0x00,
        scx: 0x00,
        ly: 0x20,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    assert_eq!(LY_READ_ADVANCE_START_DOT, DOTS_PER_SCANLINE - 6);
    visible.line_dot = LY_READ_ADVANCE_START_DOT - 1;
    assert_eq!(visible.snapshot().mode, PpuAccessMode::HBlank);
    assert_eq!(visible.read_register(0xFF44), 0x20);

    visible.line_dot = LY_READ_ADVANCE_START_DOT;
    assert_eq!(visible.snapshot().mode, PpuAccessMode::HBlank);
    assert_eq!(visible.read_register(0xFF44), 0x21);

    let mut vblank = PpuTestRig::dmg();
    vblank.apply_startup_state(PpuStartupState {
        lcdc: 0x80,
        stat: 0x01,
        scy: 0x00,
        scx: 0x00,
        ly: 0x90,
        lyc: 0x00,
        bgp: 0x00,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    vblank.line_dot = LY_READ_ADVANCE_START_DOT;
    assert_eq!(vblank.snapshot().mode, PpuAccessMode::VBlank);
    assert_eq!(vblank.read_register(0xFF44), 0x90);
}

#[test]
fn dmg_skip_boot_ly_readback_lags_the_synthetic_first_frame_until_vblank() {
    let mut ppu = PpuTestRig::dmg();
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
    ppu.apply_dmg_skip_boot_stat_irq_startup_phase();

    assert_eq!(ppu.read_register(0xFF44), 153);

    ppu.ly = 65;
    ppu.line_dot = 56;
    assert_eq!(ppu.read_register(0xFF44), 64);

    ppu.ly = VISIBLE_SCANLINES - 1;
    ppu.line_dot = DOTS_PER_SCANLINE - 1;
    ppu.tick();
    assert_eq!(ppu.read_register(0xFF44), VISIBLE_SCANLINES);
}

#[test]
fn dmg_skip_boot_power_on_cpu_ly_read_uses_the_boot_facing_publication_table() {
    let mut ppu = dmg_skip_boot_power_on_rig();

    for (delay_mcycles, expected_ly) in [(0, 0), (119, 0), (120, 1), (233, 1), (234, 2)] {
        set_dmg_skip_boot_power_on_delay(&mut ppu, delay_mcycles);

        assert_eq!(
            ppu.read_register_with_source(0xFF44, PpuRegisterReadSource::CpuBusOperation),
            expected_ly,
            "delay {delay_mcycles}"
        );
    }
}

#[test]
fn dmg_skip_boot_power_on_cpu_stat_read_uses_the_boot_facing_publication_table() {
    let mut ppu = dmg_skip_boot_power_on_rig();

    for (delay_mcycles, expected_stat) in [
        (0, 0x85),
        (5, 0x85),
        (6, 0x84),
        (7, 0x86),
        (26, 0x86),
        (27, 0x87),
        (69, 0x87),
        (70, 0x84),
        (119, 0x84),
        (120, 0x80),
        (121, 0x82),
        (140, 0x82),
        (141, 0x83),
        (183, 0x83),
        (184, 0x80),
        (234, 0x80),
        (235, 0x82),
    ] {
        set_dmg_skip_boot_power_on_delay(&mut ppu, delay_mcycles);

        assert_eq!(
            ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation),
            expected_stat,
            "delay {delay_mcycles}"
        );
    }
}

#[test]
fn dmg_real_boot_handoff_power_on_cpu_reads_use_the_wrapped_boot_facing_publication_table() {
    let mut ppu = dmg_real_boot_handoff_power_on_rig();

    for (delay_mcycles, expected_ly) in [(0, 0), (119, 0), (120, 1), (233, 1), (234, 2)] {
        set_dmg_real_boot_handoff_power_on_delay(&mut ppu, delay_mcycles);

        assert_eq!(
            ppu.read_register_with_source(0xFF44, PpuRegisterReadSource::CpuBusOperation),
            expected_ly,
            "LY delay {delay_mcycles}"
        );
    }

    for (delay_mcycles, expected_stat) in [
        (0, 0x85),
        (6, 0x84),
        (7, 0x86),
        (120, 0x80),
        (121, 0x82),
        (234, 0x80),
        (235, 0x82),
    ] {
        set_dmg_real_boot_handoff_power_on_delay(&mut ppu, delay_mcycles);

        assert_eq!(
            ppu.read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation),
            expected_stat,
            "STAT delay {delay_mcycles}"
        );
    }
}

#[test]
fn ly_is_read_only_and_obj_palettes_keep_an_explicit_uninitialized_policy() {
    let mut ppu = PpuTestRig::dmg();
    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x08,
        scy: 0x00,
        scx: 0x00,
        ly: 0x22,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    ppu.write_register(0xFF44, 0x99);

    assert_eq!(ppu.read_register(0xFF44), 0x22);
    assert_eq!(ppu.read_register(0xFF48), 0xFF);
    assert_eq!(ppu.read_register(0xFF49), 0xFF);
}

#[test]
fn cgb_obj_palettes_read_back_zero_before_any_write() {
    let mut ppu = PpuTestRig::with_model(ConsoleModel::GameBoyColor);

    assert_eq!(ppu.read_register(0xFF48), 0x00);
    assert_eq!(ppu.read_register(0xFF49), 0x00);

    ppu.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: 0x08,
        scy: 0x00,
        scx: 0x00,
        ly: 0x22,
        lyc: 0x00,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });

    assert_eq!(ppu.read_register(0xFF48), 0x00);
    assert_eq!(ppu.read_register(0xFF49), 0x00);
}
