use super::{
    CAMERA_IMAGE_FILE_DIALOG_FILTERS, DesktopRunOptions, DesktopSettingsStore,
    GamepadActionBindingTarget, GamepadBindingTarget, GamepadMenuBindingTarget, HostRtcSync,
    HotkeyAction, KeyboardBindingTarget, KeyboardMenuBindingTarget, PathDialogResult,
    PerformanceHudSnapshot, PokemonMysteryGiftCode, PokemonMysteryGiftKind,
    PokemonPikachuColorGift, ROM_FILE_DIALOG_FILTERS, RewindHudSnapshot,
    assign_gamepad_action_binding, assign_gamepad_binding, assign_gamepad_menu_binding,
    assign_keyboard_binding, assign_keyboard_menu_binding,
    assignable_key_for_binding_target_from_key_event,
    assignable_key_for_binding_target_from_keycode,
    assignable_menu_key_for_binding_target_from_keycode, compact_recent_rom_label,
    desktop_key_from_key_event, desktop_key_from_keycode, desktop_key_from_scancode,
    desktop_key_scancode, entered_pc_ranges, gamepad_action_binding_target_for_binding,
    gamepad_action_for_binding, gamepad_action_for_button, gamepad_binding_target_for_binding,
    gamepad_menu_binding_target_for_binding, hotkey_binding_target_for_key,
    joypad_binding_target_for_key, keyboard_menu_binding_target_for_key, load_machine_state_slot,
    machine_state_actions_available, machine_state_slot_load_available, machine_state_slot_path,
    map_path_dialog_result, menu_input_for_gamepad_button, menu_input_for_key,
    next_audio_volume_percent, next_boot_rom_verification_mode, next_console_model,
    next_execution_mode, next_fast_forward_speed_multiplier, next_gamepad_directional_source,
    next_gamepad_gyro_mode, next_gamepad_rumble_mode, next_machine_state_slot, next_revision,
    next_save_flush_policy, next_sgb_video_standard, next_startup_mode, next_window_scale,
    parse_cgb_ir_optical_delay_t_cycles, parse_cgb_ir_trace_event_count,
    parse_cgb_ir_trace_trigger_addresses, parse_cgb_ir_trace_watch_addresses,
    parse_edge_trace_addresses, parse_edge_trace_event_count, parse_edge_trace_pc_ranges,
    parse_pc_watch_trace_event_count, parse_pc_watch_trace_ranges, parse_trace_capture_t_cycles,
    parse_watch_trace_addresses, parse_watch_trace_event_count, performance_window_title,
    render_desktop_cgb_ir_trace_record, render_desktop_edge_trace_record,
    render_desktop_pc_watch_trace_record, render_desktop_trace_record,
    render_desktop_watch_trace_record, run_desktop, save_machine_state_slot,
    watched_bus_value_change, watched_cpu_addresses, watched_pc_ranges,
};
use crate::audio_recording::DesktopAudioRecordingOptions;
use gb_benchmark::{
    BenchmarkCase, BenchmarkMode, BenchmarkModel, BenchmarkPalette, BenchmarkStartup,
    BenchmarkStimulusRuntime,
};
use gb_core::apu::{ApuOutputSnapshot, ApuStereoOutputSnapshot};
use gb_core::{
    Apu, ApuCh4DebugSnapshot, ApuCh4Nr43LfsrAction, ApuCh4Nr43LiveWriteCategory,
    ApuCh4Nr43LiveWriteTrace, ApuCh4Nr43PassKind, ApuCh4Nr43PassTrace, ApuRecordedChannel,
    ApuRecordedChannelMask, ApuRegisterWriteObservation, ApuRegisterWriteState, ApuSampleCapture,
    BootRomAssetKind, CartridgeDiagnostic, CartridgeDiagnosticSeverity, CartridgeMappedRomSource,
    CartridgeMappedRomWindow, CgbInfraredStatus, CgbSpeedMode, ConsoleModel, CpuAddressEvent,
    CpuAddressEventKind, CpuAddressUpdateDirection, CpuBusAccessKind, CpuBusActivitySnapshot,
    CpuExecutionState, DebugWramAddressSample, Dmg07Port, ExecutionMode,
    ExternalPortAttachmentKind, ExternalPortAttachmentSnapshot, HardwareRevision, JoypadButton,
    JoypadSnapshot, JoypadStatus, LinkedTopologyKind, Machine, MachineConfig, MachineStepRegion,
    PersistentCartState, PocketCameraFrame, PpuFramebufferLayerSource, PpuStepRegion,
    PpuVisibleOutputState, PrinterCommand, SerialTickTelemetry, SgbHostProfile, SgbVideoStandard,
    StartupMode, TraceSummaryBuffer,
};
use gb_desktop::{
    BootRomVerificationMode, DesktopConfig, DesktopConsoleModel, DesktopDisplayPalette,
    DesktopExternalPortSelection, DesktopFrameBlendingMode, DesktopKey, DesktopSaveFlushPolicy,
    GamepadButtonBinding, GamepadDirectionalSource, GamepadGyroMode, GamepadMenuBindings,
    GamepadRumbleMode, MenuKeyboardBindings, RewindOptions, SaveKeyPolicy,
};
use gb_persistence::{
    CartridgeSaveKey, FilesystemCartridgeSaveStore, decode_machine_save_state_envelope,
};
use sdl3::dialog::DialogError;
use sdl3::event::Event;
use sdl3::gamepad::{Axis, Button};
use sdl3::joystick::JoystickId;
use sdl3::keyboard::{Keycode, Mod, Scancode};
use sdl3::render::Canvas;
use sdl3::video::Window;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::ffi::CString;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
const ENTRY_POINT_START: usize = 0x0100;
const LOGO_START: usize = 0x0104;
const TITLE_START: usize = 0x0134;
const CGB_FLAG_ADDRESS: usize = 0x0143;
const SGB_FLAG_ADDRESS: usize = 0x0146;
const CARTRIDGE_TYPE_ADDRESS: usize = 0x0147;
const ROM_SIZE_ADDRESS: usize = 0x0148;
const RAM_SIZE_ADDRESS: usize = 0x0149;
const HEADER_CHECKSUM_ADDRESS: usize = 0x014D;

