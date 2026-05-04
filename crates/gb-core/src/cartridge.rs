use crate::model::ExecutionMode;
use crate::save_state::SaveStateByteFingerprint;
use crate::scheduler::TCycle;
use std::{fmt, mem};

mod classify;
mod device;
mod header;
mod huc1;
mod huc3;
mod m161;
mod mbc1;
mod mbc2;
mod mbc3;
mod mbc5;
mod mbc7;
mod mmm01;
mod no_mbc;
mod persist;
mod pocket_camera;
mod slot;
mod validate;

#[cfg(test)]
use classify::{
    classify_loaded_cartridge, is_mbc1m_multicart_signature, is_wisdom_tree_signature,
    matches_padded_title, supported,
};
#[cfg(test)]
use header::{decode_cgb_flag, decode_sgb_flag};
#[cfg(test)]
use validate::{
    expected_ram_code_decompressed, record_degradable_issue, validate_huc1, validate_huc3,
    validate_m161, validate_mbc1, validate_mbc2, validate_mbc3, validate_mbc5, validate_no_mbc,
};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
const ENTRY_POINT_LEN: usize = 4;
const NINTENDO_LOGO_LEN: usize = 48;
const TITLE_BYTES_LEN: usize = TITLE_END_INCLUSIVE - TITLE_START + 1;
const MANUFACTURER_CODE_LEN: usize = MANUFACTURER_CODE_END_INCLUSIVE - MANUFACTURER_CODE_START + 1;
const NEW_LICENSEE_CODE_LEN: usize = 2;

const ENTRY_POINT_START: usize = 0x0100;
const NINTENDO_LOGO_START: usize = 0x0104;
const TITLE_START: usize = 0x0134;
const MANUFACTURER_CODE_START: usize = 0x013F;
const MANUFACTURER_CODE_END_INCLUSIVE: usize = 0x0142;
const TITLE_END_INCLUSIVE: usize = 0x0143;
const CGB_FLAG_ADDRESS: usize = 0x0143;
const NEW_LICENSEE_CODE_START: usize = 0x0144;
const SGB_FLAG_ADDRESS: usize = 0x0146;
const CARTRIDGE_TYPE_ADDRESS: usize = 0x0147;
const ROM_SIZE_ADDRESS: usize = 0x0148;
const RAM_SIZE_ADDRESS: usize = 0x0149;
const DESTINATION_CODE_ADDRESS: usize = 0x014A;
const OLD_LICENSEE_CODE_ADDRESS: usize = 0x014B;
const HEADER_CHECKSUM_ADDRESS: usize = 0x014D;

const RAM_ABSENT_READ_VALUE: u8 = 0xFF;
const NO_MBC_SUPPORTED_ROM_BYTES: usize = 32 * 1024;
const NO_MBC_SUPPORTED_RAM_BYTES: usize = 8 * 1024;
const MBC1_STANDARD_RAM_BYTES_MAX: usize = 32 * 1024;
const MBC1_LARGE_ROM_RAM_BYTES: usize = 8 * 1024;
const M161_BANK_BYTES: usize = 32 * 1024;
const M161_SUPPORTED_ROM_BANKS_MIN: usize = 5;
const M161_SUPPORTED_ROM_BANKS_MAX: usize = 8;
const M161_SUPPORTED_ROM_BYTES_MIN: usize = M161_SUPPORTED_ROM_BANKS_MIN * M161_BANK_BYTES;
const M161_SUPPORTED_ROM_BYTES_MAX: usize = M161_SUPPORTED_ROM_BANKS_MAX * M161_BANK_BYTES;
const MBC2_SUPPORTED_ROM_BYTES_MAX: usize = 256 * 1024;
const MBC2_RAM_CELL_COUNT: usize = 512;
const MBC2_RAM_ADDRESS_MASK: usize = MBC2_RAM_CELL_COUNT - 1;
const MBC2_RAM_READ_HIGH_NIBBLE: u8 = 0xF0;
const MMM01_MENU_BYTES: usize = 32 * 1024;
const MMM01_MIN_ROM_BYTES: usize = 64 * 1024;
const MMM01_SUPPORTED_ROM_BYTES_MAX: usize = 8 * 1024 * 1024;
const MANI_MMM01_SUPPORTED_ROM_BYTES: [usize; 2] = [512 * 1024, 1024 * 1024];
const MANI_MMM01_MENU_TYPE: u8 = 0x11;
const MANI_MMM01_MENU_SUFFIX: &str = " SET";
const HUC1_SUPPORTED_ROM_BYTES_MAX: usize = 1024 * 1024;
const HUC1_SUPPORTED_RAM_BYTES_MAX: usize = 32 * 1024;
const HUC3_SUPPORTED_ROM_BYTES_MAX: usize = 2 * 1024 * 1024;
const HUC3_SUPPORTED_RAM_BYTES_MAX: usize = 32 * 1024;
const HUC3_MCU_RAM_NIBBLE_COUNT: usize = 256;
const HUC3_DAY_COUNTER_MODULUS: u16 = 0x1000;
const HUC3_MINUTES_PER_DAY: u16 = 1440;
const MBC3_SUPPORTED_ROM_BYTES_MAX: usize = 2 * 1024 * 1024;
const MBC30_SUPPORTED_ROM_BYTES_MAX: usize = 4 * 1024 * 1024;
const MBC3_RTC_ACCESS_SPACING_T_CYCLES: u64 = 16;
const MBC3_RTC_CLOCK_TICKS_PER_SECOND: u64 = 32_768;
const MBC5_SUPPORTED_ROM_BYTES_MAX: usize = 8 * 1024 * 1024;
const MBC7_SUPPORTED_ROM_BYTES_MAX: usize = 2 * 1024 * 1024;
const MBC7_EEPROM_BYTES: usize = 256;
const MBC7_EEPROM_WORDS: usize = MBC7_EEPROM_BYTES / 2;
const MBC7_ACCELEROMETER_UNLATCHED_VALUE: u16 = 0x8000;
const MBC7_ACCELEROMETER_NEUTRAL_VALUE: u16 = 0x81D0;
const MBC7_ACCELEROMETER_DELTA_PER_G: i32 = 0x0070;
const POCKET_CAMERA_SUPPORTED_ROM_BYTES: usize = 1024 * 1024;
const POCKET_CAMERA_SUPPORTED_RAM_BYTES: usize = 128 * 1024;
const POCKET_CAMERA_RAM_BANK_BYTES: usize = 8 * 1024;
const POCKET_CAMERA_CAPTURE_TILE_BASE_OFFSET: usize = 0x0100;
const POCKET_CAMERA_CAPTURE_TILE_BYTES: usize = 14 * 16 * 16;
const POCKET_CAMERA_CAPTURE_TILE_WIDTH: usize = 16;
const POCKET_CAMERA_CAPTURE_WIDTH: usize = 128;
const POCKET_CAMERA_CAPTURE_HEIGHT: usize = 112;
const POCKET_CAMERA_CAPTURE_PIXEL_COUNT: usize =
    POCKET_CAMERA_CAPTURE_WIDTH * POCKET_CAMERA_CAPTURE_HEIGHT;
