use super::step::{PendingPpuMmioWrite, commit_pending_ppu_mmio_write};
use super::*;
use crate::cartridge::PersistentCartState;
use crate::model::{ConsoleModel, ExecutionMode, StartupMode};
use crate::ppu::PpuLcdState;
use crate::scheduler::SchedulerSideEffect;
use crate::scheduler::TCycle;

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
    assert_eq!(snapshot.trace.buffered_event_count, 44);
    assert_eq!(snapshot.debug_controls.breakpoint_count, 0);
    assert_eq!(snapshot.debug_controls.watchpoint_count, 0);
    assert_eq!(snapshot.cpu.console_model, ConsoleModel::Dmg);
    assert_eq!(snapshot.apu.console_model, ConsoleModel::Dmg);
    assert_eq!(snapshot.serial.console_model, ConsoleModel::Dmg);
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