fn temp_test_root(prefix: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "gb-cycle-desktop-main-tests-{prefix}-{}-{id}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("stale temp root should be removable");
    }
    fs::create_dir_all(&root).expect("temp root should be creatable");
    root
}

fn run_with_large_test_stack(name: &str, f: impl FnOnce() + Send + 'static) {
    let handle = std::thread::Builder::new()
        .name(name.to_string())
        .stack_size(8 * 1024 * 1024)
        .spawn(f)
        .expect("large-stack test helper should spawn");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

fn build_test_rom(len: usize, cartridge_type: u8, rom_size_code: u8, ram_size_code: u8) -> Vec<u8> {
    let mut rom = vec![0xFF; len.max(HEADER_MINIMUM_ROM_LEN)];
    rom[0x0000] = 0x12;
    rom[ENTRY_POINT_START..ENTRY_POINT_START + 4].copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
    rom[LOGO_START..LOGO_START + 48].copy_from_slice(&[0xCE; 48]);
    rom[TITLE_START..TITLE_START + 8].copy_from_slice(b"DESKTOP!");
    rom[CGB_FLAG_ADDRESS] = 0x80;
    rom[SGB_FLAG_ADDRESS] = 0x03;
    rom[CARTRIDGE_TYPE_ADDRESS] = cartridge_type;
    rom[ROM_SIZE_ADDRESS] = rom_size_code;
    rom[RAM_SIZE_ADDRESS] = ram_size_code;
    rom[HEADER_CHECKSUM_ADDRESS] = 0x7F;
    rom
}

fn write_test_rom(root: &Path, name: &str) -> PathBuf {
    let rom_path = root.join(name);
    fs::write(&rom_path, build_test_rom(32 * 1024, 0x00, 0x00, 0x00))
        .expect("test ROM should be writable");
    rom_path
}

fn single_panel_render_input<'a>(
    framebuffer: &'a [u8],
    layer_sources: &'a [PpuFramebufferLayerSource],
) -> super::FramebufferRenderInput<'a> {
    super::FramebufferRenderInput {
        dimensions: super::FramebufferDimensions {
            width: super::FRAMEBUFFER_WIDTH,
            height: super::FRAMEBUFFER_HEIGHT,
        },
        panels: [
            Some(super::FramebufferPanelInput {
                dimensions: super::FramebufferDimensions {
                    width: super::FRAMEBUFFER_WIDTH,
                    height: super::FRAMEBUFFER_HEIGHT,
                },
                framebuffer,
                framebuffer_layer_sources: layer_sources,
                bgwin_framebuffer: framebuffer,
                backdrop_framebuffer: framebuffer,
                bgwin_framebuffer_layer_sources: layer_sources,
                display_palette: super::DMG_DISPLAY_PALETTE,
                cgb_framebuffer_rgb555: None,
                sgb_framebuffer_rgb555: None,
            }),
            None,
            None,
            None,
        ],
    }
}

fn write_cgb_test_rom(root: &Path, name: &str, cartridge_type: u8, ram_size_code: u8) -> PathBuf {
    let rom_path = root.join(name);
    fs::write(
        &rom_path,
        build_test_rom(32 * 1024, cartridge_type, 0x00, ram_size_code),
    )
    .expect("CGB test ROM should be writable");
    rom_path
}

fn build_stop_test_rom() -> Vec<u8> {
    let mut rom = build_test_rom(32 * 1024, 0x00, 0x00, 0x00);
    rom[ENTRY_POINT_START..ENTRY_POINT_START + 4].copy_from_slice(&[
        0x10, 0x00, // STOP + padding byte
        0x18, 0xFE, // JR $0102 if STOP wakes
    ]);
    rom
}

