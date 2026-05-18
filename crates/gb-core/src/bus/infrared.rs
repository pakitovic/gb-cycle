#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CgbInfraredStatus {
    pub rp_latch: u8,
    pub emitter_on: bool,
    pub read_enabled: bool,
    pub external_optical_input: bool,
    pub optical_input_active: bool,
    pub sensor_counter: u32,
    pub sensor_warmed: bool,
    pub effective_signal_detected: bool,
    pub signal_visible_to_rp: bool,
}

impl CgbInfraredStatus {
    pub const fn receive_ready(self) -> bool {
        self.read_enabled
            && self.sensor_warmed
            && !self.emitter_on
            && !self.optical_input_active
            && !self.effective_signal_detected
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct CgbInfraredState {
    rp_latch: u8,
    external_optical_input: bool,
    sensor_counter: u32,
    effective_signal_detected: bool,
}

impl CgbInfraredState {
    const RP_EMITTER_BIT: u8 = 0x01;
    const RP_SIGNAL_BIT: u8 = 0x02;
    const RP_WRITABLE_MASK: u8 = 0xC1;
    const RP_READ_ENABLE_MASK: u8 = 0xC0;
    const RP_UNUSED_READ_MASK: u8 = 0x3C;
    const IR_WARMUP_T_CYCLES: u32 = 19_900;
    // Shonumi's hardware notes describe a delay after re-enabling RP reads, but not when read
    // enable stays asserted; a warmed sensor must therefore report short CGB-to-CGB pulses without
    // an additional post-ready threshold.
    const IR_THRESHOLD_T_CYCLES: u32 = 0;
    const IR_DECAY_T_CYCLES: u32 = 31_500;
    const IR_MAX_T_CYCLES: u32 =
        Self::IR_WARMUP_T_CYCLES + Self::IR_THRESHOLD_T_CYCLES * 2 + Self::IR_DECAY_T_CYCLES + 268;

    pub(crate) const fn new() -> Self {
        Self {
            rp_latch: 0,
            external_optical_input: false,
            sensor_counter: 0,
            effective_signal_detected: false,
        }
    }

    pub(crate) const fn read_rp(self) -> u8 {
        let signal_bit = if self.signal_visible_to_rp() {
            0
        } else {
            Self::RP_SIGNAL_BIT
        };
        Self::RP_UNUSED_READ_MASK | signal_bit | self.rp_latch
    }

    pub(crate) fn write_rp(&mut self, value: u8) {
        self.rp_latch = value & Self::RP_WRITABLE_MASK;
    }

    pub(crate) fn set_external_optical_input(&mut self, active: bool) {
        self.external_optical_input = active;
    }

    pub(crate) const fn emitter_on(self) -> bool {
        self.rp_latch & Self::RP_EMITTER_BIT != 0
    }

    pub(crate) const fn status(self) -> CgbInfraredStatus {
        CgbInfraredStatus {
            rp_latch: self.rp_latch,
            emitter_on: self.emitter_on(),
            read_enabled: self.read_enabled(),
            external_optical_input: self.external_optical_input,
            optical_input_active: self.optical_input_active(),
            sensor_counter: self.sensor_counter,
            sensor_warmed: self.sensor_counter >= Self::IR_WARMUP_T_CYCLES,
            effective_signal_detected: self.effective_signal_detected,
            signal_visible_to_rp: self.signal_visible_to_rp(),
        }
    }

    #[cfg(test)]
    pub(crate) const fn effective_signal_detected(self) -> bool {
        self.effective_signal_detected
    }

    pub(crate) fn tick_t_cycle(&mut self) {
        let sensing = self.read_enabled();
        if sensing && self.optical_input_active() {
            self.sensor_counter = self
                .sensor_counter
                .saturating_add(1)
                .min(Self::IR_MAX_T_CYCLES);
            self.effective_signal_detected = self.sensor_counter
                >= Self::IR_WARMUP_T_CYCLES + Self::IR_THRESHOLD_T_CYCLES
                && self.sensor_counter
                    <= Self::IR_WARMUP_T_CYCLES
                        + Self::IR_THRESHOLD_T_CYCLES
                        + Self::IR_DECAY_T_CYCLES;
        } else {
            let target = if sensing { Self::IR_WARMUP_T_CYCLES } else { 0 };
            self.sensor_counter = match self.sensor_counter.cmp(&target) {
                std::cmp::Ordering::Less => self.sensor_counter.saturating_add(1).min(target),
                std::cmp::Ordering::Equal => target,
                std::cmp::Ordering::Greater => self.sensor_counter.saturating_sub(1).max(target),
            };
            self.effective_signal_detected = false;
        }
    }

    const fn signal_visible_to_rp(self) -> bool {
        self.read_enabled() && self.effective_signal_detected
    }

    const fn read_enabled(self) -> bool {
        self.rp_latch & Self::RP_READ_ENABLE_MASK == Self::RP_READ_ENABLE_MASK
    }

    const fn optical_input_active(self) -> bool {
        self.external_optical_input || self.emitter_on()
    }
}

impl Default for CgbInfraredState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick(state: &mut CgbInfraredState, t_cycles: u32) {
        for _ in 0..t_cycles {
            state.tick_t_cycle();
        }
    }

    #[test]
    fn rp_latches_only_documented_writable_bits() {
        let mut state = CgbInfraredState::new();

        state.write_rp(0xFF);

        assert_eq!(state.read_rp(), 0xFF);

        state.write_rp(0x3E);

        assert_eq!(state.read_rp(), 0x3E);
        assert!(!state.emitter_on());
    }

    #[test]
    fn partial_read_enable_does_not_expose_optical_input() {
        for value in [0x40, 0x80] {
            let mut state = CgbInfraredState::new();
            state.write_rp(value);
            state.set_external_optical_input(true);

            tick(
                &mut state,
                CgbInfraredState::IR_WARMUP_T_CYCLES + CgbInfraredState::IR_THRESHOLD_T_CYCLES + 1,
            );

            assert_eq!(state.read_rp() & CgbInfraredState::RP_SIGNAL_BIT, 0x02);
            assert!(!state.effective_signal_detected());
        }
    }

    #[test]
    fn enabled_sensor_has_warmup_before_signal_is_visible() {
        let mut state = CgbInfraredState::new();
        state.write_rp(0xC0);
        state.set_external_optical_input(true);
        tick(&mut state, CgbInfraredState::IR_WARMUP_T_CYCLES - 1);

        assert_eq!(state.read_rp() & CgbInfraredState::RP_SIGNAL_BIT, 0x02);

        state.tick_t_cycle();

        assert_eq!(state.read_rp() & CgbInfraredState::RP_SIGNAL_BIT, 0x00);
    }

    #[test]
    fn readied_sensor_reports_short_pulses_without_extra_threshold() {
        let mut state = CgbInfraredState::new();
        state.write_rp(0xC0);
        tick(&mut state, CgbInfraredState::IR_WARMUP_T_CYCLES);

        state.set_external_optical_input(true);
        tick(&mut state, 128);

        assert_eq!(state.read_rp() & CgbInfraredState::RP_SIGNAL_BIT, 0x00);
    }

    #[test]
    fn sustained_signal_fades_until_it_reads_as_no_signal() {
        let mut state = CgbInfraredState::new();
        state.write_rp(0xC0);
        state.set_external_optical_input(true);

        tick(
            &mut state,
            CgbInfraredState::IR_WARMUP_T_CYCLES
                + CgbInfraredState::IR_THRESHOLD_T_CYCLES
                + CgbInfraredState::IR_DECAY_T_CYCLES,
        );
        assert_eq!(state.read_rp() & CgbInfraredState::RP_SIGNAL_BIT, 0x00);

        state.tick_t_cycle();

        assert_eq!(state.read_rp() & CgbInfraredState::RP_SIGNAL_BIT, 0x02);
    }

    #[test]
    fn sensor_recovers_after_sustained_signal_is_removed() {
        let mut state = CgbInfraredState::new();
        state.write_rp(0xC0);
        state.set_external_optical_input(true);

        tick(
            &mut state,
            CgbInfraredState::IR_WARMUP_T_CYCLES
                + CgbInfraredState::IR_THRESHOLD_T_CYCLES
                + CgbInfraredState::IR_DECAY_T_CYCLES
                + 1,
        );
        assert_eq!(state.read_rp() & CgbInfraredState::RP_SIGNAL_BIT, 0x02);

        state.set_external_optical_input(false);
        tick(&mut state, CgbInfraredState::IR_DECAY_T_CYCLES + 1);

        state.set_external_optical_input(true);
        state.tick_t_cycle();

        assert_eq!(state.read_rp() & CgbInfraredState::RP_SIGNAL_BIT, 0x00);
    }

    #[test]
    fn own_emitter_is_part_of_the_optical_input() {
        let mut state = CgbInfraredState::new();
        state.write_rp(0xC1);

        tick(
            &mut state,
            CgbInfraredState::IR_WARMUP_T_CYCLES + CgbInfraredState::IR_THRESHOLD_T_CYCLES,
        );

        assert_eq!(state.read_rp() & CgbInfraredState::RP_SIGNAL_BIT, 0x00);
    }

    #[test]
    fn cgb_infrared_status_reports_receive_ready_and_visible_signal_state() {
        let mut state = CgbInfraredState::default();
        state.write_rp(0xC0);
        tick(&mut state, CgbInfraredState::IR_WARMUP_T_CYCLES);

        let ready = state.status();
        assert!(ready.read_enabled);
        assert!(ready.sensor_warmed);
        assert!(ready.receive_ready());
        assert!(!ready.effective_signal_detected);
        assert!(!ready.signal_visible_to_rp);

        state.set_external_optical_input(true);
        state.tick_t_cycle();

        let receiving = state.status();
        assert!(receiving.optical_input_active);
        assert!(receiving.external_optical_input);
        assert!(receiving.effective_signal_detected);
        assert!(receiving.signal_visible_to_rp);
        assert!(!receiving.receive_ready());
    }
}
