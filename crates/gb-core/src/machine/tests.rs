use super::step::{PendingPpuMmioWrite, commit_pending_ppu_mmio_write, cpu_write_targets_ppu_mmio};
use super::*;
use crate::boot::{BootRomAssets, BootRomKind};
use crate::bus::DmaMemoryRegionImpact;
use crate::cartridge::{
    CartridgeSlotState, PersistentCartState, PocketCameraFrame, PocketCameraFrameError,
};
use crate::debugger::BreakpointCondition;
use crate::dma::DmaTransferLifecycle;
use crate::external_port::{ExternalPortAttachmentKind, ExternalPortResetPolicy};
use crate::joypad::JoypadButton;
use crate::model::{ConsoleModel, ExecutionMode, OperatingMode, StartupMode};
use crate::ppu::{PpuAccessMode, PpuLcdState, PpuStepRegion, PpuVisibleOutputState};
use crate::rewind::{MachineRewindBuffer, MachineRewindConfig, MachineRewindSubframeCadence};
use crate::scheduler::{ExternalEvent, SchedulerSideEffect, TCycle};
use crate::serial::{SerialPeer, SerialTransferState};
use crate::speed::CgbSpeedMode;

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;
const PPU_DOTS_PER_LINE: u32 = 456;
const PPU_LINES_PER_FRAME: u32 = 154;

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

fn build_cgb_native_test_rom(program: &[u8]) -> Vec<u8> {
    let mut rom = build_test_rom(program);
    rom[0x0143] = 0x80;
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

fn build_cgb_banked_test_rom(
    program: &[u8],
    cartridge_type: u8,
    rom_size: u8,
    ram_size: u8,
) -> Vec<u8> {
    let mut rom = build_banked_test_rom(program, cartridge_type, rom_size, ram_size);
    rom[0x0143] = 0x80;
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

#[test]
fn timer_startup_state_override_keeps_apu_div_phase_coherent() {
    let mut machine = Machine::new(crate::model::MachineConfig::new(ConsoleModel::GameBoy));
    machine
        .load_cartridge(build_test_rom(&[0x00]))
        .expect("test ROM should load");

    machine.apply_timer_startup_state(crate::timer::TimerStartupState {
        system_counter: 0x2000,
        tima: 0x12,
        tma: 0x34,
        tac: 0xF8,
    });

    let timer = machine.timer().snapshot();
    assert_eq!(timer.system_counter, 0x2000);
    assert_eq!(timer.tima, 0x12);
    assert_eq!(timer.tma, 0x34);
    assert_eq!(timer.tac, 0x00);
    assert_eq!(machine.apu().snapshot().div_apu, 0x01);
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

fn step_until_cpu_state(
    machine: &mut Machine,
    max_t_cycles: u64,
    description: &str,
    predicate: impl Fn(CpuExecutionState) -> bool,
) {
    for _ in 0..max_t_cycles {
        if predicate(machine.cpu().execution_state()) {
            return;
        }
        machine.step_t_cycle();
    }

    panic!("timed out before reaching CPU state: {description}");
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

fn dma_seed_value(seed: u8, byte_index: u16) -> u8 {
    seed.wrapping_mul(17)
        .wrapping_add(byte_index as u8)
        .rotate_left(1)
}

fn seed_dma_source_page(machine: &mut Machine, source_page: u8, seed: u8) {
    let source_start = (source_page as u16) << 8;

    for byte_index in 0..160u16 {
        machine.write_bus(source_start + byte_index, dma_seed_value(seed, byte_index));
    }
}

fn force_cgb_speed_mode(machine: &mut Machine, speed_mode: CgbSpeedMode) {
    match speed_mode {
        CgbSpeedMode::Normal => {
            assert_eq!(machine.speed().current_speed(), CgbSpeedMode::Normal);
        }
        CgbSpeedMode::Double => {
            machine.write_bus(0xFF4D, 0x01);
            assert!(machine.speed.begin_prepared_speed_switch());
            assert_eq!(machine.speed().current_speed(), CgbSpeedMode::Double);
            assert_eq!(machine.read_bus(0xFF4D), 0xFE);
        }
    }
}

fn cgb_dma_speed_test_machine(speed_mode: CgbSpeedMode) -> Machine {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_cgb_native_test_rom(&[0x00; 16]))
        .expect("CGB native test ROM should load");
    force_cgb_speed_mode(&mut machine, speed_mode);
    assert_eq!(machine.ppu().snapshot().lcd_state, PpuLcdState::Enabled);
    machine
}

fn cgb_apu_div_event_test_machine(speed_mode: CgbSpeedMode) -> Machine {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_cgb_native_test_rom(&[0x00; 16]))
        .expect("CGB native test ROM should load");
    force_cgb_speed_mode(&mut machine, speed_mode);
    machine.write_bus(0xFF26, 0x80);
    machine
}

fn ppu_dot_position(machine: &Machine) -> u32 {
    let snapshot = machine.ppu().snapshot();
    u32::from(snapshot.ly) * PPU_DOTS_PER_LINE + u32::from(snapshot.line_dot)
}

fn ppu_dot_delta(before: u32, after: u32) -> u32 {
    if after >= before {
        after - before
    } else {
        after + PPU_DOTS_PER_LINE * PPU_LINES_PER_FRAME - before
    }
}

#[test]
fn machine_new_starts_on_the_first_t_cycle() {
    let machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    assert_eq!(machine.next_t_cycle(), TCycle::ZERO);
    assert_eq!(machine.config().console_model, ConsoleModel::GameBoy);
    assert_eq!(machine.cpu().console_model(), ConsoleModel::GameBoy);
    assert_eq!(machine.boot().startup_mode(), StartupMode::SkipBoot);
    assert!(machine.cartridge().is_empty());
    assert_eq!(
        machine.external_port().attachment_kind(),
        ExternalPortAttachmentKind::None
    );
}

#[test]
fn key1_mmio_is_live_only_for_native_cgb_mode() {
    let mut cgb = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    assert_eq!(cgb.read_bus(0xFF4D), 0x7E);
    cgb.write_bus(0xFF4D, 0x01);
    assert_eq!(cgb.read_bus(0xFF4D), 0x7F);

    let mut cgb_compat = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor)
            .with_operating_mode(OperatingMode::GbCompatible)
            .with_startup_mode(StartupMode::SkipBoot),
    );
    cgb_compat.write_bus(0xFF4D, 0x01);
    assert_eq!(cgb_compat.read_bus(0xFF4D), 0xFF);
    assert_eq!(
        cgb_compat.speed().current_speed(),
        crate::speed::CgbSpeedMode::Normal
    );

    let mut dmg = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    dmg.write_bus(0xFF4D, 0x01);
    assert_eq!(dmg.read_bus(0xFF4D), 0xFF);
    assert_eq!(
        dmg.speed().current_speed(),
        crate::speed::CgbSpeedMode::Normal
    );
}

