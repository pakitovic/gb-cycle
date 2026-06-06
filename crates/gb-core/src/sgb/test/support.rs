pub(super) use super::super::host::*;
pub(super) use super::super::protocol::*;
use super::super::shell;

pub(super) use crate::cartridge::{CartridgeHeader, SgbFlag};
pub(super) use crate::joypad::JoypadButton;
pub(super) use crate::model::{HostPlatform, SgbHostProfile, SgbVideoStandard, StartupMode};

pub(super) fn test_header(sgb_flag: SgbFlag, old_licensee_code: u8) -> CartridgeHeader {
    test_header_with_title(b"SGBTEST", sgb_flag, old_licensee_code)
}

pub(super) fn test_header_with_title(
    title: &[u8],
    sgb_flag: SgbFlag,
    old_licensee_code: u8,
) -> CartridgeHeader {
    assert!(title.len() <= 16);
    let mut title_bytes = [0; 16];
    title_bytes[..title.len()].copy_from_slice(title);
    CartridgeHeader {
        entry_point: [0; 4],
        nintendo_logo: [0; 48],
        title_bytes,
        raw_title_suffix_or_manufacturer_code: [0; 4],
        title: String::from_utf8_lossy(title).to_string(),
        cgb_flag: crate::CgbFlag::None,
        sgb_flag,
        cartridge_type: 0,
        rom_size: crate::RomSizeInfo::decode(0x00),
        ram_size: crate::RamSizeInfo::decode(0x00),
        new_licensee_code: *b"01",
        destination_code: 0,
        old_licensee_code,
        header_checksum: 0,
    }
}

pub(super) fn sgb_command_packet(command_id: u8, packet_count: u8) -> [u8; SGB_PACKET_BYTES] {
    let mut bytes = [0; SGB_PACKET_BYTES];
    bytes[0] = (command_id << 3) | packet_count;
    bytes
}

pub(super) fn write_packet_color(bytes: &mut [u8; SGB_PACKET_BYTES], offset: usize, rgb555: u16) {
    let [low, high] = rgb555.to_le_bytes();
    bytes[offset] = low;
    bytes[offset + 1] = high;
}

pub(super) fn sgb_pal01_packet() -> [u8; SGB_PACKET_BYTES] {
    let mut bytes = sgb_command_packet(SGB_COMMAND_PAL01, 1);
    write_packet_color(&mut bytes, 1, 0x001F);
    write_packet_color(&mut bytes, 3, 0x03E0);
    write_packet_color(&mut bytes, 5, 0x7C00);
    write_packet_color(&mut bytes, 7, 0x4210);
    write_packet_color(&mut bytes, 9, 0x0001);
    write_packet_color(&mut bytes, 11, 0x0002);
    write_packet_color(&mut bytes, 13, 0x8003);
    bytes
}

pub(super) fn sgb_chr_trn_packet(destination: u8) -> [u8; SGB_PACKET_BYTES] {
    let mut bytes = sgb_command_packet(SGB_COMMAND_CHR_TRN, 1);
    bytes[1] = destination;
    bytes
}

pub(super) fn sgb_pct_trn_packet() -> [u8; SGB_PACKET_BYTES] {
    sgb_command_packet(SGB_COMMAND_PCT_TRN, 1)
}

pub(super) fn sgb_mask_packet(mask: SgbScreenMask) -> [u8; SGB_PACKET_BYTES] {
    let mut bytes = sgb_command_packet(SGB_COMMAND_MASK_EN, 1);
    bytes[1] = match mask {
        SgbScreenMask::Cancel => 0,
        SgbScreenMask::Freeze => 1,
        SgbScreenMask::BlankBlack => 2,
        SgbScreenMask::BlankColor0 => 3,
    };
    bytes
}

pub(super) fn sgb_pal_trn_packet() -> [u8; SGB_PACKET_BYTES] {
    sgb_command_packet(SGB_COMMAND_PAL_TRN, 1)
}

pub(super) fn sgb_attr_trn_packet() -> [u8; SGB_PACKET_BYTES] {
    sgb_command_packet(SGB_COMMAND_ATTR_TRN, 1)
}

pub(super) fn sgb_mlt_req_packet(control: u8) -> [u8; SGB_PACKET_BYTES] {
    let mut bytes = sgb_command_packet(SGB_COMMAND_MLT_REQ, 1);
    bytes[1] = control;
    bytes
}

pub(super) fn sgb_atrc_en_packet(disable: bool) -> [u8; SGB_PACKET_BYTES] {
    let mut bytes = sgb_command_packet(SGB_COMMAND_ATRC_EN, 1);
    bytes[1] = u8::from(disable);
    bytes
}

