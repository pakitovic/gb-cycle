use crate::model::ConsoleModel;
use crate::scheduler::CycleContext;

const CHANNEL_ACTIVE_CH1: u8 = 0x01;
const CHANNEL_ACTIVE_CH2: u8 = 0x02;
const CHANNEL_ACTIVE_CH3: u8 = 0x04;
const CHANNEL_ACTIVE_CH4: u8 = 0x08;
const CHANNEL_ACTIVE_MASK: u8 = 0x0F;
const NR10_FORCED_HIGH_MASK: u8 = 0x80;
const NR11_WRITE_ONLY_MASK: u8 = 0x3F;
const NR13_WRITE_ONLY_READ_VALUE: u8 = 0xFF;
const NR14_READ_MASK: u8 = 0x40;
const NR14_FORCED_HIGH_MASK: u8 = 0xBF;
const NR30_FORCED_HIGH_MASK: u8 = 0x7F;
const NR31_WRITE_ONLY_READ_VALUE: u8 = 0xFF;
const NR32_READ_MASK: u8 = 0x60;
const NR32_FORCED_HIGH_MASK: u8 = 0x9F;
const NR33_WRITE_ONLY_READ_VALUE: u8 = 0xFF;
const NR41_WRITE_ONLY_READ_VALUE: u8 = 0xFF;
const NR44_READ_MASK: u8 = 0x40;
const NR44_FORCED_HIGH_MASK: u8 = 0xBF;
const NR52_FORCED_HIGH_MASK: u8 = 0x70;
const NR52_POWER_BIT: u8 = 0x80;
const WAVE_RAM_LEN: usize = 0x10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApuStatus {
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum WaveRamStartupPolicy {
    #[default]
    DeterministicZeroed,
}

impl WaveRamStartupPolicy {
    pub const fn initial_bytes(self) -> [u8; WAVE_RAM_LEN] {
        match self {
            Self::DeterministicZeroed => [0; WAVE_RAM_LEN],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ApuStartupState {
    pub powered: bool,
    pub nr10: u8,
    pub nr11: u8,
    pub nr12: u8,
    pub nr13: u8,
    pub nr14: u8,
    pub nr21: u8,
    pub nr22: u8,
    pub nr23: u8,
    pub nr24: u8,
    pub nr30: u8,
    pub nr31: u8,
    pub nr32: u8,
    pub nr33: u8,
    pub nr34: u8,
    pub nr41: u8,
    pub nr42: u8,
    pub nr43: u8,
    pub nr44: u8,
    pub nr50: u8,
    pub nr51: u8,
    pub channel_active_mask: u8,
    pub div_apu: u8,
    pub wave_ram_startup_policy: WaveRamStartupPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Apu {
    console_model: ConsoleModel,
    status: ApuStatus,
    powered: bool,
    nr10: u8,
    nr11: u8,
    nr12: u8,
    nr13: u8,
    nr14: u8,
    nr21: u8,
    nr22: u8,
    nr23: u8,
    nr24: u8,
    nr30: u8,
    nr31: u8,
    nr32: u8,
    nr33: u8,
    nr34: u8,
    nr41: u8,
    nr42: u8,
    nr43: u8,
    nr44: u8,
    nr50: u8,
    nr51: u8,
    channel_active_mask: u8,
    div_apu: u8,
    wave_ram: [u8; WAVE_RAM_LEN],
    wave_ram_startup_policy: WaveRamStartupPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApuSnapshot {
    pub console_model: ConsoleModel,
    pub status: ApuStatus,
    pub powered: bool,
    pub nr50: u8,
    pub nr51: u8,
    pub channel_active_mask: u8,
    pub div_apu: u8,
    pub wave_ram: [u8; WAVE_RAM_LEN],
    pub wave_ram_startup_policy: WaveRamStartupPolicy,
}

impl Apu {
    pub fn new(console_model: ConsoleModel) -> Self {
        let wave_ram_startup_policy = WaveRamStartupPolicy::DeterministicZeroed;

        Self {
            console_model,
            status: ApuStatus::Ready,
            powered: false,
            nr10: 0,
            nr11: 0,
            nr12: 0,
            nr13: 0,
            nr14: 0,
            nr21: 0,
            nr22: 0,
            nr23: 0,
            nr24: 0,
            nr30: 0,
            nr31: 0,
            nr32: 0,
            nr33: 0,
            nr34: 0,
            nr41: 0,
            nr42: 0,
            nr43: 0,
            nr44: 0,
            nr50: 0,
            nr51: 0,
            channel_active_mask: 0,
            div_apu: 0,
            wave_ram: wave_ram_startup_policy.initial_bytes(),
            wave_ram_startup_policy,
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn status(&self) -> ApuStatus {
        self.status
    }

    pub fn read_register(&self, address: u16) -> u8 {
        match address {
            0xFF10 => self.nr10 | NR10_FORCED_HIGH_MASK,
            0xFF11 => (self.nr11 & 0xC0) | NR11_WRITE_ONLY_MASK,
            0xFF12 => self.nr12,
            0xFF13 => NR13_WRITE_ONLY_READ_VALUE,
            0xFF14 => (self.nr14 & NR14_READ_MASK) | NR14_FORCED_HIGH_MASK,
            0xFF15 => 0xFF,
            0xFF16 => (self.nr21 & 0xC0) | NR11_WRITE_ONLY_MASK,
            0xFF17 => self.nr22,
            0xFF18 => NR13_WRITE_ONLY_READ_VALUE,
            0xFF19 => (self.nr24 & NR14_READ_MASK) | NR14_FORCED_HIGH_MASK,
            0xFF1A => (self.nr30 & NR52_POWER_BIT) | NR30_FORCED_HIGH_MASK,
            0xFF1B => NR31_WRITE_ONLY_READ_VALUE,
            0xFF1C => (self.nr32 & NR32_READ_MASK) | NR32_FORCED_HIGH_MASK,
            0xFF1D => NR33_WRITE_ONLY_READ_VALUE,
            0xFF1E => (self.nr34 & NR14_READ_MASK) | NR14_FORCED_HIGH_MASK,
            0xFF1F => 0xFF,
            0xFF20 => NR41_WRITE_ONLY_READ_VALUE,
            0xFF21 => self.nr42,
            0xFF22 => self.nr43,
            0xFF23 => (self.nr44 & NR44_READ_MASK) | NR44_FORCED_HIGH_MASK,
            0xFF24 => self.nr50,
            0xFF25 => self.nr51,
            0xFF26 => self.read_nr52(),
            0xFF27..=0xFF2F => 0xFF,
            0xFF30..=0xFF3F => self.wave_ram[(address - 0xFF30) as usize],
            _ => 0xFF,
        }
    }

    pub fn write_register(&mut self, address: u16, value: u8) {
        if let Some(index) = self.wave_ram_index(address) {
            self.wave_ram[index] = value;
            return;
        }

        if address == 0xFF26 {
            self.write_nr52(value);
            return;
        }

        if !self.powered {
            return;
        }

        match address {
            0xFF10 => self.nr10 = value & 0x7F,
            0xFF11 => self.nr11 = value,
            0xFF12 => {
                self.nr12 = value;
                self.apply_dac_state_update(CHANNEL_ACTIVE_CH1, self.channel_1_dac_enabled());
            }
            0xFF13 => self.nr13 = value,
            0xFF14 => {
                self.nr14 = value & 0x47;
                if value & NR52_POWER_BIT != 0 {
                    self.trigger_channel(CHANNEL_ACTIVE_CH1, self.channel_1_dac_enabled());
                }
            }
            0xFF15 => {}
            0xFF16 => self.nr21 = value,
            0xFF17 => {
                self.nr22 = value;
                self.apply_dac_state_update(CHANNEL_ACTIVE_CH2, self.channel_2_dac_enabled());
            }
            0xFF18 => self.nr23 = value,
            0xFF19 => {
                self.nr24 = value & 0x47;
                if value & NR52_POWER_BIT != 0 {
                    self.trigger_channel(CHANNEL_ACTIVE_CH2, self.channel_2_dac_enabled());
                }
            }
            0xFF1A => {
                self.nr30 = value & NR52_POWER_BIT;
                self.apply_dac_state_update(CHANNEL_ACTIVE_CH3, self.channel_3_dac_enabled());
            }
            0xFF1B => self.nr31 = value,
            0xFF1C => self.nr32 = value & NR32_READ_MASK,
            0xFF1D => self.nr33 = value,
            0xFF1E => {
                self.nr34 = value & 0x47;
                if value & NR52_POWER_BIT != 0 {
                    self.trigger_channel(CHANNEL_ACTIVE_CH3, self.channel_3_dac_enabled());
                }
            }
            0xFF1F => {}
            0xFF20 => self.nr41 = value,
            0xFF21 => {
                self.nr42 = value;
                self.apply_dac_state_update(CHANNEL_ACTIVE_CH4, self.channel_4_dac_enabled());
            }
            0xFF22 => self.nr43 = value,
            0xFF23 => {
                self.nr44 = value & NR44_READ_MASK;
                if value & NR52_POWER_BIT != 0 {
                    self.trigger_channel(CHANNEL_ACTIVE_CH4, self.channel_4_dac_enabled());
                }
            }
            0xFF24 => self.nr50 = value,
            0xFF25 => self.nr51 = value,
            0xFF27..=0xFF3F => {}
            _ => {}
        }
    }

    pub fn apply_startup_state(&mut self, startup_state: ApuStartupState) {
        self.powered = startup_state.powered;
        self.nr10 = startup_state.nr10 & 0x7F;
        self.nr11 = startup_state.nr11;
        self.nr12 = startup_state.nr12;
        self.nr13 = startup_state.nr13;
        self.nr14 = startup_state.nr14 & 0x47;
        self.nr21 = startup_state.nr21;
        self.nr22 = startup_state.nr22;
        self.nr23 = startup_state.nr23;
        self.nr24 = startup_state.nr24 & 0x47;
        self.nr30 = startup_state.nr30 & NR52_POWER_BIT;
        self.nr31 = startup_state.nr31;
        self.nr32 = startup_state.nr32 & NR32_READ_MASK;
        self.nr33 = startup_state.nr33;
        self.nr34 = startup_state.nr34 & 0x47;
        self.nr41 = startup_state.nr41;
        self.nr42 = startup_state.nr42;
        self.nr43 = startup_state.nr43;
        self.nr44 = startup_state.nr44 & NR44_READ_MASK;
        self.nr50 = startup_state.nr50;
        self.nr51 = startup_state.nr51;
        self.channel_active_mask = if self.powered {
            startup_state.channel_active_mask & CHANNEL_ACTIVE_MASK
        } else {
            0
        };
        self.div_apu = startup_state.div_apu & 0x07;
        self.wave_ram_startup_policy = startup_state.wave_ram_startup_policy;
        self.wave_ram = startup_state.wave_ram_startup_policy.initial_bytes();
    }

    pub fn snapshot(&self) -> ApuSnapshot {
        ApuSnapshot {
            console_model: self.console_model,
            status: self.status,
            powered: self.powered,
            nr50: self.nr50,
            nr51: self.nr51,
            channel_active_mask: self.channel_active_mask,
            div_apu: self.div_apu,
            wave_ram: self.wave_ram,
            wave_ram_startup_policy: self.wave_ram_startup_policy,
        }
    }

    pub fn scheduler_trace_message(&self, context: &CycleContext) -> String {
        format!(
            "t_cycle={} phase={} console_model={:?} status={:?}",
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.status,
        )
    }

    fn read_nr52(&self) -> u8 {
        NR52_FORCED_HIGH_MASK
            | if self.powered { NR52_POWER_BIT } else { 0 }
            | self.channel_active_mask
    }

    fn write_nr52(&mut self, value: u8) {
        let next_powered = value & NR52_POWER_BIT != 0;

        match (self.powered, next_powered) {
            (true, false) => self.power_off(),
            (false, true) => self.powered = true,
            _ => {}
        }
    }

    fn power_off(&mut self) {
        self.powered = false;
        self.nr10 = 0;
        self.nr11 = 0;
        self.nr12 = 0;
        self.nr13 = 0;
        self.nr14 = 0;
        self.nr21 = 0;
        self.nr22 = 0;
        self.nr23 = 0;
        self.nr24 = 0;
        self.nr30 = 0;
        self.nr31 = 0;
        self.nr32 = 0;
        self.nr33 = 0;
        self.nr34 = 0;
        self.nr41 = 0;
        self.nr42 = 0;
        self.nr43 = 0;
        self.nr44 = 0;
        self.nr50 = 0;
        self.nr51 = 0;
        self.channel_active_mask = 0;
    }

    fn trigger_channel(&mut self, channel: u8, dac_enabled: bool) {
        if dac_enabled {
            self.channel_active_mask |= channel;
        }
    }

    fn apply_dac_state_update(&mut self, channel: u8, dac_enabled: bool) {
        if !dac_enabled {
            self.channel_active_mask &= !channel;
        }
    }

    fn channel_1_dac_enabled(&self) -> bool {
        self.nr12 & 0xF8 != 0
    }

    fn channel_2_dac_enabled(&self) -> bool {
        self.nr22 & 0xF8 != 0
    }

    fn channel_3_dac_enabled(&self) -> bool {
        self.nr30 & NR52_POWER_BIT != 0
    }

    fn channel_4_dac_enabled(&self) -> bool {
        self.nr42 & 0xF8 != 0
    }

    fn wave_ram_index(&self, address: u16) -> Option<usize> {
        match address {
            0xFF30..=0xFF3F => Some((address - 0xFF30) as usize),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nr52_tracks_channel_active_state_separately_from_dac_state() {
        let mut apu = Apu::new(ConsoleModel::Dmg);

        apu.write_register(0xFF26, 0x80);

        assert_eq!(apu.read_register(0xFF26), 0xF0);

        apu.write_register(0xFF12, 0xF3);
        assert_eq!(apu.read_register(0xFF26), 0xF0);

        apu.write_register(0xFF14, 0x80);
        assert_eq!(apu.read_register(0xFF26), 0xF1);

        apu.write_register(0xFF12, 0x00);
        assert_eq!(apu.read_register(0xFF26), 0xF0);
    }

    #[test]
    fn audio_register_readback_keeps_write_only_and_mixed_fields_explicit() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);

        apu.write_register(0xFF10, 0x00);
        apu.write_register(0xFF11, 0x00);
        apu.write_register(0xFF13, 0x12);
        apu.write_register(0xFF14, 0x40);
        apu.write_register(0xFF1C, 0x00);
        apu.write_register(0xFF20, 0x34);
        apu.write_register(0xFF23, 0x00);

        assert_eq!(apu.read_register(0xFF10), 0x80);
        assert_eq!(apu.read_register(0xFF11), 0x3F);
        assert_eq!(apu.read_register(0xFF13), 0xFF);
        assert_eq!(apu.read_register(0xFF14), 0xFF);
        assert_eq!(apu.read_register(0xFF1C), 0x9F);
        assert_eq!(apu.read_register(0xFF20), 0xFF);
        assert_eq!(apu.read_register(0xFF23), 0xBF);
        assert_eq!(apu.read_register(0xFF15), 0xFF);
    }

    #[test]
    fn nr52_power_off_clears_audio_registers_but_preserves_wave_ram() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);
        apu.write_register(0xFF12, 0xF3);
        apu.write_register(0xFF14, 0x80);
        apu.write_register(0xFF30, 0x12);
        apu.write_register(0xFF31, 0x34);

        apu.write_register(0xFF26, 0x00);

        assert_eq!(apu.read_register(0xFF26), 0x70);
        assert_eq!(apu.read_register(0xFF12), 0x00);
        assert_eq!(apu.read_register(0xFF14), 0xBF);
        assert_eq!(apu.read_register(0xFF30), 0x12);
        assert_eq!(apu.read_register(0xFF31), 0x34);

        apu.write_register(0xFF12, 0xF3);
        assert_eq!(apu.read_register(0xFF12), 0x00);
    }

    #[test]
    fn startup_state_recreates_the_published_post_boot_audio_snapshot() {
        let mut apu = Apu::new(ConsoleModel::Dmg);

        apu.apply_startup_state(ApuStartupState {
            powered: true,
            nr10: 0x00,
            nr11: 0x80,
            nr12: 0xF3,
            nr13: 0x00,
            nr14: 0x00,
            nr21: 0x00,
            nr22: 0x00,
            nr23: 0x00,
            nr24: 0x00,
            nr30: 0x00,
            nr31: 0x00,
            nr32: 0x00,
            nr33: 0x00,
            nr34: 0x00,
            nr41: 0x00,
            nr42: 0x00,
            nr43: 0x00,
            nr44: 0x00,
            nr50: 0x77,
            nr51: 0xF3,
            channel_active_mask: CHANNEL_ACTIVE_CH1,
            div_apu: 0,
            wave_ram_startup_policy: WaveRamStartupPolicy::DeterministicZeroed,
        });

        assert_eq!(apu.read_register(0xFF10), 0x80);
        assert_eq!(apu.read_register(0xFF11), 0xBF);
        assert_eq!(apu.read_register(0xFF12), 0xF3);
        assert_eq!(apu.read_register(0xFF13), 0xFF);
        assert_eq!(apu.read_register(0xFF14), 0xBF);
        assert_eq!(apu.read_register(0xFF16), 0x3F);
        assert_eq!(apu.read_register(0xFF17), 0x00);
        assert_eq!(apu.read_register(0xFF18), 0xFF);
        assert_eq!(apu.read_register(0xFF19), 0xBF);
        assert_eq!(apu.read_register(0xFF1A), 0x7F);
        assert_eq!(apu.read_register(0xFF1B), 0xFF);
        assert_eq!(apu.read_register(0xFF1C), 0x9F);
        assert_eq!(apu.read_register(0xFF1D), 0xFF);
        assert_eq!(apu.read_register(0xFF1E), 0xBF);
        assert_eq!(apu.read_register(0xFF20), 0xFF);
        assert_eq!(apu.read_register(0xFF21), 0x00);
        assert_eq!(apu.read_register(0xFF22), 0x00);
        assert_eq!(apu.read_register(0xFF23), 0xBF);
        assert_eq!(apu.read_register(0xFF24), 0x77);
        assert_eq!(apu.read_register(0xFF25), 0xF3);
        assert_eq!(apu.read_register(0xFF26), 0xF1);
        assert_eq!(apu.read_register(0xFF30), 0x00);

        let snapshot = apu.snapshot();
        assert_eq!(snapshot.div_apu, 0);
        assert_eq!(
            snapshot.wave_ram_startup_policy,
            WaveRamStartupPolicy::DeterministicZeroed
        );
    }

    #[test]
    fn channel_2_3_and_4_register_paths_keep_dac_enable_and_trigger_distinct() {
        let mut apu = Apu::new(ConsoleModel::Dmg);
        apu.write_register(0xFF26, 0x80);

        apu.write_register(0xFF16, 0xC7);
        apu.write_register(0xFF17, 0x00);
        apu.write_register(0xFF18, 0x12);
        apu.write_register(0xFF19, 0x80);
        assert_eq!(apu.read_register(0xFF26), 0xF0);

        apu.write_register(0xFF17, 0xF3);
        apu.write_register(0xFF19, 0x80);
        assert_eq!(apu.read_register(0xFF26), 0xF2);

        apu.write_register(0xFF1A, 0x80);
        apu.write_register(0xFF1B, 0x55);
        apu.write_register(0xFF1D, 0x34);
        apu.write_register(0xFF1E, 0x80);
        assert_eq!(apu.read_register(0xFF26), 0xF6);

        apu.write_register(0xFF21, 0xF3);
        apu.write_register(0xFF22, 0x20);
        apu.write_register(0xFF23, 0x80);
        assert_eq!(apu.read_register(0xFF26), 0xFE);

        apu.write_register(0xFF24, 0x77);
        apu.write_register(0xFF25, 0xF3);
        apu.write_register(0xFF15, 0x34);
        apu.write_register(0xFF1F, 0x12);
        apu.write_register(0xFF27, 0x56);
        apu.write_register(0xFF40, 0x78);

        assert_eq!(apu.read_register(0xFF16), 0xFF);
        assert_eq!(apu.read_register(0xFF17), 0xF3);
        assert_eq!(apu.read_register(0xFF18), 0xFF);
        assert_eq!(apu.read_register(0xFF19), 0xBF);
        assert_eq!(apu.read_register(0xFF1A), 0xFF);
        assert_eq!(apu.read_register(0xFF1B), 0xFF);
        assert_eq!(apu.read_register(0xFF1D), 0xFF);
        assert_eq!(apu.read_register(0xFF1E), 0xBF);
        assert_eq!(apu.read_register(0xFF21), 0xF3);
        assert_eq!(apu.read_register(0xFF22), 0x20);
        assert_eq!(apu.read_register(0xFF24), 0x77);
        assert_eq!(apu.read_register(0xFF25), 0xF3);
        assert_eq!(apu.read_register(0xFF1F), 0xFF);
        assert_eq!(apu.read_register(0xFF40), 0xFF);
    }

    #[test]
    fn powered_off_startup_state_clears_channel_mask_and_scheduler_trace_is_stable() {
        let mut apu = Apu::new(ConsoleModel::Dmg);

        apu.apply_startup_state(ApuStartupState {
            powered: false,
            nr10: 0x7F,
            nr11: 0xFF,
            nr12: 0xF3,
            nr13: 0x12,
            nr14: 0xFF,
            nr21: 0xFF,
            nr22: 0xF3,
            nr23: 0x34,
            nr24: 0xFF,
            nr30: 0xFF,
            nr31: 0x56,
            nr32: 0xFF,
            nr33: 0x78,
            nr34: 0xFF,
            nr41: 0x9A,
            nr42: 0xF3,
            nr43: 0xBC,
            nr44: 0xFF,
            nr50: 0x77,
            nr51: 0xF3,
            channel_active_mask: 0x0F,
            div_apu: 0xFF,
            wave_ram_startup_policy: WaveRamStartupPolicy::DeterministicZeroed,
        });

        assert_eq!(apu.read_register(0xFF26), 0x70);

        let context = CycleContext::for_cycle(crate::scheduler::TCycle::ZERO);
        let trace = apu.scheduler_trace_message(&context);
        assert_eq!(
            trace,
            "t_cycle=0 phase=external_event_ingress console_model=Dmg status=Ready"
        );
    }
}
