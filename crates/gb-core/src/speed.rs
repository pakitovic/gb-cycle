use crate::model::{ConsoleModel, OperatingMode};

pub const CGB_SPEED_SWITCH_PAUSE_T_CYCLES: u16 = 8_200;
const KEY1_UNUSED_READ_MASK: u8 = 0x7E;
const KEY1_CURRENT_SPEED_BIT: u8 = 0x80;
const KEY1_PREPARE_SWITCH_BIT: u8 = 0x01;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CgbSpeedMode {
    Normal,
    Double,
}

impl CgbSpeedMode {
    pub const fn key1_bit(self) -> u8 {
        match self {
            Self::Normal => 0,
            Self::Double => KEY1_CURRENT_SPEED_BIT,
        }
    }

    pub const fn timer_increment_per_scheduler_t_cycle(self) -> u16 {
        match self {
            Self::Normal => 1,
            Self::Double => 1,
        }
    }

    pub const fn div_apu_counter_bit(self) -> u8 {
        match self {
            Self::Normal => 12,
            // In the current CPU-visible speed-domain baseline the divider keeps one counter tick per CPU T-cycle; the APU frame sequencer still selects the undoubled wall-clock domain.
            Self::Double => 13,
        }
    }

    pub const fn serial_internal_clock_edge_bit(self) -> u8 {
        match self {
            Self::Normal => 8,
            Self::Double => 7,
        }
    }

