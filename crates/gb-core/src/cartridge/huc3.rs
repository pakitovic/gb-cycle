use super::*;

impl Huc3SelectMode {
    fn from_value(value: u8) -> Self {
        match value & 0x0F {
            0x00 => Self::RamReadOnly,
            0x0A => Self::RamReadWrite,
            0x0B => Self::RtcCommandArgument,
            0x0C => Self::RtcCommandResponse,
            0x0D => Self::RtcSemaphore,
            0x0E => Self::Ir,
            raw => Self::OpenBus(raw),
        }
    }

    fn raw_value(self) -> u8 {
        match self {
            Self::RamReadOnly => 0x00,
            Self::RamReadWrite => 0x0A,
            Self::RtcCommandArgument => 0x0B,
            Self::RtcCommandResponse => 0x0C,
            Self::RtcSemaphore => 0x0D,
            Self::Ir => 0x0E,
            Self::OpenBus(raw) => raw & 0x0F,
        }
    }
}

impl Default for Huc3Mailbox {
    fn default() -> Self {
        Self {
            command: 0,
            argument: 0,
            last_response_nybble: 0,
            semaphore_ready: true,
        }
    }
}

impl Huc3Cartridge {
    pub(in crate::cartridge) fn initialize_runtime_state(&mut self) {
        self.rtc.current_minutes_of_day %= HUC3_MINUTES_PER_DAY;
        self.rtc.current_days %= HUC3_DAY_COUNTER_MODULUS;
        self.rtc.current_subminute_seconds %= 60;
        self.rtc.event_minutes_of_day %= HUC3_MINUTES_PER_DAY;
        self.rtc.event_days %= HUC3_DAY_COUNTER_MODULUS;
        self.mask_mcu_nibbles();
        self.sync_tracked_rtc_locations();
    }

    pub(in crate::cartridge) fn trace_summary(&self) -> String {
        format!(
            " select_mode={:?} rom_bank_raw={:#04X} effective_rom_bank={:#04X} ram_bank_raw={:#04X} effective_ram_bank={} access_address={:#04X} mailbox_command={:#03X} mailbox_argument={:#03X} last_response={:#03X} semaphore_ready={} ir_emitter_on={} ir_light_detected={} tone_enabled={} tone_selection={:#03X} last_control_write={:?} last_unsupported={:?}",
            self.select_mode,
            self.rom_bank,
            self.effective_rom_bank(self.header.rom_size.bank_count.unwrap_or(0)),
            self.ram_bank,
            self.effective_ram_bank(),
            self.access_address,
            self.mailbox.command,
            self.mailbox.argument,
            self.mailbox.last_response_nybble,
            self.mailbox.semaphore_ready,
            self.ir_emitter_on,
            self.ir_light_detected,
            self.tone_enabled(),
            self.tone_selection(),
            self.last_control_write,
            self.last_unsupported_command
                .zip(self.last_unsupported_argument),
        )
    }

