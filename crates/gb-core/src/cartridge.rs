use crate::model::{
    CompatibilityPolicy, DiagnosticPolicy, ExecutionMode, HeuristicPolicy, ValidationPolicy,
};
use crate::scheduler::CycleContext;

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

impl RomSizeInfo {
    pub const fn decode(raw_code: u8) -> Self {
        match raw_code {
            0x00..=0x08 => {
                let bank_count = 2usize << raw_code;
                Self {
                    raw_code,
                    decoded_bytes: Some(16 * 1024 * bank_count),
                    bank_count: Some(bank_count),
                }
            }
            0x52 => Self {
                raw_code,
                decoded_bytes: Some(72 * 16 * 1024),
                bank_count: Some(72),
            },
            0x53 => Self {
                raw_code,
                decoded_bytes: Some(80 * 16 * 1024),
                bank_count: Some(80),
            },
            0x54 => Self {
                raw_code,
                decoded_bytes: Some(96 * 16 * 1024),
                bank_count: Some(96),
            },
            _ => Self {
                raw_code,
                decoded_bytes: None,
                bank_count: None,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RamSizeInfo {
    pub raw_code: u8,
    pub decoded_bytes: Option<usize>,
    pub bank_count: Option<usize>,
}

impl RamSizeInfo {
    pub const fn decode(raw_code: u8) -> Self {
        match raw_code {
            0x00 => Self {
                raw_code,
                decoded_bytes: Some(0),
                bank_count: Some(0),
            },
            0x01 => Self {
                raw_code,
                decoded_bytes: Some(2 * 1024),
                bank_count: Some(1),
            },
            0x02 => Self {
                raw_code,
                decoded_bytes: Some(8 * 1024),
                bank_count: Some(1),
            },
            0x03 => Self {
                raw_code,
                decoded_bytes: Some(32 * 1024),
                bank_count: Some(4),
            },
            0x04 => Self {
                raw_code,
                decoded_bytes: Some(128 * 1024),
                bank_count: Some(16),
            },
            0x05 => Self {
                raw_code,
                decoded_bytes: Some(64 * 1024),
                bank_count: Some(8),
            },
            _ => Self {
                raw_code,
                decoded_bytes: None,
                bank_count: None,
            },
        }
    }
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

impl Mbc3RtcPersistentState {
    pub fn apply_elapsed_seconds(&mut self, elapsed_seconds: u64) {
        advance_mbc3_rtc_fields(
            &mut self.seconds,
            &mut self.minutes,
            &mut self.hours,
            &mut self.day_counter,
            self.halt,
            &mut self.carry,
            elapsed_seconds,
        );
    }
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

impl CartridgeHeader {
    pub fn parse(rom_bytes: &[u8]) -> Result<Self, CartridgeHeaderParseError> {
        if rom_bytes.len() < HEADER_MINIMUM_ROM_LEN {
            return Err(CartridgeHeaderParseError::ImageTooSmall {
                actual_size: rom_bytes.len(),
                minimum_size: HEADER_MINIMUM_ROM_LEN,
            });
        }

        let mut entry_point = [0; ENTRY_POINT_LEN];
        entry_point
            .copy_from_slice(&rom_bytes[ENTRY_POINT_START..ENTRY_POINT_START + ENTRY_POINT_LEN]);

        let mut nintendo_logo = [0; NINTENDO_LOGO_LEN];
        nintendo_logo.copy_from_slice(
            &rom_bytes[NINTENDO_LOGO_START..NINTENDO_LOGO_START + NINTENDO_LOGO_LEN],
        );

        let title_bytes = &rom_bytes[TITLE_START..TITLE_END_INCLUSIVE];
        let title_len = title_bytes
            .iter()
            .position(|&byte| byte == 0 || byte == 0xFF)
            .unwrap_or(title_bytes.len());
        let title = String::from_utf8_lossy(&title_bytes[..title_len]).to_string();

        Ok(Self {
            entry_point,
            nintendo_logo,
            title,
            cgb_flag: decode_cgb_flag(rom_bytes[CGB_FLAG_ADDRESS]),
            sgb_flag: decode_sgb_flag(rom_bytes[SGB_FLAG_ADDRESS]),
            cartridge_type: rom_bytes[CARTRIDGE_TYPE_ADDRESS],
            rom_size: RomSizeInfo::decode(rom_bytes[ROM_SIZE_ADDRESS]),
            ram_size: RamSizeInfo::decode(rom_bytes[RAM_SIZE_ADDRESS]),
            header_checksum: rom_bytes[HEADER_CHECKSUM_ADDRESS],
        })
    }
}

impl CartridgeClassification {
    pub const fn classify(raw_type: u8) -> Self {
        match raw_type {
            0x00 => supported(raw_type, "ROM ONLY", SupportedCartridgeFamily::NoMbc),
            0x08 => supported(raw_type, "ROM+RAM", SupportedCartridgeFamily::NoMbc),
            0x09 => supported(raw_type, "ROM+RAM+BATTERY", SupportedCartridgeFamily::NoMbc),
            0x01 => supported(raw_type, "MBC1", SupportedCartridgeFamily::Mbc1),
            0x02 => supported(raw_type, "MBC1+RAM", SupportedCartridgeFamily::Mbc1),
            0x03 => supported(raw_type, "MBC1+RAM+BATTERY", SupportedCartridgeFamily::Mbc1),
            0x05 => supported(raw_type, "MBC2", SupportedCartridgeFamily::Mbc2),
            0x06 => supported(raw_type, "MBC2+BATTERY", SupportedCartridgeFamily::Mbc2),
            0x0F => supported(
                raw_type,
                "MBC3+TIMER+BATTERY",
                SupportedCartridgeFamily::Mbc3,
            ),
            0x10 => supported(
                raw_type,
                "MBC3+TIMER+RAM+BATTERY",
                SupportedCartridgeFamily::Mbc3,
            ),
            0x11 => supported(raw_type, "MBC3", SupportedCartridgeFamily::Mbc3),
            0x12 => supported(raw_type, "MBC3+RAM", SupportedCartridgeFamily::Mbc3),
            0x13 => supported(raw_type, "MBC3+RAM+BATTERY", SupportedCartridgeFamily::Mbc3),
            0x19 => supported(raw_type, "MBC5", SupportedCartridgeFamily::Mbc5),
            0x1A => supported(raw_type, "MBC5+RAM", SupportedCartridgeFamily::Mbc5),
            0x1B => supported(raw_type, "MBC5+RAM+BATTERY", SupportedCartridgeFamily::Mbc5),
            0x1C => supported(raw_type, "MBC5+RUMBLE", SupportedCartridgeFamily::Mbc5),
            0x1D => supported(raw_type, "MBC5+RUMBLE+RAM", SupportedCartridgeFamily::Mbc5),
            0x1E => supported(
                raw_type,
                "MBC5+RUMBLE+RAM+BATTERY",
                SupportedCartridgeFamily::Mbc5,
            ),
            0x0B => unsupported(
                raw_type,
                "MMM01",
                UnsupportedCartridgeCategory::DocumentedButUnsupported,
                "MMM01 is a documented multicart family reserved for later support",
            ),
            0x0C => unsupported(
                raw_type,
                "MMM01+RAM",
                UnsupportedCartridgeCategory::DocumentedButUnsupported,
                "MMM01 is a documented multicart family reserved for later support",
            ),
            0x0D => unsupported(
                raw_type,
                "MMM01+RAM+BATTERY",
                UnsupportedCartridgeCategory::DocumentedButUnsupported,
                "MMM01 is a documented multicart family reserved for later support",
            ),
            0x20 => unsupported(
                raw_type,
                "MBC6",
                UnsupportedCartridgeCategory::DocumentedButUnsupported,
                "MBC6 requires a dedicated cartridge-local implementation",
            ),
            0x22 => unsupported(
                raw_type,
                "MBC7+SENSOR+RUMBLE+RAM+BATTERY",
                UnsupportedCartridgeCategory::DocumentedButUnsupported,
                "MBC7 requires EEPROM and accelerometer behavior that is not implemented yet",
            ),
            0xFC => unsupported(
                raw_type,
                "POCKET CAMERA",
                UnsupportedCartridgeCategory::AccessorySpecialCase,
                "Pocket Camera needs dedicated accessory hardware",
            ),
            0xFD => unsupported(
                raw_type,
                "BANDAI TAMA5",
                UnsupportedCartridgeCategory::AccessorySpecialCase,
                "Bandai TAMA5 needs dedicated accessory hardware",
            ),
            0xFE => unsupported(
                raw_type,
                "HuC-3",
                UnsupportedCartridgeCategory::DocumentedButUnsupported,
                "HuC-3 is a documented special cartridge with its own protocol",
            ),
            0xFF => unsupported(
                raw_type,
                "HuC1+RAM+BATTERY",
                UnsupportedCartridgeCategory::DocumentedButUnsupported,
                "HuC1 needs dedicated IR-capable cartridge behavior",
            ),
            _ => unsupported(
                raw_type,
                "UNKNOWN",
                UnsupportedCartridgeCategory::UnknownCode,
                "The cartridge type code is not recognized",
            ),
        }
    }

    pub const fn raw_type(self) -> u8 {
        self.raw_type
    }

    pub const fn detected_name(self) -> &'static str {
        self.detected_name
    }

    pub const fn selection(self) -> CartridgeSelection {
        self.selection
    }

    pub const fn reason(self) -> &'static str {
        self.reason
    }
}

impl CartridgeLoadReport {
    pub fn cartridge(&self) -> &CartridgeSlot {
        &self.cartridge
    }

    pub fn diagnostics(&self) -> &[CartridgeDiagnostic] {
        &self.diagnostics
    }

    pub fn into_parts(self) -> (CartridgeSlot, Vec<CartridgeDiagnostic>) {
        (self.cartridge, self.diagnostics)
    }
}

impl CartridgeSlot {
    pub fn empty() -> Self {
        Self { device: None }
    }

    pub fn load(
        rom_bytes: Vec<u8>,
        compatibility: &CompatibilityPolicy,
    ) -> Result<CartridgeLoadReport, CartridgeLoadError> {
        let header = CartridgeHeader::parse(&rom_bytes).map_err(CartridgeLoadError::HeaderParse)?;
        let classification = classify_loaded_cartridge(&header, &rom_bytes, compatibility);
        let mut diagnostics = Vec::new();

        match classification.selection() {
            CartridgeSelection::Supported(SupportedCartridgeFamily::NoMbc) => {
                validate_no_mbc(
                    &header,
                    rom_bytes.len(),
                    compatibility,
                    &classification,
                    &mut diagnostics,
                )?;

                let has_battery = matches!(classification.raw_type(), 0x09);
                let has_ram = matches!(classification.raw_type(), 0x08 | 0x09);
                let ram = has_ram.then(|| vec![0; NO_MBC_SUPPORTED_RAM_BYTES]);
                let cartridge = Self {
                    device: Some(CartridgeDevice::NoMbc(NoMbcCartridge {
                        rom: rom_bytes,
                        ram,
                        has_battery,
                        header,
                        classification,
                    })),
                };

                Ok(CartridgeLoadReport {
                    cartridge,
                    diagnostics,
                })
            }
            CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc1) => {
                let layout = validate_mbc1(
                    &header,
                    rom_bytes.len(),
                    compatibility,
                    &classification,
                    &mut diagnostics,
                )?;

                let has_battery = matches!(classification.raw_type(), 0x03);
                let has_ram = matches!(classification.raw_type(), 0x02 | 0x03);
                let ram_len = match layout.wiring {
                    Mbc1Wiring::Standard => header.ram_size.decoded_bytes.unwrap_or(0),
                    Mbc1Wiring::LargeRom => NO_MBC_SUPPORTED_RAM_BYTES,
                };
                let ram = (has_ram && ram_len != 0).then(|| vec![0; ram_len]);
                let cartridge = Self {
                    device: Some(CartridgeDevice::Mbc1(Mbc1Cartridge {
                        rom: rom_bytes,
                        ram,
                        has_battery,
                        header,
                        classification,
                        variant: layout.variant,
                        wiring: layout.wiring,
                        ram_enabled: false,
                        rom_bank_low5: 0,
                        secondary_bank: 0,
                        banking_mode: 0,
                    })),
                };

                Ok(CartridgeLoadReport {
                    cartridge,
                    diagnostics,
                })
            }
            CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc2) => {
                validate_mbc2(
                    &header,
                    rom_bytes.len(),
                    compatibility,
                    &classification,
                    &mut diagnostics,
                )?;

                let has_battery = matches!(classification.raw_type(), 0x06);
                let cartridge = Self {
                    device: Some(CartridgeDevice::Mbc2(Mbc2Cartridge {
                        rom: rom_bytes,
                        ram_nibbles: [0; MBC2_RAM_CELL_COUNT],
                        has_battery,
                        header,
                        classification,
                        ram_enabled: false,
                        rom_bank_low4: 0,
                    })),
                };

                Ok(CartridgeLoadReport {
                    cartridge,
                    diagnostics,
                })
            }
            CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc3) => {
                let layout = validate_mbc3(
                    &header,
                    rom_bytes.len(),
                    compatibility,
                    &classification,
                    &mut diagnostics,
                )?;

                let has_battery = matches!(classification.raw_type(), 0x0F | 0x10 | 0x13);
                let has_rtc = matches!(classification.raw_type(), 0x0F | 0x10);
                let has_ram = matches!(classification.raw_type(), 0x10 | 0x12 | 0x13);
                let ram = (has_ram && header.ram_size.decoded_bytes.unwrap_or(0) != 0)
                    .then(|| vec![0; header.ram_size.decoded_bytes.unwrap_or(0)]);
                let cartridge = Self {
                    device: Some(CartridgeDevice::Mbc3(Mbc3Cartridge {
                        rom: rom_bytes,
                        ram,
                        has_battery,
                        has_rtc,
                        header,
                        classification,
                        variant: layout,
                        ram_rtc_enabled: false,
                        rom_bank: 0,
                        ram_or_rtc_select: Mbc3RamRtcSelect::RamBank(0),
                        rtc_live: Mbc3RtcState::default(),
                        rtc_latched: Mbc3RtcState::default(),
                        rtc_latched_valid: false,
                        rtc_latch_armed: true,
                    })),
                };

                Ok(CartridgeLoadReport {
                    cartridge,
                    diagnostics,
                })
            }
            CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc5) => {
                let variant = validate_mbc5(
                    &header,
                    rom_bytes.len(),
                    compatibility,
                    &classification,
                    &mut diagnostics,
                )?;

                let has_battery = variant.has_battery();
                let has_rumble = variant.has_rumble();
                let ram = (variant.has_ram() && header.ram_size.decoded_bytes.unwrap_or(0) != 0)
                    .then(|| vec![0; header.ram_size.decoded_bytes.unwrap_or(0)]);
                let cartridge = Self {
                    device: Some(CartridgeDevice::Mbc5(Mbc5Cartridge {
                        rom: rom_bytes,
                        ram,
                        has_battery,
                        has_rumble,
                        header,
                        classification,
                        variant,
                        ram_enabled: false,
                        // MBC5 keeps bank 0 valid in the switchable window, but
                        // the power-up mapping still exposes bank 1 until software
                        // writes a different value.
                        rom_bank_low8: 1,
                        rom_bank_high1: 0,
                        ram_bank_raw: 0,
                        rumble_on: false,
                    })),
                };

                Ok(CartridgeLoadReport {
                    cartridge,
                    diagnostics,
                })
            }
            CartridgeSelection::Unsupported(category) => Err(CartridgeLoadError::Rejected {
                classification,
                execution_mode: compatibility.execution_mode,
                reason: unsupported_load_reason(classification, category),
                diagnostics,
            }),
        }
    }

