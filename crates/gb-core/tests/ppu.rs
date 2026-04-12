mod common;

use common::synthetic_cartridge::{HEADER_MINIMUM_ROM_LEN, build_nom_bc_test_rom};
use gb_core::{
    ConsoleModel, CpuAddressEventKind, CpuAddressUpdateDirection, CpuBusAccessKind, Machine,
    MachineConfig, PpuAccessMode, PpuBgFetcherSource, PpuLcdState, PpuObjFetcherStage, PpuSnapshot,
    PpuVisibleOutputState, StartupMode,
};

fn build_test_rom(program: &[u8], boot_opcode: u8) -> Vec<u8> {
    build_nom_bc_test_rom(program, boot_opcode, &[])
}

fn seed_oam_entry(machine: &mut Machine, index: u8, y: u8, x: u8, tile_index: u8, attributes: u8) {
    let entry_start = 0xFE00 + index as u16 * 4;
    machine.write_bus(entry_start, y);
    machine.write_bus(entry_start + 1, x);
    machine.write_bus(entry_start + 2, tile_index);
    machine.write_bus(entry_start + 3, attributes);
}

fn seed_bg_tile_row(machine: &mut Machine, tile_index: u8, row: u8, low: u8, high: u8) {
    let tile_address = 0x8000 + tile_index as u16 * 16 + row as u16 * 2;
    machine.write_bus(tile_address, low);
    machine.write_bus(tile_address + 1, high);
}

fn seed_bg_tilemap_entry(machine: &mut Machine, x: u8, y: u8, tile_index: u8) {
    let tile_map_address = 0x9800 + y as u16 * 32 + x as u16;
    machine.write_bus(tile_map_address, tile_index);
}

fn seed_window_tilemap_entry(machine: &mut Machine, x: u8, y: u8, tile_index: u8) {
    let tile_map_address = 0x9C00 + y as u16 * 32 + x as u16;
    machine.write_bus(tile_map_address, tile_index);
}

fn build_oam_corruption_fixture() -> [[u16; 4]; 20] {
    let mut rows = [[0u16; 4]; 20];
    for (row_index, words) in rows.iter_mut().enumerate() {
        for (word_index, word) in words.iter_mut().enumerate() {
            *word = ((row_index as u16 + 1) << 8)
                | ((word_index as u16) * 0x11)
                | (row_index as u16 & 0x0F);
        }
    }
    rows
}

fn seed_oam_corruption_fixture(machine: &mut Machine, rows: &[[u16; 4]; 20]) {
    for (row_index, words) in rows.iter().enumerate() {
        for (word_index, value) in words.iter().copied().enumerate() {
            let address = 0xFE00 + row_index as u16 * 8 + word_index as u16 * 2;
            let [low, high] = value.to_le_bytes();
            machine.write_bus(address, low);
            machine.write_bus(address + 1, high);
        }
    }
}

fn read_oam_corruption_row(machine: &mut Machine, row: u8) -> [u16; 4] {
    let mut words = [0u16; 4];
    for (word_index, word) in words.iter_mut().enumerate() {
        let address = 0xFE00 + row as u16 * 8 + word_index as u16 * 2;
        let low = machine.read_bus(address);
        let high = machine.read_bus(address + 1);
        *word = u16::from_le_bytes([low, high]);
    }
    words
}

fn expected_write_corruption(rows: &[[u16; 4]; 20], row: u8) -> [u16; 4] {
    let current = rows[row as usize];
    let previous = rows[row as usize - 1];
    [
        ((current[0] ^ previous[2]) & (previous[0] ^ previous[2])) ^ previous[2],
        previous[1],
        previous[2],
        previous[3],
    ]
}

fn expected_read_corruption(rows: &[[u16; 4]; 20], row: u8) -> [u16; 4] {
    let current = rows[row as usize];
    let previous = rows[row as usize - 1];
    [
        previous[0] | (current[0] & previous[2]),
        previous[1],
        previous[2],
        previous[3],
    ]
}

fn step_until_line_dot(machine: &mut Machine, target_line_dot: u16) {
    while machine.ppu().snapshot().line_dot < target_line_dot {
        machine.step_t_cycle();
    }
}

fn step_until_hblank(machine: &mut Machine) {
    while machine.ppu().snapshot().mode != PpuAccessMode::HBlank {
        machine.step_t_cycle();
    }
}

fn step_until_position(machine: &mut Machine, target_ly: u8, target_line_dot: u16) {
    while !(machine.ppu().snapshot().ly == target_ly
        && machine.ppu().snapshot().line_dot == target_line_dot)
    {
        machine.step_t_cycle();
    }
}

fn step_until_next_frame_start(machine: &mut Machine) {
    let mut stepped = false;
    while !(stepped && machine.ppu().snapshot().ly == 0 && machine.ppu().snapshot().line_dot == 0) {
        machine.step_t_cycle();
        stepped = true;
    }
}

fn sample_after_lcd_enable<F>(machine: &mut Machine, target_m_cycle: u16, sample: F) -> u8
where
    F: Fn(&mut Machine) -> u8,
{
    for _ in 0..u32::from(target_m_cycle) * 4 {
        machine.step_t_cycle();
    }
    sample(machine)
}

fn build_lcd_enable_read_probe_rom(address: u16, delay_nops: usize) -> Vec<u8> {
    let mut program = Vec::new();
    program.push(0xF3); // di
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a
    program.extend_from_slice(&[0xEA, 0xFF, 0xFF]); // ld ($FFFF),a
    program.extend_from_slice(&[0x11, address as u8, (address >> 8) as u8]); // ld de,addr
    program.extend_from_slice(&[0x3E, 0x81]); // ld a,$81
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a
    program.extend(std::iter::repeat_n(0x00, delay_nops)); // nops
    program.push(0x1A); // ld a,(de)
    program.push(0x47); // ld b,a
    program.push(0x76); // halt
    let done_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, done_loop_pc); // jr .
    build_test_rom(&program, 0x00)
}

fn build_lcd_enable_write_probe_rom(address: u16, delay_nops: usize) -> Vec<u8> {
    let mut program = Vec::new();
    program.push(0xF3); // di
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a
    program.extend_from_slice(&[0xEA, 0xFF, 0xFF]); // ld ($FFFF),a
    program.extend_from_slice(&[0x11, address as u8, (address >> 8) as u8]); // ld de,addr
    program.extend_from_slice(&[0x3E, 0x81]); // ld a,$81
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a
    program.extend(std::iter::repeat_n(0x00, delay_nops)); // nops
    program.push(0x12); // ld (de),a

    let wait_ly_144_pc = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x44]); // ldh a,($44)
    program.extend_from_slice(&[0xFE, 0x90]); // cp $90
    emit_jr_nz(&mut program, wait_ly_144_pc); // jr nz,wait_ly_144

    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a
    program.push(0x1A); // ld a,(de)
    program.push(0x47); // ld b,a
    program.push(0x76); // halt
    let done_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, done_loop_pc); // jr .
    build_test_rom(&program, 0x00)
}

