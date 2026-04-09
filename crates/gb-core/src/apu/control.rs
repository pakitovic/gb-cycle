use super::common::{
    CHANNEL_ACTIVE_CH1, CHANNEL_ACTIVE_CH2, CHANNEL_ACTIVE_CH3, CHANNEL_ACTIVE_CH4,
    NR52_FORCED_HIGH_MASK, NR52_MASTER_POWER_BIT,
};
use super::output::OutputPathState;
use super::{Apu, ApuOutputSnapshot, WaveRamStartupPolicy};

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
pub struct ApuRegisterWriteObservation {
    pub address: u16,
    pub value: u8,
    pub before: ApuRegisterWriteState,
    pub after: ApuRegisterWriteState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApuRegisterWriteState {
    pub powered: bool,
    pub nr50: u8,
    pub nr51: u8,
    pub nr52: u8,
    pub channel_active_mask: u8,
    pub channel_dac_mask: u8,
    pub output: ApuOutputSnapshot,
}

impl Apu {
    pub fn apply_startup_state(&mut self, startup_state: ApuStartupState) {
        self.last_register_write = None;
        self.wave_ram_startup_policy = startup_state.wave_ram_startup_policy;
        self.channel_3
            .initialize_wave_ram(startup_state.wave_ram_startup_policy.initial_bytes());
        self.output_path = OutputPathState::new(self.console_model);

        if startup_state.powered {
            self.master.powered = true;
            self.master.nr50 = startup_state.nr50;
            self.master.nr51 = startup_state.nr51;
            self.channel_1.apply_powered_startup(
                startup_state.nr10,
                startup_state.nr11,
                startup_state.nr12,
                startup_state.nr13,
                startup_state.nr14,
                startup_state.channel_active_mask & CHANNEL_ACTIVE_CH1 != 0,
            );
            self.channel_2.apply_powered_startup(
                startup_state.nr21,
                startup_state.nr22,
                startup_state.nr23,
                startup_state.nr24,
                startup_state.channel_active_mask & CHANNEL_ACTIVE_CH2 != 0,
            );
            self.channel_3.apply_powered_startup(
                startup_state.nr30,
                startup_state.nr31,
                startup_state.nr32,
                startup_state.nr33,
                startup_state.nr34,
                startup_state.channel_active_mask & CHANNEL_ACTIVE_CH3 != 0,
            );
            self.channel_4.apply_powered_startup(
                startup_state.nr41,
                startup_state.nr42,
                startup_state.nr43,
                startup_state.nr44,
                startup_state.channel_active_mask & CHANNEL_ACTIVE_CH4 != 0,
            );
        } else {
            self.power_off();
        }

        self.frame_sequencer
            .apply_startup_phase(startup_state.div_apu);
        self.preview_output_path();
    }

    pub(in crate::apu) fn read_nr52(&self) -> u8 {
        NR52_FORCED_HIGH_MASK
            | if self.master.powered {
                NR52_MASTER_POWER_BIT
            } else {
                0
            }
            | self.channel_active_mask()
    }

    pub(in crate::apu) fn write_nr52(&mut self, value: u8) {
        let next_powered = value & NR52_MASTER_POWER_BIT != 0;

        match (self.master.powered, next_powered) {
            (true, false) => self.power_off(),
            (false, true) => {
                self.master.powered = true;
                self.frame_sequencer.apply_startup_phase(0);
                self.channel_1.pulse.mark_powered_on();
                self.channel_2.pulse.mark_powered_on();
            }
            _ => {}
        }
    }

    pub(in crate::apu) fn should_observe_register_write(address: u16) -> bool {
        (0xFF10..=0xFF26).contains(&address)
    }

    pub(in crate::apu) fn register_write_state(&self) -> ApuRegisterWriteState {
        ApuRegisterWriteState {
            powered: self.master.powered,
            nr50: self.master.nr50,
            nr51: self.master.nr51,
            nr52: self.read_nr52(),
            channel_active_mask: self.channel_active_mask(),
            channel_dac_mask: self.channel_dac_mask(),
            output: self.output_snapshot(),
        }
    }

    pub(in crate::apu) fn record_register_write_observation(
        &mut self,
        address: u16,
        value: u8,
        before: Option<ApuRegisterWriteState>,
    ) {
        let Some(before) = before else {
            return;
        };

        self.last_register_write = Some(ApuRegisterWriteObservation {
            address,
            value,
            before,
            after: self.register_write_state(),
        });
    }

    fn power_off(&mut self) {
        self.master.powered = false;
        self.master.nr50 = 0;
        self.master.nr51 = 0;
        self.channel_1.power_off_registers(self.console_model);
        self.channel_2.power_off_registers(self.console_model);
        self.channel_3.power_off_registers(self.console_model);
        self.channel_4.power_off_registers(self.console_model);
    }
}