    pub fn state(&self) -> CartridgeSlotState {
        match self.device {
            None => CartridgeSlotState::Empty,
            Some(CartridgeDevice::NoMbc(_)) => CartridgeSlotState::NoMbc,
            Some(CartridgeDevice::Mbc1(_)) => CartridgeSlotState::Mbc1,
            Some(CartridgeDevice::Mbc2(_)) => CartridgeSlotState::Mbc2,
            Some(CartridgeDevice::Mbc3(_)) => CartridgeSlotState::Mbc3,
            Some(CartridgeDevice::Mbc5(_)) => CartridgeSlotState::Mbc5,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.device.is_none()
    }

    pub fn header(&self) -> Option<&CartridgeHeader> {
        self.device.as_ref().map(CartridgeDevice::header)
    }

    pub fn classification(&self) -> Option<CartridgeClassification> {
        self.device.as_ref().map(CartridgeDevice::classification)
    }

    pub fn read_rom(&self, address: u16) -> u8 {
        self.device
            .as_ref()
            .map_or(RAM_ABSENT_READ_VALUE, |device| device.read_rom(address))
    }

    pub fn write_rom(&mut self, address: u16, value: u8) {
        if let Some(device) = &mut self.device {
            device.write_rom(address, value);
        }
    }

    pub fn read_ram(&self, address: u16) -> u8 {
        self.device
            .as_ref()
            .map_or(RAM_ABSENT_READ_VALUE, |device| device.read_ram(address))
    }

    pub fn write_ram(&mut self, address: u16, value: u8) {
        if let Some(device) = &mut self.device {
            device.write_ram(address, value);
        }
    }

    pub fn snapshot(&self) -> CartridgeSnapshot {
        CartridgeSnapshot {
            state: self.state(),
        }
    }

    pub fn persistence_metadata(&self) -> CartridgePersistenceMetadata {
        self.device.as_ref().map_or(
            CartridgePersistenceMetadata {
                has_battery: false,
                has_rtc: false,
                profile: CartridgePersistenceProfile::None,
            },
            CartridgeDevice::persistence_metadata,
        )
    }

    pub fn persistent_state(&self) -> PersistentCartState {
        self.device
            .as_ref()
            .map_or(PersistentCartState::None, CartridgeDevice::persistent_state)
    }

    pub fn restore_persistent_state(
        &mut self,
        state: &PersistentCartState,
    ) -> Result<(), CartridgePersistentStateError> {
        if let Some(device) = &mut self.device {
            device.restore_persistent_state(state)
        } else if matches!(state, PersistentCartState::None) {
            Ok(())
        } else {
            Err(CartridgePersistentStateError::KindMismatch {
                expected: "None",
                actual: state.kind_name(),
            })
        }
    }

    pub fn rumble_on(&self) -> bool {
        self.device.as_ref().is_some_and(CartridgeDevice::rumble_on)
    }

    pub(crate) fn advance_rtc_seconds(&mut self, seconds: u64) {
        if let Some(device) = &mut self.device {
            device.advance_rtc_seconds(seconds);
        }
    }

    pub fn scheduler_trace_message(&self, context: &CycleContext) -> String {
        format!(
            "t_cycle={} phase={} state={:?}",
            context.t_cycle().get(),
            context.phase(),
            self.state(),
        )
    }
}

impl CartridgeDevice {
    fn header(&self) -> &CartridgeHeader {
        match self {
            Self::NoMbc(cartridge) => &cartridge.header,
            Self::Mbc1(cartridge) => &cartridge.header,
            Self::Mbc2(cartridge) => &cartridge.header,
            Self::Mbc3(cartridge) => &cartridge.header,
            Self::Mbc5(cartridge) => &cartridge.header,
        }
    }

    fn classification(&self) -> CartridgeClassification {
        match self {
            Self::NoMbc(cartridge) => cartridge.classification,
            Self::Mbc1(cartridge) => cartridge.classification,
            Self::Mbc2(cartridge) => cartridge.classification,
            Self::Mbc3(cartridge) => cartridge.classification,
            Self::Mbc5(cartridge) => cartridge.classification,
        }
    }

    fn read_rom(&self, address: u16) -> u8 {
        match self {
            Self::NoMbc(cartridge) => cartridge.read_rom(address),
            Self::Mbc1(cartridge) => cartridge.read_rom(address),
            Self::Mbc2(cartridge) => cartridge.read_rom(address),
            Self::Mbc3(cartridge) => cartridge.read_rom(address),
            Self::Mbc5(cartridge) => cartridge.read_rom(address),
        }
    }

    fn write_rom(&mut self, address: u16, value: u8) {
        match self {
            Self::NoMbc(cartridge) => cartridge.write_rom(address, value),
            Self::Mbc1(cartridge) => cartridge.write_rom(address, value),
            Self::Mbc2(cartridge) => cartridge.write_rom(address, value),
            Self::Mbc3(cartridge) => cartridge.write_rom(address, value),
            Self::Mbc5(cartridge) => cartridge.write_rom(address, value),
        }
    }

    fn read_ram(&self, address: u16) -> u8 {
        match self {
            Self::NoMbc(cartridge) => cartridge.read_ram(address),
            Self::Mbc1(cartridge) => cartridge.read_ram(address),
            Self::Mbc2(cartridge) => cartridge.read_ram(address),
            Self::Mbc3(cartridge) => cartridge.read_ram(address),
            Self::Mbc5(cartridge) => cartridge.read_ram(address),
        }
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        match self {
            Self::NoMbc(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc1(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc2(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc3(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc5(cartridge) => cartridge.write_ram(address, value),
        }
    }

    fn advance_rtc_seconds(&mut self, seconds: u64) {
        match self {
            Self::NoMbc(_) | Self::Mbc1(_) | Self::Mbc2(_) | Self::Mbc5(_) => {}
            Self::Mbc3(cartridge) => cartridge.advance_rtc_seconds(seconds),
        }
    }

    fn rumble_on(&self) -> bool {
        match self {
            Self::Mbc5(cartridge) => cartridge.rumble_on(),
            Self::NoMbc(_) | Self::Mbc1(_) | Self::Mbc2(_) | Self::Mbc3(_) => false,
        }
    }

    fn persistence_metadata(&self) -> CartridgePersistenceMetadata {
        match self {
            Self::NoMbc(cartridge) => cartridge.persistence_metadata(),
            Self::Mbc1(cartridge) => cartridge.persistence_metadata(),
            Self::Mbc2(cartridge) => cartridge.persistence_metadata(),
            Self::Mbc3(cartridge) => cartridge.persistence_metadata(),
            Self::Mbc5(cartridge) => cartridge.persistence_metadata(),
        }
    }

    fn persistent_state(&self) -> PersistentCartState {
        match self {
            Self::NoMbc(cartridge) => cartridge.persistent_state(),
            Self::Mbc1(cartridge) => cartridge.persistent_state(),
            Self::Mbc2(cartridge) => cartridge.persistent_state(),
            Self::Mbc3(cartridge) => cartridge.persistent_state(),
            Self::Mbc5(cartridge) => cartridge.persistent_state(),
        }
    }

    fn restore_persistent_state(
        &mut self,
        state: &PersistentCartState,
    ) -> Result<(), CartridgePersistentStateError> {
        match self {
            Self::NoMbc(cartridge) => cartridge.restore_persistent_state(state),
            Self::Mbc1(cartridge) => cartridge.restore_persistent_state(state),
            Self::Mbc2(cartridge) => cartridge.restore_persistent_state(state),
            Self::Mbc3(cartridge) => cartridge.restore_persistent_state(state),
            Self::Mbc5(cartridge) => cartridge.restore_persistent_state(state),
        }
    }
}

impl NoMbcCartridge {
    fn read_rom(&self, address: u16) -> u8 {
        self.rom
            .get(address as usize)
            .copied()
            .unwrap_or(RAM_ABSENT_READ_VALUE)
    }

    fn write_rom(&mut self, _address: u16, _value: u8) {}

    fn read_ram(&self, address: u16) -> u8 {
        self.ram.as_ref().map_or(RAM_ABSENT_READ_VALUE, |ram| {
            ram.get((address - 0xA000) as usize)
                .copied()
                .unwrap_or(RAM_ABSENT_READ_VALUE)
        })
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        if let Some(ram) = &mut self.ram
            && let Some(byte) = ram.get_mut((address - 0xA000) as usize)
        {
            *byte = value;
        }
    }

    #[allow(dead_code)]
    fn has_battery(&self) -> bool {
        self.has_battery
    }

    fn persistence_metadata(&self) -> CartridgePersistenceMetadata {
        let profile = match self.ram.as_ref() {
            Some(ram) if self.has_battery => CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: ram.len(),
                },
            },
            Some(ram) => CartridgePersistenceProfile::NonPersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: ram.len(),
                },
            },
            None => CartridgePersistenceProfile::None,
        };

        CartridgePersistenceMetadata {
            has_battery: self.has_battery,
            has_rtc: false,
            profile,
        }
    }

    fn persistent_state(&self) -> PersistentCartState {
        if self.has_battery {
            self.ram
                .as_ref()
                .map(|ram| PersistentCartState::NoMbcRam { ram: ram.clone() })
                .unwrap_or(PersistentCartState::None)
        } else {
            PersistentCartState::None
        }
    }

    fn restore_persistent_state(
        &mut self,
        state: &PersistentCartState,
    ) -> Result<(), CartridgePersistentStateError> {
        match (self.has_battery, self.ram.as_mut(), state) {
            (false, _, PersistentCartState::None) | (true, None, PersistentCartState::None) => {
                Ok(())
            }
            (true, Some(ram), PersistentCartState::NoMbcRam { ram: persisted_ram }) => {
                if ram.len() != persisted_ram.len() {
                    return Err(CartridgePersistentStateError::RamLengthMismatch {
                        expected: ram.len(),
                        actual: persisted_ram.len(),
                    });
                }
                ram.copy_from_slice(persisted_ram);
                Ok(())
            }
            (true, Some(_), other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "NoMbcRam",
                actual: other.kind_name(),
            }),
            (false, _, other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "None",
                actual: other.kind_name(),
            }),
            (true, None, other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "None",
                actual: other.kind_name(),
            }),
        }
    }
}

impl Mbc1Cartridge {
    fn read_rom(&self, address: u16) -> u8 {
        let address = address as usize;
        let bank_count = self.header.rom_size.bank_count.unwrap_or(0);

        let rom_index = if address < 0x4000 {
            let bank = self.effective_low_rom_bank(bank_count);
            bank * 0x4000 + address
        } else {
            let bank = self.effective_high_rom_bank(bank_count);
            bank * 0x4000 + (address - 0x4000)
        };

        self.rom
            .get(rom_index)
            .copied()
            .unwrap_or(RAM_ABSENT_READ_VALUE)
    }

    fn write_rom(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1FFF => {
                self.ram_enabled = value & 0x0F == 0x0A;
            }
            0x2000..=0x3FFF => {
                self.rom_bank_low5 = value & 0x1F;
            }
            0x4000..=0x5FFF => {
                self.secondary_bank = value & 0x03;
            }
            0x6000..=0x7FFF => {
                self.banking_mode = value & 0x01;
            }
            _ => {}
        }
    }

    fn read_ram(&self, address: u16) -> u8 {
        if !self.ram_enabled {
            return RAM_ABSENT_READ_VALUE;
        }

        self.ram.as_ref().map_or(RAM_ABSENT_READ_VALUE, |ram| {
            ram.get(self.effective_ram_offset(address))
                .copied()
                .unwrap_or(RAM_ABSENT_READ_VALUE)
        })
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        if !self.ram_enabled {
            return;
        }

        let offset = self.effective_ram_offset(address);
        if let Some(ram) = &mut self.ram
            && let Some(byte) = ram.get_mut(offset)
        {
            *byte = value;
        }
    }

    fn effective_high_rom_bank(&self, bank_count: usize) -> usize {
        let raw_low5 = self.rom_bank_low5 & 0x1F;
        let translated_low5 = if raw_low5 == 0 { 1 } else { raw_low5 } as usize;

        if bank_count == 0 {
            return 0;
        }

        if self.variant == Mbc1Variant::Mbc1M {
            let raw_bank = ((self.secondary_bank as usize) << 4) | (translated_low5 & 0x0F);
            return raw_bank % bank_count;
        }

        match self.wiring {
            Mbc1Wiring::Standard => translated_low5 % bank_count,
            Mbc1Wiring::LargeRom => {
                let raw_bank = ((self.secondary_bank as usize) << 5) | translated_low5;
                raw_bank % bank_count
            }
        }
    }

    fn effective_low_rom_bank(&self, bank_count: usize) -> usize {
        if bank_count == 0 {
            return 0;
        }

        if self.variant == Mbc1Variant::Mbc1M {
            return if self.banking_mode == 0 {
                0
            } else {
                ((self.secondary_bank as usize) << 4) % bank_count
            };
        }

        match self.wiring {
            Mbc1Wiring::Standard => 0,
            Mbc1Wiring::LargeRom => {
                if self.banking_mode == 0 {
                    0
                } else {
                    ((self.secondary_bank as usize) << 5) % bank_count
                }
            }
        }
    }

    fn effective_ram_offset(&self, address: u16) -> usize {
        let base_offset = (address - 0xA000) as usize;
        let ram_bank_count = self.header.ram_size.bank_count.unwrap_or(0).max(1);

        if self.variant == Mbc1Variant::Mbc1M {
            return base_offset;
        }

        match self.wiring {
            Mbc1Wiring::Standard => {
                let bank = if self.banking_mode == 0 {
                    0
                } else {
                    (self.secondary_bank & 0x03) as usize
                };
                (bank % ram_bank_count) * 0x2000 + base_offset
            }
            Mbc1Wiring::LargeRom => base_offset,
        }
    }

    #[allow(dead_code)]
    fn has_battery(&self) -> bool {
        self.has_battery
    }

    fn persistence_metadata(&self) -> CartridgePersistenceMetadata {
        let profile = match self.ram.as_ref() {
            Some(ram) if self.has_battery => CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: ram.len(),
                },
            },
            Some(ram) => CartridgePersistenceProfile::NonPersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: ram.len(),
                },
            },
            None => CartridgePersistenceProfile::None,
        };

        CartridgePersistenceMetadata {
            has_battery: self.has_battery,
            has_rtc: false,
            profile,
        }
    }

    fn persistent_state(&self) -> PersistentCartState {
        if self.has_battery {
            self.ram
                .as_ref()
                .map(|ram| PersistentCartState::Mbc1Ram { ram: ram.clone() })
                .unwrap_or(PersistentCartState::None)
        } else {
            PersistentCartState::None
        }
    }

    fn restore_persistent_state(
        &mut self,
        state: &PersistentCartState,
    ) -> Result<(), CartridgePersistentStateError> {
        match (self.has_battery, self.ram.as_mut(), state) {
            (false, _, PersistentCartState::None) | (true, None, PersistentCartState::None) => {
                Ok(())
            }
            (true, Some(ram), PersistentCartState::Mbc1Ram { ram: persisted_ram }) => {
                if ram.len() != persisted_ram.len() {
                    return Err(CartridgePersistentStateError::RamLengthMismatch {
                        expected: ram.len(),
                        actual: persisted_ram.len(),
                    });
                }
                ram.copy_from_slice(persisted_ram);
                Ok(())
            }
            (true, Some(_), other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "Mbc1Ram",
                actual: other.kind_name(),
            }),
            (false, _, other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "None",
                actual: other.kind_name(),
            }),
            (true, None, other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "None",
                actual: other.kind_name(),
            }),
        }
    }
}

impl Mbc2Cartridge {
    fn read_rom(&self, address: u16) -> u8 {
        let address = address as usize;
        let bank_count = self.header.rom_size.bank_count.unwrap_or(0);

        let rom_index = if address < 0x4000 {
            address
        } else {
            let bank = self.effective_high_rom_bank(bank_count);
            bank * 0x4000 + (address - 0x4000)
        };

        self.rom
            .get(rom_index)
            .copied()
            .unwrap_or(RAM_ABSENT_READ_VALUE)
    }

    fn write_rom(&mut self, address: u16, value: u8) {
        if address > 0x3FFF {
            return;
        }

        if address & 0x0100 == 0 {
            self.ram_enabled = value & 0x0F == 0x0A;
        } else {
            self.rom_bank_low4 = value & 0x0F;
        }
    }

    fn read_ram(&self, address: u16) -> u8 {
        if !self.ram_enabled {
            return RAM_ABSENT_READ_VALUE;
        }

        MBC2_RAM_READ_HIGH_NIBBLE | self.ram_nibbles[self.ram_index(address)]
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        if !self.ram_enabled {
            return;
        }

        let index = self.ram_index(address);
        self.ram_nibbles[index] = value & 0x0F;
    }

    fn effective_high_rom_bank(&self, bank_count: usize) -> usize {
        let raw_low4 = self.rom_bank_low4 & 0x0F;
        let translated_low4 = if raw_low4 == 0 { 1 } else { raw_low4 } as usize;

        if bank_count == 0 {
            return 0;
        }

        translated_low4 % bank_count
    }

    fn ram_index(&self, address: u16) -> usize {
        (address as usize - 0xA000) & MBC2_RAM_ADDRESS_MASK
    }

    #[allow(dead_code)]
    fn has_battery(&self) -> bool {
        self.has_battery
    }

    fn persistence_metadata(&self) -> CartridgePersistenceMetadata {
        CartridgePersistenceMetadata {
            has_battery: self.has_battery,
            has_rtc: false,
            profile: if self.has_battery {
                CartridgePersistenceProfile::PersistentRam {
                    ram: CartridgeRamPayloadKind::Mbc2Nibbles {
                        cell_count: MBC2_RAM_CELL_COUNT,
                    },
                }
            } else {
                CartridgePersistenceProfile::NonPersistentRam {
                    ram: CartridgeRamPayloadKind::Mbc2Nibbles {
                        cell_count: MBC2_RAM_CELL_COUNT,
                    },
                }
            },
        }
    }

    fn persistent_state(&self) -> PersistentCartState {
        if self.has_battery {
            PersistentCartState::Mbc2Ram {
                ram_nibbles: self.ram_nibbles,
            }
        } else {
            PersistentCartState::None
        }
    }

    fn restore_persistent_state(
        &mut self,
        state: &PersistentCartState,
    ) -> Result<(), CartridgePersistentStateError> {
        match (self.has_battery, state) {
            (false, PersistentCartState::None) => Ok(()),
            (true, PersistentCartState::Mbc2Ram { ram_nibbles }) => {
                for (index, value) in ram_nibbles.iter().copied().enumerate() {
                    if value & 0xF0 != 0 {
                        return Err(CartridgePersistentStateError::InvalidMbc2NibbleValue {
                            index,
                            value,
                        });
                    }
                }
                self.ram_nibbles = *ram_nibbles;
                Ok(())
            }
            (true, other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "Mbc2Ram",
                actual: other.kind_name(),
            }),
            (false, other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "None",
                actual: other.kind_name(),
            }),
        }
    }
}

