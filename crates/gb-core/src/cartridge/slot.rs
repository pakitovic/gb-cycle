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

    pub(crate) fn capture_save_state(&self) -> CartridgeRuntimeSaveState {
        CartridgeRuntimeSaveState {
            device: self.device.as_ref().map(CartridgeDeviceSaveState::from),
        }
    }

    pub(crate) fn validate_save_state(
        &self,
        state: &CartridgeRuntimeSaveState,
    ) -> Result<(), CartridgeRuntimeSaveStateError> {
        let expected = self.state();
        let actual = state.slot_state();
        if expected != actual {
            return Err(CartridgeRuntimeSaveStateError::SlotStateMismatch { expected, actual });
        }

        match (&self.device, &state.device) {
            (None, None) => Ok(()),
            (
                Some(CartridgeDevice::NoMbc(current)),
                Some(CartridgeDeviceSaveState::NoMbc(saved)),
            ) => validate_optional_ram_shape("NoMBC RAM", &current.ram, &saved.ram),
            (
                Some(CartridgeDevice::Mmm01(current)),
                Some(CartridgeDeviceSaveState::Mmm01(saved)),
            ) => validate_optional_ram_shape("MMM01 RAM", &current.ram, &saved.ram),
            (Some(CartridgeDevice::M161(_)), Some(CartridgeDeviceSaveState::M161(_))) => Ok(()),
            (Some(CartridgeDevice::Huc1(current)), Some(CartridgeDeviceSaveState::Huc1(saved))) => {
                validate_optional_ram_shape("HuC-1 RAM", &current.ram, &saved.ram)
            }
            (Some(CartridgeDevice::Huc3(current)), Some(CartridgeDeviceSaveState::Huc3(saved))) => {
                validate_ram_shape("HuC-3 RAM", &current.ram, &saved.ram)
            }
            (Some(CartridgeDevice::Mbc1(current)), Some(CartridgeDeviceSaveState::Mbc1(saved))) => {
                validate_optional_ram_shape("MBC1 RAM", &current.ram, &saved.ram)
            }
            (Some(CartridgeDevice::Mbc2(_)), Some(CartridgeDeviceSaveState::Mbc2(_))) => Ok(()),
            (Some(CartridgeDevice::Mbc3(current)), Some(CartridgeDeviceSaveState::Mbc3(saved))) => {
                validate_optional_ram_shape("MBC3 RAM", &current.ram, &saved.ram)
            }
            (Some(CartridgeDevice::Mbc5(current)), Some(CartridgeDeviceSaveState::Mbc5(saved))) => {
                validate_optional_ram_shape("MBC5 RAM", &current.ram, &saved.ram)
            }
            (
                Some(CartridgeDevice::PocketCamera(current)),
                Some(CartridgeDeviceSaveState::PocketCamera(saved)),
            ) => {
                validate_ram_shape("Pocket Camera RAM", &current.ram, &saved.ram)?;
                validate_ram_shape(
                    "Pocket Camera host frame",
                    &current.host_frame,
                    &saved.host_frame,
                )
            }
            _ => unreachable!("slot state precheck should cover cartridge variant mismatches"),
        }
    }

    pub(crate) fn restore_save_state(&mut self, state: &CartridgeRuntimeSaveState) {
        debug_assert!(self.validate_save_state(state).is_ok());

        match (&mut self.device, &state.device) {
            (None, None) => {}
            (
                Some(CartridgeDevice::NoMbc(cartridge)),
                Some(CartridgeDeviceSaveState::NoMbc(state)),
            ) => {
                cartridge.ram.clone_from(&state.ram);
            }
            (
                Some(CartridgeDevice::Mmm01(cartridge)),
                Some(CartridgeDeviceSaveState::Mmm01(state)),
            ) => {
                cartridge.ram.clone_from(&state.ram);
                cartridge.mapped = state.mapped;
                cartridge.ram_enabled = state.ram_enabled;
                cartridge.ram_bank_mask = state.ram_bank_mask;
                cartridge.rom_bank_low = state.rom_bank_low;
                cartridge.rom_bank_mid = state.rom_bank_mid;
                cartridge.ram_bank_low = state.ram_bank_low;
                cartridge.ram_bank_high = state.ram_bank_high;
                cartridge.rom_bank_high = state.rom_bank_high;
                cartridge.mode_write_disable = state.mode_write_disable;
                cartridge.banking_mode = state.banking_mode;
                cartridge.rom_bank_mask = state.rom_bank_mask;
                cartridge.multiplex_enabled = state.multiplex_enabled;
            }
            (
                Some(CartridgeDevice::M161(cartridge)),
                Some(CartridgeDeviceSaveState::M161(state)),
            ) => {
                cartridge.selected_bank = state.selected_bank;
                cartridge.bank_switch_locked = state.bank_switch_locked;
                cartridge.last_bank_write = state.last_bank_write;
            }
            (
                Some(CartridgeDevice::Huc1(cartridge)),
                Some(CartridgeDeviceSaveState::Huc1(state)),
            ) => {
                cartridge.ram.clone_from(&state.ram);
                cartridge.io_mode = state.io_mode;
                cartridge.rom_bank = state.rom_bank;
                cartridge.ram_bank = state.ram_bank;
                cartridge.ir_emitter_on = state.ir_emitter_on;
                cartridge.ir_light_detected = state.ir_light_detected;
            }
            (
                Some(CartridgeDevice::Huc3(cartridge)),
                Some(CartridgeDeviceSaveState::Huc3(state)),
            ) => {
                cartridge.ram.clone_from(&state.ram);
                cartridge.select_mode = state.select_mode;
                cartridge.rom_bank = state.rom_bank;
                cartridge.ram_bank = state.ram_bank;
                cartridge.access_address = state.access_address;
                cartridge.mailbox = state.mailbox;
                cartridge.mcu_ram = state.mcu_ram;
                cartridge.rtc = state.rtc;
                cartridge.ir_emitter_on = state.ir_emitter_on;
                cartridge.ir_light_detected = state.ir_light_detected;
                cartridge.last_control_write = state.last_control_write;
                cartridge.last_unsupported_command = state.last_unsupported_command;
                cartridge.last_unsupported_argument = state.last_unsupported_argument;
            }
            (
                Some(CartridgeDevice::Mbc1(cartridge)),
                Some(CartridgeDeviceSaveState::Mbc1(state)),
            ) => {
                cartridge.ram.clone_from(&state.ram);
                cartridge.ram_enabled = state.ram_enabled;
                cartridge.rom_bank_low5 = state.rom_bank_low5;
                cartridge.secondary_bank = state.secondary_bank;
                cartridge.banking_mode = state.banking_mode;
            }
            (
                Some(CartridgeDevice::Mbc2(cartridge)),
                Some(CartridgeDeviceSaveState::Mbc2(state)),
            ) => {
                cartridge.ram_nibbles = state.ram_nibbles;
                cartridge.ram_enabled = state.ram_enabled;
                cartridge.rom_bank_low4 = state.rom_bank_low4;
            }
            (
                Some(CartridgeDevice::Mbc3(cartridge)),
                Some(CartridgeDeviceSaveState::Mbc3(state)),
            ) => {
                cartridge.ram.clone_from(&state.ram);
                cartridge.ram_rtc_enabled = state.ram_rtc_enabled;
                cartridge.rom_bank = state.rom_bank;
                cartridge.ram_or_rtc_select = state.ram_or_rtc_select;
                cartridge.rtc_live = state.rtc_live;
                cartridge.rtc_latched = state.rtc_latched;
                cartridge.rtc_latched_valid = state.rtc_latched_valid;
                cartridge.rtc_latch_armed = state.rtc_latch_armed;
                cartridge.rtc_access_ready_at = state.rtc_access_ready_at;
            }
            (
                Some(CartridgeDevice::Mbc5(cartridge)),
                Some(CartridgeDeviceSaveState::Mbc5(state)),
            ) => {
                cartridge.ram.clone_from(&state.ram);
                cartridge.ram_enabled = state.ram_enabled;
                cartridge.rom_bank_low8 = state.rom_bank_low8;
                cartridge.rom_bank_high1 = state.rom_bank_high1;
                cartridge.ram_bank_raw = state.ram_bank_raw;
                cartridge.rumble_on = state.rumble_on;
            }
            (
                Some(CartridgeDevice::PocketCamera(cartridge)),
                Some(CartridgeDeviceSaveState::PocketCamera(state)),
            ) => {
                cartridge.ram.clone_from(&state.ram);
                cartridge.ram_enabled = state.ram_enabled;
                cartridge.rom_bank = state.rom_bank;
                cartridge.ram_bank_or_register_select = state.ram_bank_or_register_select;
                cartridge.registers = state.registers;
                cartridge.host_frame.clone_from(&state.host_frame);
                cartridge.capture_state.clone_from(&state.capture_state);
            }
            _ => unreachable!("validated cartridge save-state should match loaded cartridge"),
        }
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

impl From<&CartridgeDevice> for CartridgeDeviceSaveState {
    fn from(device: &CartridgeDevice) -> Self {
        match device {
            CartridgeDevice::NoMbc(cartridge) => Self::NoMbc(NoMbcCartridgeSaveState {
                ram: cartridge.ram.clone(),
            }),
            CartridgeDevice::Mmm01(cartridge) => Self::Mmm01(Mmm01CartridgeSaveState {
                ram: cartridge.ram.clone(),
                mapped: cartridge.mapped,
                ram_enabled: cartridge.ram_enabled,
                ram_bank_mask: cartridge.ram_bank_mask,
                rom_bank_low: cartridge.rom_bank_low,
                rom_bank_mid: cartridge.rom_bank_mid,
                ram_bank_low: cartridge.ram_bank_low,
                ram_bank_high: cartridge.ram_bank_high,
                rom_bank_high: cartridge.rom_bank_high,
                mode_write_disable: cartridge.mode_write_disable,
                banking_mode: cartridge.banking_mode,
                rom_bank_mask: cartridge.rom_bank_mask,
                multiplex_enabled: cartridge.multiplex_enabled,
            }),
            CartridgeDevice::M161(cartridge) => Self::M161(M161CartridgeSaveState {
                selected_bank: cartridge.selected_bank,
                bank_switch_locked: cartridge.bank_switch_locked,
                last_bank_write: cartridge.last_bank_write,
            }),
            CartridgeDevice::Huc1(cartridge) => Self::Huc1(Huc1CartridgeSaveState {
                ram: cartridge.ram.clone(),
                io_mode: cartridge.io_mode,
                rom_bank: cartridge.rom_bank,
                ram_bank: cartridge.ram_bank,
                ir_emitter_on: cartridge.ir_emitter_on,
                ir_light_detected: cartridge.ir_light_detected,
            }),
            CartridgeDevice::Huc3(cartridge) => Self::Huc3(Huc3CartridgeSaveState {
                ram: cartridge.ram.clone(),
                select_mode: cartridge.select_mode,
                rom_bank: cartridge.rom_bank,
                ram_bank: cartridge.ram_bank,
                access_address: cartridge.access_address,
                mailbox: cartridge.mailbox,
                mcu_ram: cartridge.mcu_ram,
                rtc: cartridge.rtc,
                ir_emitter_on: cartridge.ir_emitter_on,
                ir_light_detected: cartridge.ir_light_detected,
                last_control_write: cartridge.last_control_write,
                last_unsupported_command: cartridge.last_unsupported_command,
                last_unsupported_argument: cartridge.last_unsupported_argument,
            }),
            CartridgeDevice::Mbc1(cartridge) => Self::Mbc1(Mbc1CartridgeSaveState {
                ram: cartridge.ram.clone(),
                ram_enabled: cartridge.ram_enabled,
                rom_bank_low5: cartridge.rom_bank_low5,
                secondary_bank: cartridge.secondary_bank,
                banking_mode: cartridge.banking_mode,
            }),
            CartridgeDevice::Mbc2(cartridge) => Self::Mbc2(Mbc2CartridgeSaveState {
                ram_nibbles: cartridge.ram_nibbles,
                ram_enabled: cartridge.ram_enabled,
                rom_bank_low4: cartridge.rom_bank_low4,
            }),
            CartridgeDevice::Mbc3(cartridge) => Self::Mbc3(Mbc3CartridgeSaveState {
                ram: cartridge.ram.clone(),
                ram_rtc_enabled: cartridge.ram_rtc_enabled,
                rom_bank: cartridge.rom_bank,
                ram_or_rtc_select: cartridge.ram_or_rtc_select,
                rtc_live: cartridge.rtc_live,
                rtc_latched: cartridge.rtc_latched,
                rtc_latched_valid: cartridge.rtc_latched_valid,
                rtc_latch_armed: cartridge.rtc_latch_armed,
                rtc_access_ready_at: cartridge.rtc_access_ready_at,
            }),
            CartridgeDevice::Mbc5(cartridge) => Self::Mbc5(Mbc5CartridgeSaveState {
                ram: cartridge.ram.clone(),
                ram_enabled: cartridge.ram_enabled,
                rom_bank_low8: cartridge.rom_bank_low8,
                rom_bank_high1: cartridge.rom_bank_high1,
                ram_bank_raw: cartridge.ram_bank_raw,
                rumble_on: cartridge.rumble_on,
            }),
            CartridgeDevice::PocketCamera(cartridge) => {
                Self::PocketCamera(PocketCameraCartridgeSaveState {
                    ram: cartridge.ram.clone(),
                    ram_enabled: cartridge.ram_enabled,
                    rom_bank: cartridge.rom_bank,
                    ram_bank_or_register_select: cartridge.ram_bank_or_register_select,
                    registers: cartridge.registers,
                    host_frame: cartridge.host_frame.clone(),
                    capture_state: cartridge.capture_state.clone(),
                })
            }
        }
    }
}
