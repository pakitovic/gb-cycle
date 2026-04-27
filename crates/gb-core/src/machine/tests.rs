use super::step::{PendingPpuMmioWrite, commit_pending_ppu_mmio_write};
use super::*;
use crate::cartridge::{
    CartridgeSlotState, PersistentCartState, PocketCameraFrame, PocketCameraFrameError,
};
use crate::debugger::BreakpointCondition;
use crate::dma::DmaTransferLifecycle;
use crate::external_port::{ExternalPortAttachmentKind, ExternalPortResetPolicy};
use crate::joypad::JoypadButton;
use crate::model::{ConsoleModel, ExecutionMode, StartupMode};
use crate::ppu::{PpuAccessMode, PpuLcdState, PpuStepRegion};
use crate::rewind::{MachineRewindBuffer, MachineRewindConfig, MachineRewindSubframeCadence};
use crate::scheduler::{ExternalEvent, SchedulerSideEffect, TCycle};
use crate::serial::{SerialPeer, SerialTransferState};

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

fn build_banked_test_rom(
    program: &[u8],
    cartridge_type: u8,
    rom_size: u8,
    ram_size: u8,
) -> Vec<u8> {
    let rom_len = match rom_size {
        0x00 => 32 * 1024,
        0x01 => 64 * 1024,
        0x02 => 128 * 1024,
        0x03 => 256 * 1024,
        0x04 => 512 * 1024,
        0x05 => 1024 * 1024,
        _ => 32 * 1024,
    };
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(rom_len)];

    for bank in 0..(rom.len() / 0x4000) {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    for (offset, byte) in program.iter().copied().enumerate() {
        rom[0x0100 + offset] = byte;
    }
    rom[0x0147] = cartridge_type;
    rom[0x0148] = rom_size;
    rom[0x0149] = ram_size;
    rom
}

fn build_pocket_camera_rom() -> Vec<u8> {
    let mut rom = vec![0xFF; 1024 * 1024];
    rom[0x0000] = 0x12;
    rom[0x0100..0x0104].copy_from_slice(&[0x31, 0xFE, 0xFF, 0xAF]);
    rom[0x0104..0x0134].fill(0xCE);
    rom[0x0134..0x013C].copy_from_slice(b"CAMTEST!");
    rom[0x0143] = 0x80;
    rom[0x0146] = 0x03;
    rom[0x0147] = 0xFC;
    rom[0x0148] = 0x05;
    rom[0x0149] = 0x04;

    for bank in 0..(rom.len() / 0x4000) {
        let start = bank * 0x4000;
        rom[start] = bank as u8;
        rom[start + 0x0100] = bank as u8;
    }

    rom
}

fn step_t_cycles(machine: &mut Machine, t_cycles: u64) {
    for _ in 0..t_cycles {
        machine.step_t_cycle();
    }
}

fn step_until(
    machine: &mut Machine,
    max_t_cycles: u64,
    description: &str,
    predicate: impl Fn(&Machine) -> bool,
) {
    for _ in 0..max_t_cycles {
        if predicate(machine) {
            return;
        }
        machine.step_t_cycle();
    }

    panic!("timed out before reaching save-state hardening point: {description}");
}

fn assert_save_state_restores_continuation(
    mut machine: Machine,
    label: &str,
    dirty_t_cycles: u64,
    continuation_t_cycles: u64,
) -> Machine {
    let saved = machine.capture_save_state();
    let mut uninterrupted = machine.clone();

    step_t_cycles(&mut uninterrupted, continuation_t_cycles);
    step_t_cycles(&mut machine, dirty_t_cycles);

    machine
        .restore_save_state(&saved)
        .unwrap_or_else(|error| panic!("{label}: matching save-state restore failed: {error}"));
    assert_eq!(
        machine.capture_save_state(),
        saved,
        "{label}: restore must recreate the captured boundary exactly"
    );

    step_t_cycles(&mut machine, continuation_t_cycles);
    assert_eq!(
        machine.capture_save_state(),
        uninterrupted.capture_save_state(),
        "{label}: restored continuation diverged from uninterrupted execution"
    );

    machine
}

