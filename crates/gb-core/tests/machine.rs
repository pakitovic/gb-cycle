mod common;

use gb_core::{
    ConsoleModel, HostPlatform, JoypadButton, Machine, MachineConfig, OperatingMode,
    SCHEDULER_PHASE_COUNT, SchedulerPhase, SgbCommandAcceptance, SgbPacketTraceStatus, StartupMode,
    TCycle,
};

const FIXTURE_ACCEPT_ENV: &str = common::fixture_env::MACHINE;

fn build_header_mode_rom(cgb_flag: u8) -> Vec<u8> {
    let mut rom = common::synthetic_cartridge::build_nom_bc_test_rom(&[0x00], 0x00, &[]);
    rom[0x0143] = cgb_flag;
    rom
}

fn build_sgb_supported_rom() -> Vec<u8> {
    let mut rom = build_header_mode_rom(0x00);
    rom[0x0146] = 0x03;
    rom[0x014B] = 0x33;
    rom
}

fn write_sgb_packet(machine: &mut Machine, bytes: [u8; 16]) {
    machine.write_bus(0xFF00, 0x00);
    machine.write_bus(0xFF00, 0x30);
    for byte in bytes {
        for bit_index in 0..8 {
            machine.write_bus(
                0xFF00,
                if (byte >> bit_index) & 0x01 == 0 {
                    0x20
                } else {
                    0x10
                },
            );
            machine.write_bus(0xFF00, 0x30);
        }
    }
    machine.write_bus(0xFF00, 0x20);
    machine.write_bus(0xFF00, 0x30);
}

fn write_sgb_palette_color(packet: &mut [u8; 16], offset: usize, rgb555: u16) {
    let [low, high] = rgb555.to_le_bytes();
    packet[offset] = low;
    packet[offset + 1] = high;
}

fn sgb_mlt_req_packet(control: u8) -> [u8; 16] {
    let mut packet = [0; 16];
    packet[0] = (0x11 << 3) | 1;
    packet[1] = control;
    packet
}

fn cycle_sgb_player(machine: &mut Machine) {
    machine.write_bus(0xFF00, 0x10);
    machine.write_bus(0xFF00, 0x30);
}

fn step_scheduler_cycle(machine: &mut Machine) {
    for _ in 0..SCHEDULER_PHASE_COUNT {
        machine.step_t_cycle();
    }
}

fn step_until_sgb_transfer_count(machine: &mut Machine, expected_count: u64) {
    for _ in 0..80_000 {
        if machine
            .snapshot()
            .sgb_host
            .video
            .vram_transfer
            .completed_transfer_count
            >= expected_count
        {
            return;
        }
        machine.step_t_cycle();
    }
    panic!("SGB VRAM transfer did not complete before the frame budget elapsed");
}

#[test]
fn machine_uses_a_single_step_t_cycle_entry_point() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    let context = machine.step_t_cycle();

    assert_eq!(context.t_cycle(), TCycle::new(0));
    assert_eq!(context.phase(), SchedulerPhase::CpuWakeInterruptEvaluation);
    assert_eq!(machine.next_t_cycle(), TCycle::new(1));
}

#[test]
fn machine_trace_includes_phase_aligned_subsystem_hooks() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.step_t_cycle();

    let fixture_path = common::paths::trace_fixture_path("machine_single_cycle_trace.txt");
    let expected = common::fixtures::ensure_text_fixture(
        &fixture_path,
        &machine.tracer().sink().render_text(),
        FIXTURE_ACCEPT_ENV,
    );

    assert_eq!(machine.tracer().sink().render_text(), expected);
}

#[test]
fn two_identical_machines_produce_the_same_two_cycle_trace() {
    let config = MachineConfig::new(ConsoleModel::GameBoy);
    let mut left = Machine::new(config.clone());
    let mut right = Machine::new(config);

    left.step_t_cycle();
    left.step_t_cycle();
    right.step_t_cycle();
    right.step_t_cycle();

    let fixture_path = common::paths::trace_fixture_path("machine_two_cycle_trace.txt");
    let expected = common::fixtures::ensure_text_fixture(
        &fixture_path,
        &left.tracer().sink().render_text(),
        FIXTURE_ACCEPT_ENV,
    );

    assert_eq!(left.tracer().sink().render_text(), expected);
    assert_eq!(right.tracer().sink().render_text(), expected);
    assert_eq!(left.next_t_cycle(), TCycle::new(2));
    assert_eq!(right.next_t_cycle(), TCycle::new(2));
}

