use super::classify::{classify_loaded_cartridge, unsupported_load_reason};
use super::validate::{
    validate_huc1, validate_huc3, validate_m161, validate_mbc1, validate_mbc2, validate_mbc3,
    validate_mbc5, validate_mmm01, validate_no_mbc, validate_pocket_camera,
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
        let header =
            CartridgeHeader::parse_for_load(&rom_bytes).map_err(CartridgeLoadError::HeaderParse)?;
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
            CartridgeSelection::Supported(SupportedCartridgeFamily::Mmm01) => {
                let ram_len = validate_mmm01(
                    &header,
                    rom_bytes.len(),
                    compatibility,
                    &classification,
                    &mut diagnostics,
                )?;

                let has_battery = matches!(classification.raw_type(), 0x0D);
                let ram = (ram_len != 0).then(|| vec![0; ram_len]);
                let cartridge = Self {
                    device: Some(CartridgeDevice::Mmm01(Mmm01Cartridge {
                        rom: rom_bytes,
                        ram,
                        has_battery,
                        header,
                        classification,
                        mapped: false,
                        ram_enabled: false,
                        ram_bank_mask: 0,
                        rom_bank_low: 0,
                        rom_bank_mid: 0,
                        ram_bank_low: 0,
                        ram_bank_high: 0,
                        rom_bank_high: 0,
                        mode_write_disable: false,
                        banking_mode: 0,
                        rom_bank_mask: 0,
                        multiplex_enabled: false,
                    })),
                };

                Ok(CartridgeLoadReport {
                    cartridge,
                    diagnostics,
                })
            }
            CartridgeSelection::Supported(SupportedCartridgeFamily::M161) => {
                validate_m161(
                    &header,
                    rom_bytes.len(),
                    compatibility,
                    &classification,
                    &mut diagnostics,
                )?;

                let cartridge = Self {
                    device: Some(CartridgeDevice::M161(M161Cartridge {
                        rom: rom_bytes,
                        header,
                        classification,
                        selected_bank: 0,
                        bank_switch_locked: false,
                        last_bank_write: None,
                    })),
                };

                Ok(CartridgeLoadReport {
                    cartridge,
                    diagnostics,
                })
            }
            CartridgeSelection::Supported(SupportedCartridgeFamily::Huc1) => {
                let ram_len = validate_huc1(
                    &header,
                    rom_bytes.len(),
                    compatibility,
                    &classification,
                    &mut diagnostics,
                )?;

                let cartridge = Self {
                    device: Some(CartridgeDevice::Huc1(Huc1Cartridge {
                        rom: rom_bytes,
                        ram: Some(vec![0; ram_len]),
                        has_battery: true,
                        header,
                        classification,
                        io_mode: Huc1IoMode::Ram,
                        rom_bank: 0,
                        ram_bank: 0,
                        ir_emitter_on: false,
                        ir_light_detected: false,
                    })),
                };

                Ok(CartridgeLoadReport {
                    cartridge,
                    diagnostics,
                })
            }
            CartridgeSelection::Supported(SupportedCartridgeFamily::Huc3) => {
                let ram_len = validate_huc3(
                    &header,
                    rom_bytes.len(),
                    compatibility,
                    &classification,
                    &mut diagnostics,
                )?;

                let mut mapper = Huc3Cartridge {
                    rom: rom_bytes,
                    ram: vec![0; ram_len],
                    has_battery: true,
                    header,
                    classification,
                    select_mode: Huc3SelectMode::RamReadOnly,
                    rom_bank: 0,
                    ram_bank: 0,
                    access_address: 0,
                    mailbox: Huc3Mailbox::default(),
                    mcu_ram: [0; HUC3_MCU_RAM_NIBBLE_COUNT],
                    rtc: Huc3RtcState::default(),
                    ir_emitter_on: false,
                    ir_light_detected: false,
                    last_control_write: None,
                    last_unsupported_command: None,
                    last_unsupported_argument: None,
                };
                mapper.initialize_runtime_state();
                let cartridge = Self {
                    device: Some(CartridgeDevice::Huc3(mapper)),
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
                let ram = (layout.ram_len != 0).then(|| vec![0; layout.ram_len]);
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
                        rtc_access_ready_at: None,
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
            CartridgeSelection::Supported(SupportedCartridgeFamily::PocketCamera) => {
                validate_pocket_camera(
                    &header,
                    rom_bytes.len(),
                    compatibility,
                    &classification,
                    &mut diagnostics,
                )?;

                let cartridge = Self {
                    device: Some(CartridgeDevice::PocketCamera(PocketCameraCartridge {
                        rom: rom_bytes,
                        ram: vec![0; POCKET_CAMERA_SUPPORTED_RAM_BYTES],
                        header,
                        classification,
                        ram_enabled: false,
                        rom_bank: 1,
                        ram_bank_or_register_select: 0,
                        registers: [0; POCKET_CAMERA_REGISTER_COUNT],
                        host_frame: PocketCameraCartridge::placeholder_frame(),
                        capture_state: PocketCameraCaptureState::Idle,
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
            Some(CartridgeDevice::Mmm01(_)) => CartridgeSlotState::Mmm01,
            Some(CartridgeDevice::M161(_)) => CartridgeSlotState::M161,
            Some(CartridgeDevice::Huc1(_)) => CartridgeSlotState::Huc1,
            Some(CartridgeDevice::Huc3(_)) => CartridgeSlotState::Huc3,
            Some(CartridgeDevice::Mbc1(_)) => CartridgeSlotState::Mbc1,
            Some(CartridgeDevice::Mbc2(_)) => CartridgeSlotState::Mbc2,
            Some(CartridgeDevice::Mbc3(_)) => CartridgeSlotState::Mbc3,
            Some(CartridgeDevice::Mbc5(_)) => CartridgeSlotState::Mbc5,
            Some(CartridgeDevice::PocketCamera(_)) => CartridgeSlotState::PocketCamera,
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

    pub fn rom_fingerprint(&self) -> Option<SaveStateByteFingerprint> {
        self.device.as_ref().map(CartridgeDevice::rom_fingerprint)
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

    pub fn describe_external_access(&self, address: u16) -> CartridgeExternalAccessInfo {
        self.device
            .as_ref()
            .map_or(CartridgeExternalAccessInfo::no_device(address), |device| {
                device.describe_external_access(address)
            })
    }

    pub fn rtc_access_ready_at(&self) -> Option<TCycle> {
        self.device
            .as_ref()
            .and_then(CartridgeDevice::rtc_access_ready_at)
    }

    pub(crate) fn read_ram_timed(&mut self, address: u16, t_cycle: TCycle) -> u8 {
        self.device
            .as_mut()
            .map_or(RAM_ABSENT_READ_VALUE, |device| {
                device.read_ram_timed(address, t_cycle)
            })
    }

    pub fn write_ram(&mut self, address: u16, value: u8) {
        if let Some(device) = &mut self.device {
            device.write_ram(address, value);
        }
    }

    pub(crate) fn write_ram_timed(&mut self, address: u16, value: u8, t_cycle: TCycle) {
        if let Some(device) = &mut self.device {
            device.write_ram_timed(address, value, t_cycle);
        }
    }

    pub fn snapshot(&self) -> CartridgeSnapshot {
        CartridgeSnapshot {
            state: self.state(),
            rtc_access_ready_at: self.rtc_access_ready_at(),
            camera_capture_ready_at: self
                .device
                .as_ref()
                .and_then(CartridgeDevice::camera_capture_ready_at),
            camera_registers_selected: self
                .device
                .as_ref()
                .is_some_and(CartridgeDevice::camera_registers_selected),
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

    pub fn has_pocket_camera(&self) -> bool {
        self.device
            .as_ref()
            .is_some_and(CartridgeDevice::has_pocket_camera)
    }

    pub fn set_pocket_camera_frame(
        &mut self,
        frame: PocketCameraFrame,
    ) -> Result<(), PocketCameraFrameError> {
        match &mut self.device {
            Some(device) => device.set_pocket_camera_frame(frame),
            None => Err(PocketCameraFrameError::UnsupportedCartridge),
        }
    }

    pub fn clear_pocket_camera_frame(&mut self) -> Result<(), PocketCameraFrameError> {
        match &mut self.device {
            Some(device) => device.clear_pocket_camera_frame(),
            None => Err(PocketCameraFrameError::UnsupportedCartridge),
        }
    }

    pub(crate) fn advance_rtc_seconds(&mut self, seconds: u64) {
        if let Some(device) = &mut self.device {
            device.advance_rtc_seconds(seconds);
        }
    }

    pub fn trace_summary(&self) -> String {
        let detail = self
            .device
            .as_ref()
            .map(CartridgeDevice::trace_summary)
            .unwrap_or_default();
        format!("state={:?}{}", self.state(), detail)
    }

    pub fn scheduler_trace_message(&self, context: &CycleContext) -> String {
        format!(
            "t_cycle={} phase={} {}",
            context.t_cycle().get(),
            context.phase(),
            self.trace_summary(),
        )
    }
}