pub(super) fn sgb_test_en_packet(enable: bool) -> [u8; SGB_PACKET_BYTES] {
    let mut bytes = sgb_command_packet(SGB_COMMAND_TEST_EN, 1);
    bytes[1] = u8::from(enable);
    bytes
}

pub(super) fn sgb_icon_en_packet(disable_bits: u8) -> [u8; SGB_PACKET_BYTES] {
    let mut bytes = sgb_command_packet(SGB_COMMAND_ICON_EN, 1);
    bytes[1] = disable_bits;
    bytes
}

pub(super) fn sgb_obj_trn_packet(control: u8, palette_ids: [u16; 4]) -> [u8; SGB_PACKET_BYTES] {
    let mut bytes = sgb_command_packet(SGB_COMMAND_OBJ_TRN, 1);
    bytes[1] = control;
    for (palette_index, palette_id) in palette_ids.into_iter().enumerate() {
        let [low, high] = palette_id.to_le_bytes();
        bytes[2 + palette_index * 2] = low;
        bytes[3 + palette_index * 2] = high;
    }
    bytes
}

pub(super) fn sgb_sound_packet() -> [u8; SGB_PACKET_BYTES] {
    let mut bytes = sgb_command_packet(SGB_COMMAND_SOUND, 1);
    bytes[1] = 0x17;
    bytes[2] = 0x24;
    bytes[3] = 0b10_01_11_00;
    bytes[4] = 0x05;
    bytes
}

pub(super) fn sgb_sou_trn_packet() -> [u8; SGB_PACKET_BYTES] {
    sgb_command_packet(SGB_COMMAND_SOU_TRN, 1)
}

pub(super) fn sgb_data_snd_packet() -> [u8; SGB_PACKET_BYTES] {
    let mut bytes = sgb_command_packet(SGB_COMMAND_DATA_SND, 1);
    bytes[1] = 0x00;
    bytes[2] = 0x21;
    bytes[3] = 0x7E;
    bytes[4] = 3;
    bytes[5] = 0xAA;
    bytes[6] = 0xBB;
    bytes[7] = 0xCC;
    bytes
}

pub(super) fn sgb_data_trn_packet() -> [u8; SGB_PACKET_BYTES] {
    let mut bytes = sgb_command_packet(SGB_COMMAND_DATA_TRN, 1);
    bytes[1] = 0x00;
    bytes[2] = 0x22;
    bytes[3] = 0x7E;
    bytes
}

pub(super) fn write_transfer_screen_tile(
    vram: &mut [u8],
    transfer_tile_index: usize,
    tile_index: u8,
) {
    let tile_x = transfer_tile_index % SGB_TRANSFER_DISPLAY_TILE_COLUMNS;
    let tile_y = transfer_tile_index / SGB_TRANSFER_DISPLAY_TILE_COLUMNS;
    vram[SGB_GB_BG_MAP_9800_OFFSET + tile_y * SGB_GB_TILEMAP_WIDTH + tile_x] = tile_index;
}

pub(super) fn sgb_jump_packet() -> [u8; SGB_PACKET_BYTES] {
    let mut bytes = sgb_command_packet(SGB_COMMAND_JUMP, 1);
    bytes[1] = 0x34;
    bytes[2] = 0x12;
    bytes[3] = 0x7E;
    bytes[4] = 0x78;
    bytes[5] = 0x56;
    bytes[6] = 0x7E;
    bytes
}

pub(super) fn write_system_palette_color(
    bytes: &mut [u8; SGB_VRAM_TRANSFER_BYTES],
    palette_index: usize,
    color_index: usize,
    rgb555: u16,
) {
    let [low, high] = rgb555.to_le_bytes();
    let offset = palette_index * SGB_SCREEN_PALETTE_COLORS * 2 + color_index * 2;
    bytes[offset] = low;
    bytes[offset + 1] = high;
}

pub(super) fn write_atf_cell(
    bytes: &mut [u8; SGB_VRAM_TRANSFER_BYTES],
    atf_index: usize,
    cell_x: usize,
    cell_y: usize,
    palette_index: u8,
) {
    let offset = atf_index * SGB_ATF_BYTES + cell_y * 5 + cell_x / 4;
    let shift = 6 - (cell_x % 4) * 2;
    bytes[offset] &= !(0x03 << shift);
    bytes[offset] |= (palette_index & 0x03) << shift;
}

pub(super) fn write_border_map_entry(
    bytes: &mut [u8; SGB_VRAM_TRANSFER_BYTES],
    entry: usize,
    raw: u16,
) {
    let [low, high] = raw.to_le_bytes();
    bytes[entry * 2] = low;
    bytes[entry * 2 + 1] = high;
}

