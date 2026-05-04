use super::*;

impl Mbc3RtcPersistentState {
    pub fn apply_elapsed_seconds(&mut self, elapsed_seconds: u64) {
        advance_mbc3_rtc_fields(
            &mut self.seconds,
            &mut self.minutes,
            &mut self.hours,
            &mut self.day_counter,
            self.halt,
            &mut self.carry,
            elapsed_seconds,
        );
    }
}

impl Huc3RtcPersistentState {
    pub fn apply_elapsed_seconds(&mut self, elapsed_seconds: u64) {
        advance_huc3_rtc_fields(
            &mut self.current_minutes_of_day,
            &mut self.current_days,
            &mut self.current_subminute_seconds,
            elapsed_seconds,
        );
    }
}

impl From<Mbc3RtcState> for Mbc3RtcPersistentState {
    fn from(value: Mbc3RtcState) -> Self {
        Self {
            seconds: value.seconds,
            minutes: value.minutes,
            hours: value.hours,
            day_counter: value.day_counter,
            halt: value.halt,
            carry: value.carry,
        }
    }
}

impl From<Mbc3RtcPersistentState> for Mbc3RtcState {
    fn from(value: Mbc3RtcPersistentState) -> Self {
        Self {
            seconds: value.seconds,
            minutes: value.minutes,
            hours: value.hours,
            day_counter: value.day_counter,
            halt: value.halt,
            carry: value.carry,
        }
    }
}

impl From<Huc3RtcState> for Huc3RtcPersistentState {
    fn from(value: Huc3RtcState) -> Self {
        Self {
            current_minutes_of_day: value.current_minutes_of_day,
            current_days: value.current_days,
            current_subminute_seconds: value.current_subminute_seconds,
            event_minutes_of_day: value.event_minutes_of_day,
            event_days: value.event_days,
        }
    }
}

impl From<Huc3RtcPersistentState> for Huc3RtcState {
    fn from(value: Huc3RtcPersistentState) -> Self {
        Self {
            current_minutes_of_day: value.current_minutes_of_day,
            current_days: value.current_days,
            current_subminute_seconds: value.current_subminute_seconds,
            event_minutes_of_day: value.event_minutes_of_day,
            event_days: value.event_days,
        }
    }
}

impl Mbc3RtcState {
    pub(in crate::cartridge) fn read(self, register: Mbc3RtcRegister) -> u8 {
        match register {
            Mbc3RtcRegister::Seconds => self.seconds,
            Mbc3RtcRegister::Minutes => self.minutes,
            Mbc3RtcRegister::Hours => self.hours,
            Mbc3RtcRegister::DayLow => (self.day_counter & 0x00FF) as u8,
            Mbc3RtcRegister::DayHigh => {
                ((self.day_counter >> 8) as u8 & 0x01)
                    | ((self.halt as u8) << 6)
                    | ((self.carry as u8) << 7)
            }
        }
    }

    pub(in crate::cartridge) fn write(&mut self, register: Mbc3RtcRegister, value: u8) {
        match register {
            Mbc3RtcRegister::Seconds => self.seconds = value & 0x3F,
            Mbc3RtcRegister::Minutes => self.minutes = value & 0x3F,
            Mbc3RtcRegister::Hours => self.hours = value & 0x1F,
            Mbc3RtcRegister::DayLow => {
                self.day_counter = (self.day_counter & 0x0100) | value as u16;
            }
            Mbc3RtcRegister::DayHigh => {
                self.day_counter = (self.day_counter & 0x00FF) | (((value & 0x01) as u16) << 8);
                self.halt = value & 0x40 != 0;
                self.carry = value & 0x80 != 0;
            }
        }
    }

    pub(in crate::cartridge) fn advance_seconds(&mut self, elapsed_seconds: u64) {
        advance_mbc3_rtc_fields(
            &mut self.seconds,
            &mut self.minutes,
            &mut self.hours,
            &mut self.day_counter,
            self.halt,
            &mut self.carry,
            elapsed_seconds,
        );
    }
}

pub(in crate::cartridge) fn advance_mbc3_rtc_fields(
    seconds: &mut u8,
    minutes: &mut u8,
    hours: &mut u8,
    day_counter: &mut u16,
    halt: bool,
    carry: &mut bool,
    elapsed_seconds: u64,
) {
    if halt || elapsed_seconds == 0 {
        return;
    }

    *seconds &= 0x3F;
    *minutes &= 0x3F;
    *hours &= 0x1F;
    *day_counter &= 0x01FF;

    let mut remaining_seconds = elapsed_seconds;
    while remaining_seconds > 0 && !mbc3_rtc_fields_are_canonical(*seconds, *minutes, *hours) {
        mbc3_rtc_tick_one_second(seconds, minutes, hours, day_counter, carry);
        remaining_seconds -= 1;
    }

    if remaining_seconds == 0 {
        return;
    }

    let current_total_seconds = *day_counter as u64 * 86_400
        + *hours as u64 * 3_600
        + *minutes as u64 * 60
        + *seconds as u64;
    let advanced_total_seconds = current_total_seconds + remaining_seconds;
    let total_days = advanced_total_seconds / 86_400;
    if total_days > 511 {
        *carry = true;
    }

    let wrapped_days = (total_days % 512) as u16;
    let day_seconds = advanced_total_seconds % 86_400;
    *day_counter = wrapped_days;
    *hours = (day_seconds / 3_600) as u8;
    *minutes = ((day_seconds % 3_600) / 60) as u8;
    *seconds = (day_seconds % 60) as u8;
}