#[test]
fn cgb_stop_with_prepared_key1_switches_speed_and_preserves_lcd_domain_during_pause() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_cgb_native_test_rom(&[
            0x3E, 0x01, // LD A,$01
            0xE0, 0x4D, // LDH ($FF4D),A
            0x10, 0x00, // STOP + padding byte
            0x00, // NOP after the speed-switch pause
        ]))
        .expect("CGB native test ROM should load");

    step_until_cpu_state(
        &mut machine,
        128,
        "prepared STOP speed-switch pause",
        |state| matches!(state, CpuExecutionState::SpeedSwitchPause { .. }),
    );

    assert_eq!(machine.read_bus(0xFF4D), 0xFE);
    assert_eq!(
        machine.speed().current_speed(),
        crate::speed::CgbSpeedMode::Double
    );
    assert_eq!(machine.cpu().registers().pc, 0x0106);

    let paused_timer_counter = machine.timer().snapshot().system_counter;
    let paused_ppu = machine.ppu().snapshot();
    step_t_cycles(&mut machine, 16);
    assert!(matches!(
        machine.cpu().execution_state(),
        CpuExecutionState::SpeedSwitchPause { .. }
    ));
    assert_eq!(
        machine.timer().snapshot().system_counter,
        paused_timer_counter
    );
    assert_eq!(machine.ppu().snapshot().ly, paused_ppu.ly);
    assert_eq!(machine.ppu().snapshot().line_dot, paused_ppu.line_dot + 16);

    step_until_cpu_state(
        &mut machine,
        u64::from(crate::speed::CGB_SPEED_SWITCH_PAUSE_T_CYCLES) + 128,
        "speed-switch pause completion",
        |state| matches!(state, CpuExecutionState::FetchOpcode { .. }),
    );
    assert_eq!(
        machine.speed().current_speed(),
        crate::speed::CgbSpeedMode::Double
    );

    let resumed_timer_counter = machine.timer().snapshot().system_counter;
    machine.step_t_cycle();
    assert_eq!(
        machine.timer().snapshot().system_counter,
        resumed_timer_counter.wrapping_add(1)
    );
}

#[test]
fn stop_forces_model_specific_visible_framebuffer_shade() {
    for (model, expected_stop_shade) in [
        (ConsoleModel::GameBoy, 0_u8),
        (ConsoleModel::GameBoyColor, 3_u8),
    ] {
        let mut machine =
            Machine::new(MachineConfig::new(model).with_startup_mode(StartupMode::SkipBoot));
        machine
            .load_cartridge(build_test_rom(&[
                0x10, 0x00, // STOP + padding byte
                0x18, 0xFE, // JR $0102 if STOP wakes unexpectedly
            ]))
            .expect("NoMBC STOP test ROM should load");

        step_until_cpu_state(&mut machine, 128, "STOP state", |state| {
            matches!(
                state,
                CpuExecutionState::Stopped | CpuExecutionState::ZombieStopped
            )
        });

        assert_eq!(
            machine.ppu().snapshot().visible_output,
            PpuVisibleOutputState::ForcedBlank,
            "{model:?} STOP should force visible output off the normal pixel pipeline"
        );
        assert!(
            machine
                .ppu()
                .framebuffer()
                .iter()
                .all(|&shade| shade == expected_stop_shade),
            "{model:?} STOP framebuffer should be filled with panel shade {expected_stop_shade}"
        );
    }
}

#[test]
fn step_t_cycle_advances_exactly_one_cycle_per_call() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyPocket)
            .with_execution_mode(ExecutionMode::Permissive),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoyPocket).with_startup_mode(StartupMode::RealBoot),
    );

    let parts = machine.into_parts();

    assert!(parts.debug_controls.breakpoints().is_empty());
    assert!(parts.debug_controls.watchpoints().is_empty());
    assert_eq!(parts.cpu.console_model(), ConsoleModel::GameBoyPocket);
    assert_eq!(parts.bus.console_model(), ConsoleModel::GameBoyPocket);
    assert_eq!(parts.apu.console_model(), ConsoleModel::GameBoyPocket);
    assert_eq!(parts.ppu.console_model(), ConsoleModel::GameBoyPocket);
    assert_eq!(parts.dma.console_model(), ConsoleModel::GameBoyPocket);
    assert_eq!(parts.timer.console_model(), ConsoleModel::GameBoyPocket);
    assert_eq!(parts.serial.console_model(), ConsoleModel::GameBoyPocket);
    assert_eq!(
        parts.external_port.attachment_kind(),
        ExternalPortAttachmentKind::None
    );
    assert_eq!(parts.boot.console_model(), ConsoleModel::GameBoyPocket);
    assert_eq!(
        parts.interrupts.console_model(),
        ConsoleModel::GameBoyPocket
    );
    assert_eq!(parts.joypad.console_model(), ConsoleModel::GameBoyPocket);
    assert_eq!(parts.boot.startup_mode(), StartupMode::RealBoot);
    assert!(parts.cartridge.is_empty());
}

