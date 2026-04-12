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
