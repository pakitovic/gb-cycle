use super::{ExternalSaveError, ExternalSaveLengthExpectation};
use gb_core::PersistentCartState;

pub(super) fn encode_external_mbc6_save(
    ram: &[u8],
    flash: &[u8],
    hidden_region: &[u8],
    sector0_protected: bool,
    expected_ram_len: usize,
    expected_flash_len: usize,
    expected_hidden_len: usize,
) -> Result<Vec<u8>, ExternalSaveError> {
    if ram.len() != expected_ram_len {
        return Err(ExternalSaveError::InvalidLength {
            context: "MBC6 RAM state",
            expected: ExternalSaveLengthExpectation::Exact(expected_ram_len),
            actual: ram.len(),
        });
    }
    if flash.len() != expected_flash_len {
        return Err(ExternalSaveError::InvalidLength {
            context: "MBC6 flash state",
            expected: ExternalSaveLengthExpectation::Exact(expected_flash_len),
            actual: flash.len(),
        });
    }
    if hidden_region.len() != expected_hidden_len {
        return Err(ExternalSaveError::InvalidLength {
            context: "MBC6 hidden flash state",
            expected: ExternalSaveLengthExpectation::Exact(expected_hidden_len),
            actual: hidden_region.len(),
        });
    }
    if sector0_protected || hidden_region.iter().any(|byte| *byte != 0xFF) {
        return Err(ExternalSaveError::UnsupportedStateShape {
            state_kind: "Mbc6",
            reason: "raw .sav only carries SRAM followed by main flash, not hidden flash or the non-volatile sector-0 protection bit",
        });
    }

    let mut bytes = Vec::with_capacity(expected_ram_len + expected_flash_len);
    bytes.extend_from_slice(ram);
    bytes.extend_from_slice(flash);
    Ok(bytes)
}

pub(super) fn decode_external_mbc6_save(
    bytes: &[u8],
    expected_ram_len: usize,
    expected_flash_len: usize,
    expected_hidden_len: usize,
    target_hidden_region: &[u8],
    target_sector0_protected: bool,
) -> Result<PersistentCartState, ExternalSaveError> {
    if target_hidden_region.len() != expected_hidden_len {
        return Err(ExternalSaveError::InvalidLength {
            context: "MBC6 hidden flash target",
            expected: ExternalSaveLengthExpectation::Exact(expected_hidden_len),
            actual: target_hidden_region.len(),
        });
    }
    if target_sector0_protected || target_hidden_region.iter().any(|byte| *byte != 0xFF) {
        return Err(ExternalSaveError::UnsupportedStateShape {
            state_kind: "Mbc6",
            reason: "raw .sav import cannot merge into a target with hidden flash data or sector-0 protection already set",
        });
    }

    let expected_len = expected_ram_len + expected_flash_len;
    if bytes.len() != expected_len {
        return Err(ExternalSaveError::InvalidLength {
            context: "MBC6 RAM+flash",
            expected: ExternalSaveLengthExpectation::Exact(expected_len),
            actual: bytes.len(),
        });
    }

    Ok(PersistentCartState::Mbc6 {
        ram: bytes[..expected_ram_len].to_vec(),
        flash: bytes[expected_ram_len..].to_vec(),
        hidden_region: vec![0xFF; expected_hidden_len],
        sector0_protected: false,
    })
}