#[test]
fn machine_snapshot_exposes_scheduler_trace_and_live_phase_1_subsystems() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.step_t_cycle();
    machine.step_t_cycle();

    let snapshot = machine.snapshot();

    assert_eq!(snapshot.config.console_model, ConsoleModel::GameBoy);
    assert_eq!(snapshot.scheduler.next_t_cycle, TCycle::new(2));
    assert_eq!(snapshot.trace.buffered_event_count, 42);
    assert_eq!(snapshot.debug_controls.breakpoint_count, 0);
    assert_eq!(snapshot.debug_controls.watchpoint_count, 0);
    assert_eq!(snapshot.cpu.console_model, ConsoleModel::GameBoy);
    assert_eq!(snapshot.apu.console_model, ConsoleModel::GameBoy);
    assert_eq!(snapshot.serial.console_model, ConsoleModel::GameBoy);
    assert_eq!(
        snapshot.external_port.attachment_kind(),
        ExternalPortAttachmentKind::None
    );
    assert_eq!(snapshot.interrupts.console_model, ConsoleModel::GameBoy);
    assert_eq!(snapshot.joypad.console_model, ConsoleModel::GameBoy);
    assert!(matches!(
        snapshot.cartridge.state,
        crate::CartridgeSlotState::Empty
    ));
}

#[test]
fn save_state_round_trips_exactly_at_a_t_cycle_boundary() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    let saved = source.capture_save_state();
    let mut target = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyPocket).with_startup_mode(StartupMode::SkipBoot),
    );
    let before = target.capture_save_state();

    let error = target
        .restore_save_state(&saved)
        .expect_err("model mismatch must be rejected");

    assert!(matches!(
        error,
        MachineSaveStateRestoreError::ConsoleModelMismatch {
            expected: ConsoleModel::GameBoy,
            actual: ConsoleModel::GameBoyPocket,
        }
    ));
    assert_eq!(target.capture_save_state(), before);
}

#[test]
fn save_state_restore_can_replace_runtime_boot_mapping_state() {
    let mut source = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::RealBoot),
    );
    source.write_bus(0xFF50, 0x01);
    assert!(!source.boot().is_boot_rom_mapped());
    let saved = source.capture_save_state();

    let mut target = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::RealBoot),
    );
    assert!(target.boot().is_boot_rom_mapped());

    target
        .restore_save_state(&saved)
        .expect("matching boot ROM identity should restore even when mapping state differs");

    assert_eq!(target.capture_save_state(), saved);
}

#[test]
fn cgb_real_boot_ff50_handoff_applies_boot_selected_compatible_mode() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::RealBoot),
    );

    assert!(machine.boot().is_boot_rom_mapped());
    assert_eq!(machine.config().operating_mode, OperatingMode::Cgb);
    assert_eq!(machine.bus().operating_mode(), OperatingMode::Cgb);
    assert_eq!(machine.speed().operating_mode(), OperatingMode::Cgb);

    machine.write_bus(0xFF4D, 0x01);
    assert!(machine.speed().switch_armed());

    machine.write_bus(0xFF4C, 0x04);
    machine.write_bus(0xFF50, 0x01);

    assert!(!machine.boot().is_boot_rom_mapped());
    assert_eq!(machine.config().operating_mode, OperatingMode::GbCompatible);
    assert_eq!(machine.bus().operating_mode(), OperatingMode::GbCompatible);
    assert_eq!(
        machine.speed().operating_mode(),
        OperatingMode::GbCompatible
    );
    assert!(!machine.speed().switch_armed());
    assert_eq!(machine.read_bus(0xFF4C), 0xFF);
    assert_eq!(machine.read_bus(0xFF4D), 0xFF);
    assert_eq!(machine.read_bus(0xFF4F), 0xFE);

    machine.write_bus(0xFF4C, 0x80);
    assert_eq!(machine.config().operating_mode, OperatingMode::GbCompatible);
    assert_eq!(machine.read_bus(0xFF4F), 0xFE);
}

#[test]
fn cgb_real_boot_ff50_handoff_keeps_native_mode_when_boot_selects_cgb() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::RealBoot),
    );

    machine.write_bus(0xFF4C, 0x80);
    machine.write_bus(0xFF50, 0x01);

    assert!(!machine.boot().is_boot_rom_mapped());
    assert_eq!(machine.config().operating_mode, OperatingMode::Cgb);
    assert_eq!(machine.bus().operating_mode(), OperatingMode::Cgb);
    assert_eq!(machine.speed().operating_mode(), OperatingMode::Cgb);
    assert_ne!(machine.read_bus(0xFF4D), 0xFF);
    assert_ne!(machine.read_bus(0xFF4F), 0xFF);
}

#[test]
fn cgb_real_boot_overlay_gap_routes_cartridge_header_for_compact_and_sparse_images() {
    for len in [0x0800, 0x0900] {
        let mut boot_image = vec![0xE7; len];
        boot_image[0x0000] = 0x31;
        boot_image[0x00FF] = 0x32;
        boot_image[0x0100] = 0xE1;
        boot_image[0x01FF] = 0xE2;
        if len == 0x0900 {
            boot_image[0x0200] = 0x33;
        }
        let assets = BootRomAssets::none()
            .with_bytes(BootRomKind::Cgb, boot_image)
            .expect("synthetic CGB boot image should be accepted");

        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::GameBoyColor)
                .with_startup_mode(StartupMode::RealBoot)
                .with_boot_rom_assets(assets),
        );
        machine
            .load_cartridge(build_cgb_native_test_rom(&[0xC3, 0x50, 0x01]))
            .expect("NoMBC CGB test ROM should load");

        assert!(machine.boot().is_boot_rom_mapped());
        assert_eq!(machine.read_bus(0x0000), 0x31);
        assert_eq!(machine.read_bus(0x00FF), 0x32);
        assert_eq!(machine.read_bus(0x0100), 0xC3);
        assert_eq!(machine.read_bus(0x0143), 0x80);
        assert_ne!(machine.read_bus(0x0100), 0xE1);
        assert_ne!(machine.read_bus(0x01FF), 0xE2);
    }
}

