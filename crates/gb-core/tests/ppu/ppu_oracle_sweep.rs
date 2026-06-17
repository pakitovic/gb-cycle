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

// ---- faithful vblank_if_timing round4/5 probe (wait vblank IRQ at LY=144, di, nops
// delay, then read LY). Reports the LY value + the ly/line_dot at the read. Real test:
// nops 96 -> LY 144, nops 97 -> LY 145. ----
fn build_vblank_ly_after_irq_probe_rom(delay: usize) -> Vec<u8> {
    let base = PROBE_ENTRY;
    let mut program = Vec::new();
    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000
    program.push(0xAF); // xor a
    program.push(0x57); // ld d,a  (d=0, not the marker)
    // wait_ly 142
    let w142 = base + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x44]);
    program.extend_from_slice(&[0xFE, 142]);
    emit_jr_nz_at(&mut program, w142);
    program.extend_from_slice(&[0x3E, 0x01]); // ld a,1
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh (IE),a = VBlank
    program.push(0xAF); // xor a
    program.extend_from_slice(&[0xE0, 0x0F]); // ldh (IF),a = 0
    program.push(0xFB); // ei
    program.push(0x76); // halt  (wake on VBlank IRQ at LY=144)
    program.push(0x00); // nop
    program.push(0xF3); // di
    program.extend(std::iter::repeat_n(0x00, delay)); // nops delay
    program.extend_from_slice(&[0xF0, 0x44]); // ldh a,(LY)  <-- measured read
    program.push(0x4F); // ld c,a
    program.extend_from_slice(&[0x3E, 0xAA]); // ld a,$AA
    program.push(0x57); // ld d,a  (marker)
    program.push(0x76); // halt
    let done = base + program.len() as u16;
    emit_jr_at(&mut program, done);

    let mut rom = build_nom_bc_test_rom_with_program_entry(&program, 0x00, base as usize, &[]);
    rom[0x0040] = 0xD9; // reti (VBlank vector)
    rom
}

fn run_vblank_ly_after_irq_probe(model: ConsoleModel, delay: usize) -> Option<(u8, u8, u16)> {
    let mut machine =
        Machine::new(MachineConfig::new(model).with_startup_mode(StartupMode::SkipBoot));
    machine
        .load_cartridge(build_vblank_ly_after_irq_probe_rom(delay))
        .expect("probe ROM should load");
    let mut read_at = None;
    for _ in 0..3_000_000 {
        machine.step_t_cycle();
        // Capture the LAST LY read before the marker (the measured read); the wait_ly 142
        // loop also reads 0xFF44 many times, so keep overwriting.
        if let Some(event) = machine.cpu().last_address_event()
            && event.kind == CpuAddressEventKind::Read
            && event.access_address == Some(0xFF44)
            && machine.cpu().registers().d == 0
        {
            let s = machine.ppu().snapshot();
            read_at = Some((s.ly, s.line_dot));
        }
        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d == 0xAA
        {
            let (ly, dot) = read_at?;
            return Some((machine.cpu().registers().c, ly, dot));
        }
    }
    None
}

#[test]
#[ignore = "oracle sweep (manual run with --nocapture)"]
fn oracle_sweep_vblank_ly_after_irq() {
    for model in [ConsoleModel::GameBoy, ConsoleModel::GameBoyColor] {
        println!("=== vblank_if round4/5 (LY after vblank IRQ) model={model:?} ===");
        for delay in 92..=102 {
            match run_vblank_ly_after_irq_probe(model, delay) {
                Some((v, ly, dot)) => {
                    println!("delay={delay:3} LY={v:#04X}({v}) read_at(ly={ly} dot={dot})")
                }
                None => println!("delay={delay:3} <timeout>"),
            }
        }
    }
}

