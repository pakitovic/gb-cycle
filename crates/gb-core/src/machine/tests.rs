use super::step::{PendingPpuMmioWrite, commit_pending_ppu_mmio_write};
use super::*;
use crate::cartridge::PersistentCartState;
use crate::debugger::BreakpointCondition;
use crate::external_port::{ExternalPortAttachmentKind, ExternalPortResetPolicy};
use crate::joypad::JoypadButton;
use crate::model::{ConsoleModel, ExecutionMode, StartupMode};
use crate::ppu::{PpuLcdState, PpuStepRegion};
use crate::scheduler::{ExternalEvent, SchedulerSideEffect, TCycle};
use crate::serial::SerialPeer;

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;

fn build_test_rom(program: &[u8]) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    for (offset, byte) in program.iter().copied().enumerate() {
        rom[0x0100 + offset] = byte;
    }
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom
}

fn build_test_rom_with_header(
    program: &[u8],
    cartridge_type: u8,
    rom_size: u8,
    ram_size: u8,
) -> Vec<u8> {
    let mut rom = build_test_rom(program);
    rom[0x0147] = cartridge_type;
    rom[0x0148] = rom_size;
    rom[0x0149] = ram_size;
    rom
}

#[test]
fn machine_new_starts_on_the_first_t_cycle() {
    let machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    assert_eq!(machine.next_t_cycle(), TCycle::ZERO);
    assert_eq!(machine.config().console_model, ConsoleModel::Dmg);
    assert_eq!(machine.cpu().console_model(), ConsoleModel::Dmg);
    assert_eq!(machine.boot().startup_mode(), StartupMode::SkipBoot);
    assert!(machine.cartridge().is_empty());
    assert_eq!(
        machine.external_port().attachment_kind(),
        ExternalPortAttachmentKind::None
    );
}

#[test]
fn step_t_cycle_advances_exactly_one_cycle_per_call() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Mgb).with_execution_mode(ExecutionMode::Permissive),
    );

    let first = machine.step_t_cycle();
    let second = machine.step_t_cycle();

    assert_eq!(first.t_cycle(), TCycle::new(0));
    assert_eq!(second.t_cycle(), TCycle::new(1));
    assert_eq!(machine.next_t_cycle(), TCycle::new(2));
}

#[derive(Default)]
struct RegionCollector {
    regions: Vec<MachineStepRegion>,
    ppu_regions: Vec<PpuStepRegion>,
}

impl MachineStepObserver for RegionCollector {
    fn begin_region(&mut self, region: MachineStepRegion) {
        self.regions.push(region);
    }

    fn begin_ppu_region(&mut self, region: PpuStepRegion) {
        self.ppu_regions.push(region);
    }
}

#[test]
fn step_t_cycle_with_observer_reports_regions_in_scheduler_order() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    let mut observer = RegionCollector::default();

    machine.step_t_cycle_with_observer(&mut observer);

    assert_eq!(
        observer.regions,
        vec![
            MachineStepRegion::ExternalEvents,
            MachineStepRegion::Timer,
            MachineStepRegion::Apu,
            MachineStepRegion::Dma,
            MachineStepRegion::Dma,
            MachineStepRegion::Ppu,
            MachineStepRegion::Serial,
            MachineStepRegion::Cpu,
            MachineStepRegion::Ppu,
            MachineStepRegion::Interrupts,
            MachineStepRegion::Cpu,
        ]
    );
    assert_eq!(
        observer.ppu_regions,
        vec![
            PpuStepRegion::Mode2Scan,
            PpuStepRegion::Mode2Scan,
            PpuStepRegion::Mode2Scan,
        ]
    );
}

#[test]
fn machine_can_restore_cartridge_persistent_state_through_a_narrow_host_api() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom_with_header(&[0x00], 0x09, 0x00, 0x02))
        .expect("NoMBC+RAM+BATTERY test ROM should load");

    machine
        .restore_cartridge_persistent_state(&PersistentCartState::NoMbcRam {
            ram: vec![0xAB; 8 * 1024],
        })
        .expect("restoring cartridge RAM should succeed");

    assert_eq!(machine.read_bus(0xA000), 0xAB);
}

