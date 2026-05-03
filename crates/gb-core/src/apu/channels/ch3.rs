use crate::model::ConsoleModel;

use super::super::common::{
    ChannelRuntimeState, ExtraLengthClockingContext, LENGTH_ENABLE_BIT, NR14_FORCED_HIGH_MASK,
    NR14_READ_MASK, NR30_DAC_POWER_BIT, NR30_FORCED_HIGH_MASK, NR31_WRITE_ONLY_READ_VALUE,
    NR32_FORCED_HIGH_MASK, NR32_READ_MASK, NR33_WRITE_ONLY_READ_VALUE, NRX4_WRITABLE_MASK,
    WAVE_DMG_RETRIGGER_CORRUPTION_WINDOW_T_CYCLES, WAVE_HIGH_NIBBLE_SHIFT,
    WAVE_LENGTH_COUNTER_RELOAD, WAVE_OUTPUT_LEVEL_HALF_SHIFT, WAVE_OUTPUT_LEVEL_QUARTER_SHIFT,
    WAVE_OUTPUT_LEVEL_SHIFT, WAVE_RAM_INACCESSIBLE_READ_VALUE, WAVE_RAM_LEN,
    WAVE_RETRIGGER_CORRUPTION_BLOCK_LEN, WAVE_RETRIGGER_CORRUPTION_BLOCK_MASK,
    WAVE_SAMPLE_BYTE_SHIFT, WAVE_SAMPLE_COUNT, WAVE_SAMPLE_LOW_BIT, WAVE_SAMPLE_NIBBLE_MASK,
    WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES, apply_extra_length_clocking_u16, begin_nrx4_write,
    clock_length_counter_u16, pulse_period_from_registers, wave_length_counter_from_load,
    wave_ram_mmio_policy, wave_timer_reload,
};
use super::super::registers::Channel3Register;

#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub(in crate::apu) struct Channel3State {
    nr30: u8,
    nr31: u8,
    nr32: u8,
    nr33: u8,
    nr34: u8,
    pub(in crate::apu) wave_ram: [u8; WAVE_RAM_LEN],
    pub(in crate::apu) runtime: ChannelRuntimeState,
    pub(in crate::apu) sample_index: u8,
    pub(in crate::apu) sample_buffer: u8,
    pub(in crate::apu) period_timer: u16,
    pub(in crate::apu) length_counter: u16,
    length_enabled: bool,
    wave_ram_access_window_byte_index: Option<u8>,
}

impl Channel3State {
    pub(in crate::apu) fn read_register(&self, register: Channel3Register) -> u8 {
        match register {
            Channel3Register::Nr30 => self.read_nr30(),
            Channel3Register::Nr31 => NR31_WRITE_ONLY_READ_VALUE,
            Channel3Register::Nr32 => self.read_nr32(),
            Channel3Register::Nr33 => NR33_WRITE_ONLY_READ_VALUE,
            Channel3Register::Nr34 => self.read_nr34(),
        }
    }

    pub(in crate::apu) fn write_register(
        &mut self,
        register: Channel3Register,
        value: u8,
        console_model: ConsoleModel,
        next_frame_sequencer_step: u8,
    ) {
        match register {
            Channel3Register::Nr30 => self.write_nr30(value),
            Channel3Register::Nr31 => self.write_nr31(value),
            Channel3Register::Nr32 => self.write_nr32(value),
            Channel3Register::Nr33 => self.write_nr33(value),
            Channel3Register::Nr34 => {
                self.write_nr34(value, console_model, next_frame_sequencer_step)
            }
        }
    }

    pub(in crate::apu) fn write_powered_off_register(
        &mut self,
        register: Channel3Register,
        value: u8,
        console_model: ConsoleModel,
    ) {
        if !console_model.is_dmg_family() {
            return;
        }

        if matches!(register, Channel3Register::Nr31) {
            self.write_length_while_powered_off(value);
        }
    }

    fn read_nr30(&self) -> u8 {
        (self.nr30 & NR30_DAC_POWER_BIT) | NR30_FORCED_HIGH_MASK
    }

    fn read_nr32(&self) -> u8 {
        (self.nr32 & NR32_READ_MASK) | NR32_FORCED_HIGH_MASK
    }

    fn read_nr34(&self) -> u8 {
        (self.nr34 & NR14_READ_MASK) | NR14_FORCED_HIGH_MASK
    }

    fn write_nr30(&mut self, value: u8) {
        self.nr30 = value & NR30_DAC_POWER_BIT;
        self.runtime.set_dac_enabled(self.derived_dac_enabled());
    }

    fn write_nr31(&mut self, value: u8) {
        self.nr31 = value;
        self.length_counter = wave_length_counter_from_load(value);
    }

    fn write_nr32(&mut self, value: u8) {
        self.nr32 = value & NR32_READ_MASK;
    }

    fn write_nr33(&mut self, value: u8) {
        self.nr33 = value;
    }