#[test]
fn save_state_restore_preserves_locked_cgb_real_boot_handoff_state() {
    let mut source = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::RealBoot),
    );
    source.write_bus(0xFF4C, 0x04);
    source.write_bus(0xFF50, 0x01);
    assert_eq!(source.config().operating_mode, OperatingMode::GbCompatible);
    assert!(!source.boot().is_boot_rom_mapped());
    let saved = source.capture_save_state();

    let mut target = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor)
            .with_startup_mode(StartupMode::RealBoot)
            .with_operating_mode(OperatingMode::GbCompatible),
    );
    assert!(target.boot().is_boot_rom_mapped());

    target
        .restore_save_state(&saved)
        .expect("matching CGB boot handoff metadata should restore");

    assert_eq!(target.capture_save_state(), saved);
    assert_eq!(target.config().operating_mode, OperatingMode::GbCompatible);
    assert!(!target.boot().is_boot_rom_mapped());
    assert_eq!(target.read_bus(0xFF4D), 0xFF);
    assert_eq!(target.read_bus(0xFF4F), 0xFE);
}

#[test]
fn save_state_capture_restore_supports_rewind_style_subframe_cadence() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
fn cgb_gdma_copies_wram_to_vram_and_stalls_cpu_until_complete() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_cgb_native_test_rom(&[
            0x00, // NOP
            0x00, // NOP
            0x00, // NOP
        ]))
        .expect("CGB native test ROM should load");
    machine.write_bus(0xFF40, 0x00);

    for offset in 0..0x20u16 {
        machine.write_bus(0xC120 + offset, 0x80 | offset as u8);
        machine.write_bus(0x8800 + offset, 0x00);
    }

    machine.write_bus(0xFF51, 0xC1);
    machine.write_bus(0xFF52, 0x20);
    machine.write_bus(0xFF53, 0x08);
    machine.write_bus(0xFF54, 0x00);
    let pc_before_gdma = machine.cpu().registers().pc;
    machine.write_bus(0xFF55, 0x01);

    step_t_cycles(&mut machine, 16);
    assert_eq!(
        machine.cpu().registers().pc,
        pc_before_gdma,
        "GDMA must stall CPU execution while the burst is active"
    );

    step_until(&mut machine, 128, "GDMA completion", |machine| {
        machine.dma().read_hdma5() == 0xFF
    });
    step_t_cycles(&mut machine, 8);

    let vram = machine.debug_vram_bytes();
    let expected: Vec<_> = (0x80u8..0xA0).collect();
    assert_eq!(&vram[0x0800..0x0820], expected.as_slice());
    assert_ne!(machine.cpu().registers().pc, pc_before_gdma);
}

#[test]
fn cgb_hdma_lcd_off_copies_one_block_and_waits_for_another_window() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_cgb_native_test_rom(&[0x00, 0x00, 0x00]))
        .expect("CGB native test ROM should load");
    machine.write_bus(0xFF40, 0x00);

    for offset in 0..0x40u16 {
        machine.write_bus(0xC120 + offset, 0x40 | offset as u8);
        machine.write_bus(0x8800 + offset, 0x00);
    }

    machine.write_bus(0xFF51, 0xC1);
    machine.write_bus(0xFF52, 0x20);
    machine.write_bus(0xFF53, 0x08);
    machine.write_bus(0xFF54, 0x00);
    machine.write_bus(0xFF55, 0x83);

    step_t_cycles(&mut machine, 96);

    let vram = machine.debug_vram_bytes();
    let expected: Vec<_> = (0x40u8..0x50).collect();
    assert_eq!(&vram[0x0800..0x0810], expected.as_slice());
    assert_eq!(&vram[0x0810..0x0840], &[0; 0x30]);
    assert_eq!(machine.dma().read_hdma5(), 0x02);
}

#[test]
fn cgb_hdma_active_block_publishes_video_bus_conflict_to_cpu_vram_access() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_cgb_native_test_rom(&[0x00, 0x00, 0x00]))
        .expect("CGB native HDMA bus-conflict test ROM should load");

    for offset in 0..0x10u16 {
        machine.write_bus(0xC120 + offset, 0xA0 | offset as u8);
        machine.write_bus(0x8800 + offset, 0x00);
    }

    machine.write_bus(0xFF51, 0xC1);
    machine.write_bus(0xFF52, 0x20);
    machine.write_bus(0xFF53, 0x08);
    machine.write_bus(0xFF54, 0x00);
    machine.write_bus(0xFF55, 0x80);
    assert!(
        !machine.dma().cpu_stall_active(),
        "HDMA must wait for a visible HBlank window before starting its block"
    );

    step_until(
        &mut machine,
        20_000,
        "first visible-HBlank HDMA block",
        |machine| machine.dma().cpu_stall_active(),
    );
    assert_eq!(machine.ppu().snapshot().mode, PpuAccessMode::HBlank);
    assert_eq!(
        machine.read_bus(0x8800),
        0xFF,
        "active HDMA must publish a video-bus conflict to CPU VRAM reads"
    );
    machine.write_bus(0x8800, 0x55);

    step_until(&mut machine, 128, "HDMA block completion", |machine| {
        machine.dma().read_hdma5() == 0xFF
    });

    let expected: Vec<_> = (0xA0u8..0xB0).collect();
    assert_eq!(
        &machine.debug_vram_bytes()[0x0800..0x0810],
        expected.as_slice(),
        "CPU VRAM writes attempted during active HDMA must be ignored in favor of DMA data"
    );
}