fn run_until_halted(machine: &mut Machine, max_t_cycles: usize) -> u8 {
    for _ in 0..max_t_cycles {
        machine.step_t_cycle();
        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted {
            return machine.cpu().registers().b;
        }
    }

    panic!(
        "probe ROM did not halt; pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn build_lcd_reenable_lyc_irq_probe_rom(
    lyc_before_disable: u8,
    lyc_while_off: Option<u8>,
) -> Vec<u8> {
    let mut program = Vec::new();

    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000
    program.push(0xF3); // di
    program.extend_from_slice(&[0x06, 0x00]); // ld b,$00

    let wait_ly_143_pc = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x44]); // ldh a,($44)
    program.extend_from_slice(&[0xFE, 0x8F]); // cp $8F
    emit_jr_nz(&mut program, wait_ly_143_pc); // jr nz,wait_ly_143

    let wait_ly_144_pc = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x44]); // ldh a,($44)
    program.extend_from_slice(&[0xFE, 0x90]); // cp $90
    emit_jr_nz(&mut program, wait_ly_144_pc); // jr nz,wait_ly_144

    program.extend_from_slice(&[0x3E, 0x40]); // ld a,$40
    program.extend_from_slice(&[0xE0, 0x41]); // ldh ($41),a
    program.extend_from_slice(&[0x3E, 0x02]); // ld a,$02
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh ($FF),a ; IE = STAT
    program.extend_from_slice(&[0x3E, lyc_before_disable]); // ld a,lyc_before_disable
    program.extend_from_slice(&[0xE0, 0x45]); // ldh ($45),a
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a ; disable LCD

    if let Some(lyc_while_off) = lyc_while_off {
        program.extend_from_slice(&[0x3E, lyc_while_off]); // ld a,lyc_while_off
        program.extend_from_slice(&[0xE0, 0x45]); // ldh ($45),a
    }

    program.push(0xFB); // ei
    program.push(0x00); // nop
    program.extend_from_slice(&[0x3E, 0x80]); // ld a,$80
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a ; enable LCD
    program.push(0xF3); // di
    program.push(0xF3); // di
    program.push(0x50); // ld d,b
    program.push(0x76); // halt
    let done_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, done_loop_pc); // jr .

    let mut rom = build_test_rom(&program, 0x00);
    rom[0x0048] = 0x04; // inc b
    rom[0x0049] = 0xC9; // ret
    rom
}

fn observe_lcd_reenable_lyc_irq_service_window(
    lyc_before_disable: u8,
    lyc_while_off: Option<u8>,
) -> bool {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_lcd_reenable_lyc_irq_probe_rom(
            lyc_before_disable,
            lyc_while_off,
        ))
        .expect("probe ROM should load");

    let mut saw_enable_write = false;
    let mut post_enable_tcycles = 0usize;

    for _ in 0..2_000_000 {
        machine.step_t_cycle();

        if !saw_enable_write {
            let cpu_snapshot = machine.cpu().snapshot();
            if let Some(activity) = cpu_snapshot.last_bus_activity
                && activity.kind == CpuBusAccessKind::DataWrite
                && activity.address == 0xFF40
                && activity.value == 0x80
            {
                saw_enable_write = true;
            }
            continue;
        }

        if matches!(
            machine.cpu().execution_state(),
            gb_core::CpuExecutionState::ServiceInterrupt {
                source: gb_core::InterruptSource::LcdStat,
                ..
            }
        ) {
            return true;
        }

        post_enable_tcycles += 1;
        if post_enable_tcycles >= 24 {
            return false;
        }
    }

    panic!(
        "lcd reenable lyc irq service probe did not observe the enable write; pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LcdEnableWriteProbeObservation {
    observed_value: u8,
    write_ly: u8,
    write_line_dot: u16,
    write_mode: PpuAccessMode,
    write_visible_pixels_output: u8,
}

fn run_lcd_enable_write_probe_observation(
    address: u16,
    delay_nops: u16,
) -> LcdEnableWriteProbeObservation {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_lcd_enable_write_probe_rom(
            address,
            delay_nops as usize,
        ))
        .expect("probe ROM should load");
    machine.write_bus(0xFF40, 0x00);
    machine.write_bus(address, 0x00);

    let mut write_snapshot = None;

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        if write_snapshot.is_none()
            && let Some(event) = machine.cpu().last_address_event()
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(address)
        {
            let snapshot = machine.ppu().snapshot();
            write_snapshot = Some((
                snapshot.ly,
                snapshot.line_dot,
                snapshot.mode,
                snapshot.visible_pixels_output,
            ));
        }

        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted {
            let (write_ly, write_line_dot, write_mode, write_visible_pixels_output) =
                write_snapshot.expect("probe should observe the target write");
            return LcdEnableWriteProbeObservation {
                observed_value: machine.cpu().registers().b,
                write_ly,
                write_line_dot,
                write_mode,
                write_visible_pixels_output,
            };
        }
    }

    panic!(
        "write probe did not halt; address={address:#06X} delay_nops={delay_nops} pc={:#06X} state={:?}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state()
    );
}

fn emit_jr_nz(program: &mut Vec<u8>, target_pc: u16) {
    let next_pc = 0x0100_u16 + program.len() as u16 + 2;
    let offset = target_pc as i32 - next_pc as i32;
    assert!(i8::try_from(offset).is_ok(), "jr nz target out of range");
    program.push(0x20);
    program.push(offset as i8 as u8);
}

fn emit_jr(program: &mut Vec<u8>, target_pc: u16) {
    let next_pc = 0x0100_u16 + program.len() as u16 + 2;
    let offset = target_pc as i32 - next_pc as i32;
    assert!(i8::try_from(offset).is_ok(), "jr target out of range");
    program.push(0x18);
    program.push(offset as i8 as u8);
}

fn patch_abs16(program: &mut [u8], operand_index: usize, address: u16) {
    let [low, high] = address.to_le_bytes();
    program[operand_index] = low;
    program[operand_index + 1] = high;
}

