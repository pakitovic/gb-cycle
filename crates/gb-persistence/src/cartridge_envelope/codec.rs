use crate::backend::CartridgeSaveBackendError;
use crate::format::MBC2_RAM_NIBBLE_COUNT;
use crate::wire::{ByteCursor, write_bool, write_u16, write_u32_checked};
use gb_core::{
    CartridgePersistenceProfile, CartridgeRamPayloadKind, Huc3RtcPersistentState,
    Mbc3RtcPersistentState, PersistentCartState,
};

pub(super) const RAM_KIND_LINEAR_TAG: u8 = 0;
pub(super) const RAM_KIND_MBC2_TAG: u8 = 1;
pub(super) const PROFILE_NONE_TAG: u8 = 0;
pub(super) const PROFILE_NON_PERSISTENT_RAM_TAG: u8 = 1;
pub(super) const PROFILE_PERSISTENT_RAM_TAG: u8 = 2;
pub(super) const PROFILE_PERSISTENT_RTC_TAG: u8 = 3;
pub(super) const PROFILE_PERSISTENT_RAM_AND_RTC_TAG: u8 = 4;
pub(super) const PROFILE_PERSISTENT_RAM_AND_FLASH_TAG: u8 = 5;
pub(super) const PROFILE_PERSISTENT_EEPROM_TAG: u8 = 6;
pub(super) const STATE_NONE_TAG: u8 = 0;
pub(super) const STATE_NO_MBC_RAM_TAG: u8 = 1;
pub(super) const STATE_MBC1_RAM_TAG: u8 = 2;
pub(super) const STATE_MBC2_RAM_TAG: u8 = 3;
pub(super) const STATE_MBC3_RTC_TAG: u8 = 4;
pub(super) const STATE_MBC3_RAM_TAG: u8 = 5;
pub(super) const STATE_MBC3_RAM_RTC_TAG: u8 = 6;
pub(super) const STATE_MBC5_RAM_TAG: u8 = 7;
pub(super) const STATE_MMM01_RAM_TAG: u8 = 8;
pub(super) const STATE_HUC1_RAM_TAG: u8 = 9;
pub(super) const STATE_HUC3_TAG: u8 = 10;
pub(super) const STATE_POCKET_CAMERA_RAM_TAG: u8 = 11;
pub(super) const STATE_MBC6_TAG: u8 = 12;
pub(super) const STATE_MBC7_EEPROM_TAG: u8 = 13;

pub(crate) fn persistent_state_kind_name(state: &PersistentCartState) -> &'static str {
    match state {
        PersistentCartState::None => "None",
        PersistentCartState::NoMbcRam { .. } => "NoMbcRam",
        PersistentCartState::Mmm01Ram { .. } => "Mmm01Ram",
        PersistentCartState::Huc1Ram { .. } => "Huc1Ram",
        PersistentCartState::Huc3 { .. } => "Huc3",
        PersistentCartState::Mbc1Ram { .. } => "Mbc1Ram",
        PersistentCartState::Mbc2Ram { .. } => "Mbc2Ram",
        PersistentCartState::Mbc3Rtc { .. } => "Mbc3Rtc",
        PersistentCartState::Mbc3Ram { .. } => "Mbc3Ram",
        PersistentCartState::Mbc3RamRtc { .. } => "Mbc3RamRtc",
        PersistentCartState::Mbc5Ram { .. } => "Mbc5Ram",
        PersistentCartState::Mbc6 { .. } => "Mbc6",
        PersistentCartState::Mbc7Eeprom { .. } => "Mbc7Eeprom",
        PersistentCartState::PocketCameraRam { .. } => "PocketCameraRam",
    }
}

