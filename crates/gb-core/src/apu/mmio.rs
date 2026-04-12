use super::Apu;
use super::common::{
    NR13_WRITE_ONLY_READ_VALUE, NR31_WRITE_ONLY_READ_VALUE, NR33_WRITE_ONLY_READ_VALUE,
    NR41_WRITE_ONLY_READ_VALUE,
};

impl Apu {
    pub fn read_register(&self, address: u16) -> u8 {
        if let Some(index) = Self::wave_ram_index(address) {
            return self.channel_3.read_wave_ram(self.console_model, index);
        }

        self.read_non_wave_register(address)
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        self.last_register_write = None;

        if let Some(index) = Self::wave_ram_index(address) {
            self.channel_3
                .write_wave_ram(self.console_model, index, value);
            self.preview_output_path();
            return;
        }

        let before_register_write =
            Self::should_observe_register_write(address).then(|| self.register_write_state());

        if address == 0xFF26 {
            self.write_nr52(value);
            self.preview_output_path();
            self.record_register_write_observation(address, value, before_register_write);
            return;
        }

        if !self.master.powered {
            self.write_powered_off_register(address, value);
            self.preview_output_path();
            self.record_register_write_observation(address, value, before_register_write);
            return;
        }

        self.write_powered_register(address, value);
        self.preview_output_path();
        self.record_register_write_observation(address, value, before_register_write);
    }

    fn read_non_wave_register(&self, address: u16) -> u8 {
        match address {
            0xFF10 => self.channel_1.read_nr10(),
            0xFF11 => self.channel_1.read_nr11(),
            0xFF12 => self.channel_1.nr12,
            0xFF13 => NR13_WRITE_ONLY_READ_VALUE,
            0xFF14 => self.channel_1.read_nr14(),
            0xFF15 => 0xFF,
            0xFF16 => self.channel_2.read_nr21(),
            0xFF17 => self.channel_2.nr22,
            0xFF18 => NR13_WRITE_ONLY_READ_VALUE,
            0xFF19 => self.channel_2.read_nr24(),
            0xFF1A => self.channel_3.read_nr30(),
            0xFF1B => NR31_WRITE_ONLY_READ_VALUE,
            0xFF1C => self.channel_3.read_nr32(),
            0xFF1D => NR33_WRITE_ONLY_READ_VALUE,
            0xFF1E => self.channel_3.read_nr34(),
            0xFF1F => 0xFF,
            0xFF20 => NR41_WRITE_ONLY_READ_VALUE,
            0xFF21 => self.channel_4.nr42,
            0xFF22 => self.channel_4.nr43,
            0xFF23 => self.channel_4.read_nr44(),
            0xFF24 => self.master.nr50,
            0xFF25 => self.master.nr51,
            0xFF26 => self.read_nr52(),
            0xFF27..=0xFF2F => 0xFF,
            _ => 0xFF,
        }
    }

    fn write_powered_off_register(&mut self, address: u16, value: u8) {
        if !self.console_model.is_dmg_family() {
            return;
        }

        match address {
            0xFF11 => self.channel_1.write_length_while_powered_off(value),
            0xFF16 => self.channel_2.write_length_while_powered_off(value),
            0xFF1B => self.channel_3.write_length_while_powered_off(value),
            0xFF20 => self.channel_4.write_length_while_powered_off(value),
            _ => {}
        }
    }

    fn write_powered_register(&mut self, address: u16, value: u8) {
        match address {
            0xFF10 => self.channel_1.write_nr10(value),
            0xFF11 => self.channel_1.write_nr11(value),
            0xFF12 => self.channel_1.write_nr12(value),
            0xFF13 => self.channel_1.write_nr13(value),
            0xFF14 => {
                self.channel_1
                    .write_nr14(value, self.console_model, self.frame_sequencer.step)
            }
            0xFF15 => {}
            0xFF16 => self.channel_2.write_nr21(value),
            0xFF17 => self.channel_2.write_nr22(value),
            0xFF18 => self.channel_2.write_nr23(value),
            0xFF19 => {
                self.channel_2
                    .write_nr24(value, self.console_model, self.frame_sequencer.step)
            }
            0xFF1A => self.channel_3.write_nr30(value),
            0xFF1B => self.channel_3.write_nr31(value),
            0xFF1C => self.channel_3.write_nr32(value),
            0xFF1D => self.channel_3.write_nr33(value),
            0xFF1E => {
                self.channel_3
                    .write_nr34(value, self.console_model, self.frame_sequencer.step)
            }
            0xFF1F => {}
            0xFF20 => self.channel_4.write_nr41(value),
            0xFF21 => self.channel_4.write_nr42(value),
            0xFF22 => self.channel_4.write_nr43(value),
            0xFF23 => {
                self.channel_4
                    .write_nr44(value, self.console_model, self.frame_sequencer.step)
            }
            0xFF24 => self.master.nr50 = value,
            0xFF25 => self.master.nr51 = value,
            0xFF27..=0xFF3F => {}
            _ => {}
        }
    }

    fn wave_ram_index(address: u16) -> Option<usize> {
        match address {
            0xFF30..=0xFF3F => Some((address - 0xFF30) as usize),
            _ => None,
        }
    }
}