fn build_intr_2_0_probe_rom(delay_nops: usize) -> Vec<u8> {
    let mut program = Vec::new();

    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000
    program.extend_from_slice(&[0x21, 0x41, 0xFF]); // ld hl,$FF41
    program.extend_from_slice(&[0x3E, 0x02]); // ld a,$02
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh ($FF),a ; IE = STAT

    program.push(0xCD); // call setup_and_wait_mode2
    let setup_call_operand = program.len();
    program.extend_from_slice(&[0x00, 0x00]);

    program.extend(std::iter::repeat_n(0x00, delay_nops));

    program.push(0xCD); // call setup_and_wait_mode0
    let mode0_call_operand = program.len();
    program.extend_from_slice(&[0x00, 0x00]);

    program.push(0x50); // ld d,b
    program.push(0x76); // halt
    let done_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, done_loop_pc); // jr .

    let setup_mode0_pc = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0x3E, 0x08]); // ld a,$08
    program.extend_from_slice(&[0xE0, 0x41]); // ldh ($41),a
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a
    program.push(0xFB); // ei
    program.push(0xAF); // xor a
    program.push(0x47); // ld b,a
    let mode0_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x04); // inc b
    emit_jr(&mut program, mode0_loop_pc); // jr mode0_loop

    let setup_mode2_pc = 0x0100_u16 + program.len() as u16;

    let wait_ly_loop_pc = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x44]); // ldh a,($44)
    program.extend_from_slice(&[0xFE, 0x42]); // cp $42
    emit_jr_nz(&mut program, wait_ly_loop_pc); // jr nz,wait_ly

    let wait_mode0_loop_pc = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x41]); // ldh a,($41)
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    program.extend_from_slice(&[0xFE, 0x00]); // cp $00
    emit_jr_nz(&mut program, wait_mode0_loop_pc); // jr nz,wait_mode0

    let wait_mode3_loop_pc = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x41]); // ldh a,($41)
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    program.extend_from_slice(&[0xFE, 0x03]); // cp $03
    emit_jr_nz(&mut program, wait_mode3_loop_pc); // jr nz,wait_mode3

    program.extend_from_slice(&[0x3E, 0x20]); // ld a,$20
    program.extend_from_slice(&[0xE0, 0x41]); // ldh ($41),a
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a
    program.push(0xFB); // ei
    program.push(0x76); // halt
    program.push(0x00); // nop
    let fail_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, fail_loop_pc); // jr .

    patch_abs16(&mut program, setup_call_operand, setup_mode2_pc);
    patch_abs16(&mut program, mode0_call_operand, setup_mode0_pc);

    let mut rom = build_test_rom(&program, 0x00);
    rom[0x0048] = 0xE8; // add sp,+2
    rom[0x0049] = 0x02;
    rom[0x004A] = 0xC9; // ret
    rom
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Intr20ProbeObservation {
    count: u8,
    mode0_write_ly: u8,
    mode0_write_line_dot: u16,
    mode0_write_mode: PpuAccessMode,
    second_irq_ly: u8,
    second_irq_line_dot: u16,
    second_irq_mode: PpuAccessMode,
}

