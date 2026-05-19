use super::*;
use crate::scheduler::TCycle;

const MBC7_ROM_BANK_BYTES: usize = 0x4000;
const MBC7_REGISTER_START: u16 = 0xA000;
const MBC7_REGISTER_END: u16 = 0xAFFF;
const MBC7_FIXED_OPEN_END: u16 = 0xBFFF;
const MBC7_EEPROM_PIN_CS: u8 = 0x80;
const MBC7_EEPROM_PIN_CLK: u8 = 0x40;
const MBC7_EEPROM_PIN_DI: u8 = 0x02;
const MBC7_EEPROM_PIN_DO: u8 = 0x01;
const MBC7_EEPROM_COMMAND_BITS: u8 = 10;
const MBC7_EEPROM_DATA_BITS: u8 = 16;

impl Default for Mbc7EepromPins {
    fn default() -> Self {
        Self {
            cs: false,
            clk: false,
            di: false,
            do_pin: true,
        }
    }
}

impl Mbc7Cartridge {
    pub(in crate::cartridge) fn describe_external_access(
        &self,
        address: u16,
    ) -> CartridgeExternalAccessInfo {
        if !(MBC7_REGISTER_START..=MBC7_FIXED_OPEN_END).contains(&address) {
            return CartridgeExternalAccessInfo::no_device(address);
        }

        if address > MBC7_REGISTER_END {
            return CartridgeExternalAccessInfo::new(
                address,
                CartridgeExternalTarget::Mbc7ReservedRegister { selector: 0x10 },
                CartridgeExternalAvailability::Reserved,
                CartridgeExternalReadBehavior::FallbackValue(RAM_ABSENT_READ_VALUE),
                CartridgeExternalWriteBehavior::Ignored,
            );
        }

        let selector = self.register_selector(address);
        let enabled = self.registers_enabled();
        let availability = if enabled {
            match selector {
                0x00..=0x06 | 0x08 => CartridgeExternalAvailability::Accessible,
                _ => CartridgeExternalAvailability::Reserved,
            }
        } else {
            CartridgeExternalAvailability::Disabled
        };

        let (target, read_behavior, write_behavior) = match selector {
            0x00 => (
                CartridgeExternalTarget::Mbc7AccelerometerLatchReset,
                CartridgeExternalReadBehavior::FallbackValue(RAM_ABSENT_READ_VALUE),
                CartridgeExternalWriteBehavior::Mbc7AccelerometerLatch,
            ),
            0x01 => (
                CartridgeExternalTarget::Mbc7AccelerometerLatchCommit,
                CartridgeExternalReadBehavior::FallbackValue(RAM_ABSENT_READ_VALUE),
                CartridgeExternalWriteBehavior::Mbc7AccelerometerLatch,
            ),
            0x02 => (
                CartridgeExternalTarget::Mbc7AccelerometerAxis {
                    axis: Mbc7AccelerometerAxis::X,
                    byte: Mbc7AccelerometerByte::Low,
                },
                CartridgeExternalReadBehavior::Mbc7Accelerometer,
                CartridgeExternalWriteBehavior::Ignored,
            ),
            0x03 => (
                CartridgeExternalTarget::Mbc7AccelerometerAxis {
                    axis: Mbc7AccelerometerAxis::X,
                    byte: Mbc7AccelerometerByte::High,
                },
                CartridgeExternalReadBehavior::Mbc7Accelerometer,
                CartridgeExternalWriteBehavior::Ignored,
            ),
            0x04 => (
                CartridgeExternalTarget::Mbc7AccelerometerAxis {
                    axis: Mbc7AccelerometerAxis::Y,
                    byte: Mbc7AccelerometerByte::Low,
                },
                CartridgeExternalReadBehavior::Mbc7Accelerometer,
                CartridgeExternalWriteBehavior::Ignored,
            ),
            0x05 => (
                CartridgeExternalTarget::Mbc7AccelerometerAxis {
                    axis: Mbc7AccelerometerAxis::Y,
                    byte: Mbc7AccelerometerByte::High,
                },
                CartridgeExternalReadBehavior::Mbc7Accelerometer,
                CartridgeExternalWriteBehavior::Ignored,
            ),
            0x06 => (
                CartridgeExternalTarget::Mbc7FixedRegister { value: 0x00 },
                CartridgeExternalReadBehavior::FallbackValue(0x00),
                CartridgeExternalWriteBehavior::Ignored,
            ),
            0x08 => (
                CartridgeExternalTarget::Mbc7EepromSerial,
                CartridgeExternalReadBehavior::Mbc7EepromSerial,
                CartridgeExternalWriteBehavior::Mbc7EepromSerial,
            ),
            other => (
                CartridgeExternalTarget::Mbc7ReservedRegister { selector: other },
                CartridgeExternalReadBehavior::FallbackValue(RAM_ABSENT_READ_VALUE),
                CartridgeExternalWriteBehavior::Ignored,
            ),
        };

        if enabled {
            CartridgeExternalAccessInfo::new(
                address,
                target,
                availability,
                read_behavior,
                write_behavior,
            )
        } else {
            CartridgeExternalAccessInfo::new(
                address,
                target,
                availability,
                CartridgeExternalReadBehavior::FallbackValue(RAM_ABSENT_READ_VALUE),
                CartridgeExternalWriteBehavior::Ignored,
            )
        }
    }

