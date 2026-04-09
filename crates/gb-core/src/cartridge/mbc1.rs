use super::*;

impl Mbc1Cartridge {
    pub(in crate::cartridge) fn describe_external_access(
        &self,
        address: u16,
    ) -> CartridgeExternalAccessInfo {
        let has_ram = self.ram.is_some();
        let available = self.ram_enabled && has_ram;

        CartridgeExternalAccessInfo::new(
            address,
            CartridgeExternalTarget::BankedRam {
                bank: self.effective_ram_bank(),
            },
            if available {
                CartridgeExternalAvailability::Accessible
            } else if self.ram_enabled {
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

    fn effective_ram_bank_count(&self) -> usize {
        self.ram
            .as_ref()
            .map(|ram| (ram.len() / 0x2000).max(1))
            .unwrap_or(1)
    }

    pub(in crate::cartridge) fn read_rom(&self, address: u16) -> u8 {
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

    pub(in crate::cartridge) fn write_rom(&mut self, address: u16, value: u8) {
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

    pub(in crate::cartridge) fn read_ram(&self, address: u16) -> u8 {
        if !self.ram_enabled {
            return RAM_ABSENT_READ_VALUE;
        }

        self.ram.as_ref().map_or(RAM_ABSENT_READ_VALUE, |ram| {
            ram.get(self.effective_ram_offset(address))
                .copied()
                .unwrap_or(RAM_ABSENT_READ_VALUE)
        })
    }

    pub(in crate::cartridge) fn write_ram(&mut self, address: u16, value: u8) {
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

    pub(in crate::cartridge) fn effective_high_rom_bank(&self, bank_count: usize) -> usize {
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

    pub(in crate::cartridge) fn effective_low_rom_bank(&self, bank_count: usize) -> usize {
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

    pub(in crate::cartridge) fn effective_ram_offset(&self, address: u16) -> usize {
        let base_offset = (address - 0xA000) as usize;
        let ram_bank_count = self.effective_ram_bank_count();

        if self.variant == Mbc1Variant::Mbc1M {
            return base_offset;
        }

        match self.wiring {
            Mbc1Wiring::Standard => {
                let bank = self.effective_ram_bank() as usize;
                (bank % ram_bank_count) * 0x2000 + base_offset
            }
            Mbc1Wiring::LargeRom => base_offset,
        }
    }

    pub(in crate::cartridge) fn effective_ram_bank(&self) -> u8 {
        if self.variant == Mbc1Variant::Mbc1M {
            return 0;
        }

        match self.wiring {
            Mbc1Wiring::Standard => {
                if self.banking_mode == 0 {
                    0
                } else {
                    self.secondary_bank & 0x03
                }
            }
            Mbc1Wiring::LargeRom => 0,
        }
    }

    #[allow(dead_code)]
    pub(in crate::cartridge) fn has_battery(&self) -> bool {
        self.has_battery
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
                .map(|ram| PersistentCartState::Mbc1Ram { ram: ram.clone() })
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
