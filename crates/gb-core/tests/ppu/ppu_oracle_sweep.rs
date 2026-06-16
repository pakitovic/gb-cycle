// TEMPORARY oracle-grounding sweep (L2-a.1). Not a real test suite — prints the
// CPU-observable count-vs-delay curves so the branch can be diffed against `main`
// (PR #245), which passes every mooneye/wilbertpol STAT/LYC ROM. Delete before the
// final gate.
use super::*;

// ---- ly_lyc reenable+readback probe (faithful to wilbertpol ly_lyc/ly_lyc_144) ----
// Mirrors one round: wait_ly 144; LCD off; nop; LCD on; IF=0; wait_ly wait_v;
// nops N; ldh a,(target). LYC and STAT-enable set once at top (IE=0, di → IRQ only
// latches into IF, never dispatched).

fn build_reenable_readback_rom(
    lyc: u8,
    stat_enable: u8,
    wait_v: u8,
    target_low: u8,
    nops: usize,
) -> Vec<u8> {
    let mut program = Vec::new();

    program.push(0xF3); // di
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh (IE),a = 0
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh (IF),a = 0
    program.extend_from_slice(&[0x3E, lyc]); // ld a,lyc
    program.extend_from_slice(&[0xE0, 0x45]); // ldh (LYC),a
    program.extend_from_slice(&[0x3E, stat_enable]); // ld a,stat_enable
    program.extend_from_slice(&[0xE0, 0x41]); // ldh (STAT),a

    // wait_ly 144
    let w144 = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x44]); // ldh a,(LY)
    program.extend_from_slice(&[0xFE, 144]); // cp 144
    emit_jr_nz(&mut program, w144);

    // LCD off; nop; LCD on
    program.extend_from_slice(&[0x21, 0x40, 0xFF]); // ld hl,$FF40 (LCDC)
    program.extend_from_slice(&[0xCB, 0xBE]); // res 7,(hl)  LCD off
    program.push(0x00); // nop
    program.extend_from_slice(&[0xCB, 0xFE]); // set 7,(hl)  LCD on

    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh (IF),a = 0

    // wait_ly wait_v
    let wv = 0x0100_u16 + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x44]); // ldh a,(LY)
    program.extend_from_slice(&[0xFE, wait_v]); // cp wait_v
    emit_jr_nz(&mut program, wv);

    program.extend(std::iter::repeat_n(0x00, nops)); // nops N

    program.extend_from_slice(&[0xF0, target_low]); // ldh a,(target)  <-- the measured read
    program.push(0x47); // ld b,a
    program.extend_from_slice(&[0x3E, 0x01]); // ld a,1
    program.push(0x57); // ld d,a   (halt marker)
    program.push(0x76); // halt
    let done = 0x0100_u16 + program.len() as u16;
    emit_jr(&mut program, done);

    build_test_rom(&program, 0x00)
}

#[derive(Clone, Copy, Debug)]
struct ReadbackObservation {
    value: u8,
    ly: u8,
    line_dot: u16,
    mode: PpuAccessMode,
}

fn run_reenable_readback(
    model: ConsoleModel,
    lyc: u8,
    stat_enable: u8,
    wait_v: u8,
    target_low: u8,
    nops: usize,
) -> Option<ReadbackObservation> {
    let mut machine =
        Machine::new(MachineConfig::new(model).with_startup_mode(StartupMode::SkipBoot));
    machine
        .load_cartridge(build_reenable_readback_rom(
            lyc,
            stat_enable,
            wait_v,
            target_low,
            nops,
        ))
        .expect("probe ROM should load");

    let target_addr = 0xFF00 + target_low as u16;
    let mut read_at = None;
    for _ in 0..2_000_000 {
        machine.step_t_cycle();
        if read_at.is_none()
            && let Some(event) = machine.cpu().last_address_event()
            && event.kind == CpuAddressEventKind::Read
            && event.access_address == Some(target_addr)
        {
            let s = machine.ppu().snapshot();
            read_at = Some((s.ly, s.line_dot, s.mode));
        }
        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d != 0
        {
            let (ly, line_dot, mode) = read_at?;
            return Some(ReadbackObservation {
                value: machine.cpu().registers().b,
                ly,
                line_dot,
                mode,
            });
        }
    }
    None
}

