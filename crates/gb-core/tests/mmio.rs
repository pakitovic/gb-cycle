use gb_core::{
    BootStatus, Bus, ConsoleModel, DmaTransferState, IoRegisterAvailability, IoRegisterKind,
    IoRegisterOwner, JoypadButton, Machine, MachineConfig, SerialClockMode, SerialTransferState,
    StartupMode,
};

#[test]
fn public_io_descriptor_table_covers_all_mmio_addresses_and_ie_without_gaps() {
    let bus = Bus::new(ConsoleModel::Dmg);

    for address in 0xFF00..=0xFF7F {
        assert!(
            bus.describe_io_register(address).is_some(),
            "missing MMIO contract for {address:#06X}"
        );
    }

    assert!(bus.describe_io_register(0xFFFF).is_some());

    assert_eq!(
        bus.describe_io_register(0xFF00).unwrap().owner(),
        IoRegisterOwner::Joypad
    );
    assert_eq!(
        bus.describe_io_register(0xFF46).unwrap().kind(),
        IoRegisterKind::OamDma
    );
    assert_eq!(
        bus.describe_io_register(0xFF44).unwrap().kind(),
        IoRegisterKind::Ly
    );
    assert_eq!(
        bus.describe_io_register(0xFF44).unwrap().access(),
        gb_core::IoRegisterAccess::ReadOnly
    );
    assert_eq!(
        bus.describe_io_register(0xFF13).unwrap().kind(),
        IoRegisterKind::Nr13
    );
    assert_eq!(
        bus.describe_io_register(0xFF30).unwrap().kind(),
        IoRegisterKind::WaveRam
    );
    assert_eq!(
        bus.describe_io_register(0xFF50).unwrap().kind(),
        IoRegisterKind::BootRomDisable
    );
    assert_eq!(
        bus.describe_io_register(0xFF4C).unwrap().kind(),
        IoRegisterKind::Key0
    );
    assert_eq!(
        bus.describe_io_register(0xFF4C).unwrap().availability(),
        IoRegisterAvailability::CgbOnly
    );
    assert_eq!(
        bus.describe_io_register(0xFF70).unwrap().availability(),
        IoRegisterAvailability::CgbOnly
    );
}

#[test]
fn machine_routes_phase_1_mmio_registers_through_real_subsystem_owners() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF00, 0x10);
    machine.write_bus(0xFF01, 0x12);
    machine.write_bus(0xFF02, 0xFF);
    machine.write_bus(0xFF26, 0x80);
    machine.write_bus(0xFF12, 0xF3);
    machine.write_bus(0xFF14, 0x80);
    machine.write_bus(0xFF30, 0x12);
    machine.write_bus(0xFF05, 0x12);
    machine.write_bus(0xFF06, 0x34);
    machine.write_bus(0xFF07, 0xFF);
    machine.write_bus(0xFF0F, 0x04);
    machine.write_bus(0xFF40, 0x83);
    machine.write_bus(0xFF41, 0xFF);
    machine.write_bus(0xFF42, 0x56);
    machine.write_bus(0xFF43, 0x78);
    machine.write_bus(0xFF44, 0x99);
    machine.write_bus(0xFF45, 0x00);
    machine.write_bus(0xFF47, 0xE4);
    machine.write_bus(0xFF4A, 0x9A);
    machine.write_bus(0xFF4B, 0xBC);
    machine.write_bus(0xFFFF, 0x1F);

    assert_eq!(machine.read_bus(0xFF00), 0xDF);
    assert_eq!(machine.read_bus(0xFF01), 0x12);
    assert_eq!(machine.read_bus(0xFF02), 0xFF);
    assert_eq!(machine.read_bus(0xFF12), 0xF3);
    assert_eq!(machine.read_bus(0xFF14), 0xBF);
    assert_eq!(machine.read_bus(0xFF26), 0xF1);
    assert_eq!(machine.read_bus(0xFF30), 0x12);
    assert_eq!(machine.read_bus(0xFF04), 0xAB);
    assert_eq!(machine.read_bus(0xFF05), 0x12);
    assert_eq!(machine.read_bus(0xFF06), 0x34);
    assert_eq!(machine.read_bus(0xFF07), 0xFF);
    assert_eq!(machine.read_bus(0xFF0F), 0xE4);
    assert_eq!(machine.read_bus(0xFF40), 0x83);
    assert_eq!(machine.read_bus(0xFF41), 0xFE);
    assert_eq!(machine.read_bus(0xFF42), 0x56);
    assert_eq!(machine.read_bus(0xFF43), 0x78);
    assert_eq!(machine.read_bus(0xFF44), 0x00);
    assert_eq!(machine.read_bus(0xFF45), 0x00);
    assert_eq!(machine.read_bus(0xFF47), 0xE4);
    assert_eq!(machine.read_bus(0xFF4A), 0x9A);
    assert_eq!(machine.read_bus(0xFF4B), 0xBC);
    assert_eq!(machine.read_bus(0xFFFF), 0x1F);
}

#[test]
fn joyp_readback_comes_from_hardware_button_state_plus_selected_rows() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.set_joypad_button_pressed(JoypadButton::A, true);
    machine.set_joypad_button_pressed(JoypadButton::Right, true);

    machine.write_bus(0xFF00, 0x30);
    assert_eq!(machine.read_bus(0xFF00), 0xFF);

    machine.write_bus(0xFF00, 0x10);
    assert_eq!(machine.read_bus(0xFF00), 0xDE);

    machine.write_bus(0xFF00, 0x20);
    assert_eq!(machine.read_bus(0xFF00), 0xEE);

    machine.write_bus(0xFF00, 0x00);
    assert_eq!(machine.read_bus(0xFF00), 0xCE);

    machine.write_bus(0xFF00, 0x3F);
    assert_eq!(machine.read_bus(0xFF00), 0xFF);
}

