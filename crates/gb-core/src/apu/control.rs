use super::common::{NR52_FORCED_HIGH_MASK, NR52_MASTER_POWER_BIT};
use super::output::OutputPathState;
use super::{Apu, ApuOutputSnapshot, WaveRamStartupPolicy, div_apu_phase_from_system_counter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApuRegisterWriteObservation {
    pub address: u16,
    pub value: u8,
    pub before: ApuRegisterWriteState,
    pub after: ApuRegisterWriteState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
        self.output_path = OutputPathState::new(self.console_model);

        if startup_state.powered {
            self.master.powered = true;
            self.master.nr50 = startup_state.nr50;
            self.master.nr51 = startup_state.nr51;
        } else {
            self.master.powered = false;
            self.master.nr50 = 0;
            self.master.nr51 = 0;
        }
        self.channels
            .apply_startup(self.console_model, startup_state);

        self.frame_sequencer
            .apply_startup_phase(startup_state.div_apu);
        self.preview_output_path();
    }

    pub(crate) fn apply_div_apu_startup_phase_from_system_counter(&mut self, system_counter: u16) {
        self.frame_sequencer
            .apply_startup_phase(div_apu_phase_from_system_counter(system_counter));
        self.preview_output_path();
    }

    pub(in crate::apu) fn read_nr52(&self) -> u8 {
        self.read_nr52_from_channel_output(self.channel_output_state())
    }

    pub(in crate::apu) fn read_nr52_from_channel_output(
        &self,
        channel_output: super::ChannelOutputState,
    ) -> u8 {
        NR52_FORCED_HIGH_MASK
            | if self.master.powered {
                NR52_MASTER_POWER_BIT
            } else {
                0
            }
            | channel_output.active_mask
    }

    pub(in crate::apu) fn write_nr52(&mut self, value: u8) {
        let next_powered = value & NR52_MASTER_POWER_BIT != 0;

        match (self.master.powered, next_powered) {
            (true, false) => self.power_off(),
            (false, true) => {
                self.master.powered = true;
                self.frame_sequencer.apply_startup_phase(0);
                self.channels.mark_powered_on();
            }
            _ => {}
        }
    }

    pub(in crate::apu) fn register_write_state(&self) -> ApuRegisterWriteState {
        let output_resolution = self.resolve_output_state();

        ApuRegisterWriteState {
            powered: self.master.powered,
            nr50: self.master.nr50,
            nr51: self.master.nr51,
            nr52: self.read_nr52_from_channel_output(output_resolution.channel_output),
            channel_active_mask: output_resolution.channel_output.active_mask,
            channel_dac_mask: output_resolution.channel_output.dac_mask,
            output: output_resolution.snapshot(&self.output_path),
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
        self.channels.power_off_registers(self.console_model);
    }
}
