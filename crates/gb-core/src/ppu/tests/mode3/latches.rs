use super::super::*;

#[test]
fn mode3_latch_helpers_expose_the_expected_register_snapshots_and_accessors() {
    let visible = PpuVisibleRegisters {
        lcdc: 0x97,
        scy: 0x12,
        scx: 0x34,
        bgp: 0xE4,
        obp0: Some(0xD2),
        obp1: Some(0x4B),
        wy: 0x20,
        wx: 0x0E,
    };
    let pipeline = PpuVisibleRegisters {
        lcdc: 0x93,
        scy: 0x56,
        scx: 0x78,
        bgp: 0x1B,
        obp0: Some(0x0F),
        obp1: Some(0xF0),
        wy: 0x21,
        wx: 0x40,
    };

    let from_mmio = PpuMode3RegisterLatches::from_mmio(visible);
    assert_eq!(from_mmio.visible(), visible);
    assert_eq!(from_mmio.pipeline(), visible);

    let latches = PpuMode3RegisterLatches::new(visible, pipeline);
    assert_eq!(latches.visible(), visible);
    assert_eq!(latches.pipeline(), pipeline);
    assert_eq!(latches.bg_fetch_registers(false), visible);
    assert_eq!(latches.bg_fetch_registers(true), pipeline);
    assert_eq!(latches.window_fetch_registers(false), visible);
    assert_eq!(latches.window_fetch_registers(true), pipeline);
    assert_eq!(
        latches.window_activation_registers(ConsoleModel::GameBoy),
        pipeline
    );
    assert_eq!(
        latches.window_activation_registers(ConsoleModel::GameBoyColor),
        visible
    );
    assert_eq!(latches.mode3_start_scx(), visible.scx);
    assert_eq!(latches.current_obj_height(), 16);
    assert_eq!(
        latches.pixel_pipeline_lcdc(ConsoleModel::GameBoy, 7),
        pipeline.lcdc
    );
    assert_eq!(
        latches.pixel_pipeline_lcdc(ConsoleModel::GameBoy, 8),
        visible.lcdc
    );
    assert_eq!(
        latches.pixel_pipeline_lcdc(ConsoleModel::GameBoyColor, 0),
        visible.lcdc
    );
    assert_eq!(
        latches.pixel_pipeline_bgp(ConsoleModel::GameBoy, None, None),
        0xFF
    );
    assert_eq!(
        latches.pixel_pipeline_bgp(ConsoleModel::GameBoy, Some(0x12), None),
        0x12
    );
    assert_eq!(
        latches.pixel_pipeline_bgp(ConsoleModel::GameBoy, None, Some(0x34)),
        0x34
    );
    assert_eq!(
        latches.pixel_pipeline_bgp(ConsoleModel::GameBoyColor, None, None),
        visible.bgp
    );
    assert!(latches.pixel_transfer_bg_enabled(ConsoleModel::GameBoy, 0));
    assert!(latches.pixel_transfer_obj_enabled(ConsoleModel::GameBoy, 0));
    assert!(latches.lcdc_bit_changed(LCDC_OBJ_SIZE_BIT));

    let advanced = latches.advance(PpuVisibleRegisters {
        scx: 0x99,
        ..visible
    });
    assert_eq!(advanced.visible().scx, 0x99);
    assert_eq!(advanced.pipeline(), visible);

    let refetch = PpuMode3LiveBackgroundRefetchContext::new(visible, 0x08, 0x03, 0xAA, 0xBB);
    assert_eq!(refetch.registers(), visible);
    assert_eq!(refetch.current_scanline_tile_row(), 2);
    assert_eq!(refetch.current_window_tile_row(), 3);
    assert_eq!(refetch.last_unsigned_tile_data_low_fetch(), 0xAA);
    assert_eq!(refetch.last_unsigned_tile_data_high_fetch(), 0xBB);
}

#[test]
fn mode3_fetch_and_window_helper_contexts_keep_addressing_rules_explicit() {
    let registers = PpuVisibleRegisters {
        lcdc: 0xF7,
        scy: 0x09,
        scx: 0x11,
        bgp: 0xE4,
        obp0: Some(0xD2),
        obp1: Some(0x4B),
        wy: 0x04,
        wx: 0x00,
    };

    let bg_fetch = PpuMode3BackgroundFetchContext::new(registers, registers, 0x10, 0x07);
    assert_eq!(bg_fetch.tile_index_address(), 0x1844);
    assert_eq!(bg_fetch.tile_data_address(0x05, 1), 0x0051);
    assert!(bg_fetch.uses_unsigned_tile_data());

    let window_fetch = PpuMode3WindowFetchContext::new(registers, 0x0A, 0x03);
    assert_eq!(window_fetch.tile_index_address(), 0x1C23);
    assert_eq!(window_fetch.tile_data_address(0x05, 1), 0x0055);
    assert!(window_fetch.uses_unsigned_tile_data());

    let activation = PpuMode3WindowActivationState::new(registers, false);
    assert!(activation.runtime_enabled());
    assert!(activation.is_wx_zero());
    assert!(!activation.is_wx_166());
    assert_eq!(activation.trigger_x(), Some(0));

    let wx166 = PpuMode3WindowActivationState::new(
        PpuVisibleRegisters {
            wx: 166,
            ..registers
        },
        false,
    );
    assert!(wx166.is_wx_166());
    assert_eq!(wx166.trigger_x(), Some(159));

    let forced_x0 = PpuMode3WindowActivationState::new(
        PpuVisibleRegisters {
            wx: 166,
            ..registers
        },
        true,
    );
    assert_eq!(forced_x0.trigger_x(), Some(0));
    assert!(!forced_x0.is_wx_166());

    let prepared = PpuMode3PreparedWindowLine::new(true, false, true);
    assert!(prepared.wy_triggered());
    assert!(!prepared.wy_latch());
    assert!(prepared.force_x0_this_line());
}

#[test]
fn cgb_window_activation_does_not_treat_lcdc0_as_bg_window_disable() {
    let visible = PpuVisibleRegisters {
        lcdc: LCDC_ENABLE_BIT | LCDC_WINDOW_ENABLE_BIT,
        wx: 7,
        ..PpuVisibleRegisters::default()
    };
    let latches = PpuMode3RegisterLatches::from_mmio(visible);

    let activation = PpuMode3WindowActivationState::new(
        latches.window_activation_registers(ConsoleModel::GameBoyColor),
        false,
    );

    assert!(activation.runtime_enabled());
    assert_eq!(activation.trigger_x(), Some(0));
}
