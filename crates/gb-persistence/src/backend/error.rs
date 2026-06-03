use crate::external_save::ExternalSaveError;
use crate::format::SAVE_MAGIC;
use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum CartridgeSaveBackendError {
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    InvalidMagic {
        actual: [u8; SAVE_MAGIC.len()],
    },
    UnexpectedEof {
        offset: usize,
        needed: usize,
        remaining: usize,
    },
    UnsupportedFormatVersion {
        version: u16,
    },
    UnsupportedRamPayloadKindTag {
        tag: u8,
    },
    UnsupportedPersistenceProfileTag {
        tag: u8,
    },
    UnsupportedPersistentStateTag {
        tag: u8,
    },
    UnsupportedMachineSaveStateTag {
        field: &'static str,
        tag: u8,
    },
    InvalidBooleanTag {
        field: &'static str,
        value: u8,
    },
    LengthOverflow {
        field: &'static str,
        value: usize,
    },
    InvalidMbc2NibbleValue {
        index: usize,
        value: u8,
    },
    InvalidHuc3NibbleValue {
        index: usize,
        value: u8,
    },
    MachineSaveStateCodec {
        operation: &'static str,
        message: String,
    },
    ExternalSave {
        operation: &'static str,
        path: PathBuf,
        source: ExternalSaveError,
    },
    MachineSaveStateMetadataMismatch,
    TrailingBytes {
        remaining: usize,
    },
}

impl fmt::Display for CartridgeSaveBackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation, path, ..
            } => write!(f, "{operation} failed for {}", path.display()),
            Self::InvalidMagic { actual } => write!(f, "invalid save magic: {actual:?}"),
            Self::UnexpectedEof {
                offset,
                needed,
                remaining,
            } => write!(
                f,
                "unexpected end of save payload at offset {offset}: needed {needed} bytes but only {remaining} remain"
            ),
            Self::UnsupportedFormatVersion { version } => {
                write!(f, "unsupported save format version {version}")
            }
            Self::UnsupportedRamPayloadKindTag { tag } => {
                write!(f, "unsupported RAM payload kind tag {tag:#04X}")
            }
            Self::UnsupportedPersistenceProfileTag { tag } => {
                write!(f, "unsupported persistence profile tag {tag:#04X}")
            }
            Self::UnsupportedPersistentStateTag { tag } => {
                write!(f, "unsupported persistent state tag {tag:#04X}")
            }
            Self::UnsupportedMachineSaveStateTag { field, tag } => {
                write!(
                    f,
                    "unsupported machine save-state tag for {field}: {tag:#04X}"
                )
            }
            Self::InvalidBooleanTag { field, value } => {
                write!(f, "invalid boolean tag for {field}: {value:#04X}")
            }
            Self::LengthOverflow { field, value } => {
                write!(f, "{field} length {value} exceeds format capacity")
            }
            Self::InvalidMbc2NibbleValue { index, value } => write!(
                f,
                "invalid MBC2 nibble value {value:#04X} at logical cell {index}"
            ),
            Self::InvalidHuc3NibbleValue { index, value } => write!(
                f,
                "invalid HuC-3 nibble value {value:#04X} at logical cell {index}"
            ),
            Self::MachineSaveStateCodec { operation, message } => {
                write!(f, "machine save-state {operation} failed: {message}")
            }
            Self::ExternalSave {
                operation,
                path,
                source,
            } => write!(f, "{operation} failed for {}: {source}", path.display()),
            Self::MachineSaveStateMetadataMismatch => {
                write!(
                    f,
                    "machine save-state envelope metadata does not match payload metadata"
                )
            }
            Self::TrailingBytes { remaining } => {
                write!(f, "save payload has {remaining} trailing bytes")
            }
        }
    }
}

impl std::error::Error for CartridgeSaveBackendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::ExternalSave { source, .. } => Some(source),
            _ => None,
        }
    }
}