pub(super) fn encode_persistence_profile(
    bytes: &mut Vec<u8>,
    profile: CartridgePersistenceProfile,
) -> Result<(), CartridgeSaveBackendError> {
    match profile {
        CartridgePersistenceProfile::None => bytes.push(PROFILE_NONE_TAG),
        CartridgePersistenceProfile::NonPersistentRam { ram } => {
            bytes.push(PROFILE_NON_PERSISTENT_RAM_TAG);
            encode_ram_payload_kind(bytes, ram)?;
        }
        CartridgePersistenceProfile::PersistentRam { ram } => {
            bytes.push(PROFILE_PERSISTENT_RAM_TAG);
            encode_ram_payload_kind(bytes, ram)?;
        }
        CartridgePersistenceProfile::PersistentRtc => bytes.push(PROFILE_PERSISTENT_RTC_TAG),
        CartridgePersistenceProfile::PersistentRamAndRtc { ram } => {
            bytes.push(PROFILE_PERSISTENT_RAM_AND_RTC_TAG);
            encode_ram_payload_kind(bytes, ram)?;
        }
        CartridgePersistenceProfile::PersistentRamAndFlash {
            ram,
            flash_byte_len,
            hidden_byte_len,
        } => {
            bytes.push(PROFILE_PERSISTENT_RAM_AND_FLASH_TAG);
            encode_ram_payload_kind(bytes, ram)?;
            write_u32_checked(bytes, flash_byte_len, "MBC6 flash byte_len")?;
            write_u32_checked(bytes, hidden_byte_len, "MBC6 hidden flash byte_len")?;
        }
        CartridgePersistenceProfile::PersistentEeprom { byte_len } => {
            bytes.push(PROFILE_PERSISTENT_EEPROM_TAG);
            write_u32_checked(bytes, byte_len, "persistent EEPROM byte_len")?;
        }
    }
    Ok(())
}

pub(super) fn decode_persistence_profile(
    cursor: &mut ByteCursor<'_>,
) -> Result<CartridgePersistenceProfile, CartridgeSaveBackendError> {
    let tag = cursor.read_u8()?;
    match tag {
        PROFILE_NONE_TAG => Ok(CartridgePersistenceProfile::None),
        PROFILE_NON_PERSISTENT_RAM_TAG => Ok(CartridgePersistenceProfile::NonPersistentRam {
            ram: decode_ram_payload_kind(cursor)?,
        }),
        PROFILE_PERSISTENT_RAM_TAG => Ok(CartridgePersistenceProfile::PersistentRam {
            ram: decode_ram_payload_kind(cursor)?,
        }),
        PROFILE_PERSISTENT_RTC_TAG => Ok(CartridgePersistenceProfile::PersistentRtc),
        PROFILE_PERSISTENT_RAM_AND_RTC_TAG => {
            Ok(CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: decode_ram_payload_kind(cursor)?,
            })
        }
        PROFILE_PERSISTENT_RAM_AND_FLASH_TAG => {
            Ok(CartridgePersistenceProfile::PersistentRamAndFlash {
                ram: decode_ram_payload_kind(cursor)?,
                flash_byte_len: cursor.read_u32()? as usize,
                hidden_byte_len: cursor.read_u32()? as usize,
            })
        }
        PROFILE_PERSISTENT_EEPROM_TAG => Ok(CartridgePersistenceProfile::PersistentEeprom {
            byte_len: cursor.read_u32()? as usize,
        }),
        _ => Err(CartridgeSaveBackendError::UnsupportedPersistenceProfileTag { tag }),
    }
}

pub(super) fn encode_ram_payload_kind(
    bytes: &mut Vec<u8>,
    kind: CartridgeRamPayloadKind,
) -> Result<(), CartridgeSaveBackendError> {
    match kind {
        CartridgeRamPayloadKind::Linear { byte_len } => {
            bytes.push(RAM_KIND_LINEAR_TAG);
            write_u32_checked(bytes, byte_len, "linear RAM byte_len")?;
        }
        CartridgeRamPayloadKind::Mbc2Nibbles { cell_count } => {
            bytes.push(RAM_KIND_MBC2_TAG);
            write_u32_checked(bytes, cell_count, "MBC2 RAM cell_count")?;
        }
    }
    Ok(())
}

pub(super) fn decode_ram_payload_kind(
    cursor: &mut ByteCursor<'_>,
) -> Result<CartridgeRamPayloadKind, CartridgeSaveBackendError> {
    let tag = cursor.read_u8()?;
    let len = cursor.read_u32()? as usize;
    match tag {
        RAM_KIND_LINEAR_TAG => Ok(CartridgeRamPayloadKind::Linear { byte_len: len }),
        RAM_KIND_MBC2_TAG => Ok(CartridgeRamPayloadKind::Mbc2Nibbles { cell_count: len }),
        _ => Err(CartridgeSaveBackendError::UnsupportedRamPayloadKindTag { tag }),
    }
}

