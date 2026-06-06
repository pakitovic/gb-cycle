use super::super::shell;
use super::support::*;

#[test]
fn sgb_shell_default_border_loads_for_all_sgb_hosts() {
    for startup_mode in [
        StartupMode::RealBoot,
        StartupMode::SkipBoot,
        StartupMode::CustomBoot,
    ] {
        let mut host = SgbHost::new_with_startup(HostPlatform::Sgb, startup_mode);
        assert!(host.snapshot().shell.enabled);
        assert!(host.snapshot().shell.default_border_loaded);
        assert!(host.snapshot().video.border_loaded);

        let supported = test_header(SgbFlag::Supported, SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED);
        host.apply_cartridge_header(Some(&supported));
        assert_eq!(host.command_acceptance(), SgbCommandAcceptance::Accepted);
        assert!(host.snapshot().shell.default_border_loaded);
        assert!(host.snapshot().video.border_loaded);

        let unsupported = test_header(SgbFlag::None, 0x01);
        host.apply_cartridge_header(Some(&unsupported));
        assert_eq!(
            host.command_acceptance(),
            SgbCommandAcceptance::RejectedByHeader
        );
        assert!(host.snapshot().shell.default_border_loaded);
        assert!(host.snapshot().video.border_loaded);
    }

    let sgb2 = SgbHost::new(HostPlatform::Sgb2);
    assert!(sgb2.snapshot().shell.default_border_loaded);
    assert!(sgb2.snapshot().video.border_loaded);
    assert_eq!(sgb2.snapshot().profile, Some(SgbHostProfile::Sgb2Ntsc));

    let handheld = SgbHost::new(HostPlatform::Handheld);
    assert!(!handheld.snapshot().shell.enabled);
    assert!(!handheld.snapshot().shell.default_border_loaded);
    assert!(!handheld.snapshot().video.border_loaded);
}

#[test]
fn sgb_shell_default_border_uses_owned_generic_asset_for_sgb_and_sgb2() {
    let sgb = SgbHost::new(HostPlatform::Sgb);
    let sgb2 = SgbHost::new(HostPlatform::Sgb2);
    let sgb_snapshot = sgb.snapshot();
    let sgb2_snapshot = sgb2.snapshot();
    let sgb_border = &sgb_snapshot.video.border;
    let sgb2_border = &sgb2_snapshot.video.border;

    assert_eq!(
        sgb_border, sgb2_border,
        "SGB and SGB2 use the same generic fallback border asset"
    );
    assert!(
        sgb_border
            .tile_map
            .entries
            .iter()
            .take(SGB_BORDER_TILEMAP_WIDTH * SGB_BORDER_TILEMAP_VISIBLE_HEIGHT)
            .any(|entry| entry.raw & 0x03FF != 0),
        "the owned fallback border should include visible non-blank tiles"
    );

    let (_, top_edge_color_index) =
        sgb_border.pixel_color(SGB_LCD_FRAME_ORIGIN_X, SGB_LCD_FRAME_ORIGIN_Y - 1);
    assert_ne!(
        top_edge_color_index, 0,
        "the fallback border must keep the row immediately above the LCD window opaque"
    );
    let (_, color_index) = sgb_border.pixel_color(SGB_LCD_FRAME_ORIGIN_X, SGB_LCD_FRAME_ORIGIN_Y);
    assert_eq!(
        color_index, 0,
        "asset transparency must remain color-index 0 inside the LCD window"
    );
    let (_, last_lcd_row_color_index) = sgb_border.pixel_color(
        SGB_LCD_FRAME_ORIGIN_X,
        SGB_LCD_FRAME_ORIGIN_Y + SGB_LCD_HEIGHT - 1,
    );
    assert_eq!(
        last_lcd_row_color_index, 0,
        "the fallback border transparent aperture must include the final LCD row"
    );
    let (_, bottom_edge_color_index) = sgb_border.pixel_color(
        SGB_LCD_FRAME_ORIGIN_X,
        SGB_LCD_FRAME_ORIGIN_Y + SGB_LCD_HEIGHT,
    );
    assert_ne!(
        bottom_edge_color_index, 0,
        "the fallback border must resume opaque art immediately below the LCD window"
    );
    let (_, bottom_frame_color_index) = sgb_border.pixel_color(0, SGB_FRAME_HEIGHT - 1);
    assert_ne!(
        bottom_frame_color_index, 0,
        "the fallback border bottom scanline must be opaque instead of leaking the backdrop"
    );
    let lcd = vec![3; SGB_LCD_PIXELS];
    let frame = sgb
        .compose_frame_rgb555(&lcd)
        .expect("generic SGB fallback border should compose");
    let live_lcd_color = sgb.video.lcd_pixel_for_shade(3).raw();
    assert_eq!(
        frame[SGB_LCD_FRAME_ORIGIN_Y * SGB_FRAME_WIDTH + SGB_LCD_FRAME_ORIGIN_X],
        live_lcd_color,
        "transparent fallback border pixels inside the LCD window preserve live LCD output"
    );
    assert_eq!(
        frame[(SGB_LCD_FRAME_ORIGIN_Y + SGB_LCD_HEIGHT - 1) * SGB_FRAME_WIDTH
            + SGB_LCD_FRAME_ORIGIN_X],
        live_lcd_color,
        "transparent fallback border pixels preserve the final live LCD scanline"
    );
    assert_ne!(
        frame[(SGB_LCD_FRAME_ORIGIN_Y - 1) * SGB_FRAME_WIDTH + SGB_LCD_FRAME_ORIGIN_X],
        live_lcd_color,
        "the scanline above the LCD window must be border art, not leaked LCD/backdrop color"
    );
    assert_ne!(
        frame[(SGB_FRAME_HEIGHT - 1) * SGB_FRAME_WIDTH],
        live_lcd_color,
        "the bottom host-frame scanline must be border art, not leaked LCD/backdrop color"
    );
}

