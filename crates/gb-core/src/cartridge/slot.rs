use super::classify::{classify_loaded_cartridge, unsupported_load_reason};
use super::validate::{
    validate_mbc1, validate_mbc2, validate_mbc3, validate_mbc5, validate_no_mbc,
};
use super::*;
use crate::model::CompatibilityPolicy;
use crate::scheduler::CycleContext;

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
                        rtc_latch_armed: false,
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

    pub fn has_rumble(&self) -> bool {
        self.device
            .as_ref()
            .is_some_and(CartridgeDevice::has_rumble)
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
