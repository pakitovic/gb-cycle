use crate::model::ConsoleModel;
use crate::scheduler::CycleContext;

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
    previous_visible_low_nibble: u8,
    interrupt_request_pending: bool,
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
            previous_visible_low_nibble: 0x0F,
            interrupt_request_pending: false,
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
        self.update_visible_edge_state();
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

        self.update_visible_edge_state();
    }

    pub fn apply_startup_state(&mut self, startup_state: JoypadStartupState) {
        self.selection_bits = startup_state.selection_bits & SELECT_MASK;
        self.pressed_mask = startup_state.pressed_mask;
        self.previous_visible_low_nibble = self.visible_low_nibble();
        self.interrupt_request_pending = false;
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

    pub(crate) fn consume_interrupt_request(&mut self) -> bool {
        let was_pending = self.interrupt_request_pending;
        self.interrupt_request_pending = false;
        was_pending
    }

    pub(crate) fn interrupt_request_pending(&self) -> bool {
        self.interrupt_request_pending
    }

    pub fn scheduler_trace_message(&self, context: &CycleContext) -> String {
        format!(
            "t_cycle={} phase={} console_model={:?} status={:?} p1={:#04X} selection_bits={:#04X} pressed_mask={:#04X} visible_low_nibble={:#03X} interrupt_request_pending={} stop_wake_pending={}",
            context.t_cycle().get(),
            context.phase(),
            self.console_model,
            self.status,
            self.read_p1(),
            self.selection_bits,
            self.pressed_mask,
            self.visible_low_nibble(),
            self.interrupt_request_pending,
            self.stop_wake_pending,
        )
    }

    pub(crate) fn should_emit_scheduler_trace(&self) -> bool {
        self.selection_bits != 0x00
            || self.pressed_mask != 0
            || self.interrupt_request_pending
            || self.stop_wake_pending
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

    fn update_visible_edge_state(&mut self) {
        let visible_low_nibble = self.visible_low_nibble();

        if gained_visible_low_bit(self.previous_visible_low_nibble, visible_low_nibble) {
            self.interrupt_request_pending = true;
        }

        self.previous_visible_low_nibble = visible_low_nibble;
    }
}

const fn gained_visible_low_bit(previous_visible_low_nibble: u8, visible_low_nibble: u8) -> bool {
    (previous_visible_low_nibble & !visible_low_nibble) & 0x0F != 0
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
    fn writing_0x30_reads_back_all_released_when_no_rows_are_selected() {
        let mut joypad = Joypad::new(ConsoleModel::Dmg);

        joypad.set_button_pressed(JoypadButton::A, true);
        joypad.set_button_pressed(JoypadButton::Right, true);
        joypad.write_p1(0x3F);

        assert_eq!(joypad.read_p1(), 0xFF);
    }

    #[test]
    fn button_row_and_dpad_row_are_observed_independently() {
        let mut joypad = Joypad::new(ConsoleModel::Dmg);

        joypad.set_button_pressed(JoypadButton::A, true);
        joypad.set_button_pressed(JoypadButton::Right, true);

        joypad.write_p1(0x10);
        assert_eq!(joypad.read_p1(), 0xDE);

        joypad.write_p1(0x20);
        assert_eq!(joypad.read_p1(), 0xEE);
    }

    #[test]
    fn low_nibble_writes_do_not_override_the_live_matrix_view() {
        let mut joypad = Joypad::new(ConsoleModel::Dmg);

        joypad.set_button_pressed(JoypadButton::Start, true);
        joypad.write_p1(0x17);

        assert_eq!(joypad.read_p1(), 0xD7);
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

    #[test]
    fn visible_button_press_on_the_selected_row_requests_the_joypad_interrupt() {
        let mut joypad = Joypad::new(ConsoleModel::Dmg);

        joypad.write_p1(0x10);
        joypad.set_button_pressed(JoypadButton::A, true);

        assert!(joypad.consume_interrupt_request());
        assert!(!joypad.consume_interrupt_request());
    }

    #[test]
    fn unselected_row_changes_do_not_request_until_a_selection_write_makes_them_visible() {
        let mut joypad = Joypad::new(ConsoleModel::Dmg);

        joypad.write_p1(0x20);
        joypad.set_button_pressed(JoypadButton::A, true);
        assert!(!joypad.consume_interrupt_request());

        joypad.write_p1(0x10);
        assert!(joypad.consume_interrupt_request());
    }

    #[test]
    fn repeated_visible_high_to_low_transitions_can_request_repeatedly() {
        let mut joypad = Joypad::new(ConsoleModel::Dmg);

        joypad.write_p1(0x10);
        joypad.set_button_pressed(JoypadButton::A, true);
        assert!(joypad.consume_interrupt_request());

        joypad.set_button_pressed(JoypadButton::A, false);
        assert!(!joypad.consume_interrupt_request());

        joypad.set_button_pressed(JoypadButton::A, true);
        assert!(joypad.consume_interrupt_request());
    }

    #[test]
    fn both_rows_selected_use_the_same_combined_visible_rule_for_interrupt_edges() {
        let mut joypad = Joypad::new(ConsoleModel::Dmg);

        joypad.write_p1(0x00);
        joypad.set_button_pressed(JoypadButton::A, true);
        assert!(joypad.consume_interrupt_request());

        joypad.set_button_pressed(JoypadButton::Right, true);
        assert!(!joypad.consume_interrupt_request());

        joypad.set_button_pressed(JoypadButton::Up, true);
        assert!(joypad.consume_interrupt_request());
    }

    #[test]
    fn stop_wake_is_selection_independent_across_buttons() {
        let mut joypad = Joypad::new(ConsoleModel::Dmg);

        joypad.write_p1(0x30);
        joypad.set_button_pressed(JoypadButton::A, true);
        assert!(joypad.consume_stop_wake_event());

        joypad.write_p1(0x10);
        joypad.set_button_pressed(JoypadButton::Left, true);
        assert!(joypad.consume_stop_wake_event());
    }

    #[test]
    fn stop_wake_requires_a_new_released_to_pressed_transition() {
        let mut joypad = Joypad::new(ConsoleModel::Dmg);

        joypad.set_button_pressed(JoypadButton::Start, true);
        assert!(joypad.consume_stop_wake_event());

        joypad.set_button_pressed(JoypadButton::Start, true);
        assert!(!joypad.consume_stop_wake_event());

        joypad.set_button_pressed(JoypadButton::Start, false);
        assert!(!joypad.consume_stop_wake_event());

        joypad.set_button_pressed(JoypadButton::Start, true);
        assert!(joypad.consume_stop_wake_event());
    }
}