    pub(in crate::cartridge) fn describe_external_access(
        &self,
        address: u16,
    ) -> CartridgeExternalAccessInfo {
        match self.select_mode {
            Huc3SelectMode::RamReadOnly => CartridgeExternalAccessInfo::new(
                address,
                CartridgeExternalTarget::BankedRam {
                    bank: self.effective_ram_bank(),
                },
                CartridgeExternalAvailability::Accessible,
                CartridgeExternalReadBehavior::Storage,
                CartridgeExternalWriteBehavior::Ignored,
            ),
            Huc3SelectMode::RamReadWrite => CartridgeExternalAccessInfo::new(
                address,
                CartridgeExternalTarget::BankedRam {
                    bank: self.effective_ram_bank(),
                },
                CartridgeExternalAvailability::Accessible,
                CartridgeExternalReadBehavior::Storage,
                CartridgeExternalWriteBehavior::Storage,
            ),
            Huc3SelectMode::RtcCommandArgument => CartridgeExternalAccessInfo::new(
                address,
                CartridgeExternalTarget::Huc3CommandMailbox,
                CartridgeExternalAvailability::Accessible,
                CartridgeExternalReadBehavior::OpenBus,
                CartridgeExternalWriteBehavior::Huc3MailboxCommandArgument,
            ),
            Huc3SelectMode::RtcCommandResponse => CartridgeExternalAccessInfo::new(
                address,
                CartridgeExternalTarget::Huc3ResponseMailbox,
                CartridgeExternalAvailability::Accessible,
                CartridgeExternalReadBehavior::Huc3MailboxResponse,
                CartridgeExternalWriteBehavior::Ignored,
            ),
            Huc3SelectMode::RtcSemaphore => CartridgeExternalAccessInfo::new(
                address,
                CartridgeExternalTarget::Huc3Semaphore,
                CartridgeExternalAvailability::Accessible,
                CartridgeExternalReadBehavior::Huc3SemaphoreReady,
                CartridgeExternalWriteBehavior::Huc3SemaphoreControl,
            ),
            Huc3SelectMode::Ir => CartridgeExternalAccessInfo::new(
                address,
                CartridgeExternalTarget::IrRegister,
                CartridgeExternalAvailability::Accessible,
                CartridgeExternalReadBehavior::InfraredSensor,
                CartridgeExternalWriteBehavior::InfraredTransmitter,
            ),
            Huc3SelectMode::OpenBus(raw) => CartridgeExternalAccessInfo::new(
                address,
                CartridgeExternalTarget::Huc3InvalidSelector(raw),
                CartridgeExternalAvailability::Reserved,
                CartridgeExternalReadBehavior::OpenBus,
                CartridgeExternalWriteBehavior::Ignored,
            ),
        }
    }

    pub(in crate::cartridge) fn read_rom(&self, address: u16) -> u8 {
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

    pub(in crate::cartridge) fn mapped_rom_window(
        &self,
        address: u16,
    ) -> Option<CartridgeMappedRomWindow> {
        if address >= 0x8000 {
            return None;
        }

        let bank = if address < 0x4000 {
            0
        } else {
            self.effective_rom_bank(self.header.rom_size.bank_count.unwrap_or(0))
        };
        let bank_offset = if address < 0x4000 {
            address as usize
        } else {
            address as usize - 0x4000
        };
        Some(CartridgeMappedRomWindow::rom(bank, 0x4000, bank_offset))
    }

    pub(in crate::cartridge) fn write_rom(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1FFF => {
                self.select_mode = Huc3SelectMode::from_value(value);
            }
            0x2000..=0x3FFF => {
                self.rom_bank = value & 0x7F;
            }
            0x4000..=0x5FFF => {
                self.ram_bank = value & 0x03;
            }
            0x6000..=0x7FFF => {
                self.last_control_write = Some(value);
            }
            _ => {}
        }
    }

    pub(in crate::cartridge) fn read_ram(&self, address: u16) -> u8 {
        match self.select_mode {
            Huc3SelectMode::RamReadOnly | Huc3SelectMode::RamReadWrite => self
                .ram
                .get(self.effective_ram_offset(address))
                .copied()
                .unwrap_or(RAM_ABSENT_READ_VALUE),
            Huc3SelectMode::RtcCommandArgument => 0xFF,
            Huc3SelectMode::RtcCommandResponse => {
                0x80 | ((self.mailbox.command & 0x07) << 4)
                    | (self.mailbox.last_response_nybble & 0x0F)
            }
            Huc3SelectMode::RtcSemaphore => 0x80 | self.mailbox.semaphore_ready as u8,
            Huc3SelectMode::Ir => 0x80 | self.ir_light_detected as u8,
            Huc3SelectMode::OpenBus(_) => 0xFF,
        }
    }

