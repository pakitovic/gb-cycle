use super::*;

const MBC6_FLASH_MANUFACTURER_ID: u8 = 0xC2;
const MBC6_FLASH_DEVICE_ID: u8 = 0x81;
const MBC6_FLASH_STATUS_DONE: u8 = 0x80;
const MBC6_FLASH_STATUS_SECTOR0_PROTECTED: u8 = 0x02;
const MBC6_FLASH_UNLOCK_AA_OFFSET: usize = 0x5555;
const MBC6_FLASH_UNLOCK_55_OFFSET: usize = 0x2AAA;
const MBC6_FLASH_BANK_COUNT: usize = MBC6_FLASH_BYTES / MBC6_ROM_FLASH_BANK_BYTES;
const MBC6_RAM_BANK_COUNT: usize = MBC6_SUPPORTED_RAM_BYTES / MBC6_RAM_BANK_BYTES;

impl Mbc6ProgramState {
    fn new(target: Mbc6ProgramTarget) -> Self {
        Self {
            target,
            block_base: None,
            buffer: vec![0xFF; MBC6_FLASH_PROGRAM_BLOCK_BYTES],
            written: vec![false; MBC6_FLASH_PROGRAM_BLOCK_BYTES],
            final_byte_seen: false,
        }
    }
}

impl Mbc6Cartridge {
    pub(in crate::cartridge) fn describe_external_access(
        &self,
        address: u16,
    ) -> CartridgeExternalAccessInfo {
        let (window, bank) = if address < 0xB000 {
            (Mbc6Window::A, self.effective_ram_bank(Mbc6Window::A))
        } else {
            (Mbc6Window::B, self.effective_ram_bank(Mbc6Window::B))
        };

        CartridgeExternalAccessInfo::new(
            address,
            CartridgeExternalTarget::Mbc6Ram { window, bank },
            if self.ram_enabled {
                CartridgeExternalAvailability::Accessible
            } else {
                CartridgeExternalAvailability::Disabled
            },
            if self.ram_enabled {
                CartridgeExternalReadBehavior::Storage
            } else {
                CartridgeExternalReadBehavior::FallbackValue(RAM_ABSENT_READ_VALUE)
            },
            if self.ram_enabled {
                CartridgeExternalWriteBehavior::Storage
            } else {
                CartridgeExternalWriteBehavior::Ignored
            },
        )
    }

    pub(in crate::cartridge) fn read_rom(&self, address: u16) -> u8 {
        let address = address as usize;
        match address {
            0x0000..=0x3FFF => self
                .rom
                .get(address)
                .copied()
                .unwrap_or(RAM_ABSENT_READ_VALUE),
            0x4000..=0x5FFF => self.read_high_window(Mbc6Window::A, address - 0x4000),
            0x6000..=0x7FFF => self.read_high_window(Mbc6Window::B, address - 0x6000),
            _ => RAM_ABSENT_READ_VALUE,
        }
    }

