use crate::boot::StartupMemoryPolicy;
use crate::cartridge::CartridgeSlot;
use crate::scheduler::TCycle;

use super::{
    BLOCKED_READ_VALUE, Bus, BusAddressInfo, BusIoReadView, BusIoWriteView, BusRegion,
    IoRegisterAvailability, IoRegisterImplementation, IoRegisterKind,
};

impl Bus {
    pub(super) fn perform_allowed_read(
        &self,
        target: BusAddressInfo,
        cartridge: Option<&CartridgeSlot>,
        io: BusIoReadView<'_>,
    ) -> u8 {
        match target.region() {
            BusRegion::BootRom => self.read_boot_rom_placeholder(target.address(), io),
            BusRegion::CartridgeRomBank0
            | BusRegion::CartridgeRomBankN
            | BusRegion::CartridgeExternal => {
                self.read_cartridge_target(target.address(), target.region(), cartridge)
            }
            BusRegion::Vram => self.vram.read(target.region_offset() as usize),
            BusRegion::WramBank0 | BusRegion::WramBankN | BusRegion::EchoRam => {
                self.wram.read(target.address())
            }
            BusRegion::Oam => self.oam.read(target.region_offset() as usize),
            BusRegion::Unusable => self.read_unusable_placeholder(target.address()),
            BusRegion::Mmio | BusRegion::InterruptEnable | BusRegion::Hram => {
                self.read_iohram_target(target, io)
            }
        }
    }

    pub(super) fn perform_allowed_read_timed(
        &self,
        target: BusAddressInfo,
        t_cycle: TCycle,
        cartridge: Option<&mut CartridgeSlot>,
        io: BusIoReadView<'_>,
    ) -> u8 {
        match target.region() {
            BusRegion::BootRom => self.read_boot_rom_placeholder(target.address(), io),
            BusRegion::CartridgeRomBank0
            | BusRegion::CartridgeRomBankN
            | BusRegion::CartridgeExternal => self.read_cartridge_target_timed(
                target.address(),
                target.region(),
                t_cycle,
                cartridge,
            ),
            BusRegion::Vram => self.vram.read(target.region_offset() as usize),
            BusRegion::WramBank0 | BusRegion::WramBankN | BusRegion::EchoRam => {
                self.wram.read(target.address())
            }
            BusRegion::Oam => self.oam.read(target.region_offset() as usize),
            BusRegion::Unusable => self.read_unusable_placeholder(target.address()),
            BusRegion::Mmio | BusRegion::InterruptEnable | BusRegion::Hram => {
                self.read_iohram_target(target, io)
            }
        }
    }

    pub(super) fn perform_allowed_write(
        &mut self,
        target: BusAddressInfo,
        value: u8,
        cartridge: Option<&mut CartridgeSlot>,
        io: BusIoWriteView<'_>,
    ) {
        match target.region() {
            BusRegion::BootRom => unreachable!("boot ROM overlay must not own writes"),
            BusRegion::CartridgeRomBank0
            | BusRegion::CartridgeRomBankN
            | BusRegion::CartridgeExternal => {
                self.write_cartridge_target(target.address(), target.region(), value, cartridge)
            }
            BusRegion::Vram => self.vram.write(target.region_offset() as usize, value),
            BusRegion::WramBank0 | BusRegion::WramBankN | BusRegion::EchoRam => {
                self.wram.write(target.address(), value);
            }
            BusRegion::Oam => self.oam.write(target.region_offset() as usize, value),
            BusRegion::Unusable => {}
            BusRegion::Mmio | BusRegion::InterruptEnable | BusRegion::Hram => {
                self.write_iohram_target(target, value, io)
            }
        }
    }

    fn read_iohram_target(&self, target: BusAddressInfo, io: BusIoReadView<'_>) -> u8 {
        if target.region() == BusRegion::Mmio
            && let Some(value) = self.read_cgb_bus_owned_io(target.address())
        {
            return value;
        }

        self.iohram
            .read(&self.router, self.console_model, target, io)
    }