#[test]
#[ignore = "oracle sweep (manual run with --nocapture)"]
fn oracle_sweep_intr_2_0() {
    println!("=== intr_2_0 (mode0 STAT, GameBoy) count-vs-delay ===");
    for delay in 0..=12 {
        let o = run_intr_2_0_probe(delay);
        println!(
            "delay={delay:2} count={:#04X} mode0w(ly={} dot={} {:?}) irq(ly={} dot={} {:?})",
            o.count,
            o.mode0_write_ly,
            o.mode0_write_line_dot,
            o.mode0_write_mode,
            o.second_irq_ly,
            o.second_irq_line_dot,
            o.second_irq_mode,
        );
    }
}

#[test]
#[ignore = "oracle sweep (manual run with --nocapture)"]
fn oracle_sweep_intr_2_stat_mode() {
    for target in [0x00_u8, 0x03] {
        println!("=== intr_2_stat_mode target_mode={target:#04X} (GameBoy) ===");
        for delay in 0..=12 {
            let o = run_intr_2_stat_mode_probe(delay, target);
            println!(
                "delay={delay:2} count={:#04X} irq(ly={} dot={} {:?}) halt(ly={} dot={} {:?})",
                o.count,
                o.irq_ly,
                o.irq_line_dot,
                o.irq_mode,
                o.halt_ly,
                o.halt_line_dot,
                o.halt_mode,
            );
        }
    }
}

fn sweep_readback(
    label: &str,
    model: ConsoleModel,
    lyc: u8,
    wait_v: u8,
    target_low: u8,
    nops_range: std::ops::RangeInclusive<usize>,
) {
    let tname = match target_low {
        0x44 => "LY",
        0x41 => "STAT",
        0x0F => "IF",
        _ => "?",
    };
    println!("=== {label} model={model:?} lyc={lyc:#04X} wait_ly={wait_v} read={tname} ===");
    for nops in nops_range {
        match run_reenable_readback(model, lyc, 0x40, wait_v, target_low, nops) {
            Some(o) => println!(
                "nops={nops:3} {tname}={:#04X} at(ly={} dot={} {:?})",
                o.value, o.ly, o.line_dot, o.mode
            ),
            None => println!("nops={nops:3} <timeout>"),
        }
    }
}

fn run_real_rom_capture_wram(path: &str, model: ConsoleModel, max_writes: usize) {
    let rom = std::fs::read(path).expect("rom file");
    let mut machine =
        Machine::new(MachineConfig::new(model).with_startup_mode(StartupMode::SkipBoot));
    machine.load_cartridge(rom).expect("rom load");
    let mut writes: Vec<(u16, u8)> = Vec::new();
    let mut last = None;
    for _ in 0..6_000_000 {
        machine.step_t_cycle();
        if let Some(event) = machine.cpu().last_address_event()
            && event.kind == CpuAddressEventKind::Write
            && let Some(addr) = event.access_address
            && (0xC000..0xE000).contains(&addr)
        {
            let cur = (
                addr,
                machine.cpu().registers().a,
                machine.cpu().registers().pc,
            );
            if last != Some(cur) {
                last = Some(cur);
                writes.push((addr, machine.cpu().registers().a));
                if writes.len() >= max_writes {
                    break;
                }
            }
        }
    }
    let r = machine.cpu().registers();
    println!(
        "--- {path} ({model:?}) wram writes (first {max_writes}) ---\n{:?}\nfinal regs a={:#04X} f={:#04X} b={:#04X} c={:#04X} d={:#04X} e={:#04X} h={:#04X} l={:#04X}",
        writes
            .iter()
            .map(|(a, v)| format!("[{a:#06X}]={v:#04X}"))
            .collect::<Vec<_>>(),
        r.a,
        r.f,
        r.b,
        r.c,
        r.d,
        r.e,
        r.h,
        r.l
    );
}

#[test]
#[ignore = "oracle sweep (manual run with --nocapture)"]
fn oracle_run_ly_lyc_roms() {
    let base = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../test/wilbertpol/wilbertpol/acceptance/gpu"
    );
    for (name, model) in [
        ("ly_lyc-GS", ConsoleModel::GameBoy),
        ("ly_lyc-C", ConsoleModel::GameBoyColor),
        ("ly_lyc_0-GS", ConsoleModel::GameBoy),
        ("ly_lyc_0-C", ConsoleModel::GameBoyColor),
        ("ly_lyc_144-GS", ConsoleModel::GameBoy),
        ("ly_lyc_144-C", ConsoleModel::GameBoyColor),
        ("ly_lyc_write-GS", ConsoleModel::GameBoy),
        ("ly_lyc_0_write-GS", ConsoleModel::GameBoy),
    ] {
        run_real_rom_capture_wram(&format!("{base}/{name}.gb"), model, 10);
    }
}