#[test]
#[ignore = "oracle sweep (manual run with --nocapture)"]
fn oracle_run_boot_hwio_dmg0() {
    // boot_hwio-dmg0 walks $FF00..$FF7F once at $0100 and compares to a per-row-masked
    // table; dmg0 expects DIV $19, STAT $83, LY $01. Capture the first CPU read value of
    // DIV/STAT/LY during the walk to see which register mismatches.
    for (label, rom_name, rev) in [
        (
            "dmg0",
            "boot_hwio-dmg0.gb",
            gb_core::HardwareRevision::DmgCpu0,
        ),
        (
            "dmgABCmgb",
            "boot_hwio-dmgABCmgb.gb",
            gb_core::HardwareRevision::DmgCpuC,
        ),
    ] {
        let rom = std::fs::read(format!(
            "{}/../../test/mooneye/mooneye/acceptance/{rom_name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("rom file");
        let mut machine = Machine::new(
            MachineConfig::new(ConsoleModel::GameBoy)
                .with_revision(rev)
                .with_startup_mode(StartupMode::SkipBoot),
        );
        machine.load_cartridge(rom).expect("rom load");
        let mut seen: std::collections::BTreeMap<u16, (u8, u8, u16, PpuAccessMode, u64)> =
            std::collections::BTreeMap::new();
        let mut t = 0u64;
        for _ in 0..2_000_000 {
            machine.step_t_cycle();
            t += 1;
            if let Some(event) = machine.cpu().last_address_event()
                && event.kind == CpuAddressEventKind::Read
                && let Some(addr) = event.access_address
                && matches!(addr, 0xFF04 | 0xFF41 | 0xFF44 | 0xFF05 | 0xFF40)
                && !seen.contains_key(&addr)
            {
                let s = machine.ppu().snapshot();
                seen.insert(
                    addr,
                    (machine.cpu().registers().a, s.ly, s.line_dot, s.mode, t),
                );
            }
            if seen.len() >= 5 {
                break;
            }
        }
        println!("=== {label} ({rom_name}) ===");
        for (addr, (val, ly, dot, mode, at)) in &seen {
            let name = match addr {
                0xFF04 => "DIV",
                0xFF05 => "TIMA",
                0xFF40 => "LCDC",
                0xFF41 => "STAT",
                0xFF44 => "LY",
                _ => "?",
            };
            println!(
                "  first read {name}({addr:#06X}) = {val:#04X}  ppu(ly={ly} dot={dot} {mode:?}) t={at}"
            );
        }
    }
}

#[test]
#[ignore = "oracle sweep (manual run with --nocapture)"]
fn oracle_run_intr_2_timing_rom() {
    let base = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../test/wilbertpol/wilbertpol/acceptance/gpu"
    );
    // intr_2_timing rounds (IF reads after LCD re-enable): r1 0nops->$E0, r2 109->$E0,
    // r3 110->$E2, r4 130->$E2, r5 wait143+70+clr+26->$E0, r6 +27->$E2, r7 +28->$E3.
    // Stores r1..r7 to consecutive WRAM. wb-only (DMG).
    run_real_rom_capture_wram(
        &format!("{base}/intr_2_timing.gb"),
        ConsoleModel::GameBoy,
        40,
    );
}

#[test]
#[ignore = "oracle sweep (manual run with --nocapture)"]
fn oracle_run_vblank_if_rom() {
    let base = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../test/wilbertpol/wilbertpol/acceptance/gpu"
    );
    // vblank_if_timing rounds: r1 nops97->IF $E0, r2 nops98->IF $E1, r3 wait_ly20->IF $E1,
    // r4 vblank_irq+nops96->LY 144 ($90), r5 vblank_irq+nops97->LY 145 ($91). Captures WRAM
    // round stores; identify by value sequence. wb-only (DMG).
    run_real_rom_capture_wram(
        &format!("{base}/vblank_if_timing.gb"),
        ConsoleModel::GameBoy,
        40,
    );
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
        sweep_vblank("vblank_if", model, 0x00, 92..=114);
        // intr_1_timing-ish: mode1 STAT enabled, vblank+mode1 edge (E0->E3).
        sweep_vblank("intr_1(mode1)", model, 0x10, 92..=114);
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

// ---- faithful intr_2_mode0_timing_sprites probe (wilbertpol: set up N sprites at
// Y=$52,X=given, enable OBJ, mode2 IRQ, nops delay, poll STAT until mode0 counting in b).
// Real test: round_a delay=41+extra -> count 1; round_b delay=40+extra -> count 2. ----
fn build_intr_2_sprites_probe_rom(sprite_xs: &[u8], delay: usize) -> Vec<u8> {
    let base = PROBE_ENTRY;
    let mut program = Vec::new();
    program.extend_from_slice(&[0x31, 0x00, 0xE0]); // ld sp,$E000
    program.push(0xAF); // xor a
    program.push(0x57); // ld d,a  (d=0, not the marker)
    // wait_ly 144 (vblank; OAM writable)
    let w144 = base + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x44]);
    program.extend_from_slice(&[0xFE, 144]);
    emit_jr_nz_at(&mut program, w144);
    // Clear OAM ($FE00..$FE9F) then write sprites.
    program.extend_from_slice(&[0x21, 0x00, 0xFE]); // ld hl,$FE00
    for (i, &x) in sprite_xs.iter().enumerate() {
        program.extend_from_slice(&[0x3E, 0x52]); // ld a,$52 (Y)
        program.push(0x22); // ld (hl+),a
        program.extend_from_slice(&[0x3E, x]); // ld a,x
        program.push(0x22); // ld (hl+),a
        program.extend_from_slice(&[0x3E, 0x30 + i as u8]); // ld a,tile
        program.push(0x22); // ld (hl+),a
        program.push(0xAF); // xor a
        program.push(0x22); // ld (hl+),a (flags=0)
    }
    // Enable OBJ (LCDC.1)
    program.extend_from_slice(&[0x21, 0x40, 0xFF]); // ld hl,$FF40
    program.extend_from_slice(&[0xCB, 0xCE]); // set 1,(hl)
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
    emit_jr_nz_at(&mut program, loop_pc); // jr nz,loop (until mode0)

    program.push(0x48); // ld c,b  (count -> c)
    program.extend_from_slice(&[0x3E, 0xAA]); // ld a,$AA
    program.push(0x57); // ld d,a  (marker)
    program.push(0x76); // halt
    let done = base + program.len() as u16;
    emit_jr_at(&mut program, done);

    // setup_and_wait_mode2 (identical to the mode0 timing probe):
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