    pub(in crate::apu) fn write_nr34(
        &mut self,
        value: u8,
        console_model: ConsoleModel,
        next_frame_sequencer_step: u8,
    ) {
        let mut write_plan = begin_nrx4_write(
            &mut self.nr34,
            value,
            NRX4_WRITABLE_MASK,
            next_frame_sequencer_step,
            self.length_enabled,
        );

        if write_plan.context.trigger {
            write_plan.observe_trigger_reloaded_zero_length(self.trigger(console_model));
        }

        self.length_enabled = write_plan.context.length_enabled;
        self.apply_extra_length_clocking_on_enable(
            console_model,
            write_plan.was_length_enabled,
            write_plan.context.next_step_clocks_length,
            write_plan.context.trigger,
            write_plan.trigger_reloaded_zero_length,
        );
    }

    pub(in crate::apu) fn apply_powered_startup(
        &mut self,
        nr30: u8,
        nr31: u8,
        nr32: u8,
        nr33: u8,
        nr34: u8,
        active: bool,
    ) {
        self.nr30 = nr30 & NR30_DAC_POWER_BIT;
        self.nr31 = nr31;
        self.nr32 = nr32 & NR32_READ_MASK;
        self.nr33 = nr33;
        self.nr34 = nr34 & NRX4_WRITABLE_MASK;
        self.sample_index = 0;
        self.sample_buffer = 0;
        self.period_timer = wave_timer_reload(self.period_value());
        self.length_counter = wave_length_counter_from_load(self.nr31);
        self.length_enabled = self.nr34 & LENGTH_ENABLE_BIT != 0;
        self.wave_ram_access_window_byte_index = None;
        self.runtime.clear();
        self.runtime.set_dac_enabled(self.derived_dac_enabled());
        self.runtime.set_active_from_startup(active);
    }

    fn clear_registers(&mut self) {
        self.nr30 = 0;
        self.nr31 = 0;
        self.nr32 = 0;
        self.nr33 = 0;
        self.nr34 = 0;
        self.sample_index = 0;
        self.sample_buffer = 0;
        self.period_timer = 0;
        self.length_counter = 0;
        self.length_enabled = false;
        self.wave_ram_access_window_byte_index = None;
        self.runtime.clear();
    }

    pub(in crate::apu) fn write_length_while_powered_off(&mut self, value: u8) {
        self.length_counter = wave_length_counter_from_load(value);
    }

    pub(in crate::apu) fn power_off_registers(&mut self, console_model: ConsoleModel) {
        let preserved_length = if console_model.is_dmg_family() {
            self.length_counter
        } else {
            0
        };
        self.clear_registers();
        self.length_counter = preserved_length;
    }

    fn derived_dac_enabled(&self) -> bool {
        self.nr30 & NR30_DAC_POWER_BIT != 0
    }

    pub(in crate::apu) fn period_value(&self) -> u16 {
        pulse_period_from_registers(self.nr33, self.nr34)
    }

    pub(in crate::apu) fn runtime_state(&self) -> ChannelRuntimeState {
        self.runtime
    }

    pub(in crate::apu) fn wave_ram_snapshot(&self) -> [u8; WAVE_RAM_LEN] {
        self.wave_ram
    }

    pub(in crate::apu) fn begin_t_cycle(&mut self) {
        self.wave_ram_access_window_byte_index = None;
    }

    fn apply_extra_length_clocking_on_enable(
        &mut self,
        console_model: ConsoleModel,
        was_length_enabled: bool,
        next_step_clocks_length: bool,
        trigger: bool,
        trigger_reloaded_zero_length: bool,
    ) {
        apply_extra_length_clocking_u16(
            ExtraLengthClockingContext {
                console_model,
                length_enabled: self.length_enabled,
                was_length_enabled,
                next_step_clocks_length,
                trigger,
                trigger_reloaded_zero_length,
            },
            &mut self.length_counter,
            WAVE_LENGTH_COUNTER_RELOAD,
            &mut self.runtime.active,
        );
    }

    fn trigger(&mut self, console_model: ConsoleModel) -> bool {
        self.apply_dmg_retrigger_wave_ram_corruption(console_model);
        let reloaded_zero_length = self.length_counter == 0;

        if self.length_counter == 0 {
            self.length_counter = WAVE_LENGTH_COUNTER_RELOAD;
            self.length_enabled = false;
        }

        self.period_timer =
            wave_timer_reload(self.period_value()) + WAVE_TRIGGER_STARTUP_DELAY_T_CYCLES;
        self.sample_index = 0;
        self.runtime.trigger();
        reloaded_zero_length
    }

    pub(in crate::apu) fn tick_fast_timer(&mut self) {
        if self.period_timer > 0 {
            self.period_timer -= 1;
        }

        if self.period_timer == 0 {
            self.period_timer = wave_timer_reload(self.period_value());
            self.advance_sample();
        }
    }

