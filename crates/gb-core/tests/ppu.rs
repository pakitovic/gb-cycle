use gb_core::ppu::{
    PpuBgPushDispositionSnapshot, PpuMode3TransferBackingSnapshot, PpuMode3TransferDotKindSnapshot,
    PpuMode3TransferLaneSnapshot, PpuMode3TransferReadinessSnapshot,
    PpuMode3TransferSourceWindowSnapshot,
};
use gb_core::{
    ConsoleModel, CpuAddressEventKind, CpuAddressUpdateDirection, CpuBusAccessKind, Machine,
    MachineConfig, PpuAccessMode, PpuBgFetcherSource, PpuLcdState, PpuObjFetcherStage, PpuSnapshot,
    PpuVisibleOutputState, StartupMode,
};

const HEADER_MINIMUM_ROM_LEN: usize = 0x0150;

fn build_test_rom(program: &[u8], boot_opcode: u8) -> Vec<u8> {
    let mut rom = vec![0xFF; HEADER_MINIMUM_ROM_LEN.max(32 * 1024)];
    rom[0x0000] = boot_opcode;
    for (offset, byte) in program.iter().copied().enumerate() {
        rom[0x0100 + offset] = byte;
    }
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom
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
struct Intr2Mode0TimingSpritesFailureObservation {
    testcase_index: u8,
    pc: u16,
    b: u8,
    c: u8,
    d: u8,
    e: u8,
    ly: u8,
    line_dot: u16,
    mode: PpuAccessMode,
    mode0_start_dot: u16,
    selected_sprites_len: usize,
    visible_pixels_output: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Intr2Mode0TimingSpritesAccessObservation {
    testcase_index: u8,
    kind: CpuBusAccessKind,
    address: u16,
    value: u8,
    pc: u16,
    ly: u8,
    line_dot: u16,
    mode: PpuAccessMode,
    mode0_start_dot: u16,
    selected_sprites_len: usize,
    visible_pixels_output: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Intr2Mode0SpritesProbeObservation {
    count: u8,
    irq_ly: u8,
    irq_line_dot: u16,
    irq_mode: PpuAccessMode,
    halt_ly: u8,
    halt_line_dot: u16,
    halt_mode: PpuAccessMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Intr2Mode0SpritesTwoRoundObservation {
    round_a_count: u8,
    round_b_count: u8,
    halt_ly: u8,
    halt_line_dot: u16,
    halt_mode: PpuAccessMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Intr2Mode0SpritesSingleReadObservation {
    value: u8,
    irq_ly: u8,
    irq_line_dot: u16,
    irq_mode: PpuAccessMode,
    halt_ly: u8,
    halt_line_dot: u16,
    halt_mode: PpuAccessMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Intr2Mode0TimingSpritesIrqObservation {
    armed_ly: u8,
    armed_line_dot: u16,
    armed_mode: PpuAccessMode,
    irq_ly: u8,
    irq_line_dot: u16,
    irq_mode: PpuAccessMode,
    irq_pc: u16,
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
struct Intr2Mode0TimingSpritesOpcodeFetchObservation {
    pc: u16,
    value: u8,
    ly: u8,
    line_dot: u16,
    mode: PpuAccessMode,
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct Intr2Mode0TimingSpritesLine68Observation {
    line_dot: u16,
    mode: PpuAccessMode,
    mode0_start_dot: u16,
    current_transfer_x: u8,
    visible_pixels_output: u8,
    current_transfer_lane: Option<PpuMode3TransferLaneSnapshot>,
    current_transfer_source_window: Option<PpuMode3TransferSourceWindowSnapshot>,
    current_transfer_backing: Option<PpuMode3TransferBackingSnapshot>,
    current_transfer_readiness: Option<PpuMode3TransferReadinessSnapshot>,
    bg_fifo_len: usize,
    startup_fifo_placeholders: u8,
    obj_fetcher_stage: gb_core::PpuObjFetcherStage,
    obj_fetcher_stage_dot: u8,
    obj_pending_hit_match_x: Option<u8>,
    obj_pending_hit_len: usize,
    obj_pending_hit_front_sprite_slot: Option<u8>,
    bg_fetcher_stage: gb_core::PpuBgFetcherStage,
    bg_fetcher_stage_dot: u8,
    selected_sprites_len: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Intr2Mode0TimingSpritesTerminalWindowObservation {
    line_dot: u16,
    mode: PpuAccessMode,
    mode0_start_dot: u16,
    current_transfer_x: u8,
    visible_pixels_output: u8,
    current_transfer_lane: Option<PpuMode3TransferLaneSnapshot>,
    current_transfer_source_window: Option<PpuMode3TransferSourceWindowSnapshot>,
    current_transfer_backing: Option<PpuMode3TransferBackingSnapshot>,
    current_transfer_readiness: Option<PpuMode3TransferReadinessSnapshot>,
    current_transfer_kind: Option<PpuMode3TransferDotKindSnapshot>,
    bg_fifo_len: usize,
    startup_fifo_placeholders: u8,
    bg_push_pending: bool,
    bg_push_disposition: PpuBgPushDispositionSnapshot,
    bg_push_entry_delay_remaining: u8,
    bg_fetcher_stage: gb_core::PpuBgFetcherStage,
    bg_fetcher_stage_dot: u8,
    obj_fetcher_stage: gb_core::PpuObjFetcherStage,
    obj_fetcher_stage_dot: u8,
    obj_pending_hit_len: usize,
    obj_fetched_same_x_active_count: usize,
    obj_fetched_same_x_pending_count: usize,
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

fn sample_real_mooneye_intr_2_mode0_timing_sprites_failure()
-> Intr2Mode0TimingSpritesFailureObservation {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    for _ in 0..5_000_000 {
        machine.step_t_cycle();

        let pc = machine.cpu().registers().pc;
        if (0x484D..0x4870).contains(&pc) {
            let registers = machine.cpu().registers();
            let snapshot = machine.ppu().snapshot();
            return Intr2Mode0TimingSpritesFailureObservation {
                testcase_index: machine.read_bus(0xFF80),
                pc,
                b: registers.b,
                c: registers.c,
                d: registers.d,
                e: registers.e,
                ly: snapshot.ly,
                line_dot: snapshot.line_dot,
                mode: snapshot.mode,
                mode0_start_dot: snapshot.mode0_start_dot,
                selected_sprites_len: snapshot.selected_sprites.len(),
                visible_pixels_output: snapshot.visible_pixels_output,
            };
        }
    }

    panic!(
        "real mooneye intr_2_mode0_timing_sprites sample did not reach failure path; pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn sample_real_mooneye_intr_2_mode0_timing_sprites_accesses(
    max_tail_events: usize,
) -> (
    Intr2Mode0TimingSpritesFailureObservation,
    Vec<Intr2Mode0TimingSpritesAccessObservation>,
    Vec<Intr2Mode0TimingSpritesAccessObservation>,
) {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut capture_active = false;
    let mut setup_events = Vec::new();
    let mut tail_events = Vec::with_capacity(max_tail_events);
    let mut last_ff44_read_value = None;

    for _ in 0..5_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if let Some(activity) = cpu_snapshot.last_bus_activity
            && matches!(activity.address, 0xFE00 | 0xFF40 | 0xFF41 | 0xFF44)
        {
            if !capture_active
                && testcase_index == 0
                && activity.kind == CpuBusAccessKind::DataWrite
                && matches!(activity.address, 0xFE00 | 0xFF40 | 0xFF41)
            {
                capture_active = true;
            }

            if capture_active {
                let snapshot = machine.ppu().snapshot();
                let observation = Intr2Mode0TimingSpritesAccessObservation {
                    testcase_index,
                    kind: activity.kind,
                    address: activity.address,
                    value: activity.value,
                    pc: cpu_snapshot.registers.pc,
                    ly: snapshot.ly,
                    line_dot: snapshot.line_dot,
                    mode: snapshot.mode,
                    mode0_start_dot: snapshot.mode0_start_dot,
                    selected_sprites_len: snapshot.selected_sprites.len(),
                    visible_pixels_output: snapshot.visible_pixels_output,
                };

                let is_ff44_poll =
                    activity.kind == CpuBusAccessKind::DataRead && activity.address == 0xFF44;
                if !is_ff44_poll {
                    setup_events.push(observation);
                } else {
                    let value_changed = last_ff44_read_value != Some(activity.value);
                    last_ff44_read_value = Some(activity.value);
                    if value_changed || snapshot.ly >= 143 {
                        if tail_events.len() == max_tail_events {
                            tail_events.remove(0);
                        }
                        tail_events.push(observation);
                    }
                }
            }
        }

        let pc = machine.cpu().registers().pc;
        if (0x484D..0x4871).contains(&pc) {
            let registers = machine.cpu().registers();
            let snapshot = machine.ppu().snapshot();
            return (
                Intr2Mode0TimingSpritesFailureObservation {
                    testcase_index,
                    pc,
                    b: registers.b,
                    c: registers.c,
                    d: registers.d,
                    e: registers.e,
                    ly: snapshot.ly,
                    line_dot: snapshot.line_dot,
                    mode: snapshot.mode,
                    mode0_start_dot: snapshot.mode0_start_dot,
                    selected_sprites_len: snapshot.selected_sprites.len(),
                    visible_pixels_output: snapshot.visible_pixels_output,
                },
                setup_events,
                tail_events,
            );
        }
    }

    panic!(
        "real mooneye intr_2_mode0_timing_sprites access sample did not reach failure path; pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn sample_real_mooneye_intr_2_mode0_timing_sprites_tail_accesses_before_current_failure(
    max_tail_events: usize,
) -> (
    Intr2Mode0TimingSpritesFailureObservation,
    Vec<Intr2Mode0TimingSpritesAccessObservation>,
) {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut tail_events = Vec::with_capacity(max_tail_events);
    let mut last_ff44_read_value = None;

    for _ in 0..5_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if let Some(activity) = cpu_snapshot.last_bus_activity
            && matches!(activity.address, 0xFE00..=0xFE27 | 0xFF40..=0xFF44)
        {
            let snapshot = machine.ppu().snapshot();
            let observation = Intr2Mode0TimingSpritesAccessObservation {
                testcase_index,
                kind: activity.kind,
                address: activity.address,
                value: activity.value,
                pc: cpu_snapshot.registers.pc,
                ly: snapshot.ly,
                line_dot: snapshot.line_dot,
                mode: snapshot.mode,
                mode0_start_dot: snapshot.mode0_start_dot,
                selected_sprites_len: snapshot.selected_sprites.len(),
                visible_pixels_output: snapshot.visible_pixels_output,
            };

            let is_ff44_poll =
                activity.kind == CpuBusAccessKind::DataRead && activity.address == 0xFF44;
            if !is_ff44_poll {
                if tail_events.len() == max_tail_events {
                    tail_events.remove(0);
                }
                tail_events.push(observation);
            } else {
                let value_changed = last_ff44_read_value != Some(activity.value);
                last_ff44_read_value = Some(activity.value);
                if value_changed {
                    if tail_events.len() == max_tail_events {
                        tail_events.remove(0);
                    }
                    tail_events.push(observation);
                }
            }
        }

        let registers = machine.cpu().registers();
        if registers.pc == 0x486E {
            let snapshot = machine.ppu().snapshot();
            return (
                Intr2Mode0TimingSpritesFailureObservation {
                    testcase_index,
                    pc: registers.pc,
                    b: registers.b,
                    c: registers.c,
                    d: registers.d,
                    e: registers.e,
                    ly: snapshot.ly,
                    line_dot: snapshot.line_dot,
                    mode: snapshot.mode,
                    mode0_start_dot: snapshot.mode0_start_dot,
                    selected_sprites_len: snapshot.selected_sprites.len(),
                    visible_pixels_output: snapshot.visible_pixels_output,
                },
                tail_events,
            );
        }
    }

    panic!(
        "real mooneye intr_2_mode0_timing_sprites tail-access sample did not reach current failure; pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn sample_real_mooneye_intr_2_mode0_timing_sprites_irq_after_stat_arm_for_testcase(
    target_testcase_index: u8,
) -> Option<Intr2Mode0TimingSpritesIrqObservation> {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut armed = None;

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if armed.is_none()
            && testcase_index == target_testcase_index
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            let ppu = machine.ppu().snapshot();
            armed = Some((ppu.ly, ppu.line_dot, ppu.mode));
        }

        if let Some((armed_ly, armed_line_dot, armed_mode)) = armed
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            let ppu = machine.ppu().snapshot();
            return Some(Intr2Mode0TimingSpritesIrqObservation {
                armed_ly,
                armed_line_dot,
                armed_mode,
                irq_ly: ppu.ly,
                irq_line_dot: ppu.line_dot,
                irq_mode: ppu.mode,
                irq_pc: cpu_snapshot.registers.pc,
            });
        }

        if testcase_index > target_testcase_index {
            return None;
        }
    }

    panic!(
        "real mooneye intr_2_mode0_timing_sprites irq sample did not terminate; pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn sample_real_mooneye_intr_2_mode0_timing_sprites_irq_after_stat_arm()
-> Option<Intr2Mode0TimingSpritesIrqObservation> {
    sample_real_mooneye_intr_2_mode0_timing_sprites_irq_after_stat_arm_for_testcase(0)
}

fn sample_real_mooneye_intr_2_mode0_timing_sprites_opcode_fetches_after_irq_for_testcase(
    target_testcase_index: u8,
    max_fetches: usize,
) -> Vec<Intr2Mode0TimingSpritesOpcodeFetchObservation> {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut armed = false;
    let mut saw_irq = false;
    let mut fetches = Vec::with_capacity(max_fetches);

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if !armed
            && testcase_index == target_testcase_index
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            armed = true;
        }

        if armed
            && !saw_irq
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

        if saw_irq
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::OpcodeFetch
        {
            let ppu = machine.ppu().snapshot();
            fetches.push(Intr2Mode0TimingSpritesOpcodeFetchObservation {
                pc: activity.address,
                value: activity.value,
                ly: ppu.ly,
                line_dot: ppu.line_dot,
                mode: ppu.mode,
            });

            if fetches.len() == max_fetches {
                return fetches;
            }
        }

        if saw_irq && testcase_index > target_testcase_index {
            return fetches;
        }
    }

    panic!(
        "real mooneye intr_2_mode0_timing_sprites opcode fetch sample did not terminate; pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn sample_real_mooneye_intr_2_mode0_timing_sprites_opcode_fetches_after_irq(
    max_fetches: usize,
) -> Vec<Intr2Mode0TimingSpritesOpcodeFetchObservation> {
    sample_real_mooneye_intr_2_mode0_timing_sprites_opcode_fetches_after_irq_for_testcase(
        0,
        max_fetches,
    )
}

fn sample_real_mooneye_intr_2_mode0_timing_sprites_stat_reads_after_irq_for_testcase(
    target_testcase_index: u8,
    max_reads: usize,
) -> Vec<Intr2Mode0TimingSpritesStatReadObservation> {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut armed = false;
    let mut saw_irq = false;
    let mut reads = Vec::with_capacity(max_reads);
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if !armed
            && testcase_index == target_testcase_index
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            armed = true;
        }

        if armed
            && !saw_irq
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

        if saw_irq
            && let Some(activity) = cpu_snapshot.last_bus_activity
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

            if reads.len() == max_reads {
                return reads;
            }
        }

        previous_ppu = machine.ppu().snapshot();

        if saw_irq && testcase_index > target_testcase_index {
            return reads;
        }
    }

    panic!(
        "real mooneye intr_2_mode0_timing_sprites stat-read sample did not terminate; pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn sample_real_mooneye_intr_2_mode0_timing_sprites_stat_reads_after_irq()
-> Vec<Intr2Mode0TimingSpritesStatReadObservation> {
    sample_real_mooneye_intr_2_mode0_timing_sprites_stat_reads_after_irq_for_testcase(0, 8)
}

fn sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(
    target_testcase_index: u8,
    target_ly: u8,
) -> Vec<Intr2Mode0TimingSpritesLine68Observation> {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut armed = false;
    let mut saw_irq = false;
    let mut observations = Vec::new();
    let mut previous = None;

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if !armed
            && testcase_index == target_testcase_index
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            armed = true;
        }

        if armed
            && !saw_irq
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

        if saw_irq {
            let ppu = machine.ppu().snapshot();
            if ppu.ly == target_ly {
                let current = Intr2Mode0TimingSpritesLine68Observation {
                    line_dot: ppu.line_dot,
                    mode: ppu.mode,
                    mode0_start_dot: ppu.mode0_start_dot,
                    current_transfer_x: ppu.bg_current_transfer_x,
                    visible_pixels_output: ppu.visible_pixels_output,
                    current_transfer_lane: ppu.bg_current_transfer_lane,
                    current_transfer_source_window: ppu.bg_current_transfer_source_window,
                    current_transfer_backing: ppu.bg_current_transfer_backing,
                    current_transfer_readiness: ppu.bg_current_transfer_readiness,
                    bg_fifo_len: ppu.bg_fifo_pixels.len(),
                    startup_fifo_placeholders: ppu.bg_startup_fifo_placeholders,
                    obj_fetcher_stage: ppu.obj_fetcher_stage,
                    obj_fetcher_stage_dot: ppu.obj_fetcher_stage_dot,
                    obj_pending_hit_match_x: ppu.obj_pending_hit_match_x,
                    obj_pending_hit_len: ppu.obj_pending_hit_len,
                    obj_pending_hit_front_sprite_slot: ppu.obj_pending_hit_front_sprite_slot,
                    bg_fetcher_stage: ppu.bg_fetcher_stage,
                    bg_fetcher_stage_dot: ppu.bg_fetcher_stage_dot,
                    selected_sprites_len: ppu.selected_sprites.len(),
                };
                if previous.as_ref() != Some(&current) {
                    observations.push(current.clone());
                    previous = Some(current);
                }
            } else if !observations.is_empty() && ppu.ly > target_ly {
                return observations;
            }
        }

        if saw_irq && testcase_index > target_testcase_index {
            return observations;
        }
    }

    panic!(
        "real mooneye intr_2_mode0_timing_sprites line68 sample did not terminate; pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn sample_real_mooneye_intr_2_mode0_timing_sprites_line68_changes()
-> Vec<Intr2Mode0TimingSpritesLine68Observation> {
    sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(0, 68)
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

fn build_intr_2_mode0_sprites_probe_rom(delay_nops: usize, sprite_x: u8) -> Vec<u8> {
    let mut program = Vec::new();

    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000

    program.extend_from_slice(&[0x3E, 0x52]); // ld a,$52 ; sprite y for LY=66
    program.extend_from_slice(&[0xEA, 0x00, 0xFE]); // ld ($FE00),a
    program.extend_from_slice(&[0x3E, sprite_x]); // ld a,sprite_x
    program.extend_from_slice(&[0xEA, 0x01, 0xFE]); // ld ($FE01),a
    program.extend_from_slice(&[0xAF]); // xor a
    program.extend_from_slice(&[0xEA, 0x02, 0xFE]); // ld ($FE02),a
    program.extend_from_slice(&[0xEA, 0x03, 0xFE]); // ld ($FE03),a

    program.extend_from_slice(&[0x3E, 0x93]); // ld a,$93 ; LCDC on + OBJ on
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a

    program.extend_from_slice(&[0x21, 0x00, 0xC0]); // ld hl,$C000
    program.extend_from_slice(&[0x0E, delay_nops as u8]); // ld c,delay_nops
    program.push(0xAF); // xor a
    let stub_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x22); // ld (hli),a
    program.push(0x0D); // dec c
    emit_jr_nz(&mut program, stub_loop_pc); // jr nz,stub_loop
    program.extend_from_slice(&[0x3E, 0xC9]); // ld a,$C9
    program.push(0x22); // ld (hli),a

    program.extend_from_slice(&[0x3E, 0x02]); // ld a,$02
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh ($FF),a ; IE = STAT

    program.extend_from_slice(&[0x21, 0x00, 0x00]); // ld hl,compare_addr
    let compare_addr_operand = program.len() - 2;
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0x21, 0x00, 0xC0]); // ld hl,$C000
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0xC3, 0x00, 0x00]); // jp wait_irq
    let wait_irq_operand = program.len() - 2;

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

    program.extend_from_slice(&[0x3E, 0x20]); // ld a,$20 ; mode0 STAT enable
    program.extend_from_slice(&[0xE0, 0x41]); // ldh ($41),a
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a
    program.push(0xFB); // ei
    program.push(0x76); // halt
    program.push(0x00); // nop
    let fail_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, fail_loop_pc); // jr .

    let compare_addr = 0x0100_u16 + program.len() as u16;
    program.push(0x06); // ld b,$00
    program.push(0x00);
    let mode0_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x04); // inc b
    program.extend_from_slice(&[0xF0, 0x41]); // ldh a,($41)
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    emit_jr_nz(&mut program, mode0_loop_pc); // jr nz,mode0_loop

    program.push(0x50); // ld d,b
    program.push(0x76); // halt
    let done_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, done_loop_pc); // jr .

    patch_abs16(&mut program, compare_addr_operand, compare_addr);
    patch_abs16(&mut program, wait_irq_operand, wait_ly_loop_pc);

    let mut rom = build_test_rom(&program, 0x00);
    rom[0x0048] = 0xE8; // add sp,+2
    rom[0x0049] = 0x02;
    rom[0x004A] = 0xC9; // ret
    rom
}

fn build_intr_2_mode0_sprites_multi_probe_rom_with_scx(
    delay_nops: usize,
    sprite_xs: &[u8],
    scx: u8,
) -> Vec<u8> {
    build_intr_2_mode0_sprites_multi_probe_rom_with_setup(delay_nops, sprite_xs, scx, 0x91, 0x93)
}

fn build_intr_2_mode0_sprites_multi_probe_rom_with_setup(
    delay_nops: usize,
    sprite_xs: &[u8],
    scx: u8,
    initial_lcdc: u8,
    second_lcdc: u8,
) -> Vec<u8> {
    let mut program = Vec::new();

    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000

    program.extend_from_slice(&[0x3E, 0x11]); // ld a,$11 ; LCD off, baseline bits
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a
    program.extend_from_slice(&[0x3E, scx]); // ld a,scx
    program.extend_from_slice(&[0xE0, 0x43]); // ldh ($43),a

    for (sprite_index, sprite_x) in sprite_xs.iter().copied().enumerate() {
        let entry_address = 0xFE00_u16 + sprite_index as u16 * 4;
        program.extend_from_slice(&[0x3E, 0x52]); // ld a,$52 ; sprite y for LY=66
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&entry_address.to_le_bytes());
        program.extend_from_slice(&[0x3E, sprite_x]); // ld a,sprite_x
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 1).to_le_bytes());
        program.push(0xAF); // xor a
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 2).to_le_bytes());
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 3).to_le_bytes());
    }

    program.extend_from_slice(&[0x3E, initial_lcdc]); // ld a,initial_lcdc
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a
    program.extend(std::iter::repeat_n(0x00, 6)); // 24 dots
    program.extend_from_slice(&[0x3E, second_lcdc]); // ld a,second_lcdc
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a

    program.extend_from_slice(&[0x21, 0x00, 0xC0]); // ld hl,$C000
    program.extend_from_slice(&[0x0E, delay_nops as u8]); // ld c,delay_nops
    program.push(0xAF); // xor a
    let stub_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x22); // ld (hli),a
    program.push(0x0D); // dec c
    emit_jr_nz(&mut program, stub_loop_pc); // jr nz,stub_loop
    program.extend_from_slice(&[0x3E, 0xC9]); // ld a,$C9
    program.push(0x22); // ld (hli),a

    program.extend_from_slice(&[0x3E, 0x02]); // ld a,$02
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh ($FF),a ; IE = STAT

    program.extend_from_slice(&[0x21, 0x00, 0x00]); // ld hl,compare_addr
    let compare_addr_operand = program.len() - 2;
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0x21, 0x00, 0xC0]); // ld hl,$C000
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0xC3, 0x00, 0x00]); // jp wait_irq
    let wait_irq_operand = program.len() - 2;

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

    program.extend_from_slice(&[0x3E, 0x20]); // ld a,$20 ; mode0 STAT enable
    program.extend_from_slice(&[0xE0, 0x41]); // ldh ($41),a
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a
    program.push(0xFB); // ei
    program.push(0x76); // halt
    program.push(0x00); // nop
    let fail_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, fail_loop_pc); // jr .

    let compare_addr = 0x0100_u16 + program.len() as u16;
    program.push(0x06); // ld b,$00
    program.push(0x00);
    let mode0_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x04); // inc b
    program.extend_from_slice(&[0xF0, 0x41]); // ldh a,($41)
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    emit_jr_nz(&mut program, mode0_loop_pc); // jr nz,mode0_loop

    program.push(0x50); // ld d,b
    program.push(0x76); // halt
    let done_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, done_loop_pc); // jr .

    patch_abs16(&mut program, compare_addr_operand, compare_addr);
    patch_abs16(&mut program, wait_irq_operand, wait_ly_loop_pc);

    let mut rom = build_test_rom(&program, 0x00);
    rom[0x0048] = 0xE8; // add sp,+2
    rom[0x0049] = 0x02;
    rom[0x004A] = 0xC9; // ret
    rom
}

fn build_intr_2_mode0_sprites_multi_probe_rom(delay_nops: usize, sprite_xs: &[u8]) -> Vec<u8> {
    build_intr_2_mode0_sprites_multi_probe_rom_with_scx(delay_nops, sprite_xs, 0)
}

fn build_intr_2_mode0_sprites_multi_restart_helper_probe_rom(
    delay_nops: usize,
    sprite_xs: &[u8],
) -> Vec<u8> {
    let mut program = Vec::new();

    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000

    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x42]); // ldh ($42),a
    program.extend_from_slice(&[0xE0, 0x43]); // ldh ($43),a
    program.extend_from_slice(&[0xE0, 0x4B]); // ldh ($4B),a
    program.extend_from_slice(&[0xE0, 0x4A]); // ldh ($4A),a
    program.extend_from_slice(&[0x3E, 0x11]); // ld a,$11
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a
    program.extend_from_slice(&[0x3E, 0xFC]); // ld a,$FC
    program.extend_from_slice(&[0xE0, 0x47]); // ldh ($47),a

    for byte_offset in 0..0x28_u16 {
        program.push(0xAF); // xor a
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(0xFE00_u16 + byte_offset).to_le_bytes());
    }

    for (sprite_index, sprite_x) in sprite_xs.iter().copied().enumerate() {
        let entry_address = 0xFE00_u16 + sprite_index as u16 * 4;
        program.extend_from_slice(&[0x3E, 0x52]); // ld a,$52 ; sprite y for LY=66
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&entry_address.to_le_bytes());
        program.extend_from_slice(&[0x3E, sprite_x]); // ld a,sprite_x
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 1).to_le_bytes());
        program.push(0xAF); // xor a
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 2).to_le_bytes());
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 3).to_le_bytes());
    }

    program.extend_from_slice(&[0x21, 0x40, 0xFF]); // ld hl,$FF40
    program.extend_from_slice(&[0xCB, 0xFE]); // set 7,(hl)

    let wait_8f_a_pc = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x44]); // ldh a,($44)
    program.extend_from_slice(&[0xFE, 0x8F]); // cp $8F
    emit_jr_nz(&mut program, wait_8f_a_pc); // jr nz,wait_8f_a

    let wait_90_a_pc = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x44]); // ldh a,($44)
    program.extend_from_slice(&[0xFE, 0x90]); // cp $90
    emit_jr_nz(&mut program, wait_90_a_pc); // jr nz,wait_90_a

    let wait_8f_b_pc = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x44]); // ldh a,($44)
    program.extend_from_slice(&[0xFE, 0x8F]); // cp $8F
    emit_jr_nz(&mut program, wait_8f_b_pc); // jr nz,wait_8f_b

    let wait_90_b_pc = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x44]); // ldh a,($44)
    program.extend_from_slice(&[0xFE, 0x90]); // cp $90
    emit_jr_nz(&mut program, wait_90_b_pc); // jr nz,wait_90_b

    program.extend_from_slice(&[0x21, 0x00, 0xC0]); // ld hl,$C000
    program.extend_from_slice(&[0x0E, delay_nops as u8]); // ld c,delay_nops
    program.push(0xAF); // xor a
    let stub_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x22); // ld (hli),a
    program.push(0x0D); // dec c
    emit_jr_nz(&mut program, stub_loop_pc); // jr nz,stub_loop
    program.extend_from_slice(&[0x3E, 0xC9]); // ld a,$C9
    program.push(0x22); // ld (hli),a

    program.extend_from_slice(&[0x3E, 0x02]); // ld a,$02
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh ($FF),a ; IE = STAT

    program.extend_from_slice(&[0x21, 0x00, 0x00]); // ld hl,compare_addr
    let compare_addr_operand = program.len() - 2;
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0x21, 0x00, 0xC0]); // ld hl,$C000
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0xC3, 0x00, 0x00]); // jp wait_irq
    let wait_irq_operand = program.len() - 2;

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

    program.extend_from_slice(&[0x3E, 0x20]); // ld a,$20 ; mode0 STAT enable
    program.extend_from_slice(&[0xE0, 0x41]); // ldh ($41),a
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a
    program.push(0xFB); // ei
    program.push(0x76); // halt
    program.push(0x00); // nop
    let fail_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, fail_loop_pc); // jr .

    let compare_addr = 0x0100_u16 + program.len() as u16;
    program.push(0x06); // ld b,$00
    program.push(0x00);
    let mode0_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x04); // inc b
    program.extend_from_slice(&[0xF0, 0x41]); // ldh a,($41)
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    emit_jr_nz(&mut program, mode0_loop_pc); // jr nz,mode0_loop

    program.push(0x50); // ld d,b
    program.push(0x76); // halt
    let done_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, done_loop_pc); // jr .

    patch_abs16(&mut program, compare_addr_operand, compare_addr);
    patch_abs16(&mut program, wait_irq_operand, wait_ly_loop_pc);

    let mut rom = build_test_rom(&program, 0x00);
    rom[0x0048] = 0xE8; // add sp,+2
    rom[0x0049] = 0x02;
    rom[0x004A] = 0xC9; // ret
    rom
}

fn build_intr_2_mode0_sprites_multi_live_obj_enable_probe_rom(
    delay_nops: usize,
    sprite_xs: &[u8],
) -> Vec<u8> {
    let mut program = Vec::new();

    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000
    program.extend_from_slice(&[0x3E, 0x13]); // ld a,$13 ; LCD off, OBJ bit preset like the ROM
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a

    for byte_offset in 0..0x28_u16 {
        program.push(0xAF); // xor a
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(0xFE00_u16 + byte_offset).to_le_bytes());
    }

    for (sprite_index, sprite_x) in sprite_xs.iter().copied().enumerate() {
        let entry_address = 0xFE00_u16 + sprite_index as u16 * 4;
        let tile_index = 0x30_u8 + sprite_index as u8;
        program.extend_from_slice(&[0x3E, 0x52]); // ld a,$52 ; sprite y for LY=66
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&entry_address.to_le_bytes());
        program.extend_from_slice(&[0x3E, sprite_x]); // ld a,sprite_x
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 1).to_le_bytes());
        program.extend_from_slice(&[0x3E, tile_index]); // ld a,tile_index
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 2).to_le_bytes());
        program.push(0xAF); // xor a
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 3).to_le_bytes());
    }

    program.extend_from_slice(&[0x21, 0x00, 0xC0]); // ld hl,$C000
    program.extend_from_slice(&[0x0E, delay_nops as u8]); // ld c,delay_nops
    program.push(0xAF); // xor a
    let stub_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x22); // ld (hli),a
    program.push(0x0D); // dec c
    emit_jr_nz(&mut program, stub_loop_pc); // jr nz,stub_loop
    program.extend_from_slice(&[0x3E, 0xC9]); // ld a,$C9
    program.push(0x22); // ld (hli),a

    program.extend_from_slice(&[0x21, 0x40, 0xFF]); // ld hl,$FF40
    program.extend_from_slice(&[0xCB, 0xFE]); // set 7,(hl) ; 0x13 -> 0x93? no, 0x13 -> 0x93 with bit7
    program.extend_from_slice(&[0x21, 0x40, 0xFF]); // ld hl,$FF40
    program.extend_from_slice(&[0xCB, 0xCE]); // set 1,(hl) ; live OBJ enable on already-enabled LCD

    program.extend_from_slice(&[0x3E, 0x02]); // ld a,$02
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh ($FF),a ; IE = STAT

    program.extend_from_slice(&[0x21, 0x00, 0x00]); // ld hl,compare_addr
    let compare_addr_operand = program.len() - 2;
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0x21, 0x00, 0xC0]); // ld hl,$C000
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0xC3, 0x00, 0x00]); // jp wait_irq
    let wait_irq_operand = program.len() - 2;

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

    program.extend_from_slice(&[0x3E, 0x20]); // ld a,$20 ; mode0 STAT enable
    program.extend_from_slice(&[0xE0, 0x41]); // ldh ($41),a
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a
    program.push(0xFB); // ei
    program.push(0x76); // halt
    program.push(0x00); // nop
    let fail_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, fail_loop_pc); // jr .

    let compare_addr = 0x0100_u16 + program.len() as u16;
    program.push(0x06); // ld b,$00
    program.push(0x00);
    let mode0_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x04); // inc b
    program.extend_from_slice(&[0xF0, 0x41]); // ldh a,($41)
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    emit_jr_nz(&mut program, mode0_loop_pc); // jr nz,mode0_loop

    program.push(0x50); // ld d,b
    program.push(0x76); // halt
    let done_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, done_loop_pc); // jr .

    patch_abs16(&mut program, compare_addr_operand, compare_addr);
    patch_abs16(&mut program, wait_irq_operand, wait_ly_loop_pc);

    let mut rom = build_test_rom(&program, 0x00);
    rom[0x0048] = 0xE8; // add sp,+2
    rom[0x0049] = 0x02;
    rom[0x004A] = 0xC9; // ret
    rom
}

fn build_intr_2_mode0_sprites_single_read_probe_rom(
    delay_nops: usize,
    sprite_xs: &[u8],
) -> Vec<u8> {
    let mut program = Vec::new();

    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000

    program.extend_from_slice(&[0x3E, 0x11]); // ld a,$11 ; LCD off, baseline bits
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a

    for (sprite_index, sprite_x) in sprite_xs.iter().copied().enumerate() {
        let entry_address = 0xFE00_u16 + sprite_index as u16 * 4;
        program.extend_from_slice(&[0x3E, 0x52]); // ld a,$52 ; sprite y for LY=66
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&entry_address.to_le_bytes());
        program.extend_from_slice(&[0x3E, sprite_x]); // ld a,sprite_x
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 1).to_le_bytes());
        program.push(0xAF); // xor a
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 2).to_le_bytes());
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 3).to_le_bytes());
    }

    program.extend_from_slice(&[0x3E, 0x91]); // ld a,$91 ; enable LCD without OBJ
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a
    program.extend(std::iter::repeat_n(0x00, 6)); // 24 dots
    program.extend_from_slice(&[0x3E, 0x93]); // ld a,$93 ; enable OBJ on live LCD
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a

    program.extend_from_slice(&[0x21, 0x00, 0xC0]); // ld hl,$C000
    program.extend_from_slice(&[0x0E, delay_nops as u8]); // ld c,delay_nops
    program.push(0xAF); // xor a
    let stub_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x22); // ld (hli),a
    program.push(0x0D); // dec c
    emit_jr_nz(&mut program, stub_loop_pc); // jr nz,stub_loop
    program.extend_from_slice(&[0x3E, 0xC9]); // ld a,$C9
    program.push(0x22); // ld (hli),a

    program.extend_from_slice(&[0x3E, 0x02]); // ld a,$02
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh ($FF),a ; IE = STAT

    program.extend_from_slice(&[0x21, 0x00, 0x00]); // ld hl,compare_addr
    let compare_addr_operand = program.len() - 2;
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0x21, 0x00, 0xC0]); // ld hl,$C000
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0xC3, 0x00, 0x00]); // jp wait_irq
    let wait_irq_operand = program.len() - 2;

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

    program.extend_from_slice(&[0x3E, 0x20]); // ld a,$20 ; mode0 STAT enable
    program.extend_from_slice(&[0xE0, 0x41]); // ldh ($41),a
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a
    program.push(0xFB); // ei
    program.push(0x76); // halt
    program.push(0x00); // nop
    let fail_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, fail_loop_pc); // jr .

    let compare_addr = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x41]); // ldh a,($41)
    program.push(0x57); // ld d,a
    program.push(0x76); // halt
    let done_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, done_loop_pc); // jr .

    patch_abs16(&mut program, compare_addr_operand, compare_addr);
    patch_abs16(&mut program, wait_irq_operand, wait_ly_loop_pc);

    let mut rom = build_test_rom(&program, 0x00);
    rom[0x0048] = 0xE8; // add sp,+2
    rom[0x0049] = 0x02;
    rom[0x004A] = 0xC9; // ret
    rom
}