#[test]
fn joypad_irq_reaches_if_only_after_scheduler_aggregation_and_only_for_visible_edges() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF00, 0x30);
    machine.set_joypad_button_pressed(JoypadButton::A, true);
    assert_eq!(machine.read_bus(0xFF0F) & 0x10, 0x00);

    machine.step_t_cycle();
    assert_eq!(machine.read_bus(0xFF0F) & 0x10, 0x00);

    machine.write_bus(0xFF00, 0x10);
    assert_eq!(machine.read_bus(0xFF0F) & 0x10, 0x00);

    machine.step_t_cycle();
    assert_eq!(machine.read_bus(0xFF0F) & 0x10, 0x10);
}

#[test]
fn joypad_irq_can_retrigger_after_a_new_visible_high_to_low_transition() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF00, 0x10);
    machine.set_joypad_button_pressed(JoypadButton::A, true);
    machine.step_t_cycle();
    assert_eq!(machine.read_bus(0xFF0F) & 0x10, 0x10);

    machine.write_bus(0xFF0F, 0x00);
    machine.set_joypad_button_pressed(JoypadButton::A, false);
    machine.step_t_cycle();
    assert_eq!(machine.read_bus(0xFF0F) & 0x10, 0x00);

    machine.set_joypad_button_pressed(JoypadButton::A, true);
    machine.step_t_cycle();
    assert_eq!(machine.read_bus(0xFF0F) & 0x10, 0x10);
}

#[test]
fn ff46_and_ff50_writes_take_effect_immediately_on_their_owners() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::RealBoot),
    );

    assert_eq!(machine.boot().status(), BootStatus::Ready);
    assert!(machine.boot().is_boot_rom_mapped());

    machine.write_bus(0xFF46, 0x12);
    machine.write_bus(0xFF50, 0x01);

    assert_eq!(machine.read_bus(0xFF46), 0x12);
    assert_eq!(machine.dma().source_page_latch(), 0x12);
    assert!(matches!(
        machine.dma().transfer_state(),
        DmaTransferState::Starting(_)
    ));
    assert!(!machine.boot().is_boot_rom_mapped());
    assert_eq!(machine.read_bus(0xFF50), 0xFF);
}

#[test]
fn ppu_mmio_commit_phase_emits_a_ppu_trace_for_the_committed_write() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge({
            let mut rom = vec![0xFF; 32 * 1024];
            rom[0x0100..0x0106].copy_from_slice(&[
                0x3E, 0x00, // ld a,$00
                0xE0, 0x40, // ldh ($40),a
                0x18, 0xFE, // jr .
            ]);
            rom[0x0147] = 0x00;
            rom[0x0148] = 0x00;
            rom[0x0149] = 0x00;
            rom
        })
        .expect("NoMBC test ROM should load");

    let mut commit_t_cycle = None;
    for _ in 0..32 {
        let context = machine.step_t_cycle();
        if machine.read_bus(0xFF40) == 0x00 {
            commit_t_cycle = Some(context.t_cycle().get());
            break;
        }
    }

    let commit_t_cycle = commit_t_cycle.expect("CPU LCDC write should commit within 32 T-cycles");
    let trace = machine.tracer().sink().render_text();

    let ppu_fragment = format!(
        "subsystem=ppu level=trace message=\"t_cycle={} phase=mmio_side_effect_commit",
        commit_t_cycle
    );
    assert!(trace.contains(&ppu_fragment));
    assert!(trace.contains("committed_write=0xFF40<-0x00"));
    assert!(trace.contains("lcdc=0x00"));

    let boot_fragment = format!(
        "subsystem=boot level=trace message=\"t_cycle={} phase=mmio_side_effect_commit",
        commit_t_cycle
    );
    assert!(!trace.contains(&boot_fragment));
}

#[test]
fn serial_mmio_arming_keeps_transfer_pending_without_instant_completion() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF01, 0xA5);
    machine.write_bus(0xFF02, 0x81);

    assert_eq!(machine.read_bus(0xFF01), 0xA5);
    assert_eq!(machine.read_bus(0xFF02), 0xFF);
    assert_eq!(machine.serial().clock_mode(), SerialClockMode::Internal);
    assert_eq!(
        machine.serial().transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 0 }
    );

    machine.step_t_cycle();

    assert_eq!(machine.read_bus(0xFF01), 0xA5);
    assert_eq!(machine.read_bus(0xFF02), 0xFF);
    assert_eq!(
        machine.serial().transfer_state(),
        SerialTransferState::TransferRequested { bits_shifted: 0 }
    );
}

#[test]
fn serial_snapshot_and_debug_view_expose_the_pending_transfer_shape() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF01, 0x3C);
    machine.write_bus(0xFF02, 0x80);

    let snapshot = machine.snapshot();
    let rendered = snapshot.render_text();

    assert_eq!(snapshot.serial.sb, 0x3C);
    assert_eq!(snapshot.serial.clock_mode, SerialClockMode::External);
    assert_eq!(
        snapshot.serial.transfer_state,
        SerialTransferState::TransferRequested { bits_shifted: 0 }
    );
    assert!(rendered.contains("serial.sb=0x3C"));
    assert!(rendered.contains("serial.clock_mode=External"));
    assert!(rendered.contains("serial.transfer_state=TransferRequested { bits_shifted: 0 }"));
}

#[test]
fn dmg_family_reads_ff_and_ignores_writes_for_unavailable_cgb_registers() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    for address in [0xFF4C, 0xFF4D, 0xFF4F, 0xFF70, 0xFF76] {
        machine.write_bus(address, 0xA5);
        assert_eq!(machine.read_bus(address), 0xFF);
    }
}