#[test]
fn cgb_hdma_uses_live_mbc5_rom_bank_mapping_between_blocks() {
    let mut rom = build_cgb_banked_test_rom(&[0x00; 16], 0x19, 0x01, 0x00);
    for offset in 0..0x10usize {
        rom[0x4000 + offset] = 0x10 | offset as u8;
        rom[0x8000 + 0x10 + offset] = 0x80 | offset as u8;
    }

    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(rom)
        .expect("CGB MBC5 ROM-bank HDMA test ROM should load");
    machine.write_bus(0xFF40, 0x00);
    for offset in 0..0x20u16 {
        machine.write_bus(0x8800 + offset, 0x00);
    }
    machine.write_bus(0x2000, 0x01);
    machine.write_bus(0xFF51, 0x40);
    machine.write_bus(0xFF52, 0x00);
    machine.write_bus(0xFF53, 0x08);
    machine.write_bus(0xFF54, 0x00);
    machine.write_bus(0xFF40, 0x91);
    machine.write_bus(0xFF55, 0x81);

    step_until(
        &mut machine,
        20_000,
        "first ROM-bank HDMA block",
        |machine| machine.dma().read_hdma5() == 0x00 && !machine.dma().cpu_stall_active(),
    );
    machine.write_bus(0x2000, 0x02);
    step_until(
        &mut machine,
        20_000,
        "second ROM-bank HDMA block",
        |machine| machine.dma().read_hdma5() == 0xFF,
    );

    let vram = machine.debug_vram_bytes();
    let expected_bank1: Vec<_> = (0x10u8..0x20).collect();
    let expected_bank2: Vec<_> = (0x80u8..0x90).collect();
    assert_eq!(&vram[0x0800..0x0810], expected_bank1.as_slice());
    assert_eq!(&vram[0x0810..0x0820], expected_bank2.as_slice());
}

#[test]
fn cgb_hdma_uses_live_mbc5_sram_bank_mapping_between_blocks() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_cgb_banked_test_rom(&[0x00; 16], 0x1B, 0x01, 0x03))
        .expect("CGB MBC5 SRAM-bank HDMA test ROM should load");
    machine.write_bus(0xFF40, 0x00);
    machine.write_bus(0x0000, 0x0A);
    machine.write_bus(0x4000, 0x00);
    for offset in 0..0x10u16 {
        machine.write_bus(0xA000 + offset, 0x30 | offset as u8);
        machine.write_bus(0x8800 + offset, 0x00);
    }
    machine.write_bus(0x4000, 0x01);
    for offset in 0..0x10u16 {
        machine.write_bus(0xA010 + offset, 0xB0 | offset as u8);
        machine.write_bus(0x8810 + offset, 0x00);
    }
    machine.write_bus(0x4000, 0x00);
    machine.write_bus(0xFF51, 0xA0);
    machine.write_bus(0xFF52, 0x00);
    machine.write_bus(0xFF53, 0x08);
    machine.write_bus(0xFF54, 0x00);
    machine.write_bus(0xFF40, 0x91);
    machine.write_bus(0xFF55, 0x81);

    step_until(
        &mut machine,
        20_000,
        "first SRAM-bank HDMA block",
        |machine| machine.dma().read_hdma5() == 0x00 && !machine.dma().cpu_stall_active(),
    );
    machine.write_bus(0x4000, 0x01);
    step_until(
        &mut machine,
        20_000,
        "second SRAM-bank HDMA block",
        |machine| machine.dma().read_hdma5() == 0xFF,
    );

    let vram = machine.debug_vram_bytes();
    let expected_bank0: Vec<_> = (0x30u8..0x40).collect();
    let expected_bank1: Vec<_> = (0xB0u8..0xC0).collect();
    assert_eq!(&vram[0x0800..0x0810], expected_bank0.as_slice());
    assert_eq!(&vram[0x0810..0x0820], expected_bank1.as_slice());
}

#[test]
fn cgb_model_keeps_existing_mappers_owned_by_cartridge() {
    let mut mbc1 = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    mbc1.load_cartridge(build_cgb_banked_test_rom(&[0x00; 16], 0x03, 0x01, 0x02))
        .expect("CGB MBC1 cartridge should load");
    assert_eq!(mbc1.cartridge().snapshot().state, CartridgeSlotState::Mbc1);
    assert_eq!(mbc1.read_bus(0x4000), 0x01);
    mbc1.write_bus(0x2000, 0x02);
    assert_eq!(mbc1.read_bus(0x4000), 0x02);
    mbc1.write_bus(0x0000, 0x0A);
    mbc1.write_bus(0xA000, 0x5A);
    assert_eq!(mbc1.read_bus(0xA000), 0x5A);

    let mut mbc2 = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    mbc2.load_cartridge(build_cgb_banked_test_rom(&[0x00; 16], 0x06, 0x01, 0x00))
        .expect("CGB MBC2 cartridge should load");
    assert_eq!(mbc2.cartridge().snapshot().state, CartridgeSlotState::Mbc2);
    mbc2.write_bus(0x2100, 0x03);
    assert_eq!(mbc2.read_bus(0x4000), 0x03);
    mbc2.write_bus(0x0000, 0x0A);
    mbc2.write_bus(0xA123, 0xAB);
    assert_eq!(mbc2.read_bus(0xA123) & 0x0F, 0x0B);

    let mut mbc3 = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    mbc3.load_cartridge(build_cgb_banked_test_rom(&[0x00; 16], 0x10, 0x01, 0x03))
        .expect("CGB MBC3 cartridge should load");
    assert_eq!(mbc3.cartridge().snapshot().state, CartridgeSlotState::Mbc3);
    mbc3.write_bus(0x2000, 0x02);
    assert_eq!(mbc3.read_bus(0x4000), 0x02);
    mbc3.write_bus(0x0000, 0x0A);
    mbc3.write_bus(0x4000, 0x00);
    mbc3.write_bus(0xA000, 0x6C);
    assert_eq!(mbc3.read_bus(0xA000), 0x6C);
    mbc3.write_bus(0x4000, 0x08);
    mbc3.write_bus(0xA000, 0x10);
    mbc3.write_bus(0x6000, 0x00);
    mbc3.write_bus(0x6000, 0x01);
    assert_eq!(mbc3.read_bus(0xA000), 0x10);
    mbc3.advance_mbc3_cartridge_rtc_clock_ticks(32_768);
    mbc3.write_bus(0x6000, 0x00);
    mbc3.write_bus(0x6000, 0x01);
    assert_eq!(mbc3.read_bus(0xA000), 0x11);

    let mut mbc5 = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    mbc5.load_cartridge(build_cgb_banked_test_rom(&[0x00; 16], 0x1B, 0x01, 0x03))
        .expect("CGB MBC5 cartridge should load");
    assert_eq!(mbc5.cartridge().snapshot().state, CartridgeSlotState::Mbc5);
    mbc5.write_bus(0x2000, 0x03);
    mbc5.write_bus(0x3000, 0x00);
    assert_eq!(mbc5.read_bus(0x4000), 0x03);
    mbc5.write_bus(0x0000, 0x0A);
    mbc5.write_bus(0x4000, 0x00);
    mbc5.write_bus(0xA000, 0x21);
    mbc5.write_bus(0x4000, 0x01);
    mbc5.write_bus(0xA000, 0x43);
    assert_eq!(mbc5.read_bus(0xA000), 0x43);
    mbc5.write_bus(0x4000, 0x00);
    assert_eq!(mbc5.read_bus(0xA000), 0x21);
}