#[test]
fn sgb_shell_default_border_survives_save_state_for_non_sgb_enhanced_headers() {
    let mut host = SgbHost::new(HostPlatform::Sgb);
    let header = test_header(SgbFlag::None, 0x01);
    host.apply_cartridge_header(Some(&header));
    assert_eq!(
        host.command_acceptance(),
        SgbCommandAcceptance::RejectedByHeader
    );

    let saved = host.capture_save_state();
    let mut restored = SgbHost::new(HostPlatform::Handheld);
    restored.restore_save_state(&saved);
    assert_eq!(restored.capture_save_state(), saved);

    let lcd = vec![0; SGB_LCD_PIXELS];
    let frame = restored
        .compose_frame_rgb555(&lcd)
        .expect("default SGB border should remain for non-SGB-enhanced headers");
    assert!(
        frame[..SGB_LCD_FRAME_ORIGIN_Y * SGB_FRAME_WIDTH]
            .iter()
            .any(|&pixel| pixel != 0),
        "default shell border should contribute visible pixels outside the LCD window"
    );
}

#[test]
fn sgb_enhanced_headers_start_with_fallback_until_game_border_transfer() {
    let host = accepted_sgb_host();
    assert_eq!(host.command_acceptance(), SgbCommandAcceptance::Accepted);
    assert!(host.snapshot().shell.default_border_loaded);
    assert!(host.snapshot().video.border_loaded);

    let lcd = vec![0; SGB_LCD_PIXELS];
    let frame = host
        .compose_frame_rgb555(&lcd)
        .expect("SGB-enhanced host frame should compose with fallback before game border");
    assert!(
        frame[..SGB_LCD_FRAME_ORIGIN_Y * SGB_FRAME_WIDTH]
            .iter()
            .any(|&pixel| pixel != 0),
        "fallback border should be visible while an SGB-enhanced game starts"
    );
}

