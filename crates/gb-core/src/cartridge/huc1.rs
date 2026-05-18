use super::*;

impl Huc1Cartridge {
    pub(in crate::cartridge) fn trace_summary(&self) -> String {
        format!(
            " io_mode={:?} rom_bank_raw={:#04X} effective_rom_bank={:#04X} ram_bank_raw={:#04X} effective_ram_bank={} ir_emitter_on={} ir_light_detected={}",
            self.io_mode,
            self.rom_bank,
            self.effective_rom_bank(self.header.rom_size.bank_count.unwrap_or(0)),
            self.ram_bank,
            self.effective_ram_bank(),
            self.ir_emitter_on,
            self.ir_light_detected,
        )
    }

    pub(in crate::cartridge) fn describe_external_access(
        &self,
        address: u16,
    ) -> CartridgeExternalAccessInfo {
        match self.io_mode {
            Huc1IoMode::Ram => {
                let has_ram = self.ram.is_some();
                CartridgeExternalAccessInfo::new(
                    address,
                    CartridgeExternalTarget::BankedRam {
                        bank: self.effective_ram_bank(),
                    },
                    if has_ram {
                        CartridgeExternalAvailability::Accessible
                    } else {
                        CartridgeExternalAvailability::Absent
                    },
                    if has_ram {
                        CartridgeExternalReadBehavior::Storage
                    } else {
                        CartridgeExternalReadBehavior::FallbackValue(RAM_ABSENT_READ_VALUE)
                    },
                    if has_ram {
                        CartridgeExternalWriteBehavior::Storage
                    } else {
                        CartridgeExternalWriteBehavior::Ignored
                    },
                )
            }
            Huc1IoMode::Ir => CartridgeExternalAccessInfo::new(
                address,
                CartridgeExternalTarget::IrRegister,
                CartridgeExternalAvailability::Accessible,
                CartridgeExternalReadBehavior::InfraredSensor,
                CartridgeExternalWriteBehavior::InfraredTransmitter,
            ),
        }
    }

    fn effective_ram_bank_count(&self) -> usize {
        self.ram
            .as_ref()
            .map(|ram| (ram.len() / 0x2000).max(1))
            .unwrap_or(1)
    }

    fn read_ir_register(&self) -> u8 {
        if self.ir_light_detected { 0xC1 } else { 0xC0 }
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
                self.io_mode = if value == 0x0E {
                    Huc1IoMode::Ir
                } else {
                    Huc1IoMode::Ram
                };
            }
            0x2000..=0x3FFF => {
                self.rom_bank = value & 0x3F;
            }
            0x4000..=0x5FFF => {
                self.ram_bank = value & 0x03;
            }
            0x6000..=0x7FFF => {}
            _ => {}
        }
    }

    pub(in crate::cartridge) fn read_ram(&self, address: u16) -> u8 {
        match self.io_mode {
            Huc1IoMode::Ram => self.ram.as_ref().map_or(RAM_ABSENT_READ_VALUE, |ram| {
                ram.get(self.effective_ram_offset(address))
                    .copied()
                    .unwrap_or(RAM_ABSENT_READ_VALUE)
            }),
            Huc1IoMode::Ir => self.read_ir_register(),
        }
    }

    pub(in crate::cartridge) fn write_ram(&mut self, address: u16, value: u8) {
        match self.io_mode {
            Huc1IoMode::Ram => {
                let offset = self.effective_ram_offset(address);
                if let Some(ram) = &mut self.ram
                    && let Some(byte) = ram.get_mut(offset)
                {
                    *byte = value;
                }
            }
            Huc1IoMode::Ir => {
                self.ir_emitter_on = value & 0x01 != 0;
            }
        }
    }

    pub(in crate::cartridge) fn effective_rom_bank(&self, bank_count: usize) -> usize {
        if bank_count == 0 {
            return 0;
        }

        self.rom_bank as usize % bank_count
    }

    pub(in crate::cartridge) fn effective_ram_bank(&self) -> u8 {
        let bank_count = self.effective_ram_bank_count();
        (self.ram_bank as usize % bank_count) as u8
    }

    pub(in crate::cartridge) fn effective_ram_offset(&self, address: u16) -> usize {
        let base_offset = (address - 0xA000) as usize;
        let bank = self.effective_ram_bank() as usize;
        let offset = bank * 0x2000 + base_offset;

        self.ram.as_ref().map_or(offset, |ram| {
            if ram.is_empty() {
                offset
            } else {
                offset % ram.len()
            }
        })
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
                .map(|ram| PersistentCartState::Huc1Ram { ram: ram.clone() })
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
            (true, Some(ram), PersistentCartState::Huc1Ram { ram: persisted_ram }) => {
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
                expected: "Huc1Ram",
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
