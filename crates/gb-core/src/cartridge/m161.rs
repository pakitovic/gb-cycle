use super::*;

impl M161Cartridge {
    pub(in crate::cartridge) fn trace_summary(&self) -> String {
        format!(
            " selected_bank={:#04X} bank_switch_locked={} last_bank_write={:?}",
            self.selected_bank, self.bank_switch_locked, self.last_bank_write
        )
    }

    pub(in crate::cartridge) fn describe_external_access(
        &self,
        address: u16,
    ) -> CartridgeExternalAccessInfo {
        CartridgeExternalAccessInfo::new(
            address,
            CartridgeExternalTarget::LinearRam,
            CartridgeExternalAvailability::Absent,
            CartridgeExternalReadBehavior::FallbackValue(RAM_ABSENT_READ_VALUE),
            CartridgeExternalWriteBehavior::Ignored,
        )
    }

    pub(in crate::cartridge) fn read_rom(&self, address: u16) -> u8 {
        let bank_base = self.selected_bank as usize * M161_BANK_BYTES;
        self.rom
            .get(bank_base + address as usize)
            .copied()
            .unwrap_or(RAM_ABSENT_READ_VALUE)
    }

    pub(in crate::cartridge) fn write_rom(&mut self, _address: u16, value: u8) {
        if self.bank_switch_locked {
            return;
        }

        self.selected_bank = value & 0x07;
        self.bank_switch_locked = true;
        self.last_bank_write = Some(self.selected_bank);
    }

    pub(in crate::cartridge) fn read_ram(&self, _address: u16) -> u8 {
        RAM_ABSENT_READ_VALUE
    }

    pub(in crate::cartridge) fn write_ram(&mut self, _address: u16, _value: u8) {}

    pub(in crate::cartridge) fn persistence_metadata(&self) -> CartridgePersistenceMetadata {
        CartridgePersistenceMetadata {
            has_battery: false,
            has_rtc: false,
            profile: CartridgePersistenceProfile::None,
        }
    }

    pub(in crate::cartridge) fn persistent_state(&self) -> PersistentCartState {
        PersistentCartState::None
    }

    pub(in crate::cartridge) fn restore_persistent_state(
        &mut self,
        state: &PersistentCartState,
    ) -> Result<(), CartridgePersistentStateError> {
        if matches!(state, PersistentCartState::None) {
            Ok(())
        } else {
            Err(CartridgePersistentStateError::KindMismatch {
                expected: "None",
                actual: state.kind_name(),
            })
        }
    }
}
