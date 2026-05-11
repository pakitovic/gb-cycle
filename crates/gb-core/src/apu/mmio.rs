use super::Apu;
use super::common::APU_UNMAPPED_READ_VALUE;
use super::registers::{
    ApuMmioRegister, ApuRegister, ApuRegisterOwner, Channel1Register, Channel2Register,
    Channel3Register, Channel4Register, MasterRegister,
};
use crate::speed::CgbSpeedMode;

impl Apu {
    pub fn read_register(&self, address: u16) -> u8 {
        match ApuMmioRegister::decode(address) {
            ApuMmioRegister::Register(register) => self.read_apu_register(register),
            ApuMmioRegister::WaveRam(index) => self
                .channels
                .channel_3
                .read_wave_ram(self.console_model, index),
            ApuMmioRegister::Unmapped => APU_UNMAPPED_READ_VALUE,
        }
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        self.write_register_for_speed(address, value, CgbSpeedMode::Normal);
    }

    pub(crate) fn write_register_for_speed(
        &mut self,
        address: u16,
        value: u8,
        speed_mode: CgbSpeedMode,
    ) {
        self.write_register_for_speed_with_div_apu_signal(address, value, speed_mode, false);
    }

    pub(crate) fn write_register_for_speed_with_div_apu_signal(
        &mut self,
        address: u16,
        value: u8,
        speed_mode: CgbSpeedMode,
        div_apu_signal_high: bool,
    ) {
        self.last_register_write = None;
        let decoded_register = ApuMmioRegister::decode(address);

        match decoded_register {
            ApuMmioRegister::WaveRam(index) => {
                self.channels
                    .channel_3
                    .write_wave_ram(self.console_model, index, value);
                self.preview_output_path();
            }
            ApuMmioRegister::Register(register) => {
                let before_register_write = decoded_register
                    .should_observe_register_write()
                    .then(|| self.register_write_state());

                self.write_apu_register(register, value, speed_mode, div_apu_signal_high);

                self.preview_output_path();
                self.record_register_write_observation(address, value, before_register_write);
            }
            ApuMmioRegister::Unmapped => {
                self.preview_output_path();
            }
        }
    }

    fn read_apu_register(&self, register: ApuRegister) -> u8 {
        match register.owner() {
            ApuRegisterOwner::Channel1(register) => self.channels.channel_1.read_register(register),
            ApuRegisterOwner::Channel2(register) => self.channels.channel_2.read_register(register),
            ApuRegisterOwner::Channel3(register) => self.channels.channel_3.read_register(register),
            ApuRegisterOwner::Channel4(register) => self.channels.channel_4.read_register(register),
            ApuRegisterOwner::Master(register) => self.read_master_register(register),
            ApuRegisterOwner::Unused => APU_UNMAPPED_READ_VALUE,
        }
    }

    fn read_master_register(&self, register: MasterRegister) -> u8 {
        match register {
            MasterRegister::Nr50 => self.master.nr50,
            MasterRegister::Nr51 => self.master.nr51,
            MasterRegister::Nr52 => self.read_nr52(),
        }
    }

    fn write_apu_register(
        &mut self,
        register: ApuRegister,
        value: u8,
        speed_mode: CgbSpeedMode,
        div_apu_signal_high: bool,
    ) {
        match register.owner() {
            ApuRegisterOwner::Channel1(register) => {
                self.write_channel_1_register(register, value, speed_mode)
            }
            ApuRegisterOwner::Channel2(register) => {
                self.write_channel_2_register(register, value, speed_mode)
            }
            ApuRegisterOwner::Channel3(register) => self.write_channel_3_register(register, value),
            ApuRegisterOwner::Channel4(register) => self.write_channel_4_register(register, value),
            ApuRegisterOwner::Master(register) => {
                self.write_master_register(register, value, div_apu_signal_high)
            }
            ApuRegisterOwner::Unused => {}
        }
    }

    fn effective_frame_sequencer_step(&self) -> u8 {
        if self.skip_next_frame_sequencer_edge {
            1
        } else {
            self.frame_sequencer.step
        }
    }

    fn write_channel_1_register(
        &mut self,
        register: Channel1Register,
        value: u8,
        speed_mode: CgbSpeedMode,
    ) {
        if self.master.powered {
            let step = self.effective_frame_sequencer_step();
            self.channels.channel_1.write_register(
                register,
                value,
                self.console_model,
                speed_mode,
                step,
            );
        } else {
            self.channels
                .channel_1
                .write_powered_off_register(register, value, self.console_model);
        }
    }

    fn write_channel_2_register(
        &mut self,
        register: Channel2Register,
        value: u8,
        speed_mode: CgbSpeedMode,
    ) {
        if self.master.powered {
            let step = self.effective_frame_sequencer_step();
            self.channels.channel_2.write_register(
                register,
                value,
                self.console_model,
                speed_mode,
                step,
            );
        } else {
            self.channels
                .channel_2
                .write_powered_off_register(register, value, self.console_model);
        }
    }

    fn write_channel_3_register(&mut self, register: Channel3Register, value: u8) {
        if self.master.powered {
            let step = self.effective_frame_sequencer_step();
            self.channels
                .channel_3
                .write_register(register, value, self.console_model, step);
        } else {
            self.channels
                .channel_3
                .write_powered_off_register(register, value, self.console_model);
        }
    }

    fn write_channel_4_register(&mut self, register: Channel4Register, value: u8) {
        if self.master.powered {
            let step = self.effective_frame_sequencer_step();
            self.channels
                .channel_4
                .write_register(register, value, self.console_model, step);
        } else {
            self.channels
                .channel_4
                .write_powered_off_register(register, value, self.console_model);
        }
    }

    fn write_master_register(
        &mut self,
        register: MasterRegister,
        value: u8,
        div_apu_signal_high: bool,
    ) {
        match register {
            MasterRegister::Nr50 => {
                if self.master.powered {
                    self.master.nr50 = value;
                }
            }
            MasterRegister::Nr51 => {
                if self.master.powered {
                    self.master.nr51 = value;
                }
            }
            MasterRegister::Nr52 => self.write_nr52(value, div_apu_signal_high),
        }
    }
}
