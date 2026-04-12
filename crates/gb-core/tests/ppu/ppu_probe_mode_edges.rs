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

#[allow(dead_code)]
#[derive(Debug)]
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