impl From<Mbc3RtcState> for Mbc3RtcPersistentState {
    fn from(value: Mbc3RtcState) -> Self {
        Self {
            seconds: value.seconds,
            minutes: value.minutes,
            hours: value.hours,
            day_counter: value.day_counter,
            halt: value.halt,
            carry: value.carry,
        }
    }
}

impl From<Mbc3RtcPersistentState> for Mbc3RtcState {
    fn from(value: Mbc3RtcPersistentState) -> Self {
        Self {
            seconds: value.seconds,
            minutes: value.minutes,
            hours: value.hours,
            day_counter: value.day_counter,
            halt: value.halt,
            carry: value.carry,
        }
    }
}

impl Mbc3RtcState {
    fn read(self, register: Mbc3RtcRegister) -> u8 {
        match register {
            Mbc3RtcRegister::Seconds => self.seconds,
            Mbc3RtcRegister::Minutes => self.minutes,
            Mbc3RtcRegister::Hours => self.hours,
            Mbc3RtcRegister::DayLow => (self.day_counter & 0x00FF) as u8,
            Mbc3RtcRegister::DayHigh => {
                ((self.day_counter >> 8) as u8 & 0x01)
                    | ((self.halt as u8) << 6)
                    | ((self.carry as u8) << 7)
            }
        }
    }

    fn write(&mut self, register: Mbc3RtcRegister, value: u8) {
        match register {
            Mbc3RtcRegister::Seconds => self.seconds = value & 0x3F,
            Mbc3RtcRegister::Minutes => self.minutes = value & 0x3F,
            Mbc3RtcRegister::Hours => self.hours = value & 0x1F,
            Mbc3RtcRegister::DayLow => {
                self.day_counter = (self.day_counter & 0x0100) | value as u16;
            }
            Mbc3RtcRegister::DayHigh => {
                self.day_counter = (self.day_counter & 0x00FF) | (((value & 0x01) as u16) << 8);
                self.halt = value & 0x40 != 0;
                self.carry = value & 0x80 != 0;
            }
        }
    }

    fn advance_seconds(&mut self, elapsed_seconds: u64) {
        advance_mbc3_rtc_fields(
            &mut self.seconds,
            &mut self.minutes,
            &mut self.hours,
            &mut self.day_counter,
            self.halt,
            &mut self.carry,
            elapsed_seconds,
        );
    }
}

fn advance_mbc3_rtc_fields(
    seconds: &mut u8,
    minutes: &mut u8,
    hours: &mut u8,
    day_counter: &mut u16,
    halt: bool,
    carry: &mut bool,
    elapsed_seconds: u64,
) {
    if halt || elapsed_seconds == 0 {
        return;
    }

    *seconds %= 60;
    *minutes %= 60;
    *hours %= 24;
    *day_counter &= 0x01FF;

    let current_total_seconds = *day_counter as u64 * 86_400
        + *hours as u64 * 3_600
        + *minutes as u64 * 60
        + *seconds as u64;
    let advanced_total_seconds = current_total_seconds + elapsed_seconds;
    let total_days = advanced_total_seconds / 86_400;
    if total_days > 511 {
        *carry = true;
    }

    let wrapped_days = (total_days % 512) as u16;
    let day_seconds = advanced_total_seconds % 86_400;
    *day_counter = wrapped_days;
    *hours = (day_seconds / 3_600) as u8;
    *minutes = ((day_seconds % 3_600) / 60) as u8;
    *seconds = (day_seconds % 60) as u8;
}

impl PersistentCartState {
    fn kind_name(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::NoMbcRam { .. } => "NoMbcRam",
            Self::Mbc1Ram { .. } => "Mbc1Ram",
            Self::Mbc2Ram { .. } => "Mbc2Ram",
            Self::Mbc3Rtc { .. } => "Mbc3Rtc",
            Self::Mbc3Ram { .. } => "Mbc3Ram",
            Self::Mbc3RamRtc { .. } => "Mbc3RamRtc",
            Self::Mbc5Ram { .. } => "Mbc5Ram",
        }
    }
}

impl Mbc3Cartridge {
    fn read_rom(&self, address: u16) -> u8 {
        let address = address as usize;
        let bank_count = self.header.rom_size.bank_count.unwrap_or(0);

        let rom_index = if address < 0x4000 {
            address
        } else {
            let bank = self.effective_rom_bank(bank_count);
            bank * 0x4000 + (address - 0x4000)
        };

        self.rom
            .get(rom_index)
            .copied()
            .unwrap_or(RAM_ABSENT_READ_VALUE)
    }

    fn write_rom(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1FFF => {
                self.ram_rtc_enabled = value & 0x0F == 0x0A;
            }
            0x2000..=0x3FFF => {
                self.rom_bank = value & 0x7F;
            }
            0x4000..=0x5FFF => {
                self.ram_or_rtc_select = Mbc3RamRtcSelect::from_value(value);
            }
            0x6000..=0x7FFF => {
                self.latch_rtc_if_needed(value);
            }
            _ => {}
        }
    }

    fn read_ram(&self, address: u16) -> u8 {
        if !self.ram_rtc_enabled {
            return RAM_ABSENT_READ_VALUE;
        }

        match self.ram_or_rtc_select {
            Mbc3RamRtcSelect::RamBank(raw_bank) => {
                self.ram.as_ref().map_or(RAM_ABSENT_READ_VALUE, |ram| {
                    let offset = self.effective_ram_offset(address, raw_bank);
                    ram.get(offset).copied().unwrap_or(RAM_ABSENT_READ_VALUE)
                })
            }
            Mbc3RamRtcSelect::ReservedSelector(_) => RAM_ABSENT_READ_VALUE,
            Mbc3RamRtcSelect::RtcRegister(register) => {
                if self.has_rtc {
                    self.rtc_latched.read(register)
                } else {
                    RAM_ABSENT_READ_VALUE
                }
            }
        }
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        if !self.ram_rtc_enabled {
            return;
        }

        match self.ram_or_rtc_select {
            Mbc3RamRtcSelect::RamBank(raw_bank) => {
                let offset = self.effective_ram_offset(address, raw_bank);
                if let Some(ram) = &mut self.ram
                    && let Some(byte) = ram.get_mut(offset)
                {
                    *byte = value;
                }
            }
            Mbc3RamRtcSelect::ReservedSelector(_) => {}
            Mbc3RamRtcSelect::RtcRegister(register) => {
                if self.has_rtc {
                    self.rtc_live.write(register, value);
                }
            }
        }
    }

    fn effective_rom_bank(&self, bank_count: usize) -> usize {
        let raw_bank = self.rom_bank & 0x7F;
        let translated_bank = if raw_bank == 0 { 1 } else { raw_bank } as usize;

        if bank_count == 0 {
            return 0;
        }

        translated_bank % bank_count
    }

    fn effective_ram_offset(&self, address: u16, raw_bank: u8) -> usize {
        let base_offset = (address - 0xA000) as usize;
        let bank_count = self.header.ram_size.bank_count.unwrap_or(0).max(1);
        let bank = (raw_bank as usize) % bank_count;
        bank * 0x2000 + base_offset
    }

    fn latch_rtc_if_needed(&mut self, value: u8) {
        if !self.has_rtc {
            self.rtc_latch_armed = value == 0x00;
            return;
        }

        if value == 0x00 {
            self.rtc_latch_armed = true;
            return;
        }

        if self.rtc_latch_armed {
            self.rtc_latched = self.rtc_live;
            self.rtc_latched_valid = true;
        }

        self.rtc_latch_armed = true;
    }

    fn advance_rtc_seconds(&mut self, seconds: u64) {
        if self.has_rtc {
            self.rtc_live.advance_seconds(seconds);
        }
    }

    #[allow(dead_code)]
    fn has_battery(&self) -> bool {
        self.has_battery
    }

    fn persistence_metadata(&self) -> CartridgePersistenceMetadata {
        let ram_kind = self
            .ram
            .as_ref()
            .map(|ram| CartridgeRamPayloadKind::Linear {
                byte_len: ram.len(),
            });
        let profile = match (self.has_battery, self.has_rtc, ram_kind) {
            (true, true, Some(ram)) => CartridgePersistenceProfile::PersistentRamAndRtc { ram },
            (true, true, None) => CartridgePersistenceProfile::PersistentRtc,
            (true, false, Some(ram)) => CartridgePersistenceProfile::PersistentRam { ram },
            (false, false, Some(ram)) => CartridgePersistenceProfile::NonPersistentRam { ram },
            _ => CartridgePersistenceProfile::None,
        };

        CartridgePersistenceMetadata {
            has_battery: self.has_battery,
            has_rtc: self.has_rtc,
            profile,
        }
    }

    fn persistent_state(&self) -> PersistentCartState {
        if !self.has_battery {
            return PersistentCartState::None;
        }

        match (self.ram.as_ref(), self.has_rtc) {
            (Some(ram), true) => PersistentCartState::Mbc3RamRtc {
                ram: ram.clone(),
                rtc: self.rtc_live.into(),
            },
            (Some(ram), false) => PersistentCartState::Mbc3Ram { ram: ram.clone() },
            (None, true) => PersistentCartState::Mbc3Rtc {
                rtc: self.rtc_live.into(),
            },
            (None, false) => PersistentCartState::None,
        }
    }

    fn restore_persistent_state(
        &mut self,
        state: &PersistentCartState,
    ) -> Result<(), CartridgePersistentStateError> {
        match (self.has_battery, self.has_rtc, self.ram.as_mut(), state) {
            (false, _, _, PersistentCartState::None) => Ok(()),
            (
                true,
                true,
                Some(ram),
                PersistentCartState::Mbc3RamRtc {
                    ram: persisted_ram,
                    rtc,
                },
            ) => {
                if ram.len() != persisted_ram.len() {
                    return Err(CartridgePersistentStateError::RamLengthMismatch {
                        expected: ram.len(),
                        actual: persisted_ram.len(),
                    });
                }
                ram.copy_from_slice(persisted_ram);
                self.rtc_live = (*rtc).into();
                Ok(())
            }
            (true, false, Some(ram), PersistentCartState::Mbc3Ram { ram: persisted_ram }) => {
                if ram.len() != persisted_ram.len() {
                    return Err(CartridgePersistentStateError::RamLengthMismatch {
                        expected: ram.len(),
                        actual: persisted_ram.len(),
                    });
                }
                ram.copy_from_slice(persisted_ram);
                Ok(())
            }
            (true, true, None, PersistentCartState::Mbc3Rtc { rtc }) => {
                self.rtc_live = (*rtc).into();
                Ok(())
            }
            (true, false, None, PersistentCartState::None) => Ok(()),
            (true, true, Some(_), other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "Mbc3RamRtc",
                actual: other.kind_name(),
            }),
            (true, false, Some(_), other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "Mbc3Ram",
                actual: other.kind_name(),
            }),
            (true, true, None, other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "Mbc3Rtc",
                actual: other.kind_name(),
            }),
            (true, false, None, other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "None",
                actual: other.kind_name(),
            }),
            (false, _, _, other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "None",
                actual: other.kind_name(),
            }),
        }
    }
}

impl Mbc3RamRtcSelect {
    fn from_value(value: u8) -> Self {
        let low_nibble = value & 0x0F;
        match low_nibble {
            0x00..=0x03 => Self::RamBank(low_nibble),
            0x08 => Self::RtcRegister(Mbc3RtcRegister::Seconds),
            0x09 => Self::RtcRegister(Mbc3RtcRegister::Minutes),
            0x0A => Self::RtcRegister(Mbc3RtcRegister::Hours),
            0x0B => Self::RtcRegister(Mbc3RtcRegister::DayLow),
            0x0C => Self::RtcRegister(Mbc3RtcRegister::DayHigh),
            other => Self::ReservedSelector(other),
        }
    }
}

impl Mbc5Variant {
    fn has_ram(self) -> bool {
        matches!(
            self,
            Self::Ram | Self::RamBattery | Self::RumbleRam | Self::RumbleRamBattery
        )
    }

    fn has_battery(self) -> bool {
        matches!(self, Self::RamBattery | Self::RumbleRamBattery)
    }

    fn has_rumble(self) -> bool {
        matches!(
            self,
            Self::Rumble | Self::RumbleRam | Self::RumbleRamBattery
        )
    }
}

impl Mbc5Cartridge {
    fn read_rom(&self, address: u16) -> u8 {
        let address = address as usize;
        let bank_count = self.header.rom_size.bank_count.unwrap_or(0);

        let rom_index = if address < 0x4000 {
            address
        } else {
            let bank = self.effective_rom_bank(bank_count);
            bank * 0x4000 + (address - 0x4000)
        };

        self.rom
            .get(rom_index)
            .copied()
            .unwrap_or(RAM_ABSENT_READ_VALUE)
    }

    fn write_rom(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1FFF => {
                self.ram_enabled = value & 0x0F == 0x0A;
            }
            0x2000..=0x2FFF => {
                self.rom_bank_low8 = value;
            }
            0x3000..=0x3FFF => {
                self.rom_bank_high1 = value & 0x01;
            }
            0x4000..=0x5FFF => {
                if self.has_rumble {
                    self.rumble_on = value & 0x08 != 0;
                    self.ram_bank_raw = value & 0x07;
                } else {
                    self.ram_bank_raw = value & 0x0F;
                }
            }
            _ => {}
        }
    }

    fn read_ram(&self, address: u16) -> u8 {
        if !self.ram_enabled {
            return RAM_ABSENT_READ_VALUE;
        }

        self.ram.as_ref().map_or(RAM_ABSENT_READ_VALUE, |ram| {
            let offset = self.effective_ram_offset(address);
            ram.get(offset).copied().unwrap_or(RAM_ABSENT_READ_VALUE)
        })
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        if !self.ram_enabled {
            return;
        }

        let offset = self.effective_ram_offset(address);
        if let Some(ram) = &mut self.ram
            && let Some(byte) = ram.get_mut(offset)
        {
            *byte = value;
        }
    }

    fn effective_rom_bank(&self, bank_count: usize) -> usize {
        if bank_count == 0 {
            return 0;
        }

        let raw_bank = ((self.rom_bank_high1 as usize) << 8) | self.rom_bank_low8 as usize;
        raw_bank % bank_count
    }

    fn effective_ram_offset(&self, address: u16) -> usize {
        let base_offset = (address - 0xA000) as usize;
        let bank_count = self.header.ram_size.bank_count.unwrap_or(0).max(1);
        let bank = (self.ram_bank_raw as usize) % bank_count;
        bank * 0x2000 + base_offset
    }

    fn rumble_on(&self) -> bool {
        self.has_rumble && self.rumble_on
    }

    #[allow(dead_code)]
    fn has_battery(&self) -> bool {
        self.has_battery
    }

    fn persistence_metadata(&self) -> CartridgePersistenceMetadata {
        let profile = match self.ram.as_ref() {
            Some(ram) if self.has_battery => CartridgePersistenceProfile::PersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: ram.len(),
                },
            },
            Some(ram) => CartridgePersistenceProfile::NonPersistentRam {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: ram.len(),
                },
            },
            None => CartridgePersistenceProfile::None,
        };

        CartridgePersistenceMetadata {
            has_battery: self.has_battery,
            has_rtc: false,
            profile,
        }
    }

    fn persistent_state(&self) -> PersistentCartState {
        if self.has_battery {
            self.ram
                .as_ref()
                .map(|ram| PersistentCartState::Mbc5Ram { ram: ram.clone() })
                .unwrap_or(PersistentCartState::None)
        } else {
            PersistentCartState::None
        }
    }

    fn restore_persistent_state(
        &mut self,
        state: &PersistentCartState,
    ) -> Result<(), CartridgePersistentStateError> {
        match (self.has_battery, self.ram.as_mut(), state) {
            (false, _, PersistentCartState::None) | (true, None, PersistentCartState::None) => {
                Ok(())
            }
            (true, Some(ram), PersistentCartState::Mbc5Ram { ram: persisted_ram }) => {
                if ram.len() != persisted_ram.len() {
                    return Err(CartridgePersistentStateError::RamLengthMismatch {
                        expected: ram.len(),
                        actual: persisted_ram.len(),
                    });
                }
                ram.copy_from_slice(persisted_ram);
                Ok(())
            }
            (true, Some(_), other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "Mbc5Ram",
                actual: other.kind_name(),
            }),
            (false, _, other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "None",
                actual: other.kind_name(),
            }),
            (true, None, other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "None",
                actual: other.kind_name(),
            }),
        }
    }
}

fn classify_loaded_cartridge(
    header: &CartridgeHeader,
    rom_bytes: &[u8],
    compatibility: &CompatibilityPolicy,
) -> CartridgeClassification {
    if let Some(classification) = classify_planned_variant(header) {
        return classification;
    }

    if let Some(classification) = classify_documented_special_variant(header, rom_bytes) {
        return classification;
    }

    if compatibility.heuristic_policy == HeuristicPolicy::AllowExperimental
        && let Some(classification) = classify_experimental_heuristic(header, rom_bytes)
    {
        return classification;
    }

    CartridgeClassification::classify(header.cartridge_type)
}

fn classify_planned_variant(header: &CartridgeHeader) -> Option<CartridgeClassification> {
    match header.cartridge_type {
        0x0F..=0x13 if header.ram_size.raw_code == 0x05 => Some(unsupported(
            header.cartridge_type,
            "MBC30",
            UnsupportedCartridgeCategory::PlannedVariant,
            "MBC30 is a known MBC3-family variant reserved for later support",
        )),
        _ => None,
    }
}