fn assert_rewind_restores_continuation(
    mut machine: Machine,
    label: &str,
    dirty_t_cycles: u64,
    continuation_t_cycles: u64,
) {
    let mut rewind = MachineRewindBuffer::new(
        MachineRewindConfig::default()
            .with_target_history_t_cycles(u64::MAX)
            .with_max_estimated_bytes(usize::MAX)
            .with_subframe_cadence(MachineRewindSubframeCadence::EveryTCycles(1)),
    );
    let expected_rewind_state = machine.capture_save_state();
    let mut uninterrupted = machine.clone();

    assert!(
        rewind.record_subframe(&machine),
        "{label}: rewind should capture the target state"
    );
    step_t_cycles(&mut machine, dirty_t_cycles);
    rewind
        .rewind_one(&mut machine)
        .unwrap_or_else(|error| panic!("{label}: rewind restore failed: {error}"))
        .unwrap_or_else(|| panic!("{label}: rewind buffer unexpectedly empty"));
    assert_eq!(
        machine.capture_save_state(),
        expected_rewind_state,
        "{label}: rewind must restore the captured target exactly"
    );

    step_t_cycles(&mut machine, continuation_t_cycles);
    step_t_cycles(&mut uninterrupted, continuation_t_cycles);
    assert_eq!(
        machine.capture_save_state(),
        uninterrupted.capture_save_state(),
        "{label}: rewind continuation diverged from uninterrupted execution"
    );
}

fn seed_dma_source_page(machine: &mut Machine, source_page: u8, seed: u8) {
    let source_start = (source_page as u16) << 8;

    for byte_index in 0..160u16 {
        let value = seed
            .wrapping_mul(17)
            .wrapping_add(byte_index as u8)
            .rotate_left(1);
        machine.write_bus(source_start + byte_index, value);
    }
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
fn machine_pocket_camera_host_api_is_gated_by_the_loaded_cartridge_family() {
    let frame = PocketCameraFrame {
        width: 1,
        height: 1,
        grayscale_pixels: vec![0x44],
    };

    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    assert!(!machine.has_pocket_camera());
    assert_eq!(
        machine.set_pocket_camera_frame(frame.clone()),
        Err(PocketCameraFrameError::UnsupportedCartridge)
    );
    assert_eq!(
        machine.clear_pocket_camera_frame(),
        Err(PocketCameraFrameError::UnsupportedCartridge)
    );

    machine
        .load_cartridge(build_test_rom_with_header(&[0x00], 0x09, 0x00, 0x02))
        .expect("NoMBC+RAM+BATTERY test ROM should load");
    assert!(!machine.has_pocket_camera());
    assert_eq!(
        machine.set_pocket_camera_frame(frame.clone()),
        Err(PocketCameraFrameError::UnsupportedCartridge)
    );

    machine
        .load_cartridge(build_pocket_camera_rom())
        .expect("Pocket Camera test ROM should load");
    assert!(machine.has_pocket_camera());
    machine
        .set_pocket_camera_frame(frame)
        .expect("Pocket Camera machines should accept host frames");
    machine
        .clear_pocket_camera_frame()
        .expect("Pocket Camera machines should restore the placeholder frame");
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
fn save_state_round_trips_exactly_at_a_t_cycle_boundary() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(&[
            0x3E, 0x12, // ld a,$12
            0xE0, 0x01, // ldh ($01),a
            0x00, // nop
            0x18, 0xFD, // jr $0104
        ]))
        .expect("NoMBC test ROM should load");
    machine.set_external_port_attachment(ExternalPortAttachmentKind::Loopback);
    machine.set_joypad_button_pressed(JoypadButton::A, true);

    for _ in 0..64 {
        machine.step_t_cycle();
    }

    let saved = machine.capture_save_state();

    for _ in 0..37 {
        machine.step_t_cycle();
    }
    machine
        .restore_save_state(&saved)
        .expect("matching machine metadata should restore");

    assert_eq!(machine.capture_save_state(), saved);
}

