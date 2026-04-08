use super::*;
use crate::scheduler::TCycle;

impl CartridgeDevice {
    pub(in crate::cartridge) fn describe_external_access(
        &self,
        address: u16,
    ) -> CartridgeExternalAccessInfo {
        match self {
            Self::NoMbc(cartridge) => cartridge.describe_external_access(address),
            Self::Mbc1(cartridge) => cartridge.describe_external_access(address),
            Self::Mbc2(cartridge) => cartridge.describe_external_access(address),
            Self::Mbc3(cartridge) => cartridge.describe_external_access(address),
            Self::Mbc5(cartridge) => cartridge.describe_external_access(address),
        }
    }

    pub(in crate::cartridge) fn header(&self) -> &CartridgeHeader {
        match self {
            Self::NoMbc(cartridge) => &cartridge.header,
            Self::Mbc1(cartridge) => &cartridge.header,
            Self::Mbc2(cartridge) => &cartridge.header,
            Self::Mbc3(cartridge) => &cartridge.header,
            Self::Mbc5(cartridge) => &cartridge.header,
        }
    }

    pub(in crate::cartridge) fn classification(&self) -> CartridgeClassification {
        match self {
            Self::NoMbc(cartridge) => cartridge.classification,
            Self::Mbc1(cartridge) => cartridge.classification,
            Self::Mbc2(cartridge) => cartridge.classification,
            Self::Mbc3(cartridge) => cartridge.classification,
            Self::Mbc5(cartridge) => cartridge.classification,
        }
    }

    pub(in crate::cartridge) fn read_rom(&self, address: u16) -> u8 {
        match self {
            Self::NoMbc(cartridge) => cartridge.read_rom(address),
            Self::Mbc1(cartridge) => cartridge.read_rom(address),
            Self::Mbc2(cartridge) => cartridge.read_rom(address),
            Self::Mbc3(cartridge) => cartridge.read_rom(address),
            Self::Mbc5(cartridge) => cartridge.read_rom(address),
        }
    }

    pub(in crate::cartridge) fn write_rom(&mut self, address: u16, value: u8) {
        match self {
            Self::NoMbc(cartridge) => cartridge.write_rom(address, value),
            Self::Mbc1(cartridge) => cartridge.write_rom(address, value),
            Self::Mbc2(cartridge) => cartridge.write_rom(address, value),
            Self::Mbc3(cartridge) => cartridge.write_rom(address, value),
            Self::Mbc5(cartridge) => cartridge.write_rom(address, value),
        }
    }

    pub(in crate::cartridge) fn read_ram(&self, address: u16) -> u8 {
        match self {
            Self::NoMbc(cartridge) => cartridge.read_ram(address),
            Self::Mbc1(cartridge) => cartridge.read_ram(address),
            Self::Mbc2(cartridge) => cartridge.read_ram(address),
            Self::Mbc3(cartridge) => cartridge.read_ram(address),
            Self::Mbc5(cartridge) => cartridge.read_ram(address),
        }
    }

    pub(in crate::cartridge) fn read_ram_timed(&mut self, address: u16, t_cycle: TCycle) -> u8 {
        match self {
            Self::NoMbc(cartridge) => cartridge.read_ram(address),
            Self::Mbc1(cartridge) => cartridge.read_ram(address),
            Self::Mbc2(cartridge) => cartridge.read_ram(address),
            Self::Mbc3(cartridge) => cartridge.read_ram_timed(address, t_cycle),
            Self::Mbc5(cartridge) => cartridge.read_ram(address),
        }
    }

    pub(in crate::cartridge) fn write_ram(&mut self, address: u16, value: u8) {
        match self {
            Self::NoMbc(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc1(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc2(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc3(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc5(cartridge) => cartridge.write_ram(address, value),
        }
    }

    pub(in crate::cartridge) fn write_ram_timed(
        &mut self,
        address: u16,
        value: u8,
        t_cycle: TCycle,
    ) {
        match self {
            Self::NoMbc(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc1(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc2(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc3(cartridge) => cartridge.write_ram_timed(address, value, t_cycle),
            Self::Mbc5(cartridge) => cartridge.write_ram(address, value),
        }
    }

    pub(in crate::cartridge) fn advance_rtc_seconds(&mut self, seconds: u64) {
        match self {
            Self::NoMbc(_) | Self::Mbc1(_) | Self::Mbc2(_) | Self::Mbc5(_) => {}
            Self::Mbc3(cartridge) => cartridge.advance_rtc_seconds(seconds),
        }
    }

    pub(in crate::cartridge) fn rumble_on(&self) -> bool {
        match self {
            Self::Mbc5(cartridge) => cartridge.rumble_on(),
            Self::NoMbc(_) | Self::Mbc1(_) | Self::Mbc2(_) | Self::Mbc3(_) => false,
        }
    }

    pub(in crate::cartridge) fn has_rumble(&self) -> bool {
        match self {
            Self::Mbc5(cartridge) => cartridge.has_rumble(),
            Self::NoMbc(_) | Self::Mbc1(_) | Self::Mbc2(_) | Self::Mbc3(_) => false,
        }
    }

    pub(in crate::cartridge) fn persistence_metadata(&self) -> CartridgePersistenceMetadata {
        match self {
            Self::NoMbc(cartridge) => cartridge.persistence_metadata(),
            Self::Mbc1(cartridge) => cartridge.persistence_metadata(),
            Self::Mbc2(cartridge) => cartridge.persistence_metadata(),
            Self::Mbc3(cartridge) => cartridge.persistence_metadata(),
            Self::Mbc5(cartridge) => cartridge.persistence_metadata(),
        }
    }

    pub(in crate::cartridge) fn persistent_state(&self) -> PersistentCartState {
        match self {
            Self::NoMbc(cartridge) => cartridge.persistent_state(),
            Self::Mbc1(cartridge) => cartridge.persistent_state(),
            Self::Mbc2(cartridge) => cartridge.persistent_state(),
            Self::Mbc3(cartridge) => cartridge.persistent_state(),
            Self::Mbc5(cartridge) => cartridge.persistent_state(),
        }
    }

    pub(in crate::cartridge) fn restore_persistent_state(
        &mut self,
        state: &PersistentCartState,
    ) -> Result<(), CartridgePersistentStateError> {
        match self {
            Self::NoMbc(cartridge) => cartridge.restore_persistent_state(state),
            Self::Mbc1(cartridge) => cartridge.restore_persistent_state(state),
            Self::Mbc2(cartridge) => cartridge.restore_persistent_state(state),
            Self::Mbc3(cartridge) => cartridge.restore_persistent_state(state),
            Self::Mbc5(cartridge) => cartridge.restore_persistent_state(state),
        }
    }
}