fn classify_documented_special_variant(
    header: &CartridgeHeader,
    rom_bytes: &[u8],
) -> Option<CartridgeClassification> {
    if is_m161_multicart_signature(header, rom_bytes) {
        return Some(unsupported(
            header.cartridge_type,
            "M161",
            UnsupportedCartridgeCategory::DocumentedButUnsupported,
            "M161 multicart classification came from the explicit Mani 4-in-1 signature path",
        ));
    }

    None
}

fn classify_experimental_heuristic(
    header: &CartridgeHeader,
    rom_bytes: &[u8],
) -> Option<CartridgeClassification> {
    let title_bytes = &rom_bytes[TITLE_START..=TITLE_END_INCLUSIVE];
    let destination_code = rom_bytes
        .get(DESTINATION_CODE_ADDRESS)
        .copied()
        .unwrap_or(0x00);

    if is_mbc1m_multicart_signature(header, rom_bytes) {
        return Some(supported_with_reason(
            header.cartridge_type,
            "MBC1M",
            SupportedCartridgeFamily::Mbc1,
            "MBC1 multicart classification came from an explicit experimental heuristic path",
        ));
    }

    if header.cartridge_type == 0xBE {
        return Some(unsupported(
            header.cartridge_type,
            "BUNG",
            UnsupportedCartridgeCategory::ExperimentalHeuristic,
            "Bung multicart classification came from an explicit experimental heuristic path",
        ));
    }

    if is_ems_multicart_signature(title_bytes, header.cartridge_type, destination_code) {
        return Some(unsupported(
            header.cartridge_type,
            "EMS",
            UnsupportedCartridgeCategory::ExperimentalHeuristic,
            "EMS multicart classification came from an explicit experimental heuristic path",
        ));
    }

    if is_wisdom_tree_signature(
        title_bytes,
        header.cartridge_type,
        header.rom_size.raw_code,
        rom_bytes.len(),
        destination_code,
    ) {
        return Some(unsupported(
            header.cartridge_type,
            "WISDOM TREE",
            UnsupportedCartridgeCategory::ExperimentalHeuristic,
            "Wisdom Tree classification came from an explicit experimental heuristic path",
        ));
    }

    None
}

fn is_mbc1m_multicart_signature(header: &CartridgeHeader, rom_bytes: &[u8]) -> bool {
    if header.cartridge_type != 0x01 {
        return false;
    }
    if header.rom_size.decoded_bytes != Some(1024 * 1024) || rom_bytes.len() != 1024 * 1024 {
        return false;
    }
    if header.ram_size.raw_code != 0x00 {
        return false;
    }

    [0x10usize, 0x20, 0x30].into_iter().all(|bank| {
        let start = bank * 0x4000 + NINTENDO_LOGO_START;
        let end = start + NINTENDO_LOGO_LEN;
        rom_bytes.get(start..end) == Some(header.nintendo_logo.as_slice())
    })
}

fn is_m161_multicart_signature(header: &CartridgeHeader, rom_bytes: &[u8]) -> bool {
    if rom_bytes.len() < M161_KNOWN_SUBTITLE_SET.len() * M161_BANK_BYTES
        || rom_bytes.len() > M161_SUPPORTED_ROM_BYTES_MAX
        || !rom_bytes.len().is_multiple_of(M161_BANK_BYTES)
    {
        return false;
    }

    let mut seen_titles = [false; M161_KNOWN_SUBTITLE_SET.len()];

    for bank_start in (0..rom_bytes.len()).step_by(M161_BANK_BYTES) {
        let bank_logo_start = bank_start + NINTENDO_LOGO_START;
        let bank_logo_end = bank_logo_start + NINTENDO_LOGO_LEN;
        let bank_title_start = bank_start + TITLE_START;
        let bank_title_end = bank_start + TITLE_END_INCLUSIVE;

        let Some(bank_logo) = rom_bytes.get(bank_logo_start..bank_logo_end) else {
            return false;
        };
        let Some(bank_title) = rom_bytes.get(bank_title_start..=bank_title_end) else {
            return false;
        };

        for (title_index, expected_title) in M161_KNOWN_SUBTITLE_SET.iter().enumerate() {
            if !matches_padded_title(bank_title, expected_title) {
                continue;
            }

            if seen_titles[title_index]
                || bank_logo != header.nintendo_logo.as_slice()
                || rom_bytes.get(bank_start + CARTRIDGE_TYPE_ADDRESS).copied() != Some(0x00)
                || rom_bytes.get(bank_start + ROM_SIZE_ADDRESS).copied() != Some(0x00)
                || rom_bytes.get(bank_start + RAM_SIZE_ADDRESS).copied() != Some(0x00)
            {
                return false;
            }

            seen_titles[title_index] = true;
        }
    }

    seen_titles.into_iter().all(|seen| seen)
}

fn is_ems_multicart_signature(title_bytes: &[u8], raw_type: u8, destination_code: u8) -> bool {
    matches_padded_title(title_bytes, b"EMSMENU")
        || matches_padded_title(title_bytes, b"GB16M")
        || (raw_type == 0x1B && destination_code == 0xE1)
}

fn is_wisdom_tree_signature(
    title_bytes: &[u8],
    raw_type: u8,
    rom_size_code: u8,
    actual_rom_size: usize,
    destination_code: u8,
) -> bool {
    matches_padded_title(title_bytes, b"WISDOM TREE")
        || matches_padded_title(title_bytes, b"WISDOM\0TREE")
        || (raw_type == 0x00
            && rom_size_code == 0x00
            && actual_rom_size > NO_MBC_SUPPORTED_ROM_BYTES)
        || (raw_type == 0xC0 && destination_code == 0xD1)
}

fn matches_padded_title(title_bytes: &[u8], expected: &[u8]) -> bool {
    if title_bytes.len() < expected.len() {
        return false;
    }

    title_bytes.starts_with(expected)
        && title_bytes[expected.len()..]
            .iter()
            .all(|&byte| byte == 0x00 || byte == 0xFF)
}

fn validate_no_mbc(
    header: &CartridgeHeader,
    actual_rom_size: usize,
    compatibility: &CompatibilityPolicy,
    classification: &CartridgeClassification,
    diagnostics: &mut Vec<CartridgeDiagnostic>,
) -> Result<(), CartridgeLoadError> {
    let expected_ram_code = match classification.raw_type() {
        0x00 => 0x00,
        0x08 | 0x09 => 0x02,
        _ => unreachable!("non-NoMbc type entered NoMbc validation"),
    };

    if header.ram_size.raw_code != expected_ram_code {
        record_degradable_issue(
            diagnostics,
            compatibility.validation_policy,
            format!(
                "{} expects RAM size code {expected_ram_code:#04X}, but the header declared {:#04X}",
                classification.detected_name(),
                header.ram_size.raw_code
            ),
        )
        .map_err(|message| CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: message,
            diagnostics: diagnostics.clone(),
        })?;
    }

    if header.ram_size.decoded_bytes != Some(expected_ram_code_decompressed(expected_ram_code)) {
        record_degradable_issue(
            diagnostics,
            compatibility.validation_policy,
            format!(
                "{} resolved to an unsupported RAM configuration from code {:#04X}",
                classification.detected_name(),
                header.ram_size.raw_code
            ),
        )
        .map_err(|message| CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: message,
            diagnostics: diagnostics.clone(),
        })?;
    }

    if header.rom_size.raw_code != 0x00 {
        record_degradable_issue(
            diagnostics,
            compatibility.validation_policy,
            format!(
                "{} expects ROM size code 0x00, but the header declared {:#04X}",
                classification.detected_name(),
                header.rom_size.raw_code
            ),
        )
        .map_err(|message| CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: message,
            diagnostics: diagnostics.clone(),
        })?;
    }

    if header.rom_size.decoded_bytes != Some(NO_MBC_SUPPORTED_ROM_BYTES) {
        record_degradable_issue(
            diagnostics,
            compatibility.validation_policy,
            format!(
                "{} expects a 32 KiB ROM declaration, but the header resolved to {:?} bytes",
                classification.detected_name(),
                header.rom_size.decoded_bytes
            ),
        )
        .map_err(|message| CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: message,
            diagnostics: diagnostics.clone(),
        })?;
    }

    if actual_rom_size != NO_MBC_SUPPORTED_ROM_BYTES {
        record_degradable_issue(
            diagnostics,
            compatibility.validation_policy,
            format!(
                "{} expects a 32 KiB image, but the loaded ROM is {} bytes",
                classification.detected_name(),
                actual_rom_size
            ),
        )
        .map_err(|message| CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: message,
            diagnostics: diagnostics.clone(),
        })?;
    }

    if matches!(classification.raw_type(), 0x08 | 0x09) {
        diagnostics.push(CartridgeDiagnostic {
            severity: CartridgeDiagnosticSeverity::Warning,
            message: format!(
                "{} is rare but still treated as a valid No MBC variant",
                classification.detected_name()
            ),
        });
    }

    Ok(())
}

fn validate_mbc1(
    header: &CartridgeHeader,
    actual_rom_size: usize,
    compatibility: &CompatibilityPolicy,
    classification: &CartridgeClassification,
    diagnostics: &mut Vec<CartridgeDiagnostic>,
) -> Result<Mbc1Layout, CartridgeLoadError> {
    let Some(declared_rom_bytes) = header.rom_size.decoded_bytes else {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} declared an unsupported ROM size code {:#04X}",
                classification.detected_name(),
                header.rom_size.raw_code
            ),
            diagnostics: diagnostics.to_vec(),
        });
    };

    if actual_rom_size != declared_rom_bytes {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} expects a {}-byte image, but the loaded ROM is {} bytes",
                classification.detected_name(),
                declared_rom_bytes,
                actual_rom_size
            ),
            diagnostics: diagnostics.to_vec(),
        });
    }

    if classification.detected_name() == "MBC1M" {
        if header.ram_size.raw_code != 0x00 {
            return Err(CartridgeLoadError::Rejected {
                classification: *classification,
                execution_mode: compatibility.execution_mode,
                reason: format!(
                    "{} currently only supports the no-RAM 1 MiB multicart baseline",
                    classification.detected_name()
                ),
                diagnostics: diagnostics.to_vec(),
            });
        }

        if compatibility.diagnostic_policy != DiagnosticPolicy::Quiet {
            diagnostics.push(CartridgeDiagnostic {
                severity: CartridgeDiagnosticSeverity::Warning,
                message: format!(
                    "{} banking was enabled through an explicit experimental multicart heuristic and remains non-oracle",
                    classification.detected_name()
                ),
            });
        }

        return Ok(Mbc1Layout {
            wiring: Mbc1Wiring::LargeRom,
            variant: Mbc1Variant::Mbc1M,
        });
    }

    let wiring = if MBC1_STANDARD_ROM_SIZES.contains(&declared_rom_bytes) {
        Mbc1Wiring::Standard
    } else if MBC1_LARGE_ROM_SIZES.contains(&declared_rom_bytes) {
        Mbc1Wiring::LargeRom
    } else {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} declared a ROM size that is not valid for the current MBC1 baseline: {} bytes",
                classification.detected_name(),
                declared_rom_bytes
            ),
            diagnostics: diagnostics.to_vec(),
        });
    };

    let allowed_ram_codes = match wiring {
        Mbc1Wiring::Standard => [0x00, 0x02, 0x03].as_slice(),
        Mbc1Wiring::LargeRom => [0x00, 0x02].as_slice(),
    };
    if !allowed_ram_codes.contains(&header.ram_size.raw_code) {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} declared RAM size code {:#04X}, which is not valid for the current {:?} MBC1 wiring baseline",
                classification.detected_name(),
                header.ram_size.raw_code,
                wiring
            ),
            diagnostics: diagnostics.to_vec(),
        });
    }

    Ok(Mbc1Layout {
        wiring,
        variant: Mbc1Variant::Standard,
    })
}

fn validate_mbc2(
    header: &CartridgeHeader,
    actual_rom_size: usize,
    compatibility: &CompatibilityPolicy,
    classification: &CartridgeClassification,
    diagnostics: &mut Vec<CartridgeDiagnostic>,
) -> Result<(), CartridgeLoadError> {
    let Some(declared_rom_bytes) = header.rom_size.decoded_bytes else {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} declared an unsupported ROM size code {:#04X}",
                classification.detected_name(),
                header.rom_size.raw_code
            ),
            diagnostics: diagnostics.clone(),
        });
    };

    if actual_rom_size != declared_rom_bytes {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} expects a {}-byte image, but the loaded ROM is {} bytes",
                classification.detected_name(),
                declared_rom_bytes,
                actual_rom_size
            ),
            diagnostics: diagnostics.clone(),
        });
    }

    if declared_rom_bytes > MBC2_SUPPORTED_ROM_BYTES_MAX {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} exceeds the current MBC2 ROM limit of {} bytes with {} bytes",
                classification.detected_name(),
                MBC2_SUPPORTED_ROM_BYTES_MAX,
                declared_rom_bytes
            ),
            diagnostics: diagnostics.clone(),
        });
    }

    if header.ram_size.raw_code != 0x00 {
        record_degradable_issue(
            diagnostics,
            compatibility.validation_policy,
            format!(
                "{} expects RAM size code 0x00 because MBC2 RAM is internal, but the header declared {:#04X}",
                classification.detected_name(),
                header.ram_size.raw_code
            ),
        )
        .map_err(|message| CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: message,
            diagnostics: diagnostics.clone(),
        })?;
    }

    Ok(())
}

fn validate_mbc3(
    header: &CartridgeHeader,
    actual_rom_size: usize,
    compatibility: &CompatibilityPolicy,
    classification: &CartridgeClassification,
    diagnostics: &mut Vec<CartridgeDiagnostic>,
) -> Result<Mbc3Variant, CartridgeLoadError> {
    let Some(declared_rom_bytes) = header.rom_size.decoded_bytes else {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} declared an unsupported ROM size code {:#04X}",
                classification.detected_name(),
                header.rom_size.raw_code
            ),
            diagnostics: diagnostics.clone(),
        });
    };

    if actual_rom_size != declared_rom_bytes {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} expects a {}-byte image, but the loaded ROM is {} bytes",
                classification.detected_name(),
                declared_rom_bytes,
                actual_rom_size
            ),
            diagnostics: diagnostics.clone(),
        });
    }

    if declared_rom_bytes > MBC3_SUPPORTED_ROM_BYTES_MAX {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} exceeds the current MBC3 ROM limit of {} bytes with {} bytes",
                classification.detected_name(),
                MBC3_SUPPORTED_ROM_BYTES_MAX,
                declared_rom_bytes
            ),
            diagnostics: diagnostics.clone(),
        });
    }

    let has_ram = matches!(classification.raw_type(), 0x10 | 0x12 | 0x13);
    if header.ram_size.raw_code == 0x05 {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} with 64 KiB SRAM is reserved for the future MBC30 variant, not standard MBC3",
                classification.detected_name()
            ),
            diagnostics: diagnostics.clone(),
        });
    }

    if has_ram {
        if !matches!(header.ram_size.raw_code, 0x01..=0x03) {
            return Err(CartridgeLoadError::Rejected {
                classification: *classification,
                execution_mode: compatibility.execution_mode,
                reason: format!(
                    "{} declared RAM size code {:#04X}, which is not valid for the current standard MBC3 baseline",
                    classification.detected_name(),
                    header.ram_size.raw_code
                ),
                diagnostics: diagnostics.clone(),
            });
        }
    } else if header.ram_size.raw_code != 0x00 {
        record_degradable_issue(
            diagnostics,
            compatibility.validation_policy,
            format!(
                "{} does not provide external RAM, but the header declared RAM size code {:#04X}",
                classification.detected_name(),
                header.ram_size.raw_code
            ),
        )
        .map_err(|message| CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: message,
            diagnostics: diagnostics.clone(),
        })?;
    }

    Ok(Mbc3Variant::Standard)
}

fn validate_mbc5(
    header: &CartridgeHeader,
    actual_rom_size: usize,
    compatibility: &CompatibilityPolicy,
    classification: &CartridgeClassification,
    diagnostics: &mut Vec<CartridgeDiagnostic>,
) -> Result<Mbc5Variant, CartridgeLoadError> {
    let Some(declared_rom_bytes) = header.rom_size.decoded_bytes else {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} declared an unsupported ROM size code {:#04X}",
                classification.detected_name(),
                header.rom_size.raw_code
            ),
            diagnostics: diagnostics.clone(),
        });
    };

    if actual_rom_size > MBC5_SUPPORTED_ROM_BYTES_MAX {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} exceeds the current MBC5 ROM limit of {} bytes with {} bytes",
                classification.detected_name(),
                MBC5_SUPPORTED_ROM_BYTES_MAX,
                actual_rom_size
            ),
            diagnostics: diagnostics.clone(),
        });
    }

    if actual_rom_size != declared_rom_bytes {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} expects a {}-byte image, but the loaded ROM is {} bytes",
                classification.detected_name(),
                declared_rom_bytes,
                actual_rom_size
            ),
            diagnostics: diagnostics.clone(),
        });
    }

    let variant = match classification.raw_type() {
        0x19 => Mbc5Variant::NoRam,
        0x1A => Mbc5Variant::Ram,
        0x1B => Mbc5Variant::RamBattery,
        0x1C => Mbc5Variant::Rumble,
        0x1D => Mbc5Variant::RumbleRam,
        0x1E => Mbc5Variant::RumbleRamBattery,
        _ => unreachable!("non-MBC5 type entered MBC5 validation"),
    };

    if variant.has_ram() {
        let allowed_ram_codes = if variant.has_rumble() {
            [0x02, 0x03].as_slice()
        } else {
            [0x02, 0x03, 0x04].as_slice()
        };

        if !allowed_ram_codes.contains(&header.ram_size.raw_code) {
            return Err(CartridgeLoadError::Rejected {
                classification: *classification,
                execution_mode: compatibility.execution_mode,
                reason: format!(
                    "{} declared RAM size code {:#04X}, which is not valid for the current {} MBC5 baseline",
                    classification.detected_name(),
                    header.ram_size.raw_code,
                    if variant.has_rumble() {
                        "rumble-capable"
                    } else {
                        "standard"
                    }
                ),
                diagnostics: diagnostics.clone(),
            });
        }
    } else if header.ram_size.raw_code != 0x00 {
        record_degradable_issue(
            diagnostics,
            compatibility.validation_policy,
            format!(
                "{} does not provide external RAM, but the header declared RAM size code {:#04X}",
                classification.detected_name(),
                header.ram_size.raw_code
            ),
        )
        .map_err(|message| CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: message,
            diagnostics: diagnostics.clone(),
        })?;
    }

    Ok(variant)
}