#[test]
fn save_state_continuation_matches_uninterrupted_execution() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(&[
            0x00, // nop
            0x3C, // inc a
            0xE0, 0x80, // ldh ($80),a
            0x18, 0xFA, // jr $0100
        ]))
        .expect("NoMBC test ROM should load");

    for _ in 0..91 {
        machine.step_t_cycle();
    }

    let saved = machine.capture_save_state();
    let mut uninterrupted = machine.clone();

    for _ in 0..211 {
        uninterrupted.step_t_cycle();
    }
    for _ in 0..53 {
        machine.step_t_cycle();
    }

    machine
        .restore_save_state(&saved)
        .expect("matching machine metadata should restore");
    for _ in 0..211 {
        machine.step_t_cycle();
    }

    assert_eq!(
        machine.capture_save_state(),
        uninterrupted.capture_save_state()
    );
}

#[test]
fn save_state_restore_rejects_incompatible_model_before_mutating() {
    let source = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    let saved = source.capture_save_state();
    let mut target = Machine::new(
        MachineConfig::new(ConsoleModel::Mgb).with_startup_mode(StartupMode::SkipBoot),
    );
    let before = target.capture_save_state();

    let error = target
        .restore_save_state(&saved)
        .expect_err("model mismatch must be rejected");

    assert!(matches!(
        error,
        MachineSaveStateRestoreError::ConsoleModelMismatch {
            expected: ConsoleModel::Dmg,
            actual: ConsoleModel::Mgb,
        }
    ));
    assert_eq!(target.capture_save_state(), before);
}

#[test]
fn save_state_restore_can_replace_runtime_boot_mapping_state() {
    let mut source = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::RealBoot),
    );
    source.write_bus(0xFF50, 0x01);
    assert!(!source.boot().is_boot_rom_mapped());
    let saved = source.capture_save_state();

    let mut target = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::RealBoot),
    );
    assert!(target.boot().is_boot_rom_mapped());

    target
        .restore_save_state(&saved)
        .expect("matching boot ROM identity should restore even when mapping state differs");

    assert_eq!(target.capture_save_state(), saved);
}

#[test]
fn save_state_capture_restore_supports_rewind_style_subframe_cadence() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(&[
            0x3E, 0x80, // ld a,$80
            0xE0, 0x02, // ldh ($02),a
            0x3C, // inc a
            0x18, 0xFB, // jr $0102
        ]))
        .expect("NoMBC test ROM should load");

    for cadence in [1, 7, 57, 113] {
        for _ in 0..cadence {
            machine.step_t_cycle();
        }

        let saved = machine.capture_save_state();

        for _ in 0..cadence {
            machine.step_t_cycle();
        }
        machine
            .restore_save_state(&saved)
            .expect("matching machine metadata should restore");

        assert_eq!(machine.capture_save_state(), saved);
    }
}

#[test]
fn save_state_hardening_preserves_cpu_mid_instruction_halt_and_ime_states() {
    let mut mid_instruction = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    mid_instruction
        .load_cartridge(build_test_rom(&[
            0xEA, 0x34, 0xC1, // ld ($C134),a
            0x18, 0xFE, // jr .
        ]))
        .expect("NoMBC test ROM should load");
    step_until(
        &mut mid_instruction,
        64,
        "CPU mid-instruction execution state",
        |machine| {
            let cpu = machine.cpu().snapshot();
            cpu.current_opcode == Some(0xEA)
                && matches!(
                    cpu.execution_state,
                    CpuExecutionState::Execute { step, .. } if step != 0
                )
        },
    );
    assert_save_state_restores_continuation(mid_instruction, "CPU mid-instruction", 37, 149);

    let mut ime_pending = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    ime_pending
        .load_cartridge(build_test_rom(&[
            0xFB, // ei
            0x00, // nop
            0x18, 0xFE, // jr .
        ]))
        .expect("NoMBC test ROM should load");
    step_until(&mut ime_pending, 64, "CPU pending IME enable", |machine| {
        machine.cpu().snapshot().delayed_ime_enable
    });
    assert_save_state_restores_continuation(ime_pending, "CPU pending IME", 19, 97);

    let mut halted = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    halted
        .load_cartridge(build_test_rom(&[
            0x76, // halt
            0x00, // nop padding
        ]))
        .expect("NoMBC test ROM should load");
    step_until(&mut halted, 64, "CPU HALT state", |machine| {
        machine.cpu().execution_state() == CpuExecutionState::Halted
    });
    assert_save_state_restores_continuation(halted, "CPU HALT", 31, 113);
}

