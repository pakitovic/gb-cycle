use crate::backend::CartridgeSaveBackendError;

pub(crate) fn write_bool(bytes: &mut Vec<u8>, value: bool) {
    bytes.push(u8::from(value));
}

pub(crate) fn write_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub(crate) fn write_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub(crate) fn write_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

pub(crate) fn write_u32_checked(
    bytes: &mut Vec<u8>,
    value: usize,
    field: &'static str,
) -> Result<(), CartridgeSaveBackendError> {
    let value = u32::try_from(value)
        .map_err(|_| CartridgeSaveBackendError::LengthOverflow { field, value })?;
    write_u32(bytes, value);
    Ok(())
}

pub(crate) struct ByteCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ByteCursor<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    pub(crate) fn read_u8(&mut self) -> Result<u8, CartridgeSaveBackendError> {
        let bytes = self.read_exact(1)?;
        Ok(bytes[0])
    }

    pub(crate) fn read_u16(&mut self) -> Result<u16, CartridgeSaveBackendError> {
        let bytes = self.read_exact(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub(crate) fn read_u32(&mut self) -> Result<u32, CartridgeSaveBackendError> {
        let bytes = self.read_exact(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub(crate) fn read_u64(&mut self) -> Result<u64, CartridgeSaveBackendError> {
        let bytes = self.read_exact(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    pub(crate) fn read_bool(
        &mut self,
        field: &'static str,
    ) -> Result<bool, CartridgeSaveBackendError> {
        match self.read_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(CartridgeSaveBackendError::InvalidBooleanTag { field, value }),
        }
    }

    pub(crate) fn read_vec(&mut self, len: usize) -> Result<Vec<u8>, CartridgeSaveBackendError> {
        Ok(self.read_exact(len)?.to_vec())
    }

    pub(crate) fn read_array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], CartridgeSaveBackendError> {
        let bytes = self.read_exact(N)?;
        let mut array = [0; N];
        array.copy_from_slice(bytes);
        Ok(array)
    }

    pub(crate) fn read_exact(&mut self, len: usize) -> Result<&'a [u8], CartridgeSaveBackendError> {
        if self.remaining() < len {
            return Err(CartridgeSaveBackendError::UnexpectedEof {
                offset: self.offset,
                needed: len,
                remaining: self.remaining(),
            });
        }
        let start = self.offset;
        self.offset += len;
        Ok(&self.bytes[start..self.offset])
    }
}