fn record_degradable_issue(
    diagnostics: &mut Vec<CartridgeDiagnostic>,
    validation_policy: ValidationPolicy,
    message: String,
) -> Result<(), String> {
    match validation_policy {
        ValidationPolicy::Strict => Err(message),
        ValidationPolicy::Warn => {
            diagnostics.push(CartridgeDiagnostic {
                severity: CartridgeDiagnosticSeverity::Warning,
                message,
            });
            Ok(())
        }
        ValidationPolicy::Ignore => Ok(()),
    }
}

const fn expected_ram_code_decompressed(code: u8) -> usize {
    match code {
        0x00 => 0,
        0x02 => NO_MBC_SUPPORTED_RAM_BYTES,
        _ => 0,
    }
}

const fn decode_cgb_flag(raw_flag: u8) -> CgbFlag {
    match raw_flag {
        0x00 => CgbFlag::None,
        0x80 => CgbFlag::Supported,
        0xC0 => CgbFlag::Only,
        other => CgbFlag::Unknown(other),
    }
}

const fn decode_sgb_flag(raw_flag: u8) -> SgbFlag {
    match raw_flag {
        0x00 => SgbFlag::None,
        0x03 => SgbFlag::Supported,
        other => SgbFlag::Unknown(other),
    }
}

const fn supported(
    raw_type: u8,
    detected_name: &'static str,
    family: SupportedCartridgeFamily,
) -> CartridgeClassification {
    supported_with_reason(
        raw_type,
        detected_name,
        family,
        "supported cartridge family",
    )
}

const fn supported_with_reason(
    raw_type: u8,
    detected_name: &'static str,
    family: SupportedCartridgeFamily,
    reason: &'static str,
) -> CartridgeClassification {
    CartridgeClassification {
        raw_type,
        detected_name,
        selection: CartridgeSelection::Supported(family),
        reason,
    }
}

const fn unsupported(
    raw_type: u8,
    detected_name: &'static str,
    category: UnsupportedCartridgeCategory,
    reason: &'static str,
) -> CartridgeClassification {
    CartridgeClassification {
        raw_type,
        detected_name,
        selection: CartridgeSelection::Unsupported(category),
        reason,
    }
}