#[test]
fn save_state_hardening_preserves_mode3_window_fifo_and_obj_state() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(&[0x00]))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF40, 0x00);
    machine.write_bus(0xFF42, 0x00);
    machine.write_bus(0xFF43, 0x00);
    machine.write_bus(0xFF4A, 0x00);
    machine.write_bus(0xFF4B, 0x07);
    for offset in 0..16u16 {
        machine.write_bus(0x8000 + offset, 0xFF);
    }
    machine.write_bus(0x9800, 0x00);
    machine.write_bus(0x9C00, 0x00);
    machine.write_bus(0xFE00, 16);
    machine.write_bus(0xFE01, 8);
    machine.write_bus(0xFE02, 0);
    machine.write_bus(0xFE03, 0);
    machine.write_bus(0xFF40, 0xF3);

    step_until(
        &mut machine,
        1_000,
        "PPU Mode 3 window FIFO with selected OBJ",
        |machine| {
            let ppu = machine.ppu().snapshot();
            ppu.mode == PpuAccessMode::Drawing
                && ppu.window_started_this_line
                && !ppu.selected_sprites.is_empty()
                && !ppu.bg_fifo_pixels.is_empty()
        },
    );

    assert_save_state_restores_continuation(machine, "PPU Mode 3 window/OBJ", 83, 367);
}

#[test]
fn save_state_hardening_preserves_active_dma_and_pending_restart() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(&[0x00]))
        .expect("NoMBC test ROM should load");
    seed_dma_source_page(&mut machine, 0xC0, 0x11);
    seed_dma_source_page(&mut machine, 0xC1, 0x27);

    machine.write_bus(0xFF46, 0xC0);
    step_until(&mut machine, 64, "active OAM DMA transfer", |machine| {
        machine.dma().snapshot().transfer_state.lifecycle() == DmaTransferLifecycle::Active
    });
    machine.write_bus(0xFF46, 0xC1);
    let dma = machine.dma().snapshot();
    assert_eq!(dma.transfer_state.lifecycle(), DmaTransferLifecycle::Active);
    assert!(
        dma.pending_restart.is_some(),
        "second FF46 write during active DMA should be latched as a pending restart"
    );

    assert_save_state_restores_continuation(machine, "active DMA with pending restart", 41, 777);
}

#[test]
fn save_state_hardening_preserves_timer_overflow_pipeline() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(&[0x00]))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF0F, 0x00);
    machine.write_bus(0xFF04, 0x00);
    machine.write_bus(0xFF05, 0xFF);
    machine.write_bus(0xFF06, 0x42);
    machine.write_bus(0xFF07, 0x05);
    step_t_cycles(&mut machine, 16);

    assert_eq!(machine.read_bus(0xFF05), 0x00);
    assert_eq!(machine.read_bus(0xFF0F) & 0x04, 0x00);

    assert_save_state_restores_continuation(machine, "timer overflow pipeline", 5, 29);
}

#[test]
fn save_state_hardening_preserves_serial_transfers_in_flight() {
    let mut internal_clock = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    internal_clock
        .load_cartridge(build_test_rom(&[0x00]))
        .expect("NoMBC test ROM should load");
    internal_clock.set_external_port_attachment(ExternalPortAttachmentKind::Loopback);
    internal_clock.write_bus(0xFF01, 0x96);
    internal_clock.write_bus(0xFF02, 0x81);
    step_until(
        &mut internal_clock,
        1_200,
        "internal-clock serial transfer in flight",
        |machine| {
            matches!(
                machine.serial().snapshot().transfer_state,
                SerialTransferState::TransferRequested { bits_shifted } if (1..8).contains(&bits_shifted)
            )
        },
    );
    assert_save_state_restores_continuation(
        internal_clock,
        "internal-clock serial transfer",
        113,
        1_337,
    );

    let mut external_clock = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    external_clock
        .load_cartridge(build_test_rom(&[0x00]))
        .expect("NoMBC test ROM should load");
    external_clock.write_bus(0xFF01, 0xA5);
    external_clock.write_bus(0xFF02, 0x80);
    external_clock.queue_external_serial_clock();
    step_until(
        &mut external_clock,
        32,
        "external-clock serial transfer in flight",
        |machine| {
            matches!(
                machine.serial().snapshot().transfer_state,
                SerialTransferState::TransferRequested { bits_shifted } if bits_shifted == 1
            )
        },
    );
    assert_save_state_restores_continuation(
        external_clock,
        "external-clock serial transfer",
        17,
        89,
    );
}