    pub(in crate::apu) fn clock_length(&mut self) {
        clock_length_counter_u16(
            self.length_enabled,
            &mut self.length_counter,
            &mut self.runtime.active,
        );
    }

    pub(in crate::apu) fn read_wave_ram(&self, console_model: ConsoleModel, index: usize) -> u8 {
        if let Some(active_wave_ram_byte_index) =
            self.active_wave_ram_access_byte_index(console_model)
        {
            return self.wave_ram[active_wave_ram_byte_index];
        }

        if self.runtime.active {
            match wave_ram_mmio_policy(console_model) {
                super::super::common::WaveRamMmioPolicy::DmgCurrentByteDuringFetchOnly => {
                    return WAVE_RAM_INACCESSIBLE_READ_VALUE;
                }
                super::super::common::WaveRamMmioPolicy::CgbCurrentByteWhileActive => {
                    return self.wave_ram[self.current_wave_ram_byte_index()];
                }
            }
        }

        self.wave_ram[index]
    }

    pub(in crate::apu) fn write_wave_ram(
        &mut self,
        console_model: ConsoleModel,
        index: usize,
        value: u8,
    ) {
        if let Some(active_wave_ram_byte_index) =
            self.active_wave_ram_access_byte_index(console_model)
        {
            self.wave_ram[active_wave_ram_byte_index] = value;
            return;
        }

        if self.runtime.active {
            match wave_ram_mmio_policy(console_model) {
                super::super::common::WaveRamMmioPolicy::DmgCurrentByteDuringFetchOnly => return,
                super::super::common::WaveRamMmioPolicy::CgbCurrentByteWhileActive => {
                    self.wave_ram[self.current_wave_ram_byte_index()] = value;
                    return;
                }
            }
        }

        self.wave_ram[index] = value;
    }

    pub(in crate::apu) fn initialize_wave_ram(&mut self, wave_ram: [u8; WAVE_RAM_LEN]) {
        self.wave_ram = wave_ram;
    }

    fn active_wave_ram_access_byte_index(&self, console_model: ConsoleModel) -> Option<usize> {
        if self.runtime.active && console_model.is_dmg_family() {
            return self
                .wave_ram_access_window_byte_index
                .map(|byte_index| byte_index as usize);
        }

        None
    }

    fn current_wave_ram_byte_index(&self) -> usize {
        ((self.sample_index >> WAVE_SAMPLE_BYTE_SHIFT) as usize) % WAVE_RAM_LEN
    }

    fn advance_sample(&mut self) {
        self.sample_index = (self.sample_index + 1) % WAVE_SAMPLE_COUNT;
        let current_wave_ram_byte_index = self.current_wave_ram_byte_index() as u8;
        self.wave_ram_access_window_byte_index = Some(current_wave_ram_byte_index);
        self.sample_buffer = self.wave_sample(self.sample_index);
    }

    fn apply_dmg_retrigger_wave_ram_corruption(&mut self, console_model: ConsoleModel) {
        if !console_model.is_dmg_family()
            || !self.runtime.active
            || self.period_timer != WAVE_DMG_RETRIGGER_CORRUPTION_WINDOW_T_CYCLES
        {
            return;
        }

        let current_byte_index =
            (((self.sample_index as usize) + 1) >> WAVE_SAMPLE_BYTE_SHIFT) % WAVE_RAM_LEN;

        if current_byte_index < WAVE_RETRIGGER_CORRUPTION_BLOCK_LEN {
            self.wave_ram[0] = self.wave_ram[current_byte_index];
            return;
        }

        let block_start = current_byte_index & !WAVE_RETRIGGER_CORRUPTION_BLOCK_MASK;
        let aligned_block = [
            self.wave_ram[block_start],
            self.wave_ram[block_start + 1],
            self.wave_ram[block_start + 2],
            self.wave_ram[block_start + 3],
        ];
        self.wave_ram[..aligned_block.len()].copy_from_slice(&aligned_block);
    }

    fn wave_sample(&self, sample_index: u8) -> u8 {
        let byte =
            self.wave_ram[((sample_index >> WAVE_SAMPLE_BYTE_SHIFT) as usize) % WAVE_RAM_LEN];
        if sample_index & WAVE_SAMPLE_LOW_BIT == 0 {
            byte >> WAVE_HIGH_NIBBLE_SHIFT
        } else {
            byte & WAVE_SAMPLE_NIBBLE_MASK
        }
    }

    pub(in crate::apu) fn current_digital_output(&self) -> u8 {
        if !self.runtime.active {
            return 0;
        }

        match (self.nr32 & NR32_READ_MASK) >> WAVE_OUTPUT_LEVEL_SHIFT {
            0 => 0,
            1 => self.sample_buffer,
            2 => self.sample_buffer >> WAVE_OUTPUT_LEVEL_HALF_SHIFT,
            3 => self.sample_buffer >> WAVE_OUTPUT_LEVEL_QUARTER_SHIFT,
            _ => unreachable!(),
        }
    }
}
