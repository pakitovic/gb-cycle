use crate::model::ExecutionMode;

mod classify;
mod device;
mod header;
mod mbc1;
mod mbc2;
mod mbc3;
mod mbc5;
mod no_mbc;
mod persist;
mod slot;
mod validate;

#[cfg(test)]
use classify::{classify_loaded_cartridge, is_mbc1m_multicart_signature, is_wisdom_tree_signature};
#[cfg(test)]
use validate::{validate_mbc1, validate_mbc2, validate_mbc3, validate_mbc5, validate_no_mbc};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
const ENTRY_POINT_LEN: usize = 4;
const NINTENDO_LOGO_LEN: usize = 48;

const ENTRY_POINT_START: usize = 0x0100;
const NINTENDO_LOGO_START: usize = 0x0104;
const TITLE_START: usize = 0x0134;
const TITLE_END_INCLUSIVE: usize = 0x0143;
const CGB_FLAG_ADDRESS: usize = 0x0143;
const SGB_FLAG_ADDRESS: usize = 0x0146;
const CARTRIDGE_TYPE_ADDRESS: usize = 0x0147;
const ROM_SIZE_ADDRESS: usize = 0x0148;
const RAM_SIZE_ADDRESS: usize = 0x0149;
const DESTINATION_CODE_ADDRESS: usize = 0x014A;
const HEADER_CHECKSUM_ADDRESS: usize = 0x014D;

const RAM_ABSENT_READ_VALUE: u8 = 0xFF;
const NO_MBC_SUPPORTED_ROM_BYTES: usize = 32 * 1024;
const NO_MBC_SUPPORTED_RAM_BYTES: usize = 8 * 1024;
const M161_BANK_BYTES: usize = 32 * 1024;
const M161_SUPPORTED_ROM_BYTES_MAX: usize = 8 * M161_BANK_BYTES;
const MBC2_SUPPORTED_ROM_BYTES_MAX: usize = 256 * 1024;
const MBC2_RAM_CELL_COUNT: usize = 512;
const MBC2_RAM_ADDRESS_MASK: usize = MBC2_RAM_CELL_COUNT - 1;
const MBC2_RAM_READ_HIGH_NIBBLE: u8 = 0xF0;
const MBC3_SUPPORTED_ROM_BYTES_MAX: usize = 2 * 1024 * 1024;
const MBC5_SUPPORTED_ROM_BYTES_MAX: usize = 8 * 1024 * 1024;
const MBC1_STANDARD_ROM_SIZES: [usize; 5] =
    [32 * 1024, 64 * 1024, 128 * 1024, 256 * 1024, 512 * 1024];