#[test]
fn cgb_hdma_uses_live_destination_vbk_between_blocks() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_cgb_native_test_rom(&[0x00; 16]))
        .expect("CGB native HDMA VBK test ROM should load");
    machine.write_bus(0xFF40, 0x00);
    for offset in 0..0x20u16 {
        machine.write_bus(0xC120 + offset, 0x60 | offset as u8);
    }
    for bank in 0..=1 {
        machine.write_bus(0xFF4F, bank);
        for offset in 0..0x20u16 {
            machine.write_bus(0x8800 + offset, 0x00);
        }
    }
    machine.write_bus(0xFF4F, 0x00);
    machine.write_bus(0xFF51, 0xC1);
    machine.write_bus(0xFF52, 0x20);
    machine.write_bus(0xFF53, 0x08);
    machine.write_bus(0xFF54, 0x00);
    machine.write_bus(0xFF40, 0x91);
    machine.write_bus(0xFF55, 0x81);

    step_until(&mut machine, 20_000, "first VBK HDMA block", |machine| {
        machine.dma().read_hdma5() == 0x00 && !machine.dma().cpu_stall_active()
    });
    machine.write_bus(0xFF4F, 0x01);
    step_until(&mut machine, 20_000, "second VBK HDMA block", |machine| {
        machine.dma().read_hdma5() == 0xFF
    });

    let vram = machine.debug_vram_bytes();
    let expected_first: Vec<_> = (0x60u8..0x70).collect();
    let expected_second: Vec<_> = (0x70u8..0x80).collect();
    assert_eq!(&vram[0x0800..0x0810], expected_first.as_slice());
    assert_eq!(&vram[0x0810..0x0820], &[0; 0x10]);
    assert_eq!(&vram[0x2000 + 0x0800..0x2000 + 0x0810], &[0; 0x10]);
    assert_eq!(
        &vram[0x2000 + 0x0810..0x2000 + 0x0820],
        expected_second.as_slice()
    );
}

#[test]
fn cgb_vram_dma_from_vram_source_copies_the_explicit_garbage_value() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_cgb_native_test_rom(&[0x00]))
        .expect("CGB native test ROM should load");
    machine.write_bus(0xFF40, 0x00);

    for offset in 0..0x20u16 {
        machine.write_bus(0x8000 + offset, offset as u8);
        machine.write_bus(0x8820 + offset, 0x00);
    }

    machine.write_bus(0xFF51, 0x80);
    machine.write_bus(0xFF52, 0x00);
    machine.write_bus(0xFF53, 0x08);
    machine.write_bus(0xFF54, 0x20);
    machine.write_bus(0xFF55, 0x00);
    step_until(&mut machine, 96, "VRAM-source GDMA completion", |machine| {
        machine.dma().read_hdma5() == 0xFF
    });

    assert_eq!(&machine.debug_vram_bytes()[0x0820..0x0830], &[0xFF; 0x10]);
}

#[test]
fn cgb_oam_dma_copy_blocking_and_lcd_duration_are_speed_domain_explicit() {
    for speed_mode in [CgbSpeedMode::Normal, CgbSpeedMode::Double] {
        let mut machine = cgb_dma_speed_test_machine(speed_mode);
        let seed = match speed_mode {
            CgbSpeedMode::Normal => 0x13,
            CgbSpeedMode::Double => 0x27,
        };
        seed_dma_source_page(&mut machine, 0xC0, seed);
        let expected_oam: Vec<_> = (0..160u16)
            .map(|byte_index| dma_seed_value(seed, byte_index))
            .collect();

        let ppu_dot_before = ppu_dot_position(&machine);
        machine.write_bus(0xFF46, 0xC0);
        let transfer = machine
            .dma()
            .current_transfer()
            .expect("FF46 should start CGB OAM DMA");
        let total_t_cycles = u64::from(transfer.timing().total_t_cycles());
        assert_eq!(transfer.oam_speed_mode(), speed_mode);

        step_t_cycles(&mut machine, 4);
        assert_eq!(
            machine.dma().bus_state().active_region(),
            None,
            "the post-write startup seam must remain CPU-visible before OAM DMA blocks the bus"
        );

        step_t_cycles(&mut machine, 1);
        assert_eq!(
            machine.dma().bus_state().active_region(),
            Some(DmaMemoryRegionImpact::Oam)
        );
        machine.write_bus(0xFF80, 0xA0 | u8::from(speed_mode == CgbSpeedMode::Double));
        assert_eq!(
            machine.read_bus(0xFF80),
            0xA0 | u8::from(speed_mode == CgbSpeedMode::Double),
            "HRAM must remain CPU-accessible while CGB OAM DMA owns the source bus and OAM"
        );

        step_t_cycles(&mut machine, 3);
        assert_eq!(machine.debug_oam_bytes()[0], expected_oam[0]);
        let pc_during_dma = machine.cpu().registers().pc;
        step_t_cycles(&mut machine, 8);
        assert_ne!(
            machine.cpu().registers().pc,
            pc_during_dma,
            "CGB OAM DMA should restrict the conflicted bus instead of fully stalling CPU execution"
        );

        step_t_cycles(&mut machine, total_t_cycles - 16);
        assert_eq!(
            machine.dma().transfer_lifecycle(),
            DmaTransferLifecycle::Completed
        );
        assert_eq!(&machine.debug_oam_bytes()[..160], expected_oam.as_slice());
        assert_eq!(
            ppu_dot_delta(ppu_dot_before, ppu_dot_position(&machine)),
            u32::from(transfer.lcd_domain_duration_dots()),
            "normal and double speed must differ in LCD-domain dots while preserving the 160 CPU-M-cycle DMA body"
        );
    }
}