fn run_intr_2_0_probe(delay_nops: usize) -> Intr20ProbeObservation {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_intr_2_0_probe_rom(delay_nops))
        .expect("probe ROM should load");

    let mut stat_write_count = 0;
    let mut mode0_write = None;
    let mut second_irq = None;

    for _ in 0..1_200_000 {
        machine.step_t_cycle();

        if let Some(event) = machine.cpu().last_address_event()
            && event.kind == CpuAddressEventKind::Write
            && event.access_address == Some(0xFF41)
        {
            stat_write_count += 1;
            if stat_write_count == 2 {
                let snapshot = machine.ppu().snapshot();
                mode0_write = Some((snapshot.ly, snapshot.line_dot, snapshot.mode));
            }
        }

        if stat_write_count >= 2
            && second_irq.is_none()
            && machine.cpu().registers().b != 0
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            let snapshot = machine.ppu().snapshot();
            second_irq = Some((snapshot.ly, snapshot.line_dot, snapshot.mode));
        }

        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d != 0
        {
            let (mode0_write_ly, mode0_write_line_dot, mode0_write_mode) =
                mode0_write.expect("probe should observe second STAT write");
            let (second_irq_ly, second_irq_line_dot, second_irq_mode) =
                second_irq.expect("probe should observe second LCD STAT service");
            return Intr20ProbeObservation {
                count: machine.cpu().registers().d,
                mode0_write_ly,
                mode0_write_line_dot,
                mode0_write_mode,
                second_irq_ly,
                second_irq_line_dot,
                second_irq_mode,
            };
        }
    }

    panic!(
        "probe ROM did not halt; pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn build_intr_2_oam_ok_probe_rom(delay_nops: usize) -> Vec<u8> {
    let mut program = Vec::new();

    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000

    program.extend_from_slice(&[0x21, 0x00, 0xFE]); // ld hl,$FE00
    program.extend_from_slice(&[0x3E, 0x02]); // ld a,$02
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh ($FF),a ; IE = STAT

    program.push(0xCD); // call setup_and_wait_mode2
    let setup_call_operand = program.len();
    program.extend_from_slice(&[0x00, 0x00]);

    program.extend(std::iter::repeat_n(0x00, delay_nops));

    program.push(0x06); // ld b,$00
    program.push(0x00);
    let read_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x04); // inc b
    program.push(0x7E); // ld a,(hl)
    program.extend_from_slice(&[0xE6, 0xFF]); // and $FF
    emit_jr_nz(&mut program, read_loop_pc); // jr nz,read_loop

    program.push(0xF3); // di
    program.push(0x50); // ld d,b
    program.push(0x76); // halt
    let done_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, done_loop_pc); // jr .

    let setup_mode2_pc = 0x0100_u16 + program.len() as u16;

    let wait_ly_loop_pc = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x44]); // ldh a,($44)
    program.extend_from_slice(&[0xFE, 0x42]); // cp $42
    emit_jr_nz(&mut program, wait_ly_loop_pc); // jr nz,wait_ly

    let wait_mode0_loop_pc = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x41]); // ldh a,($41)
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    program.extend_from_slice(&[0xFE, 0x00]); // cp $00
    emit_jr_nz(&mut program, wait_mode0_loop_pc); // jr nz,wait_mode0

    let wait_mode3_loop_pc = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x41]); // ldh a,($41)
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    program.extend_from_slice(&[0xFE, 0x03]); // cp $03
    emit_jr_nz(&mut program, wait_mode3_loop_pc); // jr nz,wait_mode3

    program.extend_from_slice(&[0x3E, 0x20]); // ld a,$20
    program.extend_from_slice(&[0xE0, 0x41]); // ldh ($41),a
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a
    program.push(0xFB); // ei
    program.push(0x76); // halt
    program.push(0x00); // nop
    let fail_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, fail_loop_pc); // jr .

    patch_abs16(&mut program, setup_call_operand, setup_mode2_pc);

    let mut rom = build_test_rom(&program, 0x00);
    rom[0x0048] = 0xE8; // add sp,+2
    rom[0x0049] = 0x02;
    rom[0x004A] = 0xC9; // ret
    rom
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Intr2OamOkProbeObservation {
    count: u8,
    irq_ly: u8,
    irq_line_dot: u16,
    irq_mode: PpuAccessMode,
    halt_ly: u8,
    halt_line_dot: u16,
    halt_mode: PpuAccessMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Intr2OamOkReadObservation {
    value: u8,
    pc: u16,
    ly: u8,
    line_dot: u16,
    mode: PpuAccessMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StatLycOnOffAccessObservation {
    kind: CpuBusAccessKind,
    address: u16,
    value: u8,
    pc: u16,
    ly: u8,
    line_dot: u16,
    mode: PpuAccessMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Intr2Mode0SpritesCase1RomPathArmObservation {
    ly: u8,
    line_dot: u16,
    mode: PpuAccessMode,
    pc: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Intr2Mode0SpritesCase1RomPathTerminalObservation {
    pc: u16,
    b: u8,
    c: u8,
    ly: u8,
    line_dot: u16,
    mode: PpuAccessMode,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Intr2Mode0SpritesCase1RomPathReadSample {
    arm: Intr2Mode0SpritesCase1RomPathArmObservation,
    reads: Vec<Intr2Mode0TimingSpritesStatReadObservation>,
    terminal: Option<Intr2Mode0SpritesCase1RomPathTerminalObservation>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Intr2Mode0TimingSpritesStatReadObservation {
    pc: u16,
    value: u8,
    before_ly: u8,
    before_line_dot: u16,
    before_mode: PpuAccessMode,
    before_mode0_start_dot: u16,
    ly: u8,
    line_dot: u16,
    mode: PpuAccessMode,
    mode0_start_dot: u16,
}

fn run_intr_2_oam_ok_probe(delay_nops: usize) -> Intr2OamOkProbeObservation {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_intr_2_oam_ok_probe_rom(delay_nops))
        .expect("probe ROM should load");
    machine.write_bus(0xFE00, 0x00);

    let mut irq = None;

    for _ in 0..20_000_000 {
        machine.step_t_cycle();

        if irq.is_none()
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            let snapshot = machine.ppu().snapshot();
            irq = Some((snapshot.ly, snapshot.line_dot, snapshot.mode));
        }

        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d != 0
        {
            let snapshot = machine.ppu().snapshot();
            let (irq_ly, irq_line_dot, irq_mode) =
                irq.expect("probe should observe LCD STAT service");
            return Intr2OamOkProbeObservation {
                count: machine.cpu().registers().d,
                irq_ly,
                irq_line_dot,
                irq_mode,
                halt_ly: snapshot.ly,
                halt_line_dot: snapshot.line_dot,
                halt_mode: snapshot.mode,
            };
        }
    }

    panic!(
        "oam_ok probe did not halt; delay_nops={delay_nops} pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn sample_intr_2_oam_ok_reads(
    delay_nops: usize,
    max_reads: usize,
) -> Vec<Intr2OamOkReadObservation> {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_intr_2_oam_ok_probe_rom(delay_nops))
        .expect("probe ROM should load");
    machine.write_bus(0xFE00, 0x00);

    let mut saw_irq = false;
    let mut observations = Vec::with_capacity(max_reads);

    for _ in 0..200_000 {
        machine.step_t_cycle();

        if !saw_irq
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq = true;
        }

        if !saw_irq {
            continue;
        }

        let cpu_snapshot = machine.cpu().snapshot();
        if let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFE00
        {
            let ppu = machine.ppu().snapshot();
            observations.push(Intr2OamOkReadObservation {
                value: activity.value,
                pc: cpu_snapshot.registers.pc,
                ly: ppu.ly,
                line_dot: ppu.line_dot,
                mode: ppu.mode,
            });

            if observations.len() == max_reads {
                return observations;
            }
        }
    }

    panic!(
        "oam_ok read sample did not capture {max_reads} reads; delay_nops={delay_nops} pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn sample_real_mooneye_oam_ok_reads(max_reads: usize) -> Vec<Intr2OamOkReadObservation> {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_oam_ok_timing.gb");
    let rom = std::fs::read(&rom_path).expect("mooneye intr_2_oam_ok_timing ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut observations = Vec::with_capacity(max_reads);

    for _ in 0..500_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        if let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFE00
        {
            let ppu = machine.ppu().snapshot();
            observations.push(Intr2OamOkReadObservation {
                value: activity.value,
                pc: cpu_snapshot.registers.pc,
                ly: ppu.ly,
                line_dot: ppu.line_dot,
                mode: ppu.mode,
            });

            if observations.len() == max_reads {
                return observations;
            }
        }
    }

    panic!(
        "real mooneye oam_ok read sample did not capture {max_reads} reads; pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn sample_real_mooneye_stat_lyc_onoff_accesses(
    max_events: usize,
) -> Vec<StatLycOnOffAccessObservation> {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/stat_lyc_onoff.gb");
    let rom = std::fs::read(&rom_path).expect("mooneye stat_lyc_onoff ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut observations = Vec::with_capacity(max_events);

    for _ in 0..3_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        if let Some(activity) = cpu_snapshot.last_bus_activity
            && matches!(activity.address, 0xFF0F | 0xFF40 | 0xFF41 | 0xFF45 | 0xFFFF)
        {
            let ppu = machine.ppu().snapshot();
            observations.push(StatLycOnOffAccessObservation {
                kind: activity.kind,
                address: activity.address,
                value: activity.value,
                pc: cpu_snapshot.registers.pc,
                ly: ppu.ly,
                line_dot: ppu.line_dot,
                mode: ppu.mode,
            });

            if observations.len() == max_events {
                return observations;
            }
        }
    }

    panic!(
        "real mooneye stat_lyc_onoff access sample did not capture {max_events} events; captured={} accesses={observations:?} pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        observations.len(),
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn build_intr_2_stat_mode_probe_rom(delay_nops: usize, target_mode: u8) -> Vec<u8> {
    let mut program = Vec::new();

    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000

    program.extend_from_slice(&[0x21, 0x41, 0xFF]); // ld hl,$FF41
    program.extend_from_slice(&[0x3E, 0x02]); // ld a,$02
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh ($FF),a ; IE = STAT

    program.push(0xCD); // call setup_and_wait_mode2
    let setup_call_operand = program.len();
    program.extend_from_slice(&[0x00, 0x00]);

    program.extend(std::iter::repeat_n(0x00, delay_nops));

    program.push(0x06); // ld b,$00
    program.push(0x00);
    let read_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x04); // inc b
    program.push(0x7E); // ld a,(hl)
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    program.extend_from_slice(&[0xFE, target_mode]); // cp target_mode
    emit_jr_nz(&mut program, read_loop_pc); // jr nz,read_loop

    program.push(0xF3); // di
    program.push(0x50); // ld d,b
    program.push(0x76); // halt
    let done_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, done_loop_pc); // jr .

    let setup_mode2_pc = 0x0100_u16 + program.len() as u16;

    let wait_ly_loop_pc = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x44]); // ldh a,($44)
    program.extend_from_slice(&[0xFE, 0x42]); // cp $42
    emit_jr_nz(&mut program, wait_ly_loop_pc); // jr nz,wait_ly

    let wait_mode0_loop_pc = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x41]); // ldh a,($41)
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    program.extend_from_slice(&[0xFE, 0x00]); // cp $00
    emit_jr_nz(&mut program, wait_mode0_loop_pc); // jr nz,wait_mode0

    let wait_mode3_loop_pc = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x41]); // ldh a,($41)
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    program.extend_from_slice(&[0xFE, 0x03]); // cp $03
    emit_jr_nz(&mut program, wait_mode3_loop_pc); // jr nz,wait_mode3

    program.extend_from_slice(&[0x3E, 0x20]); // ld a,$20
    program.extend_from_slice(&[0xE0, 0x41]); // ldh ($41),a
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a
    program.push(0xFB); // ei
    program.push(0x76); // halt
    program.push(0x00); // nop
    let fail_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, fail_loop_pc); // jr .

    patch_abs16(&mut program, setup_call_operand, setup_mode2_pc);

    let mut rom = build_test_rom(&program, 0x00);
    rom[0x0048] = 0xE8; // add sp,+2
    rom[0x0049] = 0x02;
    rom[0x004A] = 0xC9; // ret
    rom
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Intr2StatModeProbeObservation {
    count: u8,
    irq_ly: u8,
    irq_line_dot: u16,
    irq_mode: PpuAccessMode,
    halt_ly: u8,
    halt_line_dot: u16,
    halt_mode: PpuAccessMode,
}