fn write_test_camera_rom(root: &Path, name: &str) -> PathBuf {
    let rom_path = root.join(name);
    fs::write(&rom_path, build_test_rom(1024 * 1024, 0xFC, 0x05, 0x04))
        .expect("Pocket Camera test ROM should be writable");
    rom_path
}

fn write_grayscale_png(root: &Path, name: &str, width: u32, height: u32, pixels: &[u8]) -> PathBuf {
    write_png(root, name, width, height, png::ColorType::Grayscale, pixels)
}

fn write_png(
    root: &Path,
    name: &str,
    width: u32,
    height: u32,
    color_type: png::ColorType,
    pixels: &[u8],
) -> PathBuf {
    let path = root.join(name);
    let file = fs::File::create(&path).expect("test PNG should be creatable");
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(color_type);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .expect("test PNG header should encode");
    writer
        .write_image_data(pixels)
        .expect("test PNG pixels should encode");
    path
}

fn capture_camera_tile_bytes(machine: &mut Machine<TraceSummaryBuffer>) -> [u8; 16] {
    machine.write_bus(0x4000, 0x10);
    machine.write_bus(0xA001, 0x80);
    machine.write_bus(0xA002, 0x03);
    machine.write_bus(0xA003, 0x00);
    machine.write_bus(0xA004, 0x00);
    for cell in 0..16 {
        let base = 0xA006 + cell * 3;
        machine.write_bus(base, 64);
        machine.write_bus(base + 1, 128);
        machine.write_bus(base + 2, 192);
    }
    machine.write_bus(0xA000, 0x01);

    let mut guard = 0;
    while machine.read_bus(0xA000) & 0x01 != 0 {
        machine.step_t_cycle();
        guard += 1;
        assert!(
            guard < 300_000,
            "Pocket Camera capture should finish within the timing budget"
        );
    }

    machine.write_bus(0x4000, 0x00);
    let mut bytes = [0_u8; 16];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = machine.read_bus(0xA100 + index as u16);
    }
    bytes
}

fn serial_transfer_byte(machine: &mut Machine<TraceSummaryBuffer>, outgoing_byte: u8) -> u8 {
    machine.write_bus(0xFF01, outgoing_byte);
    machine.write_bus(0xFF02, 0x81);

    while !matches!(
        machine.serial().transfer_state(),
        gb_core::SerialTransferState::Idle
    ) {
        machine.step_t_cycle();
    }

    machine.read_bus(0xFF01)
}

fn printer_packet(command: PrinterCommand, data: &[u8]) -> Vec<u8> {
    let mut packet = vec![
        0x88,
        0x33,
        command as u8,
        0x00,
        (data.len() & 0xFF) as u8,
        ((data.len() >> 8) & 0xFF) as u8,
    ];
    packet.extend_from_slice(data);
    let checksum = packet[2..]
        .iter()
        .fold(0u16, |sum, &byte| sum.wrapping_add(byte as u16));
    packet.push((checksum & 0xFF) as u8);
    packet.push((checksum >> 8) as u8);
    packet
}

fn send_printer_packet(
    machine: &mut Machine<TraceSummaryBuffer>,
    command: PrinterCommand,
    data: &[u8],
) -> Vec<u8> {
    printer_packet(command, data)
        .into_iter()
        .map(|byte| serial_transfer_byte(machine, byte))
        .collect()
}

fn run_print_sequence(machine: &mut Machine<TraceSummaryBuffer>) {
    let tile_row = vec![0xFF; 320];

    send_printer_packet(machine, PrinterCommand::Data, &tile_row);
    assert_eq!(serial_transfer_byte(machine, 0x00), 0x81);
    assert_eq!(serial_transfer_byte(machine, 0x00), 0x08);

    send_printer_packet(machine, PrinterCommand::Data, &[]);
    serial_transfer_byte(machine, 0x00);
    serial_transfer_byte(machine, 0x00);

    send_printer_packet(machine, PrinterCommand::Print, &[0x01, 0x13, 0xE4, 0x40]);
    assert_eq!(serial_transfer_byte(machine, 0x00), 0x81);
    assert_eq!(serial_transfer_byte(machine, 0x00), 0x08);

    send_printer_packet(machine, PrinterCommand::Status, &[]);
    serial_transfer_byte(machine, 0x00);
    assert_eq!(serial_transfer_byte(machine, 0x00), 0x06);

    send_printer_packet(machine, PrinterCommand::Status, &[]);
    serial_transfer_byte(machine, 0x00);
    assert_eq!(serial_transfer_byte(machine, 0x00), 0x04);
}

fn dmg_skip_boot_summary_machine() -> Machine<TraceSummaryBuffer> {
    Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    )
}

fn cgb_skip_boot_summary_machine() -> Machine<TraceSummaryBuffer> {
    Machine::new_summary(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    )
}

