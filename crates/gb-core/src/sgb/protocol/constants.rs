use super::*;

pub(in crate::sgb) const JOYP_SELECT_BITS_MASK: u8 = 0x30;
pub(in crate::sgb) const SGB_JOYP_IDLE_BITS: u8 = 0x30;
pub(in crate::sgb) const SGB_JOYP_START_BITS: u8 = 0x00;
pub(in crate::sgb) const SGB_JOYP_ZERO_BITS: u8 = 0x20;
pub(in crate::sgb) const SGB_JOYP_ONE_BITS: u8 = 0x10;
pub const SGB_COMMAND_PACKET_BYTES: usize = 16;
pub const SGB_COMMAND_MAX_PACKETS: usize = 7;
pub const SGB_LCD_WIDTH: usize = 160;
pub const SGB_LCD_HEIGHT: usize = 144;
pub const SGB_LCD_PIXELS: usize = SGB_LCD_WIDTH * SGB_LCD_HEIGHT;
pub const SGB_FRAME_WIDTH: usize = 256;
pub const SGB_FRAME_HEIGHT: usize = 224;
pub const SGB_FRAME_PIXELS: usize = SGB_FRAME_WIDTH * SGB_FRAME_HEIGHT;
pub const SGB_LCD_FRAME_ORIGIN_X: usize = 48;
pub const SGB_LCD_FRAME_ORIGIN_Y: usize = 40;
pub const SGB_SCREEN_PALETTE_COUNT: usize = 4;
pub const SGB_SCREEN_PALETTE_COLORS: usize = 4;
pub const SGB_SYSTEM_PALETTE_COUNT: usize = 512;
pub const SGB_ATTR_MAP_WIDTH: usize = 20;
pub const SGB_ATTR_MAP_HEIGHT: usize = 18;
pub const SGB_ATTR_MAP_CELLS: usize = SGB_ATTR_MAP_WIDTH * SGB_ATTR_MAP_HEIGHT;
pub const SGB_ATF_COUNT: usize = 45;
pub const SGB_ATF_BYTES: usize = 90;
pub const SGB_ATF_TOTAL_BYTES: usize = SGB_ATF_COUNT * SGB_ATF_BYTES;
pub const SGB_VRAM_TRANSFER_BYTES: usize = 0x1000;
pub const SGB_BORDER_TILE_BYTES: usize = 32;
pub const SGB_BORDER_TILE_COUNT: usize = 256;
pub const SGB_BORDER_TILE_DATA_BYTES: usize = SGB_BORDER_TILE_BYTES * SGB_BORDER_TILE_COUNT;
pub const SGB_BORDER_TILEMAP_WIDTH: usize = 32;
pub const SGB_BORDER_TILEMAP_VISIBLE_HEIGHT: usize = 28;
pub const SGB_BORDER_TILEMAP_STORED_HEIGHT: usize = 29;
pub const SGB_BORDER_TILEMAP_ENTRIES: usize =
    SGB_BORDER_TILEMAP_WIDTH * SGB_BORDER_TILEMAP_STORED_HEIGHT;
pub const SGB_BORDER_PALETTE_COUNT: usize = 3;
pub const SGB_BORDER_PALETTE_COLORS: usize = 16;
pub const SGB_CONTROLLER_COUNT: usize = 4;
pub const SGB_DATA_SND_INLINE_BYTES: usize = 11;
pub const SGB_SNES_DATA_TRN_BYTES: u32 = SGB_VRAM_TRANSFER_BYTES as u32;
pub const SGB_OBJ_OAM_PAYLOAD_BYTES: usize = 0x70;

pub(in crate::sgb) const SGB_TRANSFER_DISPLAY_TILE_COLUMNS: usize = 20;
pub(in crate::sgb) const SGB_TRANSFER_DISPLAY_TILE_COUNT: usize =
    SGB_VRAM_TRANSFER_BYTES / SGB_GB_TILE_BYTES;