#[test]
fn chr_trn_starts_fallback_fade_before_game_pct() {
    let mut host = accepted_sgb_host();
    let fallback_frame = host
        .compose_frame_rgb555(&vec![0; SGB_LCD_PIXELS])
        .expect("fallback SGB border should compose before CHR_TRN");
    let vram = transfer_vram_from_payload(&solid_tile_color_1_transfer());

    write_joyp_packet(&mut host, sgb_chr_trn_packet(0));
    let snapshot = host.snapshot();
    assert!(!snapshot.shell.default_border_loaded);
    assert!(snapshot.shell.border_transition.fallback_border.is_some());
    assert_eq!(
        snapshot.shell.border_transition.phase,
        shell::SgbShellBorderTransitionPhase::FadeFallbackToBlack
    );

    assert_eq!(
        host.compose_frame_rgb555(&vec![0; SGB_LCD_PIXELS])
            .expect("first fade-out frame should compose"),
        fallback_frame,
        "the command frame keeps the fallback fully visible before the next host frame advances"
    );

    host.advance_frame_start(&vram, fallback_transfer_display())
        .expect("CHR_TRN should advance through the shared frame path");
    assert_ne!(
        host.compose_frame_rgb555(&vec![0; SGB_LCD_PIXELS])
            .expect("advanced fade-out frame should compose"),
        fallback_frame,
        "fallback fade-out starts before the game PCT_TRN border is ready"
    );

    for _ in 1..usize::from(shell::SGB_SHELL_BORDER_FADE_FRAMES) {
        host.advance_frame_start(&vram, fallback_transfer_display())
            .expect("fade-out should continue while waiting for PCT_TRN");
    }
    assert_eq!(
        host.snapshot().shell.border_transition.phase,
        shell::SgbShellBorderTransitionPhase::HoldBlackUntilGameBorder
    );
    assert_eq!(
        host.compose_frame_rgb555(&vec![0; SGB_LCD_PIXELS])
            .expect("black hold frame should compose")[0],
        0,
        "the fallback can reach black before the cartridge border map/palette is ready"
    );
}

#[test]
fn shell_black_transition_owns_transparent_lcd_window() {
    let mut host = accepted_sgb_host();
    write_joyp_packet(&mut host, sgb_pal01_packet());

    let lcd = vec![1; SGB_LCD_PIXELS];
    let lcd_window_index = SGB_LCD_FRAME_ORIGIN_Y * SGB_FRAME_WIDTH + SGB_LCD_FRAME_ORIGIN_X;
    let live_lcd_color = host.video.lcd_pixel_for_shade(1);
    assert_ne!(
        live_lcd_color.raw(),
        0,
        "the test LCD shade must be non-black to catch shell-window leaks"
    );
    assert_eq!(
        host.compose_frame_rgb555(&lcd)
            .expect("fallback border should compose before CHR_TRN")[lcd_window_index],
        live_lcd_color.raw(),
        "idle fallback transparency should still show the live LCD"
    );

    let vram = transfer_vram_from_payload(&solid_tile_color_1_transfer());
    write_joyp_packet(&mut host, sgb_chr_trn_packet(0));
    assert_eq!(
        host.compose_frame_rgb555(&lcd)
            .expect("first fade frame should compose")[lcd_window_index],
        live_lcd_color.raw(),
        "fade-out starts from the live LCD before the host frame advances"
    );

    host.advance_frame_start(&vram, fallback_transfer_display())
        .expect("CHR_TRN should advance through the shared frame path");
    let faded_lcd_color = shell::scale_rgb555(
        live_lcd_color,
        shell::SGB_SHELL_BORDER_FADE_FRAMES.saturating_sub(1),
    )
    .raw();
    assert_eq!(
        host.compose_frame_rgb555(&lcd)
            .expect("advanced fade frame should compose")[lcd_window_index],
        faded_lcd_color,
        "transparent LCD-window pixels must fade with the shell instead of leaking live LCD"
    );

    for _ in 1..usize::from(shell::SGB_SHELL_BORDER_FADE_FRAMES) {
        host.advance_frame_start(&vram, fallback_transfer_display())
            .expect("fade-out should continue while waiting for PCT_TRN");
    }
    assert_eq!(
        host.snapshot().shell.border_transition.phase,
        shell::SgbShellBorderTransitionPhase::HoldBlackUntilGameBorder
    );
    assert_eq!(
        host.compose_frame_rgb555(&lcd)
            .expect("black hold frame should compose")[lcd_window_index],
        0,
        "black shell hold must cover the transparent LCD aperture until PCT_TRN is ready"
    );
}

