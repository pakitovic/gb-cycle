use super::super::protocol::SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED;
use super::super::*;
use super::support::*;

#[test]
fn borrowed_border_header_gate_requires_sgb_flag_and_old_licensee() {
    let accepted = test_header(SgbFlag::Supported, SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED);
    let missing_flag = test_header(SgbFlag::None, SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED);
    let missing_licensee = test_header(SgbFlag::Supported, 0x01);

    assert!(sgb_header_accepts_borrowed_border(&accepted));
    assert!(!sgb_header_accepts_borrowed_border(&missing_flag));
    assert!(!sgb_header_accepts_borrowed_border(&missing_licensee));
}

#[test]
fn borrowed_border_exposes_only_pixels_outside_the_lcd_aperture() {
    let mut border = SgbBorderState::default();
    border.tile_data.bytes[0] = 0x80;
    border.palettes[0].colors[1] = SgbRgb555Color::new(0x1234);
    let borrowed = SgbBorrowedBorder::new(border);

    assert_eq!(borrowed.pixel_rgb555_outside_lcd(0, 0), Some(0x1234));
    assert_eq!(
        borrowed.pixel_rgb555_outside_lcd(SGB_LCD_FRAME_ORIGIN_X, SGB_LCD_FRAME_ORIGIN_Y),
        None
    );
    assert_eq!(borrowed.pixel_rgb555_outside_lcd(SGB_FRAME_WIDTH, 0), None);
    assert_eq!(borrowed.pixel_rgb555_outside_lcd(0, SGB_FRAME_HEIGHT), None);
}

#[test]
fn borrowed_border_color_zero_uses_captured_backdrop_outside_lcd() {
    let mut border = SgbBorderState::default();
    border.palettes[0].colors[0] = SgbRgb555Color::new(0x7CD2);
    let borrowed = SgbBorrowedBorder::with_backdrop_color(border, SgbRgb555Color::new(0x001F));

    assert_eq!(borrowed.backdrop_color().raw(), 0x001F);
    assert_eq!(
        borrowed.pixel_rgb555_outside_lcd(0, 0),
        Some(0x001F),
        "borrowed borders must match real SGB presentation: border color index 0 is transparent to the application backdrop instead of using local PCT_TRN palette color 0"
    );
}