const MBC1_LARGE_ROM_SIZES: [usize; 2] = [1024 * 1024, 2 * 1024 * 1024];
const M161_KNOWN_SUBTITLE_SET: [&[u8]; 4] = [b"TETRIS", b"TENNIS", b"ALLEY WAY", b"YAKUMAN"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartridgeSlotState {
    Empty,
    NoMbc,
    Mbc1,
    Mbc2,
    Mbc3,
    Mbc5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CgbFlag {
    None,
    Supported,
    Only,
    Unknown(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SgbFlag {
    None,
    Supported,
    Unknown(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RomSizeInfo {
    pub raw_code: u8,
    pub decoded_bytes: Option<usize>,
    pub bank_count: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RamSizeInfo {
    pub raw_code: u8,
    pub decoded_bytes: Option<usize>,
    pub bank_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartridgeHeader {
    pub entry_point: [u8; ENTRY_POINT_LEN],
    pub nintendo_logo: [u8; NINTENDO_LOGO_LEN],
    pub title: String,
    pub cgb_flag: CgbFlag,
    pub sgb_flag: SgbFlag,
    pub cartridge_type: u8,
    pub rom_size: RomSizeInfo,
    pub ram_size: RamSizeInfo,
    pub header_checksum: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CartridgeHeaderParseError {
    ImageTooSmall {
        actual_size: usize,
        minimum_size: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedCartridgeFamily {
    NoMbc,
    Mbc1,
    Mbc2,
    Mbc3,
    Mbc5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedCartridgeCategory {
    PlannedVariant,
    DocumentedButUnsupported,
    ExperimentalHeuristic,
    AccessorySpecialCase,
    UnknownCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartridgeSelection {
    Supported(SupportedCartridgeFamily),
    Unsupported(UnsupportedCartridgeCategory),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CartridgeClassification {
    raw_type: u8,
    detected_name: &'static str,
    selection: CartridgeSelection,
    reason: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartridgeDiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartridgeDiagnostic {
    pub severity: CartridgeDiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CartridgeLoadError {
    HeaderParse(CartridgeHeaderParseError),
    Rejected {
        classification: CartridgeClassification,
        execution_mode: ExecutionMode,
        reason: String,
        diagnostics: Vec<CartridgeDiagnostic>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartridgeLoadReport {
    cartridge: CartridgeSlot,
    diagnostics: Vec<CartridgeDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartridgeSlot {
    device: Option<CartridgeDevice>,
}

// Keep one concrete mapper-owned state object in the slot; boxing every large
// mapper here would add indirection across the whole cartridge path.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
enum CartridgeDevice {
    NoMbc(NoMbcCartridge),
    Mbc1(Mbc1Cartridge),
    Mbc2(Mbc2Cartridge),
    Mbc3(Mbc3Cartridge),
    Mbc5(Mbc5Cartridge),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NoMbcCartridge {
    rom: Vec<u8>,
    ram: Option<Vec<u8>>,
    has_battery: bool,
    header: CartridgeHeader,
    classification: CartridgeClassification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mbc1Wiring {
    Standard,
    LargeRom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum Mbc1Variant {
    Standard,
    Mbc1M,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Mbc1Layout {
    wiring: Mbc1Wiring,
    variant: Mbc1Variant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Mbc1Cartridge {
    rom: Vec<u8>,
    ram: Option<Vec<u8>>,
    has_battery: bool,
    header: CartridgeHeader,
    classification: CartridgeClassification,
    variant: Mbc1Variant,
    wiring: Mbc1Wiring,
    ram_enabled: bool,
    rom_bank_low5: u8,
    secondary_bank: u8,
    banking_mode: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Mbc2Cartridge {
    rom: Vec<u8>,
    ram_nibbles: [u8; MBC2_RAM_CELL_COUNT],
    has_battery: bool,
    header: CartridgeHeader,
    classification: CartridgeClassification,
    ram_enabled: bool,
    rom_bank_low4: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum Mbc3Variant {
    Standard,
    Mbc30Reserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mbc3RtcRegister {
    Seconds,
    Minutes,
    Hours,
    DayLow,
    DayHigh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mbc3RamRtcSelect {
    RamBank(u8),
    ReservedSelector(u8),
    RtcRegister(Mbc3RtcRegister),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Mbc3RtcState {
    seconds: u8,
    minutes: u8,
    hours: u8,
    day_counter: u16,
    halt: bool,
    carry: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Mbc3Cartridge {
    rom: Vec<u8>,
    ram: Option<Vec<u8>>,
    has_battery: bool,
    has_rtc: bool,
    header: CartridgeHeader,
    classification: CartridgeClassification,
    variant: Mbc3Variant,
    ram_rtc_enabled: bool,
    rom_bank: u8,
    ram_or_rtc_select: Mbc3RamRtcSelect,
    rtc_live: Mbc3RtcState,
    rtc_latched: Mbc3RtcState,
    rtc_latched_valid: bool,
    rtc_latch_armed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mbc5Variant {
    NoRam,
    Ram,
    RamBattery,
    Rumble,
    RumbleRam,
    RumbleRamBattery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Mbc5Cartridge {
    rom: Vec<u8>,
    ram: Option<Vec<u8>>,
    has_battery: bool,
    has_rumble: bool,
    header: CartridgeHeader,
    classification: CartridgeClassification,
    variant: Mbc5Variant,
    ram_enabled: bool,
    rom_bank_low8: u8,
    rom_bank_high1: u8,
    ram_bank_raw: u8,
    rumble_on: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartridgeSnapshot {
    pub state: CartridgeSlotState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartridgeRamPayloadKind {
    Linear { byte_len: usize },
    Mbc2Nibbles { cell_count: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartridgePersistenceProfile {
    None,
    NonPersistentRam { ram: CartridgeRamPayloadKind },
    PersistentRam { ram: CartridgeRamPayloadKind },
    PersistentRtc,
    PersistentRamAndRtc { ram: CartridgeRamPayloadKind },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CartridgePersistenceMetadata {
    pub has_battery: bool,
    pub has_rtc: bool,
    pub profile: CartridgePersistenceProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mbc3RtcPersistentState {
    pub seconds: u8,
    pub minutes: u8,
    pub hours: u8,
    pub day_counter: u16,
    pub halt: bool,
    pub carry: bool,
}

// Persist the full mapper-owned backing store shape explicitly, including the
// MBC2 nibble array, instead of hiding those semantics behind ad hoc packing.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistentCartState {
    None,
    NoMbcRam {
        ram: Vec<u8>,
    },
    Mbc1Ram {
        ram: Vec<u8>,
    },
    Mbc2Ram {
        ram_nibbles: [u8; MBC2_RAM_CELL_COUNT],
    },
    Mbc3Rtc {
        rtc: Mbc3RtcPersistentState,
    },
    Mbc3Ram {
        ram: Vec<u8>,
    },
    Mbc3RamRtc {
        ram: Vec<u8>,
        rtc: Mbc3RtcPersistentState,
    },
    Mbc5Ram {
        ram: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CartridgePersistentStateError {
    KindMismatch {
        expected: &'static str,
        actual: &'static str,
    },
    RamLengthMismatch {
        expected: usize,
        actual: usize,
    },
    InvalidMbc2NibbleValue {
        index: usize,
        value: u8,
    },
}

#[cfg(test)]
mod tests;