fn build_intr_2_mode0_sprites_lcd_restart_probe_rom(delay_nops: usize, sprite_x: u8) -> Vec<u8> {
    let mut program = Vec::new();

    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000

    program.extend_from_slice(&[0x3E, 0x11]); // ld a,$11 ; LCD off, baseline bits
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a

    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xEA, 0x00, 0xFE]); // ld ($FE00),a
    program.extend_from_slice(&[0xEA, 0x01, 0xFE]); // ld ($FE01),a
    program.extend_from_slice(&[0xEA, 0x02, 0xFE]); // ld ($FE02),a
    program.extend_from_slice(&[0xEA, 0x03, 0xFE]); // ld ($FE03),a

    program.extend_from_slice(&[0x3E, 0x52]); // ld a,$52 ; sprite y for LY=66
    program.extend_from_slice(&[0xEA, 0x00, 0xFE]); // ld ($FE00),a
    program.extend_from_slice(&[0x3E, sprite_x]); // ld a,sprite_x
    program.extend_from_slice(&[0xEA, 0x01, 0xFE]); // ld ($FE01),a
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xEA, 0x02, 0xFE]); // ld ($FE02),a
    program.extend_from_slice(&[0xEA, 0x03, 0xFE]); // ld ($FE03),a

    program.extend_from_slice(&[0x3E, 0x91]); // ld a,$91 ; enable LCD without OBJ
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a
    program.extend(std::iter::repeat_n(0x00, 6)); // 24 dots
    program.extend_from_slice(&[0x3E, 0x93]); // ld a,$93 ; enable OBJ on live LCD
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a

    program.extend_from_slice(&[0x21, 0x00, 0xC0]); // ld hl,$C000
    program.extend_from_slice(&[0x0E, delay_nops as u8]); // ld c,delay_nops
    program.push(0xAF); // xor a
    let stub_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x22); // ld (hli),a
    program.push(0x0D); // dec c
    emit_jr_nz(&mut program, stub_loop_pc); // jr nz,stub_loop
    program.extend_from_slice(&[0x3E, 0xC9]); // ld a,$C9
    program.push(0x22); // ld (hli),a

    program.extend_from_slice(&[0x3E, 0x02]); // ld a,$02
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh ($FF),a ; IE = STAT

    program.extend_from_slice(&[0x21, 0x00, 0x00]); // ld hl,compare_addr
    let compare_addr_operand = program.len() - 2;
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0x21, 0x00, 0xC0]); // ld hl,$C000
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0xC3, 0x00, 0x00]); // jp wait_irq
    let wait_irq_operand = program.len() - 2;

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

    program.extend_from_slice(&[0x3E, 0x20]); // ld a,$20 ; mode0 STAT enable
    program.extend_from_slice(&[0xE0, 0x41]); // ldh ($41),a
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a
    program.push(0xFB); // ei
    program.push(0x76); // halt
    program.push(0x00); // nop
    let fail_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, fail_loop_pc); // jr .

    let compare_addr = 0x0100_u16 + program.len() as u16;
    program.push(0x06); // ld b,$00
    program.push(0x00);
    let mode0_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x04); // inc b
    program.extend_from_slice(&[0xF0, 0x41]); // ldh a,($41)
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    emit_jr_nz(&mut program, mode0_loop_pc); // jr nz,mode0_loop

    program.push(0x50); // ld d,b
    program.push(0x76); // halt
    let done_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, done_loop_pc); // jr .

    patch_abs16(&mut program, compare_addr_operand, compare_addr);
    patch_abs16(&mut program, wait_irq_operand, wait_ly_loop_pc);

    let mut rom = build_test_rom(&program, 0x00);
    rom[0x0048] = 0xE8; // add sp,+2
    rom[0x0049] = 0x02;
    rom[0x004A] = 0xC9; // ret
    rom
}

fn run_intr_2_mode0_sprites_probe(
    delay_nops: usize,
    sprite_x: u8,
) -> Intr2Mode0SpritesProbeObservation {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_intr_2_mode0_sprites_probe_rom(delay_nops, sprite_x))
        .expect("probe ROM should load");

    let mut irq = None;

    for _ in 0..1_200_000 {
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
            return Intr2Mode0SpritesProbeObservation {
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
        "mode0-sprites probe did not halt; delay_nops={delay_nops} sprite_x={sprite_x:#04X} pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn run_intr_2_mode0_sprites_multi_probe(
    delay_nops: usize,
    sprite_xs: &[u8],
) -> Intr2Mode0SpritesProbeObservation {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_intr_2_mode0_sprites_multi_probe_rom(
            delay_nops, sprite_xs,
        ))
        .expect("probe ROM should load");

    let mut irq = None;

    for _ in 0..1_200_000 {
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
            return Intr2Mode0SpritesProbeObservation {
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
        "mode0-sprites multi probe did not halt; delay_nops={delay_nops} sprite_count={} pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        sprite_xs.len(),
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn sample_intr_2_mode0_sprites_multi_probe_stat_reads(
    delay_nops: usize,
    sprite_xs: &[u8],
    max_reads: usize,
) -> Vec<Intr2Mode0TimingSpritesStatReadObservation> {
    sample_intr_2_mode0_sprites_multi_probe_stat_reads_with_setup(
        delay_nops, sprite_xs, max_reads, 0x91, 0x93,
    )
}

fn sample_intr_2_mode0_sprites_multi_probe_stat_reads_with_setup(
    delay_nops: usize,
    sprite_xs: &[u8],
    max_reads: usize,
    initial_lcdc: u8,
    second_lcdc: u8,
) -> Vec<Intr2Mode0TimingSpritesStatReadObservation> {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_intr_2_mode0_sprites_multi_probe_rom_with_setup(
            delay_nops,
            sprite_xs,
            0,
            initial_lcdc,
            second_lcdc,
        ))
        .expect("probe ROM should load");

    let mut saw_irq = false;
    let mut reads = Vec::with_capacity(max_reads);
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..1_200_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
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

        if saw_irq
            && let Some(activity) = cpu_snapshot.last_bus_activity
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

            if reads.len() == max_reads {
                return reads;
            }
        }

        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d != 0
        {
            return reads;
        }

        previous_ppu = machine.ppu().snapshot();
    }

    reads
}

fn sample_intr_2_mode0_sprites_multi_probe_line68_changes(
    delay_nops: usize,
    sprite_xs: &[u8],
) -> Vec<Intr2Mode0TimingSpritesLine68Observation> {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_intr_2_mode0_sprites_multi_probe_rom(
            delay_nops, sprite_xs,
        ))
        .expect("probe ROM should load");

    let mut saw_irq = false;
    let mut observations = Vec::new();
    let mut previous = None;

    for _ in 0..1_200_000 {
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

        if saw_irq {
            let ppu = machine.ppu().snapshot();
            if ppu.ly == 68 {
                let current = Intr2Mode0TimingSpritesLine68Observation {
                    line_dot: ppu.line_dot,
                    mode: ppu.mode,
                    mode0_start_dot: ppu.mode0_start_dot,
                    current_transfer_x: ppu.bg_current_transfer_x,
                    visible_pixels_output: ppu.visible_pixels_output,
                    current_transfer_lane: ppu.bg_current_transfer_lane,
                    current_transfer_source_window: ppu.bg_current_transfer_source_window,
                    current_transfer_backing: ppu.bg_current_transfer_backing,
                    current_transfer_readiness: ppu.bg_current_transfer_readiness,
                    bg_fifo_len: ppu.bg_fifo_pixels.len(),
                    startup_fifo_placeholders: ppu.bg_startup_fifo_placeholders,
                    obj_fetcher_stage: ppu.obj_fetcher_stage,
                    obj_fetcher_stage_dot: ppu.obj_fetcher_stage_dot,
                    obj_pending_hit_match_x: ppu.obj_pending_hit_match_x,
                    obj_pending_hit_len: ppu.obj_pending_hit_len,
                    obj_pending_hit_front_sprite_slot: ppu.obj_pending_hit_front_sprite_slot,
                    bg_fetcher_stage: ppu.bg_fetcher_stage,
                    bg_fetcher_stage_dot: ppu.bg_fetcher_stage_dot,
                    selected_sprites_len: ppu.selected_sprites.len(),
                };
                if previous.as_ref() != Some(&current) {
                    observations.push(current.clone());
                    previous = Some(current);
                }
            } else if !observations.is_empty() && ppu.ly > 68 {
                return observations;
            }
        }

        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d != 0
        {
            return observations;
        }
    }

    panic!(
        "mode0-sprites multi probe line68 sample did not terminate; delay_nops={delay_nops} sprite_count={} pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        sprite_xs.len(),
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn run_intr_2_mode0_sprites_multi_probe_with_scx(
    delay_nops: usize,
    sprite_xs: &[u8],
    scx: u8,
) -> Intr2Mode0SpritesProbeObservation {
    run_intr_2_mode0_sprites_multi_probe_with_setup(delay_nops, sprite_xs, scx, 0x91, 0x93)
}

fn run_intr_2_mode0_sprites_multi_probe_with_setup(
    delay_nops: usize,
    sprite_xs: &[u8],
    scx: u8,
    initial_lcdc: u8,
    second_lcdc: u8,
) -> Intr2Mode0SpritesProbeObservation {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_intr_2_mode0_sprites_multi_probe_rom_with_setup(
            delay_nops,
            sprite_xs,
            scx,
            initial_lcdc,
            second_lcdc,
        ))
        .expect("probe ROM should load");

    let mut irq = None;

    for _ in 0..1_200_000 {
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
            return Intr2Mode0SpritesProbeObservation {
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
        "mode0-sprites multi probe with scx did not halt; delay_nops={delay_nops} sprite_count={} scx={scx:#04X} pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        sprite_xs.len(),
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn run_intr_2_mode0_sprites_multi_restart_helper_probe(
    delay_nops: usize,
    sprite_xs: &[u8],
) -> Intr2Mode0SpritesProbeObservation {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_intr_2_mode0_sprites_multi_restart_helper_probe_rom(
            delay_nops, sprite_xs,
        ))
        .expect("probe ROM should load");

    let mut irq = None;

    for _ in 0..2_000_000 {
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
            return Intr2Mode0SpritesProbeObservation {
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
        "mode0-sprites restart-helper probe did not halt; delay_nops={delay_nops} sprite_count={} pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        sprite_xs.len(),
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn run_intr_2_mode0_sprites_multi_live_obj_enable_probe(
    delay_nops: usize,
    sprite_xs: &[u8],
) -> Intr2Mode0SpritesProbeObservation {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_intr_2_mode0_sprites_multi_live_obj_enable_probe_rom(
            delay_nops, sprite_xs,
        ))
        .expect("probe ROM should load");

    let mut irq = None;

    for _ in 0..2_000_000 {
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
            return Intr2Mode0SpritesProbeObservation {
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
        "mode0-sprites live-obj-enable probe did not halt; delay_nops={delay_nops} sprite_count={} pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        sprite_xs.len(),
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

#[allow(dead_code)]
fn sample_intr_2_mode0_sprites_multi_probe_line_changes_with_scx(
    delay_nops: usize,
    sprite_xs: &[u8],
    scx: u8,
    target_ly: u8,
) -> Vec<Intr2Mode0TimingSpritesLine68Observation> {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_intr_2_mode0_sprites_multi_probe_rom_with_scx(
            delay_nops, sprite_xs, scx,
        ))
        .expect("probe ROM should load");

    let mut saw_irq = false;
    let mut observations = Vec::new();
    let mut previous = None;

    for _ in 0..1_200_000 {
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

        if saw_irq {
            let ppu = machine.ppu().snapshot();
            if ppu.ly == target_ly {
                let current = Intr2Mode0TimingSpritesLine68Observation {
                    line_dot: ppu.line_dot,
                    mode: ppu.mode,
                    mode0_start_dot: ppu.mode0_start_dot,
                    current_transfer_x: ppu.bg_current_transfer_x,
                    visible_pixels_output: ppu.visible_pixels_output,
                    current_transfer_lane: ppu.bg_current_transfer_lane,
                    current_transfer_source_window: ppu.bg_current_transfer_source_window,
                    current_transfer_backing: ppu.bg_current_transfer_backing,
                    current_transfer_readiness: ppu.bg_current_transfer_readiness,
                    bg_fifo_len: ppu.bg_fifo_pixels.len(),
                    startup_fifo_placeholders: ppu.bg_startup_fifo_placeholders,
                    obj_fetcher_stage: ppu.obj_fetcher_stage,
                    obj_fetcher_stage_dot: ppu.obj_fetcher_stage_dot,
                    obj_pending_hit_match_x: ppu.obj_pending_hit_match_x,
                    obj_pending_hit_len: ppu.obj_pending_hit_len,
                    obj_pending_hit_front_sprite_slot: ppu.obj_pending_hit_front_sprite_slot,
                    bg_fetcher_stage: ppu.bg_fetcher_stage,
                    bg_fetcher_stage_dot: ppu.bg_fetcher_stage_dot,
                    selected_sprites_len: ppu.selected_sprites.len(),
                };
                if previous.as_ref() != Some(&current) {
                    observations.push(current.clone());
                    previous = Some(current);
                }
            } else if !observations.is_empty() && ppu.ly > target_ly {
                return observations;
            }
        }

        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d != 0
        {
            return observations;
        }
    }

    panic!(
        "mode0-sprites multi probe with scx line sample did not terminate; delay_nops={delay_nops} sprite_count={} scx={scx:#04X} target_ly={} pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        sprite_xs.len(),
        target_ly,
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn sample_intr_2_mode0_sprites_multi_probe_line_changes_pre_irq_with_scx(
    delay_nops: usize,
    sprite_xs: &[u8],
    scx: u8,
    target_ly: u8,
) -> Vec<Intr2Mode0TimingSpritesLine68Observation> {
    sample_intr_2_mode0_sprites_multi_probe_line_changes_pre_irq_with_setup(
        delay_nops, sprite_xs, scx, 0x91, 0x93, target_ly,
    )
}

fn sample_intr_2_mode0_sprites_multi_probe_line_changes_pre_irq_with_setup(
    delay_nops: usize,
    sprite_xs: &[u8],
    scx: u8,
    initial_lcdc: u8,
    second_lcdc: u8,
    target_ly: u8,
) -> Vec<Intr2Mode0TimingSpritesLine68Observation> {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_intr_2_mode0_sprites_multi_probe_rom_with_setup(
            delay_nops,
            sprite_xs,
            scx,
            initial_lcdc,
            second_lcdc,
        ))
        .expect("probe ROM should load");

    let mut observations = Vec::new();
    let mut previous = None;

    for _ in 0..1_200_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        if ppu.ly == target_ly {
            let current = Intr2Mode0TimingSpritesLine68Observation {
                line_dot: ppu.line_dot,
                mode: ppu.mode,
                mode0_start_dot: ppu.mode0_start_dot,
                current_transfer_x: ppu.bg_current_transfer_x,
                visible_pixels_output: ppu.visible_pixels_output,
                current_transfer_lane: ppu.bg_current_transfer_lane,
                current_transfer_source_window: ppu.bg_current_transfer_source_window,
                current_transfer_backing: ppu.bg_current_transfer_backing,
                current_transfer_readiness: ppu.bg_current_transfer_readiness,
                bg_fifo_len: ppu.bg_fifo_pixels.len(),
                startup_fifo_placeholders: ppu.bg_startup_fifo_placeholders,
                obj_fetcher_stage: ppu.obj_fetcher_stage,
                obj_fetcher_stage_dot: ppu.obj_fetcher_stage_dot,
                obj_pending_hit_match_x: ppu.obj_pending_hit_match_x,
                obj_pending_hit_len: ppu.obj_pending_hit_len,
                obj_pending_hit_front_sprite_slot: ppu.obj_pending_hit_front_sprite_slot,
                bg_fetcher_stage: ppu.bg_fetcher_stage,
                bg_fetcher_stage_dot: ppu.bg_fetcher_stage_dot,
                selected_sprites_len: ppu.selected_sprites.len(),
            };
            if previous.as_ref() != Some(&current) {
                observations.push(current.clone());
                previous = Some(current);
            }
        } else if !observations.is_empty() && ppu.ly > target_ly {
            return observations;
        }
    }

    panic!(
        "mode0-sprites multi probe pre-irq line sample did not terminate; delay_nops={delay_nops} sprite_count={} scx={scx:#04X} target_ly={} pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        sprite_xs.len(),
        target_ly,
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn build_intr_2_mode0_sprites_real_setup_probe_rom(delay_nops: usize, sprite_x: u8) -> Vec<u8> {
    build_intr_2_mode0_sprites_real_setup_probe_rom_with_scx(delay_nops, sprite_x, 0)
}

fn build_intr_2_mode0_sprites_real_setup_probe_rom_with_scx(
    delay_nops: usize,
    sprite_x: u8,
    scx: u8,
) -> Vec<u8> {
    let mut program = Vec::new();

    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000

    program.extend_from_slice(&[0x3E, 0x13]); // ld a,$13 ; LCD off, OBJ enabled like the real testcase
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a
    program.extend_from_slice(&[0x3E, scx]); // ld a,scx
    program.extend_from_slice(&[0xE0, 0x43]); // ldh ($43),a

    for byte_offset in 0..0x28_u16 {
        program.push(0xAF); // xor a
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(0xFE00_u16 + byte_offset).to_le_bytes());
    }

    for sprite_index in 0..10_u8 {
        let entry_address = 0xFE00_u16 + sprite_index as u16 * 4;
        let tile_index = 0x30_u8 + sprite_index;
        program.extend_from_slice(&[0x3E, 0x52]); // ld a,$52 ; sprite y for LY=66
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&entry_address.to_le_bytes());
        program.extend_from_slice(&[0x3E, sprite_x]); // ld a,sprite_x
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 1).to_le_bytes());
        program.extend_from_slice(&[0x3E, tile_index]); // ld a,tile_index
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 2).to_le_bytes());
        program.push(0xAF); // xor a
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 3).to_le_bytes());
    }

    program.extend_from_slice(&[0x3E, 0x93]); // ld a,$93 ; enable LCD with OBJ already on
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a
    program.extend(std::iter::repeat_n(0x00, 6)); // 24 dots
    program.extend_from_slice(&[0x3E, 0x93]); // ld a,$93 ; rewrite same value like the real testcase
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a

    program.extend_from_slice(&[0x21, 0x00, 0xC0]); // ld hl,$C000
    program.extend_from_slice(&[0x0E, delay_nops as u8]); // ld c,delay_nops
    program.push(0xAF); // xor a
    let stub_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x22); // ld (hli),a
    program.push(0x0D); // dec c
    emit_jr_nz(&mut program, stub_loop_pc); // jr nz,stub_loop
    program.extend_from_slice(&[0x3E, 0xC9]); // ld a,$C9
    program.push(0x22); // ld (hli),a

    program.extend_from_slice(&[0x3E, 0x02]); // ld a,$02
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh ($FF),a ; IE = STAT

    program.extend_from_slice(&[0x21, 0x00, 0x00]); // ld hl,compare_addr
    let compare_addr_operand = program.len() - 2;
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0x21, 0x00, 0xC0]); // ld hl,$C000
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0xC3, 0x00, 0x00]); // jp wait_irq
    let wait_irq_operand = program.len() - 2;

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

    program.extend_from_slice(&[0x3E, 0x20]); // ld a,$20 ; mode0 STAT enable
    program.extend_from_slice(&[0xE0, 0x41]); // ldh ($41),a
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a
    program.push(0xFB); // ei
    program.push(0x76); // halt
    program.push(0x00); // nop
    let fail_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, fail_loop_pc); // jr .

    let compare_addr = 0x0100_u16 + program.len() as u16;
    program.push(0x06); // ld b,$00
    program.push(0x00);
    let mode0_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x04); // inc b
    program.extend_from_slice(&[0xF0, 0x41]); // ldh a,($41)
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    emit_jr_nz(&mut program, mode0_loop_pc); // jr nz,mode0_loop

    program.push(0x50); // ld d,b
    program.push(0x76); // halt
    let done_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, done_loop_pc); // jr .

    patch_abs16(&mut program, compare_addr_operand, compare_addr);
    patch_abs16(&mut program, wait_irq_operand, wait_ly_loop_pc);

    let mut rom = build_test_rom(&program, 0x00);
    rom[0x0048] = 0xE8; // add sp,+2
    rom[0x0049] = 0x02;
    rom[0x004A] = 0xC9; // ret
    rom
}

#[allow(dead_code)]
fn build_intr_2_mode0_sprites_case13_real_probe_rom(delay_nops: usize) -> Vec<u8> {
    build_intr_2_mode0_sprites_real_setup_probe_rom(delay_nops, 4)
}

fn build_intr_2_mode0_sprites_two_round_real_setup_probe_rom(
    sprite_x: u8,
    delay_a_nops: usize,
    delay_b_nops: usize,
) -> Vec<u8> {
    build_intr_2_mode0_sprites_multi_two_round_real_setup_probe_rom(
        &std::iter::repeat_n(sprite_x, 10).collect::<Vec<_>>(),
        delay_a_nops,
        delay_b_nops,
    )
}

fn build_intr_2_mode0_sprites_multi_two_round_real_setup_probe_rom(
    sprite_xs: &[u8],
    delay_a_nops: usize,
    delay_b_nops: usize,
) -> Vec<u8> {
    let mut program = Vec::new();

    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000

    program.extend_from_slice(&[0x3E, 0x13]); // ld a,$13 ; LCD off, OBJ bit preserved
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a

    for byte_offset in 0..0x28_u16 {
        program.push(0xAF); // xor a
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(0xFE00_u16 + byte_offset).to_le_bytes());
    }

    for (sprite_index, sprite_x) in sprite_xs.iter().copied().enumerate() {
        let entry_address = 0xFE00_u16 + sprite_index as u16 * 4;
        let tile_index = 0x30_u8 + sprite_index as u8;
        program.extend_from_slice(&[0x3E, 0x52]); // ld a,$52 ; sprite y for LY=66
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&entry_address.to_le_bytes());
        program.extend_from_slice(&[0x3E, sprite_x]); // ld a,sprite_x
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 1).to_le_bytes());
        program.extend_from_slice(&[0x3E, tile_index]); // ld a,tile_index
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 2).to_le_bytes());
        program.push(0xAF); // xor a
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 3).to_le_bytes());
    }

    for &(base, delay_nops) in &[(0xC000_u16, delay_a_nops), (0xC080_u16, delay_b_nops)] {
        program.extend_from_slice(&[0x21, (base & 0xFF) as u8, (base >> 8) as u8]); // ld hl,base
        program.extend_from_slice(&[0x0E, delay_nops as u8]); // ld c,delay_nops
        program.push(0xAF); // xor a
        let stub_loop_pc = 0x0100_u16 + program.len() as u16;
        program.push(0x22); // ld (hli),a
        program.push(0x0D); // dec c
        emit_jr_nz(&mut program, stub_loop_pc); // jr nz,stub_loop
        program.extend_from_slice(&[0x3E, 0xC9]); // ld a,$C9
        program.push(0x22); // ld (hli),a
    }

    program.extend_from_slice(&[0x3E, 0x02]); // ld a,$02
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh ($FF),a ; IE = STAT

    program.extend_from_slice(&[0x3E, 0x93]); // ld a,$93 ; enable LCD with OBJ preserved
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a
    program.extend(std::iter::repeat_n(0x00, 6)); // observed gap between the two live writes
    program.extend_from_slice(&[0x3E, 0x93]); // ld a,$93 ; live LCDC.1 write
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a

    program.extend_from_slice(&[0x21, 0x00, 0x00]); // ld hl,round_a_ret
    let round_a_ret_operand = program.len() - 2;
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0x21, 0x00, 0xC0]); // ld hl,$C000
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0xC3, 0x00, 0x00]); // jp setup_and_wait_mode2
    let setup_and_wait_operand_a = program.len() - 2;

    let round_a_ret = 0x0100_u16 + program.len() as u16;
    program.push(0x06); // ld b,$00
    program.push(0x00);
    let round_a_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x04); // inc b
    program.extend_from_slice(&[0xF0, 0x41]); // ldh a,($41)
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    emit_jr_nz(&mut program, round_a_loop_pc); // jr nz,loop
    program.push(0x78); // ld a,b
    program.extend_from_slice(&[0xEA, 0x00, 0xC1]); // ld ($C100),a

    program.extend_from_slice(&[0x21, 0x00, 0x00]); // ld hl,round_b_ret
    let round_b_ret_operand = program.len() - 2;
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0x21, 0x80, 0xC0]); // ld hl,$C080
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0xC3, 0x00, 0x00]); // jp setup_and_wait_mode2
    let setup_and_wait_operand_b = program.len() - 2;

    let round_b_ret = 0x0100_u16 + program.len() as u16;
    program.push(0x06); // ld b,$00
    program.push(0x00);
    let round_b_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x04); // inc b
    program.extend_from_slice(&[0xF0, 0x41]); // ldh a,($41)
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    emit_jr_nz(&mut program, round_b_loop_pc); // jr nz,loop
    program.push(0x78); // ld a,b
    program.extend_from_slice(&[0xEA, 0x01, 0xC1]); // ld ($C101),a

    program.extend_from_slice(&[0xFA, 0x00, 0xC1]); // ld a,($C100)
    program.push(0x47); // ld b,a
    program.extend_from_slice(&[0xFA, 0x01, 0xC1]); // ld a,($C101)
    program.push(0x4F); // ld c,a
    program.extend_from_slice(&[0x16, 0x01]); // ld d,$01
    program.push(0x76); // halt
    let done_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, done_loop_pc); // jr .

    let setup_and_wait_mode2 = 0x0100_u16 + program.len() as u16;
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

    program.extend_from_slice(&[0x3E, 0x20]); // ld a,$20 ; mode0 STAT enable
    program.extend_from_slice(&[0xE0, 0x41]); // ldh ($41),a
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a
    program.push(0xFB); // ei
    program.push(0x76); // halt
    program.push(0x00); // nop
    let fail_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, fail_loop_pc); // jr .

    patch_abs16(&mut program, round_a_ret_operand, round_a_ret);
    patch_abs16(&mut program, round_b_ret_operand, round_b_ret);
    patch_abs16(&mut program, setup_and_wait_operand_a, setup_and_wait_mode2);
    patch_abs16(&mut program, setup_and_wait_operand_b, setup_and_wait_mode2);

    let mut rom = build_test_rom(&program, 0x00);
    rom[0x0048] = 0xE8; // add sp,+2
    rom[0x0049] = 0x02;
    rom[0x004A] = 0xC9; // ret
    rom
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

fn build_intr_2_mode0_sprites_case29_rom_path_probe_rom() -> Vec<u8> {
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

    rom[0x0B5A..0x0C33].copy_from_slice(&source_rom[0x0B5A..0x0C33]);
    rom[0x47F0..0x4901].copy_from_slice(&source_rom[0x47F0..0x4901]);

    let mut program = Vec::new();
    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000
    program.push(0xF3); // di
    program.extend_from_slice(&[0x3E, 0x1D]); // ld a,$1D ; testcase 29 marker
    program.extend_from_slice(&[0xEA, 0x80, 0xFF]); // ld ($FF80),a
    program.extend_from_slice(&[0x21, 0x20, 0x0C]); // ld hl,$0C20 ; case29 sprite spec
    program.extend_from_slice(&[0x16, 0x39]); // ld d,$39 ; round A delay
    program.extend_from_slice(&[0x1E, 0x38]); // ld e,$38 ; round B delay
    program.extend_from_slice(&[0xCD, 0x5A, 0x0B]); // call $0B5A
    program.extend_from_slice(&[0x3E, 0x01]); // ld a,$01 ; success marker if helper returns
    program.extend_from_slice(&[0xEA, 0x00, 0xC2]); // ld ($C200),a
    program.extend_from_slice(&[0x16, 0x01]); // ld d,$01
    program.push(0x76); // halt
    program.extend_from_slice(&[0x18, 0xFE]); // jr .
    rom[0x0100..0x0100 + program.len()].copy_from_slice(&program);

    rom[0x0C20] = 0x0A; // 10 sprites
    rom[0x0C21..0x0C2B].fill(0xA0); // x=160

    rom
}

fn build_intr_2_mode0_timing_sprites_case28_case29_rom_path_probe_rom() -> Vec<u8> {
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

    rom[0x0B5A..0x0C33].copy_from_slice(&source_rom[0x0B5A..0x0C33]);
    rom[0x47F0..0x4901].copy_from_slice(&source_rom[0x47F0..0x4901]);

    let mut program = Vec::new();
    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000
    program.push(0xF3); // di

    program.extend_from_slice(&[0x3E, 0x1C]); // ld a,$1C ; testcase 28 marker
    program.extend_from_slice(&[0xEA, 0x80, 0xFF]); // ld ($FF80),a
    program.extend_from_slice(&[0x21, 0x20, 0x0C]); // ld hl,$0C20 ; case28 sprite spec
    program.extend_from_slice(&[0x16, 0x39]); // ld d,$39
    program.extend_from_slice(&[0x1E, 0x38]); // ld e,$38
    program.extend_from_slice(&[0xCD, 0x5A, 0x0B]); // call $0B5A
    program.push(0xAF); // xor a
    program.push(0x57); // ld d,a

    program.extend_from_slice(&[0x3E, 0x1D]); // ld a,$1D ; testcase 29 marker
    program.extend_from_slice(&[0xEA, 0x80, 0xFF]); // ld ($FF80),a
    program.extend_from_slice(&[0x21, 0x2B, 0x0C]); // ld hl,$0C2B ; case29 sprite spec
    program.extend_from_slice(&[0x16, 0x39]); // ld d,$39
    program.extend_from_slice(&[0x1E, 0x38]); // ld e,$38
    program.extend_from_slice(&[0xCD, 0x5A, 0x0B]); // call $0B5A

    program.extend_from_slice(&[0x3E, 0x01]); // ld a,$01 ; success marker if both helpers return
    program.extend_from_slice(&[0xEA, 0x01, 0xC2]); // ld ($C201),a
    program.extend_from_slice(&[0x16, 0x01]); // ld d,$01
    program.push(0x76); // halt
    program.extend_from_slice(&[0x18, 0xFE]); // jr .
    rom[0x0100..0x0100 + program.len()].copy_from_slice(&program);

    rom[0x0C20] = 0x0A; // case28: 10 sprites
    rom[0x0C21..0x0C2B].fill(0x21); // x=33
    rom[0x0C2B] = 0x0A; // case29: 10 sprites
    rom[0x0C2C..0x0C36].fill(0xA0); // x=160

    rom
}

fn build_intr_2_mode0_timing_sprites_case29_real_caller_probe_rom() -> Vec<u8> {
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

    rom[0x0464..0x0491].copy_from_slice(&source_rom[0x0464..0x0491]);
    rom[0x0B5A..0x0C33].copy_from_slice(&source_rom[0x0B5A..0x0C33]);
    rom[0x47F0..0x4901].copy_from_slice(&source_rom[0x47F0..0x4901]);

    let mut boot = Vec::new();
    boot.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000
    boot.push(0xF3); // di
    boot.extend_from_slice(&[0xC3, 0x64, 0x04]); // jp $0464
    rom[0x0100..0x0100 + boot.len()].copy_from_slice(&boot);

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

#[allow(dead_code)]
#[allow(clippy::type_complexity)]
fn sample_intr_2_mode0_sprites_case1_rom_path_probe_arm_and_reads(
    max_reads: usize,
) -> (
    Option<Intr2Mode0TimingSpritesIrqObservation>,
    Vec<Intr2Mode0TimingSpritesStatReadObservation>,
    Option<(u16, u8, u8, u16, PpuAccessMode)>,
) {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_intr_2_mode0_sprites_case1_rom_path_probe_rom())
        .expect("probe ROM should load");
    seed_intr_2_mode0_sprites_case1_rom_path_probe_trampolines(&mut machine);

    let mut armed = None;
    let mut irq = None;
    let mut reads = Vec::new();
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..6_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        if armed.is_none()
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            let ppu = machine.ppu().snapshot();
            armed = Some((ppu.ly, ppu.line_dot, ppu.mode));
        }

        if irq.is_none()
            && let Some((armed_ly, armed_line_dot, armed_mode)) = armed
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            let ppu = machine.ppu().snapshot();
            irq = Some(Intr2Mode0TimingSpritesIrqObservation {
                armed_ly,
                armed_line_dot,
                armed_mode,
                irq_ly: ppu.ly,
                irq_line_dot: ppu.line_dot,
                irq_mode: ppu.mode,
                irq_pc: machine.cpu().registers().pc,
            });
        }

        if let Some(irq_observation) = irq {
            let max_post_arm_t_cycles = 20_000;
            for _ in 0..max_post_arm_t_cycles {
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
                        return (Some(irq_observation), reads, None);
                    }
                }

                if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted {
                    let pc = machine.cpu().registers().pc;
                    if matches!(pc, 0x010F | 0x0C09) {
                        let ppu = machine.ppu().snapshot();
                        return (
                            Some(irq_observation),
                            reads,
                            Some((
                                pc,
                                cpu_snapshot.registers.b,
                                cpu_snapshot.registers.c,
                                ppu.ly as u16,
                                ppu.mode,
                            )),
                        );
                    }
                }

                previous_ppu = machine.ppu().snapshot();
            }
            return (Some(irq_observation), reads, None);
        }

        previous_ppu = machine.ppu().snapshot();
    }

    panic!("copied case1 ROM-path probe did not finish sampling");
}

#[allow(dead_code)]
fn sample_intr_2_mode0_sprites_case1_rom_path_probe_stat_arm() -> (u8, u16, PpuAccessMode, u16) {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_intr_2_mode0_sprites_case1_rom_path_probe_rom())
        .expect("probe ROM should load");
    seed_intr_2_mode0_sprites_case1_rom_path_probe_trampolines(&mut machine);

    for _ in 0..2_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        if let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            let ppu = machine.ppu().snapshot();
            return (ppu.ly, ppu.line_dot, ppu.mode, cpu_snapshot.registers.pc);
        }
    }

    panic!("copied case1 ROM-path probe did not arm STAT");
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

fn run_intr_2_mode0_sprites_two_round_real_setup_probe_with_delays(
    sprite_x: u8,
    delay_a_nops: usize,
    delay_b_nops: usize,
) -> Intr2Mode0SpritesTwoRoundObservation {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_intr_2_mode0_sprites_two_round_real_setup_probe_rom(
            sprite_x,
            delay_a_nops,
            delay_b_nops,
        ))
        .expect("probe ROM should load");

    for _ in 0..2_000_000 {
        machine.step_t_cycle();

        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d != 0
        {
            let snapshot = machine.ppu().snapshot();
            return Intr2Mode0SpritesTwoRoundObservation {
                round_a_count: machine.cpu().registers().b,
                round_b_count: machine.cpu().registers().c,
                halt_ly: snapshot.ly,
                halt_line_dot: snapshot.line_dot,
                halt_mode: snapshot.mode,
            };
        }
    }

    panic!(
        "mode0-sprites two-round real setup probe did not halt; sprite_x={sprite_x} pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn run_intr_2_mode0_sprites_two_round_real_setup_probe(
    sprite_x: u8,
) -> Intr2Mode0SpritesTwoRoundObservation {
    run_intr_2_mode0_sprites_two_round_real_setup_probe_with_delays(sprite_x, 56, 55)
}