pub(super) fn encode_persistent_state(
    bytes: &mut Vec<u8>,
    state: &PersistentCartState,
) -> Result<(), CartridgeSaveBackendError> {
    match state {
        PersistentCartState::None => bytes.push(STATE_NONE_TAG),
        PersistentCartState::NoMbcRam { ram } => {
            bytes.push(STATE_NO_MBC_RAM_TAG);
            encode_linear_ram(bytes, ram, "NoMBC RAM")?;
        }
        PersistentCartState::Mbc1Ram { ram } => {
            bytes.push(STATE_MBC1_RAM_TAG);
            encode_linear_ram(bytes, ram, "MBC1 RAM")?;
        }
        PersistentCartState::Mbc2Ram { ram_nibbles } => {
            bytes.push(STATE_MBC2_RAM_TAG);
            write_u32_checked(bytes, ram_nibbles.len(), "MBC2 RAM nibble count")?;
            for (index, value) in ram_nibbles.iter().copied().enumerate() {
                if value > 0x0F {
                    return Err(CartridgeSaveBackendError::InvalidMbc2NibbleValue { index, value });
                }
                bytes.push(value);
            }
        }
        PersistentCartState::Mbc3Rtc { rtc } => {
            bytes.push(STATE_MBC3_RTC_TAG);
            encode_rtc(bytes, *rtc);
        }
        PersistentCartState::Mbc3Ram { ram } => {
            bytes.push(STATE_MBC3_RAM_TAG);
            encode_linear_ram(bytes, ram, "MBC3 RAM")?;
        }
        PersistentCartState::Mbc3RamRtc { ram, rtc } => {
            bytes.push(STATE_MBC3_RAM_RTC_TAG);
            encode_linear_ram(bytes, ram, "MBC3 RAM")?;
            encode_rtc(bytes, *rtc);
        }
        PersistentCartState::Mbc5Ram { ram } => {
            bytes.push(STATE_MBC5_RAM_TAG);
            encode_linear_ram(bytes, ram, "MBC5 RAM")?;
        }
        PersistentCartState::Mbc6 {
            ram,
            flash,
            hidden_region,
            sector0_protected,
        } => {
            bytes.push(STATE_MBC6_TAG);
            encode_linear_ram(bytes, ram, "MBC6 RAM")?;
            encode_linear_ram(bytes, flash, "MBC6 flash")?;
            encode_linear_ram(bytes, hidden_region, "MBC6 hidden flash")?;
            write_bool(bytes, *sector0_protected);
        }
        PersistentCartState::Mmm01Ram { ram } => {
            bytes.push(STATE_MMM01_RAM_TAG);
            encode_linear_ram(bytes, ram, "MMM01 RAM")?;
        }
        PersistentCartState::Huc1Ram { ram } => {
            bytes.push(STATE_HUC1_RAM_TAG);
            encode_linear_ram(bytes, ram, "HuC1 RAM")?;
        }
        PersistentCartState::Huc3 {
            ram,
            mcu_ram,
            rtc,
            rom_bank,
            ram_bank,
            select_mode,
            access_address,
            mailbox_command,
            mailbox_argument,
            last_response_nybble,
            semaphore_ready,
            ir_emitter_on,
            ir_light_detected,
            last_control_write,
            last_unsupported_command,
            last_unsupported_argument,
        } => {
            bytes.push(STATE_HUC3_TAG);
            encode_linear_ram(bytes, ram, "HuC-3 RAM")?;
            write_u32_checked(bytes, mcu_ram.len(), "HuC-3 MCU RAM nibble count")?;
            for (index, value) in mcu_ram.iter().copied().enumerate() {
                if value > 0x0F {
                    return Err(CartridgeSaveBackendError::InvalidHuc3NibbleValue { index, value });
                }
                bytes.push(value);
            }
            encode_huc3_rtc(bytes, *rtc);
            bytes.push(*rom_bank);
            bytes.push(*ram_bank);
            bytes.push(*select_mode);
            bytes.push(*access_address);
            bytes.push(*mailbox_command);
            bytes.push(*mailbox_argument);
            bytes.push(*last_response_nybble);
            write_bool(bytes, *semaphore_ready);
            write_bool(bytes, *ir_emitter_on);
            write_bool(bytes, *ir_light_detected);
            encode_optional_u8(bytes, *last_control_write);
            encode_optional_u8(bytes, *last_unsupported_command);
            encode_optional_u8(bytes, *last_unsupported_argument);
        }
        PersistentCartState::PocketCameraRam { ram } => {
            bytes.push(STATE_POCKET_CAMERA_RAM_TAG);
            encode_linear_ram(bytes, ram, "Pocket Camera RAM")?;
        }
        PersistentCartState::Mbc7Eeprom { eeprom } => {
            bytes.push(STATE_MBC7_EEPROM_TAG);
            encode_linear_ram(bytes, eeprom, "MBC7 EEPROM")?;
        }
    }
    Ok(())
}