fn run_intr_2_stat_mode_probe(delay_nops: usize, target_mode: u8) -> Intr2StatModeProbeObservation {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_intr_2_stat_mode_probe_rom(delay_nops, target_mode))
        .expect("probe ROM should load");

    let mut irq = None;

    for _ in 0..500_000 {
        machine.step_t_cycle();

        if irq.is_none()
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            let snapshot = machine.ppu().snapshot();
            irq = Some((snapshot.ly, snapshot.line_dot, snapshot.mode));
        }

        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d != 0
        {
            let snapshot = machine.ppu().snapshot();
            let (irq_ly, irq_line_dot, irq_mode) =
                irq.expect("probe should observe LCD STAT service");
            return Intr2StatModeProbeObservation {
                count: machine.cpu().registers().d,
                irq_ly,
                irq_line_dot,
                irq_mode,
                halt_ly: snapshot.ly,
                halt_line_dot: snapshot.line_dot,
                halt_mode: snapshot.mode,
            };
        }
    }

    panic!(
        "stat-mode probe did not halt; target_mode={target_mode:#04X} delay_nops={delay_nops} pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn build_intr_2_mode0_sprites_case1_rom_path_probe_rom() -> Vec<u8> {
    let source_rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let source_rom = std::fs::read(&source_rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");

    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;

    rom[0x0048] = 0xE8; // add sp,+2
    rom[0x0049] = 0x02;
    rom[0x004A] = 0xC9; // ret

    rom[0x0B5B..0x0C04].copy_from_slice(&source_rom[0x0B5B..0x0C04]);
    rom[0x48C5..0x4901].copy_from_slice(&source_rom[0x48C5..0x4901]);

    let mut boot = Vec::new();
    boot.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000
    boot.push(0xF3); // di
    boot.extend_from_slice(&[0x3E, 0x02]); // ld a,$02 ; IE = STAT like the real harness state
    boot.extend_from_slice(&[0xEA, 0xFF, 0xFF]); // ld ($FFFF),a
    boot.extend_from_slice(&[0xC3, 0x29, 0x2D]); // jp $2D29 ; make the helper consume return address $2D2C like the real ROM
    rom[0x0100..0x0100 + boot.len()].copy_from_slice(&boot);

    rom[0x2D26] = 0x21; // ld hl,$0C20 ; case1 sprite spec
    rom[0x2D27] = 0x20;
    rom[0x2D28] = 0x0C;
    rom[0x2D29] = 0xCD; // call $0B5B ; second pop de sees return address $2D2C
    rom[0x2D2A] = 0x5B;
    rom[0x2D2B] = 0x0B;
    rom[0x2D2C] = 0x16; // ld d,$01 ; success marker if the helper ever returns
    rom[0x2D2D] = 0x01;
    rom[0x2D2E] = 0x76; // halt
    rom[0x2D2F] = 0x18; // jr .
    rom[0x2D30] = 0xFE;

    rom[0x0C06] = 0x16; // ld d,$FF ; failure marker from the copied compare path
    rom[0x0C07] = 0xFF;
    rom[0x0C08] = 0x76; // halt
    rom[0x0C09] = 0x18; // jr .
    rom[0x0C0A] = 0xFE;

    rom[0x0C20] = 0x02; // sprite count
    rom[0x0C21] = 0x00; // sprite 0 x
    rom[0x0C22] = 0x00; // sprite 1 x

    rom
}

fn seed_intr_2_mode0_sprites_case1_rom_path_probe_trampolines(machine: &mut Machine) {
    for address in 0xC000..=0xC02C {
        machine.write_bus(address, 0x00);
    }
    machine.write_bus(0xC02D, 0xC9);

    for address in 0xC060..=0xC08B {
        machine.write_bus(address, 0x00);
    }
    machine.write_bus(0xC08C, 0xC9);
}

fn build_intr_2_mode0_timing_sprites_real_caller_probe_rom_with_specs(
    sprite_xs: &[u8],
    delay_a: u8,
    delay_b: u8,
) -> Vec<u8> {
    let source_rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let source_rom = std::fs::read(&source_rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");

    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;

    rom[0x0048..0x004B].copy_from_slice(&source_rom[0x0048..0x004B]);
    rom[0x0B5A..0x0C33].copy_from_slice(&source_rom[0x0B5A..0x0C33]);
    rom[0x47F0..0x4901].copy_from_slice(&source_rom[0x47F0..0x4901]);

    let mut program = Vec::new();
    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000
    program.push(0xF3); // di
    program.extend_from_slice(&[0x3E, 0x02]); // ld a,$02
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh ($FF),a ; IE = STAT
    program.extend_from_slice(&[0x11, delay_b, delay_a]); // ld de,$delay_a delay_b
    program.extend_from_slice(&[0x21, 0x20, 0x0C]); // ld hl,$0C20 ; sprite spec pointer
    program.extend_from_slice(&[0xCD, 0x5A, 0x0B]); // call $0B5A
    program.extend_from_slice(&[0x3E, 0x01]); // ld a,$01 ; success marker
    program.extend_from_slice(&[0xEA, 0x0A, 0xC2]); // ld ($C20A),a
    program.extend_from_slice(&[0x16, 0x01]); // ld d,$01
    program.push(0x76); // halt
    program.extend_from_slice(&[0x18, 0xFE]); // jr .
    rom[0x0100..0x0100 + program.len()].copy_from_slice(&program);

    let sprite_count = u8::try_from(sprite_xs.len()).expect("sprite spec should fit in u8");
    rom[0x0C20] = sprite_count;
    let spec_len = sprite_xs.len();
    rom[0x0C21..0x0C21 + spec_len].copy_from_slice(sprite_xs);

    rom
}

fn sample_intr_2_mode0_sprites_case1_rom_path_probe_reads_after_arm(
    max_reads: usize,
) -> Intr2Mode0SpritesCase1RomPathReadSample {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_intr_2_mode0_sprites_case1_rom_path_probe_rom())
        .expect("probe ROM should load");
    seed_intr_2_mode0_sprites_case1_rom_path_probe_trampolines(&mut machine);

    let mut previous_ppu = machine.ppu().snapshot();
    let mut arm = None;

    for _ in 0..2_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        if let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            let ppu = machine.ppu().snapshot();
            arm = Some((ppu.ly, ppu.line_dot, ppu.mode, cpu_snapshot.registers.pc));
            break;
        }

        previous_ppu = machine.ppu().snapshot();
    }

    let (ly, line_dot, mode, pc) = arm.expect("copied case1 ROM-path probe should arm STAT");
    let arm = Intr2Mode0SpritesCase1RomPathArmObservation {
        ly,
        line_dot,
        mode,
        pc,
    };
    let mut reads = Vec::new();

    for _ in 0..20_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        if let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            let ppu = machine.ppu().snapshot();
            reads.push(Intr2Mode0TimingSpritesStatReadObservation {
                pc: cpu_snapshot.registers.pc,
                value: activity.value,
                before_ly: previous_ppu.ly,
                before_line_dot: previous_ppu.line_dot,
                before_mode: previous_ppu.mode,
                before_mode0_start_dot: previous_ppu.mode0_start_dot,
                ly: ppu.ly,
                line_dot: ppu.line_dot,
                mode: ppu.mode,
                mode0_start_dot: ppu.mode0_start_dot,
            });

            if reads.len() >= max_reads {
                return Intr2Mode0SpritesCase1RomPathReadSample {
                    arm,
                    reads,
                    terminal: None,
                };
            }
        }

        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted {
            let pc = machine.cpu().registers().pc;
            if matches!(pc, 0x010F | 0x0C09) {
                let ppu = machine.ppu().snapshot();
                return Intr2Mode0SpritesCase1RomPathReadSample {
                    arm,
                    reads,
                    terminal: Some(Intr2Mode0SpritesCase1RomPathTerminalObservation {
                        pc,
                        b: cpu_snapshot.registers.b,
                        c: cpu_snapshot.registers.c,
                        ly: ppu.ly,
                        line_dot: ppu.line_dot,
                        mode: ppu.mode,
                    }),
                };
            }
        }

        previous_ppu = machine.ppu().snapshot();
    }

    Intr2Mode0SpritesCase1RomPathReadSample {
        arm,
        reads,
        terminal: None,
    }
}