#[test]
fn save_state_hardening_preserves_active_apu_channels_and_output_path() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(&[0x00]))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF26, 0x80);
    machine.write_bus(0xFF24, 0x77);
    machine.write_bus(0xFF25, 0x11);
    machine.write_bus(0xFF11, 0x80);
    machine.write_bus(0xFF12, 0xF3);
    machine.write_bus(0xFF13, 0x00);
    machine.write_bus(0xFF14, 0x87);

    step_until(
        &mut machine,
        512,
        "active APU channel and HPF path",
        |machine| {
            let apu = machine.apu().snapshot();
            apu.powered
                && apu.channel_active_mask & 0x01 != 0
                && apu.channel_dac_mask & 0x01 != 0
                && (apu.output.hpf_capacitor.left != 0
                    || apu.output.hpf_capacitor.right != 0
                    || apu.output.hpf_output.left != 0
                    || apu.output.hpf_output.right != 0)
        },
    );

    assert_save_state_restores_continuation(machine, "active APU output path", 191, 1_024);
}

fn assert_mapper_save_state_restores_continuation(
    label: &str,
    rom: Vec<u8>,
    expected_state: CartridgeSlotState,
    configure: impl FnOnce(&mut Machine),
) {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .unwrap_or_else(|error| panic!("{label}: cartridge should load: {error:?}"));
    configure(&mut machine);
    assert_eq!(machine.cartridge().snapshot().state, expected_state);

    assert_save_state_restores_continuation(machine, label, 61, 389);
}

#[test]
fn save_state_hardening_preserves_representative_mapper_runtime_state() {
    assert_mapper_save_state_restores_continuation(
        "NoMBC RAM runtime",
        build_test_rom_with_header(&[0x00], 0x09, 0x00, 0x02),
        CartridgeSlotState::NoMbc,
        |machine| {
            machine.write_bus(0xA000, 0x5A);
            assert_eq!(machine.read_bus(0xA000), 0x5A);
        },
    );

    assert_mapper_save_state_restores_continuation(
        "MBC1 bank/RAM runtime",
        build_banked_test_rom(&[0x00], 0x03, 0x01, 0x02),
        CartridgeSlotState::Mbc1,
        |machine| {
            machine.write_bus(0x0000, 0x0A);
            machine.write_bus(0x2000, 0x02);
            machine.write_bus(0x6000, 0x01);
            machine.write_bus(0x4000, 0x01);
            machine.write_bus(0xA000, 0x66);
            assert_eq!(machine.read_bus(0xA000), 0x66);
        },
    );

    assert_mapper_save_state_restores_continuation(
        "MBC2 bank/nibble runtime",
        build_banked_test_rom(&[0x00], 0x06, 0x01, 0x00),
        CartridgeSlotState::Mbc2,
        |machine| {
            machine.write_bus(0x0000, 0x0A);
            machine.write_bus(0x2100, 0x03);
            machine.write_bus(0xA000, 0xAB);
            assert_eq!(machine.read_bus(0xA000) & 0x0F, 0x0B);
        },
    );

    assert_mapper_save_state_restores_continuation(
        "MBC3 RAM/RTC runtime",
        build_banked_test_rom(&[0x00], 0x10, 0x01, 0x02),
        CartridgeSlotState::Mbc3,
        |machine| {
            machine.write_bus(0x0000, 0x0A);
            machine.write_bus(0x2000, 0x02);
            machine.write_bus(0x4000, 0x00);
            machine.write_bus(0xA000, 0x77);
            machine.write_bus(0x4000, 0x08);
            machine.write_bus(0xA000, 0x12);
            machine.write_bus(0x6000, 0x00);
            machine.write_bus(0x6000, 0x01);
            machine.advance_cartridge_rtc_seconds(5);
            assert!(machine.cartridge().snapshot().rtc_access_ready_at.is_some());
        },
    );

    assert_mapper_save_state_restores_continuation(
        "MBC5 bank/RAM runtime",
        build_banked_test_rom(&[0x00], 0x1B, 0x01, 0x02),
        CartridgeSlotState::Mbc5,
        |machine| {
            machine.write_bus(0x0000, 0x0A);
            machine.write_bus(0x2000, 0x03);
            machine.write_bus(0x3000, 0x00);
            machine.write_bus(0x4000, 0x01);
            machine.write_bus(0xA000, 0x99);
            assert_eq!(machine.read_bus(0xA000), 0x99);
        },
    );

    assert_mapper_save_state_restores_continuation(
        "Pocket Camera capture runtime",
        build_pocket_camera_rom(),
        CartridgeSlotState::PocketCamera,
        |machine| {
            machine
                .set_pocket_camera_frame(PocketCameraFrame {
                    width: 128,
                    height: 112,
                    grayscale_pixels: vec![0x80; 128 * 112],
                })
                .expect("Pocket Camera frame should normalize");
            machine.write_bus(0x0000, 0x0A);
            machine.write_bus(0x4000, 0x10);
            machine.write_bus(0xA000, 0x01);
            let snapshot = machine.cartridge().snapshot();
            assert!(snapshot.camera_registers_selected);
            assert!(snapshot.camera_capture_ready_at.is_some());
        },
    );
}