pub(super) fn write_border_palette_color(
    bytes: &mut [u8; SGB_VRAM_TRANSFER_BYTES],
    palette_index: usize,
    color_index: usize,
    rgb555: u16,
) {
    let [low, high] = rgb555.to_le_bytes();
    let offset = 0x800 + palette_index * SGB_BORDER_PALETTE_COLORS * 2 + color_index * 2;
    bytes[offset] = low;
    bytes[offset + 1] = high;
}

pub(super) fn solid_tile_color_1_transfer() -> [u8; SGB_VRAM_TRANSFER_BYTES] {
    let mut bytes = [0; SGB_VRAM_TRANSFER_BYTES];
    for row in 0..8 {
        bytes[row * 2] = 0xFF;
    }
    bytes
}

pub(super) fn write_joyp_idle(host: &mut SgbHost) {
    host.observe_joyp_write(SGB_JOYP_IDLE_BITS);
}

pub(super) fn write_joyp_start(host: &mut SgbHost) {
    host.observe_joyp_write(SGB_JOYP_START_BITS);
    write_joyp_idle(host);
}

pub(super) fn write_joyp_data_bit(host: &mut SgbHost, bit: u8) {
    host.observe_joyp_write(if bit == 0 {
        SGB_JOYP_ZERO_BITS
    } else {
        SGB_JOYP_ONE_BITS
    });
    write_joyp_idle(host);
}

pub(super) fn write_joyp_packet(host: &mut SgbHost, bytes: [u8; SGB_PACKET_BYTES]) {
    write_joyp_start(host);
    for byte in bytes {
        for bit_index in 0..8 {
            write_joyp_data_bit(host, (byte >> bit_index) & 0x01);
        }
    }
    write_joyp_data_bit(host, 0);
}

pub(super) fn write_joyp_line(host: &mut SgbHost, line: SgbJoypLineState) {
    host.observe_joyp_write(match line {
        SgbJoypLineState::Idle => SGB_JOYP_IDLE_BITS,
        SgbJoypLineState::Start => SGB_JOYP_START_BITS,
        SgbJoypLineState::Zero => SGB_JOYP_ZERO_BITS,
        SgbJoypLineState::One => SGB_JOYP_ONE_BITS,
        SgbJoypLineState::Invalid => unreachable!("all masked JOYP states are explicit"),
    });
}

pub(super) fn write_sgb_ext_test_start(host: &mut SgbHost) {
    write_joyp_line(host, SgbJoypLineState::Start);
    write_joyp_line(host, SgbJoypLineState::Idle);
}

pub(super) fn write_sgb_ext_test_bit(host: &mut SgbHost, bit: u8) {
    write_joyp_line(
        host,
        if bit == 0 {
            SgbJoypLineState::Zero
        } else {
            SgbJoypLineState::One
        },
    );
    write_joyp_line(host, SgbJoypLineState::Idle);
}

pub(super) fn write_sgb_ext_test_packet_basic(host: &mut SgbHost, bytes: [u8; SGB_PACKET_BYTES]) {
    write_sgb_ext_test_start(host);
    for byte in bytes {
        for bit_index in 0..8 {
            write_sgb_ext_test_bit(host, (byte >> bit_index) & 0x01);
        }
    }
    write_sgb_ext_test_bit(host, 0);
}

pub(super) fn write_sgb_ext_test_packet_corrupt_stop(
    host: &mut SgbHost,
    bytes: [u8; SGB_PACKET_BYTES],
) {
    write_sgb_ext_test_start(host);
    for byte in bytes {
        for bit_index in 0..8 {
            write_sgb_ext_test_bit(host, (byte >> bit_index) & 0x01);
        }
    }
    write_sgb_ext_test_bit(host, 1);
}

pub(super) fn write_sgb_ext_test_packet_avoid_30(
    host: &mut SgbHost,
    bytes: [u8; SGB_PACKET_BYTES],
) {
    write_joyp_line(host, SgbJoypLineState::Idle);
    write_joyp_line(host, SgbJoypLineState::Start);
    for byte in bytes {
        for bit_index in 0..8 {
            write_joyp_line(
                host,
                if (byte >> bit_index) & 0x01 == 0 {
                    SgbJoypLineState::One
                } else {
                    SgbJoypLineState::Zero
                },
            );
            write_joyp_line(host, SgbJoypLineState::Start);
        }
    }
    write_joyp_line(host, SgbJoypLineState::One);
    write_joyp_line(host, SgbJoypLineState::Start);
    write_joyp_line(host, SgbJoypLineState::Idle);
}

