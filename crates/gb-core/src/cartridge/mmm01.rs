use super::*;

impl Mmm01Cartridge {
    pub(in crate::cartridge) fn describe_external_access(
        &self,
        address: u16,
    ) -> CartridgeExternalAccessInfo {
        let has_ram = self.ram.is_some();
        let available = self.mapped && self.ram_enabled && has_ram;

        CartridgeExternalAccessInfo::new(
            address,
            CartridgeExternalTarget::BankedRam {
                bank: self.effective_ram_bank(),
            },
            if available {
                CartridgeExternalAvailability::Accessible
            } else if self.mapped && self.ram_enabled {
                CartridgeExternalAvailability::Absent
            } else {
                CartridgeExternalAvailability::Disabled
            },
            if available {
                CartridgeExternalReadBehavior::Storage
            } else {
                CartridgeExternalReadBehavior::FallbackValue(RAM_ABSENT_READ_VALUE)
            },
            if available {
                CartridgeExternalWriteBehavior::Storage
            } else {
                CartridgeExternalWriteBehavior::Ignored
            },
        )
    }

    pub(in crate::cartridge) fn read_rom(&self, address: u16) -> u8 {
        let bank_count = self.rom_bank_count();
        let bank = if !self.mapped {
            self.unmapped_rom_bank(address)
        } else if address < 0x4000 {
            self.effective_low_rom_bank()
        } else {
            self.effective_high_rom_bank()
        };

        let rom_index = if address < 0x4000 {
            bank * 0x4000 + address as usize
        } else {
            bank * 0x4000 + (address as usize - 0x4000)
        };

        self.rom
            .get(if bank_count == 0 {
                0
            } else {
                rom_index % self.rom.len()
            })
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

        let bank = if !self.mapped {
            self.unmapped_rom_bank(address)
        } else if address < 0x4000 {
            self.effective_low_rom_bank()
        } else {
            self.effective_high_rom_bank()
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
                self.ram_enabled = value & 0x0F == 0x0A;
                if !self.mapped {
                    self.ram_bank_mask = (value >> 4) & 0x03;
                    if value & 0x40 != 0 {
                        self.mapped = true;
                    }
                }
            }
            0x2000..=0x3FFF => {
                self.write_rom_bank_low(value & 0x1F);
                if !self.mapped {
                    self.rom_bank_mid = (value >> 5) & 0x03;
                }
            }
            0x4000..=0x5FFF => {
                self.write_ram_bank_low(value & 0x03);
                if !self.mapped {
                    self.ram_bank_high = (value >> 2) & 0x03;
                    self.rom_bank_high = (value >> 4) & 0x03;
                    self.mode_write_disable = value & 0x40 != 0;
                }
            }
            0x6000..=0x7FFF => {
                if !self.mode_write_disable {
                    self.banking_mode = value & 0x01;
                }

                if !self.mapped {
                    self.rom_bank_mask = ((value >> 1) & 0x1F) & 0x1E;
                    self.multiplex_enabled = value & 0x40 != 0;
                }
            }
            _ => {}
        }
    }

    pub(in crate::cartridge) fn read_ram(&self, address: u16) -> u8 {
        if !self.mapped || !self.ram_enabled {
            return RAM_ABSENT_READ_VALUE;
        }

        self.ram.as_ref().map_or(RAM_ABSENT_READ_VALUE, |ram| {
            ram.get(self.effective_ram_offset(address))
                .copied()
                .unwrap_or(RAM_ABSENT_READ_VALUE)
        })
    }

    pub(in crate::cartridge) fn write_ram(&mut self, address: u16, value: u8) {
        if !self.mapped || !self.ram_enabled {
            return;
        }

        let offset = self.effective_ram_offset(address);
        if let Some(ram) = &mut self.ram
            && let Some(byte) = ram.get_mut(offset)
        {
            *byte = value;
        }
    }

    pub(in crate::cartridge) fn persistence_metadata(&self) -> CartridgePersistenceMetadata {
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

    pub(in crate::cartridge) fn persistent_state(&self) -> PersistentCartState {
        if self.has_battery {
            self.ram
                .as_ref()
                .map(|ram| PersistentCartState::Mmm01Ram { ram: ram.clone() })
                .unwrap_or(PersistentCartState::None)
        } else {
            PersistentCartState::None
        }
    }

    pub(in crate::cartridge) fn restore_persistent_state(
        &mut self,
        state: &PersistentCartState,
    ) -> Result<(), CartridgePersistentStateError> {
        match (self.has_battery, self.ram.as_mut(), state) {
            (false, _, PersistentCartState::None) | (true, None, PersistentCartState::None) => {
                Ok(())
            }
            (true, Some(ram), PersistentCartState::Mmm01Ram { ram: persisted_ram }) => {
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
                expected: "Mmm01Ram",
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

    fn rom_bank_count(&self) -> usize {
        self.header
            .rom_size
            .bank_count
            .unwrap_or(self.rom.len() / 0x4000)
            .max(1)
    }

    fn unmapped_rom_bank(&self, address: u16) -> usize {
        let bank_count = self.rom_bank_count();
        if bank_count <= 1 {
            return 0;
        }

        if address < 0x4000 {
            bank_count - 2
        } else {
            bank_count - 1
        }
    }

    fn effective_low_rom_bank(&self) -> usize {
        let bank_count = self.rom_bank_count();
        if bank_count == 0 {
            return 0;
        }

        let raw_bank = if self.multiplex_enabled {
            let mid_bits = if self.banking_mode == 0 {
                self.ram_bank_low & self.ram_bank_mask
            } else {
                self.ram_bank_low & 0x03
            };
            ((self.rom_bank_high as usize) << 7)
                | ((mid_bits as usize) << 5)
                | (self.masked_rom_bank_low() as usize)
        } else {
            ((self.rom_bank_high as usize) << 7)
                | ((self.rom_bank_mid as usize) << 5)
                | (self.masked_rom_bank_low() as usize)
        };

        raw_bank % bank_count
    }

    fn effective_high_rom_bank(&self) -> usize {
        let bank_count = self.rom_bank_count();
        if bank_count == 0 {
            return 0;
        }

        let mid_bits = if self.multiplex_enabled {
            self.ram_bank_low & 0x03
        } else {
            self.rom_bank_mid & 0x03
        };
        let raw_bank = ((self.rom_bank_high as usize) << 7)
            | ((mid_bits as usize) << 5)
            | (self.translated_rom_bank_low() as usize);

        raw_bank % bank_count
    }

    fn effective_ram_bank_count(&self) -> usize {
        self.ram
            .as_ref()
            .map(|ram| (ram.len() / 0x2000).max(1))
            .unwrap_or(1)
    }

    fn effective_ram_bank(&self) -> u8 {
        if self.multiplex_enabled {
            ((self.ram_bank_high & 0x03) << 2) | (self.rom_bank_mid & 0x03)
        } else {
            let low_bits = if self.banking_mode == 0 {
                self.ram_bank_low & self.ram_bank_mask
            } else {
                self.ram_bank_low & 0x03
            };

            ((self.ram_bank_high & 0x03) << 2) | low_bits
        }
    }

    fn effective_ram_offset(&self, address: u16) -> usize {
        let base_offset = (address - 0xA000) as usize;
        let bank = (self.effective_ram_bank() as usize) % self.effective_ram_bank_count();
        bank * 0x2000 + base_offset
    }

    fn write_rom_bank_low(&mut self, value: u8) {
        let lock_mask = self.rom_bank_mask & 0x1E;
        self.rom_bank_low = (self.rom_bank_low & lock_mask) | (value & !lock_mask);
    }

    fn write_ram_bank_low(&mut self, value: u8) {
        let lock_mask = self.ram_bank_mask & 0x03;
        self.ram_bank_low = (self.ram_bank_low & lock_mask) | (value & !lock_mask);
    }

    fn masked_rom_bank_low(&self) -> u8 {
        self.rom_bank_low & self.rom_bank_mask
    }

    fn translated_rom_bank_low(&self) -> u8 {
        let unmasked = self.rom_bank_low & !self.rom_bank_mask;
        if unmasked == 0 {
            self.rom_bank_low | 1
        } else {
            self.rom_bank_low
        }
    }
}
