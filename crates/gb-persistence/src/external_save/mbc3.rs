use super::{ExternalSaveError, ExternalSaveLengthExpectation};
use crate::format::{MBC3_EXTERNAL_RTC_SUFFIX_LEN, MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP};
use crate::wire::{write_u32, write_u64};
use gb_core::Mbc3RtcPersistentState;

pub(super) fn encode_external_mbc3_rtc_suffix(
    bytes: &mut Vec<u8>,
    rtc: Mbc3RtcPersistentState,
    current_unix_seconds: u64,
) {
    let day_low = (rtc.day_counter & 0x00FF) as u8;
    let day_high =
        ((rtc.day_counter >> 8) as u8 & 0x01) | ((rtc.halt as u8) << 6) | ((rtc.carry as u8) << 7);
    let fields = [rtc.seconds, rtc.minutes, rtc.hours, day_low, day_high];

    for field in fields {
        write_u32(bytes, u32::from(field));
    }
    for field in fields {
        write_u32(bytes, u32::from(field));
    }
    write_u64(bytes, current_unix_seconds);
}

pub(super) fn decode_external_mbc3_rtc_suffix(
    bytes: &[u8],
    current_unix_seconds: u64,
) -> Result<Mbc3RtcPersistentState, ExternalSaveError> {
    if !is_external_mbc3_rtc_suffix_len(bytes.len()) {
        return Err(ExternalSaveError::InvalidLength {
            context: "MBC3 RTC",
            expected: mbc3_external_rtc_suffix_length_expectation(),
            actual: bytes.len(),
        });
    }

    let seconds = read_external_u32_low_u8(bytes, 0) & 0x3F;
    let minutes = read_external_u32_low_u8(bytes, 4) & 0x3F;
    let hours = read_external_u32_low_u8(bytes, 8) & 0x1F;
    let day_low = read_external_u32_low_u8(bytes, 12);
    let day_high = read_external_u32_low_u8(bytes, 16);
    let saved_unix_seconds = match bytes.len() {
        MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP => {
            u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]) as u64
        }
        MBC3_EXTERNAL_RTC_SUFFIX_LEN => u64::from_le_bytes([
            bytes[40], bytes[41], bytes[42], bytes[43], bytes[44], bytes[45], bytes[46], bytes[47],
        ]),
        _ => unreachable!("MBC3 RTC suffix length should be validated before timestamp decode"),
    };

    let mut rtc = Mbc3RtcPersistentState {
        seconds,
        minutes,
        hours,
        day_counter: u16::from(day_low) | (u16::from(day_high & 0x01) << 8),
        halt: day_high & 0x40 != 0,
        carry: day_high & 0x80 != 0,
    };
    rtc.apply_elapsed_seconds(current_unix_seconds.saturating_sub(saved_unix_seconds));
    Ok(rtc)
}

pub(super) fn is_external_mbc3_rtc_suffix_len(len: usize) -> bool {
    matches!(
        len,
        MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP | MBC3_EXTERNAL_RTC_SUFFIX_LEN
    )
}

pub(super) fn mbc3_external_rtc_suffix_length_expectation() -> ExternalSaveLengthExpectation {
    ExternalSaveLengthExpectation::Either {
        first: MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP,
        second: MBC3_EXTERNAL_RTC_SUFFIX_LEN,
    }
}

pub(super) fn read_external_u32_low_u8(bytes: &[u8], offset: usize) -> u8 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]) as u8
}