    pub(in crate::cartridge) fn write_ram(&mut self, address: u16, value: u8) {
        match self.select_mode {
            Huc3SelectMode::RamReadOnly => {}
            Huc3SelectMode::RamReadWrite => {
                let offset = self.effective_ram_offset(address);
                if let Some(byte) = self.ram.get_mut(offset) {
                    *byte = value;
                }
            }
            Huc3SelectMode::RtcCommandArgument => {
                self.mailbox.command = (value >> 4) & 0x07;
                self.mailbox.argument = value & 0x0F;
                self.last_unsupported_command = None;
                self.last_unsupported_argument = None;
            }
            Huc3SelectMode::RtcCommandResponse => {}
            Huc3SelectMode::RtcSemaphore => {
                if value & 0x01 == 0 {
                    self.request_mcu_command_execution();
                }
            }
            Huc3SelectMode::Ir => {
                self.ir_emitter_on = value & 0x01 != 0;
            }
            Huc3SelectMode::OpenBus(_) => {}
        }
    }

    pub(in crate::cartridge) fn advance_rtc_seconds(&mut self, elapsed_seconds: u64) {
        let mut persisted = Huc3RtcPersistentState::from(self.rtc);
        persisted.apply_elapsed_seconds(elapsed_seconds);
        self.rtc = persisted.into();
        self.sync_tracked_rtc_locations();
    }