fn build_hblank_ly_scx_probe_rom(scx: u8, scanline: u8, delay_nops: usize) -> Vec<u8> {
    let mut program = Vec::new();

    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000
    let wait_ly_143_pc = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x44]); // ldh a,($44)
    program.extend_from_slice(&[0xFE, 0x8F]); // cp $8F
    emit_jr_nz(&mut program, wait_ly_143_pc); // jr nz,wait_ly_143

    let wait_ly_144_pc = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x44]); // ldh a,($44)
    program.extend_from_slice(&[0xFE, 0x90]); // cp $90
    emit_jr_nz(&mut program, wait_ly_144_pc); // jr nz,wait_ly_144

    program.extend_from_slice(&[0x21, 0x44, 0xFF]); // ld hl,$FF44
    program.extend_from_slice(&[0x3E, 0x08]); // ld a,$08
    program.extend_from_slice(&[0xE0, 0x41]); // ldh ($41),a
    program.extend_from_slice(&[0x3E, 0x02]); // ld a,$02
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh ($FF),a ; IE = STAT
    program.extend_from_slice(&[0x3E, scx]); // ld a,scx
    program.extend_from_slice(&[0xE0, 0x43]); // ldh ($43),a
    program.extend_from_slice(&[0x16, scanline.wrapping_sub(1)]); // ld d,scanline - 1

    program.push(0xCD); // call setup_and_wait
    let setup_call_operand = program.len();
    program.extend_from_slice(&[0x00, 0x00]);

    program.push(0xCD); // call standard_delay
    let standard_delay_call_operand = program.len();
    program.extend_from_slice(&[0x00, 0x00]);

    program.extend(std::iter::repeat_n(0x00, delay_nops));

    program.push(0x7E); // ld a,(hl)
    program.push(0x4F); // ld c,a
    program.extend_from_slice(&[0x06, 0x01]); // ld b,$01
    let done_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, done_loop_pc); // jr .

    while program.len() < HEADER_MINIMUM_ROM_LEN - 0x0100 {
        program.push(0x00);
    }

    let standard_delay_pc = 0x0100_u16 + program.len() as u16;
    program.extend(std::iter::repeat_n(0x00, 23));
    program.push(0xC9); // ret

    let setup_and_wait_pc = 0x0100_u16 + program.len() as u16;

    let wait_scanline_pc = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x44]); // ldh a,($44)
    program.push(0xBA); // cp d
    emit_jr_nz(&mut program, wait_scanline_pc); // jr nz,wait_scanline

    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a
    program.push(0xFB); // ei
    program.push(0x76); // halt
    program.push(0x00); // nop
    program.extend_from_slice(&[0x0E, 0xFF]); // ld c,$FF
    program.extend_from_slice(&[0x06, 0x02]); // ld b,$02
    let fail_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, fail_loop_pc); // jr .

    patch_abs16(&mut program, setup_call_operand, setup_and_wait_pc);
    patch_abs16(&mut program, standard_delay_call_operand, standard_delay_pc);

    let mut rom = build_test_rom(&program, 0x00);
    rom[0x0048] = 0xE8; // add sp,+2
    rom[0x0049] = 0x02;
    rom[0x004A] = 0xC9; // ret
    rom
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HblankLyScxProbeObservation {
    completion_marker: u8,
    observed_ly: u8,
    irq_ly: u8,
    irq_line_dot: u16,
    irq_mode: PpuAccessMode,
    ly_read_ly: u8,
    ly_read_line_dot: u16,
    ly_read_mode: PpuAccessMode,
}