#[test]
fn sgb_host_observes_joyp_packet_writes_after_header_unlock() {
    let mut packet = [0; 16];
    packet[0] = (0x11 << 3) | 1;
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_host_platform(HostPlatform::Sgb),
    );

    machine
        .load_cartridge(build_sgb_supported_rom())
        .expect("SGB-supported ROM should load");
    write_sgb_packet(&mut machine, packet);

    let snapshot = machine.snapshot();
    assert_eq!(
        snapshot.sgb_host.startup.command_acceptance,
        SgbCommandAcceptance::Accepted
    );
    assert_eq!(
        snapshot.sgb_host.packet_transport.last_trace.status,
        SgbPacketTraceStatus::Complete
    );
    assert_eq!(snapshot.sgb_host.command.last_command_id, Some(0x11));
    assert_eq!(snapshot.sgb_host.command.accepted_command_count, 1);
}

#[test]
fn sgb_multiplayer_routes_player_ids_and_input_slots_through_p1() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_host_platform(HostPlatform::Sgb),
    );
    machine
        .load_cartridge(build_sgb_supported_rom())
        .expect("SGB-supported ROM should load");

    write_sgb_packet(&mut machine, sgb_mlt_req_packet(3));
    assert_eq!(machine.read_bus(0xFF00), 0xFF);
    cycle_sgb_player(&mut machine);
    assert_eq!(machine.read_bus(0xFF00), 0xFE);

    machine.set_sgb_joypad_button_pressed(2, JoypadButton::A, true);
    step_scheduler_cycle(&mut machine);
    assert_eq!(
        machine.snapshot().sgb_host.multiplayer.player_pressed_masks[1],
        0x10
    );
    assert_eq!(machine.snapshot().sgb_host.multiplayer.selected_player, 2);
    machine.write_bus(0xFF00, 0x10);
    assert_eq!(
        machine.read_bus(0xFF00),
        0xDE,
        "with player 2 selected, the SGB host routes player 2's A button through the ordinary button row"
    );

    machine.set_sgb_joypad_button_pressed(1, JoypadButton::Right, true);
    step_scheduler_cycle(&mut machine);
    machine.write_bus(0xFF00, 0x20);
    assert_eq!(
        machine.read_bus(0xFF00),
        0xEF,
        "player 1 input must not leak into the currently selected SGB player slot"
    );

    let saved = machine.capture_save_state();
    let mut restored = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_host_platform(HostPlatform::Sgb),
    );
    restored
        .load_cartridge(build_sgb_supported_rom())
        .expect("SGB-supported ROM should load");
    restored
        .restore_save_state(&saved)
        .expect("SGB multiplayer state should restore");
    assert_eq!(restored.read_bus(0xFF00), 0xEF);
    assert_eq!(
        restored.snapshot().sgb_host.multiplayer.selected_player,
        3,
        "switching from the button row to the direction row raises P15 and advances the selected SGB player"
    );
    assert_eq!(
        restored
            .snapshot()
            .sgb_host
            .multiplayer
            .player_pressed_masks[1],
        0x10
    );
}

#[test]
fn sgb_lcd_color_output_maps_dmg_framebuffer_without_cgb_palette_hardware() {
    let mut packet = [0; 16];
    packet[0] = 0x01;
    write_sgb_palette_color(&mut packet, 1, 0x001F);
    write_sgb_palette_color(&mut packet, 3, 0x03E0);
    write_sgb_palette_color(&mut packet, 5, 0x7C00);
    write_sgb_palette_color(&mut packet, 7, 0x4210);
    write_sgb_palette_color(&mut packet, 9, 0x0001);
    write_sgb_palette_color(&mut packet, 11, 0x0002);
    write_sgb_palette_color(&mut packet, 13, 0x0003);

    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_host_platform(HostPlatform::Sgb),
    );
    machine
        .load_cartridge(build_sgb_supported_rom())
        .expect("SGB-supported ROM should load");
    let dmg_framebuffer_before = machine.ppu().framebuffer().to_vec();

    write_sgb_packet(&mut machine, packet);

    assert_eq!(machine.ppu().framebuffer(), dmg_framebuffer_before);
    assert!(
        machine.ppu().cgb_framebuffer_rgb555().is_none(),
        "SGB colorization must not enable or reuse CGB palette framebuffer hardware"
    );
    let sgb_lcd = machine
        .sgb_lcd_framebuffer_rgb555()
        .expect("SGB host should compose a 160x144 RGB555 LCD image");
    assert_eq!(sgb_lcd.len(), gb_core::SGB_LCD_PIXELS);
    assert!(
        sgb_lcd.iter().all(|&pixel| pixel == 0x001F),
        "the default DMG framebuffer shade 0 should map through SGB palette 0 color 0"
    );
    let snapshot = machine.snapshot();
    assert_eq!(snapshot.sgb_host.video.last_palette_command_id, Some(0x00));
    assert_eq!(snapshot.sgb_host.video.palette_command_count, 1);
}