fn assert_dmg07_slot_port(
    session: &super::linked_session::DesktopEmulationSession,
    slot: super::PlayerSlot,
    expected_port: Dmg07Port,
) {
    let machine = session
        .machine_for_player_slot(slot)
        .unwrap_or_else(|| panic!("{} should map to an active DMG-07 machine", slot.label()));
    assert_eq!(
        machine.external_port().attachment_kind(),
        ExternalPortAttachmentKind::FourPlayerAdapterDmg07
    );
    assert_eq!(
        machine.external_port().snapshot().attachment,
        ExternalPortAttachmentSnapshot::FourPlayerAdapterDmg07 {
            port: expected_port,
            incoming_byte: None,
        }
    );
}

fn schedule_quit_event() -> thread::JoinHandle<()> {
    thread::spawn(|| {
        thread::sleep(Duration::from_millis(75));
        crate::configure_headless_sdl();
        let sdl = sdl3::init().expect("SDL should initialize for quit-event helper");
        let events = sdl
            .event()
            .expect("SDL event subsystem should initialize for quit-event helper");
        events
            .push_event(Event::Quit { timestamp: 0 })
            .expect("quit event should be pushable");
    })
}

fn push_key_event(events: &sdl3::EventSubsystem, keycode: Keycode, down: bool) {
    let scancode = desktop_key_from_keycode(keycode)
        .map(desktop_key_scancode)
        .unwrap_or_else(|| keycode_to_test_scancode(keycode));
    push_key_event_with_scancode(events, keycode, scancode, down);
}

fn push_key_event_with_scancode(
    events: &sdl3::EventSubsystem,
    keycode: Keycode,
    scancode: Scancode,
    down: bool,
) {
    let event = if down {
        Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: Some(keycode),
            scancode: Some(scancode),
            keymod: Mod::NOMOD,
            repeat: false,
            which: 0,
            raw: 0,
        }
    } else {
        Event::KeyUp {
            timestamp: 0,
            window_id: 0,
            keycode: Some(keycode),
            scancode: Some(scancode),
            keymod: Mod::NOMOD,
            repeat: false,
            which: 0,
            raw: 0,
        }
    };

    events
        .push_event(event)
        .expect("keyboard event should be pushable");
}

fn keycode_to_test_scancode(keycode: Keycode) -> Scancode {
    match keycode {
        Keycode::A => Scancode::A,
        Keycode::C => Scancode::C,
        Keycode::D => Scancode::D,
        Keycode::E => Scancode::E,
        Keycode::Q => Scancode::Q,
        Keycode::S => Scancode::S,
        Keycode::V => Scancode::V,
        Keycode::W => Scancode::W,
        _ => panic!("test keycode should map to a desktop key"),
    }
}

fn schedule_key_sequence(sequence: Vec<(Keycode, bool)>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(75));
        crate::configure_headless_sdl();
        let sdl = sdl3::init().expect("SDL should initialize for key-sequence helper");
        let events = sdl
            .event()
            .expect("SDL event subsystem should initialize for key-sequence helper");
        for (keycode, down) in sequence {
            push_key_event(&events, keycode, down);
            thread::sleep(Duration::from_millis(8));
        }
        events
            .push_event(Event::Quit { timestamp: 0 })
            .expect("quit event should be pushable");
    })
}