fn sweep_vblank(
    label: &str,
    model: ConsoleModel,
    stat_enable: u8,
    nops_range: std::ops::RangeInclusive<usize>,
) {
    println!(
        "=== {label} model={model:?} stat={stat_enable:#04X} (reenable, wait_ly 143, read IF) ==="
    );
    for nops in nops_range {
        match run_reenable_readback(model, 0xF0, stat_enable, 143, 0x0F, nops) {
            Some(o) => println!(
                "nops={nops:3} IF={:#04X} at(ly={} dot={} {:?})",
                o.value, o.ly, o.line_dot, o.mode
            ),
            None => println!("nops={nops:3} <timeout>"),
        }
    }
}

#[test]
#[ignore = "oracle sweep (manual run with --nocapture)"]
fn oracle_sweep_vblank() {
    for model in [ConsoleModel::GameBoy, ConsoleModel::GameBoyColor] {
        // vblank_if_timing: STAT disabled, vblank IF edge E0->E1 (expected nops 97->98).
        sweep_vblank("vblank_if", model, 0x00, 98..=108);
        // intr_1_timing-ish: mode1 STAT enabled, vblank+mode1 edge (E0->E3).
        sweep_vblank("intr_1(mode1)", model, 0x10, 98..=108);
    }
}

#[test]
#[ignore = "oracle sweep (manual run with --nocapture)"]
fn oracle_sweep_ly_lyc() {
    for model in [ConsoleModel::GameBoy, ConsoleModel::GameBoyColor] {
        // ly_lyc: LYC=2, wait_ly 1. ROM reads LY@100/101, STAT@101/102, IF@101/102,
        // STAT@214/215. Expected (-GS): LY 1->2, STAT C0->C6, IF E0->E2, STAT C4->C0.
        sweep_readback("ly_lyc", model, 0x02, 1, 0x44, 97..=104);
        sweep_readback("ly_lyc", model, 0x02, 1, 0x41, 97..=104);
        sweep_readback("ly_lyc", model, 0x02, 1, 0x0F, 97..=104);
        sweep_readback("ly_lyc", model, 0x02, 1, 0x41, 211..=218);
        // ly_lyc_144: LYC=144, wait_ly 143. Expected (-GS): LY 143->144, STAT C0->C5,
        // IF E0->E3, STAT C5->C1.
        sweep_readback("ly_lyc_144", model, 0x90, 143, 0x44, 101..=108);
        sweep_readback("ly_lyc_144", model, 0x90, 143, 0x41, 101..=108);
        sweep_readback("ly_lyc_144", model, 0x90, 143, 0x0F, 101..=108);
    }
}

// ---- faithful intr_2_mode0_timing probe (mooneye test_iter: mode2 IRQ, nops, poll
// STAT until mode0, counting). Returns the count register b. ----
const PROBE_ENTRY: u16 = 0x0150;

fn emit_jr_nz_at(program: &mut Vec<u8>, target_pc: u16) {
    let next_pc = PROBE_ENTRY + program.len() as u16 + 2;
    let offset = target_pc as i32 - next_pc as i32;
    assert!(i8::try_from(offset).is_ok(), "jr nz target out of range");
    program.push(0x20);
    program.push(offset as i8 as u8);
}

fn emit_jr_at(program: &mut Vec<u8>, target_pc: u16) {
    let next_pc = PROBE_ENTRY + program.len() as u16 + 2;
    let offset = target_pc as i32 - next_pc as i32;
    assert!(i8::try_from(offset).is_ok(), "jr target out of range");
    program.push(0x18);
    program.push(offset as i8 as u8);
}

