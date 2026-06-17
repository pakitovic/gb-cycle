use super::*;

#[test]
#[ignore = "diagnostic case1 pre-read cpu-visible stat probe against the real mooneye ROM"]
fn cpu_stat_read_logs_case1_pre_read_state_against_real_rom() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    for _ in 0..10_000_000 {
        let cpu_before = machine.cpu().snapshot();
        if machine.read_bus(0xFF80) == 1
            && cpu_before.registers.pc == 0x0B9C
            && cpu_before.current_opcode == Some(0xF0)
            && matches!(
                cpu_before.execution_state,
                crate::CpuExecutionState::Execute { step: 2, .. }
            )
        {
            let ppu_before = machine.ppu().snapshot();
            let stat_before = machine
                .ppu()
                .read_register_with_source(0xFF41, PpuRegisterReadSource::CpuBusOperation);
            machine.step_t_cycle();
            let cpu_after = machine.cpu().snapshot();
            let ppu_after = machine.ppu().snapshot();
            let activity = cpu_after
                .last_bus_activity
                .expect("the next t-cycle should perform the FF41 read");
            println!(
                "case1_pre_read_probe stat_before={:#04X} before_pc={:#06X} before_ly={} before_line_dot={} before_mode={:?} before_mode0_start_dot={} before_x={} before_vpo={} after_value={:#04X} after_pc={:#06X} after_ly={} after_line_dot={} after_mode={:?} after_mode0_start_dot={} after_x={} after_vpo={}",
                stat_before,
                cpu_before.registers.pc,
                ppu_before.ly,
                ppu_before.line_dot,
                ppu_before.mode,
                ppu_before.mode0_start_dot,
                ppu_before.bg_current_transfer_x,
                ppu_before.visible_pixels_output,
                activity.value,
                cpu_after.registers.pc,
                ppu_after.ly,
                ppu_after.line_dot,
                ppu_after.mode,
                ppu_after.mode0_start_dot,
                ppu_after.bg_current_transfer_x,
                ppu_after.visible_pixels_output,
            );
            assert_eq!(activity.address, 0xFF41);
            return;
        }

        machine.step_t_cycle();
    }

    panic!("probe did not reach the testcase 1 pre-read state");
}