pub(in crate::sgb) const SGB_GB_VRAM_BYTES: usize = 0x2000;
pub(in crate::sgb) const SGB_GB_TILE_BYTES: usize = 16;
pub(in crate::sgb) const SGB_GB_TILEMAP_WIDTH: usize = 32;
pub(in crate::sgb) const SGB_GB_BG_MAP_9800_OFFSET: usize = 0x1800;
pub(in crate::sgb) const SGB_GB_BG_MAP_9C00_OFFSET: usize = 0x1C00;
pub(in crate::sgb) const SGB_GB_SIGNED_TILE_DATA_BASE_OFFSET: i32 = 0x1000;
pub(in crate::sgb) const SGB_TRANSFER_REQUIRED_BGP: u8 = 0xE4;
pub(in crate::sgb) const SGB_LCDC_ENABLE_BIT: u8 = 0x80;
pub(in crate::sgb) const SGB_LCDC_BG_TILE_MAP_BIT: u8 = 0x08;
pub(in crate::sgb) const SGB_LCDC_BG_WINDOW_TILE_DATA_BIT: u8 = 0x10;
pub(in crate::sgb) const SGB_LCDC_BG_ENABLE_BIT: u8 = 0x01;
pub(in crate::sgb) const SGB_PACKET_BYTES: usize = SGB_COMMAND_PACKET_BYTES;
pub(in crate::sgb) const SGB_PACKET_BITS: u8 = 128;
pub(in crate::sgb) const SGB_PACKET_COUNT_MIN: u8 = 1;
pub(in crate::sgb) const SGB_PACKET_COUNT_MAX: u8 = SGB_COMMAND_MAX_PACKETS as u8;
pub(in crate::sgb) const SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED: u8 = 0x33;
pub(in crate::sgb) const SGB_RGB555_MASK: u16 = 0x7FFF;
pub(in crate::sgb) const SGB_RGB555_WHITE: SgbRgb555Color = SgbRgb555Color::new(0x7FFF);
pub(in crate::sgb) const SGB_RGB555_LIGHT_GRAY: SgbRgb555Color = SgbRgb555Color::new(0x5294);
pub(in crate::sgb) const SGB_RGB555_DARK_GRAY: SgbRgb555Color = SgbRgb555Color::new(0x294A);
pub(in crate::sgb) const SGB_RGB555_BLACK: SgbRgb555Color = SgbRgb555Color::new(0x0000);
pub(in crate::sgb) const SGB_BOOT_PALETTE_COUNT: usize = 32;
pub(in crate::sgb) const SGB_BOOT_PALETTE_DEFAULT_INDEX: u8 = 1;
pub(in crate::sgb) const SGB_COMMAND_PAL01: u8 = 0x00;
pub(in crate::sgb) const SGB_COMMAND_PAL23: u8 = 0x01;
pub(in crate::sgb) const SGB_COMMAND_PAL03: u8 = 0x02;
pub(in crate::sgb) const SGB_COMMAND_PAL12: u8 = 0x03;
pub(in crate::sgb) const SGB_COMMAND_ATTR_BLK: u8 = 0x04;
pub(in crate::sgb) const SGB_COMMAND_ATTR_LIN: u8 = 0x05;
pub(in crate::sgb) const SGB_COMMAND_ATTR_DIV: u8 = 0x06;
pub(in crate::sgb) const SGB_COMMAND_ATTR_CHR: u8 = 0x07;
pub(in crate::sgb) const SGB_COMMAND_SOUND: u8 = 0x08;
pub(in crate::sgb) const SGB_COMMAND_SOU_TRN: u8 = 0x09;
pub(in crate::sgb) const SGB_COMMAND_PAL_SET: u8 = 0x0A;
pub(in crate::sgb) const SGB_COMMAND_PAL_TRN: u8 = 0x0B;
pub(in crate::sgb) const SGB_COMMAND_ATRC_EN: u8 = 0x0C;
pub(in crate::sgb) const SGB_COMMAND_TEST_EN: u8 = 0x0D;
pub(in crate::sgb) const SGB_COMMAND_ICON_EN: u8 = 0x0E;
pub(in crate::sgb) const SGB_COMMAND_DATA_SND: u8 = 0x0F;
pub(in crate::sgb) const SGB_COMMAND_DATA_TRN: u8 = 0x10;
pub(in crate::sgb) const SGB_COMMAND_CHR_TRN: u8 = 0x13;
pub(in crate::sgb) const SGB_COMMAND_PCT_TRN: u8 = 0x14;
pub(in crate::sgb) const SGB_COMMAND_ATTR_TRN: u8 = 0x15;
pub(in crate::sgb) const SGB_COMMAND_ATTR_SET: u8 = 0x16;
pub(in crate::sgb) const SGB_COMMAND_MASK_EN: u8 = 0x17;
pub(in crate::sgb) const SGB_COMMAND_OBJ_TRN: u8 = 0x18;
pub(in crate::sgb) const SGB_COMMAND_MLT_REQ: u8 = 0x11;
pub(in crate::sgb) const SGB_COMMAND_JUMP: u8 = 0x12;
pub(in crate::sgb) const SGB_COMMAND_PAL_PRI: u8 = 0x19;
pub(in crate::sgb) const SGB_VRAM_TRANSFER_TOTAL_FRAMES: u8 = 5;
pub(in crate::sgb) const SGB_OBJ_TRN_BUSY_FRAMES: u8 = 1;
pub(in crate::sgb) const SGB_OBJ_OAM_SOURCE_OFFSET: usize = 0x0F90;

pub(in crate::sgb) const fn direct_palette_command_pair(command_id: u8) -> Option<(usize, usize)> {
    match command_id {
        SGB_COMMAND_PAL01 => Some((0, 1)),
        SGB_COMMAND_PAL23 => Some((2, 3)),
        SGB_COMMAND_PAL03 => Some((0, 3)),
        SGB_COMMAND_PAL12 => Some((1, 2)),
        _ => None,
    }
}
