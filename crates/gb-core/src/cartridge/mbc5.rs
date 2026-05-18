use super::*;

impl Mbc5Variant {
    pub(in crate::cartridge) fn has_ram(self) -> bool {
        matches!(
            self,
            Self::Ram | Self::RamBattery | Self::RumbleRam | Self::RumbleRamBattery
        )
    }

    pub(in crate::cartridge) fn has_battery(self) -> bool {
        matches!(self, Self::RamBattery | Self::RumbleRamBattery)
    }

    pub(in crate::cartridge) fn has_rumble(self) -> bool {
        matches!(
            self,
            Self::Rumble | Self::RumbleRam | Self::RumbleRamBattery
        )
    }
}

impl Mbc5Cartridge {
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

    pub(in crate::cartridge) fn read_rom(&self, address: u16) -> u8 {
        let address = address as usize;
        let bank_count = self.rom_layout.effective_bank_count;

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

    pub(in crate::cartridge) fn write_rom(&mut self, address: u16, value: u8) {
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

    pub(in crate::cartridge) fn read_ram(&self, address: u16) -> u8 {
        if !self.ram_enabled {
            return RAM_ABSENT_READ_VALUE;
        }

        self.ram.as_ref().map_or(RAM_ABSENT_READ_VALUE, |ram| {
            let offset = self.effective_ram_offset(address);
            ram.get(offset).copied().unwrap_or(RAM_ABSENT_READ_VALUE)
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

    pub(in crate::cartridge) fn effective_rom_bank(&self, bank_count: usize) -> usize {
        if bank_count == 0 {
            return 0;
        }

        let raw_bank = ((self.rom_bank_high1 as usize) << 8) | self.rom_bank_low8 as usize;
        raw_bank % bank_count
    }

    pub(in crate::cartridge) fn effective_ram_offset(&self, address: u16) -> usize {
        let base_offset = (address - 0xA000) as usize;
        let bank = self.effective_ram_bank() as usize;
        bank * 0x2000 + base_offset
    }

    pub(in crate::cartridge) fn effective_ram_bank(&self) -> u8 {
        let bank_count = self.header.ram_size.bank_count.unwrap_or(0).max(1);
        (self.ram_bank_raw as usize % bank_count) as u8
    }

    pub(in crate::cartridge) fn rumble_on(&self) -> bool {
        self.has_rumble && self.rumble_on
    }

    pub(in crate::cartridge) fn has_rumble(&self) -> bool {
        self.has_rumble
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
                .map(|ram| PersistentCartState::Mbc5Ram { ram: ram.clone() })
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
