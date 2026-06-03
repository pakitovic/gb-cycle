use super::linear::*;
use super::mbc2::*;
use super::mbc3::*;
use super::mbc6::*;
use super::{ExternalSaveError, ExternalSaveExportFormat, ExternalSaveLengthExpectation};
use crate::cartridge_envelope::{CartridgeSaveEnvelope, persistent_state_kind_name};
use crate::format::{MBC3_EXTERNAL_RTC_SUFFIX_LEN, MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP};
use crate::hardware::{
    apply_elapsed_off_session_seconds, uses_battery_backed_hardware_persistence,
};
use gb_core::{
    CartridgePersistenceMetadata, CartridgePersistenceProfile, CartridgeRamPayloadKind,
    PersistentCartState,
};

pub fn export_external_cartridge_save(
    envelope: &CartridgeSaveEnvelope,
    current_unix_seconds: u64,
) -> Result<Vec<u8>, ExternalSaveError> {
    let mut state = envelope.persistent_state.clone();
    let elapsed_off_session_seconds =
        current_unix_seconds.saturating_sub(envelope.backend_metadata.saved_at_unix_seconds);
    apply_elapsed_off_session_seconds(&mut state, elapsed_off_session_seconds);
    encode_external_cartridge_save(
        envelope.cartridge_metadata,
        &state,
        current_unix_seconds,
        ExternalSaveExportFormat::default(),
    )
}

pub fn encode_external_cartridge_save(
    metadata: CartridgePersistenceMetadata,
    state: &PersistentCartState,
    current_unix_seconds: u64,
    format: ExternalSaveExportFormat,
) -> Result<Vec<u8>, ExternalSaveError> {
    if !uses_battery_backed_hardware_persistence(metadata) {
        return Err(ExternalSaveError::UnsupportedPersistenceProfile {
            profile: metadata.profile,
        });
    }

    match (metadata.profile, state) {
        (
            CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
            },
            PersistentCartState::NoMbcRam { ram }
            | PersistentCartState::Mmm01Ram { ram }
            | PersistentCartState::Huc1Ram { ram }
            | PersistentCartState::Mbc1Ram { ram }
            | PersistentCartState::Mbc3Ram { ram }
            | PersistentCartState::Mbc5Ram { ram }
            | PersistentCartState::PocketCameraRam { ram },
        ) => encode_external_linear_ram(ram, byte_len),
        (
            CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles { cell_count },
            },
            PersistentCartState::Mbc2Ram { ram_nibbles },
        ) => encode_external_mbc2_ram(ram_nibbles, cell_count, format),
        (CartridgePersistenceProfile::PersistentRtc, PersistentCartState::Mbc3Rtc { rtc }) => {
            let mut bytes = Vec::with_capacity(MBC3_EXTERNAL_RTC_SUFFIX_LEN);
            encode_external_mbc3_rtc_suffix(&mut bytes, *rtc, current_unix_seconds);
            Ok(bytes)
        }
        (
            CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
            },
            PersistentCartState::Mbc3RamRtc { ram, rtc },
        ) => {
            let mut bytes = encode_external_linear_ram(ram, byte_len)?;
            encode_external_mbc3_rtc_suffix(&mut bytes, *rtc, current_unix_seconds);
            Ok(bytes)
        }
        (
            CartridgePersistenceProfile::PersistentRamAndFlash {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
                flash_byte_len,
                hidden_byte_len,
            },
            PersistentCartState::Mbc6 {
                ram,
                flash,
                hidden_region,
                sector0_protected,
            },
        ) => encode_external_mbc6_save(
            ram,
            flash,
            hidden_region,
            *sector0_protected,
            byte_len,
            flash_byte_len,
            hidden_byte_len,
        ),
        (
            CartridgePersistenceProfile::PersistentRamAndFlash {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles { .. },
                ..
            },
            _,
        ) => Err(ExternalSaveError::UnsupportedPersistenceProfile {
            profile: metadata.profile,
        }),
        (
            CartridgePersistenceProfile::PersistentEeprom { byte_len },
            PersistentCartState::Mbc7Eeprom { eeprom },
        ) => encode_external_linear_ram(eeprom, byte_len),
        (
            CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles { .. },
            },
            _,
        ) => Err(ExternalSaveError::UnsupportedPersistenceProfile {
            profile: metadata.profile,
        }),
        (
            CartridgePersistenceProfile::PersistentRamAndRtc { .. },
            PersistentCartState::Huc3 { .. },
        ) => Err(ExternalSaveError::UnsupportedPersistentState {
            state_kind: persistent_state_kind_name(state),
        }),
        (CartridgePersistenceProfile::PersistentRam { .. }, PersistentCartState::Huc3 { .. })
        | (CartridgePersistenceProfile::PersistentRtc, PersistentCartState::Huc3 { .. }) => {
            Err(ExternalSaveError::UnsupportedPersistentState {
                state_kind: persistent_state_kind_name(state),
            })
        }
        (profile, _) => Err(ExternalSaveError::StateProfileMismatch {
            state_kind: persistent_state_kind_name(state),
            profile,
        }),
    }
}

