use crate::format::{
    EXTERNAL_SAVE_FILE_EXTENSION, EXTERNAL_SAVE_FILE_EXTENSION_P2, EXTERNAL_SAVE_FILE_EXTENSION_P3,
    EXTERNAL_SAVE_FILE_EXTENSION_P4, SAVE_FILE_EXTENSION, SAVE_FILE_EXTENSION_P2,
    SAVE_FILE_EXTENSION_P3, SAVE_FILE_EXTENSION_P4,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum CartridgeSaveFileExtension {
    #[default]
    P1,
    P2,
    P3,
    P4,
}

impl CartridgeSaveFileExtension {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::P1 => SAVE_FILE_EXTENSION,
            Self::P2 => SAVE_FILE_EXTENSION_P2,
            Self::P3 => SAVE_FILE_EXTENSION_P3,
            Self::P4 => SAVE_FILE_EXTENSION_P4,
        }
    }

    pub const fn external_as_str(self) -> &'static str {
        match self {
            Self::P1 => EXTERNAL_SAVE_FILE_EXTENSION,
            Self::P2 => EXTERNAL_SAVE_FILE_EXTENSION_P2,
            Self::P3 => EXTERNAL_SAVE_FILE_EXTENSION_P3,
            Self::P4 => EXTERNAL_SAVE_FILE_EXTENSION_P4,
        }
    }
}