fn run_intr_2_mode0_sprites_multi_two_round_real_setup_probe(
    sprite_xs: &[u8],
    delay_a_nops: usize,
    delay_b_nops: usize,
) -> Intr2Mode0SpritesTwoRoundObservation {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(
            build_intr_2_mode0_sprites_multi_two_round_real_setup_probe_rom(
                sprite_xs,
                delay_a_nops,
                delay_b_nops,
            ),
        )
        .expect("probe ROM should load");

    for _ in 0..2_000_000 {
        machine.step_t_cycle();

        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d != 0
        {
            let snapshot = machine.ppu().snapshot();
            return Intr2Mode0SpritesTwoRoundObservation {
                round_a_count: machine.cpu().registers().b,
                round_b_count: machine.cpu().registers().c,
                halt_ly: snapshot.ly,
                halt_line_dot: snapshot.line_dot,
                halt_mode: snapshot.mode,
            };
        }
    }

    panic!(
        "mode0-sprites multi two-round real setup probe did not halt; sprite_count={} pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        sprite_xs.len(),
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn build_intr_2_mode0_sprites_two_round_source_setup_probe_rom(
    sprite_x: u8,
    delay_a_nops: usize,
    delay_b_nops: usize,
) -> Vec<u8> {
    let mut program = Vec::new();

    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000
    program.extend_from_slice(&[0x3E, 0x11]); // ld a,$11 ; LCD off, OBJ off like disable_ppu_safe from 0x91
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a

    for byte_offset in 0..0x28_u16 {
        program.push(0xAF); // xor a
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(0xFE00_u16 + byte_offset).to_le_bytes());
    }

    for sprite_index in 0..10_u8 {
        let entry_address = 0xFE00_u16 + sprite_index as u16 * 4;
        let tile_index = 0x30_u8 + sprite_index;
        program.extend_from_slice(&[0x3E, 0x52]); // ld a,$52 ; sprite y for LY=66
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&entry_address.to_le_bytes());
        program.extend_from_slice(&[0x3E, sprite_x]); // ld a,sprite_x
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 1).to_le_bytes());
        program.extend_from_slice(&[0x3E, tile_index]); // ld a,tile_index
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 2).to_le_bytes());
        program.push(0xAF); // xor a
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 3).to_le_bytes());
    }

    for &(base, delay_nops) in &[(0xC000_u16, delay_a_nops), (0xC080_u16, delay_b_nops)] {
        program.extend_from_slice(&[0x21, (base & 0xFF) as u8, (base >> 8) as u8]); // ld hl,base
        program.extend_from_slice(&[0x0E, delay_nops as u8]); // ld c,delay_nops
        program.push(0xAF); // xor a
        let stub_loop_pc = 0x0100_u16 + program.len() as u16;
        program.push(0x22); // ld (hli),a
        program.push(0x0D); // dec c
        emit_jr_nz(&mut program, stub_loop_pc); // jr nz,stub_loop
        program.extend_from_slice(&[0x3E, 0xC9]); // ld a,$C9
        program.push(0x22); // ld (hli),a
    }

    program.extend_from_slice(&[0x3E, 0x02]); // ld a,$02
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh ($FF),a ; IE = STAT

    program.extend_from_slice(&[0x21, 0x40, 0xFF]); // ld hl,$FF40 ; LCDC
    program.extend_from_slice(&[0xCB, 0xFE]); // set 7,(hl) ; enable_ppu
    program.extend_from_slice(&[0xCB, 0xCE]); // set 1,(hl) ; enable sprites live

    program.extend_from_slice(&[0x21, 0x00, 0x00]); // ld hl,round_a_ret
    let round_a_ret_operand = program.len() - 2;
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0x21, 0x00, 0xC0]); // ld hl,$C000
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0xC3, 0x00, 0x00]); // jp setup_and_wait_mode2
    let setup_and_wait_operand_a = program.len() - 2;

    let round_a_ret = 0x0100_u16 + program.len() as u16;
    program.push(0x06); // ld b,$00
    program.push(0x00);
    let round_a_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x04); // inc b
    program.extend_from_slice(&[0xF0, 0x41]); // ldh a,($41)
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    emit_jr_nz(&mut program, round_a_loop_pc); // jr nz,loop
    program.push(0x78); // ld a,b
    program.extend_from_slice(&[0xEA, 0x00, 0xC1]); // ld ($C100),a

    program.extend_from_slice(&[0x21, 0x00, 0x00]); // ld hl,round_b_ret
    let round_b_ret_operand = program.len() - 2;
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0x21, 0x80, 0xC0]); // ld hl,$C080
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0xC3, 0x00, 0x00]); // jp setup_and_wait_mode2
    let setup_and_wait_operand_b = program.len() - 2;

    let round_b_ret = 0x0100_u16 + program.len() as u16;
    program.push(0x06); // ld b,$00
    program.push(0x00);
    let round_b_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x04); // inc b
    program.extend_from_slice(&[0xF0, 0x41]); // ldh a,($41)
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    emit_jr_nz(&mut program, round_b_loop_pc); // jr nz,loop
    program.push(0x78); // ld a,b
    program.extend_from_slice(&[0xEA, 0x01, 0xC1]); // ld ($C101),a

    program.extend_from_slice(&[0xFA, 0x00, 0xC1]); // ld a,($C100)
    program.push(0x47); // ld b,a
    program.extend_from_slice(&[0xFA, 0x01, 0xC1]); // ld a,($C101)
    program.push(0x4F); // ld c,a
    program.extend_from_slice(&[0x16, 0x01]); // ld d,$01
    program.push(0x76); // halt
    let done_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, done_loop_pc); // jr .

    let setup_and_wait_mode2 = 0x0100_u16 + program.len() as u16;
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

    program.extend_from_slice(&[0x3E, 0x20]); // ld a,$20 ; mode0 STAT enable
    program.extend_from_slice(&[0xE0, 0x41]); // ldh ($41),a
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a
    program.push(0xFB); // ei
    program.push(0x76); // halt
    program.push(0x00); // nop
    let fail_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, fail_loop_pc); // jr .

    patch_abs16(&mut program, round_a_ret_operand, round_a_ret);
    patch_abs16(&mut program, round_b_ret_operand, round_b_ret);
    patch_abs16(&mut program, setup_and_wait_operand_a, setup_and_wait_mode2);
    patch_abs16(&mut program, setup_and_wait_operand_b, setup_and_wait_mode2);

    let mut rom = build_test_rom(&program, 0x00);
    rom[0x0048] = 0xE8; // add sp,+2
    rom[0x0049] = 0x02;
    rom[0x004A] = 0xC9; // ret
    rom
}

fn build_intr_2_mode0_sprites_two_round_timed_direct_setup_probe_rom(
    sprite_x: u8,
    delay_a_nops: usize,
    delay_b_nops: usize,
) -> Vec<u8> {
    let mut program = Vec::new();

    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000
    program.extend_from_slice(&[0x3E, 0x11]); // ld a,$11 ; LCD off, OBJ off like disable_ppu_safe from 0x91
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a

    for byte_offset in 0..0x28_u16 {
        program.push(0xAF); // xor a
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(0xFE00_u16 + byte_offset).to_le_bytes());
    }

    for sprite_index in 0..10_u8 {
        let entry_address = 0xFE00_u16 + sprite_index as u16 * 4;
        let tile_index = 0x30_u8 + sprite_index;
        program.extend_from_slice(&[0x3E, 0x52]); // ld a,$52 ; sprite y for LY=66
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&entry_address.to_le_bytes());
        program.extend_from_slice(&[0x3E, sprite_x]); // ld a,sprite_x
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 1).to_le_bytes());
        program.extend_from_slice(&[0x3E, tile_index]); // ld a,tile_index
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 2).to_le_bytes());
        program.push(0xAF); // xor a
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 3).to_le_bytes());
    }

    for &(base, delay_nops) in &[(0xC000_u16, delay_a_nops), (0xC080_u16, delay_b_nops)] {
        program.extend_from_slice(&[0x21, (base & 0xFF) as u8, (base >> 8) as u8]); // ld hl,base
        program.extend_from_slice(&[0x0E, delay_nops as u8]); // ld c,delay_nops
        program.push(0xAF); // xor a
        let stub_loop_pc = 0x0100_u16 + program.len() as u16;
        program.push(0x22); // ld (hli),a
        program.push(0x0D); // dec c
        emit_jr_nz(&mut program, stub_loop_pc); // jr nz,stub_loop
        program.extend_from_slice(&[0x3E, 0xC9]); // ld a,$C9
        program.push(0x22); // ld (hli),a
    }

    program.extend_from_slice(&[0x3E, 0x02]); // ld a,$02
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh ($FF),a ; IE = STAT

    program.extend_from_slice(&[0x21, 0x40, 0xFF]); // ld hl,$FF40 ; LCDC
    program.extend_from_slice(&[0x3E, 0x91]); // ld a,$91 ; enable LCD without OBJ
    program.push(0x77); // ld (hl),a ; same first-write timing as SET 7,(hl)
    program.extend_from_slice(&[0x3E, 0x93]); // ld a,$93 ; enable sprites live
    program.push(0x77); // ld (hl),a ; same second-write timing as SET 1,(hl)

    program.extend_from_slice(&[0x21, 0x00, 0x00]); // ld hl,round_a_ret
    let round_a_ret_operand = program.len() - 2;
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0x21, 0x00, 0xC0]); // ld hl,$C000
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0xC3, 0x00, 0x00]); // jp setup_and_wait_mode2
    let setup_and_wait_operand_a = program.len() - 2;

    let round_a_ret = 0x0100_u16 + program.len() as u16;
    program.push(0x06); // ld b,$00
    program.push(0x00);
    let round_a_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x04); // inc b
    program.extend_from_slice(&[0xF0, 0x41]); // ldh a,($41)
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    emit_jr_nz(&mut program, round_a_loop_pc); // jr nz,loop
    program.push(0x78); // ld a,b
    program.extend_from_slice(&[0xEA, 0x00, 0xC1]); // ld ($C100),a

    program.extend_from_slice(&[0x21, 0x00, 0x00]); // ld hl,round_b_ret
    let round_b_ret_operand = program.len() - 2;
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0x21, 0x80, 0xC0]); // ld hl,$C080
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0xC3, 0x00, 0x00]); // jp setup_and_wait_mode2
    let setup_and_wait_operand_b = program.len() - 2;

    let round_b_ret = 0x0100_u16 + program.len() as u16;
    program.push(0x06); // ld b,$00
    program.push(0x00);
    let round_b_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x04); // inc b
    program.extend_from_slice(&[0xF0, 0x41]); // ldh a,($41)
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    emit_jr_nz(&mut program, round_b_loop_pc); // jr nz,loop
    program.push(0x78); // ld a,b
    program.extend_from_slice(&[0xEA, 0x01, 0xC1]); // ld ($C101),a

    program.extend_from_slice(&[0xFA, 0x00, 0xC1]); // ld a,($C100)
    program.push(0x47); // ld b,a
    program.extend_from_slice(&[0xFA, 0x01, 0xC1]); // ld a,($C101)
    program.push(0x4F); // ld c,a
    program.extend_from_slice(&[0x16, 0x01]); // ld d,$01
    program.push(0x76); // halt
    let done_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, done_loop_pc); // jr .

    let setup_and_wait_mode2 = 0x0100_u16 + program.len() as u16;
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

    program.extend_from_slice(&[0x3E, 0x20]); // ld a,$20 ; mode0 STAT enable
    program.extend_from_slice(&[0xE0, 0x41]); // ldh ($41),a
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a
    program.push(0xFB); // ei
    program.push(0x76); // halt
    program.push(0x00); // nop
    let fail_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, fail_loop_pc); // jr .

    patch_abs16(&mut program, round_a_ret_operand, round_a_ret);
    patch_abs16(&mut program, round_b_ret_operand, round_b_ret);
    patch_abs16(&mut program, setup_and_wait_operand_a, setup_and_wait_mode2);
    patch_abs16(&mut program, setup_and_wait_operand_b, setup_and_wait_mode2);

    let mut rom = build_test_rom(&program, 0x00);
    rom[0x0048] = 0xE8; // add sp,+2
    rom[0x0049] = 0x02;
    rom[0x004A] = 0xC9; // ret
    rom
}

fn build_intr_2_mode0_sprites_two_round_live_obj_enable_probe_rom(
    sprite_x: u8,
    delay_a_nops: usize,
    delay_b_nops: usize,
) -> Vec<u8> {
    let mut program = Vec::new();

    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000
    program.extend_from_slice(&[0x3E, 0x13]); // ld a,$13 ; LCD off, OBJ preset like the ROM
    program.extend_from_slice(&[0xE0, 0x40]); // ldh ($40),a

    for byte_offset in 0..0x28_u16 {
        program.push(0xAF); // xor a
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(0xFE00_u16 + byte_offset).to_le_bytes());
    }

    for sprite_index in 0..10_u8 {
        let entry_address = 0xFE00_u16 + sprite_index as u16 * 4;
        let tile_index = 0x30_u8 + sprite_index;
        program.extend_from_slice(&[0x3E, 0x52]); // ld a,$52 ; sprite y for LY=66
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&entry_address.to_le_bytes());
        program.extend_from_slice(&[0x3E, sprite_x]); // ld a,sprite_x
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 1).to_le_bytes());
        program.extend_from_slice(&[0x3E, tile_index]); // ld a,tile_index
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 2).to_le_bytes());
        program.push(0xAF); // xor a
        program.push(0xEA); // ld (a16),a
        program.extend_from_slice(&(entry_address + 3).to_le_bytes());
    }

    for &(base, delay_nops) in &[(0xC000_u16, delay_a_nops), (0xC080_u16, delay_b_nops)] {
        program.extend_from_slice(&[0x21, (base & 0xFF) as u8, (base >> 8) as u8]); // ld hl,base
        program.extend_from_slice(&[0x0E, delay_nops as u8]); // ld c,delay_nops
        program.push(0xAF); // xor a
        let stub_loop_pc = 0x0100_u16 + program.len() as u16;
        program.push(0x22); // ld (hli),a
        program.push(0x0D); // dec c
        emit_jr_nz(&mut program, stub_loop_pc); // jr nz,stub_loop
        program.extend_from_slice(&[0x3E, 0xC9]); // ld a,$C9
        program.push(0x22); // ld (hli),a
    }

    program.extend_from_slice(&[0x3E, 0x02]); // ld a,$02
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh ($FF),a ; IE = STAT

    program.extend_from_slice(&[0x21, 0x40, 0xFF]); // ld hl,$FF40
    program.extend_from_slice(&[0xCB, 0xFE]); // set 7,(hl)
    program.extend_from_slice(&[0x21, 0x40, 0xFF]); // ld hl,$FF40
    program.extend_from_slice(&[0xCB, 0xCE]); // set 1,(hl)

    program.extend_from_slice(&[0x21, 0x00, 0x00]); // ld hl,round_a_ret
    let round_a_ret_operand = program.len() - 2;
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0x21, 0x00, 0xC0]); // ld hl,$C000
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0xC3, 0x00, 0x00]); // jp setup_and_wait_mode2
    let setup_and_wait_operand_a = program.len() - 2;

    let round_a_ret = 0x0100_u16 + program.len() as u16;
    program.push(0x06); // ld b,$00
    program.push(0x00);
    let round_a_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x04); // inc b
    program.extend_from_slice(&[0xF0, 0x41]); // ldh a,($41)
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    emit_jr_nz(&mut program, round_a_loop_pc); // jr nz,loop
    program.push(0x78); // ld a,b
    program.extend_from_slice(&[0xEA, 0x00, 0xC1]); // ld ($C100),a

    program.extend_from_slice(&[0x21, 0x00, 0x00]); // ld hl,round_b_ret
    let round_b_ret_operand = program.len() - 2;
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0x21, 0x80, 0xC0]); // ld hl,$C080
    program.push(0xE5); // push hl
    program.extend_from_slice(&[0xC3, 0x00, 0x00]); // jp setup_and_wait_mode2
    let setup_and_wait_operand_b = program.len() - 2;

    let round_b_ret = 0x0100_u16 + program.len() as u16;
    program.push(0x06); // ld b,$00
    program.push(0x00);
    let round_b_loop_pc = 0x0100_u16 + program.len() as u16;
    program.push(0x04); // inc b
    program.extend_from_slice(&[0xF0, 0x41]); // ldh a,($41)
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    emit_jr_nz(&mut program, round_b_loop_pc); // jr nz,loop
    program.push(0x78); // ld a,b
    program.extend_from_slice(&[0xEA, 0x01, 0xC1]); // ld ($C101),a

    program.extend_from_slice(&[0xFA, 0x00, 0xC1]); // ld a,($C100)
    program.push(0x47); // ld b,a
    program.extend_from_slice(&[0xFA, 0x01, 0xC1]); // ld a,($C101)
    program.push(0x4F); // ld c,a
    program.extend_from_slice(&[0x16, 0x01]); // ld d,$01
    program.push(0x76); // halt
    let done_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, done_loop_pc); // jr .

    let setup_and_wait_mode2 = 0x0100_u16 + program.len() as u16;
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

    program.extend_from_slice(&[0x3E, 0x20]); // ld a,$20 ; mode0 STAT enable
    program.extend_from_slice(&[0xE0, 0x41]); // ldh ($41),a
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh ($0F),a
    program.push(0xFB); // ei
    program.push(0x76); // halt
    program.push(0x00); // nop
    let fail_loop_pc = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, fail_loop_pc); // jr .

    patch_abs16(&mut program, round_a_ret_operand, round_a_ret);
    patch_abs16(&mut program, round_b_ret_operand, round_b_ret);
    patch_abs16(&mut program, setup_and_wait_operand_a, setup_and_wait_mode2);
    patch_abs16(&mut program, setup_and_wait_operand_b, setup_and_wait_mode2);

    let mut rom = build_test_rom(&program, 0x00);
    rom[0x0048] = 0xE8; // add sp,+2
    rom[0x0049] = 0x02;
    rom[0x004A] = 0xC9; // ret
    rom
}