#[test]
fn sgb_vram_transfer_captures_machine_vram_on_the_next_frame_start() {
    let mut packet = [0; 16];
    packet[0] = (0x13 << 3) | 1;
    packet[1] = 0;

    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_host_platform(HostPlatform::Sgb),
    );
    machine
        .load_cartridge(build_sgb_supported_rom())
        .expect("SGB-supported ROM should load");

    machine.write_bus(0xFF40, 0x00);
    machine.write_bus(0x8000, 0xFF);
    machine.write_bus(0xFF40, 0x91);
    write_sgb_packet(&mut machine, packet);
    assert_eq!(
        machine
            .snapshot()
            .sgb_host
            .video
            .vram_transfer
            .requested_transfer_count,
        1
    );

    step_until_sgb_transfer_count(&mut machine, 1);

    let snapshot = machine.snapshot();
    assert!(snapshot.sgb_host.video.border.chr0_loaded);
    assert_eq!(snapshot.sgb_host.video.border.tile_data.bytes[0], 0xFF);
    assert_eq!(
        snapshot
            .sgb_host
            .video
            .vram_transfer
            .last_completed
            .as_ref()
            .expect("machine should retain the captured SGB transfer")
            .payload
            .bytes[0],
        0xFF
    );
}

#[test]
fn cgb_skip_boot_mode_follows_loaded_cartridge_header_without_becoming_dmg_silicon() {
    let mut machine = Machine::new(MachineConfig::new(ConsoleModel::GameBoyColor));

    machine
        .load_cartridge(build_header_mode_rom(0x00))
        .expect("DMG-compatible ROM should load on CGB");

    assert_eq!(machine.config().console_model, ConsoleModel::GameBoyColor);
    assert_eq!(machine.config().operating_mode, OperatingMode::GbCompatible);
    let capabilities = machine.config().capability_set();
    assert_eq!(capabilities.console_model(), ConsoleModel::GameBoyColor);
    assert!(capabilities.dmg_software_contract());
    assert!(!capabilities.cgb_extensions_enabled());
    assert!(!capabilities.dmg_family_quirks_enabled());
}

#[test]
fn cgb_skip_boot_mode_treats_supported_only_and_high_bit_noncanonical_as_native_cgb() {
    for cgb_flag in [0x80, 0xC0, 0xA0] {
        let mut machine = Machine::new(MachineConfig::new(ConsoleModel::GameBoyColor));

        machine
            .load_cartridge(build_header_mode_rom(cgb_flag))
            .expect("CGB ROM should load on CGB");

        assert_eq!(machine.config().console_model, ConsoleModel::GameBoyColor);
        assert_eq!(machine.config().operating_mode, OperatingMode::Cgb);
        let capabilities = machine.config().capability_set();
        assert!(!capabilities.dmg_software_contract());
        assert!(capabilities.cgb_extensions_enabled());
        assert!(!capabilities.dmg_family_quirks_enabled());
    }
}

#[test]
fn dmg_skip_boot_mode_ignores_cgb_header_without_enabling_cgb_capabilities() {
    for cgb_flag in [0x00, 0x80, 0xC0, 0xA0] {
        let mut machine = Machine::new(MachineConfig::new(ConsoleModel::GameBoy));

        machine
            .load_cartridge(build_header_mode_rom(cgb_flag))
            .expect("header matrix ROM should load on DMG");

        assert_eq!(machine.config().console_model, ConsoleModel::GameBoy);
        assert_eq!(machine.config().operating_mode, OperatingMode::Dmg);
        let capabilities = machine.config().capability_set();
        assert!(capabilities.dmg_software_contract());
        assert!(!capabilities.cgb_extensions_enabled());
        assert!(capabilities.dmg_family_quirks_enabled());
    }
}

#[test]
fn real_boot_keeps_operating_mode_boot_owned_until_cgb_handoff_work_lands() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::RealBoot),
    );

    machine
        .load_cartridge(build_header_mode_rom(0x00))
        .expect("ROM should load before real boot handoff");

    assert_eq!(machine.config().console_model, ConsoleModel::GameBoyColor);
    assert_eq!(machine.config().operating_mode, OperatingMode::Cgb);
}

#[test]
fn debug_memory_views_expose_raw_backing_storage_without_bus_reads() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    assert_eq!(machine.debug_vram_bytes().len(), 0x2000);
    assert_eq!(machine.debug_oam_bytes().len(), 0x00A0);
    assert_eq!(machine.debug_wram_bytes().len(), 0x2000);
    assert_eq!(machine.debug_hram_bytes().len(), 0x007F);

    machine.write_bus(0xC123, 0x42);
    machine.write_bus(0xE123, 0x99);
    machine.write_bus(0xFF80, 0x77);

    assert_eq!(machine.debug_wram_bytes()[0x0123], 0x99);
    assert_eq!(machine.debug_hram_bytes()[0], 0x77);
    assert_eq!(machine.debug_vram_bytes(), machine.bus().debug_vram_bytes());
    assert_eq!(machine.debug_oam_bytes(), machine.bus().debug_oam_bytes());
    assert_eq!(machine.debug_wram_bytes(), machine.bus().debug_wram_bytes());
    assert_eq!(machine.debug_hram_bytes(), machine.bus().debug_hram_bytes());
}