    pub(in crate::cartridge) fn read_rom(&self, address: u16) -> u8 {
        let address = address as usize;
        let bank_count = self.header.rom_size.bank_count.unwrap_or(0).max(1);

        let rom_index = if address < MBC7_ROM_BANK_BYTES {
            address
        } else {
            let bank = self.effective_rom_bank(bank_count);
            bank * MBC7_ROM_BANK_BYTES + (address - MBC7_ROM_BANK_BYTES)
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

        let address = address as usize;
        let bank = if address < MBC7_ROM_BANK_BYTES {
            0
        } else {
            self.effective_rom_bank(self.header.rom_size.bank_count.unwrap_or(0).max(1))
        };
        let bank_offset = if address < MBC7_ROM_BANK_BYTES {
            address
        } else {
            address - MBC7_ROM_BANK_BYTES
        };
        Some(CartridgeMappedRomWindow::rom(
            bank,
            MBC7_ROM_BANK_BYTES,
            bank_offset,
        ))
    }

    pub(in crate::cartridge) fn write_rom(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1FFF => {
                self.ram_enabled = value & 0x0F == 0x0A;
            }
            0x2000..=0x3FFF => {
                self.rom_bank = value & 0x7F;
            }
            0x4000..=0x5FFF => {
                self.sensor_eeprom_enabled = value == 0x40;
            }
            _ => {}
        }
    }

    pub(in crate::cartridge) fn read_ram(&self, address: u16) -> u8 {
        self.read_register(address)
    }

    pub(in crate::cartridge) fn read_ram_timed(&mut self, address: u16, _t_cycle: TCycle) -> u8 {
        self.read_register(address)
    }

    pub(in crate::cartridge) fn write_ram(&mut self, address: u16, value: u8) {
        self.write_register(address, value)
    }

    pub(in crate::cartridge) fn write_ram_timed(
        &mut self,
        address: u16,
        value: u8,
        _t_cycle: TCycle,
    ) {
        self.write_register(address, value)
    }

    pub(in crate::cartridge) fn persistence_metadata(&self) -> CartridgePersistenceMetadata {
        CartridgePersistenceMetadata {
            has_battery: false,
            has_rtc: false,
            profile: CartridgePersistenceProfile::PersistentEeprom {
                byte_len: self.eeprom.len(),
            },
        }
    }

    pub(in crate::cartridge) fn persistent_state(&self) -> PersistentCartState {
        PersistentCartState::Mbc7Eeprom {
            eeprom: self.eeprom.clone(),
        }
    }

    pub(in crate::cartridge) fn restore_persistent_state(
        &mut self,
        state: &PersistentCartState,
    ) -> Result<(), CartridgePersistentStateError> {
        match state {
            PersistentCartState::Mbc7Eeprom { eeprom } => {
                if self.eeprom.len() != eeprom.len() {
                    return Err(CartridgePersistentStateError::EepromLengthMismatch {
                        expected: self.eeprom.len(),
                        actual: eeprom.len(),
                    });
                }
                self.eeprom.copy_from_slice(eeprom);
                self.eeprom_pins = Mbc7EepromPins::default();
                self.eeprom_command = Mbc7EepromCommand::Idle;
                self.eeprom_write_enabled = false;
                Ok(())
            }
            other => Err(CartridgePersistentStateError::KindMismatch {
                expected: "Mbc7Eeprom",
                actual: other.kind_name(),
            }),
        }
    }