#[test]
#[ignore = "diagnostic helper conditions at the real first FF41 read for testcase 1"]
fn cpu_stat_read_logs_case1_first_read_helper_conditions_against_real_rom() {
    let rom_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../test/mooneye/acceptance/ppu/intr_2_mode0_timing_sprites.gb");
    let rom = std::fs::read(&rom_path)
        .expect("mooneye intr_2_mode0_timing_sprites ROM should be present");
    let mut machine = Machine::new(
        MachineConfig::new(ConsoleModel::GameBoy).with_startup_mode(StartupMode::SkipBoot),
    );
    machine.load_cartridge(rom).expect("probe ROM should load");

    let mut saw_irq_for_case1 = false;

    for _ in 0..10_000_000 {
        machine.step_t_cycle();

        if machine.read_bus(0xFF80) != 1 {
            continue;
        }

        if !saw_irq_for_case1
            && matches!(
                machine.cpu().execution_state(),
                crate::CpuExecutionState::ServiceInterrupt {
                    source: crate::InterruptSource::LcdStat,
                    ..
                }
            )
        {
            saw_irq_for_case1 = true;
        }

        let cpu_snapshot = machine.cpu().snapshot();
        if saw_irq_for_case1
            && let Some(activity) = cpu_snapshot.last_bus_activity
            && activity.kind == crate::CpuBusAccessKind::DataRead
            && activity.address == 0xFF41
        {
            let ppu = machine.ppu();
            let published_mode = ppu.access_mode_for_line_dot(ppu.line_dot - 1);
            let current_mode = ppu.access_mode_for_line_dot(ppu.line_dot);
            let current_transfer = ppu.current_transfer();
            let transfer_lane = current_transfer.map(|transfer| transfer.context.lane);
            let transfer_source_window =
                current_transfer.map(|transfer| transfer.context.source_window);
            println!(
                "case1_first_read_helper value={:#04X} pc={:#06X} line_dot={} ly={} published_mode={:?} current_mode={:?} current_mode0_start_dot={} blank_frame_active={} obj_stage={:?} pending_match_x={:?} pending_hit_len={} transfer_lane={:?} transfer_source_window={:?} current_transfer_x={} visible_pixels_output={} fifo_contains_real_pixels={} fifo_len={} line_dot_plus_one_eq_mode0={} ly_visible={} obj_idle={} no_pending_match={} no_pending_hits={}",
                activity.value,
                cpu_snapshot.registers.pc,
                ppu.line_dot,
                ppu.ly,
                published_mode,
                current_mode,
                ppu.current_mode0_start_dot(),
                ppu.blank_frame_active,
                ppu.obj_pipeline_state.fetch.stage,
                ppu.obj_pipeline_state.pending_match_x,
                ppu.obj_pipeline_state.pending_sprite_slots.len(),
                transfer_lane,
                transfer_source_window,
                ppu.bg_pipeline_state.current_transfer_x,
                ppu.bg_pipeline_state.visible_pixels_output,
                ppu.bg_pipeline_state.fifo_contains_real_pixels(),
                ppu.bg_pipeline_state.fifo.len(),
                ppu.line_dot + 1 == ppu.current_mode0_start_dot(),
                ppu.ly < VISIBLE_SCANLINES,
                ppu.obj_pipeline_state.fetch.stage == PpuObjFetcherStage::Idle,
                ppu.obj_pipeline_state.pending_match_x.is_none(),
                ppu.obj_pipeline_state.pending_sprite_slots.is_empty(),
            );
            return;
        }
    }

    panic!("probe did not reach the testcase 1 first FF41 read");
}

#[test]
#[ignore = "diagnostic probe for ashiepaws strikethrough line 68 DMA/OBJ overlap"]
fn sample_real_ashiepaws_strikethrough_line68_dma_obj_overlap() {
    for target_ly in 64..=72 {
        let (selected_sprites, events, segment, framebuffer_segment) =
            sample_ashiepaws_strikethrough_line(target_ly, 64);

        println!("ly={target_ly} selected_sprites={selected_sprites:#?}");
        println!("ly={target_ly} line_pixels_71_79={segment:?}");
        println!("ly={target_ly} framebuffer_71_79={framebuffer_segment:?}");
        for event in &events {
            println!("ly={target_ly} {event:?}");
        }
    }

    let (selected_sprites, events, _, _) = sample_ashiepaws_strikethrough_line(68, 64);
    assert!(!selected_sprites.is_empty() || !events.is_empty());
}

// ====================================================================
// Cut 1.0 GROUNDING PROBE (§24.25 rephase) — TEMPORARY, revert at Cut 1 close.
// Bare-rig per-dot phase scan: dumps, for each dot around the end-of-line /
// vblank-entry / line-153 transitions, the internal raster (ly, line_dot) and
// every CPU-observable readback (LY with the +1 lead, STAT mode, LYC coincidence).
// Run on THIS branch and on ../gb-cycle-main and diff the output to pin:
//   D = line_dot where the observable LY first reads N+1 on a normal line
//   K = line_dot where the observable LY first reads 0 on line 153
// and to confirm main resolves LY+STAT+LYC as one consistent post-tick phase.
// Sample point: AFTER tick() (post-tick settled PPU state). The machine-level
// pre-tick CPU read offset is cross-checked separately by the ROM probe (1.0b).
// ====================================================================