fn run_intr_2_mode0_sprites_two_round_source_setup_probe_with_delays(
    sprite_x: u8,
    delay_a_nops: usize,
    delay_b_nops: usize,
) -> Intr2Mode0SpritesTwoRoundObservation {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_intr_2_mode0_sprites_two_round_source_setup_probe_rom(
            sprite_x,
            delay_a_nops,
            delay_b_nops,
        ))
        .expect("probe ROM should load");

    for _ in 0..2_000_000 {
        machine.step_t_cycle();

        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d != 0
        {
            let snapshot = machine.ppu().snapshot();
            return Intr2Mode0SpritesTwoRoundObservation {
                round_a_count: machine.cpu().registers().b,
                round_b_count: machine.cpu().registers().c,
                halt_ly: snapshot.ly,
                halt_line_dot: snapshot.line_dot,
                halt_mode: snapshot.mode,
            };
        }
    }

    panic!(
        "mode0-sprites two-round source setup probe did not halt; sprite_x={sprite_x} pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

#[allow(dead_code)]
fn run_intr_2_mode0_sprites_two_round_source_setup_probe(
    sprite_x: u8,
) -> Intr2Mode0SpritesTwoRoundObservation {
    run_intr_2_mode0_sprites_two_round_source_setup_probe_with_delays(sprite_x, 56, 55)
}

fn run_intr_2_mode0_sprites_two_round_timed_direct_setup_probe_with_delays(
    sprite_x: u8,
    delay_a_nops: usize,
    delay_b_nops: usize,
) -> Intr2Mode0SpritesTwoRoundObservation {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(
            build_intr_2_mode0_sprites_two_round_timed_direct_setup_probe_rom(
                sprite_x,
                delay_a_nops,
                delay_b_nops,
            ),
        )
        .expect("probe ROM should load");

    for _ in 0..2_000_000 {
        machine.step_t_cycle();

        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d != 0
        {
            let snapshot = machine.ppu().snapshot();
            return Intr2Mode0SpritesTwoRoundObservation {
                round_a_count: machine.cpu().registers().b,
                round_b_count: machine.cpu().registers().c,
                halt_ly: snapshot.ly,
                halt_line_dot: snapshot.line_dot,
                halt_mode: snapshot.mode,
            };
        }
    }

    panic!(
        "mode0-sprites two-round timed direct setup probe did not halt; sprite_x={sprite_x} pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

#[allow(dead_code)]
fn run_intr_2_mode0_sprites_two_round_timed_direct_setup_probe(
    sprite_x: u8,
) -> Intr2Mode0SpritesTwoRoundObservation {
    run_intr_2_mode0_sprites_two_round_timed_direct_setup_probe_with_delays(sprite_x, 56, 55)
}

fn run_intr_2_mode0_sprites_two_round_live_obj_enable_probe_with_delays(
    sprite_x: u8,
    delay_a_nops: usize,
    delay_b_nops: usize,
) -> Intr2Mode0SpritesTwoRoundObservation {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(
            build_intr_2_mode0_sprites_two_round_live_obj_enable_probe_rom(
                sprite_x,
                delay_a_nops,
                delay_b_nops,
            ),
        )
        .expect("probe ROM should load");

    for _ in 0..2_000_000 {
        machine.step_t_cycle();

        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d != 0
        {
            let snapshot = machine.ppu().snapshot();
            return Intr2Mode0SpritesTwoRoundObservation {
                round_a_count: machine.cpu().registers().b,
                round_b_count: machine.cpu().registers().c,
                halt_ly: snapshot.ly,
                halt_line_dot: snapshot.line_dot,
                halt_mode: snapshot.mode,
            };
        }
    }

    panic!(
        "mode0-sprites two-round live-obj-enable probe did not halt; sprite_x={sprite_x} pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn sample_intr_2_mode0_sprites_stat_reads_after_irq_from_rom(
    rom: Vec<u8>,
    max_reads: usize,
) -> Vec<Intr2Mode0TimingSpritesStatReadObservation> {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut reads = Vec::new();
    let mut saw_irq = false;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..2_000_000 {
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

        if saw_irq
            && let Some(activity) = machine.cpu().snapshot().last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            let ppu = machine.ppu().snapshot();
            reads.push(Intr2Mode0TimingSpritesStatReadObservation {
                pc: machine.cpu().registers().pc,
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
                return reads;
            }
        }

        previous_ppu = machine.ppu().snapshot();

        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d != 0
        {
            return reads;
        }
    }

    panic!(
        "intr_2_mode0_sprites STAT read sample did not terminate; pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn sample_intr_2_mode0_sprites_line_changes_from_rom(
    rom: Vec<u8>,
    target_ly: u8,
) -> Vec<Intr2Mode0TimingSpritesLine68Observation> {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut saw_irq = false;
    let mut observations = Vec::new();
    let mut previous = None;

    for _ in 0..2_000_000 {
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

        if saw_irq {
            let ppu = machine.ppu().snapshot();
            if ppu.ly == target_ly {
                let current = Intr2Mode0TimingSpritesLine68Observation {
                    line_dot: ppu.line_dot,
                    mode: ppu.mode,
                    mode0_start_dot: ppu.mode0_start_dot,
                    current_transfer_x: ppu.bg_current_transfer_x,
                    visible_pixels_output: ppu.visible_pixels_output,
                    current_transfer_lane: ppu.bg_current_transfer_lane,
                    current_transfer_source_window: ppu.bg_current_transfer_source_window,
                    current_transfer_backing: ppu.bg_current_transfer_backing,
                    current_transfer_readiness: ppu.bg_current_transfer_readiness,
                    bg_fifo_len: ppu.bg_fifo_pixels.len(),
                    startup_fifo_placeholders: ppu.bg_startup_fifo_placeholders,
                    obj_fetcher_stage: ppu.obj_fetcher_stage,
                    obj_fetcher_stage_dot: ppu.obj_fetcher_stage_dot,
                    obj_pending_hit_match_x: ppu.obj_pending_hit_match_x,
                    obj_pending_hit_len: ppu.obj_pending_hit_len,
                    obj_pending_hit_front_sprite_slot: ppu.obj_pending_hit_front_sprite_slot,
                    bg_fetcher_stage: ppu.bg_fetcher_stage,
                    bg_fetcher_stage_dot: ppu.bg_fetcher_stage_dot,
                    selected_sprites_len: ppu.selected_sprites.len(),
                };
                if previous.as_ref() != Some(&current) {
                    observations.push(current.clone());
                    previous = Some(current);
                }
            } else if !observations.is_empty() && ppu.ly > target_ly {
                return observations;
            }
        }

        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d != 0
        {
            return observations;
        }
    }

    panic!(
        "intr_2_mode0_sprites line sample did not terminate; target_ly={target_ly} pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn sample_intr_2_mode0_sprites_irq_after_stat_arm_from_rom(
    rom: Vec<u8>,
) -> Option<Intr2Mode0TimingSpritesIrqObservation> {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut armed = None;

    for _ in 0..2_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        if armed.is_none()
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            let ppu = machine.ppu().snapshot();
            armed = Some((ppu.ly, ppu.line_dot, ppu.mode));
        }

        if let Some((armed_ly, armed_line_dot, armed_mode)) = armed
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            let ppu = machine.ppu().snapshot();
            return Some(Intr2Mode0TimingSpritesIrqObservation {
                armed_ly,
                armed_line_dot,
                armed_mode,
                irq_ly: ppu.ly,
                irq_line_dot: ppu.line_dot,
                irq_mode: ppu.mode,
                irq_pc: machine.cpu().registers().pc,
            });
        }

        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d != 0
        {
            return None;
        }
    }

    panic!(
        "intr_2_mode0_sprites IRQ sample did not terminate; pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn run_intr_2_mode0_sprites_real_setup_probe(
    delay_nops: usize,
    sprite_x: u8,
) -> Intr2Mode0SpritesProbeObservation {
    run_intr_2_mode0_sprites_real_setup_probe_with_scx(delay_nops, sprite_x, 0)
}

fn run_intr_2_mode0_sprites_real_setup_probe_with_scx(
    delay_nops: usize,
    sprite_x: u8,
    scx: u8,
) -> Intr2Mode0SpritesProbeObservation {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_intr_2_mode0_sprites_real_setup_probe_rom_with_scx(
            delay_nops, sprite_x, scx,
        ))
        .expect("probe ROM should load");

    let mut irq = None;

    for _ in 0..1_200_000 {
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
            return Intr2Mode0SpritesProbeObservation {
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
        "mode0-sprites case13 real probe did not halt; delay_nops={delay_nops} pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn run_intr_2_mode0_sprites_case13_real_probe(
    delay_nops: usize,
) -> Intr2Mode0SpritesProbeObservation {
    run_intr_2_mode0_sprites_real_setup_probe(delay_nops, 4)
}

fn sample_intr_2_mode0_sprites_case13_real_probe_line_changes_pre_irq(
    delay_nops: usize,
    target_ly: u8,
) -> Vec<Intr2Mode0TimingSpritesLine68Observation> {
    sample_intr_2_mode0_sprites_real_setup_probe_line_changes_pre_irq(delay_nops, 4, target_ly)
}

fn sample_intr_2_mode0_sprites_real_setup_probe_line_changes_pre_irq(
    delay_nops: usize,
    sprite_x: u8,
    target_ly: u8,
) -> Vec<Intr2Mode0TimingSpritesLine68Observation> {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_intr_2_mode0_sprites_real_setup_probe_rom(
            delay_nops, sprite_x,
        ))
        .expect("probe ROM should load");

    let mut observations = Vec::new();
    let mut previous = None;

    for _ in 0..1_200_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        if ppu.ly == target_ly {
            let current = Intr2Mode0TimingSpritesLine68Observation {
                line_dot: ppu.line_dot,
                mode: ppu.mode,
                mode0_start_dot: ppu.mode0_start_dot,
                current_transfer_x: ppu.bg_current_transfer_x,
                visible_pixels_output: ppu.visible_pixels_output,
                current_transfer_lane: ppu.bg_current_transfer_lane,
                current_transfer_source_window: ppu.bg_current_transfer_source_window,
                current_transfer_backing: ppu.bg_current_transfer_backing,
                current_transfer_readiness: ppu.bg_current_transfer_readiness,
                bg_fifo_len: ppu.bg_fifo_pixels.len(),
                startup_fifo_placeholders: ppu.bg_startup_fifo_placeholders,
                obj_fetcher_stage: ppu.obj_fetcher_stage,
                obj_fetcher_stage_dot: ppu.obj_fetcher_stage_dot,
                obj_pending_hit_match_x: ppu.obj_pending_hit_match_x,
                obj_pending_hit_len: ppu.obj_pending_hit_len,
                obj_pending_hit_front_sprite_slot: ppu.obj_pending_hit_front_sprite_slot,
                bg_fetcher_stage: ppu.bg_fetcher_stage,
                bg_fetcher_stage_dot: ppu.bg_fetcher_stage_dot,
                selected_sprites_len: ppu.selected_sprites.len(),
            };
            if previous.as_ref() != Some(&current) {
                observations.push(current.clone());
                previous = Some(current);
            }
        } else if !observations.is_empty() && ppu.ly > target_ly {
            return observations;
        }
    }

    panic!(
        "mode0-sprites case13 real probe pre-irq line sample did not terminate; delay_nops={delay_nops} target_ly={} pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        target_ly,
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
}

fn sample_intr_2_mode0_sprites_single_read_probe(
    delay_nops: usize,
    sprite_xs: &[u8],
) -> Option<Intr2Mode0SpritesSingleReadObservation> {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_intr_2_mode0_sprites_single_read_probe_rom(
            delay_nops, sprite_xs,
        ))
        .expect("probe ROM should load");

    let mut saw_irq = false;
    let mut irq = None;

    for _ in 0..1_200_000 {
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
            let snapshot = machine.ppu().snapshot();
            irq = Some((snapshot.ly, snapshot.line_dot, snapshot.mode));
        }

        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d != 0
        {
            let ppu = machine.ppu().snapshot();
            let (irq_ly, irq_line_dot, irq_mode) = irq.unwrap_or((ppu.ly, ppu.line_dot, ppu.mode));
            return Some(Intr2Mode0SpritesSingleReadObservation {
                value: machine.cpu().registers().d,
                irq_ly,
                irq_line_dot,
                irq_mode,
                halt_ly: ppu.ly,
                halt_line_dot: ppu.line_dot,
                halt_mode: ppu.mode,
            });
        }
    }

    None
}

fn run_intr_2_mode0_sprites_lcd_restart_probe(
    delay_nops: usize,
    sprite_x: u8,
) -> Intr2Mode0SpritesProbeObservation {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_intr_2_mode0_sprites_lcd_restart_probe_rom(
            delay_nops, sprite_x,
        ))
        .expect("probe ROM should load");

    let mut irq = None;

    for _ in 0..1_200_000 {
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
            return Intr2Mode0SpritesProbeObservation {
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
        "mode0-sprites lcd-restart probe did not halt; delay_nops={delay_nops} sprite_x={sprite_x:#04X} pc={:#06X} state={:?} ly={} line_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.read_bus(0xFF41)
    );
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

#[test]
fn hblank_ly_scx_probe_matches_mooneye_thresholds() {
    let mut failures = Vec::new();

    for scx in 0x00..=0x08 {
        let (delay_a, delay_b) = match scx & 0x07 {
            0 => (2, 3),
            1..=4 => (1, 2),
            _ => (0, 1),
        };

        for scanline in [0x42, 0x43] {
            let before = run_hblank_ly_scx_probe(scx, scanline, delay_a);
            let after = run_hblank_ly_scx_probe(scx, scanline, delay_b);
            if before.completion_marker != 1 {
                failures.push(format!(
                    "probe halted early for scx={scx:#04X} scanline={scanline:#04X}; before={before:?}"
                ));
            }
            if after.completion_marker != 1 {
                failures.push(format!(
                    "probe halted early for scx={scx:#04X} scanline={scanline:#04X}; after={after:?}"
                ));
            }
            if before.observed_ly != scanline.wrapping_sub(1) {
                failures.push(format!(
                    "delay_a expected previous LY for scx={scx:#04X} scanline={scanline:#04X}; before={before:?} after={after:?}"
                ));
            }
            if after.observed_ly != scanline {
                failures.push(format!(
                    "delay_b expected current LY for scx={scx:#04X} scanline={scanline:#04X}; before={before:?} after={after:?}"
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "hblank_ly_scx mismatches:\n{}",
        failures.join("\n")
    );
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
fn lcd_reenable_lyc_rise_services_lcd_stat_before_the_second_di_in_the_mooneye_round4_sequence() {
    assert!(observe_lcd_reenable_lyc_irq_service_window(0x00, None));
}

#[test]
fn lcd_reenable_retained_true_does_not_service_lcd_stat_in_the_same_sequence() {
    assert!(!observe_lcd_reenable_lyc_irq_service_window(
        0x90,
        Some(0x00)
    ));
}

#[test]
fn mode2_to_mode0_stat_interrupt_probe_matches_mooneye_counts() {
    let delay4 = run_intr_2_0_probe(4);
    let delay3 = run_intr_2_0_probe(3);

    assert_eq!(
        (delay4.count, delay3.count),
        (0x07, 0x08),
        "delay4={delay4:?} delay3={delay3:?}"
    );
}

#[test]
#[ignore = "diagnostic probe for mooneye intr_2_oam_ok_timing seam"]
fn mode2_to_oam_release_probe_matches_mooneye_counts() {
    let delay46 = run_intr_2_oam_ok_probe(46);
    let delay45 = run_intr_2_oam_ok_probe(45);

    assert_eq!(
        (delay46.count, delay45.count),
        (0x01, 0x02),
        "delay46={delay46:?} delay45={delay45:?}"
    );
}

#[test]
#[ignore = "diagnostic probe for first FE00 reads after intr_2_oam_ok_timing wake"]
fn mode2_to_oam_release_probe_logs_first_reads() {
    let delay46 = sample_intr_2_oam_ok_reads(46, 3);
    let delay45 = sample_intr_2_oam_ok_reads(45, 3);
    println!("delay46={delay46:?}");
    println!("delay45={delay45:?}");
}

#[test]
#[ignore = "diagnostic probe for FE00 reads in the real mooneye intr_2_oam_ok_timing ROM"]
fn real_mooneye_oam_ok_logs_first_reads() {
    let reads = sample_real_mooneye_oam_ok_reads(4);
    println!("reads={reads:?}");
}

#[test]
#[ignore = "diagnostic probe for the real mooneye stat_lyc_onoff ROM"]
fn real_mooneye_stat_lyc_onoff_logs_first_accesses() {
    let accesses = sample_real_mooneye_stat_lyc_onoff_accesses(48);
    println!("accesses={accesses:?}");
}

#[test]
#[ignore = "diagnostic probe for the real mooneye intr_2_mode0_timing_sprites ROM"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_failure_case() {
    let failure = sample_real_mooneye_intr_2_mode0_timing_sprites_failure();
    println!("failure={failure:?}");
}

#[test]
#[ignore = "diagnostic probe for testcase 0 of mooneye intr_2_mode0_timing_sprites"]
fn mode2_to_mode0_sprites_probe_case0_logs_counts() {
    let x0_delay43 = run_intr_2_mode0_sprites_probe(43, 0x00);
    let x0_delay42 = run_intr_2_mode0_sprites_probe(42, 0x00);
    let x1_delay43 = run_intr_2_mode0_sprites_probe(43, 0x01);
    let x1_delay42 = run_intr_2_mode0_sprites_probe(42, 0x01);
    println!("x0_delay43={x0_delay43:?}");
    println!("x0_delay42={x0_delay42:?}");
    println!("x1_delay43={x1_delay43:?}");
    println!("x1_delay42={x1_delay42:?}");
}

#[test]
#[ignore = "diagnostic lcd-restart probe for testcase 0 of mooneye intr_2_mode0_timing_sprites"]
fn mode2_to_mode0_sprites_lcd_restart_probe_case0_logs_counts() {
    let x0_delay43 = run_intr_2_mode0_sprites_lcd_restart_probe(43, 0x00);
    let x0_delay42 = run_intr_2_mode0_sprites_lcd_restart_probe(42, 0x00);
    println!("restart_x0_delay43={x0_delay43:?}");
    println!("restart_x0_delay42={x0_delay42:?}");
}

#[test]
#[ignore = "diagnostic probe that stops at pc=0x4870 for mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_pc_4870_case() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let pc = machine.cpu().registers().pc;
        if pc == 0x4870 {
            let registers = machine.cpu().registers();
            let snapshot = machine.ppu().snapshot();
            println!(
                "pc4870 testcase_index={} b={} c={} d={} e={} ly={} line_dot={} mode={:?} mode0_start_dot={} selected_sprites_len={} visible_pixels_output={}",
                machine.read_bus(0xFF80),
                registers.b,
                registers.c,
                registers.d,
                registers.e,
                snapshot.ly,
                snapshot.line_dot,
                snapshot.mode,
                snapshot.mode0_start_dot,
                snapshot.selected_sprites.len(),
                snapshot.visible_pixels_output
            );
            return;
        }
    }

    panic!("probe did not reach pc=0x4870");
}

#[test]
#[ignore = "diagnostic probe that stops at the current failure signature in pc=0x4870 for mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_failure_signature_at_pc_4870() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let registers = machine.cpu().registers();
        if registers.pc == 0x4870
            && registers.b == 12
            && registers.c == 40
            && registers.d == 66
            && registers.e == 240
        {
            let snapshot = machine.ppu().snapshot();
            println!(
                "pc4870_failure testcase_index={} ly={} line_dot={} mode={:?} mode0_start_dot={} selected_sprites_len={} visible_pixels_output={}",
                machine.read_bus(0xFF80),
                snapshot.ly,
                snapshot.line_dot,
                snapshot.mode,
                snapshot.mode0_start_dot,
                snapshot.selected_sprites.len(),
                snapshot.visible_pixels_output
            );
            return;
        }
    }

    panic!("probe did not reach the current pc=0x4870 failure signature");
}

#[test]
#[ignore = "diagnostic probe that stops at the current full-ROM failure signature for mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_current_full_rom_failure_signature() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let registers = machine.cpu().registers();
        let snapshot = machine.ppu().snapshot();
        if registers.pc == 0x486E {
            println!(
                "current_failure testcase_index={} b={} c={} d={} e={} ly={} line_dot={} mode={:?} mode0_start_dot={} selected_sprites_len={} visible_pixels_output={}",
                machine.read_bus(0xFF80),
                registers.b,
                registers.c,
                registers.d,
                registers.e,
                snapshot.ly,
                snapshot.line_dot,
                snapshot.mode,
                snapshot.mode0_start_dot,
                snapshot.selected_sprites.len(),
                snapshot.visible_pixels_output
            );
            return;
        }
    }

    panic!("probe did not reach the current full-ROM failure signature");
}

#[test]
#[ignore = "diagnostic x-range table for lcd-restart probe of mooneye intr_2_mode0_timing_sprites"]
fn mode2_to_mode0_sprites_lcd_restart_probe_logs_x_table() {
    for sprite_x in 0_u8..=17 {
        let delay43 = run_intr_2_mode0_sprites_lcd_restart_probe(43, sprite_x);
        let delay42 = run_intr_2_mode0_sprites_lcd_restart_probe(42, sprite_x);
        println!("x={sprite_x} delay43={delay43:?} delay42={delay42:?}");
    }
}

#[test]
#[ignore = "diagnostic multi-sprite probe matching early same-x cases of mooneye intr_2_mode0_timing_sprites"]
fn mode2_to_mode0_sprites_multi_probe_logs_same_x_ladder() {
    for sprite_count in 1..=4 {
        let sprite_xs = vec![0; sprite_count];
        let expected_cycles = match sprite_count {
            1 => 2,
            2 => 4,
            3 => 5,
            4 => 7,
            _ => unreachable!(),
        };
        let round_a = run_intr_2_mode0_sprites_multi_probe(41 + expected_cycles, &sprite_xs);
        let round_b = run_intr_2_mode0_sprites_multi_probe(40 + expected_cycles, &sprite_xs);
        println!("same_x_count={sprite_count} round_a={round_a:?} round_b={round_b:?}");
    }
}

#[test]
#[ignore = "diagnostic mini-probe for offscreen-right same-x cases of mooneye intr_2_mode0_timing_sprites"]
fn mode2_to_mode0_sprites_multi_probe_logs_offscreen_right_same_x_cases() {
    for &(sprite_x, expected_cycles) in &[(167_u8, 15_u8), (168_u8, 0_u8), (169_u8, 0_u8)] {
        let sprite_xs = [sprite_x; 10];
        let round_a =
            run_intr_2_mode0_sprites_multi_probe(41 + expected_cycles as usize, &sprite_xs);
        let round_b =
            run_intr_2_mode0_sprites_multi_probe(40 + expected_cycles as usize, &sprite_xs);
        println!("sprite_x={sprite_x} round_a={round_a:?} round_b={round_b:?}");
    }
}

#[test]
#[ignore = "diagnostic right-edge ladder for same-x mooneye intr_2_mode0_timing_sprites cases"]
fn mode2_to_mode0_sprites_multi_probe_logs_right_edge_same_x_ladder() {
    for sprite_x in 160_u8..=167 {
        let sprite_xs = [sprite_x; 10];
        let pair_57 = (
            run_intr_2_mode0_sprites_multi_probe(57, &sprite_xs),
            run_intr_2_mode0_sprites_multi_probe(56, &sprite_xs),
        );
        let pair_56 = (
            run_intr_2_mode0_sprites_multi_probe(56, &sprite_xs),
            run_intr_2_mode0_sprites_multi_probe(55, &sprite_xs),
        );
        let pair_55 = (
            run_intr_2_mode0_sprites_multi_probe(55, &sprite_xs),
            run_intr_2_mode0_sprites_multi_probe(54, &sprite_xs),
        );
        println!("sprite_x={sprite_x} pair57={pair_57:?} pair56={pair_56:?} pair55={pair_55:?}");
    }
}

#[test]
#[ignore = "diagnostic terminal same-x ladder before the right edge for mooneye intr_2_mode0_timing_sprites"]
fn mode2_to_mode0_sprites_multi_probe_logs_terminal_same_x_ladder() {
    for sprite_x in 152_u8..=160 {
        let sprite_xs = [sprite_x; 10];
        let pair_57 = (
            run_intr_2_mode0_sprites_multi_probe(57, &sprite_xs),
            run_intr_2_mode0_sprites_multi_probe(56, &sprite_xs),
        );
        let pair_56 = (
            run_intr_2_mode0_sprites_multi_probe(56, &sprite_xs),
            run_intr_2_mode0_sprites_multi_probe(55, &sprite_xs),
        );
        let pair_55 = (
            run_intr_2_mode0_sprites_multi_probe(55, &sprite_xs),
            run_intr_2_mode0_sprites_multi_probe(54, &sprite_xs),
        );
        println!(
            "terminal_sprite_x={sprite_x} pair57={pair_57:?} pair56={pair_56:?} pair55={pair_55:?}"
        );
    }
}

#[test]
#[ignore = "diagnostic STAT reads for offscreen-right same-x mini-probe cases of mooneye intr_2_mode0_timing_sprites"]
fn mode2_to_mode0_sprites_multi_probe_logs_offscreen_right_stat_reads() {
    for (label, delay_nops, sprite_xs) in [
        ("baseline_round_a", 41_usize, Vec::<u8>::new()),
        ("baseline_round_b", 40_usize, Vec::<u8>::new()),
        ("x167_round_a", 56_usize, vec![167_u8; 10]),
        ("x167_round_b", 55_usize, vec![167_u8; 10]),
        ("x168_round_a", 41_usize, vec![168_u8; 10]),
        ("x168_round_b", 40_usize, vec![168_u8; 10]),
    ] {
        let reads = sample_intr_2_mode0_sprites_multi_probe_stat_reads(delay_nops, &sprite_xs, 4);
        println!("{label} reads={reads:?}");
    }
}

#[test]
#[ignore = "diagnostic first-read experiment for x=168 offscreen-right same-x mini-probe"]
fn mode2_to_mode0_sprites_multi_probe_switches_to_hblank_on_the_first_round_a_read_for_x168() {
    let round_a_reads = sample_intr_2_mode0_sprites_multi_probe_stat_reads(41, &[168_u8; 10], 2);
    let round_b_reads = sample_intr_2_mode0_sprites_multi_probe_stat_reads(40, &[168_u8; 10], 2);

    assert_eq!(
        round_a_reads.first().map(|read| read.value & 0x03),
        Some(0x00),
        "round_a_reads={round_a_reads:?}"
    );
    assert_eq!(
        round_b_reads.first().map(|read| read.value & 0x03),
        Some(0x03),
        "round_b_reads={round_b_reads:?}"
    );
}

#[test]
#[ignore = "diagnostic same-x mini-probe for mooneye testcase 11 shape (10 sprites at X=2)"]
fn mode2_to_mode0_sprites_multi_probe_logs_mooneye_case11_shape() {
    let sprite_xs = [2_u8; 10];
    let round_a = run_intr_2_mode0_sprites_multi_probe(56, &sprite_xs);
    let round_b = run_intr_2_mode0_sprites_multi_probe(55, &sprite_xs);
    println!("case11_shape_round_a={round_a:?}");
    println!("case11_shape_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic STAT reads for the local case1 shape (2 sprites at X=0)"]
fn mode2_to_mode0_sprites_multi_probe_logs_case1_stat_reads() {
    let round_a_reads = sample_intr_2_mode0_sprites_multi_probe_stat_reads(45, &[0_u8, 0_u8], 4);
    let round_b_reads = sample_intr_2_mode0_sprites_multi_probe_stat_reads(44, &[0_u8, 0_u8], 4);
    println!("case1_round_a_reads={round_a_reads:?}");
    println!("case1_round_b_reads={round_b_reads:?}");
}

#[test]
#[ignore = "diagnostic ly66 pre-IRQ window for the local case1 shape (2 sprites at X=0)"]
fn mode2_to_mode0_sprites_multi_probe_logs_case1_line66_pre_irq_window() {
    let observations = sample_intr_2_mode0_sprites_multi_probe_line_changes_pre_irq_with_setup(
        45,
        &[0_u8, 0_u8],
        0,
        0x91,
        0x93,
        66,
    );
    for observation in observations {
        if (90..=170).contains(&observation.line_dot) {
            println!("case1_line66_pre_irq={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic IRQ timing after STAT arm for the local case1 shape (2 sprites at X=0)"]
fn mode2_to_mode0_sprites_multi_probe_logs_case1_irq_after_stat_arm() {
    for delay_nops in [45_usize, 44_usize] {
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );
        machine
            .load_cartridge(build_intr_2_mode0_sprites_multi_probe_rom(
                delay_nops,
                &[0_u8, 0_u8],
            ))
            .expect("probe ROM should load");

        let mut saw_irq = false;
        for _ in 0..1_200_000 {
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
                let ppu = machine.ppu().snapshot();
                println!(
                    "case1_delay{delay_nops}_irq ly={} line_dot={} mode={:?}",
                    ppu.ly, ppu.line_dot, ppu.mode
                );
                saw_irq = true;
                break;
            }
        }

        if !saw_irq {
            let ppu = machine.ppu().snapshot();
            println!(
                "case1_delay{delay_nops}_no_irq pc={:#06X} state={:?} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                machine.cpu().registers().pc,
                machine.cpu().execution_state(),
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
            );
        }
    }
}

#[test]
#[ignore = "diagnostic same-x mini-probe for mooneye testcase 12 shape (10 sprites at X=3)"]
fn mode2_to_mode0_sprites_multi_probe_logs_mooneye_case12_shape() {
    let sprite_xs = [3_u8; 10];
    let round_a = run_intr_2_mode0_sprites_multi_probe(56, &sprite_xs);
    let round_b = run_intr_2_mode0_sprites_multi_probe(55, &sprite_xs);
    println!("case12_shape_round_a={round_a:?}");
    println!("case12_shape_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic single-read probe for mooneye testcase 12 shape (10 sprites at X=3)"]
fn mode2_to_mode0_sprites_single_read_probe_logs_mooneye_case12_shape() {
    let round_a = sample_intr_2_mode0_sprites_single_read_probe(56, &[3_u8; 10]);
    let round_b = sample_intr_2_mode0_sprites_single_read_probe(55, &[3_u8; 10]);
    println!("case12_shape_single_read_round_a={round_a:?}");
    println!("case12_shape_single_read_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic same-x mini-probe for mooneye testcase 13 shape (10 sprites at X=4)"]
fn mode2_to_mode0_sprites_multi_probe_logs_mooneye_case13_shape() {
    let sprite_xs = [4_u8; 10];
    let round_a = run_intr_2_mode0_sprites_multi_probe(56, &sprite_xs);
    let round_b = run_intr_2_mode0_sprites_multi_probe(55, &sprite_xs);
    println!("case13_shape_round_a={round_a:?}");
    println!("case13_shape_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic same-x mini-probe for mooneye testcase 13 shape with scx=4"]
fn mode2_to_mode0_sprites_multi_probe_logs_mooneye_case13_shape_with_scx4() {
    let sprite_xs = [4_u8; 10];
    let round_a = run_intr_2_mode0_sprites_multi_probe_with_scx(56, &sprite_xs, 4);
    let round_b = run_intr_2_mode0_sprites_multi_probe_with_scx(55, &sprite_xs, 4);
    println!("case13_shape_scx4_round_a={round_a:?}");
    println!("case13_shape_scx4_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic ly66 window for mooneye testcase 13 shape with scx=4"]
fn mode2_to_mode0_sprites_multi_probe_logs_mooneye_case13_shape_line66_with_scx4() {
    let observations = sample_intr_2_mode0_sprites_multi_probe_line_changes_pre_irq_with_scx(
        56,
        &[4_u8; 10],
        4,
        66,
    );
    for observation in observations {
        if (110..=140).contains(&observation.line_dot) {
            println!("case13_shape_scx4_line66={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic same-x mini-probe for mooneye testcase 29 shape (10 sprites at X=160)"]
fn mode2_to_mode0_sprites_multi_probe_logs_mooneye_case29_shape() {
    let sprite_xs = [160_u8; 10];
    let round_a = run_intr_2_mode0_sprites_multi_probe(57, &sprite_xs);
    let round_b = run_intr_2_mode0_sprites_multi_probe(56, &sprite_xs);
    println!("case29_shape_round_a={round_a:?}");
    println!("case29_shape_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic ly66 window for mooneye testcase 29 shape"]
fn mode2_to_mode0_sprites_multi_probe_logs_mooneye_case29_shape_line66() {
    let observations = sample_intr_2_mode0_sprites_multi_probe_line_changes_pre_irq_with_scx(
        57,
        &[160_u8; 10],
        0,
        66,
    );
    for observation in observations {
        if (90..=320).contains(&observation.line_dot) {
            println!("case29_shape_line66={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic ly66 window for mooneye testcase 29 shape round-b"]
fn mode2_to_mode0_sprites_multi_probe_logs_mooneye_case29_shape_line66_round_b() {
    let observations = sample_intr_2_mode0_sprites_multi_probe_line_changes_pre_irq_with_scx(
        56,
        &[160_u8; 10],
        0,
        66,
    );
    for observation in observations {
        if (240..=320).contains(&observation.line_dot) {
            println!("case29_shape_round_b_line66={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic STAT reads for mooneye testcase 29 shape"]
fn mode2_to_mode0_sprites_multi_probe_logs_mooneye_case29_stat_reads() {
    let round_a_reads = sample_intr_2_mode0_sprites_multi_probe_stat_reads(57, &[160_u8; 10], 4);
    let round_b_reads = sample_intr_2_mode0_sprites_multi_probe_stat_reads(56, &[160_u8; 10], 4);
    println!("case29_shape_round_a_reads={round_a_reads:?}");
    println!("case29_shape_round_b_reads={round_b_reads:?}");
}

#[test]
#[ignore = "diagnostic same-x mini-probe for right-edge X=161 shape"]
fn mode2_to_mode0_sprites_multi_probe_logs_right_edge_x161_shape() {
    let sprite_xs = [161_u8; 10];
    let round_a = run_intr_2_mode0_sprites_multi_probe(56, &sprite_xs);
    let round_b = run_intr_2_mode0_sprites_multi_probe(55, &sprite_xs);
    println!("x161_shape_round_a={round_a:?}");
    println!("x161_shape_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic ly66 window for right-edge X=161 shape round-b"]
fn mode2_to_mode0_sprites_multi_probe_logs_right_edge_x161_shape_line66_round_b() {
    let observations = sample_intr_2_mode0_sprites_multi_probe_line_changes_pre_irq_with_scx(
        55,
        &[161_u8; 10],
        0,
        66,
    );
    for observation in observations {
        if (240..=320).contains(&observation.line_dot) {
            println!("x161_shape_round_b_line66={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic same-x mini-probe for mooneye testcase 13 real setup shape"]
fn mode2_to_mode0_sprites_multi_probe_logs_mooneye_case13_real_setup_shape() {
    let sprite_xs = [4_u8; 10];
    let round_a = run_intr_2_mode0_sprites_multi_probe_with_setup(56, &sprite_xs, 0, 0x93, 0x93);
    let round_b = run_intr_2_mode0_sprites_multi_probe_with_setup(55, &sprite_xs, 0, 0x93, 0x93);
    println!("case13_real_setup_round_a={round_a:?}");
    println!("case13_real_setup_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic same-x mini-probe for x=32 with real-style setup and scx=0"]
fn mode2_to_mode0_sprites_multi_probe_logs_x32_real_setup_shape() {
    let sprite_xs = [32_u8; 10];
    let round_a = run_intr_2_mode0_sprites_multi_probe_with_setup(57, &sprite_xs, 0, 0x13, 0x93);
    let round_b = run_intr_2_mode0_sprites_multi_probe_with_setup(56, &sprite_xs, 0, 0x13, 0x93);
    println!("x32_real_setup_round_a={round_a:?}");
    println!("x32_real_setup_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic same-x mini-probe for x=32 with real-style setup and scx=4"]
fn mode2_to_mode0_sprites_multi_probe_logs_x32_real_setup_shape_with_scx4() {
    let sprite_xs = [32_u8; 10];
    let round_a = run_intr_2_mode0_sprites_multi_probe_with_setup(57, &sprite_xs, 4, 0x13, 0x93);
    let round_b = run_intr_2_mode0_sprites_multi_probe_with_setup(56, &sprite_xs, 4, 0x13, 0x93);
    println!("x32_real_setup_scx4_round_a={round_a:?}");
    println!("x32_real_setup_scx4_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic same-x mini-probe for x=34 with real-style setup and scx=0"]
fn mode2_to_mode0_sprites_multi_probe_logs_x34_real_setup_shape() {
    let sprite_xs = [34_u8; 10];
    let round_a = run_intr_2_mode0_sprites_multi_probe_with_setup(57, &sprite_xs, 0, 0x13, 0x93);
    let round_b = run_intr_2_mode0_sprites_multi_probe_with_setup(56, &sprite_xs, 0, 0x13, 0x93);
    println!("x34_real_setup_round_a={round_a:?}");
    println!("x34_real_setup_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic same-x mini-probe for x=34 with real-style setup and scx=4"]
fn mode2_to_mode0_sprites_multi_probe_logs_x34_real_setup_shape_with_scx4() {
    let sprite_xs = [34_u8; 10];
    let round_a = run_intr_2_mode0_sprites_multi_probe_with_setup(57, &sprite_xs, 4, 0x13, 0x93);
    let round_b = run_intr_2_mode0_sprites_multi_probe_with_setup(56, &sprite_xs, 4, 0x13, 0x93);
    println!("x34_real_setup_scx4_round_a={round_a:?}");
    println!("x34_real_setup_scx4_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic split-group mini-probe for mooneye testcase 36 real setup shape"]
fn mode2_to_mode0_sprites_multi_probe_logs_mooneye_case36_real_setup_shape() {
    let sprite_xs = [0_u8, 0, 0, 0, 0, 160, 160, 160, 160, 160];
    let round_a = run_intr_2_mode0_sprites_multi_probe_with_setup(58, &sprite_xs, 0, 0x91, 0x93);
    let round_b = run_intr_2_mode0_sprites_multi_probe_with_setup(57, &sprite_xs, 0, 0x91, 0x93);
    println!("case36_real_setup_round_a={round_a:?}");
    println!("case36_real_setup_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic two-round split-group probe for mooneye testcase 36 real setup shape"]
fn mode2_to_mode0_sprites_multi_two_round_real_setup_probe_logs_case36_shape() {
    let observation = run_intr_2_mode0_sprites_multi_two_round_real_setup_probe(
        &[0_u8, 0, 0, 0, 0, 160, 160, 160, 160, 160],
        58,
        57,
    );
    println!("case36_two_round_real_setup={observation:?}");
}

#[test]
#[ignore = "diagnostic round-b STAT reads for the local two-round case36 real-setup probe"]
fn mode2_to_mode0_sprites_multi_two_round_real_setup_probe_logs_case36_round_b_reads() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(
            build_intr_2_mode0_sprites_multi_two_round_real_setup_probe_rom(
                &[0_u8, 0, 0, 0, 0, 160, 160, 160, 160, 160],
                58,
                57,
            ),
        )
        .expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..2_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        if let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
        }

        if arm_count == 2
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case36_local_round_b_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count == 2
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case36_local_round_b_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
            if read_count >= 2 {
                return;
            }
        }

        previous_ppu = machine.ppu().snapshot();
    }

    panic!("local case36 two-round probe did not reach round-b reads");
}

#[test]
#[ignore = "diagnostic ly68 window 280..320 for the local two-round case36 real-setup probe"]
fn mode2_to_mode0_sprites_multi_two_round_real_setup_probe_logs_case36_round_b_line68_window() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(
            build_intr_2_mode0_sprites_multi_two_round_real_setup_probe_rom(
                &[0_u8, 0, 0, 0, 0, 160, 160, 160, 160, 160],
                58,
                57,
            ),
        )
        .expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut previous = None;

    for _ in 0..2_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        if let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
        }

        if arm_count == 2 {
            let ppu = machine.ppu().snapshot();
            if ppu.ly == 68 && (280..=320).contains(&ppu.line_dot) {
                let current = Intr2Mode0TimingSpritesLine68Observation {
                    line_dot: ppu.line_dot,
                    mode: ppu.mode,
                    mode0_start_dot: ppu.mode0_start_dot,
                    current_transfer_x: ppu.bg_current_transfer_x,
                    visible_pixels_output: ppu.visible_pixels_output,
                    current_transfer_lane: ppu.bg_current_transfer_lane,
                    current_transfer_source_window: ppu.bg_current_transfer_source_window,
                    current_transfer_backing: ppu.bg_current_transfer_backing,
                    current_transfer_readiness: ppu.bg_current_transfer_readiness,
                    bg_fifo_len: ppu.bg_fifo_pixels.len(),
                    startup_fifo_placeholders: ppu.bg_startup_fifo_placeholders,
                    obj_fetcher_stage: ppu.obj_fetcher_stage,
                    obj_fetcher_stage_dot: ppu.obj_fetcher_stage_dot,
                    obj_pending_hit_match_x: ppu.obj_pending_hit_match_x,
                    obj_pending_hit_len: ppu.obj_pending_hit_len,
                    obj_pending_hit_front_sprite_slot: ppu.obj_pending_hit_front_sprite_slot,
                    bg_fetcher_stage: ppu.bg_fetcher_stage,
                    bg_fetcher_stage_dot: ppu.bg_fetcher_stage_dot,
                    selected_sprites_len: ppu.selected_sprites.len(),
                };
                if previous.as_ref() != Some(&current) {
                    println!("case36_local_round_b_line68={current:?}");
                    previous = Some(current);
                }
            } else if previous.is_some() && ppu.ly > 68 {
                return;
            }
        }
    }

    panic!("local case36 two-round probe did not reach line68 window");
}

#[test]
#[ignore = "diagnostic terminal ly68 window 307..315 for the local two-round case36 real-setup probe"]
fn mode2_to_mode0_sprites_multi_two_round_real_setup_probe_logs_case36_round_b_terminal_window() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(
            build_intr_2_mode0_sprites_multi_two_round_real_setup_probe_rom(
                &[0_u8, 0, 0, 0, 0, 160, 160, 160, 160, 160],
                58,
                57,
            ),
        )
        .expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut previous = None;

    for _ in 0..2_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        if let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
        }

        if arm_count == 2 {
            let ppu = machine.ppu().snapshot();
            if ppu.ly == 68 && (307..=315).contains(&ppu.line_dot) {
                let current = Intr2Mode0TimingSpritesTerminalWindowObservation {
                    line_dot: ppu.line_dot,
                    mode: ppu.mode,
                    mode0_start_dot: ppu.mode0_start_dot,
                    current_transfer_x: ppu.bg_current_transfer_x,
                    visible_pixels_output: ppu.visible_pixels_output,
                    current_transfer_lane: ppu.bg_current_transfer_lane,
                    current_transfer_source_window: ppu.bg_current_transfer_source_window,
                    current_transfer_backing: ppu.bg_current_transfer_backing,
                    current_transfer_readiness: ppu.bg_current_transfer_readiness,
                    current_transfer_kind: ppu.bg_current_transfer_kind,
                    bg_fifo_len: ppu.bg_fifo_pixels.len(),
                    startup_fifo_placeholders: ppu.bg_startup_fifo_placeholders,
                    bg_push_pending: ppu.bg_push_pending,
                    bg_push_disposition: ppu.bg_push_disposition,
                    bg_push_entry_delay_remaining: ppu.bg_push_entry_delay_remaining,
                    bg_fetcher_stage: ppu.bg_fetcher_stage,
                    bg_fetcher_stage_dot: ppu.bg_fetcher_stage_dot,
                    obj_fetcher_stage: ppu.obj_fetcher_stage,
                    obj_fetcher_stage_dot: ppu.obj_fetcher_stage_dot,
                    obj_pending_hit_len: ppu.obj_pending_hit_len,
                    obj_fetched_same_x_active_count: ppu.obj_fetched_same_x_active_count,
                    obj_fetched_same_x_pending_count: ppu.obj_fetched_same_x_pending_count,
                };
                if previous.as_ref() != Some(&current) {
                    println!("case36_local_round_b_terminal={current:?}");
                    previous = Some(current);
                }
            } else if previous.is_some() && ppu.ly > 68 {
                return;
            }
        }
    }

    panic!("local case36 two-round probe did not reach terminal window");
}

#[test]
#[ignore = "diagnostic STAT reads for mooneye testcase 36 real setup shape"]
fn mode2_to_mode0_sprites_multi_probe_logs_mooneye_case36_real_setup_stat_reads() {
    let sprite_xs = [0_u8, 0, 0, 0, 0, 160, 160, 160, 160, 160];
    let round_a_reads = sample_intr_2_mode0_sprites_multi_probe_stat_reads_with_setup(
        58, &sprite_xs, 4, 0x91, 0x93,
    );
    let round_b_reads = sample_intr_2_mode0_sprites_multi_probe_stat_reads_with_setup(
        57, &sprite_xs, 4, 0x91, 0x93,
    );
    println!("case36_real_setup_round_a_reads={round_a_reads:?}");
    println!("case36_real_setup_round_b_reads={round_b_reads:?}");
}

#[test]
#[ignore = "diagnostic ly68 window 240..320 for mooneye testcase 36 real setup shape round B"]
fn mode2_to_mode0_sprites_multi_probe_logs_mooneye_case36_real_setup_line68_round_b() {
    let sprite_xs = [0_u8, 0, 0, 0, 0, 160, 160, 160, 160, 160];
    let observations = sample_intr_2_mode0_sprites_multi_probe_line_changes_pre_irq_with_setup(
        57, &sprite_xs, 0, 0x91, 0x93, 68,
    );
    for observation in observations {
        if (240..=320).contains(&observation.line_dot) {
            println!("case36_real_setup_round_b_line68={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic ly66 window for mooneye testcase 13 real setup shape"]
fn mode2_to_mode0_sprites_multi_probe_logs_mooneye_case13_real_setup_line66() {
    let observations = sample_intr_2_mode0_sprites_multi_probe_line_changes_pre_irq_with_setup(
        56,
        &[4_u8; 10],
        0,
        0x93,
        0x93,
        66,
    );
    for observation in observations {
        if (90..=140).contains(&observation.line_dot) {
            println!("case13_real_setup_line66={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic exact case13 local probe built from the observed real setup"]
fn mode2_to_mode0_sprites_case13_real_probe_logs_shape() {
    let round_a = run_intr_2_mode0_sprites_case13_real_probe(56);
    let round_b = run_intr_2_mode0_sprites_case13_real_probe(55);
    println!("case13_exact_round_a={round_a:?}");
    println!("case13_exact_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic exact x=5 local probe built from the observed real setup pattern"]
fn mode2_to_mode0_sprites_x5_real_probe_logs_shape() {
    let round_a = run_intr_2_mode0_sprites_real_setup_probe(56, 5);
    let round_b = run_intr_2_mode0_sprites_real_setup_probe(55, 5);
    println!("x5_exact_round_a={round_a:?}");
    println!("x5_exact_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic exact x=6 local probe built from the observed real setup pattern"]
fn mode2_to_mode0_sprites_x6_real_probe_logs_shape() {
    let round_a = run_intr_2_mode0_sprites_real_setup_probe(56, 6);
    let round_b = run_intr_2_mode0_sprites_real_setup_probe(55, 6);
    println!("x6_exact_round_a={round_a:?}");
    println!("x6_exact_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic exact x=10 local probe built from the observed real setup pattern"]
fn mode2_to_mode0_sprites_x10_real_probe_logs_shape() {
    let round_a = run_intr_2_mode0_sprites_real_setup_probe(56, 10);
    let round_b = run_intr_2_mode0_sprites_real_setup_probe(55, 10);
    println!("x10_exact_round_a={round_a:?}");
    println!("x10_exact_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic exact x=15 local probe built from the observed real setup pattern"]
fn mode2_to_mode0_sprites_x15_real_probe_logs_shape() {
    let round_a = run_intr_2_mode0_sprites_real_setup_probe(56, 15);
    let round_b = run_intr_2_mode0_sprites_real_setup_probe(55, 15);
    println!("x15_exact_round_a={round_a:?}");
    println!("x15_exact_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic exact x=16 local probe built from the observed real setup pattern"]
fn mode2_to_mode0_sprites_x16_real_probe_logs_shape() {
    let round_a = run_intr_2_mode0_sprites_real_setup_probe(57, 16);
    let round_b = run_intr_2_mode0_sprites_real_setup_probe(56, 16);
    println!("x16_exact_round_a={round_a:?}");
    println!("x16_exact_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic exact x=17 local probe built from the observed real setup pattern"]
fn mode2_to_mode0_sprites_x17_real_probe_logs_shape() {
    let round_a = run_intr_2_mode0_sprites_real_setup_probe(57, 17);
    let round_b = run_intr_2_mode0_sprites_real_setup_probe(56, 17);
    println!("x17_exact_round_a={round_a:?}");
    println!("x17_exact_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic exact x=32 local probe built from the observed real setup pattern"]
fn mode2_to_mode0_sprites_x32_real_probe_logs_shape() {
    let round_a = run_intr_2_mode0_sprites_real_setup_probe(57, 32);
    let round_b = run_intr_2_mode0_sprites_real_setup_probe(56, 32);
    println!("x32_exact_round_a={round_a:?}");
    println!("x32_exact_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic exact x=32 local probe with SCX=4"]
fn mode2_to_mode0_sprites_x32_real_probe_logs_shape_with_scx4() {
    let round_a = run_intr_2_mode0_sprites_real_setup_probe_with_scx(57, 32, 4);
    let round_b = run_intr_2_mode0_sprites_real_setup_probe_with_scx(56, 32, 4);
    println!("x32_exact_scx4_round_a={round_a:?}");
    println!("x32_exact_scx4_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic exact x=33 local probe built from the observed real setup pattern"]
fn mode2_to_mode0_sprites_x33_real_probe_logs_shape() {
    let round_a = run_intr_2_mode0_sprites_real_setup_probe(57, 33);
    let round_b = run_intr_2_mode0_sprites_real_setup_probe(56, 33);
    println!("x33_exact_round_a={round_a:?}");
    println!("x33_exact_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic exact x=34 local probe built from the observed real setup pattern"]
fn mode2_to_mode0_sprites_x34_real_probe_logs_shape() {
    let round_a = run_intr_2_mode0_sprites_real_setup_probe(57, 34);
    let round_b = run_intr_2_mode0_sprites_real_setup_probe(56, 34);
    println!("x34_exact_round_a={round_a:?}");
    println!("x34_exact_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic exact x=34 local probe with SCX=4"]
fn mode2_to_mode0_sprites_x34_real_probe_logs_shape_with_scx4() {
    let round_a = run_intr_2_mode0_sprites_real_setup_probe_with_scx(57, 34, 4);
    let round_b = run_intr_2_mode0_sprites_real_setup_probe_with_scx(56, 34, 4);
    println!("x34_exact_scx4_round_a={round_a:?}");
    println!("x34_exact_scx4_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic exact x=35 local probe built from the observed real setup pattern"]
fn mode2_to_mode0_sprites_x35_real_probe_logs_shape() {
    let round_a = run_intr_2_mode0_sprites_real_setup_probe(57, 35);
    let round_b = run_intr_2_mode0_sprites_real_setup_probe(56, 35);
    println!("x35_exact_round_a={round_a:?}");
    println!("x35_exact_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic exact x=39 local probe built from the observed real setup pattern"]
fn mode2_to_mode0_sprites_x39_real_probe_logs_shape() {
    let round_a = run_intr_2_mode0_sprites_real_setup_probe(57, 39);
    let round_b = run_intr_2_mode0_sprites_real_setup_probe(56, 39);
    println!("x39_exact_round_a={round_a:?}");
    println!("x39_exact_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic exact two-round x=4 local probe built from the real mooneye setup"]
fn mode2_to_mode0_sprites_x4_two_round_real_probe_logs_shape() {
    let observation = run_intr_2_mode0_sprites_two_round_real_setup_probe(4);
    println!("x4_two_round={observation:?}");
}

#[test]
#[ignore = "diagnostic exact two-round x=5 local probe built from the real mooneye setup"]
fn mode2_to_mode0_sprites_x5_two_round_real_probe_logs_shape() {
    let observation = run_intr_2_mode0_sprites_two_round_real_setup_probe(5);
    println!("x5_two_round={observation:?}");
}

#[test]
#[ignore = "diagnostic exact two-round x=6 local probe built from the real mooneye setup"]
fn mode2_to_mode0_sprites_x6_two_round_real_probe_logs_shape() {
    let observation = run_intr_2_mode0_sprites_two_round_real_setup_probe(6);
    println!("x6_two_round={observation:?}");
}

#[test]
#[ignore = "diagnostic exact two-round x=7 local probe built from the real mooneye setup"]
fn mode2_to_mode0_sprites_x7_two_round_real_probe_logs_shape() {
    let observation = run_intr_2_mode0_sprites_two_round_real_setup_probe(7);
    println!("x7_two_round={observation:?}");
}

#[test]
#[ignore = "diagnostic exact two-round x=8 local probe built from the real mooneye setup"]
fn mode2_to_mode0_sprites_x8_two_round_real_probe_logs_shape() {
    let observation = run_intr_2_mode0_sprites_two_round_real_setup_probe(8);
    println!("x8_two_round={observation:?}");
}

#[test]
#[ignore = "diagnostic exact two-round x=9 local probe built from the real mooneye setup"]
fn mode2_to_mode0_sprites_x9_two_round_real_probe_logs_shape() {
    let observation = run_intr_2_mode0_sprites_two_round_real_setup_probe(9);
    println!("x9_two_round={observation:?}");
}

#[test]
#[ignore = "diagnostic exact two-round x=10 local probe built from the real mooneye setup"]
fn mode2_to_mode0_sprites_x10_two_round_real_probe_logs_shape() {
    let observation = run_intr_2_mode0_sprites_two_round_real_setup_probe(10);
    println!("x10_two_round={observation:?}");
}

#[test]
#[ignore = "diagnostic exact two-round multi probe for testcase 9 shape (10 sprites at X=0)"]
fn mode2_to_mode0_sprites_multi_two_round_real_setup_probe_logs_case9_shape() {
    let observation =
        run_intr_2_mode0_sprites_multi_two_round_real_setup_probe(&[0_u8; 10], 57, 56);
    println!("case9_two_round={observation:?}");
}

#[test]
#[ignore = "diagnostic round-a STAT reads for the exact two-round testcase 9 local probe"]
fn mode2_to_mode0_sprites_multi_two_round_real_setup_probe_logs_case9_round_a_reads() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(
            build_intr_2_mode0_sprites_multi_two_round_real_setup_probe_rom(&[0_u8; 10], 57, 56),
        )
        .expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..2_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        if let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
        }

        if arm_count == 1
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case9_local_round_a_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count == 1
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case9_local_round_a_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
            if read_count >= 2 {
                return;
            }
        }

        previous_ppu = machine.ppu().snapshot();
    }

    panic!("local case9 two-round probe did not reach round-a reads");
}

#[test]
#[ignore = "diagnostic ly70 terminal window 311..316 for the exact two-round testcase 9 local probe"]
fn mode2_to_mode0_sprites_multi_two_round_real_setup_probe_logs_case9_round_a_terminal_window() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(
            build_intr_2_mode0_sprites_multi_two_round_real_setup_probe_rom(&[0_u8; 10], 57, 56),
        )
        .expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut previous = None;

    for _ in 0..2_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        if let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
        }

        if arm_count == 1 {
            let ppu = machine.ppu().snapshot();
            if ppu.ly == 70 && (311..=316).contains(&ppu.line_dot) {
                let current = Intr2Mode0TimingSpritesTerminalWindowObservation {
                    line_dot: ppu.line_dot,
                    mode: ppu.mode,
                    mode0_start_dot: ppu.mode0_start_dot,
                    current_transfer_x: ppu.bg_current_transfer_x,
                    visible_pixels_output: ppu.visible_pixels_output,
                    current_transfer_lane: ppu.bg_current_transfer_lane,
                    current_transfer_source_window: ppu.bg_current_transfer_source_window,
                    current_transfer_backing: ppu.bg_current_transfer_backing,
                    current_transfer_readiness: ppu.bg_current_transfer_readiness,
                    current_transfer_kind: ppu.bg_current_transfer_kind,
                    bg_fifo_len: ppu.bg_fifo_pixels.len(),
                    startup_fifo_placeholders: ppu.bg_startup_fifo_placeholders,
                    bg_push_pending: ppu.bg_push_pending,
                    bg_push_disposition: ppu.bg_push_disposition,
                    bg_push_entry_delay_remaining: ppu.bg_push_entry_delay_remaining,
                    bg_fetcher_stage: ppu.bg_fetcher_stage,
                    bg_fetcher_stage_dot: ppu.bg_fetcher_stage_dot,
                    obj_fetcher_stage: ppu.obj_fetcher_stage,
                    obj_fetcher_stage_dot: ppu.obj_fetcher_stage_dot,
                    obj_pending_hit_len: ppu.obj_pending_hit_len,
                    obj_fetched_same_x_active_count: ppu.obj_fetched_same_x_active_count,
                    obj_fetched_same_x_pending_count: ppu.obj_fetched_same_x_pending_count,
                };
                if previous.as_ref() != Some(&current) {
                    println!("case9_local_round_a_terminal={current:?}");
                    previous = Some(current);
                }
            } else if previous.is_some() && ppu.ly > 70 {
                return;
            }
        }
    }

    panic!("local case9 two-round probe did not reach round-a terminal window");
}

#[test]
#[ignore = "diagnostic round-b STAT reads for the exact two-round testcase 9 local probe"]
fn mode2_to_mode0_sprites_multi_two_round_real_setup_probe_logs_case9_round_b_reads() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(
            build_intr_2_mode0_sprites_multi_two_round_real_setup_probe_rom(&[0_u8; 10], 57, 56),
        )
        .expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..2_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        if let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
        }

        if arm_count == 2
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case9_local_round_b_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count == 2
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case9_local_round_b_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
            if read_count >= 2 {
                return;
            }
        }

        previous_ppu = machine.ppu().snapshot();
    }

    panic!("local case9 two-round probe did not reach round-b reads");
}

#[test]
#[ignore = "diagnostic ly69 window 300..360 for the exact two-round testcase 9 local probe"]
fn mode2_to_mode0_sprites_multi_two_round_real_setup_probe_logs_case9_round_b_line69_window() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(
            build_intr_2_mode0_sprites_multi_two_round_real_setup_probe_rom(&[0_u8; 10], 57, 56),
        )
        .expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut previous = None;

    for _ in 0..2_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        if let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
        }

        if arm_count == 2 {
            let ppu = machine.ppu().snapshot();
            if ppu.ly == 69 && (300..=360).contains(&ppu.line_dot) {
                let current = Intr2Mode0TimingSpritesTerminalWindowObservation {
                    line_dot: ppu.line_dot,
                    mode: ppu.mode,
                    mode0_start_dot: ppu.mode0_start_dot,
                    current_transfer_x: ppu.bg_current_transfer_x,
                    visible_pixels_output: ppu.visible_pixels_output,
                    current_transfer_lane: ppu.bg_current_transfer_lane,
                    current_transfer_source_window: ppu.bg_current_transfer_source_window,
                    current_transfer_backing: ppu.bg_current_transfer_backing,
                    current_transfer_readiness: ppu.bg_current_transfer_readiness,
                    current_transfer_kind: ppu.bg_current_transfer_kind,
                    bg_fifo_len: ppu.bg_fifo_pixels.len(),
                    startup_fifo_placeholders: ppu.bg_startup_fifo_placeholders,
                    bg_push_pending: ppu.bg_push_pending,
                    bg_push_disposition: ppu.bg_push_disposition,
                    bg_push_entry_delay_remaining: ppu.bg_push_entry_delay_remaining,
                    bg_fetcher_stage: ppu.bg_fetcher_stage,
                    bg_fetcher_stage_dot: ppu.bg_fetcher_stage_dot,
                    obj_fetcher_stage: ppu.obj_fetcher_stage,
                    obj_fetcher_stage_dot: ppu.obj_fetcher_stage_dot,
                    obj_pending_hit_len: ppu.obj_pending_hit_len,
                    obj_fetched_same_x_active_count: ppu.obj_fetched_same_x_active_count,
                    obj_fetched_same_x_pending_count: ppu.obj_fetched_same_x_pending_count,
                };
                if previous.as_ref() != Some(&current) {
                    println!("case9_local_round_b_line69={current:?}");
                    previous = Some(current);
                }
            } else if previous.is_some() && ppu.ly > 69 {
                return;
            }
        }
    }

    panic!("local case9 two-round probe did not reach round-b line69 window");
}

#[test]
#[ignore = "diagnostic exact two-round x=160 local probe built from the real mooneye setup"]
fn mode2_to_mode0_sprites_x160_two_round_real_probe_logs_shape() {
    let observation = run_intr_2_mode0_sprites_two_round_real_setup_probe_with_delays(160, 57, 56);
    println!("x160_two_round={observation:?}");
}

#[test]
#[ignore = "diagnostic exact two-round x=161 local probe built from the real mooneye setup"]
fn mode2_to_mode0_sprites_x161_two_round_real_probe_logs_shape() {
    let observation = run_intr_2_mode0_sprites_two_round_real_setup_probe_with_delays(161, 57, 56);
    println!("x161_two_round={observation:?}");
}

#[test]
#[ignore = "diagnostic exact two-round x=167 local probe built from the real mooneye setup"]
fn mode2_to_mode0_sprites_x167_two_round_real_probe_logs_shape() {
    let observation = run_intr_2_mode0_sprites_two_round_real_setup_probe(167);
    println!("x167_two_round={observation:?}");
}

#[test]
#[ignore = "diagnostic exact two-round x=168 local probe built from the real mooneye setup"]
fn mode2_to_mode0_sprites_x168_two_round_real_probe_logs_shape() {
    let observation = run_intr_2_mode0_sprites_two_round_real_setup_probe_with_delays(168, 41, 40);
    println!("x168_two_round={observation:?}");
}

#[test]
#[ignore = "diagnostic exact two-round x=169 local probe built from the real mooneye setup"]
fn mode2_to_mode0_sprites_x169_two_round_real_probe_logs_shape() {
    let observation = run_intr_2_mode0_sprites_two_round_real_setup_probe_with_delays(169, 41, 40);
    println!("x169_two_round={observation:?}");
}

#[test]
#[ignore = "diagnostic exact x=168 local probe STAT reads built from the real mooneye setup"]
fn mode2_to_mode0_sprites_x168_two_round_real_probe_logs_stat_reads() {
    let reads = sample_intr_2_mode0_sprites_stat_reads_after_irq_from_rom(
        build_intr_2_mode0_sprites_two_round_real_setup_probe_rom(168, 41, 40),
        4,
    );
    println!("x168_two_round_reads={reads:?}");
}

#[test]
#[ignore = "diagnostic exact x=168 local probe line68 window 240..260 built from the real mooneye setup"]
fn mode2_to_mode0_sprites_x168_two_round_real_probe_logs_line68_window_240_260() {
    let observations = sample_intr_2_mode0_sprites_line_changes_from_rom(
        build_intr_2_mode0_sprites_two_round_real_setup_probe_rom(168, 41, 40),
        68,
    );
    for observation in observations {
        if (240..=260).contains(&observation.line_dot) {
            println!("x168_two_round_line68={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic exact two-round x=160 local probe built from the mooneye source setup"]
fn mode2_to_mode0_sprites_x160_two_round_source_setup_probe_logs_shape() {
    let observation =
        run_intr_2_mode0_sprites_two_round_source_setup_probe_with_delays(160, 57, 56);
    println!("x160_source_setup_two_round={observation:?}");
}

#[test]
#[ignore = "diagnostic exact two-round x=160 local probe with timed direct FF40 writes"]
fn mode2_to_mode0_sprites_x160_two_round_timed_direct_setup_probe_logs_shape() {
    let observation =
        run_intr_2_mode0_sprites_two_round_timed_direct_setup_probe_with_delays(160, 57, 56);
    println!("x160_timed_direct_setup_two_round={observation:?}");
}

#[test]
#[ignore = "diagnostic compare x=160 post-IRQ STAT reads between baseline mini-probe and timed direct setup"]
fn mode2_to_mode0_sprites_x160_compare_baseline_and_timed_direct_setup_stat_reads() {
    let baseline_reads = sample_intr_2_mode0_sprites_multi_probe_stat_reads(56, &[160; 10], 4);
    let timed_direct_reads = sample_intr_2_mode0_sprites_stat_reads_after_irq_from_rom(
        build_intr_2_mode0_sprites_two_round_timed_direct_setup_probe_rom(160, 57, 56),
        4,
    );
    println!("x160_baseline_reads={baseline_reads:?}");
    println!("x160_timed_direct_reads={timed_direct_reads:?}");
}

#[test]
#[ignore = "diagnostic line68 window 300..324 for x=160 timed direct setup"]
fn mode2_to_mode0_sprites_x160_timed_direct_setup_logs_line68_window_300_324() {
    let observations = sample_intr_2_mode0_sprites_line_changes_from_rom(
        build_intr_2_mode0_sprites_two_round_timed_direct_setup_probe_rom(160, 57, 56),
        68,
    );
    for observation in observations {
        if (300..=324).contains(&observation.line_dot) {
            println!("x160_timed_direct_line68={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic compare x=160 STAT arm to IRQ timing between baseline mini-probe and timed direct setup"]
fn mode2_to_mode0_sprites_x160_compare_baseline_and_timed_direct_irq_timing() {
    let baseline_irq = sample_intr_2_mode0_sprites_irq_after_stat_arm_from_rom(
        build_intr_2_mode0_sprites_multi_probe_rom(56, &[160; 10]),
    );
    let timed_direct_irq = sample_intr_2_mode0_sprites_irq_after_stat_arm_from_rom(
        build_intr_2_mode0_sprites_two_round_timed_direct_setup_probe_rom(160, 57, 56),
    );
    println!("x160_baseline_irq={baseline_irq:?}");
    println!("x160_timed_direct_irq={timed_direct_irq:?}");
}

#[test]
#[ignore = "diagnostic round-by-round arm, IRQ, and STAT reads for x=160 timed direct setup"]
fn mode2_to_mode0_sprites_x160_timed_direct_setup_logs_round_events() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(
            build_intr_2_mode0_sprites_two_round_timed_direct_setup_probe_rom(160, 56, 55),
        )
        .expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..2_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        if let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "x160_timed_direct_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "x160_timed_direct_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "x160_timed_direct_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d != 0
        {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic round-by-round line68 window 290..324 for x=160 timed direct setup"]
fn mode2_to_mode0_sprites_x160_timed_direct_setup_logs_line68_window_per_round() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(
            build_intr_2_mode0_sprites_two_round_timed_direct_setup_probe_rom(160, 56, 55),
        )
        .expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut previous = None;

    for _ in 0..2_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        if let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            previous = None;
        }

        let ppu = machine.ppu().snapshot();
        if arm_count > 0 && ppu.ly == 68 && (290..=324).contains(&ppu.line_dot) {
            let current = Intr2Mode0TimingSpritesLine68Observation {
                line_dot: ppu.line_dot,
                mode: ppu.mode,
                mode0_start_dot: ppu.mode0_start_dot,
                current_transfer_x: ppu.bg_current_transfer_x,
                visible_pixels_output: ppu.visible_pixels_output,
                current_transfer_lane: ppu.bg_current_transfer_lane,
                current_transfer_source_window: ppu.bg_current_transfer_source_window,
                current_transfer_backing: ppu.bg_current_transfer_backing,
                current_transfer_readiness: ppu.bg_current_transfer_readiness,
                bg_fifo_len: ppu.bg_fifo_pixels.len(),
                startup_fifo_placeholders: ppu.bg_startup_fifo_placeholders,
                obj_fetcher_stage: ppu.obj_fetcher_stage,
                obj_fetcher_stage_dot: ppu.obj_fetcher_stage_dot,
                obj_pending_hit_match_x: ppu.obj_pending_hit_match_x,
                obj_pending_hit_len: ppu.obj_pending_hit_len,
                obj_pending_hit_front_sprite_slot: ppu.obj_pending_hit_front_sprite_slot,
                bg_fetcher_stage: ppu.bg_fetcher_stage,
                bg_fetcher_stage_dot: ppu.bg_fetcher_stage_dot,
                selected_sprites_len: ppu.selected_sprites.len(),
            };
            if previous.as_ref() != Some(&current) {
                println!("x160_timed_direct_round{arm_count}_line68={current:?}");
                previous = Some(current);
            }
        }

        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d != 0
        {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic exact two-round x=161 local probe built from the mooneye source setup"]
fn mode2_to_mode0_sprites_x161_two_round_source_setup_probe_logs_shape() {
    let observation =
        run_intr_2_mode0_sprites_two_round_source_setup_probe_with_delays(161, 57, 56);
    println!("x161_source_setup_two_round={observation:?}");
}

#[test]
#[ignore = "diagnostic same-x x=160 mini-probe with both live writes using LCDC=0x93"]
fn mode2_to_mode0_sprites_x160_multi_probe_logs_shape_with_obj_enabled_from_first_live_write() {
    let sprite_xs = [160; 10];
    let round_a = run_intr_2_mode0_sprites_multi_probe_with_setup(56, &sprite_xs, 0, 0x93, 0x93);
    let round_b = run_intr_2_mode0_sprites_multi_probe_with_setup(55, &sprite_xs, 0, 0x93, 0x93);
    println!("x160_obj_on_first_live_write_round_a={round_a:?}");
    println!("x160_obj_on_first_live_write_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic same-x x=160 probe using real-style set 7/set 1 LCDC writes"]
fn mode2_to_mode0_sprites_x160_multi_live_obj_enable_probe_logs_shape() {
    let sprite_xs = [160_u8; 10];
    for delay_nops in [57_usize, 56, 55] {
        let observation =
            run_intr_2_mode0_sprites_multi_live_obj_enable_probe(delay_nops, &sprite_xs);
        println!("x160_live_obj_enable_delay{delay_nops}={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic STAT reads for x160 probe using real-style set 7/set 1 LCDC writes"]
fn mode2_to_mode0_sprites_x160_multi_live_obj_enable_probe_logs_stat_reads() {
    let reads = sample_intr_2_mode0_sprites_stat_reads_after_irq_from_rom(
        build_intr_2_mode0_sprites_multi_live_obj_enable_probe_rom(55, &[160_u8; 10]),
        4,
    );
    println!("x160_live_obj_enable_reads={reads:?}");
}

#[test]
#[ignore = "diagnostic two-round x160 probe using real-style set 7/set 1 LCDC writes"]
fn mode2_to_mode0_sprites_x160_two_round_live_obj_enable_probe_logs_shape() {
    let observation_57_56 =
        run_intr_2_mode0_sprites_two_round_live_obj_enable_probe_with_delays(160, 57, 56);
    let observation_57_55 =
        run_intr_2_mode0_sprites_two_round_live_obj_enable_probe_with_delays(160, 57, 55);
    println!("x160_two_round_live_obj_enable_57_56={observation_57_56:?}");
    println!("x160_two_round_live_obj_enable_57_55={observation_57_55:?}");
}

#[test]
#[ignore = "diagnostic first two rounds for x160 two-round probe using real-style set 7/set 1 LCDC writes"]
fn mode2_to_mode0_sprites_x160_two_round_live_obj_enable_probe_logs_round_events() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine
        .load_cartridge(build_intr_2_mode0_sprites_two_round_live_obj_enable_probe_rom(160, 57, 55))
        .expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..2_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        if let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "x160_two_round_live_obj_enable_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "x160_two_round_live_obj_enable_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "x160_two_round_live_obj_enable_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );

            if arm_count >= 2 && read_count >= 2 {
                return;
            }
        }

        previous_ppu = machine.ppu().snapshot();
    }

    panic!("x160 two-round live-obj-enable round-events probe did not terminate");
}

#[test]
#[ignore = "diagnostic ly66 window for same-x x=160 mini-probe with both live writes using LCDC=0x93"]
fn mode2_to_mode0_sprites_x160_multi_probe_logs_line66_with_obj_enabled_from_first_live_write() {
    let observations = sample_intr_2_mode0_sprites_multi_probe_line_changes_pre_irq_with_setup(
        55, &[160; 10], 0, 0x93, 0x93, 66,
    );
    for observation in observations {
        if (240..=320).contains(&observation.line_dot) {
            println!("x160_obj_on_first_live_write_line66={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic first line66 divergence between baseline x160 case29 probe and LCDC=0x93/0x93 variant"]
fn mode2_to_mode0_sprites_x160_multi_probe_logs_first_round_line66_divergence_for_obj_enabled_from_first_live_write()
 {
    let baseline = sample_intr_2_mode0_sprites_multi_probe_line_changes_pre_irq_with_setup(
        56, &[160; 10], 0, 0x91, 0x93, 66,
    );
    let obj_enabled_from_first_live_write =
        sample_intr_2_mode0_sprites_multi_probe_line_changes_pre_irq_with_setup(
            56, &[160; 10], 0, 0x93, 0x93, 66,
        );

    for (index, (baseline, obj_enabled_from_first_live_write)) in baseline
        .iter()
        .zip(obj_enabled_from_first_live_write.iter())
        .enumerate()
    {
        if baseline != obj_enabled_from_first_live_write
            && ((240..=320).contains(&baseline.line_dot)
                || (240..=320).contains(&obj_enabled_from_first_live_write.line_dot))
        {
            println!("case29_round_a_diff_index={index}");
            println!("case29_round_a_baseline={baseline:?}");
            println!(
                "case29_round_a_obj_on_first_live_write={obj_enabled_from_first_live_write:?}"
            );
            return;
        }
    }

    println!("case29_round_a_no_diff_in_window");
}

#[test]
#[ignore = "diagnostic STAT reads for x160 case29 probe with LCDC=0x93 on both live writes"]
fn mode2_to_mode0_sprites_x160_multi_probe_logs_stat_reads_with_obj_enabled_from_first_live_write()
{
    let reads = sample_intr_2_mode0_sprites_multi_probe_stat_reads_with_setup(
        56,
        &[160_u8; 10],
        4,
        0x93,
        0x93,
    );
    println!("x160_obj_on_first_live_write_reads={reads:?}");
}

#[test]
#[ignore = "diagnostic ly66 window for the exact case13 local probe"]
fn mode2_to_mode0_sprites_case13_real_probe_logs_line66() {
    let observations = sample_intr_2_mode0_sprites_case13_real_probe_line_changes_pre_irq(56, 66);
    for observation in observations {
        if (90..=140).contains(&observation.line_dot) {
            println!("case13_exact_line66={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic ly66 window for the exact x=5 local probe"]
fn mode2_to_mode0_sprites_x5_real_probe_logs_line66() {
    let observations = sample_intr_2_mode0_sprites_real_setup_probe_line_changes_pre_irq(56, 5, 66);
    for observation in observations {
        if (90..=140).contains(&observation.line_dot) {
            println!("x5_exact_line66={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic ly66 window for the exact x=10 local probe"]
fn mode2_to_mode0_sprites_x10_real_probe_logs_line66() {
    let observations =
        sample_intr_2_mode0_sprites_real_setup_probe_line_changes_pre_irq(56, 10, 66);
    for observation in observations {
        if (90..=140).contains(&observation.line_dot) {
            println!("x10_exact_line66={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic ly67 window for the exact x=10 local probe"]
fn mode2_to_mode0_sprites_x10_real_probe_logs_line67() {
    let observations =
        sample_intr_2_mode0_sprites_real_setup_probe_line_changes_pre_irq(56, 10, 67);
    for observation in observations {
        if (118..=140).contains(&observation.line_dot) {
            println!("x10_exact_line67={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic ly66 window for the exact x=16 local probe"]
fn mode2_to_mode0_sprites_x16_real_probe_logs_line66() {
    let observations = sample_intr_2_mode0_sprites_multi_probe_line_changes_pre_irq_with_setup(
        57, &[16; 10], 0, 0x13, 0x93, 66,
    );
    for observation in observations {
        if (90..=160).contains(&observation.line_dot) {
            println!("x16_exact_line66={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic ly67 window for the exact x=16 local probe"]
fn mode2_to_mode0_sprites_x16_real_probe_logs_line67() {
    let observations = sample_intr_2_mode0_sprites_multi_probe_line_changes_pre_irq_with_setup(
        57, &[16; 10], 0, 0x13, 0x93, 67,
    );
    for observation in observations {
        if (90..=160).contains(&observation.line_dot) {
            println!("x16_exact_line67={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic ly66 window for the exact x=34 local probe"]
fn mode2_to_mode0_sprites_x34_real_probe_logs_line66() {
    let observations = sample_intr_2_mode0_sprites_multi_probe_line_changes_pre_irq_with_setup(
        57, &[34; 10], 0, 0x13, 0x93, 66,
    );
    for observation in observations {
        if (90..=160).contains(&observation.line_dot) {
            println!("x34_exact_line66={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic ly67 window for the exact x=34 local probe"]
fn mode2_to_mode0_sprites_x34_real_probe_logs_line67() {
    let observations = sample_intr_2_mode0_sprites_multi_probe_line_changes_pre_irq_with_setup(
        57, &[34; 10], 0, 0x13, 0x93, 67,
    );
    for observation in observations {
        if (90..=160).contains(&observation.line_dot) {
            println!("x34_exact_line67={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic ly66 window for the x=34 real-style setup mini-probe with scx=0"]
fn mode2_to_mode0_sprites_x34_real_setup_logs_line66() {
    let observations = sample_intr_2_mode0_sprites_multi_probe_line_changes_pre_irq_with_setup(
        57, &[34; 10], 0, 0x13, 0x93, 66,
    );
    for observation in observations {
        if (90..=160).contains(&observation.line_dot) {
            println!("x34_real_setup_line66={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic ly66 window for the x=34 real-style setup mini-probe with scx=4"]
fn mode2_to_mode0_sprites_x34_real_setup_logs_line66_with_scx4() {
    let observations = sample_intr_2_mode0_sprites_multi_probe_line_changes_pre_irq_with_setup(
        57, &[34; 10], 4, 0x13, 0x93, 66,
    );
    for observation in observations {
        if (90..=160).contains(&observation.line_dot) {
            println!("x34_real_setup_scx4_line66={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic line68 state changes for mooneye testcase 11 shape mini-probe"]
fn mode2_to_mode0_sprites_multi_probe_logs_mooneye_case11_shape_line68() {
    let observations = sample_intr_2_mode0_sprites_multi_probe_line68_changes(56, &[2_u8; 10]);
    for observation in observations {
        println!("case11_shape_line68={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic single-read probe for mooneye testcase 11 shape mini-probe"]
fn mode2_to_mode0_sprites_single_read_probe_logs_mooneye_case11_shape() {
    let round_a = sample_intr_2_mode0_sprites_single_read_probe(56, &[2_u8; 10]);
    let round_b = sample_intr_2_mode0_sprites_single_read_probe(55, &[2_u8; 10]);
    println!("case11_shape_single_read_round_a={round_a:?}");
    println!("case11_shape_single_read_round_b={round_b:?}");
}

#[test]
#[ignore = "diagnostic single-read probe for offscreen-right same-x mode0 publication"]
fn mode2_to_mode0_sprites_single_read_probe_logs_offscreen_right_cases() {
    for (label, delay_nops, sprite_xs) in [
        ("x168_delay41", 41_usize, vec![168_u8; 10]),
        ("x168_delay40", 40_usize, vec![168_u8; 10]),
        ("x169_delay41", 41_usize, vec![169_u8; 10]),
        ("x169_delay40", 40_usize, vec![169_u8; 10]),
    ] {
        let read = sample_intr_2_mode0_sprites_single_read_probe(delay_nops, &sprite_xs);
        println!("{label} read={read:?}");
    }
}

#[test]
#[ignore = "diagnostic same-x sweep for local shapes near the case19 terminal read"]
fn mode2_to_mode0_sprites_single_read_probe_sweeps_same_x_shapes_near_case19() {
    for sprite_x in 0_u8..=40 {
        let read = sample_intr_2_mode0_sprites_single_read_probe(56, &[sprite_x; 10]);
        if let Some(read) = read
            && read.value == 0xA3
            && (308..=316).contains(&read.halt_line_dot)
        {
            println!("case19_sweep x={sprite_x} read={read:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic IRQ summary for late testcase ladder of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_late_case_irq_summary() {
    for testcase_index in 0..=19 {
        let irq = sample_real_mooneye_intr_2_mode0_timing_sprites_irq_after_stat_arm_for_testcase(
            testcase_index,
        );
        println!("late_case_irq testcase={testcase_index} irq={irq:?}");
    }
}

#[test]
#[ignore = "diagnostic access window for testcase 0 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_setup_accesses() {
    let (failure, setup_events, tail_events) =
        sample_real_mooneye_intr_2_mode0_timing_sprites_accesses(96);
    println!("failure={failure:?}");
    for access in setup_events {
        println!("setup={access:?}");
    }
    for access in tail_events {
        println!("tail={access:?}");
    }
}

#[test]
#[ignore = "diagnostic tail accesses before the current full-ROM failure of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_tail_accesses_before_current_failure() {
    let (failure, tail_events) =
        sample_real_mooneye_intr_2_mode0_timing_sprites_tail_accesses_before_current_failure(64);
    println!("failure={failure:?}");
    for access in tail_events {
        println!("tail={access:?}");
    }
}

#[test]
#[ignore = "diagnostic broad setup access window for testcase 0 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_broad_setup_accesses() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut capture_active = false;
    let mut events = Vec::new();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if let Some(activity) = cpu_snapshot.last_bus_activity {
            let is_broad_setup_access =
                matches!(activity.address, 0x8000..=0x9FFF | 0xFE00..=0xFE9F | 0xFF40..=0xFF44);
            if !capture_active
                && testcase_index == 0
                && activity.kind == CpuBusAccessKind::DataWrite
                && activity.address == 0xFF40
                && activity.value == 0x11
            {
                capture_active = true;
                let snapshot = machine.ppu().snapshot();
                events.push(Intr2Mode0TimingSpritesAccessObservation {
                    testcase_index,
                    kind: activity.kind,
                    address: activity.address,
                    value: activity.value,
                    pc: cpu_snapshot.registers.pc,
                    ly: snapshot.ly,
                    line_dot: snapshot.line_dot,
                    mode: snapshot.mode,
                    mode0_start_dot: snapshot.mode0_start_dot,
                    selected_sprites_len: snapshot.selected_sprites.len(),
                    visible_pixels_output: snapshot.visible_pixels_output,
                });
                continue;
            }

            if capture_active && is_broad_setup_access {
                let snapshot = machine.ppu().snapshot();
                if events.len() == 64 {
                    break;
                }
                events.push(Intr2Mode0TimingSpritesAccessObservation {
                    testcase_index,
                    kind: activity.kind,
                    address: activity.address,
                    value: activity.value,
                    pc: cpu_snapshot.registers.pc,
                    ly: snapshot.ly,
                    line_dot: snapshot.line_dot,
                    mode: snapshot.mode,
                    mode0_start_dot: snapshot.mode0_start_dot,
                    selected_sprites_len: snapshot.selected_sprites.len(),
                    visible_pixels_output: snapshot.visible_pixels_output,
                });
            }
        }
    }

    for event in events {
        println!("broad_setup={event:?}");
    }
}

#[test]
#[ignore = "diagnostic testcase-id progression for mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_testcase_progression() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut writes = Vec::new();
    for _ in 0..1_500_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        if let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF80
        {
            let snapshot = machine.ppu().snapshot();
            writes.push((
                activity.value,
                snapshot.ly,
                snapshot.line_dot,
                snapshot.mode,
                cpu_snapshot.registers.pc,
            ));
            if writes.len() == 3 {
                break;
            }
        }
    }

    for (value, ly, line_dot, mode, pc) in writes {
        println!(
            "testcase_write id={value} ly={ly} line_dot={line_dot} mode={mode:?} pc={pc:#06X}"
        );
    }
}

#[test]
#[ignore = "diagnostic failing testcase id for mooneye intr_2_mode0_timing_sprites final failure signature"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_failure_testcase_id() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        let cpu = machine.cpu().registers();
        if cpu.pc == 0x486D
            && ppu.ly == 49
            && ppu.line_dot == 248
            && ppu.mode == PpuAccessMode::Drawing
        {
            println!(
                "failure_testcase_id={} mode0_start_dot={} selected_sprites_len={} visible_pixels_output={}",
                machine.read_bus(0xFF80),
                ppu.mode0_start_dot,
                ppu.selected_sprites.len(),
                ppu.visible_pixels_output
            );
            return;
        }
    }

    panic!("probe did not reach the current final failure signature");
}

#[test]
#[ignore = "diagnostic IRQ sample for testcase 0 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_irq_after_stat_arm() {
    let irq = sample_real_mooneye_intr_2_mode0_timing_sprites_irq_after_stat_arm();
    println!("irq={irq:?}");
}

#[test]
#[ignore = "diagnostic opcode fetch sample after testcase 0 irq of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_opcode_fetches_after_irq() {
    let fetches = sample_real_mooneye_intr_2_mode0_timing_sprites_opcode_fetches_after_irq(64);
    for fetch in fetches {
        println!("fetch={fetch:?}");
    }
}

#[test]
#[ignore = "diagnostic STAT read sample after testcase 0 irq of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_stat_reads_after_irq() {
    let reads = sample_real_mooneye_intr_2_mode0_timing_sprites_stat_reads_after_irq();
    for read in reads {
        println!("read={read:?}");
    }
}

#[test]
#[ignore = "diagnostic line68 state changes for testcase 0 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_line68_changes() {
    let observations = sample_real_mooneye_intr_2_mode0_timing_sprites_line68_changes();
    for observation in observations {
        println!("line68={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic STAT read sample after testcase 1 irq of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case1_stat_reads_after_irq() {
    let reads =
        sample_real_mooneye_intr_2_mode0_timing_sprites_stat_reads_after_irq_for_testcase(1, 1);
    for read in reads {
        println!("case1_read={read:?}");
    }
}

#[test]
#[ignore = "diagnostic detailed first STAT read after testcase 1 irq of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case1_first_stat_read_details() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut saw_irq_for_case1 = false;

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let testcase_index = machine.read_bus(0xFF80);
        if testcase_index != 1 {
            continue;
        }

        if !saw_irq_for_case1
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_case1 = true;
        }

        let cpu_snapshot = machine.cpu().snapshot();
        if saw_irq_for_case1
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            let ppu = machine.ppu().snapshot();
            println!(
                "case1_first_read_detail value={:#04X} ly={} line_dot={} mode={:?} mode0_start_dot={} bg_current_transfer_x={} visible_pixels_output={} bg_lane={:?} bg_source_window={:?} bg_backing={:?} bg_readiness={:?} bg_startup_fifo_placeholders={} bg_fifo_len={} obj_stage={:?} obj_stage_dot={} obj_pending_hit_match_x={:?} obj_pending_hit_len={} selected_sprites_len={}",
                activity.value,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_current_transfer_x,
                ppu.visible_pixels_output,
                ppu.bg_current_transfer_lane,
                ppu.bg_current_transfer_source_window,
                ppu.bg_current_transfer_backing,
                ppu.bg_current_transfer_readiness,
                ppu.bg_startup_fifo_placeholders,
                ppu.bg_fifo_pixels.len(),
                ppu.obj_fetcher_stage,
                ppu.obj_fetcher_stage_dot,
                ppu.obj_pending_hit_match_x,
                ppu.obj_pending_hit_len,
                ppu.selected_sprites.len(),
            );
            return;
        }
    }

    panic!("probe did not reach testcase 1 first STAT read");
}

#[test]
#[ignore = "diagnostic two-round real-setup ladder for testcase 1 shape (2 sprites at X=0)"]
fn mode2_to_mode0_sprites_multi_two_round_real_setup_probe_logs_case1_shape() {
    let sprite_xs = [0_u8, 0];
    for (delay_a, delay_b) in [(45_usize, 44_usize), (58, 57), (57, 56), (56, 55), (55, 54)] {
        let observation =
            run_intr_2_mode0_sprites_multi_two_round_real_setup_probe(&sprite_xs, delay_a, delay_b);
        println!("case1_real_setup_delay{delay_a}_{delay_b}={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic restart-helper ladder for testcase 1 shape (2 sprites at X=0)"]
fn mode2_to_mode0_sprites_multi_restart_helper_probe_logs_case1_shape() {
    let sprite_xs = [0_u8, 0];
    for delay_nops in [58_usize, 57, 56, 55, 54] {
        let observation =
            run_intr_2_mode0_sprites_multi_restart_helper_probe(delay_nops, &sprite_xs);
        println!("case1_restart_helper_delay{delay_nops}={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic live-obj-enable ladder for testcase 1 shape (2 sprites at X=0)"]
fn mode2_to_mode0_sprites_multi_live_obj_enable_probe_logs_case1_shape() {
    let sprite_xs = [0_u8, 0];
    for delay_nops in [58_usize, 57, 56, 55, 54] {
        let observation =
            run_intr_2_mode0_sprites_multi_live_obj_enable_probe(delay_nops, &sprite_xs);
        println!("case1_live_obj_enable_delay{delay_nops}={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic STAT reads for testcase 1 shape with real-style setup"]
fn mode2_to_mode0_sprites_multi_probe_logs_case1_real_setup_stat_reads() {
    let sprite_xs = [0_u8, 0];
    let round_a_reads = sample_intr_2_mode0_sprites_multi_probe_stat_reads_with_setup(
        56, &sprite_xs, 0, 0x13, 0x93,
    );
    let round_b_reads = sample_intr_2_mode0_sprites_multi_probe_stat_reads_with_setup(
        55, &sprite_xs, 0, 0x13, 0x93,
    );
    println!("case1_real_setup_round_a_reads={round_a_reads:?}");
    println!("case1_real_setup_round_b_reads={round_b_reads:?}");
}

#[test]
#[ignore = "diagnostic first STAT read for the local case1 real-style setup probe"]
fn mode2_to_mode0_sprites_multi_probe_logs_case1_real_setup_first_stat_read() {
    let sprite_xs = [0_u8, 0];
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_intr_2_mode0_sprites_multi_probe_rom_with_setup(
            56, &sprite_xs, 0, 0x13, 0x93,
        ))
        .expect("probe ROM should load");

    let mut saw_irq = false;
    for _ in 0..2_000_000 {
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

        let cpu_snapshot = machine.cpu().snapshot();
        if saw_irq
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            let ppu = machine.ppu().snapshot();
            println!(
                "local_case1_first_read value={:#04X} ly={} line_dot={} mode={:?} mode0_start_dot={} bg_current_transfer_x={} bg_lane={:?} bg_source_window={:?} bg_backing={:?} bg_readiness={:?} bg_startup_fifo_placeholders={} bg_fifo_len={} obj_stage={:?} obj_stage_dot={} obj_pending_hit_match_x={:?} obj_pending_hit_len={} selected_sprites_len={}",
                activity.value,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_current_transfer_x,
                ppu.bg_current_transfer_lane,
                ppu.bg_current_transfer_source_window,
                ppu.bg_current_transfer_backing,
                ppu.bg_current_transfer_readiness,
                ppu.bg_startup_fifo_placeholders,
                ppu.bg_fifo_pixels.len(),
                ppu.obj_fetcher_stage,
                ppu.obj_fetcher_stage_dot,
                ppu.obj_pending_hit_match_x,
                ppu.obj_pending_hit_len,
                ppu.selected_sprites.len(),
            );
            return;
        }
    }

    panic!("local case1 real-style setup probe did not reach the first STAT read");
}

#[test]
#[ignore = "diagnostic first STAT read for the exact local case1 two-round real-setup probe"]
fn mode2_to_mode0_sprites_multi_two_round_real_setup_probe_logs_case1_first_stat_read() {
    let sprite_xs = [0_u8, 0];
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(
            build_intr_2_mode0_sprites_multi_two_round_real_setup_probe_rom(&sprite_xs, 45, 44),
        )
        .expect("probe ROM should load");

    let mut saw_irq = false;
    for _ in 0..2_000_000 {
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

        let cpu_snapshot = machine.cpu().snapshot();
        if saw_irq
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            let ppu = machine.ppu().snapshot();
            println!(
                "local_case1_exact_first_read value={:#04X} ly={} line_dot={} mode={:?} mode0_start_dot={} bg_current_transfer_x={} bg_lane={:?} bg_source_window={:?} bg_backing={:?} bg_readiness={:?} bg_startup_fifo_placeholders={} bg_fifo_len={} obj_stage={:?} obj_stage_dot={} obj_pending_hit_match_x={:?} obj_pending_hit_len={} selected_sprites_len={}",
                activity.value,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_current_transfer_x,
                ppu.bg_current_transfer_lane,
                ppu.bg_current_transfer_source_window,
                ppu.bg_current_transfer_backing,
                ppu.bg_current_transfer_readiness,
                ppu.bg_startup_fifo_placeholders,
                ppu.bg_fifo_pixels.len(),
                ppu.obj_fetcher_stage,
                ppu.obj_fetcher_stage_dot,
                ppu.obj_pending_hit_match_x,
                ppu.obj_pending_hit_len,
                ppu.selected_sprites.len(),
            );
            return;
        }
    }

    panic!("local exact case1 two-round probe did not reach the first STAT read");
}

#[test]
#[ignore = "diagnostic round-by-round STAT reads for the exact local case1 two-round real-setup probe"]
fn mode2_to_mode0_sprites_multi_two_round_real_setup_probe_logs_case1_round_reads() {
    let sprite_xs = [0_u8, 0];
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(
            build_intr_2_mode0_sprites_multi_two_round_real_setup_probe_rom(&sprite_xs, 45, 44),
        )
        .expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..2_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        if let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "local_case1_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "local_case1_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "local_case1_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d != 0
        {
            return;
        }
    }

    panic!("local exact case1 two-round probe did not terminate");
}

#[test]
#[ignore = "diagnostic copied ROM-path probe for case1 arm/read signature"]
fn mode2_to_mode0_sprites_case1_rom_path_probe_logs_arm_and_first_read() {
    let sample = sample_intr_2_mode0_sprites_case1_rom_path_probe_reads_after_arm(2);
    let Intr2Mode0SpritesCase1RomPathArmObservation {
        ly,
        line_dot,
        mode,
        pc,
    } = sample.arm;
    println!("case1_rom_path_arm ly={ly} line_dot={line_dot} mode={mode:?} pc={pc:#06X}");
    println!("case1_rom_path_reads={:?}", sample.reads);
    println!("case1_rom_path_terminal={:?}", sample.terminal);
}

#[test]
#[ignore = "diagnostic copied ROM-path probe post-arm window for case1"]
fn mode2_to_mode0_sprites_case1_rom_path_probe_logs_post_arm_window() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_intr_2_mode0_sprites_case1_rom_path_probe_rom())
        .expect("probe ROM should load");

    let mut armed = None;
    for _ in 0..2_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        if let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            let ppu = machine.ppu().snapshot();
            armed = Some((ppu.ly, ppu.line_dot, ppu.mode, cpu_snapshot.registers.pc));
            println!(
                "case1_rom_path_post_arm_arm ly={} line_dot={} mode={:?} pc={:#06X}",
                ppu.ly, ppu.line_dot, ppu.mode, cpu_snapshot.registers.pc
            );
            break;
        }
    }

    assert!(
        armed.is_some(),
        "copied case1 ROM-path probe should arm STAT"
    );

    let mut printed = 0usize;
    let mut saw_irq = false;
    let mut saw_halt = false;

    for t_after_arm in 0..2048usize {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let ppu = machine.ppu().snapshot();

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
            println!(
                "case1_rom_path_post_arm_irq t_after_arm={} pc={:#06X} ly={} line_dot={} mode={:?}",
                t_after_arm, cpu_snapshot.registers.pc, ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if !saw_halt && machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted {
            saw_halt = true;
            println!(
                "case1_rom_path_post_arm_halt t_after_arm={} pc={:#06X} ly={} line_dot={} mode={:?}",
                t_after_arm, cpu_snapshot.registers.pc, ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if printed < 48
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && (activity.kind == CpuBusAccessKind::OpcodeFetch
                || matches!(activity.address, 0xFF0F | 0xFF41 | 0xFFFF))
        {
            printed += 1;
            println!(
                "case1_rom_path_post_arm_event{} t_after_arm={} kind={:?} address={:#06X} value={:#04X} pc={:#06X} state={:?} ly={} line_dot={} mode={:?}",
                printed,
                t_after_arm,
                activity.kind,
                activity.address,
                activity.value,
                cpu_snapshot.registers.pc,
                machine.cpu().execution_state(),
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
            );
        }
    }

    println!(
        "case1_rom_path_post_arm_summary saw_irq={} saw_halt={} final_pc={:#06X} final_state={:?} final_ly={} final_line_dot={} final_mode={:?}",
        saw_irq,
        saw_halt,
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.ppu().snapshot().mode,
    );
}

#[test]
#[ignore = "diagnostic copied ROM-path probe termination state for case1"]
fn mode2_to_mode0_sprites_case1_rom_path_probe_logs_termination() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_intr_2_mode0_sprites_case1_rom_path_probe_rom())
        .expect("probe ROM should load");

    for _ in 0..6_000_000 {
        machine.step_t_cycle();

        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted {
            let cpu = machine.cpu().snapshot();
            let ppu = machine.ppu().snapshot();
            if matches!(cpu.registers.pc, 0x010F | 0x0C09) {
                println!(
                    "case1_rom_path_halt d={:#04X} b={:#04X} c={:#04X} pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} last_bus_activity={:?}",
                    cpu.registers.d,
                    cpu.registers.b,
                    cpu.registers.c,
                    cpu.registers.pc,
                    ppu.ly,
                    ppu.line_dot,
                    ppu.mode,
                    ppu.mode0_start_dot,
                    cpu.last_bus_activity,
                );
                return;
            }
        }
    }

    panic!("copied case1 ROM-path probe did not terminate");
}

#[test]
#[ignore = "diagnostic setup writes for testcase 1 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case1_setup_writes() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut in_case1 = false;

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if testcase_index == 1 {
            in_case1 = true;
        }

        if in_case1
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && (matches!(activity.address, 0xFF40 | 0xFF41 | 0xFF43)
                || (0xFE00..=0xFE27).contains(&activity.address))
        {
            println!(
                "case1_setup_write address={:#06X} value={:#04X} pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} bg_startup_fifo_placeholders={} bg_current_transfer_x={} bg_lane={:?} obj_stage={:?} obj_stage_dot={} obj_pending_hit_len={} selected_sprites_len={}",
                activity.address,
                activity.value,
                cpu_snapshot.registers.pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_startup_fifo_placeholders,
                ppu.bg_current_transfer_x,
                ppu.bg_current_transfer_lane,
                ppu.obj_fetcher_stage,
                ppu.obj_fetcher_stage_dot,
                ppu.obj_pending_hit_len,
                ppu.selected_sprites.len(),
            );

            if activity.address == 0xFF41 && activity.value == 0x20 {
                break;
            }
        }

        if in_case1 && testcase_index > 1 {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic line68 state changes for testcase 1 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case1_line68_changes() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(1, 68);
    for observation in observations {
        println!("case1_line68={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic opcode fetch sample after testcase 1 irq of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case1_opcode_fetches_after_irq() {
    let fetches =
        sample_real_mooneye_intr_2_mode0_timing_sprites_opcode_fetches_after_irq_for_testcase(
            1, 64,
        );
    for fetch in fetches {
        println!("case1_fetch={fetch:?}");
    }
}

#[test]
#[ignore = "diagnostic first compare state for testcase 1 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case1_first_compare_state() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    for _ in 0..20_000_000 {
        machine.step_t_cycle();

        if machine.read_bus(0xFF80) != 1 {
            continue;
        }

        let cpu_snapshot = machine.cpu().snapshot();
        if matches!(cpu_snapshot.registers.pc, 0x0BA5 | 0x0C06) {
            let ppu = machine.ppu().snapshot();
            println!(
                "case1_compare pc={:#06X} a={:#04X} b={:#04X} c={:#04X} d={:#04X} e={:#04X} ly={} line_dot={} mode={:?} mode0_start_dot={} current_transfer_x={} lane={:?} source_window={:?} backing={:?} readiness={:?} startup_fifo_placeholders={} obj_stage={:?} obj_stage_dot={} obj_pending_hit_match_x={:?} obj_pending_hit_len={} selected_sprites_len={} last_bus_activity={:?}",
                cpu_snapshot.registers.pc,
                cpu_snapshot.registers.a,
                cpu_snapshot.registers.b,
                cpu_snapshot.registers.c,
                cpu_snapshot.registers.d,
                cpu_snapshot.registers.e,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_current_transfer_x,
                ppu.bg_current_transfer_lane,
                ppu.bg_current_transfer_source_window,
                ppu.bg_current_transfer_backing,
                ppu.bg_current_transfer_readiness,
                ppu.bg_startup_fifo_placeholders,
                ppu.obj_fetcher_stage,
                ppu.obj_fetcher_stage_dot,
                ppu.obj_pending_hit_match_x,
                ppu.obj_pending_hit_len,
                ppu.selected_sprites.len(),
                cpu_snapshot.last_bus_activity
            );
            return;
        }
    }

    panic!("probe did not reach testcase 1 compare state");
}

#[test]
#[ignore = "diagnostic focused line68 window around the first compare of testcase 1"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case1_line68_window_252_272() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(1, 68);
    for observation in observations {
        if (252..=272).contains(&observation.line_dot) {
            println!("case1_line68_window={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 1 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case1_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 1
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case1_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case1_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case1_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        if testcase_index > 1 {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic irq timing after STAT arm for testcase 1 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case1_irq_after_stat_arm() {
    let irq = sample_real_mooneye_intr_2_mode0_timing_sprites_irq_after_stat_arm_for_testcase(1);
    println!("case1_irq={irq:?}");
}

#[test]
#[ignore = "diagnostic round-2 STAT reads for testcase 1 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case1_round2_reads_only() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 1
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
        }

        if arm_count == 2
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case1_round2_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count == 2
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case1_round2_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        let pc = machine.cpu().registers().pc;
        if testcase_index == 1 && (0x484D..0x4871).contains(&pc) {
            let registers = machine.cpu().registers();
            let ppu = machine.ppu().snapshot();
            println!(
                "case1_fail arm_count={} read_count={} pc={:#06X} b={} c={} d={} e={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                arm_count,
                read_count,
                pc,
                registers.b,
                registers.c,
                registers.d,
                registers.e,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
            );
            return;
        }

        if testcase_index > 1 {
            break;
        }
    }

    panic!("probe did not reach case1 round 2 progression");
}

#[test]
#[ignore = "diagnostic tail MMIO/OAM accesses before the testcase 1 fail path of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case1_tail_accesses_before_fail() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut tail_events = Vec::with_capacity(64);
    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 1
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && matches!(activity.address, 0xFE00..=0xFE27 | 0xFF40..=0xFF43)
        {
            let ppu = machine.ppu().snapshot();
            let observation = Intr2Mode0TimingSpritesAccessObservation {
                testcase_index,
                kind: activity.kind,
                address: activity.address,
                value: activity.value,
                pc: cpu_snapshot.registers.pc,
                ly: ppu.ly,
                line_dot: ppu.line_dot,
                mode: ppu.mode,
                mode0_start_dot: ppu.mode0_start_dot,
                selected_sprites_len: ppu.selected_sprites.len(),
                visible_pixels_output: ppu.visible_pixels_output,
            };

            if tail_events.len() == 64 {
                tail_events.remove(0);
            }
            tail_events.push(observation);
        }

        let pc = machine.cpu().registers().pc;
        if testcase_index == 1 && (0x484D..0x4871).contains(&pc) {
            let registers = machine.cpu().registers();
            let ppu = machine.ppu().snapshot();
            println!(
                "case1_fail_tail arm_pc={:#06X} b={} c={} d={} e={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                pc,
                registers.b,
                registers.c,
                registers.d,
                registers.e,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
            for access in tail_events {
                println!("case1_tail={access:?}");
            }
            return;
        }

        if testcase_index > 1 {
            break;
        }
    }

    panic!("probe did not reach testcase 1 fail tail");
}

#[test]
#[ignore = "diagnostic STAT read sample after testcase 2 irq of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case2_stat_reads_after_irq() {
    let reads =
        sample_real_mooneye_intr_2_mode0_timing_sprites_stat_reads_after_irq_for_testcase(2, 1);
    for read in reads {
        println!("case2_read={read:?}");
    }
}

#[test]
#[ignore = "diagnostic line68 state changes for testcase 2 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case2_line68_changes() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(2, 68);
    for observation in observations {
        println!("case2_line68={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic first STAT read summary for early testcase ladder of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_early_case_read_summary() {
    for testcase_index in 0..=4 {
        let reads =
            sample_real_mooneye_intr_2_mode0_timing_sprites_stat_reads_after_irq_for_testcase(
                testcase_index,
                1,
            );
        println!("case{testcase_index}_first_read={:?}", reads.first());
    }
}

#[test]
#[ignore = "diagnostic irq timing after STAT arm for testcase 2 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case2_irq_after_stat_arm() {
    let irq = sample_real_mooneye_intr_2_mode0_timing_sprites_irq_after_stat_arm_for_testcase(2);
    println!("case2_irq={irq:?}");
}

#[test]
#[ignore = "diagnostic STAT read sample after testcase 3 irq of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case3_stat_reads_after_irq() {
    let reads =
        sample_real_mooneye_intr_2_mode0_timing_sprites_stat_reads_after_irq_for_testcase(3, 4);
    for read in reads {
        println!("case3_read={read:?}");
    }
}

#[test]
#[ignore = "diagnostic line68 state changes for testcase 3 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case3_line68_changes() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(3, 68);
    for observation in observations {
        println!("case3_line68={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic focused line68 window for testcase 3 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case3_line68_window_270_281() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(3, 68);
    for observation in observations {
        if (270..=281).contains(&observation.line_dot) {
            println!("case3_line68_window={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 3 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case3_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut in_case3 = false;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 3 {
            in_case3 = true;
        } else if in_case3 && testcase_index > 3 {
            let ppu = machine.ppu().snapshot();
            println!(
                "case3_exit testcase_index={} arm_count={} saw_irq={} read_count={} pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} visible_pixels_output={} selected_sprites_len={}",
                testcase_index,
                arm_count,
                saw_irq_for_arm,
                read_count,
                cpu_snapshot.registers.pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.visible_pixels_output,
                ppu.selected_sprites.len(),
            );
            break;
        }

        if !in_case3 {
            previous_ppu = machine.ppu().snapshot();
            continue;
        }

        if testcase_index == 3
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case3_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case3_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case3_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        let pc = machine.cpu().registers().pc;
        if (0x484D..0x4871).contains(&pc) {
            let ppu = machine.ppu().snapshot();
            println!(
                "case3_break testcase_index={} arm_count={} saw_irq={} read_count={} pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} visible_pixels_output={} selected_sprites_len={}",
                testcase_index,
                arm_count,
                saw_irq_for_arm,
                read_count,
                pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.visible_pixels_output,
                ppu.selected_sprites.len(),
            );
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 2 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case2_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 2
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case2_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case2_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case2_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        let pc = machine.cpu().registers().pc;
        if arm_count > 0 && (0x484D..0x4871).contains(&pc) {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 4 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case4_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 4
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case4_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case4_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case4_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        if testcase_index > 11 {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 5 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case5_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 5
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case5_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case5_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case5_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        let pc = machine.cpu().registers().pc;
        if (0x484D..0x4871).contains(&pc) {
            let ppu = machine.ppu().snapshot();
            println!(
                "case8_break testcase_index={} arm_count={} saw_irq={} read_count={} pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} visible_pixels_output={} selected_sprites_len={}",
                testcase_index,
                arm_count,
                saw_irq_for_arm,
                read_count,
                pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.visible_pixels_output,
                ppu.selected_sprites.len(),
            );
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic line68 state changes for testcase 4 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case4_line68_changes() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(4, 68);
    for observation in observations {
        println!("case4_line68={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic focused line68 window for testcase 4 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case4_line68_window_280_285() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(4, 68);
    for observation in observations {
        if (280..=285).contains(&observation.line_dot) {
            println!("case4_line68_window={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic line68 state changes for testcase 5 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case5_line68_changes() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(5, 68);
    for observation in observations {
        println!("case5_line68={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic STAT read sample after testcase 5 irq of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case5_stat_reads_after_irq() {
    let reads =
        sample_real_mooneye_intr_2_mode0_timing_sprites_stat_reads_after_irq_for_testcase(5, 4);
    println!("case5_reads={reads:?}");
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 10 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case10_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 10
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case10_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case10_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case10_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        let pc = machine.cpu().registers().pc;
        if (0x484D..0x4871).contains(&pc) {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic line68 state changes for testcase 10 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case10_line68_changes() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(10, 68);
    for observation in observations {
        println!("case10_line68={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 11 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case11_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();
    let mut saw_case11 = false;

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        saw_case11 |= testcase_index == 11;

        if testcase_index == 11
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case11_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case11_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case11_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        if saw_case11 && testcase_index > 11 {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic line68 state changes for testcase 11 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case11_line68_changes() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(11, 68);
    for observation in observations {
        println!("case11_line68={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic line68 window 90..150 for testcase 11 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case11_line68_window_90_150() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(11, 68);
    for observation in observations
        .into_iter()
        .filter(|observation| (90..=150).contains(&observation.line_dot))
    {
        println!("case11_line68_window={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic line68 window 150..230 for testcase 11 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case11_line68_window_150_230() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(11, 68);
    for observation in observations
        .into_iter()
        .filter(|observation| (150..=230).contains(&observation.line_dot))
    {
        println!("case11_line68_tail={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic line68 window 308..313 for testcase 11 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case11_line68_window_308_313() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(11, 68);
    for observation in observations
        .into_iter()
        .filter(|observation| (308..=313).contains(&observation.line_dot))
    {
        println!("case11_line68_focus={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic STAT reads after irq for testcase 11 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case11_stat_reads_after_irq() {
    let reads =
        sample_real_mooneye_intr_2_mode0_timing_sprites_stat_reads_after_irq_for_testcase(11, 8);
    println!("case11_reads={reads:?}");
}

#[test]
#[ignore = "diagnostic first two rounds for testcase 11 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case11_first_two_rounds() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..15_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 11
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case11_first_two_rounds_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case11_first_two_rounds_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case11_first_two_rounds_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );

            if arm_count >= 2 && read_count >= 2 {
                return;
            }
        }

        previous_ppu = machine.ppu().snapshot();
    }

    panic!("case11 first-two-rounds probe did not terminate");
}

#[test]
#[ignore = "diagnostic first STAT read snapshot for testcase 11 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case11_first_read_snapshot() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut armed = false;
    let mut saw_irq = false;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..15_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if !armed
            && testcase_index == 11
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            armed = true;
        }

        if armed
            && !saw_irq
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

        if saw_irq
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            let ppu = machine.ppu().snapshot();
            println!(
                "case11_first_read_snapshot value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={} blank_frame_active={} bg_current_transfer_x={} bg_current_transfer_lane={:?} bg_current_transfer_source_window={:?} bg_current_transfer_backing={:?} bg_current_transfer_readiness={:?} bg_current_transfer_kind={:?} visible_pixels_output={} bg_fifo_len={} bg_startup_fifo_placeholders={} selected_sprites_len={} obj_stage={:?} obj_pending_hit_len={} obj_pending_hit_match_x={:?}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.blank_frame_active,
                ppu.bg_current_transfer_x,
                ppu.bg_current_transfer_lane,
                ppu.bg_current_transfer_source_window,
                ppu.bg_current_transfer_backing,
                ppu.bg_current_transfer_readiness,
                ppu.bg_current_transfer_kind,
                ppu.visible_pixels_output,
                ppu.bg_fifo_pixels.len(),
                ppu.bg_startup_fifo_placeholders,
                ppu.selected_sprites.len(),
                ppu.obj_fetcher_stage,
                ppu.obj_pending_hit_len,
                ppu.obj_pending_hit_match_x
            );
            return;
        }

        previous_ppu = machine.ppu().snapshot();
    }

    panic!("case11 first-read snapshot probe did not terminate");
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 12 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case12_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 12
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case12_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case12_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case12_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        let pc = machine.cpu().registers().pc;
        if (0x484D..0x4871).contains(&pc) {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic irq timing after STAT arm for testcase 12 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case12_irq_after_stat_arm() {
    let irq = sample_real_mooneye_intr_2_mode0_timing_sprites_irq_after_stat_arm_for_testcase(12);
    println!("case12_irq={irq:?}");
}

#[test]
#[ignore = "diagnostic opcode fetch sample after testcase 12 irq of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case12_opcode_fetches_after_irq() {
    let fetches =
        sample_real_mooneye_intr_2_mode0_timing_sprites_opcode_fetches_after_irq_for_testcase(
            12, 16,
        );
    for fetch in fetches {
        println!("case12_fetch={fetch:?}");
    }
}

#[test]
#[ignore = "diagnostic MMIO sample after testcase 12 irq of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case12_mmio_after_irq() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut armed = false;
    let mut saw_irq = false;
    let mut events = Vec::new();

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if !armed
            && testcase_index == 12
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            armed = true;
        }

        if armed
            && !saw_irq
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

        if saw_irq
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && matches!(activity.address, 0xFE00..=0xFE27 | 0xFF40..=0xFF44)
        {
            let ppu = machine.ppu().snapshot();
            events.push(Intr2Mode0TimingSpritesAccessObservation {
                testcase_index,
                kind: activity.kind,
                address: activity.address,
                value: activity.value,
                pc: cpu_snapshot.registers.pc,
                ly: ppu.ly,
                line_dot: ppu.line_dot,
                mode: ppu.mode,
                mode0_start_dot: ppu.mode0_start_dot,
                selected_sprites_len: ppu.selected_sprites.len(),
                visible_pixels_output: ppu.visible_pixels_output,
            });

            if events.len() == 32 {
                break;
            }
        }

        if saw_irq && testcase_index > 12 {
            break;
        }
    }

    for event in events {
        println!("case12_mmio={event:?}");
    }
}

#[test]
#[ignore = "diagnostic STAT read sample after testcase 12 irq of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case12_stat_reads_after_irq() {
    let reads =
        sample_real_mooneye_intr_2_mode0_timing_sprites_stat_reads_after_irq_for_testcase(12, 8);
    for read in reads {
        println!("case12_read={read:?}");
    }
}

#[test]
#[ignore = "diagnostic line68 state changes for testcase 12 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case12_line68_changes() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(12, 68);
    for observation in observations {
        println!("case12_line68={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic irq timing after STAT arm for testcase 19 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case19_irq_after_stat_arm() {
    let irq = sample_real_mooneye_intr_2_mode0_timing_sprites_irq_after_stat_arm_for_testcase(19);
    println!("case19_irq={irq:?}");
}

#[test]
#[ignore = "diagnostic MMIO sample after testcase 19 irq of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case19_mmio_after_irq() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut armed = false;
    let mut saw_irq = false;
    let mut events = Vec::new();

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if !armed
            && testcase_index == 19
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            armed = true;
        }

        if armed
            && !saw_irq
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

        if saw_irq
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && matches!(activity.address, 0xFE00..=0xFE27 | 0xFF40..=0xFF44)
        {
            let ppu = machine.ppu().snapshot();
            events.push(Intr2Mode0TimingSpritesAccessObservation {
                testcase_index,
                kind: activity.kind,
                address: activity.address,
                value: activity.value,
                pc: cpu_snapshot.registers.pc,
                ly: ppu.ly,
                line_dot: ppu.line_dot,
                mode: ppu.mode,
                mode0_start_dot: ppu.mode0_start_dot,
                selected_sprites_len: ppu.selected_sprites.len(),
                visible_pixels_output: ppu.visible_pixels_output,
            });

            if events.len() == 32 {
                break;
            }
        }

        if saw_irq && testcase_index > 19 {
            break;
        }
    }

    for event in events {
        println!("case19_mmio={event:?}");
    }
}

#[test]
#[ignore = "diagnostic line68 state changes for testcase 19 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case19_line68_changes() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(19, 68);
    for observation in observations {
        println!("case19_line68={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 13 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case13_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 13
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case13_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case13_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case13_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        let pc = machine.cpu().registers().pc;
        if (0x484D..0x4871).contains(&pc) {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 8 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case8_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 8
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case8_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case8_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case8_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        let pc = machine.cpu().registers().pc;
        if (0x484D..0x4871).contains(&pc) {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic line68 state changes for testcase 13 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case13_line68_changes() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(13, 68);
    for observation in observations {
        println!("case13_line68={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic STAT read sample after testcase 13 irq of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case13_stat_reads_after_irq() {
    let reads =
        sample_real_mooneye_intr_2_mode0_timing_sprites_stat_reads_after_irq_for_testcase(13, 4);
    println!("case13_reads={reads:?}");
}

#[test]
#[ignore = "diagnostic STAT read sample after testcase 8 irq of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case8_stat_reads_after_irq() {
    let reads =
        sample_real_mooneye_intr_2_mode0_timing_sprites_stat_reads_after_irq_for_testcase(8, 4);
    println!("case8_reads={reads:?}");
}

#[test]
#[ignore = "diagnostic FF41 access window for testcase 8 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case8_ff41_window() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut in_case8 = false;

    for _ in 0..20_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if testcase_index == 8 && !in_case8 {
            in_case8 = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case8_enter pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                cpu_snapshot.registers.pc, ppu.ly, ppu.line_dot, ppu.mode, ppu.mode0_start_dot
            );
        } else if in_case8 && testcase_index > 8 {
            let ppu = machine.ppu().snapshot();
            println!(
                "case8_exit testcase_index={} pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                testcase_index,
                cpu_snapshot.registers.pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
            return;
        }

        if !in_case8 {
            continue;
        }

        if let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.address == 0xFF41
            && matches!(
                activity.kind,
                CpuBusAccessKind::DataRead | CpuBusAccessKind::DataWrite
            )
        {
            let ppu = machine.ppu().snapshot();
            println!(
                "case8_ff41_access kind={:?} value={:#04X} pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} current_transfer_x={} visible_pixels_output={} lane={:?} source_window={:?} backing={:?} readiness={:?} startup_fifo_placeholders={} selected_sprites_len={}",
                activity.kind,
                activity.value,
                cpu_snapshot.registers.pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_current_transfer_x,
                ppu.visible_pixels_output,
                ppu.bg_current_transfer_lane,
                ppu.bg_current_transfer_source_window,
                ppu.bg_current_transfer_backing,
                ppu.bg_current_transfer_readiness,
                ppu.bg_startup_fifo_placeholders,
                ppu.selected_sprites.len(),
            );
        }

        let pc = cpu_snapshot.registers.pc;
        if (0x484D..0x4871).contains(&pc) {
            let ppu = machine.ppu().snapshot();
            println!(
                "case8_fail pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} visible_pixels_output={} selected_sprites_len={}",
                pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.visible_pixels_output,
                ppu.selected_sprites.len(),
            );
            return;
        }
    }

    panic!("probe did not terminate inside testcase 8");
}

#[test]
#[ignore = "diagnostic round arm/irq/read sequence for testcase 14 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case14_rounds_and_reads() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 14
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case14_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case14_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case14_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        let pc = machine.cpu().registers().pc;
        if (0x484D..0x4871).contains(&pc) {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic line68 state changes for testcase 14 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case14_line68_changes() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(14, 68);
    for observation in observations {
        println!("case14_line68={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic STAT read sample after testcase 14 irq of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case14_stat_reads_after_irq() {
    let reads =
        sample_real_mooneye_intr_2_mode0_timing_sprites_stat_reads_after_irq_for_testcase(14, 4);
    println!("case14_reads={reads:?}");
}

#[test]
#[ignore = "diagnostic STAT read sample after testcase 19 irq of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case19_stat_reads_after_irq() {
    let reads =
        sample_real_mooneye_intr_2_mode0_timing_sprites_stat_reads_after_irq_for_testcase(19, 4);
    println!("case19_reads={reads:?}");
}

#[test]
#[ignore = "diagnostic first STAT read snapshot for testcase 19 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case19_first_read_snapshot() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut armed = false;
    let mut saw_irq = false;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..15_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if !armed
            && testcase_index == 19
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            armed = true;
        }

        if armed
            && !saw_irq
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

        if saw_irq
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            let ppu = machine.ppu().snapshot();
            println!(
                "case19_first_read_snapshot value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={} blank_frame_active={} bg_current_transfer_x={} bg_current_transfer_lane={:?} bg_current_transfer_source_window={:?} bg_current_transfer_backing={:?} bg_current_transfer_readiness={:?} bg_current_transfer_kind={:?} visible_pixels_output={} bg_fifo_len={} bg_startup_fifo_placeholders={} selected_sprites_len={} obj_stage={:?} obj_pending_hit_len={} obj_pending_hit_match_x={:?}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.blank_frame_active,
                ppu.bg_current_transfer_x,
                ppu.bg_current_transfer_lane,
                ppu.bg_current_transfer_source_window,
                ppu.bg_current_transfer_backing,
                ppu.bg_current_transfer_readiness,
                ppu.bg_current_transfer_kind,
                ppu.visible_pixels_output,
                ppu.bg_fifo_pixels.len(),
                ppu.bg_startup_fifo_placeholders,
                ppu.selected_sprites.len(),
                ppu.obj_fetcher_stage,
                ppu.obj_pending_hit_len,
                ppu.obj_pending_hit_match_x
            );
            return;
        }

        previous_ppu = machine.ppu().snapshot();
    }

    panic!("case19 first-read snapshot probe did not terminate");
}

#[test]
#[ignore = "diagnostic irq delivery after arming testcase 19 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case19_irq_delivery() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut armed = false;

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        let ppu = machine.ppu().snapshot();

        if !armed
            && testcase_index == 19
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            armed = true;
            println!(
                "case19_armed ly={} line_dot={} mode={:?} mode0_start_dot={} current_transfer_x={} obj_stage={:?} obj_stage_dot={} obj_pending_hit_len={}",
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_current_transfer_x,
                ppu.obj_fetcher_stage,
                ppu.obj_fetcher_stage_dot,
                ppu.obj_pending_hit_len
            );
        }

        if armed
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            println!(
                "case19_irq ly={} line_dot={} mode={:?} mode0_start_dot={} pc={:#06X}",
                ppu.ly, ppu.line_dot, ppu.mode, ppu.mode0_start_dot, cpu_snapshot.registers.pc
            );
            return;
        }

        if armed && machine.cpu().registers().pc == 0x4870 {
            println!(
                "case19_failed_without_irq ly={} line_dot={} mode={:?} mode0_start_dot={} stat={:#04X}",
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                machine.read_bus(0xFF41)
            );
            return;
        }
    }

    panic!("case19 irq-delivery probe did not terminate");
}

#[test]
#[ignore = "diagnostic real STAT reads after IRQ for testcase 19 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case19_reads_after_irq() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut armed = false;
    let mut saw_irq = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        let ppu = machine.ppu().snapshot();

        if !armed
            && testcase_index == 19
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            armed = true;
            println!(
                "case19_reads_armed ly={} line_dot={} mode={:?} mode0_start_dot={}",
                ppu.ly, ppu.line_dot, ppu.mode, ppu.mode0_start_dot
            );
        }

        if armed
            && !saw_irq
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq = true;
            println!(
                "case19_reads_irq ly={} line_dot={} mode={:?} mode0_start_dot={}",
                ppu.ly, ppu.line_dot, ppu.mode, ppu.mode0_start_dot
            );
        }

        if saw_irq
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            println!(
                "case19_reads_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
            if read_count == 4 {
                return;
            }
        }

        previous_ppu = ppu.clone();

        if saw_irq && machine.cpu().registers().pc == 0x4870 {
            println!(
                "case19_reads_failed pc=0x4870 ly={} line_dot={} mode={:?} mode0_start_dot={} stat={:#04X}",
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                machine.read_bus(0xFF41)
            );
            return;
        }
    }

    panic!("case19 read-after-irq probe did not terminate");
}

#[test]
#[ignore = "diagnostic line68 window 240..330 for testcase 14 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case14_line68_window_240_330() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(14, 68);
    for observation in observations {
        if (240..=330).contains(&observation.line_dot) {
            println!("case14_line68_window={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic line66 window 40..140 for testcase 14 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case14_line66_window_40_140() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(14, 66);
    for observation in observations {
        if (40..=140).contains(&observation.line_dot) {
            println!("case14_line66_window={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic STAT-arm progression before the current full-ROM failure of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_stat_arm_progression_to_current_failure() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut last_arm = None;

    for _ in 0..5_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
            && last_arm != Some(testcase_index)
        {
            let ppu = machine.ppu().snapshot();
            println!(
                "arm testcase_index={} pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                testcase_index,
                cpu_snapshot.registers.pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
            last_arm = Some(testcase_index);
        }

        let registers = machine.cpu().registers();
        if registers.pc == 0x486E {
            let ppu = machine.ppu().snapshot();
            println!(
                "failure testcase_index={} b={} c={} d={} e={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                testcase_index,
                registers.b,
                registers.c,
                registers.d,
                registers.e,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
            return;
        }
    }

    panic!("probe did not reach the current full-ROM failure signature");
}

#[test]
#[ignore = "diagnostic tail MMIO/OAM accesses before the run_rom_suite failure signature of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_tail_before_runner_failure_signature() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut tail_events = Vec::with_capacity(64);

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if let Some(activity) = cpu_snapshot.last_bus_activity
            && matches!(activity.address, 0xFE00..=0xFE27 | 0xFF40..=0xFF43)
        {
            let ppu = machine.ppu().snapshot();
            let observation = Intr2Mode0TimingSpritesAccessObservation {
                testcase_index,
                kind: activity.kind,
                address: activity.address,
                value: activity.value,
                pc: cpu_snapshot.registers.pc,
                ly: ppu.ly,
                line_dot: ppu.line_dot,
                mode: ppu.mode,
                mode0_start_dot: ppu.mode0_start_dot,
                selected_sprites_len: ppu.selected_sprites.len(),
                visible_pixels_output: ppu.visible_pixels_output,
            };
            if tail_events.len() == 64 {
                tail_events.remove(0);
            }
            tail_events.push(observation);
        }

        let ppu = machine.ppu().snapshot();
        let registers = machine.cpu().registers();
        if registers.pc == 0x486F
            && ppu.ly == 41
            && ppu.line_dot == 48
            && ppu.mode == PpuAccessMode::OamScan
        {
            println!(
                "runner_failure testcase_index={} b={} c={} d={} e={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                testcase_index,
                registers.b,
                registers.c,
                registers.d,
                registers.e,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
            for access in tail_events {
                println!("runner_tail={access:?}");
            }
            return;
        }
    }

    panic!("probe did not reach the run_rom_suite failure signature");
}

#[test]
#[ignore = "diagnostic FF40/FF41/FF44 progression after the testcase 1 second restart of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case1_second_restart_progression() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut second_restart_seen = false;
    let mut saw_ff40_11 = false;
    let mut last_ff44_read_value = None;

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if testcase_index != 1 {
            continue;
        }

        if let Some(activity) = cpu_snapshot.last_bus_activity {
            if activity.kind == CpuBusAccessKind::DataWrite
                && activity.address == 0xFF40
                && activity.value == 0x11
            {
                saw_ff40_11 = true;
            }

            if saw_ff40_11
                && activity.kind == CpuBusAccessKind::DataWrite
                && activity.address == 0xFF40
                && activity.value == 0x91
            {
                second_restart_seen = true;
                let ppu = machine.ppu().snapshot();
                println!(
                    "case1_second_restart_seen pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                    cpu_snapshot.registers.pc, ppu.ly, ppu.line_dot, ppu.mode, ppu.mode0_start_dot
                );
            }

            if second_restart_seen && matches!(activity.address, 0xFF40 | 0xFF41 | 0xFF44) {
                let is_ff44_poll =
                    activity.kind == CpuBusAccessKind::DataRead && activity.address == 0xFF44;
                if !is_ff44_poll || last_ff44_read_value != Some(activity.value) {
                    if is_ff44_poll {
                        last_ff44_read_value = Some(activity.value);
                    }
                    let ppu = machine.ppu().snapshot();
                    println!(
                        "case1_second_restart_access kind={:?} address={:#06X} value={:#04X} pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                        activity.kind,
                        activity.address,
                        activity.value,
                        cpu_snapshot.registers.pc,
                        ppu.ly,
                        ppu.line_dot,
                        ppu.mode,
                        ppu.mode0_start_dot
                    );
                }

                if activity.kind == CpuBusAccessKind::DataWrite
                    && activity.address == 0xFF41
                    && activity.value == 0x20
                {
                    return;
                }
            }
        }

        let ppu = machine.ppu().snapshot();
        let registers = machine.cpu().registers();
        if second_restart_seen
            && registers.pc == 0x486F
            && ppu.ly == 41
            && ppu.line_dot == 48
            && ppu.mode == PpuAccessMode::OamScan
        {
            println!(
                "case1_second_restart_failure testcase_index={} b={} c={} d={} e={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                testcase_index,
                registers.b,
                registers.c,
                registers.d,
                registers.e,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
            return;
        }
    }

    panic!("probe did not reach the testcase 1 second restart progression");
}

#[test]
#[ignore = "diagnostic D-register transitions after the testcase 1 second restart of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case1_second_restart_d_transitions() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut second_restart_seen = false;
    let mut saw_ff40_11 = false;
    let mut previous_d = None;

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if testcase_index != 1 {
            continue;
        }

        if let Some(activity) = cpu_snapshot.last_bus_activity {
            if activity.kind == CpuBusAccessKind::DataWrite
                && activity.address == 0xFF40
                && activity.value == 0x11
            {
                saw_ff40_11 = true;
            }

            if saw_ff40_11
                && activity.kind == CpuBusAccessKind::DataWrite
                && activity.address == 0xFF40
                && activity.value == 0x91
            {
                second_restart_seen = true;
                previous_d = Some(cpu_snapshot.registers.d);
                let ppu = machine.ppu().snapshot();
                println!(
                    "case1_second_restart_d_start d={:#04X} pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                    cpu_snapshot.registers.d,
                    cpu_snapshot.registers.pc,
                    ppu.ly,
                    ppu.line_dot,
                    ppu.mode,
                    ppu.mode0_start_dot
                );
            }
        }

        if second_restart_seen {
            let d = cpu_snapshot.registers.d;
            if previous_d != Some(d) {
                let ppu = machine.ppu().snapshot();
                println!(
                    "case1_second_restart_d_change from={:#04X} to={:#04X} pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                    previous_d.unwrap_or(0),
                    d,
                    cpu_snapshot.registers.pc,
                    ppu.ly,
                    ppu.line_dot,
                    ppu.mode,
                    ppu.mode0_start_dot
                );
                previous_d = Some(d);
            }
        }

        let ppu = machine.ppu().snapshot();
        let registers = machine.cpu().registers();
        if second_restart_seen
            && registers.pc == 0x486F
            && ppu.ly == 41
            && ppu.line_dot == 48
            && ppu.mode == PpuAccessMode::OamScan
        {
            println!(
                "case1_second_restart_d_failure d={:#04X} b={:#04X} c={:#04X} e={:#04X} pc={:#06X} ly={} line_dot={} mode={:?}",
                registers.d,
                registers.b,
                registers.c,
                registers.e,
                registers.pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode
            );
            return;
        }
    }

    panic!("probe did not reach the testcase 1 second restart D-transition window");
}

#[test]
#[ignore = "diagnostic first D-register transition during testcase 1 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case1_first_d_transition() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut in_case1 = false;
    let mut previous_d = None;

    for _ in 0..20_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if testcase_index == 1 {
            in_case1 = true;
        } else if in_case1 && testcase_index > 1 {
            break;
        }

        if !in_case1 {
            continue;
        }

        let d = cpu_snapshot.registers.d;
        if let Some(previous_d) = previous_d
            && previous_d != d
        {
            let ppu = machine.ppu().snapshot();
            println!(
                "case1_d_change from={:#04X} to={:#04X} pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} last_bus_activity={:?}",
                previous_d,
                d,
                cpu_snapshot.registers.pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                cpu_snapshot.last_bus_activity
            );
            return;
        }

        previous_d = Some(d);
    }

    panic!("probe did not observe a D transition during testcase 1");
}

#[test]
#[ignore = "diagnostic first transition of D to 0x42 during testcase 1 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case1_transition_to_fail_marker() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut in_case1 = false;
    let mut previous_d = None;

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if testcase_index == 1 {
            in_case1 = true;
        } else if in_case1 && testcase_index > 1 {
            break;
        }

        if !in_case1 {
            continue;
        }

        let d = cpu_snapshot.registers.d;
        if previous_d != Some(d) && d == 0x42 {
            let ppu = machine.ppu().snapshot();
            println!(
                "case1_d_to_fail_marker from={:#04X} to={:#04X} pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} last_bus_activity={:?}",
                previous_d.unwrap_or(0),
                d,
                cpu_snapshot.registers.pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                cpu_snapshot.last_bus_activity
            );
            return;
        }

        previous_d = Some(d);
    }

    panic!("probe did not observe D transitioning to 0x42 during testcase 1");
}

#[test]
#[ignore = "diagnostic setup MMIO/OAM access window for testcase 14 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case14_setup_access_window() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if testcase_index == 14
            && (65..=67).contains(&ppu.ly)
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && (matches!(activity.address, 0xFF40 | 0xFF41 | 0xFF43 | 0xFF44)
                || (0xFE00..=0xFE27).contains(&activity.address))
        {
            println!(
                "case14_setup_access kind={:?} address={:#06X} value={:#04X} pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} current_transfer_x={} lane={:?} source_window={:?} backing={:?} readiness={:?} startup_fifo_placeholders={} obj_stage={:?} obj_stage_dot={} obj_pending_hit_match_x={:?} obj_pending_hit_len={} obj_pending_hit_front_sprite_slot={:?} selected_sprites_len={}",
                activity.kind,
                activity.address,
                activity.value,
                cpu_snapshot.registers.pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_current_transfer_x,
                ppu.bg_current_transfer_lane,
                ppu.bg_current_transfer_source_window,
                ppu.bg_current_transfer_backing,
                ppu.bg_current_transfer_readiness,
                ppu.bg_startup_fifo_placeholders,
                ppu.obj_fetcher_stage,
                ppu.obj_fetcher_stage_dot,
                ppu.obj_pending_hit_match_x,
                ppu.obj_pending_hit_len,
                ppu.obj_pending_hit_front_sprite_slot,
                ppu.selected_sprites.len()
            );
        }
    }
}

#[test]
#[ignore = "diagnostic setup writes for testcase 19 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case19_setup_writes() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut in_case19 = false;

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if testcase_index == 19 {
            in_case19 = true;
        }

        if in_case19
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && (matches!(activity.address, 0xFF40 | 0xFF41 | 0xFF43)
                || (0xFE00..=0xFE27).contains(&activity.address))
        {
            println!(
                "case19_setup_write address={:#06X} value={:#04X} pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} startup_fifo_placeholders={} current_transfer_x={} lane={:?} obj_stage={:?} obj_stage_dot={} obj_pending_hit_len={} selected_sprites_len={}",
                activity.address,
                activity.value,
                cpu_snapshot.registers.pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_startup_fifo_placeholders,
                ppu.bg_current_transfer_x,
                ppu.bg_current_transfer_lane,
                ppu.obj_fetcher_stage,
                ppu.obj_fetcher_stage_dot,
                ppu.obj_pending_hit_len,
                ppu.selected_sprites.len(),
            );

            if activity.address == 0xFF41 && activity.value == 0x20 {
                break;
            }
        }

        if in_case19 && testcase_index > 19 {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic setup MMIO/OAM access window for testcase 27 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case27_setup_access_window() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if testcase_index == 27
            && (65..=67).contains(&ppu.ly)
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && (matches!(activity.address, 0xFF40 | 0xFF41 | 0xFF43 | 0xFF44)
                || (0xFE00..=0xFE27).contains(&activity.address))
        {
            println!(
                "case27_setup_access kind={:?} address={:#06X} value={:#04X} pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} current_transfer_x={} lane={:?} source_window={:?} backing={:?} readiness={:?} startup_fifo_placeholders={} obj_stage={:?} obj_stage_dot={} obj_pending_hit_match_x={:?} obj_pending_hit_len={} obj_pending_hit_front_sprite_slot={:?} selected_sprites_len={}",
                activity.kind,
                activity.address,
                activity.value,
                cpu_snapshot.registers.pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_current_transfer_x,
                ppu.bg_current_transfer_lane,
                ppu.bg_current_transfer_source_window,
                ppu.bg_current_transfer_backing,
                ppu.bg_current_transfer_readiness,
                ppu.bg_startup_fifo_placeholders,
                ppu.obj_fetcher_stage,
                ppu.obj_fetcher_stage_dot,
                ppu.obj_pending_hit_match_x,
                ppu.obj_pending_hit_len,
                ppu.obj_pending_hit_front_sprite_slot,
                ppu.selected_sprites.len(),
            );
        }

        if testcase_index > 27 {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic setup MMIO/OAM access window for testcase 25 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case25_setup_access_window() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if testcase_index == 25
            && (65..=67).contains(&ppu.ly)
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && (matches!(activity.address, 0xFF40 | 0xFF41 | 0xFF43 | 0xFF44)
                || (0xFE00..=0xFE27).contains(&activity.address))
        {
            println!(
                "case25_setup_access kind={:?} address={:#06X} value={:#04X} pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} current_transfer_x={} lane={:?} source_window={:?} backing={:?} readiness={:?} startup_fifo_placeholders={} obj_stage={:?} obj_stage_dot={} obj_pending_hit_match_x={:?} obj_pending_hit_len={} obj_pending_hit_front_sprite_slot={:?} selected_sprites_len={}",
                activity.kind,
                activity.address,
                activity.value,
                cpu_snapshot.registers.pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_current_transfer_x,
                ppu.bg_current_transfer_lane,
                ppu.bg_current_transfer_source_window,
                ppu.bg_current_transfer_backing,
                ppu.bg_current_transfer_readiness,
                ppu.bg_startup_fifo_placeholders,
                ppu.obj_fetcher_stage,
                ppu.obj_fetcher_stage_dot,
                ppu.obj_pending_hit_match_x,
                ppu.obj_pending_hit_len,
                ppu.obj_pending_hit_front_sprite_slot,
                ppu.selected_sprites.len(),
            );
        }

        if testcase_index > 25 {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic setup writes for testcase 27 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case27_setup_writes() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut in_case27 = false;

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if testcase_index == 27 {
            in_case27 = true;
        }

        if in_case27
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && (matches!(activity.address, 0xFF40 | 0xFF41 | 0xFF43)
                || (0xFE00..=0xFE27).contains(&activity.address))
        {
            println!(
                "case27_setup_write address={:#06X} value={:#04X} pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} startup_fifo_placeholders={} current_transfer_x={} lane={:?} obj_stage={:?} obj_stage_dot={} obj_pending_hit_len={} selected_sprites_len={}",
                activity.address,
                activity.value,
                cpu_snapshot.registers.pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_startup_fifo_placeholders,
                ppu.bg_current_transfer_x,
                ppu.bg_current_transfer_lane,
                ppu.obj_fetcher_stage,
                ppu.obj_fetcher_stage_dot,
                ppu.obj_pending_hit_len,
                ppu.selected_sprites.len(),
            );

            if activity.address == 0xFF41 && activity.value == 0x20 {
                break;
            }
        }

        if in_case27 && testcase_index > 27 {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic setup writes for testcase 29 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case29_setup_writes() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut in_case29 = false;

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if testcase_index == 29 {
            in_case29 = true;
        }

        if in_case29
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && (matches!(activity.address, 0xFF40 | 0xFF41 | 0xFF43)
                || (0xFE00..=0xFE27).contains(&activity.address))
        {
            println!(
                "case29_setup_write address={:#06X} value={:#04X} pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} startup_fifo_placeholders={} current_transfer_x={} lane={:?} obj_stage={:?} obj_stage_dot={} obj_pending_hit_len={} selected_sprites_len={}",
                activity.address,
                activity.value,
                cpu_snapshot.registers.pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_startup_fifo_placeholders,
                ppu.bg_current_transfer_x,
                ppu.bg_current_transfer_lane,
                ppu.obj_fetcher_stage,
                ppu.obj_fetcher_stage_dot,
                ppu.obj_pending_hit_len,
                ppu.selected_sprites.len(),
            );

            if activity.address == 0xFF41 && activity.value == 0x20 {
                break;
            }
        }

        if in_case29 && testcase_index > 29 {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic helper entry context for testcase 29 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case29_helper_entry_context() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let cpu = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if testcase_index == 29 && cpu.registers.pc == 0x0B5B {
            let sp = cpu.registers.sp;
            let return_address =
                u16::from_le_bytes([machine.read_bus(sp), machine.read_bus(sp.wrapping_add(1))]);
            let ppu = machine.ppu().snapshot();
            println!(
                "case29_helper_entry sp={:#06X} ret={:#06X} hl={:#06X} de={:#06X} bc={:#06X} ly={} line_dot={} mode={:?}",
                sp,
                return_address,
                u16::from_be_bytes([cpu.registers.h, cpu.registers.l]),
                u16::from_be_bytes([cpu.registers.d, cpu.registers.e]),
                u16::from_be_bytes([cpu.registers.b, cpu.registers.c]),
                ppu.ly,
                ppu.line_dot,
                ppu.mode
            );
            return;
        }
    }

    panic!("case29 helper entry probe did not reach pc=0x0B5B");
}

#[test]
#[ignore = "diagnostic rom-path probe for testcase 29 of mooneye intr_2_mode0_timing_sprites"]
fn intr_2_mode0_timing_sprites_case29_rom_path_probe_logs_outcome() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_intr_2_mode0_sprites_case29_rom_path_probe_rom())
        .expect("probe ROM should load");

    for _ in 0..5_000_000 {
        machine.step_t_cycle();

        let cpu = machine.cpu().snapshot();
        if machine.read_bus(0xC200) == 0x01 {
            let ppu = machine.ppu().snapshot();
            println!(
                "case29_rom_path_success b={} c={} d={} e={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                cpu.registers.b,
                cpu.registers.c,
                cpu.registers.d,
                cpu.registers.e,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
            return;
        }

        if cpu.registers.pc == 0x0C06 {
            let ppu = machine.ppu().snapshot();
            println!(
                "case29_rom_path_helper_fail b={} c={} d={} e={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                cpu.registers.b,
                cpu.registers.c,
                cpu.registers.d,
                cpu.registers.e,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
            return;
        }

        if cpu.registers.pc == 0x486E {
            let ppu = machine.ppu().snapshot();
            println!(
                "case29_rom_path_failure b={} c={} d={} e={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                cpu.registers.b,
                cpu.registers.c,
                cpu.registers.d,
                cpu.registers.e,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
            return;
        }
    }

    panic!("case29 rom-path probe did not terminate");
}

#[test]
#[ignore = "diagnostic rom-path probe for testcase 28 then 29 of mooneye intr_2_mode0_timing_sprites"]
fn intr_2_mode0_timing_sprites_case28_case29_rom_path_probe_logs_outcome() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_intr_2_mode0_timing_sprites_case28_case29_rom_path_probe_rom())
        .expect("probe ROM should load");

    for _ in 0..8_000_000 {
        machine.step_t_cycle();

        let cpu = machine.cpu().snapshot();
        if machine.read_bus(0xC201) == 0x01 {
            let ppu = machine.ppu().snapshot();
            println!(
                "case28_case29_rom_path_success testcase_index={} b={} c={} d={} e={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                machine.read_bus(0xFF80),
                cpu.registers.b,
                cpu.registers.c,
                cpu.registers.d,
                cpu.registers.e,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
            return;
        }

        if cpu.registers.pc == 0x0C06 {
            let ppu = machine.ppu().snapshot();
            println!(
                "case28_case29_rom_path_helper_fail testcase_index={} b={} c={} d={} e={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                machine.read_bus(0xFF80),
                cpu.registers.b,
                cpu.registers.c,
                cpu.registers.d,
                cpu.registers.e,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
            return;
        }

        if cpu.registers.pc == 0x486E {
            let ppu = machine.ppu().snapshot();
            println!(
                "case28_case29_rom_path_failure testcase_index={} b={} c={} d={} e={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                machine.read_bus(0xFF80),
                cpu.registers.b,
                cpu.registers.c,
                cpu.registers.d,
                cpu.registers.e,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
            return;
        }
    }

    panic!("case28->29 rom-path probe did not terminate");
}

#[test]
#[ignore = "diagnostic probe that runs the real testcase-29 caller path at its original address"]
fn intr_2_mode0_timing_sprites_case29_real_caller_probe_logs_outcome() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_intr_2_mode0_timing_sprites_case29_real_caller_probe_rom())
        .expect("probe ROM should load");

    for _ in 0..5_000_000 {
        machine.step_t_cycle();

        let cpu = machine.cpu().snapshot();
        if machine.read_bus(0xFF80) == 0x1E {
            let ppu = machine.ppu().snapshot();
            println!(
                "case29_real_caller_success b={} c={} d={} e={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                cpu.registers.b,
                cpu.registers.c,
                cpu.registers.d,
                cpu.registers.e,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
            return;
        }

        if cpu.registers.pc == 0x0C06 {
            let ppu = machine.ppu().snapshot();
            println!(
                "case29_real_caller_helper_fail b={} c={} d={} e={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                cpu.registers.b,
                cpu.registers.c,
                cpu.registers.d,
                cpu.registers.e,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
            return;
        }
    }

    panic!("case29 real-caller probe did not terminate");
}

#[test]
#[ignore = "diagnostic line68 window 240..330 for testcase 13 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case13_line68_window_240_330() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(13, 68);
    for observation in observations {
        if (240..=330).contains(&observation.line_dot) {
            println!("case13_line68_window={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic line66 window 40..140 for testcase 13 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case13_line66_window_40_140() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(13, 66);
    for observation in observations {
        if (40..=140).contains(&observation.line_dot) {
            println!("case13_line66_window={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic setup MMIO/OAM access window for testcase 13 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case13_setup_access_window() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if testcase_index == 13
            && (65..=67).contains(&ppu.ly)
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && (matches!(activity.address, 0xFF40 | 0xFF41 | 0xFF43 | 0xFF44)
                || (0xFE00..=0xFE27).contains(&activity.address))
        {
            println!(
                "case13_setup_access kind={:?} address={:#06X} value={:#04X} pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} current_transfer_x={} lane={:?} source_window={:?} backing={:?} readiness={:?} startup_fifo_placeholders={} obj_stage={:?} obj_stage_dot={} obj_pending_hit_match_x={:?} obj_pending_hit_len={} obj_pending_hit_front_sprite_slot={:?} selected_sprites_len={}",
                activity.kind,
                activity.address,
                activity.value,
                cpu_snapshot.registers.pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_current_transfer_x,
                ppu.bg_current_transfer_lane,
                ppu.bg_current_transfer_source_window,
                ppu.bg_current_transfer_backing,
                ppu.bg_current_transfer_readiness,
                ppu.bg_startup_fifo_placeholders,
                ppu.obj_fetcher_stage,
                ppu.obj_fetcher_stage_dot,
                ppu.obj_pending_hit_match_x,
                ppu.obj_pending_hit_len,
                ppu.obj_pending_hit_front_sprite_slot,
                ppu.selected_sprites.len(),
            );
        }

        if testcase_index > 13 {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic setup writes for testcase 13 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case13_setup_writes() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut in_case13 = false;

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let ppu = machine.ppu().snapshot();
        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if testcase_index == 13 {
            in_case13 = true;
        }

        if in_case13
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && (matches!(activity.address, 0xFF40 | 0xFF41 | 0xFF43)
                || (0xFE00..=0xFE27).contains(&activity.address))
        {
            println!(
                "case13_setup_write address={:#06X} value={:#04X} pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} startup_fifo_placeholders={} current_transfer_x={} lane={:?} obj_stage={:?} obj_stage_dot={} obj_pending_hit_len={} selected_sprites_len={}",
                activity.address,
                activity.value,
                cpu_snapshot.registers.pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_startup_fifo_placeholders,
                ppu.bg_current_transfer_x,
                ppu.bg_current_transfer_lane,
                ppu.obj_fetcher_stage,
                ppu.obj_fetcher_stage_dot,
                ppu.obj_pending_hit_len,
                ppu.selected_sprites.len(),
            );

            if activity.address == 0xFF41 && activity.value == 0x20 {
                break;
            }
        }

        if in_case13 && testcase_index > 13 {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 15 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case15_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 15
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case15_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case15_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case15_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        let pc = machine.cpu().registers().pc;
        if (0x484D..0x4871).contains(&pc) {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 19 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case19_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 19
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case19_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case19_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case19_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        let pc = machine.cpu().registers().pc;
        if (0x484D..0x4871).contains(&pc) {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic line68 state changes for testcase 15 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case15_line68_changes() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(15, 68);
    for observation in observations {
        println!("case15_line68={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic line66 window 40..140 for testcase 19 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case19_line66_window_40_140() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(19, 66);
    for observation in observations {
        if (40..=140).contains(&observation.line_dot) {
            println!("case19_line66_window={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic line66 window 90..140 for testcase 19 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case19_line66_window_90_140() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(19, 66);
    for observation in observations {
        if (90..=140).contains(&observation.line_dot) {
            println!("case19_line66_focus={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic line67 window 118..140 for testcase 19 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case19_line67_window_118_140() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(19, 67);
    for observation in observations {
        if (118..=140).contains(&observation.line_dot) {
            println!("case19_line67_focus={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic irq timing after STAT arm for testcase 24 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case24_irq_after_stat_arm() {
    let irq = sample_real_mooneye_intr_2_mode0_timing_sprites_irq_after_stat_arm_for_testcase(24);
    println!("case24_irq={irq:?}");
}

#[test]
#[ignore = "diagnostic first STAT read snapshot for testcase 24 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case24_first_read_snapshot() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut armed = false;
    let mut saw_irq = false;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..15_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if !armed
            && testcase_index == 24
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            armed = true;
        }

        if armed
            && !saw_irq
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

        if saw_irq
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            let ppu = machine.ppu().snapshot();
            println!(
                "case24_first_read_snapshot value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={} blank_frame_active={} bg_current_transfer_x={} bg_current_transfer_lane={:?} bg_current_transfer_source_window={:?} bg_current_transfer_backing={:?} bg_current_transfer_readiness={:?} bg_current_transfer_kind={:?} visible_pixels_output={} bg_fifo_len={} bg_startup_fifo_placeholders={} selected_sprites_len={} obj_stage={:?} obj_pending_hit_len={} obj_pending_hit_match_x={:?}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.blank_frame_active,
                ppu.bg_current_transfer_x,
                ppu.bg_current_transfer_lane,
                ppu.bg_current_transfer_source_window,
                ppu.bg_current_transfer_backing,
                ppu.bg_current_transfer_readiness,
                ppu.bg_current_transfer_kind,
                ppu.visible_pixels_output,
                ppu.bg_fifo_pixels.len(),
                ppu.bg_startup_fifo_placeholders,
                ppu.selected_sprites.len(),
                ppu.obj_fetcher_stage,
                ppu.obj_pending_hit_len,
                ppu.obj_pending_hit_match_x
            );
            return;
        }

        previous_ppu = machine.ppu().snapshot();
    }

    panic!("case24 first-read snapshot probe did not terminate");
}

#[test]
#[ignore = "diagnostic irq timing after STAT arm for testcase 25 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case25_irq_after_stat_arm() {
    let irq = sample_real_mooneye_intr_2_mode0_timing_sprites_irq_after_stat_arm_for_testcase(25);
    println!("case25_irq={irq:?}");
}

#[test]
#[ignore = "diagnostic first STAT read snapshot for testcase 25 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case25_first_read_snapshot() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut armed = false;
    let mut saw_irq = false;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..15_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if !armed
            && testcase_index == 25
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            armed = true;
        }

        if armed
            && !saw_irq
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

        if saw_irq
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            let ppu = machine.ppu().snapshot();
            println!(
                "case25_first_read_snapshot value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={} blank_frame_active={} bg_current_transfer_x={} bg_current_transfer_lane={:?} bg_current_transfer_source_window={:?} bg_current_transfer_backing={:?} bg_current_transfer_readiness={:?} bg_current_transfer_kind={:?} visible_pixels_output={} bg_fifo_len={} bg_startup_fifo_placeholders={} selected_sprites_len={} obj_stage={:?} obj_pending_hit_len={} obj_pending_hit_match_x={:?}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.blank_frame_active,
                ppu.bg_current_transfer_x,
                ppu.bg_current_transfer_lane,
                ppu.bg_current_transfer_source_window,
                ppu.bg_current_transfer_backing,
                ppu.bg_current_transfer_readiness,
                ppu.bg_current_transfer_kind,
                ppu.visible_pixels_output,
                ppu.bg_fifo_pixels.len(),
                ppu.bg_startup_fifo_placeholders,
                ppu.selected_sprites.len(),
                ppu.obj_fetcher_stage,
                ppu.obj_pending_hit_len,
                ppu.obj_pending_hit_match_x
            );
            return;
        }

        previous_ppu = machine.ppu().snapshot();
    }

    panic!("case25 first-read snapshot probe did not terminate");
}

#[test]
#[ignore = "diagnostic narrow post-arm irq budget for testcase 25 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case25_irq_with_narrow_post_arm_budget() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut armed = false;

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if !armed
            && testcase_index == 25
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            let ppu = machine.ppu().snapshot();
            println!(
                "case25_narrow_arm pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} current_transfer_x={} obj_stage={:?} obj_stage_dot={} obj_pending_hit_len={} obj_pending_hit_front_sprite_slot={:?}",
                cpu_snapshot.registers.pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_current_transfer_x,
                ppu.obj_fetcher_stage,
                ppu.obj_fetcher_stage_dot,
                ppu.obj_pending_hit_len,
                ppu.obj_pending_hit_front_sprite_slot,
            );
            armed = true;
            break;
        }
    }

    assert!(armed, "case25 arm was not observed");

    for _ in 0..2_000 {
        machine.step_t_cycle();

        if matches!(
            machine.cpu().execution_state(),
            gb_core::CpuExecutionState::ServiceInterrupt {
                source: gb_core::InterruptSource::LcdStat,
                ..
            }
        ) {
            let ppu = machine.ppu().snapshot();
            println!(
                "case25_narrow_irq pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} current_transfer_x={} obj_stage={:?} obj_stage_dot={} obj_pending_hit_len={} obj_pending_hit_front_sprite_slot={:?}",
                machine.cpu().registers().pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_current_transfer_x,
                ppu.obj_fetcher_stage,
                ppu.obj_fetcher_stage_dot,
                ppu.obj_pending_hit_len,
                ppu.obj_pending_hit_front_sprite_slot,
            );
            return;
        }
    }

    let ppu = machine.ppu().snapshot();
    println!(
        "case25_narrow_no_irq pc={:#06X} state={:?} ly={} line_dot={} mode={:?} mode0_start_dot={} current_transfer_x={} obj_stage={:?} obj_stage_dot={} obj_pending_hit_len={} obj_pending_hit_front_sprite_slot={:?} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        ppu.ly,
        ppu.line_dot,
        ppu.mode,
        ppu.mode0_start_dot,
        ppu.bg_current_transfer_x,
        ppu.obj_fetcher_stage,
        ppu.obj_fetcher_stage_dot,
        ppu.obj_pending_hit_len,
        ppu.obj_pending_hit_front_sprite_slot,
        machine.read_bus(0xFF41),
    );
}

#[test]
#[ignore = "diagnostic narrow first FF41 reads after IRQ for testcase 25 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case25_first_stat_reads_after_irq() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut armed = false;

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if !armed
            && testcase_index == 25
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            armed = true;
            break;
        }
    }

    assert!(armed, "case25 arm was not observed");

    let mut saw_irq = false;
    let mut previous_ppu = machine.ppu().snapshot();
    let mut read_count = 0_u8;

    for _ in 0..4_000 {
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

        if saw_irq
            && let Some(activity) = machine.cpu().snapshot().last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case25_narrow_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={} current_transfer_x={} obj_stage={:?} obj_pending_hit_len={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_current_transfer_x,
                ppu.obj_fetcher_stage,
                ppu.obj_pending_hit_len,
            );

            if read_count >= 4 {
                return;
            }
        }

        previous_ppu = machine.ppu().snapshot();
    }

    println!(
        "case25_narrow_reads_timeout pc={:#06X} state={:?} ly={} line_dot={} mode={:?} mode0_start_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.ppu().snapshot().mode,
        machine.ppu().snapshot().mode0_start_dot,
        machine.read_bus(0xFF41),
    );
}

#[test]
#[ignore = "diagnostic narrow first FF41 reads after IRQ for testcase 17 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case17_first_stat_reads_after_irq() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut armed = false;

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if !armed
            && testcase_index == 17
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            armed = true;
            break;
        }
    }

    assert!(armed, "case17 arm was not observed");

    let mut saw_irq = false;
    let mut previous_ppu = machine.ppu().snapshot();
    let mut read_count = 0_u8;

    for _ in 0..4_000 {
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

        if saw_irq
            && let Some(activity) = machine.cpu().snapshot().last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case17_narrow_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={} current_transfer_x={} obj_stage={:?} obj_pending_hit_len={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_current_transfer_x,
                ppu.obj_fetcher_stage,
                ppu.obj_pending_hit_len,
            );

            if read_count >= 4 {
                return;
            }
        }

        previous_ppu = machine.ppu().snapshot();
    }

    println!(
        "case17_narrow_reads_timeout pc={:#06X} state={:?} ly={} line_dot={} mode={:?} mode0_start_dot={} stat={:#04X}",
        machine.cpu().registers().pc,
        machine.cpu().execution_state(),
        machine.ppu().snapshot().ly,
        machine.ppu().snapshot().line_dot,
        machine.ppu().snapshot().mode,
        machine.ppu().snapshot().mode0_start_dot,
        machine.read_bus(0xFF41),
    );
}

#[test]
#[ignore = "diagnostic short progression for testcase 17 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case17_short_progression() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut previous_ppu = machine.ppu().snapshot();
    let mut in_case17 = false;
    let mut seen_events = 0_u8;

    for _ in 0..12_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);
        if testcase_index == 17 {
            in_case17 = true;
        } else if in_case17 && testcase_index > 17 {
            println!("case17_progression_exit testcase_index={testcase_index}");
            return;
        }

        if !in_case17 {
            previous_ppu = machine.ppu().snapshot();
            continue;
        }

        if let Some(activity) = cpu_snapshot.last_bus_activity {
            if activity.kind == CpuBusAccessKind::DataWrite
                && activity.address == 0xFF41
                && activity.value == 0x20
            {
                let ppu = machine.ppu().snapshot();
                println!(
                    "case17_progression_arm pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} current_transfer_x={} obj_stage={:?} obj_pending_hit_len={}",
                    cpu_snapshot.registers.pc,
                    ppu.ly,
                    ppu.line_dot,
                    ppu.mode,
                    ppu.mode0_start_dot,
                    ppu.bg_current_transfer_x,
                    ppu.obj_fetcher_stage,
                    ppu.obj_pending_hit_len,
                );
                seen_events = seen_events.saturating_add(1);
            }

            if activity.kind == CpuBusAccessKind::DataRead && activity.address == 0xFF41 {
                let ppu = machine.ppu().snapshot();
                println!(
                    "case17_progression_read value={:#04X} pc={:#06X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={} current_transfer_x={} obj_stage={:?} obj_pending_hit_len={}",
                    activity.value,
                    cpu_snapshot.registers.pc,
                    previous_ppu.ly,
                    previous_ppu.line_dot,
                    previous_ppu.mode,
                    previous_ppu.mode0_start_dot,
                    ppu.ly,
                    ppu.line_dot,
                    ppu.mode,
                    ppu.mode0_start_dot,
                    ppu.bg_current_transfer_x,
                    ppu.obj_fetcher_stage,
                    ppu.obj_pending_hit_len,
                );
                seen_events = seen_events.saturating_add(1);
            }
        }

        if matches!(
            machine.cpu().execution_state(),
            gb_core::CpuExecutionState::ServiceInterrupt {
                source: gb_core::InterruptSource::LcdStat,
                ..
            }
        ) {
            let ppu = machine.ppu().snapshot();
            println!(
                "case17_progression_irq pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} current_transfer_x={} obj_stage={:?} obj_pending_hit_len={}",
                machine.cpu().registers().pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_current_transfer_x,
                ppu.obj_fetcher_stage,
                ppu.obj_pending_hit_len,
            );
            seen_events = seen_events.saturating_add(1);
        }

        let pc = machine.cpu().registers().pc;
        if in_case17 && (0x484D..0x4871).contains(&pc) {
            let ppu = machine.ppu().snapshot();
            println!(
                "case17_progression_fail pc={:#06X} ly={} line_dot={} mode={:?} mode0_start_dot={} current_transfer_x={} stat={:#04X}",
                pc,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.bg_current_transfer_x,
                machine.read_bus(0xFF41),
            );
            return;
        }

        if seen_events >= 12 {
            return;
        }

        previous_ppu = machine.ppu().snapshot();
    }

    panic!("case17 short progression did not terminate");
}

#[test]
#[ignore = "diagnostic MMIO sample after testcase 24 irq of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case24_mmio_after_irq() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut armed = false;
    let mut saw_irq = false;
    let mut events = Vec::new();

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if !armed
            && testcase_index == 24
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            armed = true;
        }

        if armed
            && !saw_irq
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

        if saw_irq
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && matches!(activity.address, 0xFE00..=0xFE27 | 0xFF40..=0xFF44)
        {
            let ppu = machine.ppu().snapshot();
            events.push(Intr2Mode0TimingSpritesAccessObservation {
                testcase_index,
                kind: activity.kind,
                address: activity.address,
                value: activity.value,
                pc: cpu_snapshot.registers.pc,
                ly: ppu.ly,
                line_dot: ppu.line_dot,
                mode: ppu.mode,
                mode0_start_dot: ppu.mode0_start_dot,
                selected_sprites_len: ppu.selected_sprites.len(),
                visible_pixels_output: ppu.visible_pixels_output,
            });

            if events.len() == 32 {
                break;
            }
        }

        if saw_irq && testcase_index > 24 {
            break;
        }
    }

    for event in events {
        println!("case24_mmio={event:?}");
    }
}

#[test]
#[ignore = "diagnostic line68 state changes for testcase 24 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case24_line68_changes() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(24, 68);
    for observation in observations {
        println!("case24_line68={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic line66 window 90..160 for testcase 25 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case25_line66_window_90_160() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(25, 66);
    for observation in observations {
        if (90..=160).contains(&observation.line_dot) {
            println!("case25_line66_focus={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic line68 window 300..320 for testcase 25 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case25_line68_window_300_320() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(25, 68);
    for observation in observations {
        if (300..=320).contains(&observation.line_dot) {
            println!("case25_line68_focus={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic line66 window 240..320 for testcase 29 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case29_line66_window_240_320() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(29, 66);
    for observation in observations {
        if (240..=320).contains(&observation.line_dot) {
            println!("case29_line66_window={observation:?}");
        }
    }
}

#[test]
#[ignore = "diagnostic first STAT read snapshot for testcase 29 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case29_first_read_snapshot() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut armed = false;
    let mut saw_irq = false;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..15_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if !armed
            && testcase_index == 29
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            armed = true;
        }

        if armed
            && !saw_irq
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

        if saw_irq
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            let ppu = machine.ppu().snapshot();
            println!(
                "case29_first_read_snapshot value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={} blank_frame_active={} bg_current_transfer_x={} bg_current_transfer_lane={:?} bg_current_transfer_source_window={:?} bg_current_transfer_backing={:?} bg_current_transfer_readiness={:?} bg_current_transfer_kind={:?} visible_pixels_output={} bg_fifo_len={} bg_startup_fifo_placeholders={} selected_sprites_len={} obj_stage={:?} obj_pending_hit_len={} obj_pending_hit_match_x={:?}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.blank_frame_active,
                ppu.bg_current_transfer_x,
                ppu.bg_current_transfer_lane,
                ppu.bg_current_transfer_source_window,
                ppu.bg_current_transfer_backing,
                ppu.bg_current_transfer_readiness,
                ppu.bg_current_transfer_kind,
                ppu.visible_pixels_output,
                ppu.bg_fifo_pixels.len(),
                ppu.bg_startup_fifo_placeholders,
                ppu.selected_sprites.len(),
                ppu.obj_fetcher_stage,
                ppu.obj_pending_hit_len,
                ppu.obj_pending_hit_match_x
            );
            return;
        }

        previous_ppu = machine.ppu().snapshot();
    }

    panic!("case29 first-read snapshot probe did not terminate");
}

#[test]
#[ignore = "diagnostic first two rounds for testcase 29 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case29_first_two_rounds() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..15_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 29
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case29_first_two_rounds_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case29_first_two_rounds_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case29_first_two_rounds_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );

            if arm_count >= 2 && read_count >= 2 {
                return;
            }
        }

        previous_ppu = machine.ppu().snapshot();
    }

    panic!("case29 first-two-rounds probe did not terminate");
}

#[test]
#[ignore = "diagnostic round2 reads for testcase 29 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case29_round2_reads() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..15_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 29
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case29_round2_probe_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count >= 2
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case29_round2_probe_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count >= 2
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case29_round2_probe_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );

            if read_count >= 4 {
                return;
            }
        }

        previous_ppu = machine.ppu().snapshot();
    }

    panic!("case29 round2 probe did not terminate");
}

#[test]
#[ignore = "diagnostic first round2 STAT read snapshot for testcase 29 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case29_round2_first_read_snapshot() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..15_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 29
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
        }

        if arm_count >= 2
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
        }

        if arm_count >= 2
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            let ppu = machine.ppu().snapshot();
            println!(
                "case29_round2_first_read_snapshot value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={} blank_frame_active={} bg_current_transfer_x={} bg_current_transfer_lane={:?} bg_current_transfer_source_window={:?} bg_current_transfer_backing={:?} bg_current_transfer_readiness={:?} bg_current_transfer_kind={:?} visible_pixels_output={} bg_fifo_len={} bg_startup_fifo_placeholders={} selected_sprites_len={} obj_stage={:?} obj_pending_hit_len={} obj_pending_hit_match_x={:?}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot,
                ppu.blank_frame_active,
                ppu.bg_current_transfer_x,
                ppu.bg_current_transfer_lane,
                ppu.bg_current_transfer_source_window,
                ppu.bg_current_transfer_backing,
                ppu.bg_current_transfer_readiness,
                ppu.bg_current_transfer_kind,
                ppu.visible_pixels_output,
                ppu.bg_fifo_pixels.len(),
                ppu.bg_startup_fifo_placeholders,
                ppu.selected_sprites.len(),
                ppu.obj_fetcher_stage,
                ppu.obj_pending_hit_len,
                ppu.obj_pending_hit_match_x,
            );
            return;
        }

        previous_ppu = machine.ppu().snapshot();
    }

    panic!("case29 round2 first-read snapshot probe did not terminate");
}

#[test]
#[ignore = "diagnostic real STAT reads after IRQ for testcase 25 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case25_reads_after_irq() {
    let reads =
        sample_real_mooneye_intr_2_mode0_timing_sprites_stat_reads_after_irq_for_testcase(25, 4);
    println!("case25_reads={reads:?}");
}

#[test]
#[ignore = "diagnostic real STAT reads after IRQ for testcase 29 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case29_reads_after_irq() {
    let reads =
        sample_real_mooneye_intr_2_mode0_timing_sprites_stat_reads_after_irq_for_testcase(29, 24);
    println!("case29_reads={reads:?}");
}

#[test]
#[ignore = "diagnostic real STAT reads after IRQ for testcase 27 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case27_reads_after_irq() {
    let reads =
        sample_real_mooneye_intr_2_mode0_timing_sprites_stat_reads_after_irq_for_testcase(27, 4);
    println!("case27_reads={reads:?}");
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 17 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case17_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 17
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case17_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case17_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case17_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        let pc = machine.cpu().registers().pc;
        if (0x484D..0x4871).contains(&pc) {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic line68 state changes for testcase 17 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case17_line68_changes() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(17, 68);
    for observation in observations {
        println!("case17_line68={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 24 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case24_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 24
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case24_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case24_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case24_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        let pc = machine.cpu().registers().pc;
        if (0x484D..0x4871).contains(&pc) {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 25 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case25_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 25
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case25_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case25_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case25_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        let pc = machine.cpu().registers().pc;
        if (0x484D..0x4871).contains(&pc) {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 31 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case31_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 31
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case31_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case31_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case31_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        let pc = machine.cpu().registers().pc;
        if (0x484D..0x4871).contains(&pc) {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 27 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case27_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u16;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u16;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 27
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case27_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case27_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case27_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        let pc = machine.cpu().registers().pc;
        if arm_count > 0 && (0x484D..0x48F3).contains(&pc) {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 32 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case32_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u16;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u16;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 32
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case32_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case32_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case32_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        let pc = machine.cpu().registers().pc;
        if (0x484D..0x4871).contains(&pc) {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 29 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case29_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u16;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u16;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 29
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case29_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case29_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case29_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        let pc = machine.cpu().registers().pc;
        if arm_count > 0 && (0x484D..0x4871).contains(&pc) {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 34 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case34_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u8;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u8;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 34
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case34_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case34_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case34_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        let pc = machine.cpu().registers().pc;
        if arm_count > 0 && (0x484D..0x4871).contains(&pc) {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 9 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case9_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u16;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u16;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 9
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case9_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case9_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case9_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        let pc = machine.cpu().registers().pc;
        if arm_count > 0 && (0x484D..0x4871).contains(&pc) {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic round-by-round STAT counts for testcase 36 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case36_round_counts() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut arm_count = 0_u16;
    let mut saw_irq_for_arm = false;
    let mut read_count = 0_u16;
    let mut previous_ppu = machine.ppu().snapshot();

    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        let cpu_snapshot = machine.cpu().snapshot();
        let testcase_index = machine.read_bus(0xFF80);

        if testcase_index == 36
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataWrite
            && activity.address == 0xFF41
            && activity.value == 0x20
        {
            arm_count += 1;
            saw_irq_for_arm = false;
            read_count = 0;
            let ppu = machine.ppu().snapshot();
            println!(
                "case36_round{arm_count}_armed ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && !saw_irq_for_arm
            && matches!(
                machine.cpu().execution_state(),
                gb_core::CpuExecutionState::ServiceInterrupt {
                    source: gb_core::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_arm = true;
            let ppu = machine.ppu().snapshot();
            println!(
                "case36_round{arm_count}_irq ly={} line_dot={} mode={:?}",
                ppu.ly, ppu.line_dot, ppu.mode
            );
        }

        if arm_count > 0
            && saw_irq_for_arm
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            read_count += 1;
            let ppu = machine.ppu().snapshot();
            println!(
                "case36_round{arm_count}_read{read_count} value={:#04X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} ly={} line_dot={} mode={:?} mode0_start_dot={}",
                activity.value,
                previous_ppu.ly,
                previous_ppu.line_dot,
                previous_ppu.mode,
                previous_ppu.mode0_start_dot,
                ppu.ly,
                ppu.line_dot,
                ppu.mode,
                ppu.mode0_start_dot
            );
        }

        previous_ppu = machine.ppu().snapshot();

        let pc = machine.cpu().registers().pc;
        if arm_count > 0 && (0x484D..0x4871).contains(&pc) {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic ly68 window 240..320 for testcase 36 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case36_line68_window_240_320() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.roms/test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut previous = None;
    for _ in 0..50_000_000 {
        machine.step_t_cycle();

        if machine.read_bus(0xFF80) != 36 {
            continue;
        }

        let ppu = machine.ppu().snapshot();
        if ppu.ly == 68 && (240..=320).contains(&ppu.line_dot) {
            let current = Intr2Mode0TimingSpritesLine68Observation {
                line_dot: ppu.line_dot,
                mode: ppu.mode,
                mode0_start_dot: ppu.mode0_start_dot,
                current_transfer_x: ppu.bg_current_transfer_x,
                visible_pixels_output: ppu.visible_pixels_output,
                current_transfer_lane: ppu.bg_current_transfer_lane,
                current_transfer_source_window: ppu.bg_current_transfer_source_window,
                current_transfer_backing: ppu.bg_current_transfer_backing,
                current_transfer_readiness: ppu.bg_current_transfer_readiness,
                bg_fifo_len: ppu.bg_fifo_pixels.len(),
                startup_fifo_placeholders: ppu.bg_startup_fifo_placeholders,
                obj_fetcher_stage: ppu.obj_fetcher_stage,
                obj_fetcher_stage_dot: ppu.obj_fetcher_stage_dot,
                obj_pending_hit_match_x: ppu.obj_pending_hit_match_x,
                obj_pending_hit_len: ppu.obj_pending_hit_len,
                obj_pending_hit_front_sprite_slot: ppu.obj_pending_hit_front_sprite_slot,
                bg_fetcher_stage: ppu.bg_fetcher_stage,
                bg_fetcher_stage_dot: ppu.bg_fetcher_stage_dot,
                selected_sprites_len: ppu.selected_sprites.len(),
            };
            if previous.as_ref() != Some(&current) {
                println!("case36_line68_window={current:?}");
                previous = Some(current);
            }
        } else if previous.is_some() && ppu.ly > 68 {
            break;
        }
    }
}

#[test]
#[ignore = "diagnostic irq timing after STAT arm for testcase 34 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case34_irq_after_stat_arm() {
    let irq = sample_real_mooneye_intr_2_mode0_timing_sprites_irq_after_stat_arm_for_testcase(34);
    println!("case34_irq={irq:?}");
}

#[test]
#[ignore = "diagnostic STAT read sample after testcase 34 irq of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case34_stat_reads_after_irq() {
    let reads =
        sample_real_mooneye_intr_2_mode0_timing_sprites_stat_reads_after_irq_for_testcase(34, 8);
    for read in reads {
        println!("case34_read={read:?}");
    }
}

#[test]
#[ignore = "diagnostic line68 state changes for testcase 29 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case29_line68_changes() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(29, 68);
    for observation in observations {
        println!("case29_line68={observation:?}");
    }
}

#[test]
#[ignore = "diagnostic line68 state changes for testcase 32 of mooneye intr_2_mode0_timing_sprites"]
fn real_mooneye_intr_2_mode0_timing_sprites_logs_case32_line68_changes() {
    let observations =
        sample_real_mooneye_intr_2_mode0_timing_sprites_line_changes_for_testcase(32, 68);
    for observation in observations {
        println!("case32_line68={observation:?}");
    }
}

#[test]
fn mode2_to_mode3_stat_probe_matches_mooneye_counts() {
    let delay3 = run_intr_2_stat_mode_probe(3, 0x03);
    let delay2 = run_intr_2_stat_mode_probe(2, 0x03);

    assert_eq!(
        (delay3.count, delay2.count),
        (0x01, 0x02),
        "delay3={delay3:?} delay2={delay2:?}"
    );
}

#[test]
#[ignore = "diagnostic stat-mode probe no longer matches the current external mode0 oracle"]
fn mode2_to_mode0_stat_probe_matches_mooneye_counts() {
    let delay46 = run_intr_2_stat_mode_probe(46, 0x00);
    let delay45 = run_intr_2_stat_mode_probe(45, 0x00);

    assert_eq!(
        (delay46.count, delay45.count),
        (0x01, 0x02),
        "delay46={delay46:?} delay45={delay45:?}"
    );
}

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
fn mode0_stat_request_can_precede_visible_hblank_while_vram_stays_blocked() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0x8000, 0x12);
    machine.write_bus(0xFF45, 0x01);
    step_until_line_dot(&mut machine, 80);
    machine.write_bus(0xFF41, 0x08);
    machine.write_bus(0xFF0F, 0x00);

    step_until_line_dot(&mut machine, 247);

    let drawing = machine.ppu().snapshot();
    assert_eq!(drawing.mode, PpuAccessMode::Drawing);
    assert_eq!(machine.read_bus(0xFF41) & 0x03, 0x03);
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);
    assert_eq!(machine.read_bus(0x8000), 0xFF);

    machine.step_t_cycle();

    let stat_pretrigger = machine.ppu().snapshot();
    assert_eq!(stat_pretrigger.mode, PpuAccessMode::Drawing);
    assert_eq!(stat_pretrigger.line_dot, 248);
    assert_eq!(machine.read_bus(0xFF41) & 0x03, 0x03);
    assert_eq!(machine.read_bus(0xFF0F), 0xE2);
    assert_eq!(machine.read_bus(0x8000), 0xFF);

    step_until_line_dot(&mut machine, 251);

    let late_drawing = machine.ppu().snapshot();
    assert_eq!(late_drawing.mode, PpuAccessMode::Drawing);
    assert_eq!(machine.read_bus(0xFF41) & 0x03, 0x03);
    assert_eq!(machine.read_bus(0xFF0F), 0xE2);
    assert_eq!(machine.read_bus(0x8000), 0xFF);

    machine.step_t_cycle();

    let hblank = machine.ppu().snapshot();
    assert_eq!(hblank.mode, PpuAccessMode::HBlank);
    assert_eq!(hblank.line_dot, 252);
    assert_eq!(machine.read_bus(0xFF41) & 0x03, 0x00);
    assert_eq!(machine.read_bus(0xFF0F), 0xE2);
    assert_eq!(machine.read_bus(0x8000), 0x12);
}

#[test]
fn lcd_disabled_machine_state_keeps_the_ppu_raster_frozen() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF40, 0x00);

    for _ in 0..8 {
        machine.step_t_cycle();
    }

    let snapshot = machine.ppu().snapshot();
    assert_eq!(snapshot.lcd_state, PpuLcdState::Disabled);
    assert_eq!(snapshot.visible_output, PpuVisibleOutputState::ForcedBlank);
    assert!(!snapshot.blank_frame_active);
    assert_eq!(snapshot.ly, 0);
    assert_eq!(snapshot.line_dot, 0);
    assert_eq!(snapshot.mode, PpuAccessMode::HBlank);
}

#[test]
fn mid_scanline_lcdc7_disable_resets_the_raster_and_releases_ppu_bus_blocking() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0x8000, 0x12);
    step_until_line_dot(&mut machine, 100);

    assert_eq!(machine.ppu().snapshot().mode, PpuAccessMode::Drawing);
    assert_eq!(machine.read_bus(0x8000), 0xFF);
    assert_eq!(machine.read_bus(0xFE00), 0xFF);

    machine.write_bus(0xFF40, 0x00);

    let disabled = machine.ppu().snapshot();
    assert_eq!(disabled.lcd_state, PpuLcdState::Disabled);
    assert_eq!(disabled.visible_output, PpuVisibleOutputState::ForcedBlank);
    assert!(!disabled.blank_frame_active);
    assert_eq!(disabled.ly, 0);
    assert_eq!(disabled.line_dot, 0);
    assert_eq!(disabled.mode, PpuAccessMode::HBlank);
    assert_eq!(machine.read_bus(0x8000), 0x12);
    assert_eq!(machine.read_bus(0xFE00), 0x00);
}

#[test]
fn mode2_selection_on_the_live_machine_preserves_oam_order_and_caps_at_ten_entries() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    for index in 0..12 {
        let x = match index {
            0 => 0,
            1 => 168,
            _ => 8 + index,
        };
        seed_oam_entry(&mut machine, index, 16, x, 0x40 + index, 0);
    }
    seed_oam_entry(&mut machine, 20, 8, 24, 0x99, 0);

    for _ in 0..80 {
        machine.step_t_cycle();
    }

    let snapshot = machine.ppu().snapshot();
    assert_eq!(snapshot.mode, PpuAccessMode::Drawing);
    assert_eq!(snapshot.mode2_scanned_entries, 40);
    assert_eq!(snapshot.selected_sprites.len(), 10);
    assert_eq!(
        snapshot
            .selected_sprites
            .iter()
            .map(|sprite| sprite.oam_index)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
    );
    assert_eq!(snapshot.selected_sprites[0].x, 0);
    assert_eq!(snapshot.selected_sprites[1].x, 168);
}

#[test]
fn mode2_selection_uses_live_lcdc2_on_the_machine_timeline() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    seed_oam_entry(&mut machine, 0, 0, 24, 0x10, 0);
    seed_oam_entry(&mut machine, 1, 1, 32, 0x11, 0);

    machine.step_t_cycle();
    machine.step_t_cycle();
    assert!(machine.ppu().snapshot().selected_sprites.is_empty());

    machine.write_bus(0xFF40, 0x95);
    machine.step_t_cycle();
    machine.step_t_cycle();

    let snapshot = machine.ppu().snapshot();
    assert_eq!(snapshot.mode2_scanned_entries, 2);
    assert_eq!(snapshot.selected_sprites.len(), 1);
    assert_eq!(snapshot.selected_sprites[0].oam_index, 1);
    assert_eq!(snapshot.selected_sprites[0].y, 1);
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
fn entering_vblank_can_raise_vblank_and_mode1_stat_together() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(&[0x18, 0xFE], 0x00))
        .expect("NoMBC idle ROM should load");

    machine.write_bus(0xFF45, 0xFF);
    step_until_line_dot(&mut machine, 80);
    machine.write_bus(0xFF41, 0x10);
    machine.write_bus(0xFF0F, 0x00);

    step_until_position(&mut machine, 143, 455);

    let before_vblank = machine.ppu().snapshot();
    assert_eq!(before_vblank.mode, PpuAccessMode::HBlank);
    assert_eq!(machine.read_bus(0xFF0F), 0xE0);

    machine.step_t_cycle();

    let vblank = machine.ppu().snapshot();
    assert_eq!(vblank.ly, 144);
    assert_eq!(vblank.line_dot, 0);
    assert_eq!(vblank.mode, PpuAccessMode::VBlank);
    assert_eq!(machine.read_bus(0xFF41) & 0x03, 0x01);
    assert_eq!(machine.read_bus(0xFF0F), 0xE3);
}

#[test]
fn lcd_reenable_restarts_immediately_but_keeps_the_first_frame_visibly_blank() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    machine
        .load_cartridge(build_test_rom(&[0x18, 0xFE], 0x00))
        .expect("NoMBC idle ROM should load");

    seed_bg_tile_row(&mut machine, 0, 0, 0x00, 0xFF);
    seed_bg_tilemap_entry(&mut machine, 0, 0, 0);

    machine.write_bus(0xFF40, 0x00);
    machine.write_bus(0xFF40, 0x91);

    let restart = machine.ppu().snapshot();
    assert_eq!(restart.lcd_state, PpuLcdState::Enabled);
    assert_eq!(restart.mode, PpuAccessMode::HBlank);
    assert_eq!(restart.ly, 0);
    assert_eq!(restart.line_dot, 0);
    assert_eq!(restart.visible_output, PpuVisibleOutputState::ForcedBlank);
    assert!(restart.blank_frame_active);

    step_until_line_dot(&mut machine, 252);

    let blank_line = machine.ppu().snapshot();
    assert_eq!(blank_line.mode, PpuAccessMode::HBlank);
    assert_eq!(
        blank_line.visible_output,
        PpuVisibleOutputState::ForcedBlank
    );
    assert_eq!(blank_line.visible_pixels_output, 160);
    assert_eq!(&blank_line.current_scanline_pixels[..8], &[0; 8]);

    step_until_next_frame_start(&mut machine);

    let second_frame_start = machine.ppu().snapshot();
    assert_eq!(
        second_frame_start.visible_output,
        PpuVisibleOutputState::Driving
    );
    assert!(!second_frame_start.blank_frame_active);
    assert_eq!(second_frame_start.mode, PpuAccessMode::OamScan);

    step_until_line_dot(&mut machine, 252);

    let visible_line = machine.ppu().snapshot();
    assert_eq!(visible_line.mode, PpuAccessMode::HBlank);
    assert_eq!(visible_line.visible_output, PpuVisibleOutputState::Driving);
    assert_eq!(&visible_line.current_scanline_pixels[..8], &[2; 8]);
}

#[test]
#[ignore = "pending Mooneye lcdon timing closure"]
fn lcd_reenable_initial_readback_matches_the_mooneye_lcdon_timing_probe_points() {
    const PROBE_M_CYCLES: [u16; 24] = [
        0, 17, 60, 110, 130, 174, 224, 244, 1, 18, 61, 111, 131, 175, 225, 245, 2, 19, 62, 112,
        132, 176, 226, 246,
    ];
    const EXPECTED_LY: [u8; 24] = [
        0x00, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x02, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x02,
        0x02, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x02, 0x02,
    ];
    const EXPECTED_STAT_LYC0: [u8; 24] = [
        0x84, 0x84, 0x87, 0x84, 0x82, 0x83, 0x80, 0x82, 0x84, 0x87, 0x84, 0x80, 0x82, 0x80, 0x80,
        0x82, 0x84, 0x87, 0x84, 0x82, 0x83, 0x80, 0x82, 0x83,
    ];
    const EXPECTED_STAT_LYC1: [u8; 24] = [
        0x80, 0x80, 0x83, 0x80, 0x86, 0x87, 0x84, 0x82, 0x80, 0x83, 0x80, 0x80, 0x86, 0x84, 0x80,
        0x82, 0x80, 0x83, 0x80, 0x86, 0x87, 0x84, 0x82, 0x83,
    ];
    const EXPECTED_OAM: [u8; 24] = [
        0x00, 0x00, 0xFF, 0x00, 0xFF, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0xFF, 0x00, 0xFF,
        0xFF, 0x00, 0xFF, 0x00, 0xFF, 0xFF, 0x00, 0xFF, 0xFF,
    ];
    const EXPECTED_VRAM: [u8; 24] = [
        0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00,
        0xFF, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF,
    ];

    let build_machine = || {
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );
        machine.write_bus(0xFF40, 0x00);
        machine.write_bus(0x8000, 0x00);
        machine.write_bus(0xFE00, 0x00);
        machine
    };

    let actual_ly = PROBE_M_CYCLES.map(|target_m_cycle| {
        let mut machine = build_machine();
        machine.write_bus(0xFF40, 0x81);
        sample_after_lcd_enable(&mut machine, target_m_cycle, |machine| {
            machine.read_bus(0xFF44)
        })
    });

    let actual_stat_lyc0 = PROBE_M_CYCLES.map(|target_m_cycle| {
        let mut machine = build_machine();
        machine.write_bus(0xFF45, 0x00);
        machine.write_bus(0xFF40, 0x81);
        sample_after_lcd_enable(&mut machine, target_m_cycle, |machine| {
            machine.read_bus(0xFF41)
        })
    });

    let actual_stat_lyc1 = PROBE_M_CYCLES.map(|target_m_cycle| {
        let mut machine = build_machine();
        machine.write_bus(0xFF45, 0x01);
        machine.write_bus(0xFF40, 0x81);
        sample_after_lcd_enable(&mut machine, target_m_cycle, |machine| {
            machine.read_bus(0xFF41)
        })
    });

    let actual_oam = PROBE_M_CYCLES.map(|target_m_cycle| {
        let mut machine = build_machine();
        machine.write_bus(0xFF40, 0x81);
        sample_after_lcd_enable(&mut machine, target_m_cycle, |machine| {
            machine.read_bus(0xFE00)
        })
    });

    let actual_vram = PROBE_M_CYCLES.map(|target_m_cycle| {
        let mut machine = build_machine();
        machine.write_bus(0xFF40, 0x81);
        sample_after_lcd_enable(&mut machine, target_m_cycle, |machine| {
            machine.read_bus(0x8000)
        })
    });

    if actual_ly != EXPECTED_LY
        || actual_stat_lyc0 != EXPECTED_STAT_LYC0
        || actual_stat_lyc1 != EXPECTED_STAT_LYC1
        || actual_oam != EXPECTED_OAM
        || actual_vram != EXPECTED_VRAM
    {
        panic!(
            "actual_ly={actual_ly:?}\nactual_stat_lyc0={actual_stat_lyc0:?}\nactual_stat_lyc1={actual_stat_lyc1:?}\nactual_oam={actual_oam:?}\nactual_vram={actual_vram:?}"
        );
    }
}

#[test]
#[ignore = "investigating CPU-path LCD enable chronology"]
fn cpu_path_lcd_enable_read_probe_matches_the_mooneye_probe_points() {
    const PROBE_M_CYCLES: [u16; 24] = [
        0, 17, 60, 110, 130, 174, 224, 244, 1, 18, 61, 111, 131, 175, 225, 245, 2, 19, 62, 112,
        132, 176, 226, 246,
    ];
    const EXPECTED_LY: [u8; 24] = [
        0x00, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x02, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x02,
        0x02, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x02, 0x02,
    ];
    const EXPECTED_STAT_LYC0: [u8; 24] = [
        0x84, 0x84, 0x87, 0x84, 0x82, 0x83, 0x80, 0x82, 0x84, 0x87, 0x84, 0x80, 0x82, 0x80, 0x80,
        0x82, 0x84, 0x87, 0x84, 0x82, 0x83, 0x80, 0x82, 0x83,
    ];
    const EXPECTED_STAT_LYC1: [u8; 24] = [
        0x80, 0x80, 0x83, 0x80, 0x86, 0x87, 0x84, 0x82, 0x80, 0x83, 0x80, 0x80, 0x86, 0x84, 0x80,
        0x82, 0x80, 0x83, 0x80, 0x86, 0x87, 0x84, 0x82, 0x83,
    ];
    const EXPECTED_OAM: [u8; 24] = [
        0x00, 0x00, 0xFF, 0x00, 0xFF, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0xFF, 0x00, 0xFF,
        0xFF, 0x00, 0xFF, 0x00, 0xFF, 0xFF, 0x00, 0xFF, 0xFF,
    ];
    const EXPECTED_VRAM: [u8; 24] = [
        0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00,
        0xFF, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFF,
    ];

    let run_probe = |address: u16, delay_nops: u16| -> u8 {
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );
        machine
            .load_cartridge(build_lcd_enable_read_probe_rom(
                address,
                delay_nops as usize,
            ))
            .expect("probe ROM should load");
        machine.write_bus(0xFF40, 0x00);
        run_until_halted(&mut machine, 1_000_000)
    };

    let actual_ly = PROBE_M_CYCLES.map(|delay| run_probe(0xFF44, delay));
    let actual_stat_lyc0 = PROBE_M_CYCLES.map(|delay| {
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );
        machine
            .load_cartridge(build_lcd_enable_read_probe_rom(0xFF41, delay as usize))
            .expect("probe ROM should load");
        machine.write_bus(0xFF40, 0x00);
        machine.write_bus(0xFF45, 0x00);
        run_until_halted(&mut machine, 1_000_000)
    });
    let actual_stat_lyc1 = PROBE_M_CYCLES.map(|delay| {
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
        );
        machine
            .load_cartridge(build_lcd_enable_read_probe_rom(0xFF41, delay as usize))
            .expect("probe ROM should load");
        machine.write_bus(0xFF40, 0x00);
        machine.write_bus(0xFF45, 0x01);
        run_until_halted(&mut machine, 1_000_000)
    });
    let actual_oam = PROBE_M_CYCLES.map(|delay| run_probe(0xFE00, delay));
    let actual_vram = PROBE_M_CYCLES.map(|delay| run_probe(0x8000, delay));

    if actual_ly != EXPECTED_LY
        || actual_stat_lyc0 != EXPECTED_STAT_LYC0
        || actual_stat_lyc1 != EXPECTED_STAT_LYC1
        || actual_oam != EXPECTED_OAM
        || actual_vram != EXPECTED_VRAM
    {
        panic!(
            "actual_ly={actual_ly:?}\nactual_stat_lyc0={actual_stat_lyc0:?}\nactual_stat_lyc1={actual_stat_lyc1:?}\nactual_oam={actual_oam:?}\nactual_vram={actual_vram:?}"
        );
    }
}

#[test]
#[ignore = "investigating CPU-path LCDC.7 write chronology"]
fn cpu_path_lcd_enable_write_probe_matches_the_mooneye_probe_points() {
    const NOP_COUNTS: [u16; 19] = [
        0, 17, 18, 60, 61, 110, 111, 112, 130, 131, 132, 174, 175, 224, 225, 226, 244, 245, 246,
    ];
    const EXPECTED_OAM: [u8; 19] = [
        0x81, 0x81, 0x00, 0x00, 0x81, 0x81, 0x81, 0x00, 0x00, 0x81, 0x00, 0x00, 0x81, 0x81, 0x81,
        0x00, 0x00, 0x81, 0x00,
    ];
    const EXPECTED_VRAM: [u8; 19] = [
        0x81, 0x81, 0x00, 0x00, 0x81, 0x81, 0x81, 0x81, 0x81, 0x81, 0x00, 0x00, 0x81, 0x81, 0x81,
        0x81, 0x81, 0x81, 0x00,
    ];

    let actual_oam = NOP_COUNTS
        .map(|delay| run_lcd_enable_write_probe_observation(0xFE00, delay).observed_value);
    let actual_vram = NOP_COUNTS
        .map(|delay| run_lcd_enable_write_probe_observation(0x8000, delay).observed_value);

    if actual_oam != EXPECTED_OAM || actual_vram != EXPECTED_VRAM {
        panic!("actual_oam={actual_oam:?}\nactual_vram={actual_vram:?}");
    }
}

#[test]
#[ignore = "diagnostic probe for lcd enable write chronology"]
fn cpu_path_lcd_enable_write_probe_logs_boundary_snapshots() {
    for delay in [111_u16, 112, 131, 132, 225, 226, 245, 246] {
        let oam = run_lcd_enable_write_probe_observation(0xFE00, delay);
        let vram = run_lcd_enable_write_probe_observation(0x8000, delay);
        println!("delay={delay} oam={oam:?} vram={vram:?}");
    }
}

#[test]
fn lcd_off_releases_ppu_mode_restrictions_without_overriding_dma_hram_only_blocking() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );

    machine.write_bus(0xFF40, 0x00);
    machine.write_bus(0x8000, 0x12);
    machine.write_bus(0xFE00, 0x34);
    machine.write_bus(0xFF46, 0x80);
    for _ in 0..5 {
        machine.step_t_cycle();
    }

    let disabled = machine.ppu().snapshot();
    assert_eq!(disabled.lcd_state, PpuLcdState::Disabled);
    assert_eq!(machine.read_bus(0x8000), 0xFF);
    assert_eq!(machine.read_bus(0xFE00), 0xFF);
    assert_eq!(machine.read_bus(0xFF80), 0x00);
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

#[test]
fn direct_mode2_oam_write_corrupts_the_live_row_without_storing_the_cpu_byte() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    let rows = build_oam_corruption_fixture();

    machine.write_bus(0xFF40, 0x00);
    seed_oam_corruption_fixture(&mut machine, &rows);
    machine.write_bus(0xFF40, 0x80);

    while machine.ppu().snapshot().current_oam_scan_row != Some(1) {
        machine.step_t_cycle();
    }

    let row = machine.ppu().snapshot().current_oam_scan_row.unwrap();
    machine.write_bus(0xFE20, 0x99);
    machine.write_bus(0xFF40, 0x00);

    assert_eq!(
        read_oam_corruption_row(&mut machine, row),
        expected_write_corruption(&rows, row)
    );
}

#[test]
fn direct_mode2_fea0_read_uses_blocked_readback_and_the_same_read_corruption_path() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    let rows = build_oam_corruption_fixture();

    machine.write_bus(0xFF40, 0x00);
    seed_oam_corruption_fixture(&mut machine, &rows);
    machine.write_bus(0xFF40, 0x80);

    while machine.ppu().snapshot().current_oam_scan_row != Some(2) {
        machine.step_t_cycle();
    }

    let row = machine.ppu().snapshot().current_oam_scan_row.unwrap();
    assert_eq!(machine.read_bus(0xFEA0), 0xFF);
    machine.write_bus(0xFF40, 0x00);

    assert_eq!(
        read_oam_corruption_row(&mut machine, row),
        expected_read_corruption(&rows, row)
    );
}

#[test]
fn cpu_inc_hl_inside_fe_range_reaches_the_same_mode2_corruption_controller() {
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::Dmg).with_startup_mode(StartupMode::SkipBoot),
    );
    let rows = build_oam_corruption_fixture();
    let mut program = vec![0x21, 0x08, 0xFE];
    program.extend(std::iter::repeat_n(0x00, 119));
    program.extend([0x23, 0x00]);

    machine
        .load_cartridge(build_test_rom(&program, 0x12))
        .expect("NoMBC test ROM should load");

    machine.write_bus(0xFF40, 0x00);
    seed_oam_corruption_fixture(&mut machine, &rows);

    for _ in 0..12 {
        machine.step_t_cycle();
    }

    machine.write_bus(0xFF40, 0x80);

    let mut triggered_row = None;
    for _ in 0..1_024 {
        machine.step_t_cycle();
        if let Some(event) = machine.cpu().last_address_event()
            && event.kind == CpuAddressEventKind::IncDec
            && event.idu_address == Some(0xFE09)
            && event.update_direction == Some(CpuAddressUpdateDirection::Increment)
        {
            triggered_row = machine.ppu().snapshot().current_oam_scan_row;
            break;
        }
    }

    let row = triggered_row.expect("INC HL should trigger during Mode 2");
    assert!(row > 0);
    machine.write_bus(0xFF40, 0x00);

    assert_eq!(
        read_oam_corruption_row(&mut machine, row),
        expected_write_corruption(&rows, row)
    );
}
