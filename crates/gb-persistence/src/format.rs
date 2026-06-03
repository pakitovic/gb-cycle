pub(crate) const SAVE_MAGIC: [u8; 8] = *b"GBCSAVE\0";
pub(crate) const MACHINE_SAVE_STATE_MAGIC: [u8; 8] = *b"GBSTATE\0";

pub const CURRENT_SAVE_FORMAT_VERSION: u16 = 1;
pub const CURRENT_MACHINE_SAVE_STATE_FORMAT_VERSION: u16 = 1;
pub const SAVE_FILE_EXTENSION: &str = "gbsav";
pub const SAVE_FILE_EXTENSION_P2: &str = "gbsa2";
pub const SAVE_FILE_EXTENSION_P3: &str = "gbsa3";
pub const SAVE_FILE_EXTENSION_P4: &str = "gbsa4";
pub const EXTERNAL_SAVE_FILE_EXTENSION: &str = "sav";
pub const EXTERNAL_SAVE_FILE_EXTENSION_P2: &str = "sa2";
pub const EXTERNAL_SAVE_FILE_EXTENSION_P3: &str = "sa3";
pub const EXTERNAL_SAVE_FILE_EXTENSION_P4: &str = "sa4";
pub const MACHINE_SAVE_STATE_FILE_EXTENSION: &str = "gbstate";

pub(crate) const MBC2_RAM_NIBBLE_COUNT: usize = 512;
pub(crate) const MBC2_MGBA_PACKED_BYTE_COUNT: usize = MBC2_RAM_NIBBLE_COUNT / 2;
pub(crate) const MBC3_EXTERNAL_RTC_SUFFIX_LEN_32BIT_TIMESTAMP: usize = 44;
pub(crate) const MBC3_EXTERNAL_RTC_SUFFIX_LEN: usize = 48;