pub fn import_external_cartridge_save(
    metadata: CartridgePersistenceMetadata,
    target_state: &PersistentCartState,
    bytes: &[u8],
    current_unix_seconds: u64,
) -> Result<PersistentCartState, ExternalSaveError> {
    if !uses_battery_backed_hardware_persistence(metadata) {
        return Err(ExternalSaveError::UnsupportedPersistenceProfile {
            profile: metadata.profile,
        });
    }

    match (metadata.profile, target_state) {
        (
            CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
            },
            PersistentCartState::NoMbcRam { .. },
        ) => decode_external_linear_ram(bytes, byte_len, "linear RAM")
            .map(|ram| PersistentCartState::NoMbcRam { ram }),
        (
            CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
            },
            PersistentCartState::Mmm01Ram { .. },
        ) => decode_external_linear_ram(bytes, byte_len, "MMM01 RAM")
            .map(|ram| PersistentCartState::Mmm01Ram { ram }),
        (
            CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
            },
            PersistentCartState::Huc1Ram { .. },
        ) => decode_external_linear_ram(bytes, byte_len, "HuC1 RAM")
            .map(|ram| PersistentCartState::Huc1Ram { ram }),
        (
            CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
            },
            PersistentCartState::Mbc1Ram { .. },
        ) => decode_external_linear_ram(bytes, byte_len, "MBC1 RAM")
            .map(|ram| PersistentCartState::Mbc1Ram { ram }),
        (
            CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
            },
            PersistentCartState::Mbc3Ram { .. },
        ) => decode_external_linear_ram(bytes, byte_len, "MBC3 RAM")
            .map(|ram| PersistentCartState::Mbc3Ram { ram }),
        (
            CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
            },
            PersistentCartState::Mbc5Ram { .. },
        ) => decode_external_linear_ram(bytes, byte_len, "MBC5 RAM")
            .map(|ram| PersistentCartState::Mbc5Ram { ram }),
        (
            CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
            },
            PersistentCartState::PocketCameraRam { .. },
        ) => decode_external_linear_ram(bytes, byte_len, "Pocket Camera RAM")
            .map(|ram| PersistentCartState::PocketCameraRam { ram }),
        (
            CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles { cell_count },
            },
            PersistentCartState::Mbc2Ram { .. },
        ) => decode_external_mbc2_ram(bytes, cell_count)
            .map(|ram_nibbles| PersistentCartState::Mbc2Ram { ram_nibbles }),
        (CartridgePersistenceProfile::PersistentRtc, PersistentCartState::Mbc3Rtc { .. }) => {
            if !is_external_mbc3_rtc_suffix_len(bytes.len()) {
                return Err(ExternalSaveError::InvalidLength {
                    context: "MBC3 RTC",
                    expected: mbc3_external_rtc_suffix_length_expectation(),
                    actual: bytes.len(),
                });
            }
            let rtc = decode_external_mbc3_rtc_suffix(bytes, current_unix_seconds)?;
            Ok(PersistentCartState::Mbc3Rtc { rtc })
        }
        (
            CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
            },
            PersistentCartState::Mbc3RamRtc { .. },
        ) => {
            let expected_len_32bit_timestamp =
                byte_len + MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP;
            let expected_len = byte_len + MBC3_EXTERNAL_RTC_SUFFIX_LEN;
            if bytes.len() != expected_len_32bit_timestamp && bytes.len() != expected_len {
                return Err(ExternalSaveError::InvalidLength {
                    context: "MBC3 RAM+RTC",
                    expected: ExternalSaveLengthExpectation::Either {
                        first: expected_len_32bit_timestamp,
                        second: expected_len,
                    },
                    actual: bytes.len(),
                });
            }
            let ram = bytes[..byte_len].to_vec();
            let rtc = decode_external_mbc3_rtc_suffix(&bytes[byte_len..], current_unix_seconds)?;
            Ok(PersistentCartState::Mbc3RamRtc { ram, rtc })
        }
        (
            CartridgePersistenceProfile::PersistentRamAndFlash {
                ram: CartridgeRamPayloadKind::Linear { byte_len },
                flash_byte_len,
                hidden_byte_len,
            },
            PersistentCartState::Mbc6 {
                hidden_region,
                sector0_protected,
                ..
            },
        ) => decode_external_mbc6_save(
            bytes,
            byte_len,
            flash_byte_len,
            hidden_byte_len,
            hidden_region,
            *sector0_protected,
        ),
        (
            CartridgePersistenceProfile::PersistentRamAndFlash {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles { .. },
                ..
            },
            _,
        ) => Err(ExternalSaveError::UnsupportedPersistenceProfile {
            profile: metadata.profile,
        }),
        (
            CartridgePersistenceProfile::PersistentEeprom { byte_len },
            PersistentCartState::Mbc7Eeprom { .. },
        ) => decode_external_linear_ram(bytes, byte_len, "MBC7 EEPROM")
            .map(|eeprom| PersistentCartState::Mbc7Eeprom { eeprom }),
        (
            CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: CartridgeRamPayloadKind::Mbc2Nibbles { .. },
            },
            _,
        ) => Err(ExternalSaveError::UnsupportedPersistenceProfile {
            profile: metadata.profile,
        }),
        (
            CartridgePersistenceProfile::PersistentRamAndRtc { .. },
            PersistentCartState::Huc3 { .. },
        ) => Err(ExternalSaveError::UnsupportedPersistentState {
            state_kind: persistent_state_kind_name(target_state),
        }),
        (profile, _) => Err(ExternalSaveError::StateProfileMismatch {
            state_kind: persistent_state_kind_name(target_state),
            profile,
        }),
    }
}

pub(crate) fn external_save_error_allows_internal_fallback(error: &ExternalSaveError) -> bool {
    matches!(
        error,
        ExternalSaveError::UnsupportedPersistentState { .. }
            | ExternalSaveError::UnsupportedPersistenceProfile { .. }
            | ExternalSaveError::UnsupportedStateShape { .. }
    )
}
