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
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
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