#[test]
fn cgb_double_speed_does_not_shorten_hdma_block_timing() {
    let mut machine = cgb_dma_speed_test_machine(CgbSpeedMode::Double);
    machine.write_bus(0xFF40, 0x00);

    for offset in 0..0x10u16 {
        machine.write_bus(0xC120 + offset, 0xE0 | offset as u8);
        machine.write_bus(0x8840 + offset, 0x00);
    }

    machine.write_bus(0xFF51, 0xC1);
    machine.write_bus(0xFF52, 0x20);
    machine.write_bus(0xFF53, 0x08);
    machine.write_bus(0xFF54, 0x40);
    machine.write_bus(0xFF55, 0x80);

    step_t_cycles(&mut machine, 31);
    assert_eq!(machine.dma().read_hdma5(), 0x00);
    assert!(machine.dma().cpu_stall_active());

    step_t_cycles(&mut machine, 1);
    assert_eq!(machine.dma().read_hdma5(), 0xFF);
    let expected: Vec<_> = (0xE0u8..0xF0).collect();
    assert_eq!(
        &machine.debug_vram_bytes()[0x0840..0x0850],
        expected.as_slice()
    );
    assert!(
        !machine.dma().cpu_stall_active(),
        "HDMA block timing must stay in the VRAM-DMA domain instead of inheriting OAM-DMA speed handling"
    );
}

#[test]
fn cgb_double_speed_oam_dma_does_not_gate_lcd_or_apu_domains() {
    let mut with_dma = cgb_dma_speed_test_machine(CgbSpeedMode::Double);
    let mut without_dma = cgb_dma_speed_test_machine(CgbSpeedMode::Double);
    seed_dma_source_page(&mut with_dma, 0xC0, 0x31);

    with_dma.write_bus(0xFF46, 0xC0);
    step_t_cycles(&mut with_dma, 5);
    step_t_cycles(&mut without_dma, 5);
    assert_eq!(
        with_dma.dma().bus_state().active_region(),
        Some(DmaMemoryRegionImpact::Oam)
    );

    for machine in [&mut with_dma, &mut without_dma] {
        machine.apply_timer_startup_state(crate::timer::TimerStartupState {
            system_counter: 0x3FFE,
            tima: 0x00,
            tma: 0x00,
            tac: 0xF8,
        });
    }

    let ppu_with_before = ppu_dot_position(&with_dma);
    let ppu_without_before = ppu_dot_position(&without_dma);
    step_t_cycles(&mut with_dma, 16);
    step_t_cycles(&mut without_dma, 16);

    assert_eq!(
        ppu_dot_delta(ppu_with_before, ppu_dot_position(&with_dma)),
        ppu_dot_delta(ppu_without_before, ppu_dot_position(&without_dma)),
        "active OAM DMA must not gate or multiply the LCD domain"
    );
    assert_eq!(
        with_dma.apu().snapshot().div_apu,
        without_dma.apu().snapshot().div_apu,
        "active OAM DMA must not alter the double-speed APU frame-sequencer domain"
    );
    assert_eq!(with_dma.apu().snapshot().div_apu, 0x02);
    assert_eq!(
        with_dma.timer().snapshot().system_counter,
        without_dma.timer().snapshot().system_counter
    );
    assert_eq!(
        with_dma.dma().bus_state().active_region(),
        Some(DmaMemoryRegionImpact::Oam)
    );
}

#[test]
fn natural_div_apu_edges_advance_the_shared_apu_frame_sequencer() {
    let mut machine = cgb_apu_div_event_test_machine(CgbSpeedMode::Normal);
    machine.apply_timer_startup_state(crate::timer::TimerStartupState {
        system_counter: 0x1FFF,
        tima: 0x00,
        tma: 0x00,
        tac: 0xF8,
    });

    step_t_cycles(&mut machine, 1);

    assert_eq!(
        machine.apu().snapshot().div_apu,
        0x01,
        "natural timer edges must advance the APU only through the central DIV->APU frame-sequencer path"
    );
}

#[test]
fn div_mmio_writes_drive_the_same_shared_apu_event_as_natural_edges() {
    let mut machine = cgb_apu_div_event_test_machine(CgbSpeedMode::Normal);
    machine.apply_timer_startup_state(crate::timer::TimerStartupState {
        system_counter: 0x1000,
        tima: 0x00,
        tma: 0x00,
        tac: 0xF8,
    });

    machine.write_bus(0xFF04, 0x00);

    assert_eq!(
        machine.apu().snapshot().div_apu,
        0x01,
        "DIV writes must feed the same central DIV->APU event instead of a separate channel-specific route"
    );
}

