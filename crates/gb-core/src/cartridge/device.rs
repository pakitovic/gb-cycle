use super::*;
use crate::scheduler::TCycle;

impl CartridgeDevice {
    fn rom_bytes(&self) -> &[u8] {
        match self {
            Self::NoMbc(cartridge) => &cartridge.rom,
            Self::Mmm01(cartridge) => &cartridge.rom,
            Self::M161(cartridge) => &cartridge.rom,
            Self::Huc1(cartridge) => &cartridge.rom,
            Self::Huc3(cartridge) => &cartridge.rom,
            Self::Mbc1(cartridge) => &cartridge.rom,
            Self::Mbc2(cartridge) => &cartridge.rom,
            Self::Mbc3(cartridge) => &cartridge.rom,
            Self::Mbc5(cartridge) => &cartridge.rom,
            Self::Mbc6(cartridge) => &cartridge.rom,
            Self::Mbc7(cartridge) => &cartridge.rom,
            Self::PocketCamera(cartridge) => &cartridge.rom,
        }
    }

    pub(in crate::cartridge) fn compute_rom_fingerprint(&self) -> SaveStateByteFingerprint {
        SaveStateByteFingerprint::from_bytes(self.rom_bytes())
    }

    pub(in crate::cartridge) fn trace_summary(&self) -> String {
        match self {
            Self::M161(cartridge) => cartridge.trace_summary(),
            Self::Huc1(cartridge) => cartridge.trace_summary(),
            Self::Huc3(cartridge) => cartridge.trace_summary(),
            Self::NoMbc(_)
            | Self::Mmm01(_)
            | Self::Mbc1(_)
            | Self::Mbc2(_)
            | Self::Mbc3(_)
            | Self::Mbc5(_)
            | Self::Mbc6(_)
            | Self::Mbc7(_)
            | Self::PocketCamera(_) => String::new(),
        }
    }

    pub(in crate::cartridge) fn describe_external_access(
        &self,
        address: u16,
    ) -> CartridgeExternalAccessInfo {
        match self {
            Self::NoMbc(cartridge) => cartridge.describe_external_access(address),
            Self::Mmm01(cartridge) => cartridge.describe_external_access(address),
            Self::M161(cartridge) => cartridge.describe_external_access(address),
            Self::Huc1(cartridge) => cartridge.describe_external_access(address),
            Self::Huc3(cartridge) => cartridge.describe_external_access(address),
            Self::Mbc1(cartridge) => cartridge.describe_external_access(address),
            Self::Mbc2(cartridge) => cartridge.describe_external_access(address),
            Self::Mbc3(cartridge) => cartridge.describe_external_access(address),
            Self::Mbc5(cartridge) => cartridge.describe_external_access(address),
            Self::Mbc6(cartridge) => cartridge.describe_external_access(address),
            Self::Mbc7(cartridge) => cartridge.describe_external_access(address),
            Self::PocketCamera(cartridge) => cartridge.describe_external_access(address),
        }
    }

    pub(in crate::cartridge) fn header(&self) -> &CartridgeHeader {
        match self {
            Self::NoMbc(cartridge) => &cartridge.header,
            Self::Mmm01(cartridge) => &cartridge.header,
            Self::M161(cartridge) => &cartridge.header,
            Self::Huc1(cartridge) => &cartridge.header,
            Self::Huc3(cartridge) => &cartridge.header,
            Self::Mbc1(cartridge) => &cartridge.header,
            Self::Mbc2(cartridge) => &cartridge.header,
            Self::Mbc3(cartridge) => &cartridge.header,
            Self::Mbc5(cartridge) => &cartridge.header,
            Self::Mbc6(cartridge) => &cartridge.header,
            Self::Mbc7(cartridge) => &cartridge.header,
            Self::PocketCamera(cartridge) => &cartridge.header,
        }
    }