    fn write_iohram_target(&mut self, target: BusAddressInfo, value: u8, io: BusIoWriteView<'_>) {
        if target.region() == BusRegion::Mmio
            && self.write_cgb_bus_owned_io(target.address(), value)
        {
            return;
        }

        self.iohram
            .write(&self.router, self.console_model, target, value, io);
    }

    fn read_cgb_bus_owned_io(&self, address: u16) -> Option<u8> {
        let info = self.router.describe_io_register(address)?;
        match info.kind() {
            IoRegisterKind::Key0
            | IoRegisterKind::Vbk
            | IoRegisterKind::Svbk
            | IoRegisterKind::CgbUndocumented72
            | IoRegisterKind::CgbUndocumented73
            | IoRegisterKind::CgbUndocumented74
            | IoRegisterKind::CgbUndocumented75 => {}
            _ => return None,
        }

        if !self.io_register_is_live(info.availability())
            || info.implementation() != IoRegisterImplementation::Implemented
        {
            return Some(BLOCKED_READ_VALUE);
        }

        Some(match info.kind() {
            IoRegisterKind::Key0 => self.iohram.read_key0(),
            IoRegisterKind::Vbk => self.vram.read_vbk(),
            IoRegisterKind::Svbk => self.wram.read_svbk(),
            IoRegisterKind::CgbUndocumented72
            | IoRegisterKind::CgbUndocumented73
            | IoRegisterKind::CgbUndocumented74
            | IoRegisterKind::CgbUndocumented75 => self.iohram.read_cgb_misc_io(address),
            _ => unreachable!("filtered CGB bus-owned IO kind changed"),
        })
    }

    fn write_cgb_bus_owned_io(&mut self, address: u16, value: u8) -> bool {
        let Some(info) = self.router.describe_io_register(address) else {
            return false;
        };
        match info.kind() {
            IoRegisterKind::Key0
            | IoRegisterKind::Vbk
            | IoRegisterKind::Svbk
            | IoRegisterKind::CgbUndocumented72
            | IoRegisterKind::CgbUndocumented73
            | IoRegisterKind::CgbUndocumented74
            | IoRegisterKind::CgbUndocumented75 => {}
            _ => return false,
        }

        if !self.io_register_is_live(info.availability())
            || info.implementation() != IoRegisterImplementation::Implemented
        {
            return true;
        }

        match info.kind() {
            IoRegisterKind::Key0 => self.iohram.write_key0(value),
            IoRegisterKind::Vbk => self.vram.write_vbk(value),
            IoRegisterKind::Svbk => self.wram.write_svbk(value),
            IoRegisterKind::CgbUndocumented72
            | IoRegisterKind::CgbUndocumented73
            | IoRegisterKind::CgbUndocumented74
            | IoRegisterKind::CgbUndocumented75 => self.iohram.write_cgb_misc_io(address, value),
            _ => unreachable!("filtered CGB bus-owned IO kind changed"),
        }
        true
    }

    fn io_register_is_live(&self, availability: IoRegisterAvailability) -> bool {
        match availability {
            IoRegisterAvailability::Shared | IoRegisterAvailability::DmgCompatible => true,
            IoRegisterAvailability::CgbOnly => self.cgb_extensions_enabled(),
        }
    }

    pub fn apply_startup_memory_policy(&mut self, policy: StartupMemoryPolicy) {
        self.wram.apply_startup_memory_policy(policy);
        self.iohram.apply_startup_memory_policy(policy);
    }

    fn read_boot_rom_placeholder(&self, address: u16, io: BusIoReadView<'_>) -> u8 {
        io.boot
            .map_or(BLOCKED_READ_VALUE, |boot| boot.read_boot_rom(address))
    }

