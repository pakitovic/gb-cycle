mod ch1;
mod ch2;
mod ch3;
mod ch4;
mod ch4_live_write;
mod pulse;

use crate::model::ConsoleModel;

use super::common::{
    CHANNEL_ACTIVE_CH1, CHANNEL_ACTIVE_CH2, CHANNEL_ACTIVE_CH3, CHANNEL_ACTIVE_CH4,
    CHANNEL_ACTIVE_MASK, CHANNEL_COUNT, CHANNEL_MASKS, ChannelRuntimeState, WAVE_RAM_LEN,
};
use super::control::ApuStartupState;

pub(super) use ch1::Channel1State;
pub(super) use ch2::Channel2State;
pub(super) use ch3::Channel3State;
pub(super) use ch4::Channel4State;

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct ApuChannels {
    pub(in crate::apu) channel_1: Channel1State,
    pub(in crate::apu) channel_2: Channel2State,
    pub(in crate::apu) channel_3: Channel3State,
    pub(in crate::apu) channel_4: Channel4State,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct ChannelOutputState {
    pub(super) active_mask: u8,
    pub(super) dac_mask: u8,
    pub(super) digital_outputs: [u8; CHANNEL_COUNT],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct ChannelResolvedOutput {
    runtime: ChannelRuntimeState,
    digital_output: u8,
}

impl ApuChannels {
    pub(super) fn begin_t_cycle(&mut self) {
        self.channel_3.begin_t_cycle();
    }

    pub(super) fn tick_fast_timers(
        &mut self,
        console_model: ConsoleModel,
        clock_generation_timers: bool,
        apu_clock: u8,
        t_cycle_phase: u8,
    ) {
        self.channel_1.tick_fast_timer_with_clock_gate(
            console_model,
            clock_generation_timers,
            apu_clock,
            t_cycle_phase,
        );
        self.channel_2
            .tick_fast_timer_with_clock_gate(clock_generation_timers);
        if clock_generation_timers {
            self.channel_3.tick_fast_timer();
            self.channel_4.tick_fast_timer();
        }
    }

    pub(super) fn tick_powered_off_timebase(&mut self) {
        /*
         SameBoy keeps CH4's alignment phase advancing even while NR52 is powered off. The noise
         hidden counter itself still stays idle because the start path and active/background flags
         remain off; only the timebase that future DMG delayed starts observe keeps moving. The
         caller owns the CGB speed-domain gate so powered-off and powered-on fast APU timebases
         stay in the same wall-clock domain.
        */
        self.channel_4.tick_alignment_phase_only();
    }

    pub(super) fn clock_length_all(&mut self) {
        self.channel_1.clock_length();
        self.channel_2.clock_length();
        self.channel_3.clock_length();
        self.channel_4.clock_length();
    }

    pub(super) fn clock_envelope_all(&mut self) {
        self.channel_1.clock_envelope();
        self.channel_2.clock_envelope();
        self.channel_4.clock_envelope();
    }

    pub(super) fn clock_cgb_live_write_pending_even_envelope_all(&mut self) {
        self.channel_1
            .clock_cgb_live_write_pending_even_envelope_tick();
        self.channel_2
            .clock_cgb_live_write_pending_even_envelope_tick();
        self.channel_4
            .clock_cgb_live_write_pending_even_envelope_tick();
    }

    pub(super) fn clock_sweep_ch1(&mut self, console_model: ConsoleModel) {
        self.channel_1.clock_sweep(console_model);
    }

    pub(super) fn apply_startup(
        &mut self,
        console_model: ConsoleModel,
        startup_state: ApuStartupState,
    ) {
        self.initialize_wave_ram(startup_state.wave_ram_startup_policy.initial_bytes());

        if !startup_state.powered {
            self.power_off_registers(console_model);
            return;
        }

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
    }

    pub(super) fn mark_powered_on(&mut self) {
        self.channel_1.mark_powered_on();
        self.channel_2.mark_powered_on();
        self.channel_4.mark_powered_on();
    }

    pub(super) fn power_off_registers(&mut self, console_model: ConsoleModel) {
        self.channel_1.power_off_registers(console_model);
        self.channel_2.power_off_registers(console_model);
        self.channel_3.power_off_registers(console_model);
        self.channel_4.power_off_registers(console_model);
    }

    pub(super) fn output_state(&self) -> ChannelOutputState {
        let resolved = [
            ChannelResolvedOutput {
                runtime: self.channel_1.runtime_state(),
                digital_output: self.channel_1.current_digital_output(),
            },
            ChannelResolvedOutput {
                runtime: self.channel_2.runtime_state(),
                digital_output: self.channel_2.current_digital_output(),
            },
            ChannelResolvedOutput {
                runtime: self.channel_3.runtime_state(),
                digital_output: self.channel_3.current_digital_output(),
            },
            ChannelResolvedOutput {
                runtime: self.channel_4.runtime_state(),
                digital_output: self.channel_4.current_digital_output(),
            },
        ];

        let mut active_mask = 0;
        let mut dac_mask = 0;
        let mut digital_outputs = [0; CHANNEL_COUNT];

        for (index, resolved_channel) in resolved.into_iter().enumerate() {
            let mask = CHANNEL_MASKS[index];
            if resolved_channel.runtime.active {
                active_mask |= mask;
            }
            if resolved_channel.runtime.dac_enabled {
                dac_mask |= mask;
            }
            digital_outputs[index] = resolved_channel.digital_output;
        }

        ChannelOutputState {
            active_mask: active_mask & CHANNEL_ACTIVE_MASK,
            dac_mask: dac_mask & CHANNEL_ACTIVE_MASK,
            digital_outputs,
        }
    }

    pub(super) fn wave_ram_snapshot(&self) -> [u8; WAVE_RAM_LEN] {
        self.channel_3.wave_ram_snapshot()
    }

    pub(super) fn initialize_wave_ram(&mut self, wave_ram: [u8; WAVE_RAM_LEN]) {
        self.channel_3.initialize_wave_ram(wave_ram);
    }
}