    pub(in crate::cartridge) fn set_accelerometer_input(&mut self, input: Mbc7AccelerometerInput) {
        self.accelerometer_input = input;
    }

    fn effective_rom_bank(&self, bank_count: usize) -> usize {
        self.rom_bank as usize % bank_count.max(1)
    }

    fn registers_enabled(&self) -> bool {
        self.ram_enabled && self.sensor_eeprom_enabled
    }

    fn read_register(&self, address: u16) -> u8 {
        if !(MBC7_REGISTER_START..=MBC7_REGISTER_END).contains(&address)
            || !self.registers_enabled()
        {
            return RAM_ABSENT_READ_VALUE;
        }

        match self.register_selector(address) {
            0x02 => self.latched_x as u8,
            0x03 => (self.latched_x >> 8) as u8,
            0x04 => self.latched_y as u8,
            0x05 => (self.latched_y >> 8) as u8,
            0x06 => 0x00,
            0x08 => self.read_eeprom_pins(),
            _ => RAM_ABSENT_READ_VALUE,
        }
    }

    fn write_register(&mut self, address: u16, value: u8) {
        if !(MBC7_REGISTER_START..=MBC7_REGISTER_END).contains(&address)
            || !self.registers_enabled()
        {
            return;
        }

        match self.register_selector(address) {
            0x00 if value == 0x55 => {
                self.accelerometer_latch_armed = true;
                self.latched_x = MBC7_ACCELEROMETER_UNLATCHED_VALUE;
                self.latched_y = MBC7_ACCELEROMETER_UNLATCHED_VALUE;
            }
            0x01 if value == 0xAA && self.accelerometer_latch_armed => {
                self.latched_x = self.accelerometer_input.x_raw;
                self.latched_y = self.accelerometer_input.y_raw;
                self.accelerometer_latch_armed = false;
            }
            0x08 => self.write_eeprom_pins(value),
            _ => {}
        }
    }

    fn register_selector(&self, address: u16) -> u8 {
        ((address >> 4) & 0x0F) as u8
    }

    fn read_eeprom_pins(&self) -> u8 {
        (u8::from(self.eeprom_pins.cs) << 7)
            | (u8::from(self.eeprom_pins.clk) << 6)
            | (u8::from(self.eeprom_pins.di) << 1)
            | if self.eeprom_pins.do_pin {
                MBC7_EEPROM_PIN_DO
            } else {
                0
            }
    }

    fn write_eeprom_pins(&mut self, value: u8) {
        let previous_clk = self.eeprom_pins.clk;
        let new_cs = value & MBC7_EEPROM_PIN_CS != 0;
        let new_clk = value & MBC7_EEPROM_PIN_CLK != 0;
        let new_di = value & MBC7_EEPROM_PIN_DI != 0;

        self.eeprom_pins.cs = new_cs;
        self.eeprom_pins.clk = new_clk;
        self.eeprom_pins.di = new_di;

        if !new_cs {
            self.eeprom_command = Mbc7EepromCommand::Idle;
            self.eeprom_pins.do_pin = true;
            return;
        }

        if !previous_clk && new_clk {
            self.clock_eeprom();
        }
    }

    fn clock_eeprom(&mut self) {
        let command = self.eeprom_command;
        match command {
            Mbc7EepromCommand::Idle => {
                if self.eeprom_pins.di {
                    self.eeprom_command = Mbc7EepromCommand::ReceivingCommand { bits: 0, value: 0 };
                    self.eeprom_pins.do_pin = true;
                }
            }
            Mbc7EepromCommand::ReceivingCommand { bits, value } => {
                let value = (value << 1) | u16::from(self.eeprom_pins.di);
                let bits = bits + 1;
                if bits == MBC7_EEPROM_COMMAND_BITS {
                    self.finish_eeprom_command(value);
                } else {
                    self.eeprom_command = Mbc7EepromCommand::ReceivingCommand { bits, value };
                }
            }
            Mbc7EepromCommand::ReceivingData {
                target,
                bits,
                value,
            } => {
                let value = (value << 1) | u16::from(self.eeprom_pins.di);
                let bits = bits + 1;
                if bits == MBC7_EEPROM_DATA_BITS {
                    self.commit_eeprom_data(target, value);
                } else {
                    self.eeprom_command = Mbc7EepromCommand::ReceivingData {
                        target,
                        bits,
                        value,
                    };
                }
            }
            Mbc7EepromCommand::SendingRead {
                bits_remaining,
                value,
            } => {
                if bits_remaining == 0 {
                    self.eeprom_command = Mbc7EepromCommand::Idle;
                    return;
                }

                let bit_index = bits_remaining - 1;
                self.eeprom_pins.do_pin = (value >> bit_index) & 0x0001 != 0;
                let bits_remaining = bits_remaining - 1;
                self.eeprom_command = if bits_remaining == 0 {
                    Mbc7EepromCommand::Idle
                } else {
                    Mbc7EepromCommand::SendingRead {
                        bits_remaining,
                        value,
                    }
                };
            }
        }
    }