fn wait_for_profiled_counter_sample(counter: &mut super::PerformanceCounter) {
    for _ in 0..200 {
        counter.collect_emulation_profile_results();
        if counter.sample_profiled_frames > 0 {
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("timed out waiting for an async emulation profile sample");
}

struct VirtualGamepad {
    joystick_id: JoystickId,
    raw: *mut sdl3::sys::joystick::SDL_Joystick,
    _name: CString,
}

impl VirtualGamepad {
    fn attach(name: &str) -> Self {
        let name = CString::new(name).expect("virtual gamepad name");
        let mut descriptor = sdl3::sys::joystick::SDL_VirtualJoystickDesc::new();
        descriptor.r#type = sdl3::sys::joystick::SDL_JOYSTICK_TYPE_GAMEPAD.0 as u16;
        descriptor.naxes = 0;
        descriptor.nbuttons = 16;
        descriptor.button_mask = (1 << Button::Guide as u32)
            | (1 << Button::South as u32)
            | (1 << Button::East as u32)
            | (1 << Button::DPadDown as u32)
            | (1 << Button::North as u32);
        descriptor.name = name.as_ptr();

        let joystick_id = unsafe { sdl3::sys::joystick::SDL_AttachVirtualJoystick(&descriptor) };
        assert_ne!(joystick_id.0, 0, "failed to attach a virtual SDL gamepad");
        let raw = unsafe { sdl3::sys::joystick::SDL_OpenJoystick(joystick_id) };
        assert!(!raw.is_null(), "failed to open the virtual SDL gamepad");

        Self {
            joystick_id,
            raw,
            _name: name,
        }
    }

    fn set_button(&self, button: Button, down: bool) {
        let success = unsafe {
            sdl3::sys::joystick::SDL_SetJoystickVirtualButton(self.raw, button as i32, down)
        };
        assert!(success, "failed to update virtual SDL gamepad button");
        unsafe { sdl3::sys::joystick::SDL_UpdateJoysticks() };
    }
}

impl Drop for VirtualGamepad {
    fn drop(&mut self) {
        unsafe {
            sdl3::sys::joystick::SDL_CloseJoystick(self.raw);
            let _ = sdl3::sys::joystick::SDL_DetachVirtualJoystick(self.joystick_id);
        }
    }
}

#[allow(dead_code)]
struct FrontendHarness {
    root: PathBuf,
    settings_path: PathBuf,
    sdl: sdl3::Sdl,
    canvas: Canvas<Window>,
    event_pump: sdl3::EventPump,
    session: super::DesktopSession,
    machine: super::DesktopEmulationSession,
    runtime: super::FrontendRuntime,
    settings_store: DesktopSettingsStore,
    performance_counter: super::PerformanceCounter,
    frame_pacer: super::FramePacer,
    _gamepad_subsystem: Option<sdl3::GamepadSubsystem>,
}

#[allow(dead_code)]
impl FrontendHarness {
    fn new(name: &str, with_rom: bool, with_audio: bool, with_gamepad: bool) -> Self {
        crate::configure_headless_sdl();
        let root = temp_test_root(name);
        let settings_path = root.join("desktop-settings.toml");
        let mut config = DesktopConfig::default();
        config.boot_rom.verification = BootRomVerificationMode::Off;
        config.audio.enabled = with_audio;
        config.input.gamepad.enabled = with_gamepad;
        if with_gamepad {
            config.input.gamepad.preferred_device.name = Some("Saved Pad".to_string());
        }
        let boot_rom_root = root.join("bootroms");
        fs::create_dir_all(&boot_rom_root).expect("frontend harness boot ROM root");
        for file_name in ["dmg0_boot.bin", "dmg_boot.bin", "mgb_boot.bin"] {
            fs::write(boot_rom_root.join(file_name), vec![0_u8; 0x0100])
                .expect("frontend harness boot ROM image");
        }

        let rom_path = root.join(format!("{name}.gb"));
        let rom_bytes = build_test_rom(32 * 1024, 0x00, 0x00, 0x00);
        fs::write(&rom_path, &rom_bytes).expect("frontend harness ROM should be writable");
        let loaded_rom = with_rom.then_some(super::LoadedRom {
            path: rom_path,
            bytes: rom_bytes.clone(),
        });
        let current_dir = root.clone();
        let mut machine = if with_rom {
            super::DesktopEmulationSession::new_single(
                super::load_machine_for_rom(&config, &current_dir, &rom_bytes)
                    .expect("frontend harness machine should load")
                    .machine,
            )
        } else {
            super::DesktopEmulationSession::new_single(Machine::new_summary(
                MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
            ))
        };

        let session = super::DesktopSession {
            config: config.clone(),
            test_runner: false,
            benchmark: None,
            current_dir,
            loaded_rom,
            linked_secondary_rom: None,
            dmg07_player_count: None,
            cgb_infrared_link_active: false,
            pokemon_pikachu_color_active: false,
            pokemon_pikachu_color_gift: PokemonPikachuColorGift::default(),
            pokemon_mystery_gift_active: false,
            pokemon_mystery_gift_kind: PokemonMysteryGiftKind::default(),
            pokemon_mystery_gift_code: PokemonMysteryGiftCode::default(),
            last_open_directory: Some(root.clone()),
            recent_roms: Vec::new(),
            pocket_camera_frame: None,
            external_port_selection: DesktopExternalPortSelection::None,
        };

        let sdl = sdl3::init().expect("frontend harness SDL should initialize");
        let mut player_inputs = super::PlayerInputStates::new();
        let audio_output = if with_audio {
            Some(
                super::DesktopAudioOutput::new(
                    &sdl.audio().expect("frontend harness audio subsystem"),
                    &config.audio,
                    super::audio_source_machine(&machine).apu().console_model(),
                )
                .expect("frontend harness audio output"),
            )
        } else {
            None
        };
        let gamepad_subsystem = if with_gamepad {
            Some(sdl.gamepad().expect("frontend harness gamepad subsystem"))
        } else {
            None
        };
        let gamepad_manager = gamepad_subsystem.as_ref().map(|subsystem| {
            super::GamepadManager::new(
                subsystem,
                config.input.gamepad.clone(),
                player_inputs.input_mut(super::PlayerSlot::P1),
                machine
                    .machine_for_player_slot_mut(super::PlayerSlot::P1)
                    .expect("P1 should always map to an active desktop machine"),
            )
            .expect("frontend harness gamepad manager")
        });

        let video = sdl.video().expect("frontend harness video subsystem");
        let window = video
            .window("frontend-harness", 160 * 4, 144 * 4)
            .build()
            .expect("frontend harness window");
        let mut canvas = window.into_canvas();
        let mut frame_pacer = super::FramePacer::new(
            config.video.vsync,
            super::frame_duration_for_config(&config),
        );
        super::apply_renderer_vsync(&mut canvas, &mut frame_pacer, config.video.vsync)
            .expect("frontend harness vsync");
        let event_pump = sdl.event_pump().expect("frontend harness event pump");
        let settings_store = DesktopSettingsStore::new_for_tests(settings_path.clone());
        let mut performance_counter = super::PerformanceCounter::new_with_emulation_profile_mode(
            super::window_title(&session, &config),
            super::EmulationProfileMode::Disabled,
        );
        performance_counter
            .set_target_frame_rate_hz(super::target_frame_rate_hz_for_config(&config));
        let save_sessions = super::open_save_sessions_for_session(&session, &mut machine)
            .expect("frontend harness save sessions");
        let runtime = super::FrontendRuntime {
            paused: !with_rom,
            menu_state: super::OverlayMenuState::default(),
            player_inputs,
            keyboard_bindings: config.input.keyboard,
            video_options: config.video.clone(),
            frame_blending_state: super::FrameBlendingState::default(),
            audio_volume_percent: config.audio.volume_percent,
            audio_channel_mask: super::ApuRecordedChannelMask::ALL,
            audio_output,
            audio_recording_mode: super::DesktopAudioRecordingMode::Disabled,
            audio_recorder: None,
            gamepad_manager,
            save_sessions,
            machine_state_slot: super::DEFAULT_MACHINE_STATE_SLOT,
            rewind_buffer: super::MachineRewindBuffer::new(config.rewind.machine_rewind_config()),
            rewind_frame_tracker: super::MachineRewindFrameBoundaryTracker::new(),
            rewind_hotkey_active: false,
            rewind_gamepad_active: false,
            fast_forward_hotkey_active: false,
            fast_forward_gamepad_active: false,
            gamepad_trigger_state: super::GamepadTriggerState::default(),
            fast_forward_audio_suppressed: false,
            fast_forward_vsync_suppressed: false,
            rtc_sync: super::HostRtcSync::from_host_clock(),
            open_rom_dialog: super::PathSelectionDialog::new(),
            open_rom_dialog_mode: super::OpenRomDialogMode::Primary,
            camera_image_dialog: super::PathSelectionDialog::new(),
            pocket_camera_live: super::PocketCameraLiveInput::unavailable_for_tests(
                "test camera backend disabled",
            ),
            boot_rom_directory_dialog: super::PathSelectionDialog::new(),
            save_directory_dialog: super::PathSelectionDialog::new(),
            external_save_export_dialog: super::PathSelectionDialog::new(),
            external_save_import_dialog: super::PathSelectionDialog::new(),
            trace_capture: super::DesktopTraceCapture {
                enabled: false,
                output_path: None,
                max_t_cycles: super::DEFAULT_TRACE_CAPTURE_T_CYCLES,
                records: VecDeque::new(),
            },
            watch_trace: super::DesktopWatchTraceCapture {
                output_path: None,
                watched_addresses: BTreeSet::new(),
                max_records: super::DEFAULT_WATCH_TRACE_EVENTS,
                records: VecDeque::new(),
            },
            pc_watch_trace: super::DesktopPcWatchTraceCapture {
                output_path: None,
                watched_ranges: Vec::new(),
                max_records: super::DEFAULT_PC_WATCH_TRACE_EVENTS,
                records: VecDeque::new(),
            },
            edge_trace: super::DesktopEdgeTraceCapture {
                output_path: None,
                watched_addresses: BTreeSet::new(),
                watched_pc_ranges: Vec::new(),
                active_pc_ranges: BTreeSet::new(),
                last_observed_values: BTreeMap::new(),
                max_records: super::DEFAULT_EDGE_TRACE_EVENTS,
                records: VecDeque::new(),
            },
            cgb_ir_trace: super::DesktopCgbIrTraceCapture {
                output_path: None,
                watched_addresses: BTreeSet::new(),
                watched_trigger_addresses: BTreeSet::new(),
                max_records: super::DEFAULT_CGB_IR_TRACE_EVENTS,
                records: VecDeque::new(),
                last_p1_status: None,
                last_p2_status: None,
                last_p1_pressed_mask: None,
                last_p2_pressed_mask: None,
            },
            ch4_nr43_trace: super::DesktopCh4Nr43TraceCapture {
                output_path: None,
                records: Vec::new(),
            },
            ch4_startup_trace: super::DesktopCh4StartupTraceCapture {
                output_path: None,
                records: Vec::new(),
                last_ch4: None,
            },
            cpu_window_trace: super::DesktopCpuWindowTraceCapture {
                output_path: None,
                records: Vec::new(),
                active: false,
                finished: false,
            },
            printer_output: super::PrinterOutputState::default(),
        };

        Self {
            root,
            settings_path,
            sdl,
            canvas,
            event_pump,
            session,
            machine,
            runtime,
            settings_store,
            performance_counter,
            frame_pacer,
            _gamepad_subsystem: gamepad_subsystem,
        }
    }

    fn execute_action(
        &mut self,
        action: super::MenuAction,
    ) -> Result<Option<super::LoopSignal>, String> {
        let mut context = super::FrontendActionContext {
            session: &mut self.session,
            machine: &mut self.machine,
            runtime: &mut self.runtime,
            performance_counter: &mut self.performance_counter,
            frame_pacer: &mut self.frame_pacer,
            settings_store: &mut self.settings_store,
        };
        super::execute_menu_action(action, &self.event_pump, &mut self.canvas, &mut context)
    }

    fn push_key(&self, keycode: Keycode, down: bool) {
        let events = self
            .sdl
            .event()
            .expect("frontend harness event subsystem should initialize");
        push_key_event(&events, keycode, down);
    }

    fn push_key_with_scancode(&self, keycode: Keycode, scancode: Scancode, down: bool) {
        let events = self
            .sdl
            .event()
            .expect("frontend harness event subsystem should initialize");
        push_key_event_with_scancode(&events, keycode, scancode, down);
    }

    fn process_events(&mut self) -> Result<super::LoopSignal, String> {
        let mut context = super::FrontendActionContext {
            session: &mut self.session,
            machine: &mut self.machine,
            runtime: &mut self.runtime,
            performance_counter: &mut self.performance_counter,
            frame_pacer: &mut self.frame_pacer,
            settings_store: &mut self.settings_store,
        };
        super::process_events(&mut self.event_pump, &mut self.canvas, &mut context)
    }

    fn process_test_runner_events(&mut self) -> Result<super::LoopSignal, String> {
        let mut context = super::FrontendActionContext {
            session: &mut self.session,
            machine: &mut self.machine,
            runtime: &mut self.runtime,
            performance_counter: &mut self.performance_counter,
            frame_pacer: &mut self.frame_pacer,
            settings_store: &mut self.settings_store,
        };
        super::process_test_runner_events(&mut self.event_pump, &mut context)
    }

    fn step_until_next_frame(&mut self) -> Result<super::LoopSignal, String> {
        let mut context = super::FrontendActionContext {
            session: &mut self.session,
            machine: &mut self.machine,
            runtime: &mut self.runtime,
            performance_counter: &mut self.performance_counter,
            frame_pacer: &mut self.frame_pacer,
            settings_store: &mut self.settings_store,
        };
        super::step_until_next_frame(&mut self.event_pump, &mut self.canvas, &mut context)
            .map(|result| result.signal)
    }

    fn process_pending_open_rom_dialog(&mut self) -> Result<(), String> {
        let mut context = super::FrontendActionContext {
            session: &mut self.session,
            machine: &mut self.machine,
            runtime: &mut self.runtime,
            performance_counter: &mut self.performance_counter,
            frame_pacer: &mut self.frame_pacer,
            settings_store: &mut self.settings_store,
        };
        super::process_pending_open_rom_dialog(&self.event_pump, &mut self.canvas, &mut context)
    }

    fn process_pending_camera_image_dialog(&mut self) -> Result<(), String> {
        let mut context = super::FrontendActionContext {
            session: &mut self.session,
            machine: &mut self.machine,
            runtime: &mut self.runtime,
            performance_counter: &mut self.performance_counter,
            frame_pacer: &mut self.frame_pacer,
            settings_store: &mut self.settings_store,
        };
        super::process_pending_camera_image_dialog(&mut self.canvas, &mut context)
    }

    fn process_pocket_camera_live_frame(&mut self) {
        let mut context = super::FrontendActionContext {
            session: &mut self.session,
            machine: &mut self.machine,
            runtime: &mut self.runtime,
            performance_counter: &mut self.performance_counter,
            frame_pacer: &mut self.frame_pacer,
            settings_store: &mut self.settings_store,
        };
        super::process_pocket_camera_live_frame(&mut self.canvas, &mut context);
    }

    fn process_pending_boot_rom_directory_dialog(&mut self) -> Result<(), String> {
        let mut context = super::FrontendActionContext {
            session: &mut self.session,
            machine: &mut self.machine,
            runtime: &mut self.runtime,
            performance_counter: &mut self.performance_counter,
            frame_pacer: &mut self.frame_pacer,
            settings_store: &mut self.settings_store,
        };
        super::process_pending_boot_rom_directory_dialog(&mut self.canvas, &mut context)
    }

    fn process_pending_save_directory_dialog(&mut self) -> Result<(), String> {
        let mut context = super::FrontendActionContext {
            session: &mut self.session,
            machine: &mut self.machine,
            runtime: &mut self.runtime,
            performance_counter: &mut self.performance_counter,
            frame_pacer: &mut self.frame_pacer,
            settings_store: &mut self.settings_store,
        };
        super::process_pending_save_directory_dialog(&mut self.canvas, &mut context)
    }

    fn process_pending_external_save_export_dialog(&mut self) -> Result<(), String> {
        let mut context = super::FrontendActionContext {
            session: &mut self.session,
            machine: &mut self.machine,
            runtime: &mut self.runtime,
            performance_counter: &mut self.performance_counter,
            frame_pacer: &mut self.frame_pacer,
            settings_store: &mut self.settings_store,
        };
        super::process_pending_external_save_export_dialog(&mut self.canvas, &mut context)
    }

    fn process_pending_external_save_import_dialog(&mut self) -> Result<(), String> {
        let mut context = super::FrontendActionContext {
            session: &mut self.session,
            machine: &mut self.machine,
            runtime: &mut self.runtime,
            performance_counter: &mut self.performance_counter,
            frame_pacer: &mut self.frame_pacer,
            settings_store: &mut self.settings_store,
        };
        super::process_pending_external_save_import_dialog(&mut self.canvas, &mut context)
    }
}

fn open_cgb_primary_rom(
    harness: &mut FrontendHarness,
    name: &str,
    cartridge_type: u8,
    ram_size_code: u8,
) -> PathBuf {
    harness.session.config.launch.console_model = DesktopConsoleModel::GameBoyColor;
    let rom_path = write_cgb_test_rom(&harness.root, name, cartridge_type, ram_size_code);
    harness
        .runtime
        .open_rom_dialog
        .sender
        .send(PathDialogResult::Selected(PathBuf::from(name)))
        .expect("CGB primary ROM selection should send");
    harness
        .process_pending_open_rom_dialog()
        .expect("CGB primary ROM should load");
    assert_eq!(
        harness.session.config.launch.console_model,
        DesktopConsoleModel::GameBoyColor
    );
    assert_eq!(harness.session.rom_path(), Some(rom_path.as_path()));
    rom_path
}

fn load_initial_emulation_session_supports_direct_cgb_ir_startup_inner() {
    let root = temp_test_root("direct-cgb-ir-startup");
    let primary_rom_path = write_cgb_test_rom(&root, "gold.gbc", 0x00, 0x00);
    let secondary_rom_path = write_cgb_test_rom(&root, "silver.gbc", 0x00, 0x00);
    let primary_bytes = fs::read(&primary_rom_path).expect("primary ROM should exist");
    let secondary_bytes = fs::read(&secondary_rom_path).expect("secondary ROM should exist");
    let mut config = DesktopConfig::default();
    config.launch.console_model = DesktopConsoleModel::GameBoyColor;
    let mut session = super::DesktopSession {
        config,
        test_runner: false,
        benchmark: None,
        current_dir: root.clone(),
        loaded_rom: Some(super::LoadedRom {
            path: primary_rom_path,
            bytes: primary_bytes,
        }),
        linked_secondary_rom: Some(super::LoadedRom {
            path: secondary_rom_path,
            bytes: secondary_bytes,
        }),
        dmg07_player_count: None,
        cgb_infrared_link_active: true,
        pokemon_pikachu_color_active: false,
        pokemon_pikachu_color_gift: PokemonPikachuColorGift::default(),
        pokemon_mystery_gift_active: false,
        pokemon_mystery_gift_kind: PokemonMysteryGiftKind::default(),
        pokemon_mystery_gift_code: PokemonMysteryGiftCode::default(),
        last_open_directory: Some(root.clone()),
        recent_roms: Vec::new(),
        pocket_camera_frame: None,
        external_port_selection: super::DesktopExternalPortSelection::None,
    };

    let (machine, diagnostics) = super::load_initial_emulation_session(&mut session)
        .expect("linked CGB IR startup helper should build an infrared session");

    assert!(diagnostics.is_empty());
    assert!(session.cgb_infrared_link_active());
    assert!(machine.is_linked_cgb_infrared_two_player());
    assert_eq!(
        machine.linked_topology_kind(),
        LinkedTopologyKind::CgbInfrared
    );
    assert!(machine.secondary_machine().is_some());
}

#[path = "test/benchmark.rs"]
mod benchmark;
#[path = "test/camera.rs"]
mod camera;
#[path = "test/controls.rs"]
mod controls;
#[path = "test/diagnostics.rs"]
mod diagnostics;
#[path = "test/dialogs.rs"]
mod dialogs;
#[path = "test/frame_loop.rs"]
mod frame_loop;
#[path = "test/input_events.rs"]
mod input_events;
#[path = "test/linked_sessions.rs"]
mod linked_sessions;
#[path = "test/persistence.rs"]
mod persistence;
#[path = "test/presentation.rs"]
mod presentation;
#[path = "test/profiling.rs"]
mod profiling;
#[path = "test/rendering.rs"]
mod rendering;
#[path = "test/runtime_actions.rs"]
mod runtime_actions;
#[path = "test/runtime_dialogs.rs"]
mod runtime_dialogs;
#[path = "test/startup.rs"]
mod startup;
#[path = "test/timing.rs"]
mod timing;