pub(super) fn decode_persistent_state(
    cursor: &mut ByteCursor<'_>,
) -> Result<PersistentCartState, CartridgeSaveBackendError> {
    let tag = cursor.read_u8()?;
    match tag {
        STATE_NONE_TAG => Ok(PersistentCartState::None),
        STATE_NO_MBC_RAM_TAG => Ok(PersistentCartState::NoMbcRam {
            ram: decode_linear_ram(cursor)?,
        }),
        STATE_MBC1_RAM_TAG => Ok(PersistentCartState::Mbc1Ram {
            ram: decode_linear_ram(cursor)?,
        }),
        STATE_MBC2_RAM_TAG => {
            let cell_count = cursor.read_u32()? as usize;
            let nibble_bytes = cursor.read_vec(cell_count)?;
            let mut ram_nibbles = [0; MBC2_RAM_NIBBLE_COUNT];
            if cell_count != ram_nibbles.len() {
                return Err(CartridgeSaveBackendError::LengthOverflow {
                    field: "decoded MBC2 RAM nibble count",
                    value: cell_count,
                });
            }
            for (index, value) in nibble_bytes.into_iter().enumerate() {
                if value > 0x0F {
                    return Err(CartridgeSaveBackendError::InvalidMbc2NibbleValue { index, value });
                }
                ram_nibbles[index] = value;
            }
            Ok(PersistentCartState::Mbc2Ram { ram_nibbles })
        }
        STATE_MBC3_RTC_TAG => Ok(PersistentCartState::Mbc3Rtc {
            rtc: decode_rtc(cursor)?,
        }),
        STATE_MBC3_RAM_TAG => Ok(PersistentCartState::Mbc3Ram {
            ram: decode_linear_ram(cursor)?,
        }),
        STATE_MBC3_RAM_RTC_TAG => Ok(PersistentCartState::Mbc3RamRtc {
            ram: decode_linear_ram(cursor)?,
            rtc: decode_rtc(cursor)?,
        }),
        STATE_MBC5_RAM_TAG => Ok(PersistentCartState::Mbc5Ram {
            ram: decode_linear_ram(cursor)?,
        }),
        STATE_MBC6_TAG => Ok(PersistentCartState::Mbc6 {
            ram: decode_linear_ram(cursor)?,
            flash: decode_linear_ram(cursor)?,
            hidden_region: decode_linear_ram(cursor)?,
            sector0_protected: cursor.read_bool("mbc6.sector0_protected")?,
        }),
        STATE_MMM01_RAM_TAG => Ok(PersistentCartState::Mmm01Ram {
            ram: decode_linear_ram(cursor)?,
        }),
        STATE_HUC1_RAM_TAG => Ok(PersistentCartState::Huc1Ram {
            ram: decode_linear_ram(cursor)?,
        }),
        STATE_HUC3_TAG => {
            let ram = decode_linear_ram(cursor)?;
            let nibble_count = cursor.read_u32()? as usize;
            if nibble_count != 256 {
                return Err(CartridgeSaveBackendError::LengthOverflow {
                    field: "decoded HuC-3 MCU RAM nibble count",
                    value: nibble_count,
                });
            }
            let nibble_bytes = cursor.read_vec(nibble_count)?;
            let mut mcu_ram = [0; 256];
            for (index, value) in nibble_bytes.into_iter().enumerate() {
                if value > 0x0F {
                    return Err(CartridgeSaveBackendError::InvalidHuc3NibbleValue { index, value });
                }
                mcu_ram[index] = value;
            }
            Ok(PersistentCartState::Huc3 {
                ram,
                mcu_ram,
                rtc: decode_huc3_rtc(cursor)?,
                rom_bank: cursor.read_u8()?,
                ram_bank: cursor.read_u8()?,
                select_mode: cursor.read_u8()?,
                access_address: cursor.read_u8()?,
                mailbox_command: cursor.read_u8()?,
                mailbox_argument: cursor.read_u8()?,
                last_response_nybble: cursor.read_u8()?,
                semaphore_ready: cursor.read_bool("huc3.semaphore_ready")?,
                ir_emitter_on: cursor.read_bool("huc3.ir_emitter_on")?,
                ir_light_detected: cursor.read_bool("huc3.ir_light_detected")?,
                last_control_write: decode_optional_u8(cursor, "huc3.last_control_write")?,
                last_unsupported_command: decode_optional_u8(
                    cursor,
                    "huc3.last_unsupported_command",
                )?,
                last_unsupported_argument: decode_optional_u8(
                    cursor,
                    "huc3.last_unsupported_argument",
                )?,
            })
        }
        STATE_POCKET_CAMERA_RAM_TAG => Ok(PersistentCartState::PocketCameraRam {
            ram: decode_linear_ram(cursor)?,
        }),
        STATE_MBC7_EEPROM_TAG => Ok(PersistentCartState::Mbc7Eeprom {
            eeprom: decode_linear_ram(cursor)?,
        }),
        _ => Err(CartridgeSaveBackendError::UnsupportedPersistentStateTag { tag }),
    }
}