    const fn toggled(self) -> Self {
        match self {
            Self::Normal => Self::Double,
            Self::Double => Self::Normal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SpeedStatus {
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SpeedController {
    console_model: ConsoleModel,
    operating_mode: OperatingMode,
    status: SpeedStatus,
    current_speed: CgbSpeedMode,
    switch_armed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SpeedSaveState {
    console_model: ConsoleModel,
    operating_mode: OperatingMode,
    status: SpeedStatus,
    current_speed: CgbSpeedMode,
    switch_armed: bool,
}

impl SpeedSaveState {
    pub(crate) const fn dynamic_payload_bytes(&self) -> usize {
        0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeedSnapshot {
    pub console_model: ConsoleModel,
    pub operating_mode: OperatingMode,
    pub status: SpeedStatus,
    pub current_speed: CgbSpeedMode,
    pub switch_armed: bool,
    pub cgb_speed_switch_enabled: bool,
}

impl SpeedController {
    pub fn new(console_model: ConsoleModel, operating_mode: OperatingMode) -> Self {
        Self {
            console_model,
            operating_mode,
            status: SpeedStatus::Ready,
            current_speed: CgbSpeedMode::Normal,
            switch_armed: false,
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn operating_mode(&self) -> OperatingMode {
        self.operating_mode
    }

    pub fn status(&self) -> SpeedStatus {
        self.status
    }

    pub fn current_speed(&self) -> CgbSpeedMode {
        if self.cgb_speed_switch_enabled() {
            self.current_speed
        } else {
            CgbSpeedMode::Normal
        }
    }

    pub fn switch_armed(&self) -> bool {
        self.cgb_speed_switch_enabled() && self.switch_armed
    }

    pub fn cgb_speed_switch_enabled(&self) -> bool {
        self.console_model.is_cgb_family() && self.operating_mode.enables_cgb_extensions()
    }

    pub fn reset_for_model_axes(
        &mut self,
        console_model: ConsoleModel,
        operating_mode: OperatingMode,
    ) {
        *self = Self::new(console_model, operating_mode);
    }

    pub(crate) fn capture_save_state(&self) -> SpeedSaveState {
        SpeedSaveState {
            console_model: self.console_model,
            operating_mode: self.operating_mode,
            status: self.status,
            current_speed: self.current_speed,
            switch_armed: self.switch_armed,
        }
    }

    pub(crate) fn restore_save_state(&mut self, state: &SpeedSaveState) {
        self.console_model = state.console_model;
        self.operating_mode = state.operating_mode;
        self.status = state.status;
        self.current_speed = state.current_speed;
        self.switch_armed = state.switch_armed;
    }

    pub fn read_key1(&self) -> u8 {
        if !self.cgb_speed_switch_enabled() {
            return 0xFF;
        }

        KEY1_UNUSED_READ_MASK
            | self.current_speed.key1_bit()
            | (u8::from(self.switch_armed) * KEY1_PREPARE_SWITCH_BIT)
    }

    pub fn write_key1(&mut self, value: u8) {
        if !self.cgb_speed_switch_enabled() {
            return;
        }

        self.switch_armed = value & KEY1_PREPARE_SWITCH_BIT != 0;
    }

    pub(crate) fn begin_prepared_speed_switch(&mut self) -> bool {
        if !self.switch_armed() {
            return false;
        }

        self.switch_armed = false;
        self.current_speed = self.current_speed.toggled();
        true
    }

    pub fn snapshot(&self) -> SpeedSnapshot {
        SpeedSnapshot {
            console_model: self.console_model,
            operating_mode: self.operating_mode,
            status: self.status,
            current_speed: self.current_speed(),
            switch_armed: self.switch_armed(),
            cgb_speed_switch_enabled: self.cgb_speed_switch_enabled(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key1_is_unavailable_outside_native_cgb_mode() {
        let mut dmg = SpeedController::new(ConsoleModel::GameBoy, OperatingMode::Dmg);
        dmg.write_key1(0x01);
        assert_eq!(dmg.read_key1(), 0xFF);
        assert_eq!(dmg.current_speed(), CgbSpeedMode::Normal);
        assert!(!dmg.switch_armed());

        let mut cgb_compat =
            SpeedController::new(ConsoleModel::GameBoyColor, OperatingMode::GbCompatible);
        cgb_compat.write_key1(0x01);
        assert_eq!(cgb_compat.read_key1(), 0xFF);
        assert_eq!(cgb_compat.current_speed(), CgbSpeedMode::Normal);
        assert!(!cgb_compat.switch_armed());
    }

    #[test]
    fn key1_tracks_prepare_bit_and_current_speed() {
        let mut speed = SpeedController::new(ConsoleModel::GameBoyColor, OperatingMode::Cgb);

        assert_eq!(speed.read_key1(), 0x7E);
        speed.write_key1(0x01);
        assert_eq!(speed.read_key1(), 0x7F);
        assert!(speed.begin_prepared_speed_switch());
        assert_eq!(speed.read_key1(), 0xFE);
        assert!(!speed.begin_prepared_speed_switch());
        speed.write_key1(0x00);
        assert_eq!(speed.read_key1(), 0xFE);
        speed.write_key1(0x01);
        assert!(speed.begin_prepared_speed_switch());
        assert_eq!(speed.read_key1(), 0x7E);
    }

    #[test]
    fn model_axes_snapshot_and_save_state_preserve_speed_state() {
        let mut speed = SpeedController::new(ConsoleModel::GameBoyColor, OperatingMode::Cgb);

        assert_eq!(speed.console_model(), ConsoleModel::GameBoyColor);
        assert_eq!(speed.operating_mode(), OperatingMode::Cgb);
        assert_eq!(speed.status(), SpeedStatus::Ready);
        assert!(speed.cgb_speed_switch_enabled());

        speed.write_key1(0x01);
        assert!(speed.begin_prepared_speed_switch());
        speed.write_key1(0x01);

        let snapshot = speed.snapshot();
        assert_eq!(snapshot.console_model, ConsoleModel::GameBoyColor);
        assert_eq!(snapshot.operating_mode, OperatingMode::Cgb);
        assert_eq!(snapshot.status, SpeedStatus::Ready);
        assert_eq!(snapshot.current_speed, CgbSpeedMode::Double);
        assert!(snapshot.switch_armed);
        assert!(snapshot.cgb_speed_switch_enabled);

        let save_state = speed.capture_save_state();
        assert_eq!(save_state.dynamic_payload_bytes(), 0);

        let mut restored = SpeedController::new(ConsoleModel::GameBoy, OperatingMode::Dmg);
        restored.restore_save_state(&save_state);
        assert_eq!(restored.console_model(), ConsoleModel::GameBoyColor);
        assert_eq!(restored.operating_mode(), OperatingMode::Cgb);
        assert_eq!(restored.status(), SpeedStatus::Ready);
        assert_eq!(restored.current_speed(), CgbSpeedMode::Double);
        assert!(restored.switch_armed());

        restored.reset_for_model_axes(ConsoleModel::GameBoy, OperatingMode::Dmg);
        assert_eq!(restored.console_model(), ConsoleModel::GameBoy);
        assert_eq!(restored.operating_mode(), OperatingMode::Dmg);
        assert_eq!(restored.status(), SpeedStatus::Ready);
        assert_eq!(restored.current_speed(), CgbSpeedMode::Normal);
        assert!(!restored.switch_armed());
        assert!(!restored.cgb_speed_switch_enabled());
        assert_eq!(restored.snapshot().current_speed, CgbSpeedMode::Normal);
    }

    #[test]
    fn speed_mode_publishes_domain_cadences() {
        assert_eq!(CgbSpeedMode::Normal.key1_bit(), 0x00);
        assert_eq!(CgbSpeedMode::Double.key1_bit(), KEY1_CURRENT_SPEED_BIT);
        assert_eq!(
            CgbSpeedMode::Normal.timer_increment_per_scheduler_t_cycle(),
            1
        );
        assert_eq!(
            CgbSpeedMode::Double.timer_increment_per_scheduler_t_cycle(),
            1
        );
        assert_eq!(CgbSpeedMode::Normal.div_apu_counter_bit(), 12);
        assert_eq!(CgbSpeedMode::Double.div_apu_counter_bit(), 13);
        assert_eq!(CgbSpeedMode::Normal.serial_internal_clock_edge_bit(), 8);
        assert_eq!(CgbSpeedMode::Double.serial_internal_clock_edge_bit(), 7);
    }
}