fn build_intr_2_mode0_timing_probe_rom(delay: usize, target_mode: u8) -> Vec<u8> {
    let base = PROBE_ENTRY;
    let mut program = Vec::new();
    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000
    program.push(0xAF); // xor a
    program.push(0x57); // ld d,a  (d=0, not the marker)
    // wait_ly 144 (rough vblank sync)
    let w144 = base + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x44]);
    program.extend_from_slice(&[0xFE, 144]);
    emit_jr_nz_at(&mut program, w144);
    program.extend_from_slice(&[0x3E, 0x02]); // ld a,2
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh (IE),a = STAT
    program.extend_from_slice(&[0x21, 0x41, 0xFF]); // ld hl,$FF41 (STAT)

    program.push(0xCD); // call setup_and_wait_mode2
    let setup_operand = program.len();
    program.extend_from_slice(&[0x00, 0x00]);

    program.extend(std::iter::repeat_n(0x00, delay)); // nops delay
    program.push(0x06); // ld b,$00
    program.push(0x00);
    let loop_pc = base + program.len() as u16;
    program.push(0x04); // inc b
    program.push(0x7E); // ld a,(hl)  ; read STAT
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    program.extend_from_slice(&[0xFE, target_mode]); // cp target_mode
    emit_jr_nz_at(&mut program, loop_pc); // jr nz,loop

    program.push(0x48); // ld c,b  (count -> c)
    program.extend_from_slice(&[0x3E, 0xAA]); // ld a,$AA
    program.push(0x57); // ld d,a  (marker)
    program.push(0x76); // halt
    let done = base + program.len() as u16;
    emit_jr_at(&mut program, done);

    // setup_and_wait_mode2:
    let setup_pc = base + program.len() as u16;
    let wly = base + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x44]); // ldh a,(LY)
    program.extend_from_slice(&[0xFE, 0x42]); // cp $42
    emit_jr_nz_at(&mut program, wly);
    let wm0 = base + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x41]);
    program.extend_from_slice(&[0xE6, 0x03]);
    program.extend_from_slice(&[0xFE, 0x00]);
    emit_jr_nz_at(&mut program, wm0);
    let wm3 = base + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x41]);
    program.extend_from_slice(&[0xE6, 0x03]);
    program.extend_from_slice(&[0xFE, 0x03]);
    emit_jr_nz_at(&mut program, wm3);
    program.extend_from_slice(&[0x3E, 0x20]); // ld a,$20 (mode2 enable)
    program.extend_from_slice(&[0xE0, 0x41]); // ldh (STAT),a
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh (IF),a
    program.push(0xFB); // ei
    program.push(0x76); // halt
    program.push(0x00); // nop
    let fl = base + program.len() as u16;
    emit_jr_at(&mut program, fl); // jr . (fail loop)

    patch_abs16(&mut program, setup_operand, setup_pc);

    let mut rom = build_nom_bc_test_rom_with_program_entry(&program, 0x00, base as usize, &[]);
    rom[0x0048] = 0xE8; // add sp,+2
    rom[0x0049] = 0x02;
    rom[0x004A] = 0xC9; // ret
    rom
}

fn run_intr_2_mode0_timing_probe(model: ConsoleModel, delay: usize, target_mode: u8) -> Option<u8> {
    let mut machine =
        Machine::new(MachineConfig::new(model).with_startup_mode(StartupMode::SkipBoot));
    machine
        .load_cartridge(build_intr_2_mode0_timing_probe_rom(delay, target_mode))
        .expect("probe ROM should load");
    for _ in 0..3_000_000 {
        machine.step_t_cycle();
        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d == 0xAA
        {
            return Some(machine.cpu().registers().c);
        }
    }
    None
}

#[test]
#[ignore = "oracle sweep (manual run with --nocapture)"]
fn oracle_sweep_intr_2_mode0_timing() {
    for model in [ConsoleModel::GameBoy, ConsoleModel::GameBoyColor] {
        // intr_2_mode0_timing: mode2 IRQ -> nops -> poll until mode0. assert d(46)=1, e(45)=2.
        println!("=== intr_2_mode0_timing model={model:?} (poll until mode0) ===");
        for delay in 42..=49 {
            match run_intr_2_mode0_timing_probe(model, delay, 0x00) {
                Some(c) => println!("delay={delay:3} count={c:#04X}"),
                None => println!("delay={delay:3} <timeout>"),
            }
        }
        // intr_2_oam_ok_timing polls until mode2 (target 2); same setup.
        println!("=== intr_2 poll-until-mode2 model={model:?} ===");
        for delay in 42..=49 {
            match run_intr_2_mode0_timing_probe(model, delay, 0x02) {
                Some(c) => println!("delay={delay:3} count={c:#04X}"),
                None => println!("delay={delay:3} <timeout>"),
            }
        }
    }
}
