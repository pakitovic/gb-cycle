use super::super::*;

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

    assert!(ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());

    ppu.write_register(0xFF45, 0x21);

    assert!(ppu.snapshot().lyc_coincidence);
    assert!(ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());
}

#[test]
fn dmg_mode2_enable_requests_lcd_stat_at_vblank_entry_only() {
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
    ppu.line_dot = DOTS_PER_SCANLINE - 1;
    ppu.refresh_stat_irq_line(false);
    assert!(!ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());

    ppu.tick();

    assert_eq!(ppu.snapshot().ly, 144);
    assert_eq!(ppu.snapshot().mode, PpuAccessMode::VBlank);
    assert!(ppu.snapshot().stat_irq_line);
    assert_eq!(
        drain_ppu_interrupts(&mut ppu),
        vec![InterruptSource::VBlank, InterruptSource::LcdStat]
    );

    ppu.tick();

    assert_eq!(ppu.snapshot().ly, 144);
    assert_eq!(ppu.snapshot().line_dot, 1);
    assert!(!ppu.snapshot().stat_irq_line);
    assert!(drain_ppu_interrupts(&mut ppu).is_empty());
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
    assert!(ppu.snapshot().lyc_coincidence);

    ppu.write_register(0xFF45, 153);
    assert!(!ppu.snapshot().lyc_coincidence);

    ppu.advance_until_line_start(153);
    assert_eq!(ppu.snapshot().ly, 153);
    assert!(ppu.snapshot().lyc_coincidence);

    ppu.write_register(0xFF45, 0);
    assert!(!ppu.snapshot().lyc_coincidence);

    ppu.tick_n(u64::from(LINE_153_LY0_DOT));
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

    ppu.tick_n(u64::from(LINE_153_LY0_DOT));

    assert_eq!(ppu.snapshot().ly, 153);
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
