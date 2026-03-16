use crate::model::ConsoleModel;

const SELECT_MASK: u8 = 0x30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoypadStatus {
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JoypadButton {
    Right,
    Left,
    Up,
    Down,
    A,
    B,
    Select,
    Start,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JoypadStartupState {
    pub selection_bits: u8,
    pub pressed_mask: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Joypad {
    console_model: ConsoleModel,
    status: JoypadStatus,
    selection_bits: u8,
    pressed_mask: u8,
    stop_wake_pending: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoypadSnapshot {
    pub console_model: ConsoleModel,
    pub status: JoypadStatus,
    pub selection_bits: u8,
    pub pressed_mask: u8,
}

impl Joypad {
    pub fn new(console_model: ConsoleModel) -> Self {
        Self {
            console_model,
            status: JoypadStatus::Ready,
            selection_bits: SELECT_MASK,
            pressed_mask: 0,
            stop_wake_pending: false,
        }
    }

    pub fn console_model(&self) -> ConsoleModel {
        self.console_model
    }

    pub fn status(&self) -> JoypadStatus {
        self.status
    }

    pub fn read_p1(&self) -> u8 {
        0xC0 | self.selection_bits | self.visible_low_nibble()
    }

    pub fn write_p1(&mut self, value: u8) {
        self.selection_bits = value & SELECT_MASK;
    }

    pub fn set_button_pressed(&mut self, button: JoypadButton, pressed: bool) {
        let bit = button_mask(button);
        let was_pressed = self.pressed_mask & bit != 0;

        if pressed {
            self.pressed_mask |= bit;
        } else {
            self.pressed_mask &= !bit;
        }

        if pressed && !was_pressed {
            self.stop_wake_pending = true;
        }
    }

    pub fn apply_startup_state(&mut self, startup_state: JoypadStartupState) {
        self.selection_bits = startup_state.selection_bits & SELECT_MASK;
        self.pressed_mask = startup_state.pressed_mask;
        self.stop_wake_pending = false;
    }

    pub fn snapshot(&self) -> JoypadSnapshot {
        JoypadSnapshot {
            console_model: self.console_model,
            status: self.status,
            selection_bits: self.selection_bits,
            pressed_mask: self.pressed_mask,
        }
    }

    pub(crate) fn consume_stop_wake_event(&mut self) -> bool {
        let was_pending = self.stop_wake_pending;
        self.stop_wake_pending = false;
        was_pending
    }

    fn visible_low_nibble(&self) -> u8 {
        let mut low = 0x0F;

        if self.selection_bits & 0x20 == 0 {
            low &= !button_row_low_bits(self.pressed_mask);
        }

        if self.selection_bits & 0x10 == 0 {
            low &= !dpad_row_low_bits(self.pressed_mask);
        }

        low
    }
}

const fn button_mask(button: JoypadButton) -> u8 {
    match button {
        JoypadButton::Right => 0x01,
        JoypadButton::Left => 0x02,
        JoypadButton::Up => 0x04,
        JoypadButton::Down => 0x08,
        JoypadButton::A => 0x10,
        JoypadButton::B => 0x20,
        JoypadButton::Select => 0x40,
        JoypadButton::Start => 0x80,
    }
}

const fn button_row_low_bits(pressed_mask: u8) -> u8 {
    let mut low = 0;

    if pressed_mask & button_mask(JoypadButton::A) != 0 {
        low |= 0x01;
    }
    if pressed_mask & button_mask(JoypadButton::B) != 0 {
        low |= 0x02;
    }
    if pressed_mask & button_mask(JoypadButton::Select) != 0 {
        low |= 0x04;
    }
    if pressed_mask & button_mask(JoypadButton::Start) != 0 {
        low |= 0x08;
    }

    low
}

const fn dpad_row_low_bits(pressed_mask: u8) -> u8 {
    let mut low = 0;

    if pressed_mask & button_mask(JoypadButton::Right) != 0 {
        low |= 0x01;
    }
    if pressed_mask & button_mask(JoypadButton::Left) != 0 {
        low |= 0x02;
    }
    if pressed_mask & button_mask(JoypadButton::Up) != 0 {
        low |= 0x04;
    }
    if pressed_mask & button_mask(JoypadButton::Down) != 0 {
        low |= 0x08;
    }

    low
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joyp_keeps_upper_bits_high_and_uses_row_selection() {
        let mut joypad = Joypad::new(ConsoleModel::Dmg);

        joypad.set_button_pressed(JoypadButton::A, true);
        joypad.write_p1(0x10);

        assert_eq!(joypad.read_p1(), 0xDE);
    }

    #[test]
    fn selecting_both_rows_combines_the_visible_matrix_without_priority() {
        let mut joypad = Joypad::new(ConsoleModel::Dmg);

        joypad.set_button_pressed(JoypadButton::A, true);
        joypad.set_button_pressed(JoypadButton::Right, true);
        joypad.write_p1(0x00);

        assert_eq!(joypad.read_p1(), 0xCE);
    }

    #[test]
    fn startup_state_can_recreate_the_documented_post_boot_p1_snapshot() {
        let mut joypad = Joypad::new(ConsoleModel::Dmg);

        joypad.apply_startup_state(JoypadStartupState {
            selection_bits: 0x00,
            pressed_mask: 0x00,
        });

        assert_eq!(joypad.read_p1(), 0xCF);
    }
}
