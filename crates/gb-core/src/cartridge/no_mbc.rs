use super::*;

impl NoMbcCartridge {
    pub(in crate::cartridge) fn read_rom(&self, address: u16) -> u8 {
        self.rom
            .get(address as usize)
            .copied()
            .unwrap_or(RAM_ABSENT_READ_VALUE)
    }

    pub(in crate::cartridge) fn write_rom(&mut self, _address: u16, _value: u8) {}

    pub(in crate::cartridge) fn read_ram(&self, address: u16) -> u8 {
        self.ram.as_ref().map_or(RAM_ABSENT_READ_VALUE, |ram| {
            ram.get((address - 0xA000) as usize)
                .copied()
                .unwrap_or(RAM_ABSENT_READ_VALUE)
        })
    }

    pub(in crate::cartridge) fn write_ram(&mut self, address: u16, value: u8) {
        if let Some(ram) = &mut self.ram
            && let Some(byte) = ram.get_mut((address - 0xA000) as usize)
        {
            *byte = value;
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
                .map(|ram| PersistentCartState::NoMbcRam { ram: ram.clone() })
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
            (true, Some(ram), PersistentCartState::NoMbcRam { ram: persisted_ram }) => {
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
                expected: "NoMbcRam",
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