fn mbc3_rtc_fields_are_canonical(seconds: u8, minutes: u8, hours: u8) -> bool {
    seconds < 60 && minutes < 60 && hours < 24
}

fn mbc3_rtc_tick_one_second(
    seconds: &mut u8,
    minutes: &mut u8,
    hours: &mut u8,
    day_counter: &mut u16,
    carry: &mut bool,
) {
    match *seconds {
        59 => {
            *seconds = 0;
            mbc3_rtc_tick_one_minute(minutes, hours, day_counter, carry);
        }
        63 => {
            *seconds = 0;
        }
        _ => {
            *seconds = (*seconds + 1) & 0x3F;
        }
    }
}

fn mbc3_rtc_tick_one_minute(
    minutes: &mut u8,
    hours: &mut u8,
    day_counter: &mut u16,
    carry: &mut bool,
) {
    match *minutes {
        59 => {
            *minutes = 0;
            mbc3_rtc_tick_one_hour(hours, day_counter, carry);
        }
        63 => {
            *minutes = 0;
        }
        _ => {
            *minutes = (*minutes + 1) & 0x3F;
        }
    }
}

fn mbc3_rtc_tick_one_hour(hours: &mut u8, day_counter: &mut u16, carry: &mut bool) {
    match *hours {
        23 => {
            *hours = 0;
            mbc3_rtc_tick_one_day(day_counter, carry);
        }
        31 => {
            *hours = 0;
        }
        _ => {
            *hours = (*hours + 1) & 0x1F;
        }
    }
}

fn mbc3_rtc_tick_one_day(day_counter: &mut u16, carry: &mut bool) {
    let next_day = (*day_counter & 0x01FF) + 1;
    if next_day > 0x01FF {
        *day_counter = 0;
        *carry = true;
    } else {
        *day_counter = next_day;
    }
}

pub(in crate::cartridge) fn advance_huc3_rtc_fields(
    current_minutes_of_day: &mut u16,
    current_days: &mut u16,
    current_subminute_seconds: &mut u8,
    elapsed_seconds: u64,
) {
    if elapsed_seconds == 0 {
        return;
    }

    *current_minutes_of_day %= HUC3_MINUTES_PER_DAY;
    *current_days %= HUC3_DAY_COUNTER_MODULUS;
    *current_subminute_seconds %= 60;

    let total_seconds = *current_subminute_seconds as u64 + elapsed_seconds;
    let minute_delta = total_seconds / 60;
    *current_subminute_seconds = (total_seconds % 60) as u8;

    if minute_delta == 0 {
        return;
    }

    let current_total_minutes =
        *current_days as u64 * HUC3_MINUTES_PER_DAY as u64 + *current_minutes_of_day as u64;
    let wrapped_total_minutes = (current_total_minutes + minute_delta)
        % (HUC3_DAY_COUNTER_MODULUS as u64 * HUC3_MINUTES_PER_DAY as u64);
    *current_days = (wrapped_total_minutes / HUC3_MINUTES_PER_DAY as u64) as u16;
    *current_minutes_of_day = (wrapped_total_minutes % HUC3_MINUTES_PER_DAY as u64) as u16;
}

impl PersistentCartState {
    pub(in crate::cartridge) fn kind_name(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::NoMbcRam { .. } => "NoMbcRam",
            Self::Mmm01Ram { .. } => "Mmm01Ram",
            Self::Huc1Ram { .. } => "Huc1Ram",
            Self::Huc3 { .. } => "Huc3",
            Self::Mbc1Ram { .. } => "Mbc1Ram",
            Self::Mbc2Ram { .. } => "Mbc2Ram",
            Self::Mbc3Rtc { .. } => "Mbc3Rtc",
            Self::Mbc3Ram { .. } => "Mbc3Ram",
            Self::Mbc3RamRtc { .. } => "Mbc3RamRtc",
            Self::Mbc5Ram { .. } => "Mbc5Ram",
            Self::Mbc6 { .. } => "Mbc6",
            Self::PocketCameraRam { .. } => "PocketCameraRam",
        }
    }
}