#[test]
fn rewind_preserves_cpu_mid_instruction_halt_and_ime_states() {
    let mut mid_instruction = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    mid_instruction
        .load_cartridge(build_test_rom(&[
            0xEA, 0x34, 0xC1, // ld ($C134),a
            0x18, 0xFE, // jr .
        ]))
        .expect("NoMBC test ROM should load");
    step_until(
        &mut mid_instruction,
        64,
        "rewind CPU mid-instruction execution state",
        |machine| {
            let cpu = machine.cpu().snapshot();
            cpu.current_opcode == Some(0xEA)
                && matches!(
                    cpu.execution_state,
                    CpuExecutionState::Execute { step, .. } if step != 0
                )
        },
    );
    assert_rewind_restores_continuation(mid_instruction, "rewind CPU mid-instruction", 37, 149);

    let mut ime_pending = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    ime_pending
        .load_cartridge(build_test_rom(&[
            0xFB, // ei
            0x00, // nop
            0x18, 0xFE, // jr .
        ]))
        .expect("NoMBC test ROM should load");
    step_until(
        &mut ime_pending,
        64,
        "rewind CPU pending IME enable",
        |machine| machine.cpu().snapshot().delayed_ime_enable,
    );
    assert_rewind_restores_continuation(ime_pending, "rewind CPU pending IME", 19, 97);

    let mut halted = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    halted
        .load_cartridge(build_test_rom(&[
            0x76, // halt
            0x00, // nop padding
        ]))
        .expect("NoMBC test ROM should load");
    step_until(&mut halted, 64, "rewind CPU HALT state", |machine| {
        machine.cpu().execution_state() == CpuExecutionState::Halted
    });
    assert_rewind_restores_continuation(halted, "rewind CPU HALT", 31, 113);
}

#[test]
fn rewind_preserves_mode3_window_fifo_and_obj_state() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(&[0x00]))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF40, 0x00);
    machine.write_bus(0xFF42, 0x00);
    machine.write_bus(0xFF43, 0x00);
    machine.write_bus(0xFF4A, 0x00);
    machine.write_bus(0xFF4B, 0x07);
    for offset in 0..16u16 {
        machine.write_bus(0x8000 + offset, 0xFF);
    }
    machine.write_bus(0x9800, 0x00);
    machine.write_bus(0x9C00, 0x00);
    machine.write_bus(0xFE00, 16);
    machine.write_bus(0xFE01, 8);
    machine.write_bus(0xFE02, 0);
    machine.write_bus(0xFE03, 0);
    machine.write_bus(0xFF40, 0xF3);

    step_until(
        &mut machine,
        1_000,
        "rewind PPU Mode 3 window FIFO with selected OBJ",
        |machine| {
            let ppu = machine.ppu().snapshot();
            ppu.mode == PpuAccessMode::Drawing
                && ppu.window_started_this_line
                && !ppu.selected_sprites.is_empty()
                && !ppu.bg_fifo_pixels.is_empty()
        },
    );

    assert_rewind_restores_continuation(machine, "rewind PPU Mode 3 window/OBJ", 83, 367);
}

