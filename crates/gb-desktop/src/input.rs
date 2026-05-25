use gb_core::{JoypadButton, Machine, Mbc7AccelerometerInput, TraceSummaryBuffer};
use gb_desktop::{
    GamepadActionBindings, GamepadButtonBinding, GamepadButtonBindings, GamepadDirectionalSource,
    GamepadGyroMode, GamepadMenuBindings, GamepadOptions, GamepadRumbleMode,
    PreferredGamepadIdentity,
};
use sdl3::GamepadSubsystem;
use sdl3::event::Event;
use sdl3::gamepad::{Axis, Button, Gamepad};
use sdl3::joystick::JoystickId;
use sdl3::sensor::SensorType;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const LEFT_STICK_PRESS_THRESHOLD: i16 = 16_384;
const LEFT_STICK_RELEASE_THRESHOLD: i16 = 12_288;
const GAMEPAD_TRIGGER_PRESS_THRESHOLD: i16 = 16_384;
const GAMEPAD_TRIGGER_RELEASE_THRESHOLD: i16 = 12_288;
const GAMEPAD_RUMBLE_DURATION: Duration = Duration::from_millis(250);
const GAMEPAD_RUMBLE_REFRESH_INTERVAL: Duration = Duration::from_millis(125);
const STRONG_GAMEPAD_RUMBLE_INTENSITY: u16 = u16::MAX;
const WEAK_GAMEPAD_RUMBLE_INTENSITY: u16 = 0x6000;
const RIGHT_STICK_MBC7_MILLI_G_RANGE: i16 = 1_000;
const SDL_STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED: f32 = 9.80665;
const GAMEPAD_ACCELEROMETER_SENSORS: [SensorType; 3] = [
    SensorType::Accelerometer,
    SensorType::AccelerometerLeft,
    SensorType::AccelerometerRight,
];
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

    fn merged(states: [&SourceJoypadState; 3]) -> Self {
        let mut merged = Self::default();
        for button in JOYPAD_BUTTONS {
            let pressed = states.iter().any(|state| state.is_pressed(button));
            merged.set_pressed(button, pressed);
        }
        merged
    }

    fn sanitize_opposite_directions(&mut self) {
        sanitize_direction_pair(self, JoypadButton::Left, JoypadButton::Right);
        sanitize_direction_pair(self, JoypadButton::Up, JoypadButton::Down);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct LeftStickDigitalState {
    left: bool,
    right: bool,
    up: bool,
    down: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct AppliedGamepadRumble {
    joystick_id: JoystickId,
    low_frequency: u16,
    high_frequency: u16,
}

#[derive(Clone, Default)]
struct GamepadRumbleState {
    applied: Option<AppliedGamepadRumble>,
    next_refresh_at: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct GamepadAccelerometerSample {
    x: f32,
    y: f32,
}

#[derive(Clone, Default)]
struct GamepadGyroState {
    baseline: Option<GamepadAccelerometerSample>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontendJoypadTarget {
    Local,
    SgbPlayer(u8),
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

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    pub fn set_keyboard_button_for_target(
        &mut self,
        machine: &mut Machine<TraceSummaryBuffer>,
        target: FrontendJoypadTarget,
        button: JoypadButton,
        pressed: bool,
    ) {
        self.set_source_button(InputSource::Keyboard, machine, target, button, pressed);
    }

    pub fn set_gamepad_button(
        &mut self,
        machine: &mut Machine<TraceSummaryBuffer>,
        button: JoypadButton,
        pressed: bool,
    ) {
        self.set_gamepad_button_for_target(machine, FrontendJoypadTarget::Local, button, pressed);
    }

    pub fn set_gamepad_button_for_target(
        &mut self,
        machine: &mut Machine<TraceSummaryBuffer>,
        target: FrontendJoypadTarget,
        button: JoypadButton,
        pressed: bool,
    ) {
        self.set_source_button(
            InputSource::GamepadButtons,
            machine,
            target,
            button,
            pressed,
        );
    }

    pub fn set_gamepad_left_stick_button(
        &mut self,
        machine: &mut Machine<TraceSummaryBuffer>,
        button: JoypadButton,
        pressed: bool,
    ) {
        self.set_gamepad_left_stick_button_for_target(
            machine,
            FrontendJoypadTarget::Local,
            button,
            pressed,
        );
    }

    pub fn set_gamepad_left_stick_button_for_target(
        &mut self,
        machine: &mut Machine<TraceSummaryBuffer>,
        target: FrontendJoypadTarget,
        button: JoypadButton,
        pressed: bool,
    ) {
        self.set_source_button(
            InputSource::GamepadLeftStick,
            machine,
            target,
            button,
            pressed,
        );
    }

    pub fn clear_gamepad(&mut self, machine: &mut Machine<TraceSummaryBuffer>) {
        self.clear_gamepad_for_target(machine, FrontendJoypadTarget::Local);
    }

    pub fn clear_gamepad_for_target(
        &mut self,
        machine: &mut Machine<TraceSummaryBuffer>,
        target: FrontendJoypadTarget,
    ) {
        for button in JOYPAD_BUTTONS {
            self.set_gamepad_button_for_target(machine, target, button, false);
            self.set_gamepad_left_stick_button_for_target(machine, target, button, false);
        }
    }

    pub fn clear_keyboard_for_target(
        &mut self,
        machine: &mut Machine<TraceSummaryBuffer>,
        target: FrontendJoypadTarget,
    ) {
        for button in JOYPAD_BUTTONS {
            self.set_keyboard_button_for_target(machine, target, button, false);
        }
    }

    pub fn clear_all_for_target(
        &mut self,
        machine: &mut Machine<TraceSummaryBuffer>,
        target: FrontendJoypadTarget,
    ) {
        self.clear_keyboard_for_target(machine, target);
        self.clear_gamepad_for_target(machine, target);
        for button in JOYPAD_BUTTONS {
            apply_button_to_machine(machine, target, button, false);
        }
    }

    fn set_source_button(
        &mut self,
        source: InputSource,
        machine: &mut Machine<TraceSummaryBuffer>,
        target: FrontendJoypadTarget,
        button: JoypadButton,
        pressed: bool,
    ) {
        let previous_effective = self.effective_state();
        self.source_state_mut(source).set_pressed(button, pressed);
        let next_effective = self.effective_state();
        self.apply_effective_state_delta(machine, target, previous_effective, next_effective);
    }

    #[cfg(test)]
    fn is_effectively_pressed(&self, button: JoypadButton) -> bool {
        self.effective_state().is_pressed(button)
    }

    fn effective_state(&self) -> SourceJoypadState {
        let mut effective = SourceJoypadState::merged([
            &self.keyboard,
            &self.gamepad_buttons,
            &self.gamepad_left_stick,
        ]);
        effective.sanitize_opposite_directions();
        effective
    }

    fn apply_effective_state_delta(
        &self,
        machine: &mut Machine<TraceSummaryBuffer>,
        target: FrontendJoypadTarget,
        previous: SourceJoypadState,
        next: SourceJoypadState,
    ) {
        for button in JOYPAD_BUTTONS {
            let was_pressed = previous.is_pressed(button);
            let is_pressed = next.is_pressed(button);
            if was_pressed != is_pressed {
                apply_button_to_machine(machine, target, button, is_pressed);
            }
        }
    }

    fn source_state_mut(&mut self, source: InputSource) -> &mut SourceJoypadState {
        match source {
            InputSource::Keyboard => &mut self.keyboard,
            InputSource::GamepadButtons => &mut self.gamepad_buttons,
            InputSource::GamepadLeftStick => &mut self.gamepad_left_stick,
        }
    }
}

fn apply_button_to_machine(
    machine: &mut Machine<TraceSummaryBuffer>,
    target: FrontendJoypadTarget,
    button: JoypadButton,
    pressed: bool,
) {
    match target {
        FrontendJoypadTarget::Local => machine.set_joypad_button_pressed(button, pressed),
        FrontendJoypadTarget::SgbPlayer(player) => {
            machine.set_sgb_joypad_button_pressed(player, button, pressed);
        }
    }
}

pub struct GamepadManager {
    subsystem: GamepadSubsystem,
    options: GamepadOptions,
    opened: BTreeMap<JoystickId, OpenGamepad>,
    active: Option<JoystickId>,
    left_stick_state: LeftStickDigitalState,
    rumble: GamepadRumbleState,
    gyro: GamepadGyroState,
}

struct OpenGamepad {
    gamepad: Gamepad,
    name: String,
    path: Option<String>,
    supports_rumble: bool,
    accelerometer_sensor: Option<SensorType>,
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
            rumble: GamepadRumbleState::default(),
            gyro: GamepadGyroState::default(),
        };

        let mut gamepads = manager
            .subsystem
            .gamepads()
            .map_err(format_gamepad_enumeration_error)?;
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
                self.remove_gamepad(joystick_id_from_event(which), input_state, machine)?;
            }
            Event::ControllerDeviceRemapped { which, .. } => {
                if self.active == Some(joystick_id_from_event(which)) {
                    eprintln!("info: active SDL gamepad remapped");
                    self.gyro.baseline = None;
                    self.sync_accelerometer_sensor_enabled()?;
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
            .map_err(|error| format_open_gamepad_error(joystick_id, error))?;
        let opened_gamepad = OpenGamepad {
            path: gamepad.path(),
            name: match gamepad.name() {
                Some(name) => name,
                None => default_gamepad_name(joystick_id),
            },
            supports_rumble: unsafe { gamepad.has_rumble() },
            accelerometer_sensor: detect_gamepad_accelerometer_sensor(&gamepad),
            gamepad,
        };

        let previous_active = self.active;
        self.opened.insert(joystick_id, opened_gamepad);
        self.active = self.select_active_gamepad();
        if previous_active != self.active {
            self.gyro.baseline = None;
            self.sync_accelerometer_sensor_enabled()?;
        }

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
    ) -> Result<(), String> {
        let removed_name = self.opened.remove(&joystick_id).map(|gamepad| gamepad.name);
        let removed_name = match removed_name {
            Some(name) => name,
            None => default_gamepad_name(joystick_id),
        };
        eprintln!("info: SDL gamepad disconnected: {removed_name}");

        let previous_active = self.active;
        self.active = self.select_active_gamepad();
        if previous_active == self.active {
            return Ok(());
        }
        self.gyro.baseline = None;
        self.sync_accelerometer_sensor_enabled()?;

        if let Some(next_active) = self.active {
            let next_name = self
                .opened
                .get(&next_active)
                .map(|gamepad| gamepad.name.as_str())
                .unwrap_or("SDL gamepad");
            eprintln!("info: active SDL gamepad: {next_name}");
        }

        self.sync_active_gamepad_state(input_state, machine);
        Ok(())
    }

    pub fn poll_active_gamepad_state(
        &mut self,
        input_state: &mut FrontendInputState,
        machine: &mut Machine<TraceSummaryBuffer>,
    ) {
        let Some(active_joystick_id) = self.active else {
            set_mbc7_accelerometer_input_if_supported(machine, Mbc7AccelerometerInput::neutral());
            return;
        };
        let Some(gamepad) = self.opened.get(&active_joystick_id) else {
            set_mbc7_accelerometer_input_if_supported(machine, Mbc7AccelerometerInput::neutral());
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
        let right_x = gamepad.gamepad.axis(Axis::RightX);
        let right_y = gamepad.gamepad.axis(Axis::RightY);
        let accelerometer = if matches!(self.options.gyro_mode, GamepadGyroMode::PadGyro) {
            gamepad
                .accelerometer_sensor
                .and_then(|sensor| read_gamepad_accelerometer(&gamepad.gamepad, sensor).ok())
        } else {
            None
        };

        self.apply_polled_bound_buttons(bound_button_states, input_state, machine);
        self.apply_polled_directional_inputs(left_x, left_y, input_state, machine);
        self.apply_polled_mbc7_accelerometer_input(accelerometer, right_x, right_y, machine);
    }

    pub fn is_active_gamepad(&self, joystick_id: JoystickId) -> bool {
        self.active == Some(joystick_id)
    }

    pub fn active_gamepad_joystick_id(&self) -> Option<JoystickId> {
        self.active
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

    pub fn rumble_mode(&self) -> GamepadRumbleMode {
        self.options.rumble_mode
    }

    pub fn gyro_mode(&self) -> GamepadGyroMode {
        self.options.gyro_mode
    }

    pub fn button_bindings(&self) -> GamepadButtonBindings {
        self.options.bindings
    }

    pub fn action_bindings(&self) -> GamepadActionBindings {
        self.options.actions
    }

    pub fn menu_bindings(&self) -> GamepadMenuBindings {
        self.options.menu
    }

    pub fn active_gamepad_has_rumble(&self) -> bool {
        self.active_gamepad()
            .is_some_and(|gamepad| gamepad.supports_rumble)
    }

    pub fn active_gamepad_has_accelerometer(&self) -> bool {
        self.active_gamepad()
            .is_some_and(|gamepad| gamepad.accelerometer_sensor.is_some())
    }

    pub fn has_active_rumble_effect(&self) -> bool {
        self.rumble.applied.is_some()
    }

    pub fn can_deliver_rumble(&self) -> bool {
        self.active_gamepad_has_rumble()
            && !matches!(self.options.rumble_mode, GamepadRumbleMode::Off)
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

    pub fn set_action_bindings(&mut self, bindings: GamepadActionBindings) {
        self.options.actions = bindings;
    }

    pub fn set_menu_bindings(&mut self, bindings: GamepadMenuBindings) {
        self.options.menu = bindings;
    }

    pub fn set_rumble_mode(&mut self, rumble_mode: GamepadRumbleMode) {
        self.options.rumble_mode = rumble_mode;
    }

    pub fn set_gyro_mode(
        &mut self,
        gyro_mode: GamepadGyroMode,
        machine: &mut Machine<TraceSummaryBuffer>,
    ) -> Result<(), String> {
        if self.options.gyro_mode == gyro_mode {
            return Ok(());
        }

        self.options.gyro_mode = gyro_mode;
        self.gyro.baseline = None;
        self.sync_accelerometer_sensor_enabled()?;
        if matches!(gyro_mode, GamepadGyroMode::Off) {
            set_mbc7_accelerometer_input_if_supported(machine, Mbc7AccelerometerInput::neutral());
        }
        Ok(())
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
            self.gyro.baseline = None;
            self.sync_accelerometer_sensor_enabled_or_log();
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
        self.gyro.baseline = None;
        self.sync_accelerometer_sensor_enabled_or_log();
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

    pub fn update_rumble(&mut self, rumble_requested: bool, now: Instant) -> Result<(), String> {
        let desired = if rumble_requested {
            self.desired_rumble_effect()
        } else {
            None
        };
        let refresh_due = self
            .rumble
            .next_refresh_at
            .is_some_and(|deadline| now >= deadline);

        if desired == self.rumble.applied && !(refresh_due && desired.is_some()) {
            return Ok(());
        }

        match desired {
            Some(effect) => {
                if let Some(previous) = self.rumble.applied
                    && previous.joystick_id != effect.joystick_id
                {
                    let _ = self.apply_rumble(previous.joystick_id, 0, 0);
                }

                self.apply_rumble(
                    effect.joystick_id,
                    effect.low_frequency,
                    effect.high_frequency,
                )?;
                self.rumble.applied = Some(effect);
                self.rumble.next_refresh_at = Some(now + GAMEPAD_RUMBLE_REFRESH_INTERVAL);
            }
            None => {
                if let Some(previous) = self.rumble.applied {
                    let _ = self.apply_rumble(previous.joystick_id, 0, 0);
                }
                self.rumble.applied = None;
                self.rumble.next_refresh_at = None;
            }
        }

        Ok(())
    }

    fn sync_accelerometer_sensor_enabled(&mut self) -> Result<(), String> {
        for (joystick_id, gamepad) in &self.opened {
            let Some(sensor) = gamepad.accelerometer_sensor else {
                continue;
            };
            let enabled = self.active == Some(*joystick_id)
                && matches!(self.options.gyro_mode, GamepadGyroMode::PadGyro);
            if gamepad.gamepad.sensor_enabled(sensor) == enabled {
                continue;
            }
            gamepad
                .gamepad
                .sensor_set_enabled(sensor, enabled)
                .map_err(|error| {
                    let action = if enabled { "enable" } else { "disable" };
                    format!(
                        "failed to {action} SDL3 gamepad accelerometer for {}: {error}",
                        gamepad.name
                    )
                })?;
        }
        Ok(())
    }

    fn sync_accelerometer_sensor_enabled_or_log(&mut self) {
        if let Err(error) = self.sync_accelerometer_sensor_enabled() {
            eprintln!("warning: {error}");
        }
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

    fn desired_rumble_effect(&self) -> Option<AppliedGamepadRumble> {
        let active_joystick_id = self.active?;
        let active_gamepad = self.active_gamepad()?;
        let (low_frequency, high_frequency) = rumble_intensity(self.options.rumble_mode)?;
        if !active_gamepad.supports_rumble {
            return None;
        }

        Some(AppliedGamepadRumble {
            joystick_id: active_joystick_id,
            low_frequency,
            high_frequency,
        })
    }

    fn apply_rumble(
        &mut self,
        joystick_id: JoystickId,
        low_frequency: u16,
        high_frequency: u16,
    ) -> Result<(), String> {
        let Some(gamepad) = self.opened.get_mut(&joystick_id) else {
            return Ok(());
        };

        gamepad
            .gamepad
            .set_rumble(
                low_frequency,
                high_frequency,
                GAMEPAD_RUMBLE_DURATION.as_millis() as u32,
            )
            .map_err(|error| {
                format!(
                    "failed to set SDL3 gamepad rumble for {}: {error}",
                    gamepad.name
                )
            })
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

    fn apply_polled_mbc7_accelerometer_input(
        &mut self,
        accelerometer: Option<GamepadAccelerometerSample>,
        right_x: i16,
        right_y: i16,
        machine: &mut Machine<TraceSummaryBuffer>,
    ) {
        let input = match self.options.gyro_mode {
            GamepadGyroMode::Off => {
                self.gyro.baseline = None;
                Mbc7AccelerometerInput::neutral()
            }
            GamepadGyroMode::PadGyro => {
                let Some(accelerometer) = accelerometer else {
                    self.gyro.baseline = None;
                    set_mbc7_accelerometer_input_if_supported(
                        machine,
                        Mbc7AccelerometerInput::neutral(),
                    );
                    return;
                };
                let baseline = match self.gyro.baseline {
                    Some(baseline) => baseline,
                    None => {
                        self.gyro.baseline = Some(accelerometer);
                        accelerometer
                    }
                };
                Mbc7AccelerometerInput::from_milli_g(
                    acceleration_to_milli_g(accelerometer.x - baseline.x),
                    acceleration_to_milli_g(accelerometer.y - baseline.y),
                )
            }
            GamepadGyroMode::PadInput => {
                self.gyro.baseline = None;
                Mbc7AccelerometerInput::from_milli_g(
                    right_stick_axis_to_milli_g(right_x),
                    right_stick_axis_to_milli_g(right_y),
                )
            }
        };
        set_mbc7_accelerometer_input_if_supported(machine, input);
    }
}

fn gamepad_button_binding_state(gamepad: &OpenGamepad, binding: GamepadButtonBinding) -> bool {
    if let Some(button) = sdl_button_for_binding(binding) {
        return gamepad.gamepad.button(button);
    }

    gamepad_trigger_axis_for_binding(binding)
        .is_some_and(|axis| gamepad_trigger_axis_is_pressed(gamepad.gamepad.axis(axis)))
}

fn detect_gamepad_accelerometer_sensor(gamepad: &Gamepad) -> Option<SensorType> {
    GAMEPAD_ACCELEROMETER_SENSORS
        .into_iter()
        .find(|sensor| unsafe { gamepad.has_sensor(*sensor) })
}

fn read_gamepad_accelerometer(
    gamepad: &Gamepad,
    sensor: SensorType,
) -> Result<GamepadAccelerometerSample, String> {
    let mut data = [0.0_f32; 3];
    gamepad
        .sensor_get_data(sensor, &mut data)
        .map_err(|error| error.to_string())?;
    Ok(GamepadAccelerometerSample {
        x: data[0],
        y: data[1],
    })
}

fn set_mbc7_accelerometer_input_if_supported(
    machine: &mut Machine<TraceSummaryBuffer>,
    input: Mbc7AccelerometerInput,
) {
    if machine.has_mbc7_accelerometer() {
        let _ = machine.set_mbc7_accelerometer_input(input);
    }
}

fn right_stick_axis_to_milli_g(value: i16) -> i16 {
    ((i32::from(value) * i32::from(RIGHT_STICK_MBC7_MILLI_G_RANGE)) / i32::from(i16::MAX)).clamp(
        -i32::from(RIGHT_STICK_MBC7_MILLI_G_RANGE),
        i32::from(RIGHT_STICK_MBC7_MILLI_G_RANGE),
    ) as i16
}

fn acceleration_to_milli_g(value: f32) -> i16 {
    ((value / SDL_STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED) * 1_000.0)
        .round()
        .clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16
}

fn rumble_intensity(mode: GamepadRumbleMode) -> Option<(u16, u16)> {
    match mode {
        GamepadRumbleMode::Off => None,
        GamepadRumbleMode::Strong => Some((
            STRONG_GAMEPAD_RUMBLE_INTENSITY,
            STRONG_GAMEPAD_RUMBLE_INTENSITY,
        )),
        GamepadRumbleMode::Weak => {
            Some((WEAK_GAMEPAD_RUMBLE_INTENSITY, WEAK_GAMEPAD_RUMBLE_INTENSITY))
        }
    }
}

pub fn sdl_button_for_binding(binding: GamepadButtonBinding) -> Option<Button> {
    match binding {
        GamepadButtonBinding::South => Some(Button::South),
        GamepadButtonBinding::East => Some(Button::East),
        GamepadButtonBinding::West => Some(Button::West),
        GamepadButtonBinding::North => Some(Button::North),
        GamepadButtonBinding::Back => Some(Button::Back),
        GamepadButtonBinding::Start => Some(Button::Start),
        GamepadButtonBinding::Guide => Some(Button::Guide),
        GamepadButtonBinding::LeftShoulder => Some(Button::LeftShoulder),
        GamepadButtonBinding::RightShoulder => Some(Button::RightShoulder),
        GamepadButtonBinding::LeftTrigger | GamepadButtonBinding::RightTrigger => None,
        GamepadButtonBinding::LeftStickClick => Some(Button::LeftStick),
        GamepadButtonBinding::RightStickClick => Some(Button::RightStick),
        GamepadButtonBinding::DPadUp => Some(Button::DPadUp),
        GamepadButtonBinding::DPadDown => Some(Button::DPadDown),
        GamepadButtonBinding::DPadLeft => Some(Button::DPadLeft),
        GamepadButtonBinding::DPadRight => Some(Button::DPadRight),
        GamepadButtonBinding::Misc1 => Some(Button::Misc1),
    }
}

pub fn gamepad_trigger_axis_for_binding(binding: GamepadButtonBinding) -> Option<Axis> {
    match binding {
        GamepadButtonBinding::LeftTrigger => Some(Axis::TriggerLeft),
        GamepadButtonBinding::RightTrigger => Some(Axis::TriggerRight),
        GamepadButtonBinding::South
        | GamepadButtonBinding::East
        | GamepadButtonBinding::West
        | GamepadButtonBinding::North
        | GamepadButtonBinding::Back
        | GamepadButtonBinding::Start
        | GamepadButtonBinding::Guide
        | GamepadButtonBinding::LeftShoulder
        | GamepadButtonBinding::RightShoulder
        | GamepadButtonBinding::LeftStickClick
        | GamepadButtonBinding::RightStickClick
        | GamepadButtonBinding::DPadUp
        | GamepadButtonBinding::DPadDown
        | GamepadButtonBinding::DPadLeft
        | GamepadButtonBinding::DPadRight
        | GamepadButtonBinding::Misc1 => None,
    }
}

pub fn gamepad_button_binding_from_sdl_axis(axis: Axis) -> Option<GamepadButtonBinding> {
    match axis {
        Axis::TriggerLeft => Some(GamepadButtonBinding::LeftTrigger),
        Axis::TriggerRight => Some(GamepadButtonBinding::RightTrigger),
        Axis::LeftX | Axis::LeftY | Axis::RightX | Axis::RightY => None,
    }
}

pub fn gamepad_trigger_axis_is_pressed(value: i16) -> bool {
    value >= GAMEPAD_TRIGGER_PRESS_THRESHOLD
}

pub fn gamepad_trigger_axis_next_pressed(value: i16, was_pressed: bool) -> bool {
    if was_pressed {
        value >= GAMEPAD_TRIGGER_RELEASE_THRESHOLD
    } else {
        value >= GAMEPAD_TRIGGER_PRESS_THRESHOLD
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

fn sanitize_direction_pair(
    state: &mut SourceJoypadState,
    negative: JoypadButton,
    positive: JoypadButton,
) {
    if state.is_pressed(negative) && state.is_pressed(positive) {
        state.set_pressed(negative, false);
        state.set_pressed(positive, false);
    }
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

fn default_gamepad_name(joystick_id: JoystickId) -> String {
    format!("SDL gamepad {}", joystick_id.0)
}

fn format_gamepad_enumeration_error(error: sdl3::Error) -> String {
    format!("failed to enumerate SDL3 gamepads: {error}")
}

fn format_open_gamepad_error(joystick_id: JoystickId, error: sdl3::Error) -> String {
    format!("failed to open SDL3 gamepad {}: {error}", joystick_id.0)
}

#[cfg(test)]
mod tests {
    use super::{
        AppliedGamepadRumble, FrontendInputState, FrontendJoypadTarget,
        GAMEPAD_ACCELEROMETER_SENSORS, GAMEPAD_RUMBLE_REFRESH_INTERVAL, GamepadGyroState,
        GamepadManager, GamepadRumbleState, LeftStickDigitalState,
        SDL_STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED, STRONG_GAMEPAD_RUMBLE_INTENSITY,
        WEAK_GAMEPAD_RUMBLE_INTENSITY, acceleration_to_milli_g, axis_direction_state,
        default_gamepad_name, format_gamepad_enumeration_error, format_open_gamepad_error,
        gamepad_button_binding_from_sdl_axis, gamepad_button_binding_from_sdl_button,
        gamepad_trigger_axis_for_binding, gamepad_trigger_axis_is_pressed,
        gamepad_trigger_axis_next_pressed, joystick_id_from_event, right_stick_axis_to_milli_g,
        rumble_intensity, sdl_button_for_binding,
    };
    use gb_core::{
        ConsoleModel, JoypadButton, Machine, MachineConfig, Mbc7AccelerometerInput, StartupMode,
        TraceSummaryBuffer,
    };
    use gb_desktop::{
        GamepadButtonBinding, GamepadFaceLayout, GamepadGyroMode, GamepadOptions,
        GamepadRumbleMode, PreferredGamepadIdentity,
    };
    use sdl3::event::Event;
    use sdl3::gamepad::{Axis, Button};
    use sdl3::joystick::JoystickId;
    use sdl3::sensor::SensorType;
    use sdl3::{GamepadSubsystem, hint};
    use std::collections::BTreeMap;
    use std::ffi::CString;
    use std::time::{Duration, Instant};

    fn init_gamepad_subsystem() -> (sdl3::Sdl, GamepadSubsystem) {
        crate::configure_headless_sdl();
        assert!(hint::set("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1"));
        let sdl = sdl3::init().expect("failed to initialize SDL");
        let gamepad = sdl
            .gamepad()
            .expect("failed to initialize SDL gamepad subsystem");
        (sdl, gamepad)
    }

    #[test]
    fn default_gamepad_name_uses_the_sdl_identifier_suffix() {
        let joystick_id = JoystickId::from(sdl3::sys::joystick::SDL_JoystickID(7));

        assert_eq!(default_gamepad_name(joystick_id), "SDL gamepad 7");
    }

    #[test]
    fn gamepad_error_formatters_include_the_host_context() {
        sdl3::clear_error();
        sdl3::set_error("enumeration failed").expect("SDL test error should be writable");
        let enumeration = format_gamepad_enumeration_error(sdl3::get_error());
        assert!(enumeration.contains("failed to enumerate SDL3 gamepads"));
        assert!(enumeration.contains("enumeration failed"));

        sdl3::clear_error();
        sdl3::set_error("open failed").expect("SDL test error should be writable");
        let joystick_id = JoystickId::from(sdl3::sys::joystick::SDL_JoystickID(9));
        let open_error = format_open_gamepad_error(joystick_id, sdl3::get_error());
        assert!(open_error.contains("failed to open SDL3 gamepad 9"));
        assert!(open_error.contains("open failed"));
    }

    fn test_machine() -> Machine<TraceSummaryBuffer> {
        Machine::new_summary(
            MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
        )
    }

    fn mbc7_machine() -> Machine<TraceSummaryBuffer> {
        let mut machine = test_machine();
        machine
            .load_cartridge(build_mbc7_rom())
            .expect("MBC7 ROM should load");
        machine
    }

    fn build_mbc7_rom() -> Vec<u8> {
        const ROM_LEN: usize = 256 * 1024;
        const ENTRY_POINT_START: usize = 0x0100;
        const ENTRY_POINT_LEN: usize = 4;
        const NINTENDO_LOGO_START: usize = 0x0104;
        const NINTENDO_LOGO_LEN: usize = 48;
        const TITLE_START: usize = 0x0134;
        const CGB_FLAG_ADDRESS: usize = 0x0143;
        const SGB_FLAG_ADDRESS: usize = 0x0146;
        const CARTRIDGE_TYPE_ADDRESS: usize = 0x0147;
        const ROM_SIZE_ADDRESS: usize = 0x0148;
        const RAM_SIZE_ADDRESS: usize = 0x0149;

        let mut rom = vec![0xFF; ROM_LEN];
        rom[ENTRY_POINT_START..ENTRY_POINT_START + ENTRY_POINT_LEN]
            .copy_from_slice(&[0x00, 0xC3, 0x50, 0x01]);
        rom[NINTENDO_LOGO_START..NINTENDO_LOGO_START + NINTENDO_LOGO_LEN]
            .copy_from_slice(&[0xCE; NINTENDO_LOGO_LEN]);
        rom[TITLE_START..TITLE_START + 7].copy_from_slice(b"GBTEST1");
        rom[CGB_FLAG_ADDRESS] = 0x80;
        rom[SGB_FLAG_ADDRESS] = 0x03;
        rom[CARTRIDGE_TYPE_ADDRESS] = 0x22;
        rom[ROM_SIZE_ADDRESS] = 0x03;
        rom[RAM_SIZE_ADDRESS] = 0x00;

        for bank in 0..(ROM_LEN / 0x4000) {
            let start = bank * 0x4000;
            rom[start] = bank as u8;
            rom[start + 0x0100] = bank as u8;
        }

        rom
    }

    fn latched_mbc7_accelerometer(
        machine: &mut Machine<TraceSummaryBuffer>,
    ) -> Mbc7AccelerometerInput {
        machine.write_bus(0x0000, 0x0A);
        machine.write_bus(0x4000, 0x40);
        machine.write_bus(0xA000, 0x55);
        machine.write_bus(0xA010, 0xAA);
        Mbc7AccelerometerInput::from_raw(
            u16::from(machine.read_bus(0xA020)) | (u16::from(machine.read_bus(0xA030)) << 8),
            u16::from(machine.read_bus(0xA040)) | (u16::from(machine.read_bus(0xA050)) << 8),
        )
    }

    fn pressed_mask(machine: &Machine<TraceSummaryBuffer>) -> u8 {
        machine.joypad().snapshot().pressed_mask
    }

    fn ingest_host_input(machine: &mut Machine<TraceSummaryBuffer>) {
        machine.step_t_cycle();
    }

    fn joypad_mask(button: JoypadButton) -> u8 {
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

    struct VirtualGamepad {
        joystick_id: JoystickId,
        raw: *mut sdl3::sys::joystick::SDL_Joystick,
        _name: CString,
        _sensors: Vec<sdl3::sys::joystick::SDL_VirtualJoystickSensorDesc>,
    }

    impl VirtualGamepad {
        fn attach(name: &str) -> Self {
            Self::attach_with_sensors(name, Vec::new())
        }

        fn attach_with_accelerometer(name: &str) -> Self {
            Self::attach_with_sensors(
                name,
                vec![sdl3::sys::joystick::SDL_VirtualJoystickSensorDesc {
                    r#type: sdl3::sys::sensor::SDL_SensorType::ACCEL,
                    rate: 60.0,
                }],
            )
        }

        fn attach_with_sensors(
            name: &str,
            sensors: Vec<sdl3::sys::joystick::SDL_VirtualJoystickSensorDesc>,
        ) -> Self {
            let name = CString::new(name).expect("virtual gamepad name");
            let mut descriptor = sdl3::sys::joystick::SDL_VirtualJoystickDesc::new();
            descriptor.r#type = sdl3::sys::joystick::SDL_JOYSTICK_TYPE_GAMEPAD.0 as u16;
            descriptor.naxes = 6;
            descriptor.nbuttons = 16;
            descriptor.nsensors = sensors.len() as u16;
            descriptor.button_mask = (1 << Button::South as u32)
                | (1 << Button::East as u32)
                | (1 << Button::Back as u32)
                | (1 << Button::Start as u32)
                | (1 << Button::DPadUp as u32)
                | (1 << Button::DPadDown as u32)
                | (1 << Button::DPadLeft as u32)
                | (1 << Button::DPadRight as u32);
            descriptor.axis_mask = (1 << Axis::LeftX as u32)
                | (1 << Axis::LeftY as u32)
                | (1 << Axis::RightX as u32)
                | (1 << Axis::RightY as u32)
                | (1 << Axis::TriggerLeft as u32)
                | (1 << Axis::TriggerRight as u32);
            descriptor.name = name.as_ptr();
            descriptor.sensors = sensors.as_ptr();

            let joystick_id =
                unsafe { sdl3::sys::joystick::SDL_AttachVirtualJoystick(&descriptor) };
            assert_ne!(joystick_id.0, 0, "failed to attach a virtual SDL gamepad");
            let raw = unsafe { sdl3::sys::joystick::SDL_OpenJoystick(joystick_id) };
            assert!(!raw.is_null(), "failed to open the virtual SDL gamepad");

            Self {
                joystick_id,
                raw,
                _name: name,
                _sensors: sensors,
            }
        }

        fn set_button(&self, button: Button, pressed: bool) {
            assert!(unsafe {
                sdl3::sys::joystick::SDL_SetJoystickVirtualButton(self.raw, button as i32, pressed)
            });
        }

        fn set_axis(&self, axis: Axis, value: i16) {
            assert!(unsafe {
                sdl3::sys::joystick::SDL_SetJoystickVirtualAxis(self.raw, axis as i32, value)
            });
        }

        fn set_accelerometer(&self, x: f32, y: f32, z: f32) {
            let data = [x, y, z];
            assert!(unsafe {
                sdl3::sys::joystick::SDL_SendJoystickVirtualSensorData(
                    self.raw,
                    sdl3::sys::sensor::SDL_SensorType::ACCEL,
                    0,
                    data.as_ptr(),
                    data.len() as i32,
                )
            });
        }
    }

    impl Drop for VirtualGamepad {
        fn drop(&mut self) {
            unsafe {
                sdl3::sys::joystick::SDL_CloseJoystick(self.raw);
                assert!(sdl3::sys::joystick::SDL_DetachVirtualJoystick(
                    self.joystick_id
                ));
            }
        }
    }

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
            Some(Button::LeftShoulder)
        );
        assert_eq!(
            sdl_button_for_binding(GamepadButtonBinding::RightShoulder),
            Some(Button::RightShoulder)
        );
    }

    #[test]
    fn trigger_bindings_map_to_the_expected_sdl_axes() {
        assert_eq!(
            gamepad_trigger_axis_for_binding(GamepadButtonBinding::LeftTrigger),
            Some(Axis::TriggerLeft)
        );
        assert_eq!(
            gamepad_trigger_axis_for_binding(GamepadButtonBinding::RightTrigger),
            Some(Axis::TriggerRight)
        );
        assert_eq!(
            gamepad_button_binding_from_sdl_axis(Axis::TriggerLeft),
            Some(GamepadButtonBinding::LeftTrigger)
        );
        assert_eq!(
            gamepad_button_binding_from_sdl_axis(Axis::TriggerRight),
            Some(GamepadButtonBinding::RightTrigger)
        );
        assert_eq!(gamepad_button_binding_from_sdl_axis(Axis::LeftX), None);
        assert_eq!(
            sdl_button_for_binding(GamepadButtonBinding::LeftTrigger),
            None
        );
        assert!(gamepad_trigger_axis_next_pressed(17_000, false));
        assert!(gamepad_trigger_axis_next_pressed(13_000, true));
        assert!(!gamepad_trigger_axis_next_pressed(11_000, true));
        assert!(gamepad_trigger_axis_is_pressed(i16::MAX));
        assert!(!gamepad_trigger_axis_is_pressed(0));
    }

    #[test]
    fn axis_direction_state_uses_hysteresis_before_releasing() {
        assert_eq!(axis_direction_state(17_000, false, false), (false, true));
        assert_eq!(axis_direction_state(13_000, false, true), (false, true));
        assert_eq!(axis_direction_state(11_000, false, true), (false, false));
    }

    #[test]
    fn mbc7_gyro_helpers_map_host_units_to_milli_g() {
        assert_eq!(right_stick_axis_to_milli_g(0), 0);
        assert_eq!(right_stick_axis_to_milli_g(i16::MAX), 1_000);
        assert_eq!(right_stick_axis_to_milli_g(i16::MIN), -1_000);
        assert_eq!(
            acceleration_to_milli_g(SDL_STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED),
            1_000
        );
        assert_eq!(
            acceleration_to_milli_g(-SDL_STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED),
            -1_000
        );
        assert_eq!(
            GAMEPAD_ACCELEROMETER_SENSORS,
            [
                SensorType::Accelerometer,
                SensorType::AccelerometerLeft,
                SensorType::AccelerometerRight
            ]
        );
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

    #[test]
    fn effective_input_neutralizes_opposite_horizontal_directions_until_the_conflict_clears() {
        let mut machine = test_machine();
        let mut input_state = FrontendInputState::new();

        input_state.set_keyboard_button_for_target(
            &mut machine,
            FrontendJoypadTarget::Local,
            JoypadButton::Left,
            true,
        );
        ingest_host_input(&mut machine);
        assert_eq!(pressed_mask(&machine), joypad_mask(JoypadButton::Left));

        input_state.set_gamepad_left_stick_button(&mut machine, JoypadButton::Right, true);
        ingest_host_input(&mut machine);
        assert_eq!(pressed_mask(&machine), 0);

        input_state.set_gamepad_left_stick_button(&mut machine, JoypadButton::Right, false);
        ingest_host_input(&mut machine);
        assert_eq!(pressed_mask(&machine), joypad_mask(JoypadButton::Left));
    }

    #[test]
    fn effective_input_neutralizes_opposite_gamepad_directions_between_dpad_and_left_stick() {
        let mut machine = test_machine();
        let mut input_state = FrontendInputState::new();

        input_state.set_gamepad_button(&mut machine, JoypadButton::Left, true);
        ingest_host_input(&mut machine);
        assert_eq!(pressed_mask(&machine), joypad_mask(JoypadButton::Left));

        input_state.set_gamepad_left_stick_button(&mut machine, JoypadButton::Right, true);
        ingest_host_input(&mut machine);
        assert_eq!(pressed_mask(&machine), 0);

        input_state.set_gamepad_left_stick_button(&mut machine, JoypadButton::Right, false);
        ingest_host_input(&mut machine);
        assert_eq!(pressed_mask(&machine), joypad_mask(JoypadButton::Left));
    }

    #[test]
    fn effective_input_neutralizes_opposite_vertical_directions_from_the_same_source() {
        let mut machine = test_machine();
        let mut input_state = FrontendInputState::new();

        input_state.set_keyboard_button_for_target(
            &mut machine,
            FrontendJoypadTarget::Local,
            JoypadButton::Up,
            true,
        );
        ingest_host_input(&mut machine);
        assert_eq!(pressed_mask(&machine), joypad_mask(JoypadButton::Up));

        input_state.set_keyboard_button_for_target(
            &mut machine,
            FrontendJoypadTarget::Local,
            JoypadButton::Down,
            true,
        );
        ingest_host_input(&mut machine);
        assert_eq!(pressed_mask(&machine), 0);

        input_state.set_keyboard_button_for_target(
            &mut machine,
            FrontendJoypadTarget::Local,
            JoypadButton::Up,
            false,
        );
        ingest_host_input(&mut machine);
        assert_eq!(pressed_mask(&machine), joypad_mask(JoypadButton::Down));
    }

    #[test]
    fn frontend_input_state_updates_machine_state_and_clears_each_source() {
        let mut machine = test_machine();
        let mut input_state = FrontendInputState::new();

        input_state.set_keyboard_button_for_target(
            &mut machine,
            FrontendJoypadTarget::Local,
            JoypadButton::A,
            true,
        );
        ingest_host_input(&mut machine);
        assert_eq!(pressed_mask(&machine), joypad_mask(JoypadButton::A));

        input_state.set_gamepad_button(&mut machine, JoypadButton::A, true);
        input_state.set_keyboard_button_for_target(
            &mut machine,
            FrontendJoypadTarget::Local,
            JoypadButton::A,
            false,
        );
        ingest_host_input(&mut machine);
        assert_eq!(pressed_mask(&machine), joypad_mask(JoypadButton::A));

        input_state.set_gamepad_left_stick_button(&mut machine, JoypadButton::Left, true);
        ingest_host_input(&mut machine);
        assert_eq!(
            pressed_mask(&machine),
            joypad_mask(JoypadButton::A) | joypad_mask(JoypadButton::Left)
        );

        input_state.clear_keyboard_for_target(&mut machine, FrontendJoypadTarget::Local);
        ingest_host_input(&mut machine);
        assert_eq!(
            pressed_mask(&machine),
            joypad_mask(JoypadButton::A) | joypad_mask(JoypadButton::Left)
        );

        input_state.clear_gamepad(&mut machine);
        ingest_host_input(&mut machine);
        assert_eq!(pressed_mask(&machine), 0);

        input_state.set_keyboard_button_for_target(
            &mut machine,
            FrontendJoypadTarget::Local,
            JoypadButton::Start,
            true,
        );
        input_state.set_gamepad_button(&mut machine, JoypadButton::B, true);
        input_state.clear_all_for_target(&mut machine, FrontendJoypadTarget::Local);
        ingest_host_input(&mut machine);
        assert_eq!(pressed_mask(&machine), 0);
    }

    #[test]
    fn clear_all_forces_machine_buttons_released_after_external_restore() {
        let mut machine = test_machine();
        let mut input_state = FrontendInputState::new();

        machine.set_joypad_button_pressed(JoypadButton::Right, true);
        ingest_host_input(&mut machine);
        assert_eq!(pressed_mask(&machine), joypad_mask(JoypadButton::Right));

        input_state.clear_all_for_target(&mut machine, FrontendJoypadTarget::Local);
        ingest_host_input(&mut machine);
        assert_eq!(pressed_mask(&machine), 0);
        assert!(!input_state.is_effectively_pressed(JoypadButton::Right));
    }

    #[test]
    fn gamepad_button_helpers_round_trip_supported_buttons() {
        for (binding, button) in [
            (GamepadButtonBinding::South, Button::South),
            (GamepadButtonBinding::East, Button::East),
            (GamepadButtonBinding::West, Button::West),
            (GamepadButtonBinding::North, Button::North),
            (GamepadButtonBinding::Back, Button::Back),
            (GamepadButtonBinding::Start, Button::Start),
            (GamepadButtonBinding::Guide, Button::Guide),
            (GamepadButtonBinding::LeftShoulder, Button::LeftShoulder),
            (GamepadButtonBinding::RightShoulder, Button::RightShoulder),
            (GamepadButtonBinding::LeftStickClick, Button::LeftStick),
            (GamepadButtonBinding::RightStickClick, Button::RightStick),
            (GamepadButtonBinding::DPadUp, Button::DPadUp),
            (GamepadButtonBinding::DPadDown, Button::DPadDown),
            (GamepadButtonBinding::DPadLeft, Button::DPadLeft),
            (GamepadButtonBinding::DPadRight, Button::DPadRight),
            (GamepadButtonBinding::Misc1, Button::Misc1),
        ] {
            assert_eq!(sdl_button_for_binding(binding), Some(button));
            assert_eq!(
                gamepad_button_binding_from_sdl_button(button),
                Some(binding)
            );
        }
        assert_eq!(
            gamepad_button_binding_from_sdl_button(Button::Touchpad),
            None
        );
        assert_eq!(joystick_id_from_event(77).0, 77);
        assert_eq!(rumble_intensity(GamepadRumbleMode::Off), None);
        assert_eq!(
            rumble_intensity(GamepadRumbleMode::Strong),
            Some((
                STRONG_GAMEPAD_RUMBLE_INTENSITY,
                STRONG_GAMEPAD_RUMBLE_INTENSITY,
            ))
        );
        assert_eq!(
            rumble_intensity(GamepadRumbleMode::Weak),
            Some((WEAK_GAMEPAD_RUMBLE_INTENSITY, WEAK_GAMEPAD_RUMBLE_INTENSITY))
        );
    }

    #[test]
    fn gamepad_manager_rumble_helpers_cover_state_transitions() {
        let _guard = crate::lock_sdl_test();
        let (_sdl, subsystem) = init_gamepad_subsystem();
        let virtual_gamepad = VirtualGamepad::attach("Rumble Pad");
        subsystem.update();

        let mut machine = test_machine();
        let mut input_state = FrontendInputState::new();
        let options = GamepadOptions {
            preferred_device: PreferredGamepadIdentity {
                path: None,
                name: Some("Rumble Pad".to_string()),
            },
            ..GamepadOptions::default()
        };
        let mut manager = GamepadManager::new(&subsystem, options, &mut input_state, &mut machine)
            .expect("gamepad manager");

        assert_eq!(manager.rumble_mode(), GamepadRumbleMode::Strong);
        assert!(!manager.active_gamepad_has_rumble());
        assert!(!manager.has_active_rumble_effect());
        assert!(!manager.can_deliver_rumble());

        manager.set_rumble_mode(GamepadRumbleMode::Weak);
        assert_eq!(manager.rumble_mode(), GamepadRumbleMode::Weak);
        manager
            .opened
            .get_mut(&virtual_gamepad.joystick_id)
            .expect("virtual gamepad should be opened")
            .supports_rumble = true;
        assert!(manager.active_gamepad_has_rumble());
        assert!(manager.can_deliver_rumble());

        let desired = manager
            .desired_rumble_effect()
            .expect("active rumble effect should be derived");
        assert_eq!(desired.joystick_id.0, virtual_gamepad.joystick_id.0);
        assert_eq!(
            (desired.low_frequency, desired.high_frequency),
            (WEAK_GAMEPAD_RUMBLE_INTENSITY, WEAK_GAMEPAD_RUMBLE_INTENSITY)
        );

        let now = Instant::now();
        let future_refresh = now + Duration::from_secs(1);
        manager.rumble.applied = Some(desired);
        manager.rumble.next_refresh_at = Some(future_refresh);
        manager
            .update_rumble(true, now)
            .expect("matching rumble state should be a no-op");
        let applied = manager
            .rumble
            .applied
            .expect("rumble state should remain applied");
        assert_eq!(applied.joystick_id.0, desired.joystick_id.0);
        assert_eq!(applied.low_frequency, desired.low_frequency);
        assert_eq!(applied.high_frequency, desired.high_frequency);
        assert_eq!(manager.rumble.next_refresh_at, Some(future_refresh));
        assert!(manager.has_active_rumble_effect());

        manager.rumble.applied = Some(AppliedGamepadRumble {
            joystick_id: joystick_id_from_event(9_999),
            low_frequency: 1,
            high_frequency: 2,
        });
        manager.rumble.next_refresh_at = Some(now);
        manager
            .update_rumble(false, now)
            .expect("clearing stale rumble should not require a live SDL gamepad");
        assert!(manager.rumble.applied.is_none());
        assert!(manager.rumble.next_refresh_at.is_none());
        assert!(!manager.has_active_rumble_effect());

        manager.set_rumble_mode(GamepadRumbleMode::Strong);
        let strong_effect = manager
            .desired_rumble_effect()
            .expect("strong rumble effect should be derived");
        assert_eq!(
            (strong_effect.low_frequency, strong_effect.high_frequency),
            (
                STRONG_GAMEPAD_RUMBLE_INTENSITY,
                STRONG_GAMEPAD_RUMBLE_INTENSITY
            )
        );
        let refresh_result =
            manager.update_rumble(true, future_refresh + GAMEPAD_RUMBLE_REFRESH_INTERVAL);
        if let Err(error) = refresh_result {
            assert!(error.contains("failed to set SDL3 gamepad rumble"));
        }

        manager
            .opened
            .get_mut(&virtual_gamepad.joystick_id)
            .expect("virtual gamepad should remain opened")
            .supports_rumble = false;
        assert!(!manager.active_gamepad_has_rumble());
        assert!(manager.desired_rumble_effect().is_none());
    }

    #[test]
    fn gamepad_manager_without_active_gamepad_keeps_mbc7_accelerometer_neutral() {
        let _guard = crate::lock_sdl_test();
        let (_sdl, subsystem) = init_gamepad_subsystem();

        let mut machine = mbc7_machine();
        machine
            .set_mbc7_accelerometer_input(Mbc7AccelerometerInput::from_milli_g(1_000, 1_000))
            .expect("MBC7 accelerometer should be writable");
        let mut input_state = FrontendInputState::new();
        let options = GamepadOptions {
            gyro_mode: GamepadGyroMode::PadInput,
            ..GamepadOptions::default()
        };
        let mut manager = GamepadManager {
            subsystem,
            options,
            opened: BTreeMap::new(),
            active: None,
            left_stick_state: LeftStickDigitalState::default(),
            rumble: GamepadRumbleState::default(),
            gyro: GamepadGyroState::default(),
        };
        manager.sync_active_gamepad_state(&mut input_state, &mut machine);

        assert!(!manager.has_connected_gamepad());
        assert_eq!(
            latched_mbc7_accelerometer(&mut machine),
            Mbc7AccelerometerInput::neutral()
        );

        machine
            .set_mbc7_accelerometer_input(Mbc7AccelerometerInput::from_milli_g(-1_000, -1_000))
            .expect("MBC7 accelerometer should be writable");
        manager.poll_active_gamepad_state(&mut input_state, &mut machine);
        assert_eq!(
            latched_mbc7_accelerometer(&mut machine),
            Mbc7AccelerometerInput::neutral()
        );
    }

    #[test]
    fn gamepad_manager_pad_input_feeds_mbc7_accelerometer_from_right_stick() {
        let _guard = crate::lock_sdl_test();
        let (_sdl, subsystem) = init_gamepad_subsystem();
        let virtual_gamepad = VirtualGamepad::attach("Tilt Stick");
        subsystem.update();

        let mut machine = mbc7_machine();
        let mut input_state = FrontendInputState::new();
        let options = GamepadOptions {
            gyro_mode: GamepadGyroMode::PadInput,
            preferred_device: PreferredGamepadIdentity {
                path: None,
                name: Some("Tilt Stick".to_string()),
            },
            ..GamepadOptions::default()
        };
        let mut manager = GamepadManager::new(&subsystem, options, &mut input_state, &mut machine)
            .expect("gamepad manager");

        virtual_gamepad.set_axis(Axis::RightX, i16::MAX);
        virtual_gamepad.set_axis(Axis::RightY, i16::MIN);
        subsystem.update();
        manager.poll_active_gamepad_state(&mut input_state, &mut machine);
        assert_eq!(
            latched_mbc7_accelerometer(&mut machine),
            Mbc7AccelerometerInput::from_milli_g(1_000, -1_000)
        );

        virtual_gamepad.set_axis(Axis::RightX, 0);
        virtual_gamepad.set_axis(Axis::RightY, 0);
        subsystem.update();
        manager.poll_active_gamepad_state(&mut input_state, &mut machine);
        assert_eq!(
            latched_mbc7_accelerometer(&mut machine),
            Mbc7AccelerometerInput::neutral()
        );
    }

    #[test]
    fn gamepad_manager_gyro_off_returns_mbc7_accelerometer_to_neutral() {
        let _guard = crate::lock_sdl_test();
        let (_sdl, subsystem) = init_gamepad_subsystem();
        let virtual_gamepad = VirtualGamepad::attach("Tilt Toggle");
        subsystem.update();

        let mut machine = mbc7_machine();
        let mut input_state = FrontendInputState::new();
        let options = GamepadOptions {
            gyro_mode: GamepadGyroMode::PadInput,
            preferred_device: PreferredGamepadIdentity {
                path: None,
                name: Some("Tilt Toggle".to_string()),
            },
            ..GamepadOptions::default()
        };
        let mut manager = GamepadManager::new(&subsystem, options, &mut input_state, &mut machine)
            .expect("gamepad manager");

        virtual_gamepad.set_axis(Axis::RightX, i16::MAX);
        subsystem.update();
        manager.poll_active_gamepad_state(&mut input_state, &mut machine);
        assert_ne!(
            latched_mbc7_accelerometer(&mut machine),
            Mbc7AccelerometerInput::neutral()
        );

        manager
            .set_gyro_mode(GamepadGyroMode::Off, &mut machine)
            .expect("gyro mode should change");
        assert_eq!(manager.gyro_mode(), GamepadGyroMode::Off);
        assert_eq!(
            latched_mbc7_accelerometer(&mut machine),
            Mbc7AccelerometerInput::neutral()
        );
    }

    #[test]
    fn gamepad_manager_pad_gyro_auto_centers_virtual_accelerometer() {
        let _guard = crate::lock_sdl_test();
        let (_sdl, subsystem) = init_gamepad_subsystem();
        let virtual_gamepad = VirtualGamepad::attach_with_accelerometer("Motion Pad");
        subsystem.update();

        let mut machine = mbc7_machine();
        let mut input_state = FrontendInputState::new();
        let options = GamepadOptions {
            preferred_device: PreferredGamepadIdentity {
                path: None,
                name: Some("Motion Pad".to_string()),
            },
            ..GamepadOptions::default()
        };
        let mut manager = GamepadManager::new(&subsystem, options, &mut input_state, &mut machine)
            .expect("gamepad manager");
        assert!(manager.active_gamepad_has_accelerometer());

        manager
            .set_gyro_mode(GamepadGyroMode::PadGyro, &mut machine)
            .expect("PAD GYRO should enable virtual accelerometer");
        virtual_gamepad.set_accelerometer(
            2.0 * SDL_STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
            -SDL_STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
            0.0,
        );
        subsystem.update();
        manager.poll_active_gamepad_state(&mut input_state, &mut machine);
        assert_eq!(
            latched_mbc7_accelerometer(&mut machine),
            Mbc7AccelerometerInput::neutral()
        );
        assert!(manager.gyro.baseline.is_some());

        virtual_gamepad.set_accelerometer(
            3.0 * SDL_STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
            -2.0 * SDL_STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
            0.0,
        );
        subsystem.update();
        manager.poll_active_gamepad_state(&mut input_state, &mut machine);
        assert_eq!(
            latched_mbc7_accelerometer(&mut machine),
            Mbc7AccelerometerInput::from_milli_g(1_000, -1_000)
        );

        manager
            .set_gyro_mode(GamepadGyroMode::Off, &mut machine)
            .expect("gyro off should disable virtual accelerometer");
        manager
            .set_gyro_mode(GamepadGyroMode::PadGyro, &mut machine)
            .expect("gyro on should request a new baseline");
        assert!(manager.gyro.baseline.is_none());
    }

    #[test]
    fn gamepad_manager_active_gamepad_change_resets_pad_gyro_baseline() {
        let _guard = crate::lock_sdl_test();
        let (_sdl, subsystem) = init_gamepad_subsystem();
        let first = VirtualGamepad::attach_with_accelerometer("First Motion");
        let second = VirtualGamepad::attach_with_accelerometer("Second Motion");
        subsystem.update();

        let mut machine = mbc7_machine();
        let mut input_state = FrontendInputState::new();
        let options = GamepadOptions {
            gyro_mode: GamepadGyroMode::PadGyro,
            ..GamepadOptions::default()
        };
        let mut manager = GamepadManager::new(&subsystem, options, &mut input_state, &mut machine)
            .expect("gamepad manager");

        first.set_accelerometer(SDL_STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED, 0.0, 0.0);
        second.set_accelerometer(0.0, SDL_STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED, 0.0);
        subsystem.update();
        manager.poll_active_gamepad_state(&mut input_state, &mut machine);
        assert!(manager.gyro.baseline.is_some());

        let next_active = if manager.is_active_gamepad(first.joystick_id) {
            second.joystick_id
        } else {
            first.joystick_id
        };
        assert!(manager.activate_gamepad_from_input(next_active, &mut input_state, &mut machine));
        assert_eq!(
            latched_mbc7_accelerometer(&mut machine),
            Mbc7AccelerometerInput::neutral()
        );
        assert!(manager.gyro.baseline.is_some());
    }

    #[test]
    fn gamepad_manager_polls_virtual_gamepad_buttons_and_left_stick() {
        let _guard = crate::lock_sdl_test();
        let (_sdl, subsystem) = init_gamepad_subsystem();
        let virtual_gamepad = VirtualGamepad::attach("Player One");
        subsystem.update();

        let mut machine = test_machine();
        let mut input_state = FrontendInputState::new();
        let options = GamepadOptions {
            preferred_device: PreferredGamepadIdentity {
                path: None,
                name: Some("Player One".to_string()),
            },
            ..GamepadOptions::default()
        };
        let mut manager = GamepadManager::new(&subsystem, options, &mut input_state, &mut machine)
            .expect("gamepad manager");

        assert!(manager.has_connected_gamepad());
        assert_eq!(manager.active_gamepad_name(), Some("Player One"));
        assert_eq!(
            manager.active_gamepad_identity(),
            Some(PreferredGamepadIdentity {
                path: None,
                name: Some("Player One".to_string()),
            })
        );

        virtual_gamepad.set_button(Button::East, true);
        virtual_gamepad.set_button(Button::DPadLeft, true);
        virtual_gamepad.set_button(Button::Start, true);
        virtual_gamepad.set_axis(Axis::LeftY, -20_000);
        subsystem.update();
        manager.poll_active_gamepad_state(&mut input_state, &mut machine);
        ingest_host_input(&mut machine);

        assert_eq!(
            pressed_mask(&machine),
            joypad_mask(JoypadButton::A)
                | joypad_mask(JoypadButton::Left)
                | joypad_mask(JoypadButton::Up)
        );

        manager.set_directional_source(
            gb_desktop::GamepadDirectionalSource::DpadOnly,
            &mut input_state,
            &mut machine,
        );
        ingest_host_input(&mut machine);
        assert_eq!(
            manager.directional_source(),
            gb_desktop::GamepadDirectionalSource::DpadOnly
        );
        assert_eq!(
            pressed_mask(&machine),
            joypad_mask(JoypadButton::A) | joypad_mask(JoypadButton::Left)
        );

        let mut bindings = manager.button_bindings();
        bindings.a = GamepadButtonBinding::South;
        manager.set_button_bindings(bindings, &mut input_state, &mut machine);
        virtual_gamepad.set_button(Button::South, true);
        subsystem.update();
        manager.poll_active_gamepad_state(&mut input_state, &mut machine);
        ingest_host_input(&mut machine);
        assert!(pressed_mask(&machine) & joypad_mask(JoypadButton::A) != 0);

        let mut bindings = manager.button_bindings();
        bindings.b = GamepadButtonBinding::LeftTrigger;
        manager.set_button_bindings(bindings, &mut input_state, &mut machine);
        virtual_gamepad.set_axis(Axis::TriggerLeft, i16::MAX);
        subsystem.update();
        manager.poll_active_gamepad_state(&mut input_state, &mut machine);
        ingest_host_input(&mut machine);
        assert!(pressed_mask(&machine) & joypad_mask(JoypadButton::B) != 0);
        virtual_gamepad.set_axis(Axis::TriggerLeft, 0);
        subsystem.update();
        manager.poll_active_gamepad_state(&mut input_state, &mut machine);
        ingest_host_input(&mut machine);
        assert_eq!(pressed_mask(&machine) & joypad_mask(JoypadButton::B), 0);

        manager.set_menu_bindings(gb_desktop::GamepadMenuBindings {
            confirm: GamepadButtonBinding::North,
            ..manager.menu_bindings()
        });
        assert_eq!(manager.menu_bindings().confirm, GamepadButtonBinding::North);
    }

    #[test]
    fn gamepad_manager_respects_preferred_devices_and_handle_event_transitions() {
        let _guard = crate::lock_sdl_test();
        let (_sdl, subsystem) = init_gamepad_subsystem();
        let first = VirtualGamepad::attach("First Pad");
        let second = VirtualGamepad::attach("Second Pad");
        subsystem.update();

        let mut machine = test_machine();
        let mut input_state = FrontendInputState::new();
        let options = GamepadOptions {
            preferred_device: PreferredGamepadIdentity {
                path: None,
                name: Some("Second Pad".to_string()),
            },
            ..GamepadOptions::default()
        };
        let mut manager = GamepadManager::new(&subsystem, options, &mut input_state, &mut machine)
            .expect("manager");

        assert_eq!(manager.active_gamepad_name(), Some("Second Pad"));
        assert_eq!(
            manager.preferred_device().name.as_deref(),
            Some("Second Pad")
        );
        assert!(manager.active_matches_preferred());
        assert_eq!(manager.preferred_device_name(), Some("Second Pad"));
        assert!(!manager.activate_gamepad_from_input(
            first.joystick_id,
            &mut input_state,
            &mut machine
        ));

        manager.set_preferred_device(
            PreferredGamepadIdentity::default(),
            &mut input_state,
            &mut machine,
        );
        assert!(!manager.active_matches_preferred());
        let activated_joystick = if manager.is_active_gamepad(first.joystick_id) {
            second.joystick_id
        } else {
            first.joystick_id
        };
        assert!(manager.activate_gamepad_from_input(
            activated_joystick,
            &mut input_state,
            &mut machine
        ));
        assert!(manager.is_active_gamepad(activated_joystick));

        let activated_gamepad = if activated_joystick == first.joystick_id {
            &first
        } else {
            &second
        };
        activated_gamepad.set_button(Button::East, true);
        subsystem.update();
        manager.poll_active_gamepad_state(&mut input_state, &mut machine);
        ingest_host_input(&mut machine);
        assert!(pressed_mask(&machine) & joypad_mask(JoypadButton::A) != 0);

        manager
            .handle_event(
                &Event::ControllerDeviceRemapped {
                    timestamp: 0,
                    which: activated_joystick.0,
                },
                &mut input_state,
                &mut machine,
            )
            .expect("remap event");

        manager
            .handle_event(
                &Event::ControllerDeviceRemoved {
                    timestamp: 0,
                    which: activated_joystick.0,
                },
                &mut input_state,
                &mut machine,
            )
            .expect("remove event");
        ingest_host_input(&mut machine);
        assert!(!manager.is_active_gamepad(activated_joystick));
        assert!(manager.has_connected_gamepad());
        assert_eq!(pressed_mask(&machine), 0);
    }

    #[test]
    fn gamepad_manager_can_open_new_virtual_devices_from_added_events() {
        let _guard = crate::lock_sdl_test();
        let (_sdl, subsystem) = init_gamepad_subsystem();
        let mut machine = test_machine();
        let mut input_state = FrontendInputState::new();
        let options = GamepadOptions {
            preferred_device: PreferredGamepadIdentity {
                path: None,
                name: Some("Hot Plugged".to_string()),
            },
            ..GamepadOptions::default()
        };
        let mut manager = GamepadManager::new(&subsystem, options, &mut input_state, &mut machine)
            .expect("gamepad manager");

        let added = VirtualGamepad::attach("Hot Plugged");
        subsystem.update();
        manager
            .handle_event(
                &Event::ControllerDeviceAdded {
                    timestamp: 0,
                    which: added.joystick_id.0,
                },
                &mut input_state,
                &mut machine,
            )
            .expect("added event");

        assert!(manager.has_connected_gamepad());
        assert_eq!(manager.active_gamepad_name(), Some("Hot Plugged"));
        assert!(manager.active_matches_preferred());
    }

    #[test]
    fn gamepad_manager_added_event_can_keep_the_existing_active_device() {
        let _guard = crate::lock_sdl_test();
        let (_sdl, subsystem) = init_gamepad_subsystem();
        let first = VirtualGamepad::attach("First Pad");
        subsystem.update();

        let mut machine = test_machine();
        let mut input_state = FrontendInputState::new();
        let mut manager = GamepadManager::new(
            &subsystem,
            GamepadOptions::default(),
            &mut input_state,
            &mut machine,
        )
        .expect("gamepad manager");
        let active_before = manager.active.expect("active SDL gamepad");
        let active_name_before = manager.active_gamepad_name().map(str::to_owned);

        let second = VirtualGamepad::attach("Second Pad");
        subsystem.update();
        manager
            .handle_event(
                &Event::ControllerDeviceAdded {
                    timestamp: 0,
                    which: second.joystick_id.0,
                },
                &mut input_state,
                &mut machine,
            )
            .expect("added event");

        assert!(manager.has_connected_gamepad());
        assert!(manager.is_active_gamepad(active_before));
        assert_eq!(manager.active_gamepad_name(), active_name_before.as_deref());
        assert!(manager.opened.contains_key(&first.joystick_id));
        assert!(manager.opened.contains_key(&second.joystick_id));
    }

    #[test]
    fn gamepad_manager_remove_unknown_device_keeps_the_active_gamepad() {
        let _guard = crate::lock_sdl_test();
        let (_sdl, subsystem) = init_gamepad_subsystem();
        let first = VirtualGamepad::attach("First Pad");
        subsystem.update();

        let mut machine = test_machine();
        let mut input_state = FrontendInputState::new();
        let mut manager = GamepadManager::new(
            &subsystem,
            GamepadOptions::default(),
            &mut input_state,
            &mut machine,
        )
        .expect("gamepad manager");

        let active_before = manager.active.expect("active SDL gamepad");
        let active_name_before = manager.active_gamepad_name().map(str::to_owned);
        let unknown_joystick_id = (1..=10_000)
            .map(joystick_id_from_event)
            .find(|joystick_id| !manager.opened.contains_key(joystick_id))
            .expect("unused SDL joystick id");

        manager
            .handle_event(
                &Event::ControllerDeviceRemoved {
                    timestamp: 0,
                    which: unknown_joystick_id.0,
                },
                &mut input_state,
                &mut machine,
            )
            .expect("remove event");

        assert!(manager.has_connected_gamepad());
        assert!(manager.is_active_gamepad(active_before));
        assert_eq!(manager.active_gamepad_name(), active_name_before.as_deref());
        assert!(manager.opened.contains_key(&first.joystick_id));
    }

    #[test]
    fn gamepad_manager_can_match_a_preferred_device_by_path() {
        let _guard = crate::lock_sdl_test();
        let (_sdl, subsystem) = init_gamepad_subsystem();
        let first = VirtualGamepad::attach("Path Pad");
        subsystem.update();

        let mut machine = test_machine();
        let mut input_state = FrontendInputState::new();
        let mut manager = GamepadManager::new(
            &subsystem,
            GamepadOptions::default(),
            &mut input_state,
            &mut machine,
        )
        .expect("gamepad manager");
        manager
            .opened
            .get_mut(&first.joystick_id)
            .expect("virtual gamepad should be opened")
            .path = Some("/dev/input/path-pad".to_string());

        manager.set_preferred_device(
            PreferredGamepadIdentity {
                path: Some("/dev/input/path-pad".to_string()),
                name: None,
            },
            &mut input_state,
            &mut machine,
        );

        assert!(manager.active_matches_preferred());
        assert_eq!(
            manager.preferred_device().path.as_deref(),
            Some("/dev/input/path-pad")
        );
    }
}