#[test]
fn pct_transfer_fades_from_shell_fallback_to_game_border() {
    let mut host = accepted_sgb_host();
    let before_count = host.snapshot().video.border.pct_transfer_count;
    let fallback_frame = host
        .compose_frame_rgb555(&vec![0; SGB_LCD_PIXELS])
        .expect("fallback SGB border should compose before PCT_TRN");
    let mut payload = [0; SGB_VRAM_TRANSFER_BYTES];
    for tile_index in 0..SGB_BORDER_TILEMAP_WIDTH * SGB_BORDER_TILEMAP_VISIBLE_HEIGHT {
        write_border_map_entry(&mut payload, tile_index, (4 << 10) | 1);
    }
    let [low, high] = 0x001Fu16.to_le_bytes();
    payload[0x800 + 2] = low;
    payload[0x800 + 3] = high;
    let vram = transfer_vram_from_payload(&payload);

    write_joyp_packet(&mut host, sgb_pct_trn_packet());
    assert!(
        host.snapshot()
            .shell
            .border_transition
            .fallback_border
            .is_some()
    );
    assert_eq!(
        host.snapshot().shell.border_transition.phase,
        shell::SgbShellBorderTransitionPhase::FadeFallbackToBlack
    );

    let fade_frame = host
        .compose_frame_rgb555(&vec![0; SGB_LCD_PIXELS])
        .expect("first transition frame should compose");
    assert_eq!(
        fallback_frame, fade_frame,
        "transition starts from the fully visible fallback border before the fade advances"
    );

    host.advance_frame_start(&vram, fallback_transfer_display())
        .expect("fade should advance one host frame");
    let fade_frame = host
        .compose_frame_rgb555(&vec![0; SGB_LCD_PIXELS])
        .expect("advanced fade frame should compose");
    assert_ne!(fallback_frame, fade_frame);

    let saved = host.capture_save_state();
    assert!(saved.dynamic_payload_bytes() > host.snapshot().video.dynamic_payload_bytes());
    let mut restored = SgbHost::new(HostPlatform::Handheld);
    restored.restore_save_state(&saved);
    assert_eq!(restored.capture_save_state(), saved);
    assert_eq!(
        restored
            .compose_frame_rgb555(&vec![0; SGB_LCD_PIXELS])
            .expect("restored fade frame should compose identically"),
        fade_frame
    );
    host = restored;

    for _ in 1..SGB_VRAM_TRANSFER_TOTAL_FRAMES {
        host.advance_frame_start(&vram, fallback_transfer_display())
            .expect("PCT_TRN should advance through the shared frame path");
    }

    let snapshot = host.snapshot();
    assert_eq!(snapshot.video.border.pct_transfer_count, before_count + 1);
    assert!(!snapshot.shell.default_border_loaded);
    assert!(snapshot.video.border_loaded);
    assert_eq!(
        snapshot.shell.border_transition.phase,
        shell::SgbShellBorderTransitionPhase::FadeFallbackToBlack
    );
    assert!(snapshot.shell.border_transition.game_border_ready);
    assert_eq!(snapshot.video.border.tile_map.entries[0].raw, (4 << 10) | 1);

    for _ in usize::from(snapshot.shell.border_transition.frame)
        ..usize::from(shell::SGB_SHELL_BORDER_FADE_FRAMES)
    {
        host.advance_frame_start(&vram, fallback_transfer_display())
            .expect("fade should advance to game-border phase");
    }
    assert_eq!(
        host.snapshot().shell.border_transition.phase,
        shell::SgbShellBorderTransitionPhase::FadeBlackToGame
    );

    for _ in 0..usize::from(shell::SGB_SHELL_BORDER_FADE_FRAMES) {
        host.advance_frame_start(&vram, fallback_transfer_display())
            .expect("fade should complete");
    }
    assert_eq!(
        host.snapshot().shell.border_transition.phase,
        shell::SgbShellBorderTransitionPhase::Idle
    );
    assert!(
        host.snapshot()
            .shell
            .border_transition
            .fallback_border
            .is_none()
    );
}

#[test]
fn save_state_restores_the_explicit_host_shell_state() {
    let mut host = SgbHost::new(HostPlatform::Sgb2);
    let saved = host.capture_save_state();
    host = SgbHost::new(HostPlatform::Handheld);
    host.restore_save_state(&saved);
    assert_eq!(host.capture_save_state(), saved);
    assert_eq!(host.profile(), Some(SgbHostProfile::Sgb2Ntsc));
}