#[test]
fn rewind_preserves_active_dma_and_pending_restart() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(&[0x00]))
        .expect("NoMBC test ROM should load");
    seed_dma_source_page(&mut machine, 0xC0, 0x11);
    seed_dma_source_page(&mut machine, 0xC1, 0x27);

    machine.write_bus(0xFF46, 0xC0);
    step_until(
        &mut machine,
        64,
        "rewind active OAM DMA transfer",
        |machine| {
            machine.dma().snapshot().transfer_state.lifecycle() == DmaTransferLifecycle::Active
        },
    );
    machine.write_bus(0xFF46, 0xC1);
    assert!(
        machine.dma().snapshot().pending_restart.is_some(),
        "second FF46 write during active DMA should be latched as a pending restart"
    );

    assert_rewind_restores_continuation(machine, "rewind active DMA with pending restart", 41, 777);
}

#[test]
fn rewind_preserves_timer_overflow_pipeline() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(&[0x00]))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF0F, 0x00);
    machine.write_bus(0xFF04, 0x00);
    machine.write_bus(0xFF05, 0xFF);
    machine.write_bus(0xFF06, 0x42);
    machine.write_bus(0xFF07, 0x05);
    step_t_cycles(&mut machine, 16);

    assert_eq!(machine.read_bus(0xFF05), 0x00);
    assert_eq!(machine.read_bus(0xFF0F) & 0x04, 0x00);

    assert_rewind_restores_continuation(machine, "rewind timer overflow pipeline", 5, 29);
}

#[test]
fn rewind_preserves_serial_transfers_in_flight() {
    let mut internal_clock = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    internal_clock
        .load_cartridge(build_test_rom(&[0x00]))
        .expect("NoMBC test ROM should load");
    internal_clock.set_external_port_attachment(ExternalPortAttachmentKind::Loopback);
    internal_clock.write_bus(0xFF01, 0x96);
    internal_clock.write_bus(0xFF02, 0x81);
    step_until(
        &mut internal_clock,
        1_200,
        "rewind internal-clock serial transfer in flight",
        |machine| {
            matches!(
                machine.serial().snapshot().transfer_state,
                SerialTransferState::TransferRequested { bits_shifted } if (1..8).contains(&bits_shifted)
            )
        },
    );
    assert_rewind_restores_continuation(
        internal_clock,
        "rewind internal-clock serial transfer",
        113,
        1_337,
    );

    let mut external_clock = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    external_clock
        .load_cartridge(build_test_rom(&[0x00]))
        .expect("NoMBC test ROM should load");
    external_clock.write_bus(0xFF01, 0xA5);
    external_clock.write_bus(0xFF02, 0x80);
    external_clock.queue_external_serial_clock();
    step_until(
        &mut external_clock,
        32,
        "rewind external-clock serial transfer in flight",
        |machine| {
            matches!(
                machine.serial().snapshot().transfer_state,
                SerialTransferState::TransferRequested { bits_shifted } if bits_shifted == 1
            )
        },
    );
    assert_rewind_restores_continuation(
        external_clock,
        "rewind external-clock serial transfer",
        17,
        89,
    );
}

#[test]
fn rewind_preserves_active_apu_channels_and_output_path() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(&[0x00]))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF26, 0x80);
    machine.write_bus(0xFF24, 0x77);
    machine.write_bus(0xFF25, 0x11);
    machine.write_bus(0xFF11, 0x80);
    machine.write_bus(0xFF12, 0xF3);
    machine.write_bus(0xFF13, 0x00);
    machine.write_bus(0xFF14, 0x87);

    step_until(
        &mut machine,
        512,
        "rewind active APU channel and HPF path",
        |machine| {
            let apu = machine.apu().snapshot();
            apu.powered
                && apu.channel_active_mask & 0x01 != 0
                && apu.channel_dac_mask & 0x01 != 0
                && (apu.output.hpf_capacitor.left != 0
                    || apu.output.hpf_capacitor.right != 0
                    || apu.output.hpf_output.left != 0
                    || apu.output.hpf_output.right != 0)
        },
    );

    assert_rewind_restores_continuation(machine, "rewind active APU output path", 191, 1_024);
}