fn write_div_when_visible_div_is_0x10(machine: &mut Machine) {
    while machine.read_bus(0xFF04) != 0x10 {
        machine.step_t_cycle();
    }
    machine.write_bus(0xFF04, 0x00);
}

#[test]
fn nr52_power_on_while_div_apu_signal_is_high_skips_the_first_div_write_length_clock() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_cgb_native_test_rom(&[0x00; 16]))
        .expect("CGB native test ROM should load");
    machine.apply_timer_startup_state(crate::timer::TimerStartupState {
        system_counter: 0x1000,
        tima: 0x00,
        tma: 0x00,
        tac: 0xF8,
    });

    machine.write_bus(0xFF26, 0x00);
    machine.write_bus(0xFF26, 0x80);

    assert_eq!(machine.apu().snapshot().div_apu, 0x07);

    machine.write_bus(0xFF13, 0xFF);
    machine.write_bus(0xFF11, 0xBF);
    machine.write_bus(0xFF12, 0xBF);
    machine.write_bus(0xFF14, 0xC1);
    assert_eq!(machine.read_bus(0xFF26) & 0x01, 0x01);

    write_div_when_visible_div_is_0x10(&mut machine);

    assert_eq!(
        machine.read_bus(0xFF26) & 0x01,
        0x01,
        "NR52 power-on during the high DIV-APU half must consume the timer-owned signal and skip the first write-induced length clock"
    );
}

#[test]
fn cgb_double_speed_apu_edges_consume_the_slice2_speed_domain_contract() {
    let mut machine = cgb_apu_div_event_test_machine(CgbSpeedMode::Double);
    machine.apply_timer_startup_state(crate::timer::TimerStartupState {
        system_counter: 0x1FFF,
        tima: 0x00,
        tma: 0x00,
        tac: 0xF8,
    });

    step_t_cycles(&mut machine, 1);
    assert_eq!(
        machine.apu().snapshot().div_apu,
        0x00,
        "double speed must not clock the APU from the normal-speed DIV bit"
    );

    machine.apply_timer_startup_state(crate::timer::TimerStartupState {
        system_counter: 0x3FFF,
        tima: 0x00,
        tma: 0x00,
        tac: 0xF8,
    });
    assert_eq!(machine.apu().snapshot().div_apu, 0x01);

    step_t_cycles(&mut machine, 1);

    assert_eq!(
        machine.apu().snapshot().div_apu,
        0x02,
        "double speed must consume the Slice 2 DIV/APU bit instead of creating a second frame-sequencer route"
    );
}

#[test]
fn save_state_hardening_preserves_timer_overflow_pipeline() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
fn save_state_hardening_preserves_cgb_fast_serial_and_rp_latches() {
    let mut serial = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    serial
        .load_cartridge(build_cgb_native_test_rom(&[0x00]))
        .expect("CGB NoMBC test ROM should load");
    serial.set_external_port_attachment(ExternalPortAttachmentKind::Loopback);
    serial.write_bus(0xFF01, 0x96);
    serial.write_bus(0xFF02, 0x83);
    step_until(
        &mut serial,
        64,
        "CGB high-speed internal serial transfer in flight",
        |machine| {
            machine.serial().snapshot().cgb_high_speed_clock
                && matches!(
                    machine.serial().snapshot().transfer_state,
                    SerialTransferState::TransferRequested { bits_shifted } if (1..8).contains(&bits_shifted)
                )
        },
    );
    assert_save_state_restores_continuation(
        serial,
        "CGB high-speed internal serial transfer",
        19,
        257,
    );

    let mut rp = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoyColor).with_startup_mode(StartupMode::SkipBoot),
    );
    rp.load_cartridge(build_cgb_native_test_rom(&[0x00]))
        .expect("CGB NoMBC test ROM should load");
    rp.write_bus(0xFF56, 0xC1);
    assert_eq!(rp.read_bus(0xFF56), 0xFF);

    let saved = rp.capture_save_state();
    let mut uninterrupted = rp.clone();
    step_t_cycles(&mut uninterrupted, 32);

    rp.write_bus(0xFF56, 0x00);
    assert_eq!(rp.read_bus(0xFF56), 0x3E);
    step_t_cycles(&mut rp, 7);
    rp.restore_save_state(&saved)
        .expect("matching CGB RP save-state should restore");
    assert_eq!(rp.read_bus(0xFF56), 0xFF);
    step_t_cycles(&mut rp, 32);
    assert_eq!(rp.capture_save_state(), uninterrupted.capture_save_state());
}

#[test]
fn save_state_hardening_preserves_active_apu_channels_and_output_path() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
    let mut ppu = Ppu::new(ConsoleModel::GameBoy);
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
fn cgb_palette_ppu_mmio_commit_route_is_native_cgb_only() {
    let native =
        crate::bus::Bus::new_with_operating_mode(ConsoleModel::GameBoyColor, OperatingMode::Cgb);
    let compatible = crate::bus::Bus::new_with_operating_mode(
        ConsoleModel::GameBoyColor,
        OperatingMode::GbCompatible,
    );
    let dmg = crate::bus::Bus::new(ConsoleModel::GameBoy);

    assert!(cpu_write_targets_ppu_mmio(&native, 0xFF68));
    assert!(cpu_write_targets_ppu_mmio(&native, 0xFF69));
    assert!(!cpu_write_targets_ppu_mmio(&compatible, 0xFF68));
    assert!(!cpu_write_targets_ppu_mmio(&compatible, 0xFF69));
    assert!(!cpu_write_targets_ppu_mmio(&dmg, 0xFF68));
    assert!(!cpu_write_targets_ppu_mmio(&dmg, 0xFF69));
}

#[test]
fn cpu_ppu_mmio_writes_commit_during_phase_7_of_the_same_t_cycle() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::RealBoot),
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