#[test]
fn machine_parts_keep_the_current_subsystem_boundaries_explicit() {
    let machine = Machine::new(
        MachineConfig::new(ConsoleModel::Mgb).with_startup_mode(StartupMode::RealBoot),
    );

    let parts = machine.into_parts();

    assert!(parts.debug_controls.breakpoints().is_empty());
    assert!(parts.debug_controls.watchpoints().is_empty());
    assert_eq!(parts.cpu.console_model(), ConsoleModel::Mgb);
    assert_eq!(parts.bus.console_model(), ConsoleModel::Mgb);
    assert_eq!(parts.apu.console_model(), ConsoleModel::Mgb);
    assert_eq!(parts.ppu.console_model(), ConsoleModel::Mgb);
    assert_eq!(parts.dma.console_model(), ConsoleModel::Mgb);
    assert_eq!(parts.timer.console_model(), ConsoleModel::Mgb);
    assert_eq!(parts.serial.console_model(), ConsoleModel::Mgb);
    assert_eq!(
        parts.external_port.attachment_kind(),
        ExternalPortAttachmentKind::None
    );
    assert_eq!(parts.boot.console_model(), ConsoleModel::Mgb);
    assert_eq!(parts.interrupts.console_model(), ConsoleModel::Mgb);
    assert_eq!(parts.joypad.console_model(), ConsoleModel::Mgb);
    assert_eq!(parts.boot.startup_mode(), StartupMode::RealBoot);
    assert!(parts.cartridge.is_empty());
}

#[test]
fn machine_snapshot_exposes_scheduler_trace_and_live_phase_1_subsystems() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.step_t_cycle();
    machine.step_t_cycle();

    let snapshot = machine.snapshot();

    assert_eq!(snapshot.config.console_model, ConsoleModel::Dmg);
    assert_eq!(snapshot.scheduler.next_t_cycle, TCycle::new(2));
    assert_eq!(snapshot.trace.buffered_event_count, 42);
    assert_eq!(snapshot.debug_controls.breakpoint_count, 0);
    assert_eq!(snapshot.debug_controls.watchpoint_count, 0);
    assert_eq!(snapshot.cpu.console_model, ConsoleModel::Dmg);
    assert_eq!(snapshot.apu.console_model, ConsoleModel::Dmg);
    assert_eq!(snapshot.serial.console_model, ConsoleModel::Dmg);
    assert_eq!(
        snapshot.external_port.attachment_kind(),
        ExternalPortAttachmentKind::None
    );
    assert_eq!(snapshot.interrupts.console_model, ConsoleModel::Dmg);
    assert_eq!(snapshot.joypad.console_model, ConsoleModel::Dmg);
    assert!(matches!(
        snapshot.cartridge.state,
        crate::CartridgeSlotState::Empty
    ));
}

#[test]
fn staged_ppu_mmio_write_leaves_ppu_storage_unchanged_until_commit_phase() {
    let mut ppu = Ppu::new(ConsoleModel::Dmg);
    let mut pending = Some(PendingPpuMmioWrite {
        address: 0xFF42,
        value: 0x12,
    });

    assert_eq!(ppu.read_register(0xFF42), 0x00);

    commit_pending_ppu_mmio_write(&mut ppu, &mut pending);

    assert_eq!(ppu.read_register(0xFF42), 0x12);
    assert!(pending.is_none());
}

#[test]
fn cpu_ppu_mmio_writes_commit_during_phase_7_of_the_same_t_cycle() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(&[
            0x3E, 0x00, // ld a,$00
            0xE0, 0x40, // ldh ($40),a
            0x18, 0xFE, // jr .
        ]))
        .expect("NoMBC test ROM should load");

    let mut commit_context = None;
    for _ in 0..32 {
        let context = machine.step_t_cycle();
        if machine.read_bus(0xFF40) == 0x00 {
            commit_context = Some(context);
            break;
        }
    }

    let context = commit_context.expect("CPU LCDC write should commit within 32 T-cycles");
    assert!(
        context
            .queued_side_effects()
            .contains(&SchedulerSideEffect::CommitMmioWrite)
    );
    assert_eq!(machine.ppu().snapshot().lcd_state, PpuLcdState::Disabled);
}

#[test]
fn joypad_host_input_is_ingested_during_external_event_ingress() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF00, 0x10);
    machine.set_joypad_button_pressed(JoypadButton::A, true);

    assert_eq!(machine.read_bus(0xFF00), 0xDF);
    assert_eq!(machine.joypad().pressed_mask(), 0x00);

    let context = machine.step_t_cycle();

    assert_eq!(
        context.phase(),
        crate::scheduler::SchedulerPhase::CpuWakeInterruptEvaluation
    );
    assert_eq!(
        context.external_events(),
        &[ExternalEvent::HostInputChanged]
    );
    assert_eq!(machine.joypad().pressed_mask(), 0x10);
    assert_eq!(machine.read_bus(0xFF00), 0xDE);
}

#[test]
fn external_serial_clock_is_ingested_during_external_event_ingress() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF01, 0x81);
    machine.write_bus(0xFF02, 0x80);
    machine.queue_external_serial_clock();

    let context = machine.step_t_cycle();

    assert_eq!(
        context.external_events(),
        &[ExternalEvent::ExternalSerialClock]
    );
    assert_eq!(machine.read_bus(0xFF01), 0x03);
}

