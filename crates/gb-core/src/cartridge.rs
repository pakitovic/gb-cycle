use crate::model::{CompatibilityPolicy, ExecutionMode, ValidationPolicy};
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
const HEADER_CHECKSUM_ADDRESS: usize = 0x014D;

const RAM_ABSENT_READ_VALUE: u8 = 0xFF;
const NO_MBC_SUPPORTED_ROM_BYTES: usize = 32 * 1024;
const NO_MBC_SUPPORTED_RAM_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartridgeSlotState {
    Empty,
    NoMbc,
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum CartridgeDevice {
    NoMbc(NoMbcCartridge),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NoMbcCartridge {
    rom: Vec<u8>,
    ram: Option<Vec<u8>>,
    has_battery: bool,
    header: CartridgeHeader,
    classification: CartridgeClassification,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartridgeSnapshot {
    pub state: CartridgeSlotState,
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
        let classification = CartridgeClassification::classify(header.cartridge_type);
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
            CartridgeSelection::Supported(family) => Err(CartridgeLoadError::Rejected {
                classification,
                execution_mode: compatibility.execution_mode,
                reason: format!(
                    "{} is recognized as {:?}, but that runtime family is reserved for a later phase",
                    classification.detected_name(),
                    family
                ),
                diagnostics,
            }),
            CartridgeSelection::Unsupported(category) => Err(CartridgeLoadError::Rejected {
                classification,
                execution_mode: compatibility.execution_mode,
                reason: format!(
                    "{} ({category:?}) is not implemented: {}",
                    classification.detected_name(),
                    classification.reason()
                ),
                diagnostics,
            }),
        }
    }

    pub fn state(&self) -> CartridgeSlotState {
        match self.device {
            None => CartridgeSlotState::Empty,
            Some(CartridgeDevice::NoMbc(_)) => CartridgeSlotState::NoMbc,
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
        }
    }

    fn classification(&self) -> CartridgeClassification {
        match self {
            Self::NoMbc(cartridge) => cartridge.classification,
        }
    }

    fn read_rom(&self, address: u16) -> u8 {
        match self {
            Self::NoMbc(cartridge) => cartridge.read_rom(address),
        }
    }

    fn write_rom(&mut self, address: u16, value: u8) {
        match self {
            Self::NoMbc(cartridge) => cartridge.write_rom(address, value),
        }
    }

    fn read_ram(&self, address: u16) -> u8 {
        match self {
            Self::NoMbc(cartridge) => cartridge.read_ram(address),
        }
    }

    fn write_ram(&mut self, address: u16, value: u8) {
        match self {
            Self::NoMbc(cartridge) => cartridge.write_ram(address, value),
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
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} expects RAM size code {expected_ram_code:#04X}, but the header declared {:#04X}",
                classification.detected_name(),
                header.ram_size.raw_code
            ),
            diagnostics: diagnostics.clone(),
        });
    }

    if header.ram_size.decoded_bytes != Some(expected_ram_code_decompressed(expected_ram_code)) {
        return Err(CartridgeLoadError::Rejected {
            classification: *classification,
            execution_mode: compatibility.execution_mode,
            reason: format!(
                "{} resolved to an unsupported RAM configuration from code {:#04X}",
                classification.detected_name(),
                header.ram_size.raw_code
            ),
            diagnostics: diagnostics.clone(),
        });
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
    CartridgeClassification {
        raw_type,
        detected_name,
        selection: CartridgeSelection::Supported(family),
        reason: "supported cartridge family",
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
    fn loading_known_but_unimplemented_supported_family_fails_explicitly() {
        let rom = build_test_rom(128 * 1024, 0x01, 0x02, 0x00);
        let error = CartridgeSlot::load(rom, &CompatibilityPolicy::strict())
            .expect_err("MBC1 should stay reserved");

        match error {
            CartridgeLoadError::Rejected {
                classification,
                reason,
                ..
            } => {
                assert_eq!(
                    classification.selection(),
                    CartridgeSelection::Supported(SupportedCartridgeFamily::Mbc1)
                );
                assert!(reason.contains("reserved for a later phase"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