    pub(in crate::cartridge) fn persistence_metadata(&self) -> CartridgePersistenceMetadata {
        CartridgePersistenceMetadata {
            has_battery: self.has_battery,
            has_rtc: true,
            profile: CartridgePersistenceProfile::PersistentRamAndRtc {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: self.ram.len(),
                },
            },
        }
    }

    pub(in crate::cartridge) fn persistent_state(&self) -> PersistentCartState {
        if !self.has_battery {
            return PersistentCartState::None;
        }

        PersistentCartState::Huc3 {
            ram: self.ram.clone(),
            mcu_ram: self.mcu_ram,
            rtc: self.rtc.into(),
            rom_bank: self.rom_bank,
            ram_bank: self.ram_bank,
            select_mode: self.select_mode.raw_value(),
            access_address: self.access_address,
            mailbox_command: self.mailbox.command,
            mailbox_argument: self.mailbox.argument,
            last_response_nybble: self.mailbox.last_response_nybble,
            semaphore_ready: self.mailbox.semaphore_ready,
            ir_emitter_on: self.ir_emitter_on,
            ir_light_detected: self.ir_light_detected,
            last_control_write: self.last_control_write,
            last_unsupported_command: self.last_unsupported_command,
            last_unsupported_argument: self.last_unsupported_argument,
        }
    }

    pub(in crate::cartridge) fn restore_persistent_state(
        &mut self,
        state: &PersistentCartState,
    ) -> Result<(), CartridgePersistentStateError> {
        match (self.has_battery, state) {
            (false, PersistentCartState::None) => Ok(()),
            (
                true,
                PersistentCartState::Huc3 {
                    ram: persisted_ram,
                    mcu_ram,
                    rtc,
                    rom_bank,
                    ram_bank,
                    select_mode,
                    access_address,
                    mailbox_command,
                    mailbox_argument,
                    last_response_nybble,
                    semaphore_ready,
                    ir_emitter_on,
                    ir_light_detected,
                    last_control_write,
                    last_unsupported_command,
                    last_unsupported_argument,
                },
            ) => {
                if self.ram.len() != persisted_ram.len() {
                    return Err(CartridgePersistentStateError::RamLengthMismatch {
                        expected: self.ram.len(),
                        actual: persisted_ram.len(),
                    });
                }
                for (index, value) in mcu_ram.iter().copied().enumerate() {
                    if value > 0x0F {
                        return Err(CartridgePersistentStateError::InvalidHuc3NibbleValue {
                            index,
                            value,
                        });
                    }
                }

                self.ram.copy_from_slice(persisted_ram);
                self.mcu_ram = *mcu_ram;
                self.rtc = (*rtc).into();
                self.rom_bank = *rom_bank & 0x7F;
                self.ram_bank = *ram_bank & 0x03;
                self.select_mode = Huc3SelectMode::from_value(*select_mode);
                self.access_address = *access_address;
                self.mailbox = Huc3Mailbox {
                    command: *mailbox_command & 0x07,
                    argument: *mailbox_argument & 0x0F,
                    last_response_nybble: *last_response_nybble & 0x0F,
                    semaphore_ready: *semaphore_ready,
                };
                self.ir_emitter_on = *ir_emitter_on;
                self.ir_light_detected = *ir_light_detected;
                self.last_control_write = *last_control_write;
                self.last_unsupported_command = last_unsupported_command.map(|value| value & 0x07);
                self.last_unsupported_argument =
                    last_unsupported_argument.map(|value| value & 0x0F);
                self.initialize_runtime_state();
                Ok(())
            }
            (true, other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "Huc3",
                actual: other.kind_name(),
            }),
            (false, other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "None",
                actual: other.kind_name(),
            }),
        }
    }

    fn request_mcu_command_execution(&mut self) {
        self.mailbox.semaphore_ready = false;
        self.execute_mailbox_command();
        self.mailbox.semaphore_ready = true;
    }

    fn execute_mailbox_command(&mut self) {
        self.last_unsupported_command = None;
        self.last_unsupported_argument = None;

        match self.mailbox.command & 0x07 {
            0x01 => {
                self.mailbox.last_response_nybble =
                    self.mcu_ram[self.access_address as usize] & 0x0F;
                self.access_address = self.access_address.wrapping_add(1);
            }
            0x03 => {
                let address = self.access_address;
                self.write_mcu_nibble(address, self.mailbox.argument);
                self.access_address = self.access_address.wrapping_add(1);
            }
            0x04 => {
                self.access_address = (self.access_address & 0xF0) | (self.mailbox.argument & 0x0F);
            }
            0x05 => {
                self.access_address =
                    (self.access_address & 0x0F) | ((self.mailbox.argument & 0x0F) << 4);
            }
            0x06 => self.execute_extended_command(self.mailbox.argument),
            other => {
                self.last_unsupported_command = Some(other);
                self.last_unsupported_argument = Some(self.mailbox.argument & 0x0F);
            }
        }
    }

    fn execute_extended_command(&mut self, argument: u8) {
        match argument & 0x0F {
            0x00 => self.copy_current_time_to_io_window(),
            0x01 => self.apply_io_window_current_time_preserving_event_delta(),
            0x02 => self.mailbox.last_response_nybble = 0x01,
            other => {
                self.last_unsupported_command = Some(0x06);
                self.last_unsupported_argument = Some(other);
            }
        }
    }

    fn copy_current_time_to_io_window(&mut self) {
        self.write_triplet_nybbles(0x00, self.rtc.current_minutes_of_day);
        self.write_triplet_nybbles(0x03, self.rtc.current_days);
        self.mcu_ram[0x06] = 0;
    }

    fn apply_io_window_current_time_preserving_event_delta(&mut self) {
        let remaining_minutes = self.remaining_event_delta_minutes();
        self.rtc.current_minutes_of_day = self.read_triplet_nybbles(0x00) % HUC3_MINUTES_PER_DAY;
        self.rtc.current_days = self.read_triplet_nybbles(0x03) % HUC3_DAY_COUNTER_MODULUS;
        self.rtc.current_subminute_seconds = 0;

        let cycle = Self::rtc_cycle_minutes();
        let new_event_total = (self.current_total_minutes() + remaining_minutes) % cycle;
        self.set_event_total_minutes(new_event_total);
        self.sync_tracked_rtc_locations();
    }

    fn tone_enabled(&self) -> bool {
        self.mcu_ram[0x27] == 0x01
    }

    fn tone_selection(&self) -> u8 {
        self.mcu_ram[0x26] & 0x03
    }

    fn effective_rom_bank(&self, bank_count: usize) -> usize {
        if bank_count == 0 {
            return 0;
        }
        (self.rom_bank as usize) % bank_count
    }

    fn effective_ram_bank_count(&self) -> usize {
        (self.ram.len() / 0x2000).max(1)
    }

    fn effective_ram_bank(&self) -> u8 {
        (self.ram_bank as usize % self.effective_ram_bank_count()) as u8
    }

    fn effective_ram_offset(&self, address: u16) -> usize {
        let base_offset = (address - 0xA000) as usize;
        (base_offset + self.effective_ram_bank() as usize * 0x2000) % self.ram.len().max(1)
    }

    fn mask_mcu_nibbles(&mut self) {
        for value in &mut self.mcu_ram {
            *value &= 0x0F;
        }
    }

    fn write_mcu_nibble(&mut self, address: u8, value: u8) {
        self.mcu_ram[address as usize] = value & 0x0F;
        match address {
            0x10..=0x15 => self.sync_current_time_from_mcu_window(),
            0x58..=0x5D => self.sync_event_time_from_mcu_window(),
            _ => {}
        }
    }

    pub(in crate::cartridge) fn read_triplet_nybbles(&self, start: usize) -> u16 {
        (self.mcu_ram[start] as u16)
            | ((self.mcu_ram[start + 1] as u16) << 4)
            | ((self.mcu_ram[start + 2] as u16) << 8)
    }

    pub(in crate::cartridge) fn write_triplet_nybbles(&mut self, start: usize, value: u16) {
        self.mcu_ram[start] = (value & 0x0F) as u8;
        self.mcu_ram[start + 1] = ((value >> 4) & 0x0F) as u8;
        self.mcu_ram[start + 2] = ((value >> 8) & 0x0F) as u8;
    }

    fn sync_current_time_from_mcu_window(&mut self) {
        self.rtc.current_minutes_of_day = self.read_triplet_nybbles(0x10) % HUC3_MINUTES_PER_DAY;
        self.rtc.current_days = self.read_triplet_nybbles(0x13) % HUC3_DAY_COUNTER_MODULUS;
        self.rtc.current_subminute_seconds = 0;
    }

    fn sync_event_time_from_mcu_window(&mut self) {
        self.rtc.event_minutes_of_day = self.read_triplet_nybbles(0x58) % HUC3_MINUTES_PER_DAY;
        self.rtc.event_days = self.read_triplet_nybbles(0x5B) % HUC3_DAY_COUNTER_MODULUS;
    }

    pub(in crate::cartridge) fn sync_tracked_rtc_locations(&mut self) {
        self.write_triplet_nybbles(0x10, self.rtc.current_minutes_of_day);
        self.write_triplet_nybbles(0x13, self.rtc.current_days);
        self.write_triplet_nybbles(0x58, self.rtc.event_minutes_of_day);
        self.write_triplet_nybbles(0x5B, self.rtc.event_days);
    }

    fn current_total_minutes(&self) -> u32 {
        self.rtc.current_days as u32 * HUC3_MINUTES_PER_DAY as u32
            + self.rtc.current_minutes_of_day as u32
    }

    fn event_total_minutes(&self) -> u32 {
        self.rtc.event_days as u32 * HUC3_MINUTES_PER_DAY as u32
            + self.rtc.event_minutes_of_day as u32
    }

    fn remaining_event_delta_minutes(&self) -> u32 {
        let cycle = Self::rtc_cycle_minutes();
        let current = self.current_total_minutes();
        let event = self.event_total_minutes();
        (event + cycle - current) % cycle
    }

    fn set_event_total_minutes(&mut self, total_minutes: u32) {
        let cycle = Self::rtc_cycle_minutes();
        let wrapped = total_minutes % cycle;
        self.rtc.event_days =
            ((wrapped / HUC3_MINUTES_PER_DAY as u32) as u16) % HUC3_DAY_COUNTER_MODULUS;
        self.rtc.event_minutes_of_day = (wrapped % HUC3_MINUTES_PER_DAY as u32) as u16;
    }

    fn rtc_cycle_minutes() -> u32 {
        HUC3_DAY_COUNTER_MODULUS as u32 * HUC3_MINUTES_PER_DAY as u32
    }
}
