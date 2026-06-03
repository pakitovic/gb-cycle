use super::{ExternalSaveError, ExternalSaveLengthExpectation};

pub(super) fn encode_external_linear_ram(
    ram: &[u8],
    expected_len: usize,
) -> Result<Vec<u8>, ExternalSaveError> {
    if ram.len() != expected_len {
        return Err(ExternalSaveError::InvalidLength {
            context: "linear RAM state",
            expected: ExternalSaveLengthExpectation::Exact(expected_len),
            actual: ram.len(),
        });
    }
    Ok(ram.to_vec())
}

pub(super) fn decode_external_linear_ram(
    bytes: &[u8],
    expected_len: usize,
    context: &'static str,
) -> Result<Vec<u8>, ExternalSaveError> {
    if bytes.len() != expected_len {
        return Err(ExternalSaveError::InvalidLength {
            context,
            expected: ExternalSaveLengthExpectation::Exact(expected_len),
            actual: bytes.len(),
        });
    }
    Ok(bytes.to_vec())
}