fn run_intr_2_sprites_probe(model: ConsoleModel, sprite_xs: &[u8], delay: usize) -> Option<u8> {
    let mut machine =
        Machine::new(MachineConfig::new(model).with_startup_mode(StartupMode::SkipBoot));
    machine
        .load_cartridge(build_intr_2_sprites_probe_rom(sprite_xs, delay))
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
fn oracle_sweep_intr_2_sprites() {
    // (extra, sprite_xs) drawn from intr_2_mode0_timing_sprites.s. Real test asserts
    // round_a (delay=41+extra) -> count 1, round_b (delay=40+extra) -> count 2.
    let cases: &[(i32, Vec<u8>)] = &[
        (2, vec![0]),
        (4, vec![0, 0]),
        (5, vec![0, 0, 0]),
        (16, vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        (16, vec![1; 10]),
        (15, vec![2; 10]),
        (15, vec![3; 10]),
        (15, vec![4; 10]),
        (15, vec![7; 10]),
        (16, vec![8; 10]),
        (16, vec![9; 10]),
        (2, vec![0]),
        (1, vec![4]),
        (1, vec![5]),
        (2, vec![8]),
        (1, vec![12]),
        (27, vec![0, 8, 16, 24, 32, 40, 48, 56, 64, 72]),
    ];
    for model in [ConsoleModel::GameBoy, ConsoleModel::GameBoyColor] {
        println!("=== intr_2_mode0_timing_sprites model={model:?} (a:41+x->1, b:40+x->2) ===");
        for (extra, xs) in cases {
            let da = (41 + extra) as usize;
            let db = (40 + extra) as usize;
            let a = run_intr_2_sprites_probe(model, xs, da);
            let b = run_intr_2_sprites_probe(model, xs, db);
            // Also locate the real boundary (first delay whose count==1).
            let mut boundary = None;
            for d in db.saturating_sub(2)..=da + 3 {
                if run_intr_2_sprites_probe(model, xs, d) == Some(1) {
                    boundary = Some(d);
                    break;
                }
            }
            let pass = a == Some(1) && b == Some(2);
            println!(
                "extra={extra:3} n={:2} x={:?} a(d={da})={a:?} b(d={db})={b:?} boundary={boundary:?} {}",
                xs.len(),
                xs.first().copied().unwrap_or(0),
                if pass { "PASS" } else { "FAIL" }
            );
        }
    }
}

// ---- faithful intr_2_mode0_scxN_timing_nops probe (wilbertpol: set SCX, mode2 IRQ,
// nops delay, then a SINGLE `ld a,(STAT); and $03` read = the mode bits at that dot).
// Returns the mode bits. Real test (scx3): delay 49 -> 0x03 (mode3), delay 50 -> 0x00
// (mode0). (scx7): delay 50 -> 0x03, delay 51 -> 0x00. ----
fn build_intr_2_scx_single_read_probe_rom(scx: u8, delay: usize) -> Vec<u8> {
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
    program.extend_from_slice(&[0x3E, scx]); // ld a,scx
    program.extend_from_slice(&[0xE0, 0x43]); // ldh (SCX),a
    program.extend_from_slice(&[0x3E, 0x02]); // ld a,2
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh (IE),a = STAT
    program.extend_from_slice(&[0x21, 0x41, 0xFF]); // ld hl,$FF41 (STAT)

    program.push(0xCD); // call setup_and_wait_mode2
    let setup_operand = program.len();
    program.extend_from_slice(&[0x00, 0x00]);

    program.extend(std::iter::repeat_n(0x00, delay)); // nops delay
    program.push(0x7E); // ld a,(hl)  ; single STAT read
    program.extend_from_slice(&[0xE6, 0x03]); // and $03
    program.push(0x4F); // ld c,a  (mode bits -> c)
    program.extend_from_slice(&[0x3E, 0xAA]); // ld a,$AA
    program.push(0x57); // ld d,a  (marker)
    program.push(0x76); // halt
    let done = base + program.len() as u16;
    emit_jr_at(&mut program, done);

    // setup_and_wait_mode2 (identical to the mode0 timing probe):
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

fn run_intr_2_scx_single_read_probe(model: ConsoleModel, scx: u8, delay: usize) -> Option<u8> {
    run_intr_2_scx_single_read_probe_detailed(model, scx, delay).map(|(m, _, _, _)| m)
}

// Returns (observed_mode, line_dot_at_read, mode0_start_dot_at_read, internal_access_mode_bits).
fn run_intr_2_scx_single_read_probe_detailed(
    model: ConsoleModel,
    scx: u8,
    delay: usize,
) -> Option<(u8, u16, u16, u8)> {
    let mut machine =
        Machine::new(MachineConfig::new(model).with_startup_mode(StartupMode::SkipBoot));
    machine
        .load_cartridge(build_intr_2_scx_single_read_probe_rom(scx, delay))
        .expect("probe ROM should load");
    let mut read_at: Option<(u16, u16, u8)> = None;
    for _ in 0..3_000_000 {
        machine.step_t_cycle();
        // Keep the LAST $FF41 read (the measurement read), not the setup wait_mode polls.
        if let Some(event) = machine.cpu().last_address_event()
            && event.kind == CpuAddressEventKind::Read
            && event.access_address == Some(0xFF41)
        {
            let s = machine.ppu().snapshot();
            let internal = match s.mode {
                PpuAccessMode::HBlank => 0,
                PpuAccessMode::VBlank => 1,
                PpuAccessMode::OamScan => 2,
                PpuAccessMode::Drawing => 3,
            };
            read_at = Some((s.line_dot, s.mode0_start_dot, internal));
        }
        if machine.cpu().execution_state() == gb_core::CpuExecutionState::Halted
            && machine.cpu().registers().d == 0xAA
        {
            let (line_dot, m0, internal) = read_at?;
            return Some((machine.cpu().registers().c, line_dot, m0, internal));
        }
    }
    None
}

#[test]
#[ignore = "oracle sweep (manual run with --nocapture)"]
fn oracle_sweep_intr_2_mode0_scx_timing() {
    for model in [ConsoleModel::GameBoy, ConsoleModel::GameBoyColor] {
        for scx in 0..=8_u8 {
            // mode3->mode0 STAT readback bracket at SCX. scx3 asserts 49->mode3, 50->mode0;
            // scx7 asserts 50->mode3, 51->mode0.
            println!(
                "=== intr_2_mode0_scx{scx}_timing model={model:?} (single STAT read after delay) ==="
            );
            for delay in 44..=58 {
                match run_intr_2_scx_single_read_probe(model, scx, delay) {
                    Some(m) => println!("delay={delay:3} mode={m:#04X}"),
                    None => println!("delay={delay:3} <timeout>"),
                }
            }
        }
    }
}

// ---- internal mode3-length probe: set SCX, then sample the PPU's own
// `mode0_start_dot()` on a steady visible line (ly=70, deep HBlank). Answers whether
// the per-scx mode3 LENGTH differs between trees, or it is purely the readback skew. ----
fn build_scx_spin_rom(scx: u8) -> Vec<u8> {
    let base = PROBE_ENTRY;
    let mut program = Vec::new();
    program.extend_from_slice(&[0x3E, scx]); // ld a,scx
    program.extend_from_slice(&[0xE0, 0x43]); // ldh (SCX),a
    let spin = base + program.len() as u16;
    emit_jr_at(&mut program, spin); // jr .
    build_nom_bc_test_rom_with_program_entry(&program, 0x00, base as usize, &[])
}

fn build_sprites_spin_rom(sprite_xs: &[u8]) -> Vec<u8> {
    let base = PROBE_ENTRY;
    let mut program = Vec::new();
    // wait_ly 144 (vblank; OAM writable)
    let w144 = base + program.len() as u16;
    program.extend_from_slice(&[0xF0, 0x44]);
    program.extend_from_slice(&[0xFE, 144]);
    emit_jr_nz_at(&mut program, w144);
    program.extend_from_slice(&[0x21, 0x00, 0xFE]); // ld hl,$FE00
    for (i, &x) in sprite_xs.iter().enumerate() {
        program.extend_from_slice(&[0x3E, 0x52]); // ld a,$52 (Y)
        program.push(0x22);
        program.extend_from_slice(&[0x3E, x]); // ld a,x
        program.push(0x22);
        program.extend_from_slice(&[0x3E, 0x30 + i as u8]); // tile
        program.push(0x22);
        program.push(0xAF); // xor a
        program.push(0x22); // flags=0
    }
    program.extend_from_slice(&[0x21, 0x40, 0xFF]); // ld hl,$FF40
    program.extend_from_slice(&[0xCB, 0xCE]); // set 1,(hl)  OBJ enable
    let spin = base + program.len() as u16;
    emit_jr_at(&mut program, spin);
    build_nom_bc_test_rom_with_program_entry(&program, 0x00, base as usize, &[])
}

fn run_sprites_internal_length_probe(model: ConsoleModel, sprite_xs: &[u8]) -> Option<(u16, u16)> {
    let mut machine =
        Machine::new(MachineConfig::new(model).with_startup_mode(StartupMode::SkipBoot));
    machine
        .load_cartridge(build_sprites_spin_rom(sprite_xs))
        .expect("probe ROM should load");
    let mut last_drawing: Option<(u16, u16)> = None; // (last_drawing_dot, mode0_start_dot)
    for _ in 0..3_000_000 {
        machine.step_t_cycle();
        let s = machine.ppu().snapshot();
        if s.ly == 68 && s.mode == PpuAccessMode::Drawing {
            last_drawing = Some((s.line_dot, s.mode0_start_dot));
        }
        if s.ly == 69 && last_drawing.is_some() {
            return last_drawing;
        }
    }
    None
}

#[test]
#[ignore = "oracle sweep (manual run with --nocapture)"]
fn oracle_sweep_intr_2_sprites_internal_length() {
    let cases: &[(i32, Vec<u8>)] = &[
        (2, vec![0]),
        (4, vec![0, 0]),
        (5, vec![0, 0, 0]),
        (16, vec![0; 10]),
        (15, vec![3; 10]),
        (16, vec![8; 10]),
        (27, vec![0, 8, 16, 24, 32, 40, 48, 56, 64, 72]),
        (2, vec![8]),
    ];
    for model in [ConsoleModel::GameBoy, ConsoleModel::GameBoyColor] {
        println!(
            "=== intr_2 sprites internal mode0_start_dot model={model:?} (ly68 last-Drawing) ==="
        );
        for (extra, xs) in cases {
            match run_sprites_internal_length_probe(model, xs) {
                Some((last_draw, len)) => println!(
                    "extra={extra:3} n={:2} x={:3} last_drawing_dot={last_draw} mode0_start_dot={len} (len-252={})",
                    xs.len(),
                    xs.first().copied().unwrap_or(0),
                    len as i32 - 252
                ),
                None => println!("extra={extra} <timeout>"),
            }
        }
    }
}

fn run_scx_internal_mode0_start_dot_probe(model: ConsoleModel, scx: u8) -> Option<(u8, u16, u16)> {
    let mut machine =
        Machine::new(MachineConfig::new(model).with_startup_mode(StartupMode::SkipBoot));
    machine
        .load_cartridge(build_scx_spin_rom(scx))
        .expect("probe ROM should load");
    // Track, on ly=70, the last Drawing dot and the mode3-length at the mode3->HBlank flip.
    let mut last_drawing: Option<(u8, u16, u16)> = None; // (scx, last_drawing_dot, len_there)
    for _ in 0..3_000_000 {
        machine.step_t_cycle();
        let s = machine.ppu().snapshot();
        if s.ly == 70 && s.mode == PpuAccessMode::Drawing {
            last_drawing = Some((s.scx, s.line_dot, s.mode0_start_dot));
        }
        if s.ly == 71 {
            return last_drawing;
        }
    }
    None
}

#[test]
#[ignore = "oracle sweep (manual run with --nocapture)"]
fn oracle_sweep_intr_2_mode0_scx_internal_length() {
    for model in [ConsoleModel::GameBoy, ConsoleModel::GameBoyColor] {
        println!("=== intr_2 internal mode0_start_dot model={model:?} (ly70 last-Drawing) ===");
        for scx in 0..=8_u8 {
            match run_scx_internal_mode0_start_dot_probe(model, scx) {
                Some((rscx, last_draw, len)) => println!(
                    "scx_set={scx} scx_seen={rscx} last_drawing_dot={last_draw} mode0_start_dot={len}"
                ),
                None => println!("scx={scx} <timeout>"),
            }
        }
    }
}

#[test]
#[ignore = "oracle sweep (manual run with --nocapture)"]
fn oracle_sweep_intr_2_mode0_scx_timing_detailed() {
    // For each scx, print observed STAT mode + the raster state at the read instant, so a
    // canonical readback model can be derived offline. main column (oracle, §24.21):
    // scx0..8 mode0@nop = 49,50,50,50,51,51,51,51,50.
    for model in [ConsoleModel::GameBoy] {
        for scx in 0..=8_u8 {
            println!(
                "=== scx{scx} model={model:?} (delay, observed, line_dot@read, mode0_start) ==="
            );
            for delay in 46..=54 {
                match run_intr_2_scx_single_read_probe_detailed(model, scx, delay) {
                    Some((m, ld, m0, internal)) => println!(
                        "delay={delay:3} observed={m:#04X} line_dot@read={ld:3} mode0_start={m0} internal={internal}"
                    ),
                    None => println!("delay={delay:3} <timeout>"),
                }
            }
        }
    }
}

// ---- faithful intr_2_oam_ok_timing probe (mooneye: mode2 IRQ, nops, poll OAM read
// until it returns 0 = OAM accessible / HBlank). OAM[0] is cleared to 0 in vblank;
// while blocked the read returns 0xFF. Returns count register c. Real test asserts
// delay 46 -> 1, delay 45 -> 2. ----
fn build_intr_2_oam_poll_probe_rom(delay: usize) -> Vec<u8> {
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
    // OAM[0] = 0 (writable in vblank). hl stays $FE00 for the poll loop.
    program.extend_from_slice(&[0x21, 0x00, 0xFE]); // ld hl,$FE00 (OAM)
    program.push(0xAF); // xor a
    program.push(0x77); // ld (hl),a   OAM[0]=0
    program.extend_from_slice(&[0x3E, 0x02]); // ld a,2
    program.extend_from_slice(&[0xE0, 0xFF]); // ldh (IE),a = STAT

    program.push(0xCD); // call setup_and_wait_mode2 (preserves hl=$FE00)
    let setup_operand = program.len();
    program.extend_from_slice(&[0x00, 0x00]);

    program.extend(std::iter::repeat_n(0x00, delay)); // nops delay
    program.push(0x06); // ld b,$00
    program.push(0x00);
    let loop_pc = base + program.len() as u16;
    program.push(0x04); // inc b
    program.push(0x7E); // ld a,(hl)  ; read OAM[0]
    program.extend_from_slice(&[0xE6, 0xFF]); // and $FF  (Z if a==0 = accessible)
    emit_jr_nz_at(&mut program, loop_pc); // jr nz,loop

    program.push(0x48); // ld c,b  (count -> c)
    program.extend_from_slice(&[0x3E, 0xAA]); // ld a,$AA
    program.push(0x57); // ld d,a  (marker)
    program.push(0x76); // halt
    let done = base + program.len() as u16;
    emit_jr_at(&mut program, done);

    // setup_and_wait_mode2 (identical to the mode0 timing probe):
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

fn run_intr_2_oam_poll_probe(model: ConsoleModel, delay: usize) -> Option<u8> {
    let mut machine =
        Machine::new(MachineConfig::new(model).with_startup_mode(StartupMode::SkipBoot));
    machine
        .load_cartridge(build_intr_2_oam_poll_probe_rom(delay))
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
fn oracle_sweep_intr_2_oam_ok_timing() {
    for model in [ConsoleModel::GameBoy, ConsoleModel::GameBoyColor] {
        // intr_2_oam_ok_timing: mode2 IRQ -> nops -> poll OAM until accessible (==0).
        // Real test asserts delay 46 -> count 1, delay 45 -> count 2.
        println!("=== intr_2_oam_ok_timing model={model:?} (poll OAM until accessible) ===");
        for delay in 42..=49 {
            match run_intr_2_oam_poll_probe(model, delay) {
                Some(c) => println!("delay={delay:3} count={c:#04X}"),
                None => println!("delay={delay:3} <timeout>"),
            }
        }
    }
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