fn unsupported_load_reason(
    classification: CartridgeClassification,
    category: UnsupportedCartridgeCategory,
) -> String {
    match category {
        UnsupportedCartridgeCategory::PlannedVariant => format!(
            "{} is a known reserved variant that is not implemented yet: {}",
            classification.detected_name(),
            classification.reason()
        ),
        UnsupportedCartridgeCategory::ExperimentalHeuristic => format!(
            "{} was identified through an explicit experimental heuristic path and is not implemented: {}",
            classification.detected_name(),
            classification.reason()
        ),
        UnsupportedCartridgeCategory::DocumentedButUnsupported
        | UnsupportedCartridgeCategory::AccessorySpecialCase
        | UnsupportedCartridgeCategory::UnknownCode => format!(
            "{} ({category:?}) is not implemented: {}",
            classification.detected_name(),
            classification.reason()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CompatibilityPolicy, DiagnosticPolicy, HeuristicPolicy, OverridePolicy};

    fn build_test_rom(
        len: usize,
        cartridge_type: u8,
        rom_size_code: u8,
        ram_size_code: u8,
    ) -> Vec<u8> {
        let mut rom = vec![0xFF; len.max(HEADER_MINIMUM_ROM_LEN)];
        rom[ENTRY_POINT_START..ENTRY_POINT_START + ENTRY_POINT_LEN]
            .copy_from_slice(&[0x00, 0xC3, 0x50, 0x01]);
        rom[NINTENDO_LOGO_START..NINTENDO_LOGO_START + NINTENDO_LOGO_LEN]
            .copy_from_slice(&[0xCE; NINTENDO_LOGO_LEN]);
        rom[TITLE_START..TITLE_START + 7].copy_from_slice(b"GBTEST1");
        rom[CGB_FLAG_ADDRESS] = 0x80;
        rom[SGB_FLAG_ADDRESS] = 0x03;
        rom[CARTRIDGE_TYPE_ADDRESS] = cartridge_type;
        rom[ROM_SIZE_ADDRESS] = rom_size_code;
        rom[RAM_SIZE_ADDRESS] = ram_size_code;
        rom
    }

    fn build_banked_mbc1_rom_with_type(
        cartridge_type: u8,
        rom_size_code: u8,
        ram_size_code: u8,
    ) -> Vec<u8> {
        let rom_size = RomSizeInfo::decode(rom_size_code)
            .decoded_bytes
            .expect("test ROM size should decode");
        let bank_count = RomSizeInfo::decode(rom_size_code)
            .bank_count
            .expect("test ROM bank count should decode");
        let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

        for bank in 0..bank_count {
            let start = bank * 0x4000;
            rom[start] = bank as u8;
            rom[start + 0x0100] = bank as u8;
        }

        rom
    }

    fn build_banked_mbc1_rom(rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
        build_banked_mbc1_rom_with_type(0x03, rom_size_code, ram_size_code)
    }

    fn build_m161_signature_rom() -> Vec<u8> {
        let mut rom = vec![0xFF; 5 * M161_BANK_BYTES];
        let titles = [
            b"MANI 4 IN 1".as_slice(),
            b"TETRIS".as_slice(),
            b"TENNIS".as_slice(),
            b"ALLEY WAY".as_slice(),
            b"YAKUMAN".as_slice(),
        ];

        for (bank, title) in titles.into_iter().enumerate() {
            let start = bank * M161_BANK_BYTES;
            let bank_rom = &mut rom[start..start + M161_BANK_BYTES];
            bank_rom[ENTRY_POINT_START..ENTRY_POINT_START + ENTRY_POINT_LEN]
                .copy_from_slice(&[0x00, 0xC3, 0x50, 0x01]);
            bank_rom[NINTENDO_LOGO_START..NINTENDO_LOGO_START + NINTENDO_LOGO_LEN]
                .copy_from_slice(&[0xCE; NINTENDO_LOGO_LEN]);
            bank_rom[TITLE_START..=TITLE_END_INCLUSIVE].fill(0x00);
            bank_rom[TITLE_START..TITLE_START + title.len()].copy_from_slice(title);
            bank_rom[CARTRIDGE_TYPE_ADDRESS] = 0x00;
            bank_rom[ROM_SIZE_ADDRESS] = 0x00;
            bank_rom[RAM_SIZE_ADDRESS] = 0x00;
        }

        rom
    }

    fn mark_mbc1_multicart_subheaders(rom: &mut [u8]) {
        let logo = rom[NINTENDO_LOGO_START..NINTENDO_LOGO_START + NINTENDO_LOGO_LEN].to_vec();

        for bank in [0x10usize, 0x20, 0x30] {
            let start = bank * 0x4000 + NINTENDO_LOGO_START;
            rom[start..start + NINTENDO_LOGO_LEN].copy_from_slice(&logo);
        }
    }

    fn build_banked_mbc2_rom(cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
        let rom_size = RomSizeInfo::decode(rom_size_code)
            .decoded_bytes
            .expect("test ROM size should decode");
        let bank_count = RomSizeInfo::decode(rom_size_code)
            .bank_count
            .expect("test ROM bank count should decode");
        let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

        for bank in 0..bank_count {
            let start = bank * 0x4000;
            rom[start] = bank as u8;
            rom[start + 0x0100] = bank as u8;
        }

        rom
    }

    fn build_banked_mbc3_rom(cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
        let rom_size = RomSizeInfo::decode(rom_size_code)
            .decoded_bytes
            .expect("test ROM size should decode");
        let bank_count = RomSizeInfo::decode(rom_size_code)
            .bank_count
            .expect("test ROM bank count should decode");
        let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

        for bank in 0..bank_count {
            let start = bank * 0x4000;
            rom[start] = bank as u8;
            rom[start + 0x0100] = bank as u8;
        }

        rom
    }

    fn build_banked_mbc5_rom(cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
        let rom_size = RomSizeInfo::decode(rom_size_code)
            .decoded_bytes
            .expect("test ROM size should decode");
        let bank_count = RomSizeInfo::decode(rom_size_code)
            .bank_count
            .expect("test ROM bank count should decode");
        let mut rom = build_test_rom(rom_size, cartridge_type, rom_size_code, ram_size_code);

        for bank in 0..bank_count {
            let start = bank * 0x4000;
            rom[start] = bank as u8;
            rom[start + 1] = ((bank >> 8) & 0x01) as u8;
            rom[start + 0x0100] = bank as u8;
        }

        rom
    }

    fn warn_policy() -> CompatibilityPolicy {
        CompatibilityPolicy {
            execution_mode: ExecutionMode::Permissive,
            validation_policy: ValidationPolicy::Warn,
            heuristic_policy: HeuristicPolicy::Disabled,
            override_policy: OverridePolicy::default(),
            diagnostic_policy: DiagnosticPolicy::Standard,
        }
    }

    #[test]
    fn header_parser_decodes_typed_core_fields() {
        let rom = build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x09, 0x00, 0x02);
        let header = CartridgeHeader::parse(&rom).expect("header should parse");

        assert_eq!(header.entry_point, [0x00, 0xC3, 0x50, 0x01]);
        assert_eq!(header.title, "GBTEST1");
        assert_eq!(header.cgb_flag, CgbFlag::Supported);
        assert_eq!(header.sgb_flag, SgbFlag::Supported);
        assert_eq!(header.cartridge_type, 0x09);
        assert_eq!(
            header.rom_size.decoded_bytes,
            Some(NO_MBC_SUPPORTED_ROM_BYTES)
        );
        assert_eq!(
            header.ram_size.decoded_bytes,
            Some(NO_MBC_SUPPORTED_RAM_BYTES)
        );
    }

    #[test]
    fn classification_keeps_supported_families_and_structured_unsupported_categories_explicit() {
        let no_mbc = CartridgeClassification::classify(0x09);
        let mbc1 = CartridgeClassification::classify(0x03);
        let camera = CartridgeClassification::classify(0xFC);
        let unknown = CartridgeClassification::classify(0xAA);

        assert_eq!(
            no_mbc.selection(),
            CartridgeSelection::Supported(SupportedCartridgeFamily::NoMbc)
        );
        assert_eq!(
            mbc1.selection(),
            CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc1)
        );
        assert_eq!(
            camera.selection(),
            CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::AccessorySpecialCase)
        );
        assert_eq!(
            unknown.selection(),
            CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::UnknownCode)
        );
        assert_eq!(camera.detected_name(), "POCKET CAMERA");
        assert_eq!(unknown.raw_type(), 0xAA);
    }

    #[test]
    fn contextual_classification_promotes_mbc30_and_opt_in_heuristics_over_the_raw_header() {
        let mbc30_rom = build_test_rom(256 * 1024, 0x13, 0x03, 0x05);
        let mbc30_header = CartridgeHeader::parse(&mbc30_rom).expect("header should parse");
        let mbc30_classification =
            classify_loaded_cartridge(&mbc30_header, &mbc30_rom, &CompatibilityPolicy::strict());

        assert_eq!(mbc30_classification.detected_name(), "MBC30");
        assert_eq!(
            mbc30_classification.selection(),
            CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::PlannedVariant)
        );

        let mut ems_rom = build_test_rom(256 * 1024, 0x1B, 0x03, 0x03);
        ems_rom[TITLE_START..TITLE_START + 7].copy_from_slice(b"EMSMENU");
        ems_rom[TITLE_START + 7..=TITLE_END_INCLUSIVE].fill(0x00);
        ems_rom[DESTINATION_CODE_ADDRESS] = 0xE1;
        let ems_header = CartridgeHeader::parse(&ems_rom).expect("header should parse");

        let strict_classification =
            classify_loaded_cartridge(&ems_header, &ems_rom, &CompatibilityPolicy::strict());
        assert_eq!(
            strict_classification.selection(),
            CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc5)
        );

        let experimental_classification =
            classify_loaded_cartridge(&ems_header, &ems_rom, &CompatibilityPolicy::experimental());
        assert_eq!(experimental_classification.detected_name(), "EMS");
        assert_eq!(
            experimental_classification.selection(),
            CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::ExperimentalHeuristic)
        );

        let mut mbc1m_rom = build_banked_mbc1_rom_with_type(0x01, 0x05, 0x00);
        mark_mbc1_multicart_subheaders(&mut mbc1m_rom);
        let mbc1m_header = CartridgeHeader::parse(&mbc1m_rom).expect("header should parse");

        let strict_mbc1m =
            classify_loaded_cartridge(&mbc1m_header, &mbc1m_rom, &CompatibilityPolicy::strict());
        assert_eq!(strict_mbc1m.detected_name(), "MBC1");
        assert_eq!(
            strict_mbc1m.selection(),
            CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc1)
        );

        let experimental_mbc1m = classify_loaded_cartridge(
            &mbc1m_header,
            &mbc1m_rom,
            &CompatibilityPolicy::experimental(),
        );
        assert_eq!(experimental_mbc1m.detected_name(), "MBC1M");
        assert_eq!(
            experimental_mbc1m.selection(),
            CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc1)
        );

        let m161_rom = build_m161_signature_rom();
        let m161_header = CartridgeHeader::parse(&m161_rom).expect("header should parse");
        let m161_classification =
            classify_loaded_cartridge(&m161_header, &m161_rom, &CompatibilityPolicy::strict());
        assert_eq!(m161_classification.detected_name(), "M161");
        assert_eq!(
            m161_classification.selection(),
            CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::DocumentedButUnsupported)
        );
    }

    #[test]
    fn no_mbc_loader_builds_rom_only_and_ram_variants() {
        let rom_only = build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x00, 0x00, 0x00);
        let with_ram = build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x09, 0x00, 0x02);

        let rom_only_report = CartridgeSlot::load(rom_only, &CompatibilityPolicy::strict())
            .expect("ROM-only NoMBC should load");
        let with_ram_report = CartridgeSlot::load(with_ram, &CompatibilityPolicy::strict())
            .expect("RAM NoMBC should load");

        assert_eq!(
            rom_only_report.cartridge().state(),
            CartridgeSlotState::NoMbc
        );
        assert_eq!(
            with_ram_report.cartridge().state(),
            CartridgeSlotState::NoMbc
        );
        assert_eq!(
            rom_only_report.cartridge().read_ram(0xA000),
            RAM_ABSENT_READ_VALUE
        );
        assert!(with_ram_report.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("rare but still treated as a valid No MBC variant")
        }));
    }

    #[test]
    fn strict_validation_rejects_invalid_no_mbc_ram_configuration() {
        let rom = build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x08, 0x00, 0x03);
        let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
            .expect_err("invalid RAM config must fail");

        match error {
            CartridgeLoadError::Rejected {
                classification,
                execution_mode,
                reason,
                ..
            } => {
                assert_eq!(classification.detected_name(), "ROM+RAM");
                assert_eq!(execution_mode, ExecutionMode::Strict);
                assert!(reason.contains("expects RAM size code"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn warn_validation_can_admit_unambiguous_no_mbc_size_mismatches_with_diagnostics() {
        let rom = build_test_rom(64 * 1024, 0x00, 0x01, 0x00);
        let report = CartridgeSlot::load(rom, &warn_policy())
            .expect("warn policy should admit the mismatch");

        assert_eq!(report.cartridge().state(), CartridgeSlotState::NoMbc);
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message.contains("expects ROM size code 0x00"))
        );
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message.contains("expects a 32 KiB image"))
        );
    }

    #[test]
    fn warn_validation_can_admit_unambiguous_no_mbc_ram_header_mismatches_with_diagnostics() {
        let rom = build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x08, 0x00, 0x01);
        let report = CartridgeSlot::load(rom, &warn_policy())
            .expect("warn policy should admit the legacy RAM-size mismatch");

        assert_eq!(report.cartridge().state(), CartridgeSlotState::NoMbc);
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message.contains("expects RAM size code 0x02"))
        );
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message.contains("unsupported RAM configuration"))
        );
    }

    #[test]
    fn loading_supported_mbc1_family_constructs_the_mapper_device() {
        let rom = build_banked_mbc1_rom(0x02, 0x00);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");

        assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc1);
        assert_eq!(report.cartridge().read_rom(0x0000), 0x00);
        assert_eq!(report.cartridge().read_rom(0x4000), 0x01);
    }

    #[test]
    fn loading_32kib_mbc1_images_keeps_the_switchable_window_on_bank_one() {
        let rom = build_banked_mbc1_rom(0x00, 0x00);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");

        assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc1);
        assert_eq!(report.cartridge().read_rom(0x0000), 0x00);
        assert_eq!(report.cartridge().read_rom(0x4000), 0x01);
    }

    #[test]
    fn mbc1_power_up_state_is_explicit_and_starts_the_high_window_on_bank_one() {
        let rom = build_banked_mbc1_rom(0x03, 0x03);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");

        let Some(CartridgeDevice::Mbc1(cartridge)) = report.cartridge().device.as_ref() else {
            panic!("expected MBC1 cartridge");
        };

        assert_eq!(cartridge.wiring, Mbc1Wiring::Standard);
        assert_eq!(cartridge.variant, Mbc1Variant::Standard);
        assert!(!cartridge.ram_enabled);
        assert_eq!(cartridge.rom_bank_low5, 0);
        assert_eq!(cartridge.secondary_bank, 0);
        assert_eq!(cartridge.banking_mode, 0);
        assert_eq!(cartridge.read_rom(0x4000), 0x01);
    }

    #[test]
    fn mbc1_raw_low_bank_zero_translates_to_bank_one_before_size_masking() {
        let rom = build_banked_mbc1_rom(0x04, 0x00);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
        let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
            panic!("expected MBC1 cartridge");
        };

        cartridge.write_rom(0x2000, 0x00);
        assert_eq!(cartridge.rom_bank_low5, 0);
        assert_eq!(cartridge.read_rom(0x4000), 0x01);

        cartridge.write_rom(0x2000, 0x1F);
        assert_eq!(cartridge.read_rom(0x4000), 0x1F);
    }

    #[test]
    fn mbc1_small_rom_masking_can_make_bank_zero_visible_in_the_high_window() {
        let rom = build_banked_mbc1_rom(0x01, 0x00);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
        let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
            panic!("expected MBC1 cartridge");
        };

        cartridge.write_rom(0x2000, 0x04);

        assert_eq!(cartridge.read_rom(0x4000), 0x00);
    }

    #[test]
    fn mbc1_control_writes_update_raw_registers_and_gate_ram_access_immediately() {
        let rom = build_banked_mbc1_rom(0x02, 0x03);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
        let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
            panic!("expected MBC1 cartridge");
        };

        assert_eq!(cartridge.read_ram(0xA000), RAM_ABSENT_READ_VALUE);
        cartridge.write_ram(0xA000, 0x5A);
        assert_eq!(cartridge.read_ram(0xA000), RAM_ABSENT_READ_VALUE);

        cartridge.write_rom(0x0000, 0x0A);
        cartridge.write_rom(0x4000, 0x02);
        cartridge.write_rom(0x6000, 0x01);
        cartridge.write_ram(0xA000, 0x5A);

        assert!(cartridge.ram_enabled);
        assert_eq!(cartridge.secondary_bank, 0x02);
        assert_eq!(cartridge.banking_mode, 0x01);
        assert_eq!(cartridge.read_ram(0xA000), 0x5A);
    }

    #[test]
    fn mbc1_standard_8kib_ram_ignores_mode_one_ram_bank_selection() {
        let rom = build_banked_mbc1_rom(0x01, 0x02);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
        let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
            panic!("expected MBC1 cartridge");
        };

        cartridge.write_rom(0x0000, 0x0A);
        cartridge.write_ram(0xA000, 0x11);
        cartridge.write_ram(0xB000, 0x22);

        cartridge.write_rom(0x6000, 0x01);
        for bank in 0..=3 {
            cartridge.write_rom(0x4000, bank);
            assert_eq!(cartridge.read_ram(0xA000), 0x11);
            assert_eq!(cartridge.read_ram(0xB000), 0x22);
        }

        cartridge.write_rom(0x4000, 0x03);
        cartridge.write_ram(0xA000, 0x33);
        cartridge.write_ram(0xB000, 0x44);

        cartridge.write_rom(0x4000, 0x01);
        assert_eq!(cartridge.read_ram(0xA000), 0x33);
        assert_eq!(cartridge.read_ram(0xB000), 0x44);
    }

    #[test]
    fn mbc1_large_rom_high_window_reaches_documented_odd_bank_entries_only() {
        let rom = build_banked_mbc1_rom(0x06, 0x00);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
        let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
            panic!("expected MBC1 cartridge");
        };

        assert_eq!(cartridge.wiring, Mbc1Wiring::LargeRom);

        cartridge.write_rom(0x2000, 0x00);
        cartridge.write_rom(0x4000, 0x01);
        assert_eq!(cartridge.read_rom(0x4000), 0x21);

        cartridge.write_rom(0x4000, 0x02);
        assert_eq!(cartridge.read_rom(0x4000), 0x41);

        cartridge.write_rom(0x4000, 0x03);
        assert_eq!(cartridge.read_rom(0x4000), 0x61);
    }

    #[test]
    fn mbc1_large_rom_mode_one_remaps_the_low_window_from_secondary_bits() {
        let rom = build_banked_mbc1_rom(0x06, 0x00);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
        let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
            panic!("expected MBC1 cartridge");
        };

        cartridge.write_rom(0x2000, 0x01);
        cartridge.write_rom(0x4000, 0x02);
        assert_eq!(cartridge.read_rom(0x0000), 0x00);
        assert_eq!(cartridge.read_rom(0x4000), 0x41);

        cartridge.write_rom(0x6000, 0x01);
        assert_eq!(cartridge.read_rom(0x0000), 0x40);
        assert_eq!(cartridge.read_rom(0x4000), 0x41);
    }

    #[test]
    fn mbc1_large_rom_keeps_one_fixed_8kib_ram_window_across_modes() {
        let rom = build_banked_mbc1_rom(0x05, 0x02);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
        let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
            panic!("expected MBC1 cartridge");
        };

        cartridge.write_rom(0x0000, 0x0A);
        cartridge.write_ram(0xA000, 0x11);

        cartridge.write_rom(0x4000, 0x01);
        cartridge.write_rom(0x6000, 0x01);

        assert_eq!(cartridge.read_ram(0xA000), 0x11);
        cartridge.write_ram(0xA000, 0x22);

        cartridge.write_rom(0x4000, 0x00);
        cartridge.write_rom(0x6000, 0x00);
        assert_eq!(cartridge.read_ram(0xA000), 0x22);
    }

    #[test]
    fn experimental_mbc1m_multicart_banking_uses_the_documented_game_select_layout() {
        let mut rom = build_banked_mbc1_rom_with_type(0x01, 0x05, 0x00);
        mark_mbc1_multicart_subheaders(&mut rom);
        let report = CartridgeSlot::load(rom, &CompatibilityPolicy::experimental())
            .expect("experimental MBC1M should load");
        let Some(CartridgeDevice::Mbc1(mut cartridge)) = report.cartridge().device.clone() else {
            panic!("expected MBC1 cartridge");
        };

        assert_eq!(cartridge.variant, Mbc1Variant::Mbc1M);
        assert!(report.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("explicit experimental multicart heuristic")
        }));

        cartridge.write_rom(0x2000, 0x00);
        cartridge.write_rom(0x4000, 0x00);
        assert_eq!(cartridge.read_rom(0x4000), 0x01);

        cartridge.write_rom(0x2000, 0x10);
        assert_eq!(cartridge.read_rom(0x4000), 0x00);

        cartridge.write_rom(0x2000, 0x00);
        cartridge.write_rom(0x4000, 0x01);
        assert_eq!(cartridge.read_rom(0x4000), 0x11);

        cartridge.write_rom(0x2000, 0x10);
        assert_eq!(cartridge.read_rom(0x4000), 0x10);

        cartridge.write_rom(0x6000, 0x01);
        cartridge.write_rom(0x4000, 0x02);
        assert_eq!(cartridge.read_rom(0x0000), 0x20);
        assert_eq!(cartridge.read_rom(0x4000), 0x20);

        cartridge.write_rom(0x2000, 0x01);
        assert_eq!(cartridge.read_rom(0x4000), 0x21);
    }

    #[test]
    fn strict_validation_rejects_large_rom_mbc1_with_32kib_ram_declaration() {
        let rom = build_banked_mbc1_rom(0x05, 0x03);
        let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
            .expect_err("invalid large-ROM MBC1 RAM config must fail");

        match error {
            CartridgeLoadError::Rejected { reason, .. } => {
                assert!(reason.contains("not valid for the current LargeRom MBC1 wiring baseline"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn strict_validation_admits_32kib_mbc1_images_as_small_standard_wiring() {
        let rom = build_banked_mbc1_rom(0x00, 0x00);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC1 should load");
        let Some(CartridgeDevice::Mbc1(cartridge)) = report.cartridge().device.as_ref() else {
            panic!("expected MBC1 cartridge");
        };

        assert_eq!(cartridge.wiring, Mbc1Wiring::Standard);
        assert_eq!(cartridge.read_rom(0x4000), 0x01);
    }

    #[test]
    fn mbc2_power_up_state_is_explicit_and_starts_the_high_window_on_bank_one() {
        let rom = build_banked_mbc2_rom(0x06, 0x03, 0x00);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC2 should load");

        let Some(CartridgeDevice::Mbc2(cartridge)) = report.cartridge().device.as_ref() else {
            panic!("expected MBC2 cartridge");
        };

        assert!(!cartridge.ram_enabled);
        assert_eq!(cartridge.rom_bank_low4, 0);
        assert_eq!(cartridge.read_rom(0x4000), 0x01);
        assert!(cartridge.has_battery);
    }

    #[test]
    fn mbc2_address_bit_8_decode_controls_enable_and_bank_registers_separately() {
        let rom = build_banked_mbc2_rom(0x05, 0x03, 0x00);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC2 should load");
        let Some(CartridgeDevice::Mbc2(mut cartridge)) = report.cartridge().device.clone() else {
            panic!("expected MBC2 cartridge");
        };

        cartridge.write_rom(0x0000, 0x0A);
        assert!(cartridge.ram_enabled);
        assert_eq!(cartridge.rom_bank_low4, 0);

        cartridge.write_rom(0x0100, 0x03);
        assert!(cartridge.ram_enabled);
        assert_eq!(cartridge.rom_bank_low4, 0x03);
        assert_eq!(cartridge.read_rom(0x4000), 0x03);
    }

    #[test]
    fn mbc2_internal_ram_masks_to_low_nibbles_and_aliases_on_low_9_bits() {
        let rom = build_banked_mbc2_rom(0x06, 0x03, 0x00);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC2 should load");
        let Some(CartridgeDevice::Mbc2(mut cartridge)) = report.cartridge().device.clone() else {
            panic!("expected MBC2 cartridge");
        };

        assert_eq!(cartridge.read_ram(0xA000), RAM_ABSENT_READ_VALUE);
        cartridge.write_ram(0xA000, 0xAB);
        assert_eq!(cartridge.read_ram(0xA000), RAM_ABSENT_READ_VALUE);

        cartridge.write_rom(0x0000, 0x0A);
        cartridge.write_ram(0xA000, 0xAB);

        assert_eq!(cartridge.read_ram(0xA000), 0xFB);
        assert_eq!(cartridge.read_ram(0xA200), 0xFB);
        assert_eq!(cartridge.read_ram(0xBFFF), 0xF0);
    }

    #[test]
    fn mbc2_ignores_rom_space_writes_outside_the_control_window() {
        let rom = build_banked_mbc2_rom(0x06, 0x03, 0x00);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC2 should load");
        let Some(CartridgeDevice::Mbc2(mut cartridge)) = report.cartridge().device.clone() else {
            panic!("expected MBC2 cartridge");
        };

        cartridge.write_rom(0x0000, 0x0A);
        cartridge.write_ram(0xA000, 0x0B);
        assert!(cartridge.ram_enabled);
        assert_eq!(cartridge.read_ram(0xA000), 0xFB);

        cartridge.write_rom(0x0100, 0x03);
        assert_eq!(cartridge.read_rom(0x4000), 0x03);

        cartridge.write_rom(0x4000, 0x00);
        cartridge.write_rom(0x4100, 0x01);

        assert!(cartridge.ram_enabled);
        assert_eq!(cartridge.read_ram(0xA000), 0xFB);
        assert_eq!(cartridge.read_rom(0x4000), 0x03);
    }

    #[test]
    fn strict_validation_rejects_oversized_mbc2_roms() {
        let rom = build_banked_mbc2_rom(0x05, 0x04, 0x00);
        let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
            .expect_err("oversized MBC2 should fail validation");

        match error {
            CartridgeLoadError::Rejected { reason, .. } => {
                assert!(reason.contains("exceeds the current MBC2 ROM limit"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn permissive_validation_can_warn_on_nonzero_mbc2_ram_size_metadata() {
        let rom = build_banked_mbc2_rom(0x06, 0x03, 0x02);
        let report = CartridgeSlot::load(rom, &warn_policy())
            .expect("warn policy should admit nonzero MBC2 RAM metadata");

        assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc2);
        assert!(report.diagnostics().iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("expects RAM size code 0x00 because MBC2 RAM is internal")
        }));
    }

    #[test]
    fn mbc3_power_up_state_is_explicit_and_starts_the_high_window_on_bank_one() {
        let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");

        let Some(CartridgeDevice::Mbc3(cartridge)) = report.cartridge().device.as_ref() else {
            panic!("expected MBC3 cartridge");
        };

        assert_eq!(cartridge.variant, Mbc3Variant::Standard);
        assert!(cartridge.has_rtc);
        assert!(!cartridge.ram_rtc_enabled);
        assert_eq!(cartridge.rom_bank, 0);
        assert_eq!(cartridge.ram_or_rtc_select, Mbc3RamRtcSelect::RamBank(0));
        assert!(!cartridge.rtc_latched_valid);
        assert!(cartridge.rtc_latch_armed);
        assert_eq!(cartridge.read_rom(0x4000), 0x01);
    }

    #[test]
    fn mbc3_reaches_banks_0x20_0x40_and_0x60_without_mbc1_style_anomalies() {
        let rom = build_banked_mbc3_rom(0x13, 0x06, 0x03);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
        let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
            panic!("expected MBC3 cartridge");
        };

        for bank in [0x20, 0x40, 0x60] {
            cartridge.write_rom(0x2000, bank);
            assert_eq!(cartridge.read_rom(0x4000), bank);
        }
    }

    #[test]
    fn mbc3_selector_keeps_ram_reserved_and_rtc_targets_distinct() {
        let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
        let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
            panic!("expected MBC3 cartridge");
        };

        cartridge.write_rom(0x4000, 0x02);
        assert_eq!(cartridge.ram_or_rtc_select, Mbc3RamRtcSelect::RamBank(0x02));

        cartridge.write_rom(0x4000, 0x05);
        assert_eq!(
            cartridge.ram_or_rtc_select,
            Mbc3RamRtcSelect::ReservedSelector(0x05)
        );

        cartridge.write_rom(0x4000, 0x0C);
        assert_eq!(
            cartridge.ram_or_rtc_select,
            Mbc3RamRtcSelect::RtcRegister(Mbc3RtcRegister::DayHigh)
        );
    }

    #[test]
    fn mbc3_selector_ignores_upper_data_bits_and_decodes_from_the_low_nibble() {
        let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
        let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
            panic!("expected MBC3 cartridge");
        };

        cartridge.write_rom(0x4000, 0x12);
        assert_eq!(cartridge.ram_or_rtc_select, Mbc3RamRtcSelect::RamBank(0x02));

        cartridge.write_rom(0x4000, 0x1C);
        assert_eq!(
            cartridge.ram_or_rtc_select,
            Mbc3RamRtcSelect::RtcRegister(Mbc3RtcRegister::DayHigh)
        );

        cartridge.write_rom(0x4000, 0x17);
        assert_eq!(
            cartridge.ram_or_rtc_select,
            Mbc3RamRtcSelect::ReservedSelector(0x07)
        );
    }

    #[test]
    fn strict_validation_admits_mbc3_headers_with_2kib_ram_metadata() {
        let rom = build_banked_mbc3_rom(0x13, 0x00, 0x01);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");

        assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc3);
    }

    #[test]
    fn mbc3_rtc_latch_reads_from_snapshot_while_writes_hit_live_state() {
        let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
        let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
            panic!("expected MBC3 cartridge");
        };

        cartridge.advance_rtc_seconds(93_784);
        cartridge.write_rom(0x0000, 0x0A);
        cartridge.write_rom(0x4000, 0x08);

        assert_eq!(cartridge.read_ram(0xA000), 0x00);

        cartridge.write_rom(0x6000, 0x01);
        assert_eq!(cartridge.read_ram(0xA000), 0x04);

        cartridge.advance_rtc_seconds(1);
        assert_eq!(cartridge.read_ram(0xA000), 0x04);

        cartridge.write_rom(0x6000, 0x00);
        cartridge.write_rom(0x6000, 0x01);
        assert_eq!(cartridge.read_ram(0xA000), 0x05);

        cartridge.write_ram(0xA000, 0x2A);
        assert_eq!(cartridge.rtc_live.seconds, 0x2A);
        assert_eq!(cartridge.read_ram(0xA000), 0x05);

        cartridge.write_rom(0x6000, 0x55);
        assert_eq!(cartridge.read_ram(0xA000), 0x2A);
    }

    #[test]
    fn mbc3_rtc_register_writes_echo_raw_bytes_until_time_advances() {
        let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
        let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
            panic!("expected MBC3 cartridge");
        };

        cartridge.write_rom(0x0000, 0x0A);

        cartridge.write_rom(0x4000, 0x08);
        cartridge.write_ram(0xA000, 0x74);

        cartridge.write_rom(0x4000, 0x09);
        cartridge.write_ram(0xA000, 0xF2);

        cartridge.write_rom(0x4000, 0x0A);
        cartridge.write_ram(0xA000, 0x62);
        assert_eq!(cartridge.read_ram(0xA000), 0x00);

        cartridge.write_rom(0x6000, 0x01);

        cartridge.write_rom(0x4000, 0x08);
        assert_eq!(cartridge.read_ram(0xA000), 0x34);

        cartridge.advance_rtc_seconds(1);

        cartridge.write_rom(0x4000, 0x08);
        assert_eq!(cartridge.read_ram(0xA000), 0x34);

        cartridge.write_rom(0x4000, 0x09);
        assert_eq!(cartridge.read_ram(0xA000), 0x32);

        cartridge.write_rom(0x4000, 0x0A);
        assert_eq!(cartridge.read_ram(0xA000), 0x02);
    }

    #[test]
    fn mbc3_latch_stays_armed_after_a_successful_latch_until_a_zero_write_resets_it() {
        let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
        let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
            panic!("expected MBC3 cartridge");
        };

        cartridge.write_rom(0x0000, 0x0A);
        cartridge.write_rom(0x4000, 0x08);

        cartridge.write_ram(0xA000, 0x11);
        cartridge.write_rom(0x6000, 0x00);
        cartridge.write_rom(0x6000, 0x01);
        assert_eq!(cartridge.read_ram(0xA000), 0x11);

        cartridge.write_ram(0xA000, 0x37);
        assert_eq!(cartridge.read_ram(0xA000), 0x11);

        cartridge.write_rom(0x6000, 0x44);
        assert_eq!(cartridge.read_ram(0xA000), 0x37);
    }

    #[test]
    fn mbc3_halt_and_carry_behavior_follow_the_live_rtc_rules() {
        let rom = build_banked_mbc3_rom(0x10, 0x03, 0x03);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC3 should load");
        let Some(CartridgeDevice::Mbc3(mut cartridge)) = report.cartridge().device.clone() else {
            panic!("expected MBC3 cartridge");
        };

        cartridge.write_rom(0x0000, 0x0A);
        cartridge.write_rom(0x4000, 0x0C);
        cartridge.write_ram(0xA000, 0x40);
        cartridge.advance_rtc_seconds(86_400);
        assert_eq!(cartridge.rtc_live.day_counter, 0);

        cartridge.write_ram(0xA000, 0x00);
        cartridge.advance_rtc_seconds(86_400);
        assert_eq!(cartridge.rtc_live.day_counter, 1);

        cartridge.rtc_live.day_counter = 511;
        cartridge.rtc_live.hours = 23;
        cartridge.rtc_live.minutes = 59;
        cartridge.rtc_live.seconds = 59;
        cartridge.advance_rtc_seconds(1);
        assert_eq!(cartridge.rtc_live.day_counter, 0);
        assert!(cartridge.rtc_live.carry);

        cartridge.write_ram(0xA000, 0x00);
        assert!(!cartridge.rtc_live.carry);
    }

    #[test]
    fn mbc3_persistent_rtc_elapsed_seconds_follow_the_live_rules() {
        let mut rtc = Mbc3RtcPersistentState {
            seconds: 59,
            minutes: 59,
            hours: 23,
            day_counter: 511,
            halt: false,
            carry: false,
        };

        rtc.apply_elapsed_seconds(2);

        assert_eq!(rtc.seconds, 1);
        assert_eq!(rtc.minutes, 0);
        assert_eq!(rtc.hours, 0);
        assert_eq!(rtc.day_counter, 0);
        assert!(rtc.carry);

        let halted = rtc;
        let mut halted = Mbc3RtcPersistentState {
            halt: true,
            ..halted
        };
        halted.apply_elapsed_seconds(86_400);
        assert_eq!(halted.seconds, rtc.seconds);
        assert_eq!(halted.minutes, rtc.minutes);
        assert_eq!(halted.hours, rtc.hours);
        assert_eq!(halted.day_counter, rtc.day_counter);
        assert!(halted.carry);
    }

    #[test]
    fn strict_validation_rejects_mbc30_like_64kib_sram_configurations() {
        let rom = build_banked_mbc3_rom(0x13, 0x06, 0x05);
        let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
            .expect_err("MBC30-like SRAM should fail the standard MBC3 baseline");

        match error {
            CartridgeLoadError::Rejected {
                classification,
                reason,
                ..
            } => {
                assert_eq!(classification.detected_name(), "MBC30");
                assert_eq!(
                    classification.selection(),
                    CartridgeSelection::Unsupported(UnsupportedCartridgeCategory::PlannedVariant)
                );
                assert!(reason.contains("known reserved variant"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn permissive_validation_can_warn_when_no_ram_mbc3_headers_still_declare_ram() {
        let rom = build_banked_mbc3_rom(0x11, 0x03, 0x02);
        let report = CartridgeSlot::load(rom, &warn_policy())
            .expect("warn policy should admit a no-RAM MBC3 mismatch");

        assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc3);
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message.contains("does not provide external RAM"))
        );
    }

    #[test]
    fn mbc5_power_up_state_starts_the_high_window_on_bank_one_while_keeping_bank_zero_reachable() {
        let rom = build_banked_mbc5_rom(0x1E, 0x08, 0x03);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC5 should load");

        let Some(CartridgeDevice::Mbc5(cartridge)) = report.cartridge().device.as_ref() else {
            panic!("expected MBC5 cartridge");
        };

        assert_eq!(cartridge.variant, Mbc5Variant::RumbleRamBattery);
        assert!(!cartridge.ram_enabled);
        assert_eq!(cartridge.rom_bank_low8, 1);
        assert_eq!(cartridge.rom_bank_high1, 0);
        assert_eq!(cartridge.ram_bank_raw, 0);
        assert!(!cartridge.rumble_on());
        assert_eq!(cartridge.read_rom(0x4000), 0x01);
        assert_eq!(cartridge.read_rom(0x4001), 0x00);

        let mut cartridge = cartridge.clone();
        cartridge.write_rom(0x2000, 0x00);
        assert_eq!(cartridge.read_rom(0x4000), 0x00);
        assert_eq!(cartridge.read_rom(0x4001), 0x00);
    }

    #[test]
    fn mbc5_reaches_bank_0x1ff_without_applying_a_zero_to_one_translation() {
        let rom = build_banked_mbc5_rom(0x1B, 0x08, 0x04);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC5 should load");
        let Some(CartridgeDevice::Mbc5(mut cartridge)) = report.cartridge().device.clone() else {
            panic!("expected MBC5 cartridge");
        };

        cartridge.write_rom(0x2000, 0xFF);
        cartridge.write_rom(0x3000, 0x00);
        assert_eq!(cartridge.read_rom(0x4000), 0xFF);
        assert_eq!(cartridge.read_rom(0x4001), 0x00);

        cartridge.write_rom(0x2000, 0x00);
        cartridge.write_rom(0x3000, 0x01);
        assert_eq!(cartridge.read_rom(0x4000), 0x00);
        assert_eq!(cartridge.read_rom(0x4001), 0x01);

        cartridge.write_rom(0x2000, 0xFF);
        assert_eq!(cartridge.read_rom(0x4000), 0xFF);
        assert_eq!(cartridge.read_rom(0x4001), 0x01);
    }

    #[test]
    fn mbc5_rumble_control_keeps_motor_state_distinct_from_effective_ram_bank() {
        let rom = build_banked_mbc5_rom(0x1E, 0x03, 0x03);
        let report =
            CartridgeSlot::load(rom, &CompatibilityPolicy::strict()).expect("MBC5 should load");
        let Some(CartridgeDevice::Mbc5(mut cartridge)) = report.cartridge().device.clone() else {
            panic!("expected MBC5 cartridge");
        };

        assert_eq!(cartridge.read_ram(0xA000), RAM_ABSENT_READ_VALUE);
        cartridge.write_rom(0x0000, 0x0A);

        cartridge.write_rom(0x4000, 0x03);
        cartridge.write_ram(0xA000, 0x33);

        cartridge.write_rom(0x4000, 0x0B);
        assert!(cartridge.rumble_on());
        assert_eq!(cartridge.ram_bank_raw, 0x03);
        assert_eq!(cartridge.read_ram(0xA000), 0x33);

        cartridge.write_rom(0x4000, 0x03);
        assert!(!cartridge.rumble_on());
        assert_eq!(cartridge.read_ram(0xA000), 0x33);
    }

    #[test]
    fn strict_validation_rejects_oversized_mbc5_images_and_invalid_rumble_ram_sizes() {
        let oversized = build_test_rom(16 * 1024 * 1024, 0x1B, 0x08, 0x04);
        let oversized_error = CartridgeSlot::load(oversized, &CompatibilityPolicy::strict())
            .expect_err("oversized MBC5 should fail validation");

        match oversized_error {
            CartridgeLoadError::Rejected { reason, .. } => {
                assert!(reason.contains("exceeds the current MBC5 ROM limit"));
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let invalid_rumble_ram = build_banked_mbc5_rom(0x1E, 0x03, 0x04);
        let invalid_rumble_error =
            CartridgeSlot::load(invalid_rumble_ram, &CompatibilityPolicy::strict())
                .expect_err("128 KiB rumble MBC5 should fail validation");

        match invalid_rumble_error {
            CartridgeLoadError::Rejected { reason, .. } => {
                assert!(reason.contains("rumble-capable MBC5 baseline"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn permissive_validation_can_warn_when_no_ram_mbc5_headers_still_declare_ram() {
        let rom = build_banked_mbc5_rom(0x19, 0x03, 0x02);
        let report = CartridgeSlot::load(rom, &warn_policy())
            .expect("warn policy should admit a no-RAM MBC5 mismatch");

        assert_eq!(report.cartridge().state(), CartridgeSlotState::Mbc5);
        assert!(
            report
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message.contains("does not provide external RAM"))
        );
    }

    #[test]
    fn persistence_metadata_keeps_ram_shapes_and_battery_policy_explicit() {
        let no_mbc_report = CartridgeSlot::load(
            build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x09, 0x00, 0x02),
            &CompatibilityPolicy::strict(),
        )
        .expect("NoMBC+BATTERY should load");
        assert_eq!(
            no_mbc_report.cartridge().persistence_metadata(),
            CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: false,
                profile: CartridgePersistenceProfile::PersistentRam {
                    ram: CartridgeRamPayloadKind::Linear {
                        byte_len: NO_MBC_SUPPORTED_RAM_BYTES,
                    },
                },
            }
        );

        let mbc2_report = CartridgeSlot::load(
            build_banked_mbc2_rom(0x05, 0x03, 0x00),
            &CompatibilityPolicy::strict(),
        )
        .expect("MBC2 should load");
        assert_eq!(
            mbc2_report.cartridge().persistence_metadata(),
            CartridgePersistenceMetadata {
                has_battery: false,
                has_rtc: false,
                profile: CartridgePersistenceProfile::NonPersistentRam {
                    ram: CartridgeRamPayloadKind::Mbc2Nibbles {
                        cell_count: MBC2_RAM_CELL_COUNT,
                    },
                },
            }
        );
    }

    #[test]
    fn restore_persistent_state_validates_mbc2_nibble_payload_values() {
        let report = CartridgeSlot::load(
            build_banked_mbc2_rom(0x06, 0x03, 0x00),
            &CompatibilityPolicy::strict(),
        )
        .expect("MBC2+BATTERY should load");
        let Some(CartridgeDevice::Mbc2(mut cartridge)) = report.cartridge().device.clone() else {
            panic!("expected MBC2 cartridge");
        };

        let mut invalid_nibbles = [0u8; MBC2_RAM_CELL_COUNT];
        invalid_nibbles[7] = 0xF1;
        let error = cartridge
            .restore_persistent_state(&PersistentCartState::Mbc2Ram {
                ram_nibbles: invalid_nibbles,
            })
            .expect_err("invalid high bits must fail");

        assert_eq!(
            error,
            CartridgePersistentStateError::InvalidMbc2NibbleValue {
                index: 7,
                value: 0xF1,
            }
        );
    }

    #[test]
    fn slot_accessors_and_restore_paths_cover_empty_no_mbc_mbc1_and_mbc2_families() {
        let mut empty = CartridgeSlot::empty();
        assert!(empty.is_empty());
        assert_eq!(empty.classification(), None);
        assert_eq!(empty.persistent_state(), PersistentCartState::None);
        assert_eq!(
            empty.restore_persistent_state(&PersistentCartState::None),
            Ok(())
        );
        assert_eq!(
            empty.restore_persistent_state(&PersistentCartState::Mbc1Ram { ram: vec![] }),
            Err(CartridgePersistentStateError::KindMismatch {
                expected: "None",
                actual: "Mbc1Ram",
            }),
        );

        let no_mbc_report = CartridgeSlot::load(
            build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x09, 0x00, 0x02),
            &CompatibilityPolicy::strict(),
        )
        .expect("NoMBC+BATTERY should load");
        let (mut no_mbc, _) = no_mbc_report.into_parts();
        assert!(!no_mbc.is_empty());
        assert_eq!(
            no_mbc
                .classification()
                .map(CartridgeClassification::selection),
            Some(CartridgeSelection::Supported(
                SupportedCartridgeFamily::NoMbc
            )),
        );
        assert_eq!(
            no_mbc.persistence_metadata(),
            CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: false,
                profile: CartridgePersistenceProfile::PersistentRam {
                    ram: CartridgeRamPayloadKind::Linear {
                        byte_len: NO_MBC_SUPPORTED_RAM_BYTES,
                    },
                },
            },
        );
        no_mbc.write_ram(0xA000, 0x12);
        assert_eq!(no_mbc.read_ram(0xA000), 0x12);
        let no_mbc_before_rtc = no_mbc.persistent_state();
        no_mbc.advance_rtc_seconds(7);
        assert_eq!(no_mbc.persistent_state(), no_mbc_before_rtc);

        let restored_no_mbc = PersistentCartState::NoMbcRam {
            ram: vec![0x5A; NO_MBC_SUPPORTED_RAM_BYTES],
        };
        no_mbc
            .restore_persistent_state(&restored_no_mbc)
            .expect("NoMBC RAM state should restore");
        assert_eq!(no_mbc.persistent_state(), restored_no_mbc);
        assert_eq!(
            no_mbc.restore_persistent_state(&PersistentCartState::NoMbcRam { ram: vec![0; 4] }),
            Err(CartridgePersistentStateError::RamLengthMismatch {
                expected: NO_MBC_SUPPORTED_RAM_BYTES,
                actual: 4,
            }),
        );
        assert_eq!(
            no_mbc.restore_persistent_state(&PersistentCartState::Mbc1Ram {
                ram: vec![0; NO_MBC_SUPPORTED_RAM_BYTES],
            }),
            Err(CartridgePersistentStateError::KindMismatch {
                expected: "NoMbcRam",
                actual: "Mbc1Ram",
            }),
        );

        let mbc1_report = CartridgeSlot::load(
            build_banked_mbc1_rom(0x03, 0x03),
            &CompatibilityPolicy::strict(),
        )
        .expect("MBC1 should load");
        let (mut mbc1, _) = mbc1_report.into_parts();
        assert_eq!(
            mbc1.classification()
                .map(CartridgeClassification::selection),
            Some(CartridgeSelection::Supported(
                SupportedCartridgeFamily::Mbc1
            )),
        );
        mbc1.write_rom(0x0000, 0x0A);
        mbc1.write_ram(0xA000, 0x34);
        assert_eq!(mbc1.read_ram(0xA000), 0x34);
        let restored_mbc1 = PersistentCartState::Mbc1Ram {
            ram: vec![0x77; 32 * 1024],
        };
        mbc1.restore_persistent_state(&restored_mbc1)
            .expect("MBC1 RAM state should restore");
        assert_eq!(mbc1.persistent_state(), restored_mbc1);
        assert_eq!(
            mbc1.restore_persistent_state(&PersistentCartState::Mbc1Ram { ram: vec![0; 8] }),
            Err(CartridgePersistentStateError::RamLengthMismatch {
                expected: 32 * 1024,
                actual: 8,
            }),
        );
        assert_eq!(
            mbc1.restore_persistent_state(&PersistentCartState::None),
            Err(CartridgePersistentStateError::KindMismatch {
                expected: "Mbc1Ram",
                actual: "None",
            }),
        );

        let mbc2_report = CartridgeSlot::load(
            build_banked_mbc2_rom(0x06, 0x03, 0x00),
            &CompatibilityPolicy::strict(),
        )
        .expect("MBC2+BATTERY should load");
        let (mut mbc2, _) = mbc2_report.into_parts();
        assert_eq!(
            mbc2.classification()
                .map(CartridgeClassification::selection),
            Some(CartridgeSelection::Supported(
                SupportedCartridgeFamily::Mbc2
            )),
        );
        assert_eq!(
            mbc2.persistence_metadata(),
            CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: false,
                profile: CartridgePersistenceProfile::PersistentRam {
                    ram: CartridgeRamPayloadKind::Mbc2Nibbles {
                        cell_count: MBC2_RAM_CELL_COUNT,
                    },
                },
            },
        );
        mbc2.write_rom(0x0000, 0x0A);
        mbc2.write_ram(0xA000, 0xAB);
        assert_eq!(mbc2.read_ram(0xA000), 0xFB);

        let mut restored_nibbles = [0_u8; MBC2_RAM_CELL_COUNT];
        restored_nibbles[0] = 0x0C;
        let restored_mbc2 = PersistentCartState::Mbc2Ram {
            ram_nibbles: restored_nibbles,
        };
        mbc2.restore_persistent_state(&restored_mbc2)
            .expect("MBC2 nibble state should restore");
        assert_eq!(mbc2.persistent_state(), restored_mbc2);
        assert_eq!(
            mbc2.restore_persistent_state(&PersistentCartState::None),
            Err(CartridgePersistentStateError::KindMismatch {
                expected: "Mbc2Ram",
                actual: "None",
            }),
        );
    }

    #[test]
    fn slot_accessors_and_restore_paths_cover_mbc3_and_mbc5_rtc_and_rumble_paths() {
        let mbc3_report = CartridgeSlot::load(
            build_banked_mbc3_rom(0x10, 0x03, 0x03),
            &CompatibilityPolicy::strict(),
        )
        .expect("MBC3+TIMER+RAM+BATTERY should load");
        let (mut mbc3, _) = mbc3_report.into_parts();
        assert_eq!(
            mbc3.classification()
                .map(CartridgeClassification::selection),
            Some(CartridgeSelection::Supported(
                SupportedCartridgeFamily::Mbc3
            )),
        );
        assert_eq!(
            mbc3.persistence_metadata(),
            CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: true,
                profile: CartridgePersistenceProfile::PersistentRamAndRtc {
                    ram: CartridgeRamPayloadKind::Linear {
                        byte_len: 32 * 1024,
                    },
                },
            },
        );
        mbc3.advance_rtc_seconds(3_661);
        assert_eq!(
            mbc3.persistent_state(),
            PersistentCartState::Mbc3RamRtc {
                ram: vec![0; 32 * 1024],
                rtc: Mbc3RtcPersistentState {
                    seconds: 1,
                    minutes: 1,
                    hours: 1,
                    day_counter: 0,
                    halt: false,
                    carry: false,
                },
            },
        );

        let restored_mbc3 = PersistentCartState::Mbc3RamRtc {
            ram: vec![0x6B; 32 * 1024],
            rtc: Mbc3RtcPersistentState {
                seconds: 9,
                minutes: 8,
                hours: 7,
                day_counter: 6,
                halt: true,
                carry: true,
            },
        };
        mbc3.restore_persistent_state(&restored_mbc3)
            .expect("MBC3 RAM+RTC state should restore");
        assert_eq!(mbc3.persistent_state(), restored_mbc3);
        assert_eq!(
            mbc3.restore_persistent_state(&PersistentCartState::Mbc3Ram {
                ram: vec![0; 32 * 1024],
            }),
            Err(CartridgePersistentStateError::KindMismatch {
                expected: "Mbc3RamRtc",
                actual: "Mbc3Ram",
            }),
        );
        assert_eq!(
            mbc3.restore_persistent_state(&PersistentCartState::Mbc3RamRtc {
                ram: vec![0; 4],
                rtc: Mbc3RtcPersistentState {
                    seconds: 0,
                    minutes: 0,
                    hours: 0,
                    day_counter: 0,
                    halt: false,
                    carry: false,
                },
            }),
            Err(CartridgePersistentStateError::RamLengthMismatch {
                expected: 32 * 1024,
                actual: 4,
            }),
        );

        let mbc5_report = CartridgeSlot::load(
            build_banked_mbc5_rom(0x1E, 0x03, 0x03),
            &CompatibilityPolicy::strict(),
        )
        .expect("MBC5+RUMBLE+RAM+BATTERY should load");
        let (mut mbc5, _) = mbc5_report.into_parts();
        assert_eq!(
            mbc5.classification()
                .map(CartridgeClassification::selection),
            Some(CartridgeSelection::Supported(
                SupportedCartridgeFamily::Mbc5
            )),
        );
        assert!(!mbc5.rumble_on());
        mbc5.advance_rtc_seconds(99);
        assert!(!mbc5.rumble_on());
        mbc5.write_rom(0x0000, 0x0A);
        mbc5.write_rom(0x4000, 0x0B);
        mbc5.write_ram(0xA000, 0x44);
        assert!(mbc5.rumble_on());
        assert_eq!(mbc5.read_ram(0xA000), 0x44);
        assert_eq!(
            mbc5.persistence_metadata(),
            CartridgePersistenceMetadata {
                has_battery: true,
                has_rtc: false,
                profile: CartridgePersistenceProfile::PersistentRam {
                    ram: CartridgeRamPayloadKind::Linear {
                        byte_len: 32 * 1024,
                    },
                },
            },
        );

        let restored_mbc5 = PersistentCartState::Mbc5Ram {
            ram: vec![0x24; 32 * 1024],
        };
        mbc5.restore_persistent_state(&restored_mbc5)
            .expect("MBC5 RAM state should restore");
        assert_eq!(mbc5.persistent_state(), restored_mbc5);
        assert_eq!(
            mbc5.restore_persistent_state(&PersistentCartState::NoMbcRam {
                ram: vec![0; 32 * 1024],
            }),
            Err(CartridgePersistentStateError::KindMismatch {
                expected: "Mbc5Ram",
                actual: "NoMbcRam",
            }),
        );
        assert_eq!(
            mbc5.restore_persistent_state(&PersistentCartState::Mbc5Ram { ram: vec![0; 8] }),
            Err(CartridgePersistentStateError::RamLengthMismatch {
                expected: 32 * 1024,
                actual: 8,
            }),
        );
    }

    #[test]
    fn private_validation_helpers_cover_remaining_mapper_rejection_and_signature_paths() {
        let strict = CompatibilityPolicy::strict();
        let mut diagnostics = Vec::new();

        let mbc2_header = CartridgeHeader::parse(&build_banked_mbc2_rom(0x06, 0x03, 0x02))
            .expect("header should parse");
        let mbc2_error = validate_mbc2(
            &mbc2_header,
            256 * 1024,
            &strict,
            &CartridgeClassification::classify(0x06),
            &mut diagnostics,
        )
        .expect_err("strict MBC2 validation should reject external-RAM metadata");
        assert!(matches!(mbc2_error, CartridgeLoadError::Rejected { .. }));

        diagnostics.clear();
        let mbc3_header = CartridgeHeader::parse(&build_banked_mbc3_rom(0x11, 0x03, 0x02))
            .expect("header should parse");
        let mbc3_error = validate_mbc3(
            &mbc3_header,
            256 * 1024,
            &strict,
            &CartridgeClassification::classify(0x11),
            &mut diagnostics,
        )
        .expect_err("strict no-RAM MBC3 validation should reject RAM metadata");
        assert!(matches!(mbc3_error, CartridgeLoadError::Rejected { .. }));

        diagnostics.clear();
        let mbc5_header = CartridgeHeader::parse(&build_banked_mbc5_rom(0x19, 0x03, 0x02))
            .expect("header should parse");
        let mbc5_error = validate_mbc5(
            &mbc5_header,
            256 * 1024,
            &strict,
            &CartridgeClassification::classify(0x19),
            &mut diagnostics,
        )
        .expect_err("strict no-RAM MBC5 validation should reject RAM metadata");
        assert!(matches!(mbc5_error, CartridgeLoadError::Rejected { .. }));

        let ordinary_mbc1_rom = build_banked_mbc1_rom_with_type(0x03, 0x05, 0x00);
        let ordinary_mbc1_header =
            CartridgeHeader::parse(&ordinary_mbc1_rom).expect("header should parse");
        assert!(!is_mbc1m_multicart_signature(
            &ordinary_mbc1_header,
            &ordinary_mbc1_rom,
        ));

        let mut mbc1m_candidate = build_banked_mbc1_rom_with_type(0x01, 0x05, 0x00);
        mark_mbc1_multicart_subheaders(&mut mbc1m_candidate);
        let mbc1m_header = CartridgeHeader::parse(&mbc1m_candidate).expect("header should parse");
        assert!(is_mbc1m_multicart_signature(
            &mbc1m_header,
            &mbc1m_candidate
        ));

        let title_bytes = b"NOTHING\0\0\0\0\0\0\0\0";
        assert!(is_wisdom_tree_signature(
            title_bytes,
            0x00,
            0x00,
            64 * 1024,
            0x00,
        ));
        assert!(is_wisdom_tree_signature(
            title_bytes,
            0xC0,
            0x00,
            NO_MBC_SUPPORTED_ROM_BYTES,
            0xD1,
        ));
        assert!(!is_wisdom_tree_signature(
            title_bytes,
            0x01,
            0x00,
            NO_MBC_SUPPORTED_ROM_BYTES,
            0x00,
        ));
    }

    #[test]
    fn private_validation_helpers_cover_remaining_size_code_and_image_mismatch_rejections() {
        let strict = CompatibilityPolicy::strict();
        let mut diagnostics = Vec::new();

        let mbc1_unknown_size = CartridgeHeader::parse(&build_test_rom(
            NO_MBC_SUPPORTED_ROM_BYTES,
            0x03,
            0xFF,
            0x00,
        ))
        .expect("header should parse");
        let mbc1_unknown_size_error = validate_mbc1(
            &mbc1_unknown_size,
            NO_MBC_SUPPORTED_ROM_BYTES,
            &strict,
            &CartridgeClassification::classify(0x03),
            &mut diagnostics,
        )
        .expect_err("unknown MBC1 ROM size code should fail");
        assert!(matches!(
            mbc1_unknown_size_error,
            CartridgeLoadError::Rejected { reason, .. }
                if reason.contains("unsupported ROM size code")
        ));

        diagnostics.clear();
        let mbc1_mismatch = CartridgeHeader::parse(&build_banked_mbc1_rom(0x03, 0x00))
            .expect("header should parse");
        let mbc1_mismatch_error = validate_mbc1(
            &mbc1_mismatch,
            128 * 1024,
            &strict,
            &CartridgeClassification::classify(0x03),
            &mut diagnostics,
        )
        .expect_err("MBC1 image-size mismatches should fail");
        assert!(matches!(
            mbc1_mismatch_error,
            CartridgeLoadError::Rejected { reason, .. }
                if reason.contains("loaded ROM is 131072 bytes")
        ));

        diagnostics.clear();
        let mbc1_invalid_layout = CartridgeHeader::parse(&build_banked_mbc1_rom(0x52, 0x00))
            .expect("header should parse");
        let mbc1_invalid_layout_error = validate_mbc1(
            &mbc1_invalid_layout,
            72 * 16 * 1024,
            &strict,
            &CartridgeClassification::classify(0x03),
            &mut diagnostics,
        )
        .expect_err("non-baseline MBC1 ROM sizes should fail");
        assert!(matches!(
            mbc1_invalid_layout_error,
            CartridgeLoadError::Rejected { reason, .. }
                if reason.contains("not valid for the current MBC1 baseline")
        ));

        diagnostics.clear();
        let mbc2_unknown_size = CartridgeHeader::parse(&build_test_rom(
            NO_MBC_SUPPORTED_ROM_BYTES,
            0x06,
            0xFF,
            0x00,
        ))
        .expect("header should parse");
        let mbc2_unknown_size_error = validate_mbc2(
            &mbc2_unknown_size,
            NO_MBC_SUPPORTED_ROM_BYTES,
            &strict,
            &CartridgeClassification::classify(0x06),
            &mut diagnostics,
        )
        .expect_err("unknown MBC2 ROM size code should fail");
        assert!(matches!(
            mbc2_unknown_size_error,
            CartridgeLoadError::Rejected { reason, .. }
                if reason.contains("unsupported ROM size code")
        ));

        diagnostics.clear();
        let mbc2_mismatch = CartridgeHeader::parse(&build_banked_mbc2_rom(0x06, 0x03, 0x00))
            .expect("header should parse");
        let mbc2_mismatch_error = validate_mbc2(
            &mbc2_mismatch,
            128 * 1024,
            &strict,
            &CartridgeClassification::classify(0x06),
            &mut diagnostics,
        )
        .expect_err("MBC2 image-size mismatches should fail");
        assert!(matches!(
            mbc2_mismatch_error,
            CartridgeLoadError::Rejected { reason, .. }
                if reason.contains("loaded ROM is 131072 bytes")
        ));

        diagnostics.clear();
        let mbc3_unknown_size = CartridgeHeader::parse(&build_test_rom(
            NO_MBC_SUPPORTED_ROM_BYTES,
            0x10,
            0xFF,
            0x03,
        ))
        .expect("header should parse");
        let mbc3_unknown_size_error = validate_mbc3(
            &mbc3_unknown_size,
            NO_MBC_SUPPORTED_ROM_BYTES,
            &strict,
            &CartridgeClassification::classify(0x10),
            &mut diagnostics,
        )
        .expect_err("unknown MBC3 ROM size code should fail");
        assert!(matches!(
            mbc3_unknown_size_error,
            CartridgeLoadError::Rejected { reason, .. }
                if reason.contains("unsupported ROM size code")
        ));

        diagnostics.clear();
        let mbc5_unknown_size = CartridgeHeader::parse(&build_test_rom(
            NO_MBC_SUPPORTED_ROM_BYTES,
            0x1A,
            0xFF,
            0x02,
        ))
        .expect("header should parse");
        let mbc5_unknown_size_error = validate_mbc5(
            &mbc5_unknown_size,
            NO_MBC_SUPPORTED_ROM_BYTES,
            &strict,
            &CartridgeClassification::classify(0x1A),
            &mut diagnostics,
        )
        .expect_err("unknown MBC5 ROM size code should fail");
        assert!(matches!(
            mbc5_unknown_size_error,
            CartridgeLoadError::Rejected { reason, .. }
                if reason.contains("unsupported ROM size code")
        ));

        diagnostics.clear();
        let mbc5_mismatch = CartridgeHeader::parse(&build_banked_mbc5_rom(0x1A, 0x03, 0x02))
            .expect("header should parse");
        let mbc5_mismatch_error = validate_mbc5(
            &mbc5_mismatch,
            128 * 1024,
            &strict,
            &CartridgeClassification::classify(0x1A),
            &mut diagnostics,
        )
        .expect_err("MBC5 image-size mismatches should fail");
        assert!(matches!(
            mbc5_mismatch_error,
            CartridgeLoadError::Rejected { reason, .. }
                if reason.contains("loaded ROM is 131072 bytes")
        ));
    }

    #[test]
    fn size_decoders_cover_extended_and_unknown_header_codes() {
        assert_eq!(
            RomSizeInfo::decode(0x52),
            RomSizeInfo {
                raw_code: 0x52,
                decoded_bytes: Some(72 * 16 * 1024),
                bank_count: Some(72),
            }
        );
        assert_eq!(
            RomSizeInfo::decode(0x53),
            RomSizeInfo {
                raw_code: 0x53,
                decoded_bytes: Some(80 * 16 * 1024),
                bank_count: Some(80),
            }
        );
        assert_eq!(
            RomSizeInfo::decode(0x54),
            RomSizeInfo {
                raw_code: 0x54,
                decoded_bytes: Some(96 * 16 * 1024),
                bank_count: Some(96),
            }
        );
        assert_eq!(
            RomSizeInfo::decode(0xFF),
            RomSizeInfo {
                raw_code: 0xFF,
                decoded_bytes: None,
                bank_count: None,
            }
        );

        assert_eq!(
            RamSizeInfo::decode(0x01),
            RamSizeInfo {
                raw_code: 0x01,
                decoded_bytes: Some(2 * 1024),
                bank_count: Some(1),
            }
        );
        assert_eq!(
            RamSizeInfo::decode(0x04),
            RamSizeInfo {
                raw_code: 0x04,
                decoded_bytes: Some(128 * 1024),
                bank_count: Some(16),
            }
        );
        assert_eq!(
            RamSizeInfo::decode(0x05),
            RamSizeInfo {
                raw_code: 0x05,
                decoded_bytes: Some(64 * 1024),
                bank_count: Some(8),
            }
        );
        assert_eq!(
            RamSizeInfo::decode(0xFF),
            RamSizeInfo {
                raw_code: 0xFF,
                decoded_bytes: None,
                bank_count: None,
            }
        );
    }

    #[test]
    fn header_parser_rejects_small_images_and_keeps_full_titles_without_terminators() {
        let error = CartridgeHeader::parse(&vec![0x00; HEADER_MINIMUM_ROM_LEN - 1])
            .expect_err("undersized images must be rejected");
        assert_eq!(
            error,
            CartridgeHeaderParseError::ImageTooSmall {
                actual_size: HEADER_MINIMUM_ROM_LEN - 1,
                minimum_size: HEADER_MINIMUM_ROM_LEN,
            }
        );

        let mut rom = build_test_rom(NO_MBC_SUPPORTED_ROM_BYTES, 0x09, 0x00, 0x02);
        rom[TITLE_START..=TITLE_END_INCLUSIVE].copy_from_slice(b"FULLTITLE1234567");

        let header = CartridgeHeader::parse(&rom).expect("header should parse");
        assert_eq!(header.title, "FULLTITLE123456");
    }
}