fn run_hblank_ly_scx_probe(
    scx: u8,
    scanline: u8,
    delay_nops: usize,
) -> HblankLyScxProbeObservation {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_hblank_ly_scx_probe_rom(scx, scanline, delay_nops))
        .expect("probe ROM should load");

    let mut irq = None;
    let mut ly_read = None;

    for _ in 0..20_000_000 {
        machine.step_t_cycle();

        if irq.is_none()
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            let snapshot = machine.ppu().snapshot();
            irq = Some((snapshot.ly, snapshot.line_dot, snapshot.mode));
        }

        if irq.is_some()
            && ly_read.is_none()
            && let Some(event) = machine.cpu().last_address_event()
            && event.kind == CpuAddressEventKind::Read
            && event.access_address == Some(0xFF44)
        {
            let snapshot = machine.ppu().snapshot();
            ly_read = Some((snapshot.ly, snapshot.line_dot, snapshot.mode));
        }

        if machine.cpu().registers().b != 0 {
            let (irq_ly, irq_line_dot, irq_mode) = irq.expect("probe should service STAT");
            let (ly_read_ly, ly_read_line_dot, ly_read_mode) =
                ly_read.expect("probe should read LY after the STAT service");
            return HblankLyScxProbeObservation {
                completion_marker: machine.cpu().registers().b,
                observed_ly: machine.cpu().registers().c,
                irq_ly,
                irq_line_dot,
                irq_mode,
                ly_read_ly,
                ly_read_line_dot,
                ly_read_mode,
            };
        }
    }

    panic!(
        "probe ROM did not complete; scx={scx} scanline={scanline:#04X} delay_nops={delay_nops} pc={:#06X} state={:?}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state()
    );
}

#[path = "ppu/ppu_diag.rs"]
mod ppu_diag;
#[path = "ppu/ppu_lcd_restart.rs"]
mod ppu_lcd_restart;
#[path = "ppu/ppu_mode_edges.rs"]
mod ppu_mode_edges;
#[path = "ppu/ppu_oam_dma.rs"]
mod ppu_oam_dma;

#[test]
fn live_machine_bus_access_uses_the_current_ppu_mode_from_the_raster_state() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.step_t_cycle();
    assert_eq!(machine.ppu().snapshot().mode, PpuAccessMode::OamScan);
    assert_eq!(machine.read_bus(0xFE00), 0xFF);

    for _ in 1..80 {
        machine.step_t_cycle();
    }

    let drawing = machine.ppu().snapshot();
    assert_eq!(drawing.mode, PpuAccessMode::Drawing);
    assert_eq!(drawing.line_dot, 80);
    assert_eq!(machine.read_bus(0x8000), 0xFF);
}

#[test]
fn skip_boot_ppu_state_continues_from_the_published_snapshot_on_the_shared_timeline() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    let startup = machine.ppu().snapshot();
    assert_eq!(startup.mode, PpuAccessMode::VBlank);
    assert_eq!(startup.ly, 0);
    assert_eq!(startup.line_dot, 0);
    assert_eq!(startup.lcd_state, PpuLcdState::Enabled);
    assert_eq!(startup.visible_output, PpuVisibleOutputState::Driving);

    machine.step_t_cycle();

    let after_first_dot = machine.ppu().snapshot();
    assert_eq!(after_first_dot.mode, PpuAccessMode::OamScan);
    assert_eq!(after_first_dot.ly, 0);
    assert_eq!(after_first_dot.line_dot, 1);
    assert_eq!(after_first_dot.mode_dot, 1);
}

#[test]
fn bg_only_mode3_produces_visible_pixels_from_vram_on_the_machine_timeline() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_bg_tile_row(&mut machine, 0, 0, 0x55, 0x33);
    seed_bg_tile_row(&mut machine, 1, 0, 0xAA, 0xCC);
    seed_bg_tilemap_entry(&mut machine, 0, 0, 0);
    seed_bg_tilemap_entry(&mut machine, 1, 0, 1);

    step_until_line_dot(&mut machine, 252);

    let snapshot = machine.ppu().snapshot();
    assert_eq!(snapshot.mode, PpuAccessMode::HBlank);
    assert_eq!(snapshot.mode0_start_dot, 252);
    assert_eq!(snapshot.visible_pixels_output, 160);
    assert_eq!(
        &snapshot.current_scanline_pixels[..16],
        &[0, 1, 2, 3, 0, 1, 2, 3, 3, 2, 1, 0, 3, 2, 1, 0]
    );
}

#[test]
fn scx_discard_keeps_vram_blocked_until_the_variable_mode3_end() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0x8000, 0x12);
    machine.write_bus(0xFF43, 0x07);

    step_until_line_dot(&mut machine, 252);

    let extended_drawing = machine.ppu().snapshot();
    assert_eq!(extended_drawing.mode, PpuAccessMode::Drawing);
    assert_eq!(extended_drawing.mode0_start_dot, 259);
    assert_eq!(machine.read_bus(0x8000), 0xFF);

    step_until_line_dot(&mut machine, 259);

    let hblank = machine.ppu().snapshot();
    assert_eq!(hblank.mode, PpuAccessMode::HBlank);
    assert_eq!(hblank.mode0_start_dot, 259);
    assert_eq!(machine.read_bus(0x8000), 0x12);
}