fn cut1_grounding_setup(model: ConsoleModel, lyc: u8) -> PpuTestRig {
    let mut rig = PpuTestRig::with_model(model);
    rig.apply_startup_state(PpuStartupState {
        lcdc: 0x91,
        stat: STAT_LYC_INTERRUPT_ENABLE_BIT,
        scy: 0x00,
        scx: 0x00,
        ly: 0,
        lyc,
        bgp: 0xFC,
        wy: 0x00,
        wx: 0x00,
        obj_palette_read_policy: DmgObjPaletteReadPolicy::ReadAsFfUntilWritten,
    });
    rig.lcd_restart_phase = PpuLcdRestartPhase::Inactive;
    rig.blank_frame_active = false;
    rig.stat_state.irq_line = false;
    // Settle two frames so the startup latch / restart phase clear.
    rig.tick_n(2 * TOTAL_SCANLINES as u64 * DOTS_PER_SCANLINE as u64);
    rig
}

fn cut1_dump_phase(model: ConsoleModel, label: &str, target_ly: u8, lyc: u8) {
    let mut rig = cut1_grounding_setup(model, lyc);
    let mut guard = 0u64;
    while !(rig.ly == target_ly && rig.line_dot == 444) {
        rig.tick();
        guard += 1;
        assert!(guard < 4 * TOTAL_SCANLINES as u64 * DOTS_PER_SCANLINE as u64);
    }
    let model_tag = if model.is_cgb_family() { "CGB" } else { "DMG" };
    // ~24 dots: tail of target line (444..=455) + head of next line (0..=16).
    for _ in 0..(12 + 1 + 17) {
        rig.tick();
        let ly = rig.ly;
        let line_dot = rig.line_dot;
        if !(line_dot >= 444 || line_dot <= 16) {
            continue;
        }
        let obs_ly = rig.read_ly(PpuRegisterReadSource::CpuBusOperation);
        let stat_mode = rig.cpu_visible_stat_mode();
        let internal_mode = rig.current_access_mode();
        let lyc_coin = rig.live_lyc_coincidence();
        println!(
            "CUT1GND {model_tag} {label} ly={ly:>3} dot={line_dot:>3} | obs_ly={obs_ly:>3} stat_mode={stat_mode:?} internal_mode={internal_mode:?} lyc_coin={lyc_coin}"
        );
    }
}

#[test]
#[ignore = "Cut 1.0 real-ROM trace: ly143_144_mode3_0 FF41/FF44 reads; revert after"]
fn cut1_trace_ly143_144_mode3_0() {
    for (name, model) in [
        ("ly143_144_mode3_0-GS", ConsoleModel::GameBoy),
        ("ly143_144_mode3_0-C", ConsoleModel::GameBoyColor),
    ] {
        let candidates = [
            format!(
                "{}/../../test/wilbertpol/wilbertpol/acceptance/gpu/{name}.gb",
                env!("CARGO_MANIFEST_DIR")
            ),
            format!(
                "{}/../../test/wilbertpol/wilbertpol/acceptance/gpu/ly143_144_mode3_0.gb",
                env!("CARGO_MANIFEST_DIR")
            ),
        ];
        let rom = candidates
            .iter()
            .find_map(|p| std::fs::read(p).ok())
            .expect("ly143_144_mode3_0 ROM present");
        let mut machine =
            Machine::new(MachineConfig::new(model).with_startup_mode(StartupMode::SkipBoot));
        machine.load_cartridge(rom).expect("rom loads");
        let tag = if model.is_cgb_family() { "CGB" } else { "DMG" };
        let mut count = 0u32;
        for _ in 0..3_000_000 {
            machine.step_t_cycle();
            let cpu = machine.cpu().snapshot();
            if let Some(act) = cpu.last_bus_activity
                && act.kind == crate::CpuBusAccessKind::DataRead
                && (act.address == 0xFF41 || act.address == 0xFF44)
            {
                let s = machine.ppu().snapshot();
                if (142..=145).contains(&s.ly) {
                    println!(
                        "L143TRACE {tag} addr={:#06X} val={:#04X} ly={} dot={} mode={:?}",
                        act.address, act.value, s.ly, s.line_dot, s.mode
                    );
                    count += 1;
                    if count > 60 {
                        break;
                    }
                }
            }
            if machine.cpu().execution_state() == crate::CpuExecutionState::Halted {
                break;
            }
        }
    }
}

