use gb_core::{JoypadButton, Machine, TraceSummaryBuffer};
use gb_desktop::{
    GamepadButtonBinding, GamepadButtonBindings, GamepadDirectionalSource, GamepadMenuBindings,
    GamepadOptions, PreferredGamepadIdentity,
};
use sdl3::GamepadSubsystem;
use sdl3::event::Event;
use sdl3::gamepad::{Axis, Button, Gamepad};
use sdl3::joystick::JoystickId;
use std::collections::BTreeMap;

const LEFT_STICK_PRESS_THRESHOLD: i16 = 16_384;
const LEFT_STICK_RELEASE_THRESHOLD: i16 = 12_288;
const JOYPAD_BUTTONS: [JoypadButton; 8] = [
    JoypadButton::Up,
    JoypadButton::Down,
    JoypadButton::Left,
    JoypadButton::Right,
    JoypadButton::A,
    JoypadButton::B,
    JoypadButton::Select,
    JoypadButton::Start,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputSource {
    Keyboard,
    GamepadButtons,
    GamepadLeftStick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct SourceJoypadState {
    pressed: [bool; JOYPAD_BUTTONS.len()],
}

impl SourceJoypadState {
    fn is_pressed(&self, button: JoypadButton) -> bool {
        self.pressed[joypad_button_index(button)]
    }

    fn set_pressed(&mut self, button: JoypadButton, pressed: bool) {
        self.pressed[joypad_button_index(button)] = pressed;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct LeftStickDigitalState {
    left: bool,
    right: bool,
    up: bool,
    down: bool,
}

pub struct FrontendInputState {
    keyboard: SourceJoypadState,
    gamepad_buttons: SourceJoypadState,
    gamepad_left_stick: SourceJoypadState,
}

impl FrontendInputState {
    pub fn new() -> Self {
        Self {
            keyboard: SourceJoypadState::default(),
            gamepad_buttons: SourceJoypadState::default(),
            gamepad_left_stick: SourceJoypadState::default(),
        }
    }

    pub fn set_keyboard_button(
        &mut self,
        machine: &mut Machine<TraceSummaryBuffer>,
        button: JoypadButton,
        pressed: bool,
    ) {
        self.set_source_button(InputSource::Keyboard, machine, button, pressed);
    }

    pub fn set_gamepad_button(
        &mut self,
        machine: &mut Machine<TraceSummaryBuffer>,
        button: JoypadButton,
        pressed: bool,
    ) {
        self.set_source_button(InputSource::GamepadButtons, machine, button, pressed);
    }

    pub fn set_gamepad_left_stick_button(
        &mut self,
        machine: &mut Machine<TraceSummaryBuffer>,
        button: JoypadButton,
        pressed: bool,
    ) {
        self.set_source_button(InputSource::GamepadLeftStick, machine, button, pressed);
    }

    pub fn clear_gamepad(&mut self, machine: &mut Machine<TraceSummaryBuffer>) {
        for button in JOYPAD_BUTTONS {
            self.set_gamepad_button(machine, button, false);
            self.set_gamepad_left_stick_button(machine, button, false);
        }
    }

    pub fn clear_keyboard(&mut self, machine: &mut Machine<TraceSummaryBuffer>) {
        for button in JOYPAD_BUTTONS {
            self.set_keyboard_button(machine, button, false);
        }
    }

    pub fn clear_all(&mut self, machine: &mut Machine<TraceSummaryBuffer>) {
        self.clear_keyboard(machine);
        self.clear_gamepad(machine);
    }

    fn set_source_button(
        &mut self,
        source: InputSource,
        machine: &mut Machine<TraceSummaryBuffer>,
        button: JoypadButton,
        pressed: bool,
    ) {
        let was_effective = self.is_effectively_pressed(button);
        self.source_state_mut(source).set_pressed(button, pressed);
        let is_effective = self.is_effectively_pressed(button);
        if is_effective != was_effective {
            machine.set_joypad_button_pressed(button, is_effective);
        }
    }

    fn is_effectively_pressed(&self, button: JoypadButton) -> bool {
        self.keyboard.is_pressed(button)
            || self.gamepad_buttons.is_pressed(button)
            || self.gamepad_left_stick.is_pressed(button)
    }

    fn source_state_mut(&mut self, source: InputSource) -> &mut SourceJoypadState {
        match source {
            InputSource::Keyboard => &mut self.keyboard,
            InputSource::GamepadButtons => &mut self.gamepad_buttons,
            InputSource::GamepadLeftStick => &mut self.gamepad_left_stick,
        }
    }
}

pub struct GamepadManager {
    subsystem: GamepadSubsystem,
    options: GamepadOptions,
    opened: BTreeMap<JoystickId, OpenGamepad>,
    active: Option<JoystickId>,
    left_stick_state: LeftStickDigitalState,
}

struct OpenGamepad {
    gamepad: Gamepad,
    name: String,
    path: Option<String>,
}

impl OpenGamepad {
    fn identity(&self) -> PreferredGamepadIdentity {
        PreferredGamepadIdentity {
            path: self.path.clone(),
            name: Some(self.name.clone()),
        }
    }
}

impl GamepadManager {
    pub fn new(
        subsystem: &GamepadSubsystem,
        options: GamepadOptions,
        input_state: &mut FrontendInputState,
        machine: &mut Machine<TraceSummaryBuffer>,
    ) -> Result<Self, String> {
        let mut manager = Self {
            subsystem: subsystem.clone(),
            options,
            opened: BTreeMap::new(),
            active: None,
            left_stick_state: LeftStickDigitalState::default(),
        };

        let mut gamepads = manager
            .subsystem
            .gamepads()
            .map_err(|error| format!("failed to enumerate SDL3 gamepads: {error}"))?;
        gamepads.sort_unstable();
        for joystick_id in gamepads {
            manager.open_gamepad(joystick_id)?;
        }
        manager.sync_active_gamepad_state(input_state, machine);

        Ok(manager)
    }

    pub fn handle_event(
        &mut self,
        event: &Event,
        input_state: &mut FrontendInputState,
        machine: &mut Machine<TraceSummaryBuffer>,
    ) -> Result<(), String> {
        match *event {
            Event::ControllerDeviceAdded { which, .. } => {
                let became_active = self.open_gamepad(joystick_id_from_event(which))?;
                if became_active {
                    self.sync_active_gamepad_state(input_state, machine);
                }
            }
            Event::ControllerDeviceRemoved { which, .. } => {
                self.remove_gamepad(joystick_id_from_event(which), input_state, machine);
            }
            Event::ControllerDeviceRemapped { which, .. } => {
                if self.active == Some(joystick_id_from_event(which)) {
                    eprintln!("info: active SDL gamepad remapped");
                    self.sync_active_gamepad_state(input_state, machine);
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn open_gamepad(&mut self, joystick_id: JoystickId) -> Result<bool, String> {
        if self.opened.contains_key(&joystick_id) {
            return Ok(false);
        }

        let gamepad = self
            .subsystem
            .open(joystick_id)
            .map_err(|error| format!("failed to open SDL3 gamepad {}: {error}", joystick_id.0))?;
        let opened_gamepad = OpenGamepad {
            path: gamepad.path(),
            name: gamepad
                .name()
                .unwrap_or_else(|| format!("SDL gamepad {}", joystick_id.0)),
            gamepad,
        };

        let previous_active = self.active;
        self.opened.insert(joystick_id, opened_gamepad);
        self.active = self.select_active_gamepad();

        let Some(gamepad) = self.opened.get(&joystick_id) else {
            return Ok(false);
        };

        if self.active == Some(joystick_id) && previous_active != Some(joystick_id) {
            eprintln!("info: active SDL gamepad: {}", gamepad.name);
            return Ok(true);
        }

        eprintln!("info: SDL gamepad connected: {}", gamepad.name);
        Ok(false)
    }

    fn select_active_gamepad(&self) -> Option<JoystickId> {
        self.opened
            .iter()
            .find_map(|(joystick_id, gamepad)| {
                self.matches_preferred_device(gamepad)
                    .then_some(*joystick_id)
            })
            .or_else(|| self.opened.keys().next().copied())
    }

    fn matches_preferred_device(&self, gamepad: &OpenGamepad) -> bool {
        let preferred_device = &self.options.preferred_device;
        if !preferred_device.is_configured() {
            return false;
        }

        if let Some(preferred_path) = preferred_device.path.as_deref()
            && gamepad.path.as_deref() == Some(preferred_path)
        {
            return true;
        }

        if let Some(preferred_name) = preferred_device.name.as_deref() {
            return gamepad.name == preferred_name;
        }

        false
    }

    fn remove_gamepad(
        &mut self,
        joystick_id: JoystickId,
        input_state: &mut FrontendInputState,
        machine: &mut Machine<TraceSummaryBuffer>,
    ) {
        let removed_name = self
            .opened
            .remove(&joystick_id)
            .map(|gamepad| gamepad.name)
            .unwrap_or_else(|| format!("SDL gamepad {}", joystick_id.0));
        eprintln!("info: SDL gamepad disconnected: {removed_name}");

        let previous_active = self.active;
        self.active = self.select_active_gamepad();
        if previous_active == self.active {
            return;
        }

        if let Some(next_active) = self.active {
            let next_name = self
                .opened
                .get(&next_active)
                .map(|gamepad| gamepad.name.as_str())
                .unwrap_or("SDL gamepad");
            eprintln!("info: active SDL gamepad: {next_name}");
        }

        self.sync_active_gamepad_state(input_state, machine);
    }

    pub fn poll_active_gamepad_state(
        &mut self,
        input_state: &mut FrontendInputState,
        machine: &mut Machine<TraceSummaryBuffer>,
    ) {
        let Some(active_joystick_id) = self.active else {
            return;
        };
        let Some(gamepad) = self.opened.get(&active_joystick_id) else {
            return;
        };

        let bound_button_states = [
            (
                JoypadButton::Up,
                gamepad_button_binding_state(gamepad, self.options.bindings.up),
            ),
            (
                JoypadButton::Down,
                gamepad_button_binding_state(gamepad, self.options.bindings.down),
            ),
            (
                JoypadButton::Left,
                gamepad_button_binding_state(gamepad, self.options.bindings.left),
            ),
            (
                JoypadButton::Right,
                gamepad_button_binding_state(gamepad, self.options.bindings.right),
            ),
            (
                JoypadButton::A,
                gamepad_button_binding_state(gamepad, self.options.bindings.a),
            ),
            (
                JoypadButton::B,
                gamepad_button_binding_state(gamepad, self.options.bindings.b),
            ),
            (
                JoypadButton::Select,
                gamepad_button_binding_state(gamepad, self.options.bindings.select),
            ),
            (
                JoypadButton::Start,
                gamepad_button_binding_state(gamepad, self.options.bindings.start),
            ),
        ];
        let left_x = gamepad.gamepad.axis(Axis::LeftX);
        let left_y = gamepad.gamepad.axis(Axis::LeftY);

        self.apply_polled_bound_buttons(bound_button_states, input_state, machine);
        self.apply_polled_directional_inputs(left_x, left_y, input_state, machine);
    }

    pub fn is_active_gamepad(&self, joystick_id: JoystickId) -> bool {
        self.active == Some(joystick_id)
    }

    pub fn has_connected_gamepad(&self) -> bool {
        self.active.is_some()
    }

    pub fn active_gamepad_name(&self) -> Option<&str> {
        self.active_gamepad().map(|gamepad| gamepad.name.as_str())
    }

    pub fn active_gamepad_identity(&self) -> Option<PreferredGamepadIdentity> {
        self.active_gamepad().map(OpenGamepad::identity)
    }

    pub fn preferred_device(&self) -> &PreferredGamepadIdentity {
        &self.options.preferred_device
    }

    pub fn preferred_device_name(&self) -> Option<&str> {
        if self.active_matches_preferred() {
            return self.active_gamepad_name();
        }

        self.options.preferred_device.name.as_deref()
    }

    pub fn active_matches_preferred(&self) -> bool {
        self.active_gamepad()
            .is_some_and(|gamepad| self.matches_preferred_device(gamepad))
    }

    pub fn directional_source(&self) -> GamepadDirectionalSource {
        self.options.directional_source
    }

    pub fn button_bindings(&self) -> GamepadButtonBindings {
        self.options.bindings
    }

    pub fn menu_bindings(&self) -> GamepadMenuBindings {
        self.options.menu
    }

    pub fn set_directional_source(
        &mut self,
        directional_source: GamepadDirectionalSource,
        input_state: &mut FrontendInputState,
        machine: &mut Machine<TraceSummaryBuffer>,
    ) {
        if self.options.directional_source == directional_source {
            return;
        }

        self.options.directional_source = directional_source;
        self.sync_active_gamepad_state(input_state, machine);
    }

    pub fn set_button_bindings(
        &mut self,
        bindings: GamepadButtonBindings,
        input_state: &mut FrontendInputState,
        machine: &mut Machine<TraceSummaryBuffer>,
    ) {
        if self.options.bindings == bindings {
            return;
        }

        self.options.bindings = bindings;
        self.sync_active_gamepad_state(input_state, machine);
    }

    pub fn set_menu_bindings(&mut self, bindings: GamepadMenuBindings) {
        self.options.menu = bindings;
    }

    pub fn set_preferred_device(
        &mut self,
        preferred_device: PreferredGamepadIdentity,
        input_state: &mut FrontendInputState,
        machine: &mut Machine<TraceSummaryBuffer>,
    ) {
        if self.options.preferred_device == preferred_device {
            return;
        }

        self.options.preferred_device = preferred_device;
        let previous_active = self.active;
        self.active = self.select_active_gamepad();
        if previous_active != self.active {
            self.log_active_gamepad();
        }
        self.sync_active_gamepad_state(input_state, machine);
    }

    pub fn activate_gamepad_from_input(
        &mut self,
        joystick_id: JoystickId,
        input_state: &mut FrontendInputState,
        machine: &mut Machine<TraceSummaryBuffer>,
    ) -> bool {
        if self.active == Some(joystick_id) || !self.opened.contains_key(&joystick_id) {
            return false;
        }

        if self.options.preferred_device.is_configured() && self.active_matches_preferred() {
            return false;
        }

        self.active = Some(joystick_id);
        self.log_active_gamepad();
        self.sync_active_gamepad_state(input_state, machine);
        true
    }

    pub fn sync_active_gamepad_state(
        &mut self,
        input_state: &mut FrontendInputState,
        machine: &mut Machine<TraceSummaryBuffer>,
    ) {
        self.left_stick_state = LeftStickDigitalState::default();
        input_state.clear_gamepad(machine);
        self.poll_active_gamepad_state(input_state, machine);
    }

    fn active_gamepad(&self) -> Option<&OpenGamepad> {
        self.active
            .and_then(|joystick_id| self.opened.get(&joystick_id))
    }

    fn log_active_gamepad(&self) {
        if let Some(gamepad) = self.active_gamepad() {
            eprintln!("info: active SDL gamepad: {}", gamepad.name);
        }
    }

    fn apply_polled_bound_buttons(
        &self,
        states: [(JoypadButton, bool); 8],
        input_state: &mut FrontendInputState,
        machine: &mut Machine<TraceSummaryBuffer>,
    ) {
        for (joypad_button, pressed) in states {
            input_state.set_gamepad_button(machine, joypad_button, pressed);
        }
    }

    fn apply_polled_directional_inputs(
        &mut self,
        left_x: i16,
        left_y: i16,
        input_state: &mut FrontendInputState,
        machine: &mut Machine<TraceSummaryBuffer>,
    ) {
        let (left, right, up, down) = if self.options.directional_source.uses_left_stick() {
            let (left, right) = axis_direction_state(
                left_x,
                self.left_stick_state.left,
                self.left_stick_state.right,
            );
            let (up, down) =
                axis_direction_state(left_y, self.left_stick_state.up, self.left_stick_state.down);
            (left, right, up, down)
        } else {
            (false, false, false, false)
        };

        update_left_stick_button(
            JoypadButton::Left,
            &mut self.left_stick_state.left,
            left,
            input_state,
            machine,
        );
        update_left_stick_button(
            JoypadButton::Right,
            &mut self.left_stick_state.right,
            right,
            input_state,
            machine,
        );
        update_left_stick_button(
            JoypadButton::Up,
            &mut self.left_stick_state.up,
            up,
            input_state,
            machine,
        );
        update_left_stick_button(
            JoypadButton::Down,
            &mut self.left_stick_state.down,
            down,
            input_state,
            machine,
        );
    }
}

fn gamepad_button_binding_state(gamepad: &OpenGamepad, binding: GamepadButtonBinding) -> bool {
    gamepad.gamepad.button(sdl_button_for_binding(binding))
}

pub fn sdl_button_for_binding(binding: GamepadButtonBinding) -> Button {
    match binding {
        GamepadButtonBinding::South => Button::South,
        GamepadButtonBinding::East => Button::East,
        GamepadButtonBinding::West => Button::West,
        GamepadButtonBinding::North => Button::North,
        GamepadButtonBinding::Back => Button::Back,
        GamepadButtonBinding::Start => Button::Start,
        GamepadButtonBinding::Guide => Button::Guide,
        GamepadButtonBinding::LeftShoulder => Button::LeftShoulder,
        GamepadButtonBinding::RightShoulder => Button::RightShoulder,
        GamepadButtonBinding::LeftStickClick => Button::LeftStick,
        GamepadButtonBinding::RightStickClick => Button::RightStick,
        GamepadButtonBinding::DPadUp => Button::DPadUp,
        GamepadButtonBinding::DPadDown => Button::DPadDown,
        GamepadButtonBinding::DPadLeft => Button::DPadLeft,
        GamepadButtonBinding::DPadRight => Button::DPadRight,
        GamepadButtonBinding::Misc1 => Button::Misc1,
    }
}

pub fn gamepad_button_binding_from_sdl_button(button: Button) -> Option<GamepadButtonBinding> {
    match button {
        Button::South => Some(GamepadButtonBinding::South),
        Button::East => Some(GamepadButtonBinding::East),
        Button::West => Some(GamepadButtonBinding::West),
        Button::North => Some(GamepadButtonBinding::North),
        Button::Back => Some(GamepadButtonBinding::Back),
        Button::Start => Some(GamepadButtonBinding::Start),
        Button::Guide => Some(GamepadButtonBinding::Guide),
        Button::LeftShoulder => Some(GamepadButtonBinding::LeftShoulder),
        Button::RightShoulder => Some(GamepadButtonBinding::RightShoulder),
        Button::LeftStick => Some(GamepadButtonBinding::LeftStickClick),
        Button::RightStick => Some(GamepadButtonBinding::RightStickClick),
        Button::DPadUp => Some(GamepadButtonBinding::DPadUp),
        Button::DPadDown => Some(GamepadButtonBinding::DPadDown),
        Button::DPadLeft => Some(GamepadButtonBinding::DPadLeft),
        Button::DPadRight => Some(GamepadButtonBinding::DPadRight),
        Button::Misc1 => Some(GamepadButtonBinding::Misc1),
        _ => None,
    }
}

fn axis_direction_state(
    value: i16,
    negative_was_pressed: bool,
    positive_was_pressed: bool,
) -> (bool, bool) {
    let negative = if negative_was_pressed {
        value <= -LEFT_STICK_RELEASE_THRESHOLD
    } else {
        value <= -LEFT_STICK_PRESS_THRESHOLD
    };
    let positive = if positive_was_pressed {
        value >= LEFT_STICK_RELEASE_THRESHOLD
    } else {
        value >= LEFT_STICK_PRESS_THRESHOLD
    };
    (negative, positive)
}

fn joystick_id_from_event(which: u32) -> JoystickId {
    sdl3::sys::joystick::SDL_JoystickID(which)
}

fn update_left_stick_button(
    button: JoypadButton,
    current_state: &mut bool,
    next_state: bool,
    input_state: &mut FrontendInputState,
    machine: &mut Machine<TraceSummaryBuffer>,
) {
    if *current_state == next_state {
        return;
    }

    *current_state = next_state;
    input_state.set_gamepad_left_stick_button(machine, button, next_state);
}

fn joypad_button_index(button: JoypadButton) -> usize {
    match button {
        JoypadButton::Up => 0,
        JoypadButton::Down => 1,
        JoypadButton::Left => 2,
        JoypadButton::Right => 3,
        JoypadButton::A => 4,
        JoypadButton::B => 5,
        JoypadButton::Select => 6,
        JoypadButton::Start => 7,
    }
}

#[cfg(test)]
mod tests {
    use super::{axis_direction_state, sdl_button_for_binding};
    use gb_core::JoypadButton;
    use gb_desktop::{GamepadButtonBinding, GamepadFaceLayout};
    use sdl3::gamepad::Button;

    #[test]
    fn east_a_face_layout_uses_east_for_a_and_south_for_b() {
        let (a, b) = GamepadFaceLayout::EastASouthB.face_buttons();

        assert_eq!(a, GamepadButtonBinding::East);
        assert_eq!(b, GamepadButtonBinding::South);
    }

    #[test]
    fn south_a_face_layout_uses_south_for_a_and_east_for_b() {
        let (a, b) = GamepadFaceLayout::SouthAEastB.face_buttons();

        assert_eq!(a, GamepadButtonBinding::South);
        assert_eq!(b, GamepadButtonBinding::East);
    }

    #[test]
    fn shoulder_bindings_map_to_the_expected_sdl_buttons() {
        assert_eq!(
            sdl_button_for_binding(GamepadButtonBinding::LeftShoulder),
            Button::LeftShoulder
        );
        assert_eq!(
            sdl_button_for_binding(GamepadButtonBinding::RightShoulder),
            Button::RightShoulder
        );
    }

    #[test]
    fn axis_direction_state_uses_hysteresis_before_releasing() {
        assert_eq!(axis_direction_state(17_000, false, false), (false, true));
        assert_eq!(axis_direction_state(13_000, false, true), (false, true));
        assert_eq!(axis_direction_state(11_000, false, true), (false, false));
    }

    #[test]
    fn effective_input_keeps_direction_pressed_while_dpad_and_stick_overlap() {
        let mut input_state = super::FrontendInputState::new();

        input_state
            .gamepad_buttons
            .set_pressed(JoypadButton::Left, true);
        input_state
            .gamepad_left_stick
            .set_pressed(JoypadButton::Left, true);
        assert!(input_state.is_effectively_pressed(JoypadButton::Left));

        input_state
            .gamepad_buttons
            .set_pressed(JoypadButton::Left, false);
        assert!(input_state.is_effectively_pressed(JoypadButton::Left));

        input_state
            .gamepad_left_stick
            .set_pressed(JoypadButton::Left, false);
        assert!(!input_state.is_effectively_pressed(JoypadButton::Left));
    }
}
