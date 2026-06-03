use super::{ExternalSaveError, ExternalSaveExportFormat, ExternalSaveLengthExpectation};
use crate::format::{MBC2_MGBA_PACKED_BYTE_COUNT, MBC2_RAM_NIBBLE_COUNT};

pub(super) fn encode_external_mbc2_ram(
    ram_nibbles: &[u8; MBC2_RAM_NIBBLE_COUNT],
    expected_cell_count: usize,
    format: ExternalSaveExportFormat,
) -> Result<Vec<u8>, ExternalSaveError> {
    if expected_cell_count != MBC2_RAM_NIBBLE_COUNT {
        return Err(ExternalSaveError::InvalidLength {
            context: "MBC2 metadata",
            expected: ExternalSaveLengthExpectation::Exact(MBC2_RAM_NIBBLE_COUNT),
            actual: expected_cell_count,
        });
    }

    match format {
        ExternalSaveExportFormat::Mgba => {
            let mut bytes = Vec::with_capacity(MBC2_MGBA_PACKED_BYTE_COUNT);
            for pair in ram_nibbles.chunks_exact(2) {
                bytes.push((pair[0] & 0x0F) | ((pair[1] & 0x0F) << 4));
            }
            Ok(bytes)
        }
    }
}

pub(super) fn decode_external_mbc2_ram(
    bytes: &[u8],
    expected_cell_count: usize,
) -> Result<[u8; MBC2_RAM_NIBBLE_COUNT], ExternalSaveError> {
    if expected_cell_count != MBC2_RAM_NIBBLE_COUNT {
        return Err(ExternalSaveError::InvalidLength {
            context: "MBC2 metadata",
            expected: ExternalSaveLengthExpectation::Exact(MBC2_RAM_NIBBLE_COUNT),
            actual: expected_cell_count,
        });
    }

    let mut ram_nibbles = [0; MBC2_RAM_NIBBLE_COUNT];
    match bytes.len() {
        MBC2_MGBA_PACKED_BYTE_COUNT => {
            for (index, byte) in bytes.iter().copied().enumerate() {
                ram_nibbles[index * 2] = byte & 0x0F;
                ram_nibbles[index * 2 + 1] = (byte >> 4) & 0x0F;
            }
            Ok(ram_nibbles)
        }
        MBC2_RAM_NIBBLE_COUNT => {
            for (index, byte) in bytes.iter().copied().enumerate() {
                ram_nibbles[index] = byte & 0x0F;
            }
            Ok(ram_nibbles)
        }
        actual => Err(ExternalSaveError::InvalidLength {
            context: "MBC2 RAM",
            expected: ExternalSaveLengthExpectation::Either {
                first: MBC2_MGBA_PACKED_BYTE_COUNT,
                second: MBC2_RAM_NIBBLE_COUNT,
            },
            actual,
        }),
    }
}
