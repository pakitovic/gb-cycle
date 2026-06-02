use gb_core::CgbSpeedMode;

use super::DeterministicMbc3RtcClock;

#[test]
fn mbc3_rtc_clock_ticks_once_per_128_normal_speed_tcycles() {
    let mut clock = DeterministicMbc3RtcClock::default();

    for _ in 0..127 {
        assert_eq!(clock.tick_t_cycle_for_speed(CgbSpeedMode::Normal), 0);
    }
    assert_eq!(clock.tick_t_cycle_for_speed(CgbSpeedMode::Normal), 1);
}

#[test]
fn mbc3_rtc_clock_ticks_once_per_256_double_speed_tcycles() {
    let mut clock = DeterministicMbc3RtcClock::default();

    for _ in 0..255 {
        assert_eq!(clock.tick_t_cycle_for_speed(CgbSpeedMode::Double), 0);
    }
    assert_eq!(clock.tick_t_cycle_for_speed(CgbSpeedMode::Double), 1);
}

#[test]
fn mbc3_rtc_clock_preserves_remainder_across_speed_changes() {
    let mut clock = DeterministicMbc3RtcClock::default();

    for _ in 0..64 {
        assert_eq!(clock.tick_t_cycle_for_speed(CgbSpeedMode::Normal), 0);
    }
    for _ in 0..127 {
        assert_eq!(clock.tick_t_cycle_for_speed(CgbSpeedMode::Double), 0);
    }
    assert_eq!(clock.tick_t_cycle_for_speed(CgbSpeedMode::Double), 1);
}