#[test]
fn external_serial_clock_is_dropped_while_cpu_stop_is_active() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x10, 0x00, 0x00]))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF00, 0x10);
    machine.write_bus(0xFF01, 0x81);
    machine.write_bus(0xFF02, 0x80);

    for _ in 0..8 {
        machine.step_t_cycle();
    }

    assert_eq!(
        machine.cpu().execution_state(),
        crate::cpu::CpuExecutionState::Stopped
    );

    machine.queue_external_serial_clock();
    let context = machine.step_t_cycle();

    assert!(context.external_events().is_empty());
    assert_eq!(machine.read_bus(0xFF01), 0x81);
    assert_eq!(
        machine.serial().transfer_state(),
        crate::serial::SerialTransferState::TransferRequested { bits_shifted: 0 }
    );
}

#[test]
fn load_cartridge_restarts_skip_boot_runtime_from_cycle_zero() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    let mut first_rom = build_test_rom(&[0x00]);
    let mut second_rom = build_test_rom(&[0x00]);
    first_rom[0x014D] = 0x7F;
    second_rom[0x014D] = 0x00;

    machine
        .load_cartridge(first_rom)
        .expect("supported NoMBC image should load");
    machine
        .debug_controls_mut()
        .add_breakpoint(BreakpointCondition::ProgramCounter(0x0100));
    machine.set_external_port_attachment(ExternalPortAttachmentKind::Loopback);
    machine.set_joypad_button_pressed(JoypadButton::A, true);
    machine.step_t_cycle();
    machine.step_t_cycle();

    assert_eq!(machine.next_t_cycle(), TCycle::new(2));
    assert_eq!(machine.joypad().pressed_mask(), 0x10);
    assert!(machine.tracer().snapshot().buffered_event_count > 0);

    machine
        .load_cartridge(second_rom)
        .expect("reloading a supported NoMBC image should succeed");

    assert_eq!(machine.next_t_cycle(), TCycle::ZERO);
    assert_eq!(machine.cpu().startup_state().pc, 0x0100);
    assert_eq!(machine.cpu().startup_state().f, 0x80);
    assert_eq!(machine.joypad().pressed_mask(), 0x00);
    assert_eq!(
        machine.external_port().attachment_kind(),
        ExternalPortAttachmentKind::Loopback
    );
    assert_eq!(machine.serial().peer(), SerialPeer::Loopback);
    assert_eq!(machine.tracer().next_sequence(), 0);
    assert_eq!(machine.tracer().snapshot().buffered_event_count, 0);
    assert_eq!(machine.debug_controls().breakpoints().len(), 1);

    let context = machine.step_t_cycle();

    assert_eq!(
        context.external_events(),
        &[ExternalEvent::HostInputChanged]
    );
    assert_eq!(machine.joypad().pressed_mask(), 0x10);
}

#[test]
fn external_port_attachment_selection_updates_the_serial_peer_boundary() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.set_external_port_attachment(ExternalPortAttachmentKind::Loopback);

    assert_eq!(
        machine.external_port().attachment_kind(),
        ExternalPortAttachmentKind::Loopback
    );
    assert_eq!(machine.serial().peer(), SerialPeer::Loopback);
}

#[test]
fn machine_exposes_external_port_reset_policy_configuration() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.set_external_port_reset_policy(ExternalPortResetPolicy::PreserveAttachmentKind);

    assert_eq!(
        machine.external_port().reset_policy(),
        ExternalPortResetPolicy::PreserveAttachmentKind
    );
}

#[test]
fn load_cartridge_restarts_real_boot_from_power_on_state() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::RealBoot),
    );

    machine
        .load_cartridge(build_test_rom(&[0x00]))
        .expect("supported NoMBC image should load");

    for _ in 0..16 {
        machine.step_t_cycle();
    }
    machine.write_bus(0xFF50, 0x01);

    assert_ne!(machine.next_t_cycle(), TCycle::ZERO);
    assert_ne!(machine.cpu().registers().pc, 0x0000);
    assert!(!machine.boot().is_boot_rom_mapped());

    machine
        .load_cartridge(build_test_rom(&[0x00]))
        .expect("reloading a supported NoMBC image should succeed");

    assert_eq!(machine.next_t_cycle(), TCycle::ZERO);
    assert_eq!(machine.cpu().startup_state().pc, 0x0000);
    assert_eq!(machine.cpu().registers().pc, 0x0000);
    assert!(machine.boot().is_boot_rom_mapped());
}