    pub(in crate::cartridge) fn classification(&self) -> CartridgeClassification {
        match self {
            Self::NoMbc(cartridge) => cartridge.classification,
            Self::Mmm01(cartridge) => cartridge.classification,
            Self::M161(cartridge) => cartridge.classification,
            Self::Huc1(cartridge) => cartridge.classification,
            Self::Huc3(cartridge) => cartridge.classification,
            Self::Mbc1(cartridge) => cartridge.classification,
            Self::Mbc2(cartridge) => cartridge.classification,
            Self::Mbc3(cartridge) => cartridge.classification,
            Self::Mbc5(cartridge) => cartridge.classification,
            Self::Mbc6(cartridge) => cartridge.classification,
            Self::Mbc7(cartridge) => cartridge.classification,
            Self::PocketCamera(cartridge) => cartridge.classification,
        }
    }

    pub(in crate::cartridge) fn read_rom(&self, address: u16) -> u8 {
        match self {
            Self::NoMbc(cartridge) => cartridge.read_rom(address),
            Self::Mmm01(cartridge) => cartridge.read_rom(address),
            Self::M161(cartridge) => cartridge.read_rom(address),
            Self::Huc1(cartridge) => cartridge.read_rom(address),
            Self::Huc3(cartridge) => cartridge.read_rom(address),
            Self::Mbc1(cartridge) => cartridge.read_rom(address),
            Self::Mbc2(cartridge) => cartridge.read_rom(address),
            Self::Mbc3(cartridge) => cartridge.read_rom(address),
            Self::Mbc5(cartridge) => cartridge.read_rom(address),
            Self::Mbc6(cartridge) => cartridge.read_rom(address),
            Self::Mbc7(cartridge) => cartridge.read_rom(address),
            Self::PocketCamera(cartridge) => cartridge.read_rom(address),
        }
    }

    pub(in crate::cartridge) fn write_rom(&mut self, address: u16, value: u8) {
        match self {
            Self::NoMbc(cartridge) => cartridge.write_rom(address, value),
            Self::Mmm01(cartridge) => cartridge.write_rom(address, value),
            Self::M161(cartridge) => cartridge.write_rom(address, value),
            Self::Huc1(cartridge) => cartridge.write_rom(address, value),
            Self::Huc3(cartridge) => cartridge.write_rom(address, value),
            Self::Mbc1(cartridge) => cartridge.write_rom(address, value),
            Self::Mbc2(cartridge) => cartridge.write_rom(address, value),
            Self::Mbc3(cartridge) => cartridge.write_rom(address, value),
            Self::Mbc5(cartridge) => cartridge.write_rom(address, value),
            Self::Mbc6(cartridge) => cartridge.write_rom(address, value),
            Self::Mbc7(cartridge) => cartridge.write_rom(address, value),
            Self::PocketCamera(cartridge) => cartridge.write_rom(address, value),
        }
    }

    pub(in crate::cartridge) fn read_ram(&self, address: u16) -> u8 {
        match self {
            Self::NoMbc(cartridge) => cartridge.read_ram(address),
            Self::Mmm01(cartridge) => cartridge.read_ram(address),
            Self::M161(cartridge) => cartridge.read_ram(address),
            Self::Huc1(cartridge) => cartridge.read_ram(address),
            Self::Huc3(cartridge) => cartridge.read_ram(address),
            Self::Mbc1(cartridge) => cartridge.read_ram(address),
            Self::Mbc2(cartridge) => cartridge.read_ram(address),
            Self::Mbc3(cartridge) => cartridge.read_ram(address),
            Self::Mbc5(cartridge) => cartridge.read_ram(address),
            Self::Mbc6(cartridge) => cartridge.read_ram(address),
            Self::Mbc7(cartridge) => cartridge.read_ram(address),
            Self::PocketCamera(cartridge) => cartridge.read_ram(address),
        }
    }

