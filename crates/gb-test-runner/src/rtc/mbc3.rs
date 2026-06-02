use gb_core::CgbSpeedMode;

const MBC3_RTC_TICK_HALF_NORMAL_T_CYCLES: u16 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct DeterministicMbc3RtcClock {
    half_normal_t_cycle_remainder: u16,
}

impl DeterministicMbc3RtcClock {
    pub(crate) fn tick_t_cycle_for_speed(&mut self, speed_mode: CgbSpeedMode) -> u64 {
        self.half_normal_t_cycle_remainder += match speed_mode {
            CgbSpeedMode::Normal => 2,
            CgbSpeedMode::Double => 1,
        };

        if self.half_normal_t_cycle_remainder >= MBC3_RTC_TICK_HALF_NORMAL_T_CYCLES {
            self.half_normal_t_cycle_remainder -= MBC3_RTC_TICK_HALF_NORMAL_T_CYCLES;
            1
        } else {
            0
        }
    }
}