    pub(in crate::cartridge) fn write_rom(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x03FF => self.ram_enabled = value & 0x0F == 0x0A,
            0x0400..=0x07FF => self.ram_bank_a = value & 0x07,
            0x0800..=0x0BFF => self.ram_bank_b = value & 0x07,
            0x0C00..=0x0FFF => {
                self.flash_enabled = value & 0x01 != 0;
                if !self.flash_enabled {
                    self.flash_mode = Mbc6FlashMode::ReadArray;
                }
            }
            0x1000 => self.flash_write_enabled = value & 0x01 != 0,
            0x2000..=0x27FF => self.rom_flash_bank_a = value & 0x7F,
            0x2800..=0x2FFF => {
                self.window_select_a = if value & 0x08 != 0 {
                    Mbc6WindowSelect::Flash
                } else {
                    Mbc6WindowSelect::Rom
                };
            }
            0x3000..=0x37FF => self.rom_flash_bank_b = value & 0x7F,
            0x3800..=0x3FFF => {
                self.window_select_b = if value & 0x08 != 0 {
                    Mbc6WindowSelect::Flash
                } else {
                    Mbc6WindowSelect::Rom
                };
            }
            0x4000..=0x5FFF => self.write_flash_window(Mbc6Window::A, address, value),
            0x6000..=0x7FFF => self.write_flash_window(Mbc6Window::B, address, value),
            _ => {}
        }
    }

    pub(in crate::cartridge) fn read_ram(&self, address: u16) -> u8 {
        if !self.ram_enabled {
            return RAM_ABSENT_READ_VALUE;
        }

        let offset = self.effective_ram_offset(address);
        self.ram
            .get(offset)
            .copied()
            .unwrap_or(RAM_ABSENT_READ_VALUE)
    }

    pub(in crate::cartridge) fn write_ram(&mut self, address: u16, value: u8) {
        if !self.ram_enabled {
            return;
        }

        let offset = self.effective_ram_offset(address);
        if let Some(byte) = self.ram.get_mut(offset) {
            *byte = value;
        }
    }

    pub(in crate::cartridge) fn persistence_metadata(&self) -> CartridgePersistenceMetadata {
        CartridgePersistenceMetadata {
            has_battery: self.has_battery,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentRamAndFlash {
                ram: CartridgeRamPayloadKind::Linear {
                    byte_len: self.ram.len(),
                },
                flash_byte_len: self.flash.len(),
                hidden_byte_len: self.hidden_region.len(),
            },
        }
    }

    pub(in crate::cartridge) fn persistent_state(&self) -> PersistentCartState {
        if self.has_battery {
            PersistentCartState::Mbc6 {
                ram: self.ram.clone(),
                flash: self.flash.clone(),
                hidden_region: self.hidden_region.clone(),
                sector0_protected: self.sector0_protected,
            }
        } else {
            PersistentCartState::None
        }
    }

    pub(in crate::cartridge) fn restore_persistent_state(
        &mut self,
        state: &PersistentCartState,
    ) -> Result<(), CartridgePersistentStateError> {
        match (self.has_battery, state) {
            (
                true,
                PersistentCartState::Mbc6 {
                    ram,
                    flash,
                    hidden_region,
                    sector0_protected,
                },
            ) => {
                if self.ram.len() != ram.len() {
                    return Err(CartridgePersistentStateError::RamLengthMismatch {
                        expected: self.ram.len(),
                        actual: ram.len(),
                    });
                }
                if self.flash.len() != flash.len() {
                    return Err(CartridgePersistentStateError::RamLengthMismatch {
                        expected: self.flash.len(),
                        actual: flash.len(),
                    });
                }
                if self.hidden_region.len() != hidden_region.len() {
                    return Err(CartridgePersistentStateError::RamLengthMismatch {
                        expected: self.hidden_region.len(),
                        actual: hidden_region.len(),
                    });
                }

                self.ram.copy_from_slice(ram);
                self.flash.copy_from_slice(flash);
                self.hidden_region.copy_from_slice(hidden_region);
                self.sector0_protected = *sector0_protected;
                Ok(())
            }
            (true, other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "Mbc6",
                actual: other.kind_name(),
            }),
            (false, PersistentCartState::None) => Ok(()),
            (false, other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "None",
                actual: other.kind_name(),
            }),
        }
    }

    fn read_high_window(&self, window: Mbc6Window, window_offset: usize) -> u8 {
        match self.window_select(window) {
            Mbc6WindowSelect::Rom => {
                let bank = self.effective_rom_flash_bank(window, self.rom_flash_bank_count());
                let index = bank * MBC6_ROM_FLASH_BANK_BYTES + window_offset;
                self.rom
                    .get(index)
                    .copied()
                    .unwrap_or(RAM_ABSENT_READ_VALUE)
            }
            Mbc6WindowSelect::Flash if self.flash_enabled => {
                let offset = self.flash_offset(window, window_offset);
                self.read_flash(offset)
            }
            Mbc6WindowSelect::Flash => RAM_ABSENT_READ_VALUE,
        }
    }

    fn write_flash_window(&mut self, window: Mbc6Window, address: u16, value: u8) {
        if !self.flash_enabled || self.window_select(window) != Mbc6WindowSelect::Flash {
            return;
        }

        let window_offset = match window {
            Mbc6Window::A => (address - 0x4000) as usize,
            Mbc6Window::B => (address - 0x6000) as usize,
        };
        let offset = self.flash_offset(window, window_offset);
        self.write_flash(offset, value);
    }

    fn read_flash(&self, offset: usize) -> u8 {
        match &self.flash_mode {
            Mbc6FlashMode::IdMode => match offset & 0x000F {
                0 => MBC6_FLASH_MANUFACTURER_ID,
                1 => MBC6_FLASH_DEVICE_ID,
                _ => RAM_ABSENT_READ_VALUE,
            },
            Mbc6FlashMode::HiddenReadMode => self.hidden_region[offset & (MBC6_HIDDEN_BYTES - 1)],
            Mbc6FlashMode::Status { status, .. } => *status,
            _ => self.flash[offset % self.flash.len()],
        }
    }

    fn write_flash(&mut self, offset: usize, value: u8) {
        if value == 0xF0 {
            self.flash_mode = Mbc6FlashMode::ReadArray;
            return;
        }

        let mode = std::mem::replace(&mut self.flash_mode, Mbc6FlashMode::ReadArray);
        self.flash_mode = match mode {
            Mbc6FlashMode::ReadArray
            | Mbc6FlashMode::IdMode
            | Mbc6FlashMode::HiddenReadMode
            | Mbc6FlashMode::Status { .. } => self.restart_or_idle(offset, value),
            Mbc6FlashMode::AwaitUnlock2 => {
                if self.is_unlock_55(offset, value) {
                    Mbc6FlashMode::AwaitCommand
                } else {
                    self.restart_or_idle(offset, value)
                }
            }
            Mbc6FlashMode::AwaitCommand => self.execute_primary_command(offset, value),
            Mbc6FlashMode::EraseAwaitUnlock1 => {
                if self.is_unlock_aa(offset, value) {
                    Mbc6FlashMode::EraseAwaitUnlock2
                } else {
                    self.restart_or_idle(offset, value)
                }
            }
            Mbc6FlashMode::EraseAwaitUnlock2 => {
                if self.is_unlock_55(offset, value) {
                    Mbc6FlashMode::EraseAwaitCommand
                } else {
                    self.restart_or_idle(offset, value)
                }
            }
            Mbc6FlashMode::EraseAwaitCommand => self.execute_erase_command(offset, value),
            Mbc6FlashMode::ExtendedAwaitUnlock1 => {
                if self.is_unlock_aa(offset, value) {
                    Mbc6FlashMode::ExtendedAwaitUnlock2
                } else {
                    self.restart_or_idle(offset, value)
                }
            }
            Mbc6FlashMode::ExtendedAwaitUnlock2 => {
                if self.is_unlock_55(offset, value) {
                    Mbc6FlashMode::ExtendedAwaitCommand
                } else {
                    self.restart_or_idle(offset, value)
                }
            }
            Mbc6FlashMode::ExtendedAwaitCommand => self.execute_extended_command(value),
            Mbc6FlashMode::HiddenReadAwaitUnlock1 => {
                if self.is_unlock_aa(offset, value) {
                    Mbc6FlashMode::HiddenReadAwaitUnlock2
                } else {
                    self.restart_or_idle(offset, value)
                }
            }
            Mbc6FlashMode::HiddenReadAwaitUnlock2 => {
                if self.is_unlock_55(offset, value) {
                    Mbc6FlashMode::HiddenReadAwaitCommand
                } else {
                    self.restart_or_idle(offset, value)
                }
            }
            Mbc6FlashMode::HiddenReadAwaitCommand => {
                if value == 0x77 {
                    Mbc6FlashMode::HiddenReadMode
                } else {
                    self.restart_or_idle(offset, value)
                }
            }
            Mbc6FlashMode::Program(mut state) => {
                if self.program_write(&mut state, offset, value) {
                    self.status_mode(Mbc6FlashStatusSource::Program)
                } else {
                    Mbc6FlashMode::Program(state)
                }
            }
            Mbc6FlashMode::HiddenProgram(mut state) => {
                if self.program_write(&mut state, offset, value) {
                    self.status_mode(Mbc6FlashStatusSource::Program)
                } else {
                    Mbc6FlashMode::HiddenProgram(state)
                }
            }
        };
    }

    fn restart_or_idle(&self, offset: usize, value: u8) -> Mbc6FlashMode {
        if self.is_unlock_aa(offset, value) {
            Mbc6FlashMode::AwaitUnlock2
        } else {
            Mbc6FlashMode::ReadArray
        }
    }

    fn execute_primary_command(&self, offset: usize, value: u8) -> Mbc6FlashMode {
        match value {
            0x80 => Mbc6FlashMode::EraseAwaitUnlock1,
            0x90 => Mbc6FlashMode::IdMode,
            0xA0 => Mbc6FlashMode::Program(Mbc6ProgramState::new(Mbc6ProgramTarget::MainFlash)),
            0x60 => Mbc6FlashMode::ExtendedAwaitUnlock1,
            0x77 => Mbc6FlashMode::HiddenReadAwaitUnlock1,
            _ => self.restart_or_idle(offset, value),
        }
    }

    fn execute_erase_command(&mut self, offset: usize, value: u8) -> Mbc6FlashMode {
        match value {
            0x10 if offset == MBC6_FLASH_UNLOCK_AA_OFFSET => {
                self.erase_chip();
                self.status_mode(Mbc6FlashStatusSource::Erase)
            }
            0x30 => {
                self.erase_sector(offset);
                self.status_mode(Mbc6FlashStatusSource::Erase)
            }
            _ => self.restart_or_idle(offset, value),
        }
    }

    fn execute_extended_command(&mut self, value: u8) -> Mbc6FlashMode {
        match value {
            0x04 => {
                if self.flash_write_enabled {
                    self.hidden_region.fill(0xFF);
                    self.status_mode(Mbc6FlashStatusSource::Erase)
                } else {
                    self.status_mode(Mbc6FlashStatusSource::IgnoredWriteProtected)
                }
            }
            0xE0 if self.flash_write_enabled => {
                Mbc6FlashMode::HiddenProgram(Mbc6ProgramState::new(Mbc6ProgramTarget::HiddenRegion))
            }
            0xE0 => self.status_mode(Mbc6FlashStatusSource::IgnoredWriteProtected),
            0x40 if self.flash_write_enabled => {
                self.sector0_protected = false;
                self.status_mode(Mbc6FlashStatusSource::Protect)
            }
            0x20 if self.flash_write_enabled => {
                self.sector0_protected = true;
                self.status_mode(Mbc6FlashStatusSource::Protect)
            }
            0x40 | 0x20 => self.status_mode(Mbc6FlashStatusSource::IgnoredWriteProtected),
            _ => Mbc6FlashMode::ReadArray,
        }
    }

    fn erase_chip(&mut self) {
        for sector in 0..(MBC6_FLASH_BYTES / MBC6_FLASH_SECTOR_BYTES) {
            if sector == 0 && (!self.flash_write_enabled || self.sector0_protected) {
                continue;
            }
            self.erase_sector_index(sector);
        }
    }

    fn erase_sector(&mut self, offset: usize) {
        let sector =
            (offset / MBC6_FLASH_SECTOR_BYTES) % (MBC6_FLASH_BYTES / MBC6_FLASH_SECTOR_BYTES);
        if sector == 0 && (!self.flash_write_enabled || self.sector0_protected) {
            return;
        }
        self.erase_sector_index(sector);
    }

    fn erase_sector_index(&mut self, sector: usize) {
        let start = sector * MBC6_FLASH_SECTOR_BYTES;
        let end = start + MBC6_FLASH_SECTOR_BYTES;
        self.flash[start..end].fill(0xFF);
    }

    fn program_write(&mut self, state: &mut Mbc6ProgramState, offset: usize, value: u8) -> bool {
        let block_base = self.program_block_base(state.target, offset);
        if state.block_base != Some(block_base) {
            state.block_base = Some(block_base);
            state.final_byte_seen = false;
            state.written.fill(false);
            self.seed_program_buffer(state, block_base);
        }

        let byte_offset = self.program_byte_offset(state.target, offset);
        if let Some(byte) = state.buffer.get_mut(byte_offset) {
            *byte = value;
        }
        if let Some(written) = state.written.get_mut(byte_offset) {
            *written = true;
        }

        if byte_offset != MBC6_FLASH_PROGRAM_BLOCK_BYTES - 1 {
            return false;
        }

        if state.final_byte_seen {
            self.commit_program_buffer(state, block_base);
            true
        } else {
            state.final_byte_seen = true;
            false
        }
    }

    fn seed_program_buffer(&self, state: &mut Mbc6ProgramState, block_base: usize) {
        match state.target {
            Mbc6ProgramTarget::MainFlash => {
                let end = block_base + MBC6_FLASH_PROGRAM_BLOCK_BYTES;
                state.buffer.copy_from_slice(&self.flash[block_base..end]);
            }
            Mbc6ProgramTarget::HiddenRegion => {
                let end = block_base + MBC6_FLASH_PROGRAM_BLOCK_BYTES;
                state
                    .buffer
                    .copy_from_slice(&self.hidden_region[block_base..end]);
            }
        }
    }

    fn commit_program_buffer(&mut self, state: &Mbc6ProgramState, block_base: usize) {
        match state.target {
            Mbc6ProgramTarget::MainFlash => {
                if block_base < MBC6_FLASH_SECTOR_BYTES
                    && (!self.flash_write_enabled || self.sector0_protected)
                {
                    return;
                }
                for index in 0..MBC6_FLASH_PROGRAM_BLOCK_BYTES {
                    if state.written[index] {
                        self.flash[block_base + index] &= state.buffer[index];
                    }
                }
            }
            Mbc6ProgramTarget::HiddenRegion => {
                if !self.flash_write_enabled {
                    return;
                }
                for index in 0..MBC6_FLASH_PROGRAM_BLOCK_BYTES {
                    if state.written[index] {
                        self.hidden_region[block_base + index] &= state.buffer[index];
                    }
                }
            }
        }
    }

    fn status_mode(&self, source: Mbc6FlashStatusSource) -> Mbc6FlashMode {
        Mbc6FlashMode::Status {
            source,
            status: MBC6_FLASH_STATUS_DONE
                | if self.sector0_protected {
                    MBC6_FLASH_STATUS_SECTOR0_PROTECTED
                } else {
                    0
                },
        }
    }

    fn program_block_base(&self, target: Mbc6ProgramTarget, offset: usize) -> usize {
        match target {
            Mbc6ProgramTarget::MainFlash => {
                (offset % self.flash.len()) & !(MBC6_FLASH_PROGRAM_BLOCK_BYTES - 1)
            }
            Mbc6ProgramTarget::HiddenRegion => {
                (offset & (MBC6_HIDDEN_BYTES - 1)) & !(MBC6_FLASH_PROGRAM_BLOCK_BYTES - 1)
            }
        }
    }

    fn program_byte_offset(&self, target: Mbc6ProgramTarget, offset: usize) -> usize {
        match target {
            Mbc6ProgramTarget::MainFlash => offset & (MBC6_FLASH_PROGRAM_BLOCK_BYTES - 1),
            Mbc6ProgramTarget::HiddenRegion => offset & (MBC6_FLASH_PROGRAM_BLOCK_BYTES - 1),
        }
    }

    fn is_unlock_aa(&self, offset: usize, value: u8) -> bool {
        offset == MBC6_FLASH_UNLOCK_AA_OFFSET && value == 0xAA
    }

    fn is_unlock_55(&self, offset: usize, value: u8) -> bool {
        offset == MBC6_FLASH_UNLOCK_55_OFFSET && value == 0x55
    }

    fn effective_ram_offset(&self, address: u16) -> usize {
        let (window, window_offset) = if address < 0xB000 {
            (Mbc6Window::A, (address - 0xA000) as usize)
        } else {
            (Mbc6Window::B, (address - 0xB000) as usize)
        };
        self.effective_ram_bank(window) as usize * MBC6_RAM_BANK_BYTES + window_offset
    }

    fn effective_ram_bank(&self, window: Mbc6Window) -> u8 {
        let raw = match window {
            Mbc6Window::A => self.ram_bank_a,
            Mbc6Window::B => self.ram_bank_b,
        };
        raw % MBC6_RAM_BANK_COUNT as u8
    }

    fn flash_offset(&self, window: Mbc6Window, window_offset: usize) -> usize {
        let bank = self.effective_rom_flash_bank(window, MBC6_FLASH_BANK_COUNT);
        bank * MBC6_ROM_FLASH_BANK_BYTES + window_offset
    }

    fn effective_rom_flash_bank(&self, window: Mbc6Window, bank_count: usize) -> usize {
        if bank_count == 0 {
            return 0;
        }

        let raw = match window {
            Mbc6Window::A => self.rom_flash_bank_a,
            Mbc6Window::B => self.rom_flash_bank_b,
        };
        raw as usize % bank_count
    }

    fn rom_flash_bank_count(&self) -> usize {
        (self.rom.len() / MBC6_ROM_FLASH_BANK_BYTES).max(1)
    }

    fn window_select(&self, window: Mbc6Window) -> Mbc6WindowSelect {
        match window {
            Mbc6Window::A => self.window_select_a,
            Mbc6Window::B => self.window_select_b,
        }
    }
}