    fn read_cartridge_target(
        &self,
        address: u16,
        region: BusRegion,
        cartridge: Option<&CartridgeSlot>,
    ) -> u8 {
        match cartridge {
            Some(cartridge) => match region {
                BusRegion::CartridgeRomBank0 | BusRegion::CartridgeRomBankN => {
                    cartridge.read_rom(address)
                }
                BusRegion::CartridgeExternal => cartridge.read_ram(address),
                _ => unreachable!("non-cartridge region routed to cartridge target"),
            },
            None => BLOCKED_READ_VALUE,
        }
    }

    pub(super) fn read_cartridge_target_timed(
        &self,
        address: u16,
        region: BusRegion,
        t_cycle: TCycle,
        cartridge: Option<&mut CartridgeSlot>,
    ) -> u8 {
        match cartridge {
            Some(cartridge) => match region {
                BusRegion::CartridgeRomBank0 | BusRegion::CartridgeRomBankN => {
                    cartridge.read_rom(address)
                }
                BusRegion::CartridgeExternal => cartridge.read_ram_timed(address, t_cycle),
                _ => unreachable!("non-cartridge region routed to cartridge target"),
            },
            None => BLOCKED_READ_VALUE,
        }
    }

    fn write_cartridge_target(
        &mut self,
        address: u16,
        region: BusRegion,
        value: u8,
        cartridge: Option<&mut CartridgeSlot>,
    ) {
        if let Some(cartridge) = cartridge {
            match region {
                BusRegion::CartridgeRomBank0 | BusRegion::CartridgeRomBankN => {
                    cartridge.write_rom(address, value)
                }
                BusRegion::CartridgeExternal => cartridge.write_ram(address, value),
                _ => unreachable!("non-cartridge region routed to cartridge target"),
            }
        }
    }

    pub(super) fn write_cartridge_target_timed(
        &mut self,
        address: u16,
        region: BusRegion,
        value: u8,
        t_cycle: TCycle,
        cartridge: Option<&mut CartridgeSlot>,
    ) {
        if let Some(cartridge) = cartridge {
            match region {
                BusRegion::CartridgeRomBank0 | BusRegion::CartridgeRomBankN => {
                    cartridge.write_rom(address, value)
                }
                BusRegion::CartridgeExternal => cartridge.write_ram_timed(address, value, t_cycle),
                _ => unreachable!("non-cartridge region routed to cartridge target"),
            }
        }
    }

    pub(super) fn perform_allowed_write_timed(
        &mut self,
        target: BusAddressInfo,
        value: u8,
        t_cycle: TCycle,
        cartridge: Option<&mut CartridgeSlot>,
        io: BusIoWriteView<'_>,
    ) {
        match target.region() {
            BusRegion::BootRom => unreachable!("boot ROM overlay must not own writes"),
            BusRegion::CartridgeRomBank0
            | BusRegion::CartridgeRomBankN
            | BusRegion::CartridgeExternal => self.write_cartridge_target_timed(
                target.address(),
                target.region(),
                value,
                t_cycle,
                cartridge,
            ),
            BusRegion::Vram => self.vram.write(target.region_offset() as usize, value),
            BusRegion::WramBank0 | BusRegion::WramBankN | BusRegion::EchoRam => {
                self.wram.write(target.address(), value);
            }
            BusRegion::Oam => self.oam.write(target.region_offset() as usize, value),
            BusRegion::Unusable => {}
            BusRegion::Mmio | BusRegion::InterruptEnable | BusRegion::Hram => {
                self.write_iohram_target(target, value, io)
            }
        }
    }

    fn read_unusable_placeholder(&self, address: u16) -> u8 {
        self.describe_unusable_area(address)
            .map(|info| info.runtime_fallback_read_value())
            .unwrap_or(BLOCKED_READ_VALUE)
    }

    #[cfg(test)]
    pub(super) fn read_io_target(&self, address: u16, io: BusIoReadView<'_>) -> u8 {
        let target = self.decode_address(address);
        self.read_iohram_target(target, io)
    }
}