const POCKET_CAMERA_SENSOR_EXTRA_LINES: usize = 8;
const POCKET_CAMERA_SENSOR_HEIGHT: usize =
    POCKET_CAMERA_CAPTURE_HEIGHT + POCKET_CAMERA_SENSOR_EXTRA_LINES;
const POCKET_CAMERA_SENSOR_PIXEL_COUNT: usize =
    POCKET_CAMERA_CAPTURE_WIDTH * POCKET_CAMERA_SENSOR_HEIGHT;
const POCKET_CAMERA_REGISTER_COUNT: usize = 0x36;
const POCKET_CAMERA_REGISTER_MIRROR_MASK: usize = 0x7F;
const POCKET_CAMERA_WORKING_RAM_READ_VALUE: u8 = 0x00;
const MBC1_STANDARD_ROM_SIZES: [usize; 5] =
    [32 * 1024, 64 * 1024, 128 * 1024, 256 * 1024, 512 * 1024];
const MBC1_LARGE_ROM_SIZES: [usize; 2] = [1024 * 1024, 2 * 1024 * 1024];
const M161_SYNTHETIC_MENU_TITLE: &[u8] = b"MANI 4 IN 1";
const M161_COMMERCIAL_MENU_TITLE: &[u8] = b"TETRIS SET";
const M161_KNOWN_SUBTITLE_SET: [&[u8]; 4] = [b"TETRIS", b"TENNIS", b"ALLEY WAY", b"YAKUMAN"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CartridgeSlotState {
    Empty,
    NoMbc,
    Mmm01,
    M161,
    Huc1,
    Huc3,
    Mbc1,
    Mbc2,
    Mbc3,
    Mbc5,
    Mbc7,
    PocketCamera,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CgbFlag {
    None,
    Supported,
    Only,
    SupportedNonCanonical(u8),
    Unknown(u8),
}

impl CgbFlag {
    pub const fn enables_cgb_native_mode(self) -> bool {
        matches!(
            self,
            Self::Supported | Self::Only | Self::SupportedNonCanonical(_)
        )
    }

    pub const fn is_cgb_only(self) -> bool {
        matches!(self, Self::Only)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SgbFlag {
    None,
    Supported,
    Unknown(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RomSizeInfo {
    pub raw_code: u8,
    pub decoded_bytes: Option<usize>,
    pub bank_count: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RamSizeInfo {
    pub raw_code: u8,
    pub decoded_bytes: Option<usize>,
    pub bank_count: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CartridgeHeader {
    pub entry_point: [u8; ENTRY_POINT_LEN],
    #[serde(with = "serde_big_array::BigArray")]
    pub nintendo_logo: [u8; NINTENDO_LOGO_LEN],
    pub title_bytes: [u8; TITLE_BYTES_LEN],
    pub raw_title_suffix_or_manufacturer_code: [u8; MANUFACTURER_CODE_LEN],
    pub title: String,
    pub cgb_flag: CgbFlag,
    pub sgb_flag: SgbFlag,
    pub cartridge_type: u8,
    pub rom_size: RomSizeInfo,
    pub ram_size: RamSizeInfo,
    pub new_licensee_code: [u8; NEW_LICENSEE_CODE_LEN],
    pub destination_code: u8,
    pub old_licensee_code: u8,
    pub header_checksum: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CartridgeHeaderParseError {
    ImageTooSmall {
        actual_size: usize,
        minimum_size: usize,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SupportedCartridgeFamily {
    NoMbc,
    Mmm01,
    M161,
    Huc1,
    Huc3,
    Mbc1,
    Mbc2,
    Mbc3,
    Mbc5,
    Mbc7,
    PocketCamera,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UnsupportedCartridgeCategory {
    PlannedVariant,
    DocumentedButUnsupported,
    ExperimentalHeuristic,
    AccessorySpecialCase,
    UnknownCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CartridgeSelection {
    Supported(SupportedCartridgeFamily),
    Unsupported(UnsupportedCartridgeCategory),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct CartridgeClassification {
    raw_type: u8,
    detected_name: &'static str,
    selection: CartridgeSelection,
    reason: &'static str,
}

impl<'de> serde::Deserialize<'de> for CartridgeClassification {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct CartridgeClassificationFields {
            raw_type: u8,
            detected_name: String,
            selection: CartridgeSelection,
            reason: String,
        }

        let fields = CartridgeClassificationFields::deserialize(deserializer)?;
        Ok(Self {
            raw_type: fields.raw_type,
            detected_name: known_classification_str(&fields.detected_name).ok_or_else(|| {
                serde::de::Error::custom(format!(
                    "unknown cartridge classification name {:?}",
                    fields.detected_name
                ))
            })?,
            selection: fields.selection,
            reason: known_classification_str(&fields.reason).ok_or_else(|| {
                serde::de::Error::custom(format!(
                    "unknown cartridge classification reason {:?}",
                    fields.reason
                ))
            })?,
        })
    }
}

fn known_classification_str(value: &str) -> Option<&'static str> {
    KNOWN_CLASSIFICATION_STRINGS
        .iter()
        .copied()
        .find(|known| *known == value)
}

const KNOWN_CLASSIFICATION_STRINGS: &[&str] = &[
    "ROM ONLY",
    "ROM+RAM",
    "ROM+RAM+BATTERY",
    "MBC1",
    "MBC1+RAM",
    "MBC1+RAM+BATTERY",
    "MBC1M",
    "MBC2",
    "MBC2+BATTERY",
    "MBC3",
    "MBC3+RAM",
    "MBC3+RAM+BATTERY",
    "MBC3+TIMER+BATTERY",
    "MBC3+TIMER+RAM+BATTERY",
    "MBC30",
    "MBC5",
    "MBC5+RAM",
    "MBC5+RAM+BATTERY",
    "MBC5+RUMBLE",
    "MBC5+RUMBLE+RAM",
    "MBC5+RUMBLE+RAM+BATTERY",
    "MBC6",
    "MBC7+SENSOR+RUMBLE+RAM+BATTERY",
    "MMM01",
    "MMM01+RAM",
    "MMM01+RAM+BATTERY",
    "M161",
    "HuC-3",
    "HuC1+RAM+BATTERY",
    "POCKET CAMERA",
    "BANDAI TAMA5",
    "BUNG",
    "EMS",
    "WISDOM TREE",
    "UNKNOWN",
    "supported cartridge family",
    "MBC6 requires a dedicated cartridge-local implementation",
    "MBC7 requires EEPROM and accelerometer behavior that is not implemented yet",
    "Bandai TAMA5 needs dedicated accessory hardware",
    "The cartridge type code is not recognized",
    "M161 multicart classification came from the explicit Mani 4-in-1 signature path",
    "MMM01 classification came from the explicit later Mani trailing-menu signature path",
    "MBC1 multicart classification came from the explicit subheader signature path",
    "MBC30 classification came from the MBC3 64 KiB SRAM header shape",
    "MBC30 is a known MBC3-family variant reserved for later support",
    "Bung multicart classification came from an explicit experimental heuristic path",
    "EMS multicart classification came from an explicit experimental heuristic path",
    "Wisdom Tree classification came from an explicit experimental heuristic path",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CartridgeDiagnosticSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CartridgeDiagnostic {
    pub severity: CartridgeDiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CartridgeLoadError {
    HeaderParse(CartridgeHeaderParseError),
    Rejected {
        classification: CartridgeClassification,
        execution_mode: ExecutionMode,
        reason: String,
        diagnostics: Vec<CartridgeDiagnostic>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CartridgeLoadReport {
    cartridge: CartridgeSlot,
    diagnostics: Vec<CartridgeDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CartridgeSlot {
    device: Option<CartridgeDevice>,
    rom_fingerprint: Option<SaveStateByteFingerprint>,
}

// Keep one concrete mapper-owned state object in the slot; boxing every large
// mapper here would add indirection across the whole cartridge path.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum CartridgeDevice {
    NoMbc(NoMbcCartridge),
    Mmm01(Mmm01Cartridge),
    M161(M161Cartridge),
    Huc1(Huc1Cartridge),
    Huc3(Huc3Cartridge),
    Mbc1(Mbc1Cartridge),
    Mbc2(Mbc2Cartridge),
    Mbc3(Mbc3Cartridge),
    Mbc5(Mbc5Cartridge),
    Mbc7(Mbc7Cartridge),
    PocketCamera(PocketCameraCartridge),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct NoMbcCartridge {
    rom: Vec<u8>,
    ram: Option<Vec<u8>>,
    has_battery: bool,
    header: CartridgeHeader,
    classification: CartridgeClassification,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Mmm01Cartridge {
    rom: Vec<u8>,
    ram: Option<Vec<u8>>,
    has_battery: bool,
    header: CartridgeHeader,
    classification: CartridgeClassification,
    mapped: bool,
    ram_enabled: bool,
    ram_bank_mask: u8,
    rom_bank_low: u8,
    rom_bank_mid: u8,
    ram_bank_low: u8,
    ram_bank_high: u8,
    rom_bank_high: u8,
    mode_write_disable: bool,
    banking_mode: u8,
    rom_bank_mask: u8,
    multiplex_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct M161Cartridge {
    rom: Vec<u8>,
    header: CartridgeHeader,
    classification: CartridgeClassification,
    selected_bank: u8,
    bank_switch_locked: bool,
    last_bank_write: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Huc1IoMode {
    Ram,
    Ir,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Huc1Cartridge {
    rom: Vec<u8>,
    ram: Option<Vec<u8>>,
    has_battery: bool,
    header: CartridgeHeader,
    classification: CartridgeClassification,
    io_mode: Huc1IoMode,
    rom_bank: u8,
    ram_bank: u8,
    ir_emitter_on: bool,
    ir_light_detected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Huc3SelectMode {
    RamReadOnly,
    RamReadWrite,
    RtcCommandArgument,
    RtcCommandResponse,
    RtcSemaphore,
    Ir,
    OpenBus(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Huc3Mailbox {
    command: u8,
    argument: u8,
    last_response_nybble: u8,
    semaphore_ready: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
struct Huc3RtcState {
    current_minutes_of_day: u16,
    current_days: u16,
    current_subminute_seconds: u8,
    event_minutes_of_day: u16,
    event_days: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Huc3Cartridge {
    rom: Vec<u8>,
    ram: Vec<u8>,
    has_battery: bool,
    header: CartridgeHeader,
    classification: CartridgeClassification,
    select_mode: Huc3SelectMode,
    rom_bank: u8,
    ram_bank: u8,
    access_address: u8,
    mailbox: Huc3Mailbox,
    #[serde(with = "serde_big_array::BigArray")]
    mcu_ram: [u8; HUC3_MCU_RAM_NIBBLE_COUNT],
    rtc: Huc3RtcState,
    ir_emitter_on: bool,
    ir_light_detected: bool,
    last_control_write: Option<u8>,
    last_unsupported_command: Option<u8>,
    last_unsupported_argument: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Mbc1Wiring {
    Standard,
    LargeRom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
enum Mbc1Variant {
    Standard,
    Mbc1M,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Mbc1Layout {
    wiring: Mbc1Wiring,
    variant: Mbc1Variant,
    ram_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Mbc2Cartridge {
    rom: Vec<u8>,
    #[serde(with = "serde_big_array::BigArray")]
    ram_nibbles: [u8; MBC2_RAM_CELL_COUNT],
    has_battery: bool,
    header: CartridgeHeader,
    classification: CartridgeClassification,
    ram_enabled: bool,
    rom_bank_low4: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
enum Mbc3Variant {
    Standard,
    Mbc30,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Mbc3RtcRegister {
    Seconds,
    Minutes,
    Hours,
    DayLow,
    DayHigh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Mbc3RamRtcSelect {
    RamBank(u8),
    ReservedSelector(u8),
    RtcRegister(Mbc3RtcRegister),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
struct Mbc3RtcState {
    seconds: u8,
    minutes: u8,
    hours: u8,
    day_counter: u16,
    halt: bool,
    carry: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    #[serde(default)]
    rtc_subsecond_ticks: u16,
    rtc_access_ready_at: Option<TCycle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Mbc5Variant {
    NoRam,
    Ram,
    RamBattery,
    Rumble,
    RumbleRam,
    RumbleRamBattery,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Mbc7Cartridge {
    rom: Vec<u8>,
    eeprom: Vec<u8>,
    header: CartridgeHeader,
    classification: CartridgeClassification,
    ram_enabled: bool,
    sensor_eeprom_enabled: bool,
    rom_bank: u8,
    accelerometer_input: Mbc7AccelerometerInput,
    accelerometer_latch_armed: bool,
    latched_x: u16,
    latched_y: u16,
    eeprom_pins: Mbc7EepromPins,
    eeprom_command: Mbc7EepromCommand,
    eeprom_write_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Mbc7EepromPins {
    cs: bool,
    clk: bool,
    di: bool,
    do_pin: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Mbc7EepromCommand {
    Idle,
    ReceivingCommand {
        bits: u8,
        value: u16,
    },
    ReceivingData {
        target: Mbc7EepromDataTarget,
        bits: u8,
        value: u16,
    },
    SendingRead {
        bits_remaining: u8,
        value: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Mbc7EepromDataTarget {
    WriteWord { address: u8 },
    WriteAll,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct PocketCameraCartridge {
    rom: Vec<u8>,
    ram: Vec<u8>,
    header: CartridgeHeader,
    classification: CartridgeClassification,
    ram_enabled: bool,
    rom_bank: u8,
    ram_bank_or_register_select: u8,
    #[serde(with = "serde_big_array::BigArray")]
    registers: [u8; POCKET_CAMERA_REGISTER_COUNT],
    host_frame: Vec<u8>,
    capture_state: PocketCameraCaptureState,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum PocketCameraCaptureState {
    Idle,
    Working {
        ready_at: TCycle,
        staged_tiles: Vec<u8>,
    },
    Paused {
        remaining_t_cycles: u64,
        staged_tiles: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CartridgeRuntimeSaveState {
    device: Option<CartridgeDeviceSaveState>,
}

impl CartridgeRuntimeSaveState {
    pub(crate) fn dynamic_payload_bytes(&self) -> usize {
        self.device
            .as_ref()
            .map(CartridgeDeviceSaveState::dynamic_payload_bytes)
            .unwrap_or(0)
    }

    pub(crate) fn slot_state(&self) -> CartridgeSlotState {
        self.device
            .as_ref()
            .map(CartridgeDeviceSaveState::slot_state)
            .unwrap_or(CartridgeSlotState::Empty)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CartridgeRuntimeSaveStateError {
    SlotStateMismatch {
        expected: CartridgeSlotState,
        actual: CartridgeSlotState,
    },
    RamShapeMismatch {
        field: &'static str,
        expected: Option<usize>,
        actual: Option<usize>,
    },
}

impl fmt::Display for CartridgeRuntimeSaveStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SlotStateMismatch { expected, actual } => write!(
                f,
                "save-state cartridge runtime state mismatch: expected {:?}, got {:?}",
                expected, actual
            ),
            Self::RamShapeMismatch {
                field,
                expected,
                actual,
            } => write!(
                f,
                "save-state cartridge {field} shape mismatch: expected {:?} bytes, got {:?} bytes",
                expected, actual
            ),
        }
    }
}

impl std::error::Error for CartridgeRuntimeSaveStateError {}

fn optional_ram_shape(bytes: &Option<Vec<u8>>) -> Option<usize> {
    bytes.as_ref().map(Vec::len)
}

fn validate_optional_ram_shape(
    field: &'static str,
    expected: &Option<Vec<u8>>,
    actual: &Option<Vec<u8>>,
) -> Result<(), CartridgeRuntimeSaveStateError> {
    let expected = optional_ram_shape(expected);
    let actual = optional_ram_shape(actual);
    if expected == actual {
        Ok(())
    } else {
        Err(CartridgeRuntimeSaveStateError::RamShapeMismatch {
            field,
            expected,
            actual,
        })
    }
}

fn validate_ram_shape(
    field: &'static str,
    expected: &[u8],
    actual: &[u8],
) -> Result<(), CartridgeRuntimeSaveStateError> {
    if expected.len() == actual.len() {
        Ok(())
    } else {
        Err(CartridgeRuntimeSaveStateError::RamShapeMismatch {
            field,
            expected: Some(expected.len()),
            actual: Some(actual.len()),
        })
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum CartridgeDeviceSaveState {
    NoMbc(NoMbcCartridgeSaveState),
    Mmm01(Mmm01CartridgeSaveState),
    M161(M161CartridgeSaveState),
    Huc1(Huc1CartridgeSaveState),
    Huc3(Huc3CartridgeSaveState),
    Mbc1(Mbc1CartridgeSaveState),
    Mbc2(Mbc2CartridgeSaveState),
    Mbc3(Mbc3CartridgeSaveState),
    Mbc5(Mbc5CartridgeSaveState),
    Mbc7(Mbc7CartridgeSaveState),
    PocketCamera(PocketCameraCartridgeSaveState),
}

impl CartridgeDeviceSaveState {
    fn slot_state(&self) -> CartridgeSlotState {
        match self {
            Self::NoMbc(_) => CartridgeSlotState::NoMbc,
            Self::Mmm01(_) => CartridgeSlotState::Mmm01,
            Self::M161(_) => CartridgeSlotState::M161,
            Self::Huc1(_) => CartridgeSlotState::Huc1,
            Self::Huc3(_) => CartridgeSlotState::Huc3,
            Self::Mbc1(_) => CartridgeSlotState::Mbc1,
            Self::Mbc2(_) => CartridgeSlotState::Mbc2,
            Self::Mbc3(_) => CartridgeSlotState::Mbc3,
            Self::Mbc5(_) => CartridgeSlotState::Mbc5,
            Self::Mbc7(_) => CartridgeSlotState::Mbc7,
            Self::PocketCamera(_) => CartridgeSlotState::PocketCamera,
        }
    }

    fn dynamic_payload_bytes(&self) -> usize {
        match self {
            Self::NoMbc(state) => state.dynamic_payload_bytes(),
            Self::Mmm01(state) => state.dynamic_payload_bytes(),
            Self::M161(state) => state.dynamic_payload_bytes(),
            Self::Huc1(state) => state.dynamic_payload_bytes(),
            Self::Huc3(state) => state.dynamic_payload_bytes(),
            Self::Mbc1(state) => state.dynamic_payload_bytes(),
            Self::Mbc2(state) => state.dynamic_payload_bytes(),
            Self::Mbc3(state) => state.dynamic_payload_bytes(),
            Self::Mbc5(state) => state.dynamic_payload_bytes(),
            Self::Mbc7(state) => state.dynamic_payload_bytes(),
            Self::PocketCamera(state) => state.dynamic_payload_bytes(),
        }
    }
}

fn optional_bytes_len(bytes: &Option<Vec<u8>>) -> usize {
    bytes.as_ref().map(Vec::len).unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct NoMbcCartridgeSaveState {
    ram: Option<Vec<u8>>,
}

impl NoMbcCartridgeSaveState {
    fn dynamic_payload_bytes(&self) -> usize {
        optional_bytes_len(&self.ram)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Mmm01CartridgeSaveState {
    ram: Option<Vec<u8>>,
    mapped: bool,
    ram_enabled: bool,
    ram_bank_mask: u8,
    rom_bank_low: u8,
    rom_bank_mid: u8,
    ram_bank_low: u8,
    ram_bank_high: u8,
    rom_bank_high: u8,
    mode_write_disable: bool,
    banking_mode: u8,
    rom_bank_mask: u8,
    multiplex_enabled: bool,
}

impl Mmm01CartridgeSaveState {
    fn dynamic_payload_bytes(&self) -> usize {
        optional_bytes_len(&self.ram)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct M161CartridgeSaveState {
    selected_bank: u8,
    bank_switch_locked: bool,
    last_bank_write: Option<u8>,
}

impl M161CartridgeSaveState {
    fn dynamic_payload_bytes(&self) -> usize {
        0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Huc1CartridgeSaveState {
    ram: Option<Vec<u8>>,
    io_mode: Huc1IoMode,
    rom_bank: u8,
    ram_bank: u8,
    ir_emitter_on: bool,
    ir_light_detected: bool,
}

impl Huc1CartridgeSaveState {
    fn dynamic_payload_bytes(&self) -> usize {
        optional_bytes_len(&self.ram)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Huc3CartridgeSaveState {
    ram: Vec<u8>,
    select_mode: Huc3SelectMode,
    rom_bank: u8,
    ram_bank: u8,
    access_address: u8,
    mailbox: Huc3Mailbox,
    #[serde(with = "serde_big_array::BigArray")]
    mcu_ram: [u8; HUC3_MCU_RAM_NIBBLE_COUNT],
    rtc: Huc3RtcState,
    ir_emitter_on: bool,
    ir_light_detected: bool,
    last_control_write: Option<u8>,
    last_unsupported_command: Option<u8>,
    last_unsupported_argument: Option<u8>,
}

impl Huc3CartridgeSaveState {
    fn dynamic_payload_bytes(&self) -> usize {
        self.ram.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Mbc1CartridgeSaveState {
    ram: Option<Vec<u8>>,
    ram_enabled: bool,
    rom_bank_low5: u8,
    secondary_bank: u8,
    banking_mode: u8,
}

impl Mbc1CartridgeSaveState {
    fn dynamic_payload_bytes(&self) -> usize {
        optional_bytes_len(&self.ram)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Mbc2CartridgeSaveState {
    #[serde(with = "serde_big_array::BigArray")]
    ram_nibbles: [u8; MBC2_RAM_CELL_COUNT],
    ram_enabled: bool,
    rom_bank_low4: u8,
}

impl Mbc2CartridgeSaveState {
    fn dynamic_payload_bytes(&self) -> usize {
        0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Mbc3CartridgeSaveState {
    ram: Option<Vec<u8>>,
    ram_rtc_enabled: bool,
    rom_bank: u8,
    ram_or_rtc_select: Mbc3RamRtcSelect,
    rtc_live: Mbc3RtcState,
    rtc_latched: Mbc3RtcState,
    rtc_latched_valid: bool,
    rtc_latch_armed: bool,
    #[serde(default)]
    rtc_subsecond_ticks: u16,
    rtc_access_ready_at: Option<TCycle>,
}

impl Mbc3CartridgeSaveState {
    fn dynamic_payload_bytes(&self) -> usize {
        optional_bytes_len(&self.ram)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Mbc5CartridgeSaveState {
    ram: Option<Vec<u8>>,
    ram_enabled: bool,
    rom_bank_low8: u8,
    rom_bank_high1: u8,
    ram_bank_raw: u8,
    rumble_on: bool,
}

impl Mbc5CartridgeSaveState {
    fn dynamic_payload_bytes(&self) -> usize {
        optional_bytes_len(&self.ram)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Mbc7CartridgeSaveState {
    eeprom: Vec<u8>,
    ram_enabled: bool,
    sensor_eeprom_enabled: bool,
    rom_bank: u8,
    accelerometer_input: Mbc7AccelerometerInput,
    accelerometer_latch_armed: bool,
    latched_x: u16,
    latched_y: u16,
    eeprom_pins: Mbc7EepromPins,
    eeprom_command: Mbc7EepromCommand,
    eeprom_write_enabled: bool,
}

impl Mbc7CartridgeSaveState {
    fn dynamic_payload_bytes(&self) -> usize {
        self.eeprom.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct PocketCameraCartridgeSaveState {
    ram: Vec<u8>,
    ram_enabled: bool,
    rom_bank: u8,
    ram_bank_or_register_select: u8,
    #[serde(with = "serde_big_array::BigArray")]
    registers: [u8; POCKET_CAMERA_REGISTER_COUNT],
    host_frame: Vec<u8>,
    capture_state: PocketCameraCaptureState,
}

impl PocketCameraCartridgeSaveState {
    fn dynamic_payload_bytes(&self) -> usize {
        self.ram
            .len()
            .saturating_add(self.host_frame.len())
            .saturating_add(self.capture_state.dynamic_payload_bytes())
    }
}

impl PocketCameraCaptureState {
    fn dynamic_payload_bytes(&self) -> usize {
        match self {
            Self::Idle => 0,
            Self::Working {
                ready_at: _,
                staged_tiles,
            }
            | Self::Paused {
                remaining_t_cycles: _,
                staged_tiles,
            } => staged_tiles.len().saturating_mul(mem::size_of::<u8>()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CartridgeSnapshot {
    pub state: CartridgeSlotState,
    pub rtc_access_ready_at: Option<TCycle>,
    pub camera_capture_ready_at: Option<TCycle>,
    pub camera_registers_selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CartridgeRtcRegister {
    Seconds,
    Minutes,
    Hours,
    DayLow,
    DayHigh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CartridgeExternalTarget {
    NoDevice,
    LinearRam,
    BankedRam {
        bank: u8,
    },
    Mbc2InternalRam,
    IrRegister,
    Huc3CommandMailbox,
    Huc3ResponseMailbox,
    Huc3Semaphore,
    Huc3InvalidSelector(u8),
    RtcRegister(CartridgeRtcRegister),
    ReservedSelector(u8),
    Mbc7AccelerometerLatchReset,
    Mbc7AccelerometerLatchCommit,
    Mbc7AccelerometerAxis {
        axis: Mbc7AccelerometerAxis,
        byte: Mbc7AccelerometerByte,
    },
    Mbc7FixedRegister {
        value: u8,
    },
    Mbc7EepromSerial,
    Mbc7ReservedRegister {
        selector: u8,
    },
    PocketCameraRegister {
        offset: u8,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Mbc7AccelerometerAxis {
    X,
    Y,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Mbc7AccelerometerByte {
    Low,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CartridgeExternalAvailability {
    Accessible,
    Disabled,
    Absent,
    Reserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CartridgeExternalReadBehavior {
    Storage,
    InfraredSensor,
    OpenBus,
    Huc3MailboxResponse,
    Huc3SemaphoreReady,
    RtcLatched,
    Mbc7Accelerometer,
    Mbc7EepromSerial,
    FallbackValue(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CartridgeExternalWriteBehavior {
    Storage,
    InfraredTransmitter,
    Huc3MailboxCommandArgument,
    Huc3SemaphoreControl,
    RtcLive,
    Mbc7AccelerometerLatch,
    Mbc7EepromSerial,
    Ignored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CartridgeExternalAccessInfo {
    address: u16,
    target: CartridgeExternalTarget,
    availability: CartridgeExternalAvailability,
    read_behavior: CartridgeExternalReadBehavior,
    write_behavior: CartridgeExternalWriteBehavior,
    rtc_access_ready_at: Option<TCycle>,
}

impl CartridgeExternalAccessInfo {
    pub const fn new(
        address: u16,
        target: CartridgeExternalTarget,
        availability: CartridgeExternalAvailability,
        read_behavior: CartridgeExternalReadBehavior,
        write_behavior: CartridgeExternalWriteBehavior,
    ) -> Self {
        Self {
            address,
            target,
            availability,
            read_behavior,
            write_behavior,
            rtc_access_ready_at: None,
        }
    }

    pub const fn no_device(address: u16) -> Self {
        Self::new(
            address,
            CartridgeExternalTarget::NoDevice,
            CartridgeExternalAvailability::Absent,
            CartridgeExternalReadBehavior::FallbackValue(RAM_ABSENT_READ_VALUE),
            CartridgeExternalWriteBehavior::Ignored,
        )
    }

    pub const fn address(self) -> u16 {
        self.address
    }

    pub const fn target(self) -> CartridgeExternalTarget {
        self.target
    }

    pub const fn availability(self) -> CartridgeExternalAvailability {
        self.availability
    }

    pub const fn read_behavior(self) -> CartridgeExternalReadBehavior {
        self.read_behavior
    }

    pub const fn write_behavior(self) -> CartridgeExternalWriteBehavior {
        self.write_behavior
    }

    pub const fn rtc_access_ready_at(self) -> Option<TCycle> {
        self.rtc_access_ready_at
    }

    pub const fn with_rtc_access_ready_at(mut self, rtc_access_ready_at: Option<TCycle>) -> Self {
        self.rtc_access_ready_at = rtc_access_ready_at;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CartridgeRamPayloadKind {
    Linear { byte_len: usize },
    Mbc2Nibbles { cell_count: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CartridgePersistenceProfile {
    None,
    NonPersistentRam { ram: CartridgeRamPayloadKind },
    PersistentRam { ram: CartridgeRamPayloadKind },
    PersistentRtc,
    PersistentRamAndRtc { ram: CartridgeRamPayloadKind },
    PersistentEeprom { byte_len: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CartridgePersistenceMetadata {
    pub has_battery: bool,
    pub has_rtc: bool,
    pub profile: CartridgePersistenceProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Mbc3RtcPersistentState {
    pub seconds: u8,
    pub minutes: u8,
    pub hours: u8,
    pub day_counter: u16,
    pub halt: bool,
    pub carry: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Huc3RtcPersistentState {
    pub current_minutes_of_day: u16,
    pub current_days: u16,
    pub current_subminute_seconds: u8,
    pub event_minutes_of_day: u16,
    pub event_days: u16,
}

// Persist the full mapper-owned backing store shape explicitly, including the
// MBC2 nibble array, instead of hiding those semantics behind ad hoc packing.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PersistentCartState {
    None,
    NoMbcRam {
        ram: Vec<u8>,
    },
    Mmm01Ram {
        ram: Vec<u8>,
    },
    Huc1Ram {
        ram: Vec<u8>,
    },
    Huc3 {
        ram: Vec<u8>,
        #[serde(with = "serde_big_array::BigArray")]
        mcu_ram: [u8; HUC3_MCU_RAM_NIBBLE_COUNT],
        rtc: Huc3RtcPersistentState,
        rom_bank: u8,
        ram_bank: u8,
        select_mode: u8,
        access_address: u8,
        mailbox_command: u8,
        mailbox_argument: u8,
        last_response_nybble: u8,
        semaphore_ready: bool,
        ir_emitter_on: bool,
        ir_light_detected: bool,
        last_control_write: Option<u8>,
        last_unsupported_command: Option<u8>,
        last_unsupported_argument: Option<u8>,
    },
    Mbc1Ram {
        ram: Vec<u8>,
    },
    Mbc2Ram {
        #[serde(with = "serde_big_array::BigArray")]
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
    Mbc7Eeprom {
        eeprom: Vec<u8>,
    },
    PocketCameraRam {
        ram: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CartridgePersistentStateError {
    KindMismatch {
        expected: &'static str,
        actual: &'static str,
    },
    RamLengthMismatch {
        expected: usize,
        actual: usize,
    },
    EepromLengthMismatch {
        expected: usize,
        actual: usize,
    },
    InvalidMbc2NibbleValue {
        index: usize,
        value: u8,
    },
    InvalidHuc3NibbleValue {
        index: usize,
        value: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PocketCameraFrame {
    pub width: u16,
    pub height: u16,
    pub grayscale_pixels: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Mbc7AccelerometerInput {
    pub x_raw: u16,
    pub y_raw: u16,
}

impl Mbc7AccelerometerInput {
    pub const fn neutral() -> Self {
        Self {
            x_raw: MBC7_ACCELEROMETER_NEUTRAL_VALUE,
            y_raw: MBC7_ACCELEROMETER_NEUTRAL_VALUE,
        }
    }

    pub const fn from_raw(x_raw: u16, y_raw: u16) -> Self {
        Self { x_raw, y_raw }
    }

    pub fn from_milli_g(x_milli_g: i16, y_milli_g: i16) -> Self {
        Self {
            x_raw: mbc7_accelerometer_raw_from_milli_g(x_milli_g),
            y_raw: mbc7_accelerometer_raw_from_milli_g(y_milli_g),
        }
    }
}

impl Default for Mbc7AccelerometerInput {
    fn default() -> Self {
        Self::neutral()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Mbc7AccelerometerError {
    UnsupportedCartridge,
}

fn mbc7_accelerometer_raw_from_milli_g(milli_g: i16) -> u16 {
    let delta = (i32::from(milli_g) * MBC7_ACCELEROMETER_DELTA_PER_G) / 1000;
    (i32::from(MBC7_ACCELEROMETER_NEUTRAL_VALUE) + delta).clamp(0, u16::MAX as i32) as u16
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PocketCameraFrameError {
    UnsupportedCartridge,
    InvalidDimensions {
        width: u16,
        height: u16,
        pixel_len: usize,
    },
}

#[cfg(test)]
mod tests;