// ====================================================================
// §24.25.3 RESUME PROBE — differential dispatch-IRQ trace (TEMPORARY).
// Runs a failing wilbertpol ROM and dumps the full STAT-IRQ <-> STAT-readback
// relationship around the relevant transitions, so the A'-branch vs main offset
// between the IRQ latch (CPU-visible) and the now-shifted readback can be pinned.
// Per relevant t-cycle it prints, on ANY of:
//   - STAT (0x02) raw/visible PPU-pending change (PENDdelta)
//   - ServiceInterrupt{LcdStat} rising edge (SERVICE)
//   - CPU DataRead of IF/STAT/LY (0xFF0F/0xFF41/0xFF44) (READ)
//   - terminal magic breakpoint 0x40 (RESULT, with the fibonacci registers)
// the (ly, line_dot, mode), the committed IF (`interrupts().read_if()`), the raw
// vs cpu-visible PPU pending masks, the read value, and pc.
// Copy this file verbatim into ../gb-cycle-main and run the same test to diff.
// ====================================================================

fn irq_dispatch_trace(rom_relpath: &str, model: ConsoleModel, max_cycles: u64) {
    let rom_path = format!("{}/../../{rom_relpath}", env!("CARGO_MANIFEST_DIR"));
    let rom = std::fs::read(&rom_path).unwrap_or_else(|_| panic!("ROM present: {rom_path}"));
    let mut machine =
        Machine::new(MachineConfig::new(model).with_startup_mode(StartupMode::SkipBoot));
    machine.load_cartridge(rom).expect("rom loads");
    let tag = if model.is_cgb_family() { "CGB" } else { "DMG" };

    let mut out: Vec<String> = Vec::new();
    let mut prev_iff = 0u8;
    let mut prev_ff44 = 0xFFu16; // sentinel; dedups the busy-poll on FF44 (log only on change)
    let mut terminal = "(none)";

    for cycle in 0u64..max_cycles {
        machine.step_t_cycle();

        // Snapshot every cycle: these readback ROMs disable interrupts and busy-poll FF44,
        // so the signal is the register READS (need last_bus_activity), not IRQ edges.
        let cpu = machine.cpu().snapshot();
        let exec = cpu.execution_state;
        let iff = machine.interrupts().read_if();

        let read = cpu.last_bus_activity.filter(|a| {
            a.kind == crate::CpuBusAccessKind::DataRead
                && matches!(a.address, 0xFF0F | 0xFF41 | 0xFF44)
        });

        // FF44 (LY) reads: log only when the read VALUE changes (the busy-poll exit shows
        // as a value transition). FF41/FF0F: always. Also log any IF lower-bit change.
        let iff_change = (iff & 0x03) != (prev_iff & 0x03);
        prev_iff = iff;
        let log_read = match read {
            Some(a) if a.address == 0xFF44 => {
                let changed = a.value as u16 != prev_ff44;
                prev_ff44 = a.value as u16;
                changed
            }
            Some(_) => true,
            None => false,
        };

        if log_read || iff_change {
            let (raddr, rval) = read.map(|a| (a.address, a.value)).unwrap_or((0, 0));
            let mode = machine.ppu().current_access_mode();
            out.push(format!(
                "T {tag} ly={:>3} dot={:>3} mode={mode:?} IF={iff:#04X} | rd={raddr:#06X}:{rval:#04X} pc={:#06X} @{cycle}",
                machine.ppu().ly(),
                machine.ppu().line_dot(),
                cpu.registers.pc,
            ));
        }

        // Terminal: legacy magic breakpoint 0xED or 0x40 in Execute -> dump fib registers.
        if matches!(cpu.current_opcode, Some(0x40) | Some(0xED))
            && matches!(exec, crate::CpuExecutionState::Execute { .. })
        {
            let r = &cpu.registers;
            let pass = [r.b, r.c, r.d, r.e, r.h, r.l] == [3, 5, 8, 13, 21, 34];
            out.push(format!(
                "T {tag} RESULT pass={pass} B={:#04X} C={:#04X} D={:#04X} E={:#04X} H={:#04X} L={:#04X} pc={:#06X} @{cycle}",
                r.b, r.c, r.d, r.e, r.h, r.l, r.pc
            ));
            terminal = "magic";
            break;
        }
        if exec == crate::CpuExecutionState::Halted {
            out.push(format!("T {tag} HALTED @{cycle}"));
            terminal = "halt";
            break;
        }
    }

    let path = format!("/tmp/irq_trace_{}_{tag}.txt", trace_rom_tag(rom_relpath));
    std::fs::write(&path, out.join("\n")).expect("write trace");
    println!(
        "IRQT {tag} wrote {} lines to {path} (terminal={terminal})",
        out.len()
    );
}

