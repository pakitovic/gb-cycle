use super::{
    AppliedGamepadRumble, FrontendInputState, FrontendJoypadTarget, GAMEPAD_ACCELEROMETER_SENSORS,
    GAMEPAD_RUMBLE_REFRESH_INTERVAL, GamepadGyroState, GamepadManager, GamepadRumbleState,
    LeftStickDigitalState, SDL_STANDARD_GRAVITY_METERS_PER_SECOND_SQUARED,
    STRONG_GAMEPAD_RUMBLE_INTENSITY, WEAK_GAMEPAD_RUMBLE_INTENSITY, acceleration_to_milli_g,
    axis_direction_state, default_gamepad_name, format_gamepad_enumeration_error,
    format_open_gamepad_error, gamepad_button_binding_from_sdl_axis,
    gamepad_button_binding_from_sdl_button, gamepad_trigger_axis_for_binding,
    gamepad_trigger_axis_is_pressed, gamepad_trigger_axis_next_pressed, joystick_id_from_event,
    right_stick_axis_to_milli_g, rumble_intensity, sdl_button_for_binding,
};
use gb_core::{
    ConsoleModel, JoypadButton, Machine, MachineConfig, Mbc7AccelerometerInput, StartupMode,
    TraceSummaryBuffer,
};
use gb_desktop::{
    GamepadButtonBinding, GamepadFaceLayout, GamepadGyroMode, GamepadOptions, GamepadRumbleMode,
    PreferredGamepadIdentity,
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

fn latched_mbc7_accelerometer(machine: &mut Machine<TraceSummaryBuffer>) -> Mbc7AccelerometerInput {
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

        let joystick_id = unsafe { sdl3::sys::joystick::SDL_AttachVirtualJoystick(&descriptor) };
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

#[path = "test/bindings.rs"]
mod bindings;
#[path = "test/effective_state.rs"]
mod effective_state;
#[path = "test/gamepad_manager.rs"]
mod gamepad_manager;
#[path = "test/gamepad_mapping.rs"]
mod gamepad_mapping;
#[path = "test/gamepad_mbc7.rs"]
mod gamepad_mbc7;
#[path = "test/helpers.rs"]
mod helpers;