    fn finish_eeprom_command(&mut self, command: u16) {
        let opcode = (command >> 8) & 0x03;
        let address = (command & 0x7F) as u8;

        match opcode {
            0b10 => {
                let value = self.read_eeprom_word(address);
                self.eeprom_command = Mbc7EepromCommand::SendingRead {
                    bits_remaining: MBC7_EEPROM_DATA_BITS,
                    value,
                };
                self.eeprom_pins.do_pin = false;
            }
            0b01 => {
                self.eeprom_command = Mbc7EepromCommand::ReceivingData {
                    target: Mbc7EepromDataTarget::WriteWord { address },
                    bits: 0,
                    value: 0,
                };
            }
            0b11 => {
                if self.eeprom_write_enabled {
                    self.write_eeprom_word(address, u16::MAX);
                }
                self.eeprom_command = Mbc7EepromCommand::Idle;
                self.eeprom_pins.do_pin = true;
            }
            0b00 => self.finish_eeprom_extended_command(command),
            _ => unreachable!("two-bit MBC7 EEPROM opcode should be exhaustive"),
        }
    }

    fn finish_eeprom_extended_command(&mut self, command: u16) {
        match (command >> 6) & 0x03 {
            0b00 => {
                self.eeprom_write_enabled = false;
                self.eeprom_command = Mbc7EepromCommand::Idle;
                self.eeprom_pins.do_pin = true;
            }
            0b01 => {
                self.eeprom_command = Mbc7EepromCommand::ReceivingData {
                    target: Mbc7EepromDataTarget::WriteAll,
                    bits: 0,
                    value: 0,
                };
            }
            0b10 => {
                if self.eeprom_write_enabled {
                    self.eeprom.fill(0xFF);
                }
                self.eeprom_command = Mbc7EepromCommand::Idle;
                self.eeprom_pins.do_pin = true;
            }
            0b11 => {
                self.eeprom_write_enabled = true;
                self.eeprom_command = Mbc7EepromCommand::Idle;
                self.eeprom_pins.do_pin = true;
            }
            _ => unreachable!("two-bit MBC7 EEPROM extended opcode should be exhaustive"),
        }
    }

    fn commit_eeprom_data(&mut self, target: Mbc7EepromDataTarget, value: u16) {
        if self.eeprom_write_enabled {
            match target {
                Mbc7EepromDataTarget::WriteWord { address } => {
                    self.write_eeprom_word(address, value);
                }
                Mbc7EepromDataTarget::WriteAll => {
                    for address in 0..MBC7_EEPROM_WORDS as u8 {
                        self.write_eeprom_word(address, value);
                    }
                }
            }
        }
        self.eeprom_command = Mbc7EepromCommand::Idle;
        self.eeprom_pins.do_pin = true;
    }

    fn read_eeprom_word(&self, address: u8) -> u16 {
        let offset = self.eeprom_word_offset(address);
        u16::from_be_bytes([self.eeprom[offset], self.eeprom[offset + 1]])
    }

    fn write_eeprom_word(&mut self, address: u8, value: u16) {
        let offset = self.eeprom_word_offset(address);
        let bytes = value.to_be_bytes();
        self.eeprom[offset] = bytes[0];
        self.eeprom[offset + 1] = bytes[1];
    }

    fn eeprom_word_offset(&self, address: u8) -> usize {
        (address as usize % MBC7_EEPROM_WORDS) * 2
    }
}