pub(super) fn encode_linear_ram(
    bytes: &mut Vec<u8>,
    ram: &[u8],
    field: &'static str,
) -> Result<(), CartridgeSaveBackendError> {
    write_u32_checked(bytes, ram.len(), field)?;
    bytes.extend_from_slice(ram);
    Ok(())
}

pub(super) fn decode_linear_ram(
    cursor: &mut ByteCursor<'_>,
) -> Result<Vec<u8>, CartridgeSaveBackendError> {
    let len = cursor.read_u32()? as usize;
    cursor.read_vec(len)
}

pub(super) fn encode_rtc(bytes: &mut Vec<u8>, rtc: Mbc3RtcPersistentState) {
    bytes.push(rtc.seconds);
    bytes.push(rtc.minutes);
    bytes.push(rtc.hours);
    write_u16(bytes, rtc.day_counter);
    write_bool(bytes, rtc.halt);
    write_bool(bytes, rtc.carry);
}

pub(super) fn decode_rtc(
    cursor: &mut ByteCursor<'_>,
) -> Result<Mbc3RtcPersistentState, CartridgeSaveBackendError> {
    Ok(Mbc3RtcPersistentState {
        seconds: cursor.read_u8()?,
        minutes: cursor.read_u8()?,
        hours: cursor.read_u8()?,
        day_counter: cursor.read_u16()?,
        halt: cursor.read_bool("rtc.halt")?,
        carry: cursor.read_bool("rtc.carry")?,
    })
}

pub(super) fn encode_huc3_rtc(bytes: &mut Vec<u8>, rtc: Huc3RtcPersistentState) {
    write_u16(bytes, rtc.current_minutes_of_day);
    write_u16(bytes, rtc.current_days);
    bytes.push(rtc.current_subminute_seconds);
    write_u16(bytes, rtc.event_minutes_of_day);
    write_u16(bytes, rtc.event_days);
}

pub(super) fn decode_huc3_rtc(
    cursor: &mut ByteCursor<'_>,
) -> Result<Huc3RtcPersistentState, CartridgeSaveBackendError> {
    Ok(Huc3RtcPersistentState {
        current_minutes_of_day: cursor.read_u16()?,
        current_days: cursor.read_u16()?,
        current_subminute_seconds: cursor.read_u8()?,
        event_minutes_of_day: cursor.read_u16()?,
        event_days: cursor.read_u16()?,
    })
}

pub(super) fn encode_optional_u8(bytes: &mut Vec<u8>, value: Option<u8>) {
    write_bool(bytes, value.is_some());
    bytes.push(value.unwrap_or(0));
}

pub(super) fn decode_optional_u8(
    cursor: &mut ByteCursor<'_>,
    field: &'static str,
) -> Result<Option<u8>, CartridgeSaveBackendError> {
    let present = cursor.read_bool(field)?;
    let value = cursor.read_u8()?;
    Ok(present.then_some(value))
}
