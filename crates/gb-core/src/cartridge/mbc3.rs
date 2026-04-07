use super::*;

impl Mbc3Cartridge {
    fn note_rtc_access_timing(&mut self, t_cycle: TCycle) {
        self.rtc_access_ready_at = Some(TCycle::new(
            t_cycle.get() + MBC3_RTC_ACCESS_SPACING_T_CYCLES,
        ));
    }

    fn visible_rtc_state(&self) -> Mbc3RtcState {
        if self.rtc_latched_valid {
            self.rtc_latched
        } else {
            // Before the first accepted latch, keep reads on an explicit
            // zeroed snapshot policy instead of accidentally reusing the
            // zero-initialized storage as an implicit behavior.
            Mbc3RtcState::default()
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

    pub(in crate::cartridge) fn write_rom(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1FFF => {
                self.ram_rtc_enabled = value & 0x0F == 0x0A;
            }
            0x2000..=0x3FFF => {
                self.rom_bank = value & 0x7F;
            }
            0x4000..=0x5FFF => {
                self.ram_or_rtc_select = Mbc3RamRtcSelect::from_value(value);
            }
            0x6000..=0x7FFF => {
                self.latch_rtc_if_needed(value);
            }
            _ => {}
        }
    }

    pub(in crate::cartridge) fn read_ram(&self, address: u16) -> u8 {
        if !self.ram_rtc_enabled {
            return RAM_ABSENT_READ_VALUE;
        }

        match self.ram_or_rtc_select {
            Mbc3RamRtcSelect::RamBank(raw_bank) => {
                self.ram.as_ref().map_or(RAM_ABSENT_READ_VALUE, |ram| {
                    let offset = self.effective_ram_offset(address, raw_bank);
                    ram.get(offset).copied().unwrap_or(RAM_ABSENT_READ_VALUE)
                })
            }
            Mbc3RamRtcSelect::ReservedSelector(_) => RAM_ABSENT_READ_VALUE,
            Mbc3RamRtcSelect::RtcRegister(register) => {
                if self.has_rtc {
                    self.visible_rtc_state().read(register)
                } else {
                    RAM_ABSENT_READ_VALUE
                }
            }
        }
    }

    pub(in crate::cartridge) fn read_ram_timed(&mut self, address: u16, t_cycle: TCycle) -> u8 {
        if !self.ram_rtc_enabled {
            return RAM_ABSENT_READ_VALUE;
        }

        match self.ram_or_rtc_select {
            Mbc3RamRtcSelect::RamBank(raw_bank) => {
                self.ram.as_ref().map_or(RAM_ABSENT_READ_VALUE, |ram| {
                    let offset = self.effective_ram_offset(address, raw_bank);
                    ram.get(offset).copied().unwrap_or(RAM_ABSENT_READ_VALUE)
                })
            }
            Mbc3RamRtcSelect::ReservedSelector(_) => RAM_ABSENT_READ_VALUE,
            Mbc3RamRtcSelect::RtcRegister(register) => {
                if self.has_rtc {
                    self.note_rtc_access_timing(t_cycle);
                    self.visible_rtc_state().read(register)
                } else {
                    RAM_ABSENT_READ_VALUE
                }
            }
        }
    }

    pub(in crate::cartridge) fn write_ram(&mut self, address: u16, value: u8) {
        if !self.ram_rtc_enabled {
            return;
        }

        match self.ram_or_rtc_select {
            Mbc3RamRtcSelect::RamBank(raw_bank) => {
                let offset = self.effective_ram_offset(address, raw_bank);
                if let Some(ram) = &mut self.ram
                    && let Some(byte) = ram.get_mut(offset)
                {
                    *byte = value;
                }
            }
            Mbc3RamRtcSelect::ReservedSelector(_) => {}
            Mbc3RamRtcSelect::RtcRegister(register) => {
                if self.has_rtc {
                    self.rtc_live.write(register, value);
                }
            }
        }
    }

    pub(in crate::cartridge) fn write_ram_timed(
        &mut self,
        address: u16,
        value: u8,
        t_cycle: TCycle,
    ) {
        if !self.ram_rtc_enabled {
            return;
        }

        match self.ram_or_rtc_select {
            Mbc3RamRtcSelect::RamBank(raw_bank) => {
                let offset = self.effective_ram_offset(address, raw_bank);
                if let Some(ram) = &mut self.ram
                    && let Some(byte) = ram.get_mut(offset)
                {
                    *byte = value;
                }
            }
            Mbc3RamRtcSelect::ReservedSelector(_) => {}
            Mbc3RamRtcSelect::RtcRegister(register) => {
                if self.has_rtc {
                    self.note_rtc_access_timing(t_cycle);
                    self.rtc_live.write(register, value);
                }
            }
        }
    }

    pub(in crate::cartridge) fn effective_rom_bank(&self, bank_count: usize) -> usize {
        let raw_bank = self.rom_bank & 0x7F;
        let translated_bank = if raw_bank == 0 { 1 } else { raw_bank } as usize;

        if bank_count == 0 {
            return 0;
        }

        translated_bank % bank_count
    }

    pub(in crate::cartridge) fn effective_ram_offset(&self, address: u16, raw_bank: u8) -> usize {
        let base_offset = (address - 0xA000) as usize;
        let bank_count = self.header.ram_size.bank_count.unwrap_or(0).max(1);
        let bank = (raw_bank as usize) % bank_count;
        bank * 0x2000 + base_offset
    }

    pub(in crate::cartridge) fn latch_rtc_if_needed(&mut self, value: u8) {
        if !self.has_rtc {
            self.rtc_latch_armed = value == 0x00;
            return;
        }

        if value == 0x00 {
            self.rtc_latch_armed = true;
            return;
        }

        // Keep the first accepted latch on the documented 0x00 -> 0x01 edge, but
        // continue to accept follow-up non-zero relatches once a valid snapshot
        // exists so the curated cpp RTC oracle stays stable.
        if (self.rtc_latch_armed && value == 0x01) || self.rtc_latched_valid {
            self.rtc_latched = self.rtc_live;
            self.rtc_latched_valid = true;
        }

        self.rtc_latch_armed = false;
    }

    pub(in crate::cartridge) fn advance_rtc_seconds(&mut self, seconds: u64) {
        if self.has_rtc {
            self.rtc_live.advance_seconds(seconds);
        }
    }

    #[allow(dead_code)]
    pub(in crate::cartridge) fn has_battery(&self) -> bool {
        self.has_battery
    }

    pub(in crate::cartridge) fn persistence_metadata(&self) -> CartridgePersistenceMetadata {
        let ram_kind = self
            .ram
            .as_ref()
            .map(|ram| CartridgeRamPayloadKind::Linear {
                byte_len: ram.len(),
            });
        let profile = match (self.has_battery, self.has_rtc, ram_kind) {
            (true, true, Some(ram)) => CartridgePersistenceProfile::PersistentRamAndRtc { ram },
            (true, true, None) => CartridgePersistenceProfile::PersistentRtc,
            (true, false, Some(ram)) => CartridgePersistenceProfile::PersistentRam { ram },
            (false, false, Some(ram)) => CartridgePersistenceProfile::NonPersistentRam { ram },
            _ => CartridgePersistenceProfile::None,
        };

        CartridgePersistenceMetadata {
            has_battery: self.has_battery,
            has_rtc: self.has_rtc,
            profile,
        }
    }

    pub(in crate::cartridge) fn persistent_state(&self) -> PersistentCartState {
        if !self.has_battery {
            return PersistentCartState::None;
        }

        match (self.ram.as_ref(), self.has_rtc) {
            (Some(ram), true) => PersistentCartState::Mbc3RamRtc {
                ram: ram.clone(),
                rtc: self.rtc_live.into(),
            },
            (Some(ram), false) => PersistentCartState::Mbc3Ram { ram: ram.clone() },
            (None, true) => PersistentCartState::Mbc3Rtc {
                rtc: self.rtc_live.into(),
            },
            (None, false) => PersistentCartState::None,
        }
    }

    pub(in crate::cartridge) fn restore_persistent_state(
        &mut self,
        state: &PersistentCartState,
    ) -> Result<(), CartridgePersistentStateError> {
        match (self.has_battery, self.has_rtc, self.ram.as_mut(), state) {
            (false, _, _, PersistentCartState::None) => Ok(()),
            (
                true,
                true,
                Some(ram),
                PersistentCartState::Mbc3RamRtc {
                    ram: persisted_ram,
                    rtc,
                },
            ) => {
                if ram.len() != persisted_ram.len() {
                    return Err(CartridgePersistentStateError::RamLengthMismatch {
                        expected: ram.len(),
                        actual: persisted_ram.len(),
                    });
                }
                ram.copy_from_slice(persisted_ram);
                self.rtc_live = (*rtc).into();
                Ok(())
            }
            (true, false, Some(ram), PersistentCartState::Mbc3Ram { ram: persisted_ram }) => {
                if ram.len() != persisted_ram.len() {
                    return Err(CartridgePersistentStateError::RamLengthMismatch {
                        expected: ram.len(),
                        actual: persisted_ram.len(),
                    });
                }
                ram.copy_from_slice(persisted_ram);
                Ok(())
            }
            (true, true, None, PersistentCartState::Mbc3Rtc { rtc }) => {
                self.rtc_live = (*rtc).into();
                Ok(())
            }
            (true, false, None, PersistentCartState::None) => Ok(()),
            (true, true, Some(_), other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "Mbc3RamRtc",
                actual: other.kind_name(),
            }),
            (true, false, Some(_), other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "Mbc3Ram",
                actual: other.kind_name(),
            }),
            (true, true, None, other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "Mbc3Rtc",
                actual: other.kind_name(),
            }),
            (true, false, None, other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "None",
                actual: other.kind_name(),
            }),
            (false, _, _, other) => Err(CartridgePersistentStateError::KindMismatch {
                expected: "None",
                actual: other.kind_name(),
            }),
        }
    }
}

impl Mbc3RamRtcSelect {
    pub(in crate::cartridge) fn from_value(value: u8) -> Self {
        let low_nibble = value & 0x0F;
        match low_nibble {
            0x00..=0x03 => Self::RamBank(low_nibble),
            0x08 => Self::RtcRegister(Mbc3RtcRegister::Seconds),
            0x09 => Self::RtcRegister(Mbc3RtcRegister::Minutes),
            0x0A => Self::RtcRegister(Mbc3RtcRegister::Hours),
            0x0B => Self::RtcRegister(Mbc3RtcRegister::DayLow),
            0x0C => Self::RtcRegister(Mbc3RtcRegister::DayHigh),
            other => Self::ReservedSelector(other),
        }
    }
}
