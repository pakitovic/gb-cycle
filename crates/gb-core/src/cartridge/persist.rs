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

    *seconds %= 60;
    *minutes %= 60;
    *hours %= 24;
    *day_counter &= 0x01FF;

    let current_total_seconds = *day_counter as u64 * 86_400
        + *hours as u64 * 3_600
        + *minutes as u64 * 60
        + *seconds as u64;
    let advanced_total_seconds = current_total_seconds + elapsed_seconds;
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

impl PersistentCartState {
    pub(in crate::cartridge) fn kind_name(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::NoMbcRam { .. } => "NoMbcRam",
            Self::Mmm01Ram { .. } => "Mmm01Ram",
            Self::Mbc1Ram { .. } => "Mbc1Ram",
            Self::Mbc2Ram { .. } => "Mbc2Ram",
            Self::Mbc3Rtc { .. } => "Mbc3Rtc",
            Self::Mbc3Ram { .. } => "Mbc3Ram",
            Self::Mbc3RamRtc { .. } => "Mbc3RamRtc",
            Self::Mbc5Ram { .. } => "Mbc5Ram",
        }
    }
}