#[test]
fn window_starts_mid_scanline_on_the_live_machine_without_recomputing_the_bg_prefix() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF40, 0xF1);
    machine.write_bus(0xFF4A, 0x00);
    machine.write_bus(0xFF4B, 0x0F);

    seed_bg_tile_row(&mut machine, 0, 0, 0x55, 0x33);
    seed_bg_tile_row(&mut machine, 1, 0, 0xCC, 0xF0);
    seed_bg_tilemap_entry(&mut machine, 0, 0, 0);
    seed_window_tilemap_entry(&mut machine, 0, 0, 1);

    step_until_line_dot(&mut machine, 270);

    let snapshot = machine.ppu().snapshot();
    assert_eq!(snapshot.mode, PpuAccessMode::HBlank);
    assert_eq!(snapshot.bg_fetcher_source, PpuBgFetcherSource::Window);
    assert!(snapshot.window_started_this_line);
    assert_eq!(
        &snapshot.current_scanline_pixels[..16],
        &[0, 1, 2, 3, 0, 1, 2, 3, 3, 3, 2, 2, 1, 1, 0, 0]
    );
}

#[test]
fn window_status_bar_style_activation_uses_the_internal_line_counter_on_later_lines() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF40, 0xF1);
    machine.write_bus(0xFF4A, 0x01);
    machine.write_bus(0xFF4B, 0x07);

    seed_bg_tile_row(&mut machine, 0, 0, 0x55, 0x33);
    seed_bg_tile_row(&mut machine, 1, 0, 0xCC, 0xF0);
    seed_bg_tilemap_entry(&mut machine, 0, 0, 0);
    seed_window_tilemap_entry(&mut machine, 0, 0, 1);

    while !(machine.ppu().snapshot().ly == 1
        && machine.ppu().snapshot().mode == PpuAccessMode::HBlank)
    {
        machine.step_t_cycle();
    }

    let snapshot = machine.ppu().snapshot();
    assert_eq!(snapshot.window_line_counter, 0);
    assert_eq!(snapshot.bg_fetcher_source, PpuBgFetcherSource::Window);
    assert_eq!(
        &snapshot.current_scanline_pixels[..8],
        &[3, 3, 2, 2, 1, 1, 0, 0]
    );

    while !(machine.ppu().snapshot().ly == 2 && machine.ppu().snapshot().line_dot == 0) {
        machine.step_t_cycle();
    }

    assert_eq!(machine.ppu().snapshot().window_line_counter, 1);
}

#[test]
fn live_machine_obj_fetch_stretches_mode3_and_keeps_vram_blocked_until_hblank() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_oam_entry(&mut machine, 0, 16, 8, 0, 0);
    seed_bg_tile_row(&mut machine, 0, 0, 0x00, 0xFF);
    machine.write_bus(0xFF40, 0x82);

    step_until_line_dot(&mut machine, 252);

    let drawing = machine.ppu().snapshot();
    assert_eq!(drawing.mode, PpuAccessMode::Drawing);
    assert!(drawing.mode0_start_dot > 252);
    assert_eq!(machine.read_bus(0x8000), 0xFF);

    let mode0_start_dot = drawing.mode0_start_dot;
    step_until_line_dot(&mut machine, mode0_start_dot);

    let hblank = machine.ppu().snapshot();
    assert_eq!(hblank.mode, PpuAccessMode::HBlank);
    assert_eq!(hblank.mode0_start_dot, mode0_start_dot);
    assert_eq!(&hblank.current_scanline_pixels[..8], &[2; 8]);
    assert_eq!(machine.read_bus(0x8000), 0x00);
}

#[test]
fn disabling_lcdc1_during_live_object_fetch_keeps_the_timing_cost_but_drops_pixels() {
    fn run_case(disable_obj_during_fetch: bool) -> PpuSnapshot {
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );

        seed_oam_entry(&mut machine, 0, 16, 8, 0, 0);
        seed_bg_tile_row(&mut machine, 0, 0, 0x00, 0xFF);
        machine.write_bus(0xFF40, 0x82);

        let mut waited_t_cycles = 0;
        loop {
            let fetching = machine.ppu().snapshot();
            if fetching.obj_fetcher_stage != PpuObjFetcherStage::Idle {
                assert_eq!(fetching.mode, PpuAccessMode::Drawing);
                assert!(fetching.visible_pixels_output <= 1);
                break;
            }

            machine.step_t_cycle();
            waited_t_cycles += 1;
            assert!(
                waited_t_cycles < 160,
                "left-edge OBJ fetch should begin during the first visible scanline"
            );
        }

        let fetching = machine.ppu().snapshot();
        assert_eq!(fetching.mode, PpuAccessMode::Drawing);
        assert_ne!(fetching.obj_fetcher_stage, PpuObjFetcherStage::Idle);

        if disable_obj_during_fetch {
            machine.write_bus(0xFF40, 0x80);
        }

        step_until_hblank(&mut machine);

        let hblank = machine.ppu().snapshot();
        assert_eq!(hblank.mode, PpuAccessMode::HBlank);
        hblank
    }

    let enabled = run_case(false);
    let disabled = run_case(true);

    assert_eq!(disabled.mode0_start_dot, enabled.mode0_start_dot);
    assert_ne!(enabled.current_scanline_pixels[0], 0);
    assert_eq!(&disabled.current_scanline_pixels[..8], &[0; 8]);
}

#[test]
fn window_start_keeps_the_obj_fifo_alive_for_final_mixing_on_the_live_machine() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_oam_entry(&mut machine, 0, 16, 28, 1, 0x80);
    seed_bg_tile_row(&mut machine, 0, 0, 0x00, 0x00);
    seed_bg_tile_row(&mut machine, 1, 0, 0x00, 0xFF);
    seed_bg_tile_row(&mut machine, 2, 0, 0xA0, 0x00);
    seed_window_tilemap_entry(&mut machine, 0, 0, 2);
    machine.write_bus(0xFF40, 0xF3);
    machine.write_bus(0xFF4A, 0x00);
    machine.write_bus(0xFF4B, 0x1F);

    step_until_hblank(&mut machine);

    let snapshot = machine.ppu().snapshot();
    assert_eq!(snapshot.mode, PpuAccessMode::HBlank);
    assert!(snapshot.window_started_this_line);
    assert_eq!(snapshot.bg_fetcher_source, PpuBgFetcherSource::Window);
    assert_eq!(
        &snapshot.current_scanline_pixels[20..28],
        &[2, 2, 2, 2, 1, 2, 1, 2]
    );
}
