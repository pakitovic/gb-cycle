use super::*;

impl Mbc2Cartridge {
    pub(in crate::cartridge) fn read_rom(&self, address: u16) -> u8 {
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

    pub(in crate::cartridge) fn write_rom(&mut self, address: u16, value: u8) {
        if address > 0x3FFF {
            return;
        }

        if address & 0x0100 == 0 {
            self.ram_enabled = value & 0x0F == 0x0A;
        } else {
            self.rom_bank_low4 = value & 0x0F;
        }
    }

    pub(in crate::cartridge) fn read_ram(&self, address: u16) -> u8 {
        if !self.ram_enabled {
            return RAM_ABSENT_READ_VALUE;
        }

        MBC2_RAM_READ_HIGH_NIBBLE | self.ram_nibbles[self.ram_index(address)]
    }

    pub(in crate::cartridge) fn write_ram(&mut self, address: u16, value: u8) {
        if !self.ram_enabled {
            return;
        }

        let index = self.ram_index(address);
        self.ram_nibbles[index] = value & 0x0F;
    }

    pub(in crate::cartridge) fn effective_high_rom_bank(&self, bank_count: usize) -> usize {
        let raw_low4 = self.rom_bank_low4 & 0x0F;
        let translated_low4 = if raw_low4 == 0 { 1 } else { raw_low4 } as usize;

        if bank_count == 0 {
            return 0;
        }

        translated_low4 % bank_count
    }

    pub(in crate::cartridge) fn ram_index(&self, address: u16) -> usize {
        (address as usize - 0xA000) & MBC2_RAM_ADDRESS_MASK
    }

    #[allow(dead_code)]
    pub(in crate::cartridge) fn has_battery(&self) -> bool {
        self.has_battery
    }

    pub(in crate::cartridge) fn persistence_metadata(&self) -> CartridgePersistenceMetadata {
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

    pub(in crate::cartridge) fn persistent_state(&self) -> PersistentCartState {
        if self.has_battery {
            PersistentCartState::Mbc2Ram {
                ram_nibbles: self.ram_nibbles,
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