fn assert_mapper_rewind_restores_continuation(
    label: &str,
    rom: Vec<u8>,
    expected_state: CartridgeSlotState,
    configure: impl FnOnce(&mut Machine),
) {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .unwrap_or_else(|error| panic!("{label}: cartridge should load: {error:?}"));
    configure(&mut machine);
    assert_eq!(machine.cartridge().snapshot().state, expected_state);

    assert_rewind_restores_continuation(machine, label, 61, 389);
}

#[test]
fn rewind_preserves_representative_mapper_runtime_state() {
    assert_mapper_rewind_restores_continuation(
        "rewind NoMBC RAM runtime",
        build_test_rom_with_header(&[0x00], 0x09, 0x00, 0x02),
        CartridgeSlotState::NoMbc,
        |machine| {
            machine.write_bus(0xA000, 0x5A);
            assert_eq!(machine.read_bus(0xA000), 0x5A);
        },
    );

    assert_mapper_rewind_restores_continuation(
        "rewind MBC1 bank/RAM runtime",
        build_banked_test_rom(&[0x00], 0x03, 0x01, 0x02),
        CartridgeSlotState::Mbc1,
        |machine| {
            machine.write_bus(0x0000, 0x0A);
            machine.write_bus(0x2000, 0x02);
            machine.write_bus(0x6000, 0x01);
            machine.write_bus(0x4000, 0x01);
            machine.write_bus(0xA000, 0x66);
            assert_eq!(machine.read_bus(0xA000), 0x66);
        },
    );

    assert_mapper_rewind_restores_continuation(
        "rewind MBC2 bank/nibble runtime",
        build_banked_test_rom(&[0x00], 0x06, 0x01, 0x00),
        CartridgeSlotState::Mbc2,
        |machine| {
            machine.write_bus(0x0000, 0x0A);
            machine.write_bus(0x2100, 0x03);
            machine.write_bus(0xA000, 0xAB);
            assert_eq!(machine.read_bus(0xA000) & 0x0F, 0x0B);
        },
    );

    assert_mapper_rewind_restores_continuation(
        "rewind MBC3 RAM/RTC runtime",
        build_banked_test_rom(&[0x00], 0x10, 0x01, 0x02),
        CartridgeSlotState::Mbc3,
        |machine| {
            machine.write_bus(0x0000, 0x0A);
            machine.write_bus(0x2000, 0x02);
            machine.write_bus(0x4000, 0x00);
            machine.write_bus(0xA000, 0x77);
            machine.write_bus(0x4000, 0x08);
            machine.write_bus(0xA000, 0x12);
            machine.write_bus(0x6000, 0x00);
            machine.write_bus(0x6000, 0x01);
            machine.advance_cartridge_rtc_seconds(5);
            assert!(machine.cartridge().snapshot().rtc_access_ready_at.is_some());
        },
    );

    assert_mapper_rewind_restores_continuation(
        "rewind MBC5 bank/RAM runtime",
        build_banked_test_rom(&[0x00], 0x1B, 0x01, 0x02),
        CartridgeSlotState::Mbc5,
        |machine| {
            machine.write_bus(0x0000, 0x0A);
            machine.write_bus(0x2000, 0x03);
            machine.write_bus(0x3000, 0x00);
            machine.write_bus(0x4000, 0x01);
            machine.write_bus(0xA000, 0x99);
            assert_eq!(machine.read_bus(0xA000), 0x99);
        },
    );

    assert_mapper_rewind_restores_continuation(
        "rewind Pocket Camera capture runtime",
        build_pocket_camera_rom(),
        CartridgeSlotState::PocketCamera,
        |machine| {
            machine
                .set_pocket_camera_frame(PocketCameraFrame {
                    width: 128,
                    height: 112,
                    grayscale_pixels: vec![0x80; 128 * 112],
                })
                .expect("Pocket Camera frame should normalize");
            machine.write_bus(0x0000, 0x0A);
            machine.write_bus(0x4000, 0x10);
            machine.write_bus(0xA000, 0x01);
            let snapshot = machine.cartridge().snapshot();
            assert!(snapshot.camera_registers_selected);
            assert!(snapshot.camera_capture_ready_at.is_some());
        },
    );
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