fn trace_rom_tag(rom_relpath: &str) -> String {
    rom_relpath
        .rsplit('/')
        .next()
        .unwrap_or(rom_relpath)
        .trim_end_matches(".gb")
        .to_string()
}

const LY143_ROM: &str = "test/wilbertpol/wilbertpol/acceptance/gpu/ly143_144_mode3_0.gb";
const SPRITES_NOPS_ROM: &str =
    "test/wilbertpol/wilbertpol/acceptance/gpu/intr_2_mode0_timing_sprites_nops.gb";
const LY00_ROM: &str = "test/wilbertpol/wilbertpol/acceptance/gpu/ly00_mode3_0.gb";

#[test]
#[ignore = "§24.25.5 Cut R0: ly00_mode3_0 (PASSES) DMG re-enable line_dot grounding; copy to main + diff"]
fn irq_trace_ly00_mode3_0_dmg() {
    irq_dispatch_trace(LY00_ROM, ConsoleModel::GameBoy, 1_500_000);
}

#[test]
#[ignore = "§24.25.3 differential IRQ trace: ly143_144_mode3_0 DMG; copy to main + diff"]
fn irq_trace_ly143_144_mode3_0_dmg() {
    irq_dispatch_trace(LY143_ROM, ConsoleModel::GameBoy, 1_500_000);
}

#[test]
#[ignore = "§24.25.3 differential IRQ trace: ly143_144_mode3_0 CGB; copy to main + diff"]
fn irq_trace_ly143_144_mode3_0_cgb() {
    irq_dispatch_trace(LY143_ROM, ConsoleModel::GameBoyColor, 1_500_000);
}

#[test]
#[ignore = "§24.25.3 differential IRQ trace: intr_2_mode0_timing_sprites_nops DMG; copy to main + diff"]
fn irq_trace_intr_2_sprites_nops_dmg() {
    irq_dispatch_trace(SPRITES_NOPS_ROM, ConsoleModel::GameBoy, 2_200_000);
}

#[test]
#[ignore = "§24.25.3 differential IRQ trace: intr_2_mode0_timing_sprites_nops CGB; copy to main + diff"]
fn irq_trace_intr_2_sprites_nops_cgb() {
    irq_dispatch_trace(SPRITES_NOPS_ROM, ConsoleModel::GameBoyColor, 1_500_000);
}

#[test]
#[ignore = "Cut 1.0 grounding probe (manual run with --nocapture); revert at Cut 1 close"]
fn cut1_grounding_internal_ly_phase() {
    for model in [ConsoleModel::GameBoy, ConsoleModel::GameBoyColor] {
        // Normal visible line: where does observable LY flip 40->41 (= D)?
        cut1_dump_phase(model, "normal(ly40,lyc41)", 40, 41);
        // VBlank entry: 143->144 mode + LY + (mode2/vblank IRQ edge context).
        cut1_dump_phase(model, "vblank_entry(ly143,lyc144)", 143, 144);
        // Line 153 wrap: where does observable LY flip 153->0 (= K), LYC153/LYC0.
        cut1_dump_phase(model, "line153(ly152,lyc153)", 152, 153);
        cut1_dump_phase(model, "line153(ly152,lyc0)", 152, 0);
    }
}