    pub(in crate::cartridge) fn read_ram_timed(&mut self, address: u16, t_cycle: TCycle) -> u8 {
        match self {
            Self::NoMbc(cartridge) => cartridge.read_ram(address),
            Self::Mmm01(cartridge) => cartridge.read_ram(address),
            Self::M161(cartridge) => cartridge.read_ram(address),
            Self::Huc1(cartridge) => cartridge.read_ram(address),
            Self::Huc3(cartridge) => cartridge.read_ram(address),
            Self::Mbc1(cartridge) => cartridge.read_ram(address),
            Self::Mbc2(cartridge) => cartridge.read_ram(address),
            Self::Mbc3(cartridge) => cartridge.read_ram_timed(address, t_cycle),
            Self::Mbc5(cartridge) => cartridge.read_ram(address),
            Self::Mbc6(cartridge) => cartridge.read_ram(address),
            Self::Mbc7(cartridge) => cartridge.read_ram_timed(address, t_cycle),
            Self::PocketCamera(cartridge) => cartridge.read_ram_timed(address, t_cycle),
        }
    }

    pub(in crate::cartridge) fn write_ram(&mut self, address: u16, value: u8) {
        match self {
            Self::NoMbc(cartridge) => cartridge.write_ram(address, value),
            Self::Mmm01(cartridge) => cartridge.write_ram(address, value),
            Self::M161(cartridge) => cartridge.write_ram(address, value),
            Self::Huc1(cartridge) => cartridge.write_ram(address, value),
            Self::Huc3(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc1(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc2(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc3(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc5(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc6(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc7(cartridge) => cartridge.write_ram(address, value),
            Self::PocketCamera(cartridge) => cartridge.write_ram(address, value),
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
            Self::Mmm01(cartridge) => cartridge.write_ram(address, value),
            Self::M161(cartridge) => cartridge.write_ram(address, value),
            Self::Huc1(cartridge) => cartridge.write_ram(address, value),
            Self::Huc3(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc1(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc2(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc3(cartridge) => cartridge.write_ram_timed(address, value, t_cycle),
            Self::Mbc5(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc6(cartridge) => cartridge.write_ram(address, value),
            Self::Mbc7(cartridge) => cartridge.write_ram_timed(address, value, t_cycle),
            Self::PocketCamera(cartridge) => cartridge.write_ram_timed(address, value, t_cycle),
        }
    }

    pub(in crate::cartridge) fn advance_rtc_seconds(&mut self, seconds: u64) {
        match self {
            Self::NoMbc(_)
            | Self::Mmm01(_)
            | Self::M161(_)
            | Self::Huc1(_)
            | Self::Mbc1(_)
            | Self::Mbc2(_)
            | Self::Mbc5(_)
            | Self::Mbc6(_)
            | Self::Mbc7(_)
            | Self::PocketCamera(_) => {}
            Self::Mbc3(cartridge) => cartridge.advance_rtc_seconds(seconds),
            Self::Huc3(cartridge) => cartridge.advance_rtc_seconds(seconds),
        }
    }

    pub(in crate::cartridge) fn advance_mbc3_rtc_clock_ticks(&mut self, ticks: u64) {
        if let Self::Mbc3(cartridge) = self {
            cartridge.advance_rtc_clock_ticks(ticks);
        }
    }

    pub(in crate::cartridge) fn rumble_on(&self) -> bool {
        match self {
            Self::Mbc5(cartridge) => cartridge.rumble_on(),
            Self::NoMbc(_)
            | Self::Mmm01(_)
            | Self::M161(_)
            | Self::Huc1(_)
            | Self::Huc3(_)
            | Self::Mbc1(_)
            | Self::Mbc2(_)
            | Self::Mbc3(_)
            | Self::Mbc6(_)
            | Self::Mbc7(_)
            | Self::PocketCamera(_) => false,
        }
    }

    pub(in crate::cartridge) fn has_rumble(&self) -> bool {
        match self {
            Self::Mbc5(cartridge) => cartridge.has_rumble(),
            Self::NoMbc(_)
            | Self::Mmm01(_)
            | Self::M161(_)
            | Self::Huc1(_)
            | Self::Huc3(_)
            | Self::Mbc1(_)
            | Self::Mbc2(_)
            | Self::Mbc3(_)
            | Self::Mbc6(_)
            | Self::Mbc7(_)
            | Self::PocketCamera(_) => false,
        }
    }

    pub(in crate::cartridge) fn persistence_metadata(&self) -> CartridgePersistenceMetadata {
        match self {
            Self::NoMbc(cartridge) => cartridge.persistence_metadata(),
            Self::Mmm01(cartridge) => cartridge.persistence_metadata(),
            Self::M161(cartridge) => cartridge.persistence_metadata(),
            Self::Huc1(cartridge) => cartridge.persistence_metadata(),
            Self::Huc3(cartridge) => cartridge.persistence_metadata(),
            Self::Mbc1(cartridge) => cartridge.persistence_metadata(),
            Self::Mbc2(cartridge) => cartridge.persistence_metadata(),
            Self::Mbc3(cartridge) => cartridge.persistence_metadata(),
            Self::Mbc5(cartridge) => cartridge.persistence_metadata(),
            Self::Mbc6(cartridge) => cartridge.persistence_metadata(),
            Self::Mbc7(cartridge) => cartridge.persistence_metadata(),
            Self::PocketCamera(cartridge) => cartridge.persistence_metadata(),
        }
    }

    pub(in crate::cartridge) fn rtc_access_ready_at(&self) -> Option<TCycle> {
        match self {
            Self::Mbc3(cartridge) => cartridge.rtc_access_ready_at,
            Self::NoMbc(_)
            | Self::Mmm01(_)
            | Self::M161(_)
            | Self::Huc1(_)
            | Self::Huc3(_)
            | Self::Mbc1(_)
            | Self::Mbc2(_)
            | Self::Mbc5(_)
            | Self::Mbc6(_)
            | Self::Mbc7(_)
            | Self::PocketCamera(_) => None,
        }
    }

    pub(in crate::cartridge) fn camera_capture_ready_at(&self) -> Option<TCycle> {
        match self {
            Self::PocketCamera(cartridge) => cartridge.capture_ready_at(),
            Self::NoMbc(_)
            | Self::Mmm01(_)
            | Self::M161(_)
            | Self::Huc1(_)
            | Self::Huc3(_)
            | Self::Mbc1(_)
            | Self::Mbc2(_)
            | Self::Mbc3(_)
            | Self::Mbc5(_)
            | Self::Mbc6(_)
            | Self::Mbc7(_) => None,
        }
    }

    pub(in crate::cartridge) fn camera_registers_selected(&self) -> bool {
        match self {
            Self::PocketCamera(cartridge) => cartridge.registers_selected(),
            Self::NoMbc(_)
            | Self::Mmm01(_)
            | Self::M161(_)
            | Self::Huc1(_)
            | Self::Huc3(_)
            | Self::Mbc1(_)
            | Self::Mbc2(_)
            | Self::Mbc3(_)
            | Self::Mbc5(_)
            | Self::Mbc6(_)
            | Self::Mbc7(_) => false,
        }
    }

    pub(in crate::cartridge) fn persistent_state(&self) -> PersistentCartState {
        match self {
            Self::NoMbc(cartridge) => cartridge.persistent_state(),
            Self::Mmm01(cartridge) => cartridge.persistent_state(),
            Self::M161(cartridge) => cartridge.persistent_state(),
            Self::Huc1(cartridge) => cartridge.persistent_state(),
            Self::Huc3(cartridge) => cartridge.persistent_state(),
            Self::Mbc1(cartridge) => cartridge.persistent_state(),
            Self::Mbc2(cartridge) => cartridge.persistent_state(),
            Self::Mbc3(cartridge) => cartridge.persistent_state(),
            Self::Mbc5(cartridge) => cartridge.persistent_state(),
            Self::Mbc6(cartridge) => cartridge.persistent_state(),
            Self::Mbc7(cartridge) => cartridge.persistent_state(),
            Self::PocketCamera(cartridge) => cartridge.persistent_state(),
        }
    }

    pub(in crate::cartridge) fn restore_persistent_state(
        &mut self,
        state: &PersistentCartState,
    ) -> Result<(), CartridgePersistentStateError> {
        match self {
            Self::NoMbc(cartridge) => cartridge.restore_persistent_state(state),
            Self::Mmm01(cartridge) => cartridge.restore_persistent_state(state),
            Self::M161(cartridge) => cartridge.restore_persistent_state(state),
            Self::Huc1(cartridge) => cartridge.restore_persistent_state(state),
            Self::Huc3(cartridge) => cartridge.restore_persistent_state(state),
            Self::Mbc1(cartridge) => cartridge.restore_persistent_state(state),
            Self::Mbc2(cartridge) => cartridge.restore_persistent_state(state),
            Self::Mbc3(cartridge) => cartridge.restore_persistent_state(state),
            Self::Mbc5(cartridge) => cartridge.restore_persistent_state(state),
            Self::Mbc6(cartridge) => cartridge.restore_persistent_state(state),
            Self::Mbc7(cartridge) => cartridge.restore_persistent_state(state),
            Self::PocketCamera(cartridge) => cartridge.restore_persistent_state(state),
        }
    }

    pub(in crate::cartridge) fn has_pocket_camera(&self) -> bool {
        matches!(self, Self::PocketCamera(_))
    }

    pub(in crate::cartridge) fn has_mbc7_accelerometer(&self) -> bool {
        matches!(self, Self::Mbc7(_))
    }

    pub(in crate::cartridge) fn set_mbc7_accelerometer_input(
        &mut self,
        input: Mbc7AccelerometerInput,
    ) -> Result<(), Mbc7AccelerometerError> {
        match self {
            Self::Mbc7(cartridge) => {
                cartridge.set_accelerometer_input(input);
                Ok(())
            }
            Self::NoMbc(_)
            | Self::Mmm01(_)
            | Self::M161(_)
            | Self::Huc1(_)
            | Self::Huc3(_)
            | Self::Mbc1(_)
            | Self::Mbc2(_)
            | Self::Mbc3(_)
            | Self::Mbc5(_)
            | Self::Mbc6(_)
            | Self::PocketCamera(_) => Err(Mbc7AccelerometerError::UnsupportedCartridge),
        }
    }

    pub(in crate::cartridge) fn set_pocket_camera_frame(
        &mut self,
        frame: PocketCameraFrame,
    ) -> Result<(), PocketCameraFrameError> {
        match self {
            Self::PocketCamera(cartridge) => cartridge.set_host_frame(frame),
            Self::NoMbc(_)
            | Self::Mmm01(_)
            | Self::M161(_)
            | Self::Huc1(_)
            | Self::Huc3(_)
            | Self::Mbc1(_)
            | Self::Mbc2(_)
            | Self::Mbc3(_)
            | Self::Mbc5(_)
            | Self::Mbc6(_)
            | Self::Mbc7(_) => Err(PocketCameraFrameError::UnsupportedCartridge),
        }
    }

    pub(in crate::cartridge) fn clear_pocket_camera_frame(
        &mut self,
    ) -> Result<(), PocketCameraFrameError> {
        match self {
            Self::PocketCamera(cartridge) => {
                cartridge.clear_host_frame();
                Ok(())
            }
            Self::NoMbc(_)
            | Self::Mmm01(_)
            | Self::M161(_)
            | Self::Huc1(_)
            | Self::Huc3(_)
            | Self::Mbc1(_)
            | Self::Mbc2(_)
            | Self::Mbc3(_)
            | Self::Mbc5(_)
            | Self::Mbc6(_)
            | Self::Mbc7(_) => Err(PocketCameraFrameError::UnsupportedCartridge),
        }
    }
}