pub(super) fn write_sgb_ext_test_packet_with_second_byte_bit_transition(
    host: &mut SgbHost,
    bytes: [u8; SGB_PACKET_BYTES],
    transition: &[SgbJoypLineState],
) {
    write_sgb_ext_test_start(host);
    for bit_index in 0..8 {
        write_sgb_ext_test_bit(host, (bytes[0] >> bit_index) & 0x01);
    }
    for &line in transition {
        write_joyp_line(host, line);
    }
    for bit_index in 1..8 {
        write_sgb_ext_test_bit(host, (bytes[1] >> bit_index) & 0x01);
    }
    for byte in &bytes[2..] {
        for bit_index in 0..8 {
            write_sgb_ext_test_bit(host, (byte >> bit_index) & 0x01);
        }
    }
    write_sgb_ext_test_bit(host, 0);
}

pub(super) fn write_sgb_ext_test_packet_short_start(
    host: &mut SgbHost,
    bytes: [u8; SGB_PACKET_BYTES],
) {
    write_joyp_line(host, SgbJoypLineState::Start);
    for byte in bytes {
        for bit_index in 0..8 {
            write_sgb_ext_test_bit(host, (byte >> bit_index) & 0x01);
        }
    }
    write_sgb_ext_test_bit(host, 0);
}

pub(super) fn sgb_ext_test_player_count(host: &mut SgbHost) -> u8 {
    for _ in 0..SGB_CONTROLLER_COUNT + 1 {
        write_joyp_line(host, SgbJoypLineState::Start);
        write_joyp_line(host, SgbJoypLineState::Idle);
        if host.joyp_read_value(0xFF) & 0x0F == 0x0F {
            break;
        }
    }

    let mut count = 0_u8;
    for _ in 0..SGB_CONTROLLER_COUNT + 1 {
        count += 1;
        write_joyp_line(host, SgbJoypLineState::Start);
        write_joyp_line(host, SgbJoypLineState::Idle);
        if host.joyp_read_value(0xFF) & 0x0F == 0x0F {
            return count;
        }
    }
    count
}

pub(super) fn cycle_sgb_player(host: &mut SgbHost) {
    host.observe_joyp_write(SGB_JOYP_ONE_BITS);
    host.observe_joyp_write(SGB_JOYP_IDLE_BITS);
}

pub(super) fn sgb_player_id_value(host: &SgbHost) -> u8 {
    host.joyp_read_value(0xFF)
}

pub(super) fn accepted_sgb_host() -> SgbHost {
    let mut host = SgbHost::new(HostPlatform::Sgb);
    let header = test_header(SgbFlag::Supported, SGB_HEADER_OLD_LICENSEE_CODE_REQUIRED);
    host.apply_cartridge_header(Some(&header));
    host
}

pub(super) fn fallback_transfer_display() -> SgbVramTransferDisplayState {
    SgbVramTransferDisplayState::new(0x81, 0, 1, SGB_TRANSFER_REQUIRED_BGP)
}

pub(super) fn transfer_vram_from_payload(
    payload: &[u8; SGB_VRAM_TRANSFER_BYTES],
) -> [u8; SGB_GB_VRAM_BYTES] {
    let mut vram = [0; SGB_GB_VRAM_BYTES];
    vram[..SGB_VRAM_TRANSFER_BYTES].copy_from_slice(payload);
    vram
}

pub(super) fn finish_shell_border_transition(host: &mut SgbHost) {
    let vram = [0; SGB_GB_VRAM_BYTES];
    for _ in 0..usize::from(shell::SGB_SHELL_BORDER_FADE_FRAMES) * 2 {
        host.advance_frame_start(&vram, fallback_transfer_display())
            .expect("test-only shell border transition should advance deterministically");
    }
}

pub(super) fn signed_display_vram_from_payload(
    payload: &[u8; SGB_VRAM_TRANSFER_BYTES],
) -> [u8; SGB_GB_VRAM_BYTES] {
    let mut vram = [0xE7; SGB_GB_VRAM_BYTES];
    let lcdc = SGB_LCDC_ENABLE_BIT | SGB_LCDC_BG_ENABLE_BIT;
    for transfer_tile_index in 0..SGB_TRANSFER_DISPLAY_TILE_COUNT {
        let tile_index = 0x80u8.wrapping_add(transfer_tile_index as u8);
        write_transfer_screen_tile(&mut vram, transfer_tile_index, tile_index);
        let tile_offset = gb_tile_data_offset(lcdc, tile_index);
        let payload_offset = transfer_tile_index * SGB_GB_TILE_BYTES;
        vram[tile_offset..tile_offset + SGB_GB_TILE_BYTES]
            .copy_from_slice(&payload[payload_offset..payload_offset + SGB_GB_TILE_BYTES]);
    }
    vram
}

pub(super) fn frame_payload(frame_index: u8) -> [u8; SGB_VRAM_TRANSFER_BYTES] {
    let mut payload = [0; SGB_VRAM_TRANSFER_BYTES];
    for (byte_index, byte) in payload.iter_mut().enumerate() {
        *byte = frame_index
            .wrapping_mul(0x31)
            .wrapping_add(byte_index as u8);
    }
    payload
}
