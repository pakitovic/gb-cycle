# CGB CPU↔PPU Timing Redesign — Design Doc (working)

> **STATUS: DESIGN DOC for a future workstream. Not started.** This is the plan for the
> coordinated CGB timing rework that the PPU-hardening campaign (`04-ppu-fix.md`) proved is
> required to close `mealybug-cgb-m3-scy-change`. It supersedes the scoped-fix directions for
> that ROM. Durable open-work ledger stays in [`docs/TODO.md`](../TODO.md); hardware contract in
> [`docs/hardware/PPU.md`](../hardware/PPU.md). Oracles: SameBoy (`$HOME/workspace/SameBoy`,
> read-only) and DocBoy (`$HOME/workspace/docboy`, modifiable for instrumentation, revert at
> close). Agent-agnostic (Claude + Codex).

## 1. Problem statement

`mealybug-cgb-m3-scy-change` is the **only** failing case in the CGB mealybug suite
(**23/24**; 121 mismatching px after the seed fix below). DMG `m3_scy_change` passes
(shootout 264/264). The ROM is a diagonal-skew test: it disables then re-enables the LCD, then
writes SCY once per tile during Mode 3 (an unrolled `ld (hl),a` run) so each tile reads a
different `(scy+ly)%8` row.

The campaign established — by elimination, with reproducible experiments — that the residual is
**not** a PPU-mode3 fetcher bug. It is a **CGB CPU↔PPU phase offset of ~4 dots** entangled with
the CGB SCY write-observation latency and the startup fetch timing. No single timing lever moves
it. Closing it needs a **coordinated CGB timing redesign**, validated against the full CGB suite.

**Gate to declare done:** `cargo rom-report mealybug-tearoom-tests` CGB suite **24/24**, with
blargg 58, mooneye 113, wilbertpol 117, gb-emulator-shootout 264, DMG mealybug all still green,
and `cargo fmt-check`/`cargo lint`/`cargo tests` clean.

## 2. What is landed (do not redo)

- **Seed fix — LANDED, commit `4495a546`.** `BgFetcherState.cgb_startup_seed_get_tile_scy_row`
  captured at `advance_bg_fetcher_tile_index_dot1` (CGB + `AlignmentSeedPending`), carried to
  `BgCachedSlice.cgb_startup_frozen_tile_row` via `from_fetcher`, consumed in
  `recompute_live_background_cached_slice` (state.rs) instead of `current_scanline_tile_row()`.
  Closes the ly0–7 band (CGB m3_scy_change **156→121px**). CPU-invisible (no machine-trace
  fixture regen), CGB-scoped, zero regression. It works **only** because the seed lands in the
  stable scy=0 region where the ±4/±2 offsets don't cross a write boundary; it is NOT a model of
  the underlying phase and **MUST be deleted by P3 (§9), not left beside the new path** — its
  survival past the redesign means the redesign failed its anti-seam purpose.

## 3. Verified facts (numbers; instrumentation in §8)

PPU dot origin is aligned between gb-cycle `line_dot` and DocBoy `dots` (M0: mode3 enters @80 in
both; first visible pixel ~92 vs 91; mode0 boundary 252 vs 251 — all ±1).

- **The failing pixels are the first two screen tiles (x0–15) across 4 line bands** (ly0–7,
  23–31, 72–79, 136–143). Per line exactly ONE startup tile is wrong; the steady tiles (x≥16) are
  already CGB-correct. The failing tile tracks a **per-line sprite** (sprite x = ly/8); the obj
  fetch stalls the BG fetcher just before that tile.
- **DocBoy CGB framebuffer == the mealybug fixture exactly** (verified ly 0/24/28/74) — DocBoy is
  a faithful oracle. DocBoy is built without a boot ROM (`ENABLE_BOOTROM=OFF`) yet reproduces the
  hardware fixture, so the hardware CPU↔PPU phase is what DocBoy's direct-boot produces.
- **The CPU writes SCY at identical `line_dot`s in gb-DMG and gb-CGB**: `@80→0, 88→1, 96→2,
  104→3, 112→4, 120→3, …` (every 8 dots from dot80). gb-DMG passes m3_scy_change with exactly
  these writes — so gb's CPU↔PPU phase is console-consistent and correct for DMG.
- **DocBoy (hardware CGB) writes SCY 4 dots earlier**: `@dots 76→0, 84→1, 92→2, 100→3, 108→4, …`
  (verified: DocBoy `write_scy` emits val=0 @dots76, 4 dots BEFORE mode3@80; gb writes @80, AT
  mode3). With CGB's `pending_write` (+2), DocBoy's PPU OBSERVES `@78,86,94,102,…` = gb−2.
- ⇒ **Hardware CGB runs the CPU ~4 dots ahead of the PPU vs DMG; gb-cycle CGB uses the DMG
  phase.** This is the root −4-dot CPU↔PPU offset. It is set at the **LCD re-enable / restart**
  (the LCD is on at the SkipBoot handoff, the ROM disables then re-enables it; the phase is fixed
  at the restart, not the boot handoff).

DocBoy's CGB model (the hardware-true target), from `$HOME/workspace/docboy` `ppu.cpp`:
1. `write_scy` sets `pending_write.scy.countdown = 2` (ppu.cpp ~3537): the PPU's `scy` register
   updates 2 T-cycles after the CPU write (`tick_pending_write`, end of each `tick()`).
2. The fetcher latches `bwf.scy = scy` **once at GetTile0** (ppu.cpp ~2004) and uses it for both
   tile-data planes (ppu.cpp ~2600). No mid-tile SCY change on CGB. (DMG re-reads `scy` live per
   byte — that's why the DMG bitplane-desync works and DMG passes.)

## 4. Refuted scoped fixes (do NOT repeat — all measured, all reverted)

| # | Experiment | Result |
|---|---|---|
| a | Neutralize `dmg_single_selected_sprite_phase_policy` (drop LCDC obj-phase tables) | broke 5 sprite-coupled LCDC ROMs (DMG+CGB) — tables encode real obj-stall-coupled behavior |
| b | Broaden the seed GetTile0-capture to all startup tiles | 272px (worse; multiple TileIndex passes per tile in the seam overwrite the seed) |
| c | Keep the fetched row for `StartupContinuation` slices (skip recompute) | 225px (worse; the fetched row is wrong for most sprite-x) |
| d | `apply_cgb_boot_handoff_raster_correction(4)` on CGB SkipBoot path | no effect on SCY-write dots (the restart resets the handoff phase) |
| e | CGB `CPU_LCDC_ENABLE_EFFECT_DELAY_T_CYCLES + 4` | no effect on SCY-write dots |
| f | `CGB_LINE_153_LY_READ_ZERO_DOT` 8→4 | no effect (the ROM never reads LY at the wrap) |
| g | `CGB_DEFAULT_DIRECT_BOOT_SYSTEM_COUNTER` 0x2674→0x2678 (+4) | no effect on SCY-write dots |
| h | Feed the recompute the 1-dot-delayed `pipeline.scy` for CGB (observation latency) | 228px (worse; the recompute is already too LATE = output-dot scy) |

**Conclusion:** the −4-dot CPU↔PPU phase, the SCY 2-dot observation latency, and the fetcher
startup timing are entangled; the SCY writes land @80,88,96,104 robustly regardless of
handoff/enable/line153/system-counter, and the only latency that would help must move the fetch
sample *earlier* (toward the obj-stall-delayed GetTile0), which the capture experiments showed is
wrong for low-x sprites. Hence: coordinated redesign, not a lever.

## 5. The three coupled components to model together

1. **CGB LCD-enable → PPU-start phase (the −4-dot root) — model the relationship, never a constant.**
   `enter_lcd_enabled_restart_state` (irq.rs) sets `line_dot = LCD_REENABLE_INITIAL_LINE_DOT (=0)`;
   the `LCD_REENABLE_LINE0_*` constants (ppu.rs:61-67: re-enable line0 is `DOTS_PER_SCANLINE-8`,
   mode3 starts at `MODE2_DOTS-8`) are **shared DMG/CGB** and pinned by the wilbertpol
   `lcd_restart`/`intr_2_*` suite. gb-cycle today carries a DMG-specific enable/handoff phase family
   (`DMG0_DIRECT_BOOT_HANDOFF_PPU_PHASE_BASE_OFFSET_DOTS`,
   `DMG_REAL_BOOT_POWER_ON_LCD_ENABLE_INITIAL_LINE_DOT`, ppu.rs:58/62) but **zero CGB equivalents** —
   the CGB path inherits the DMG phase. That absence is the defect.
   - **The deliverable is the phase MECHANISM, not a magnitude.** The −4 MUST *emerge* from a
     CGB-specific model of "when does the PPU restart/start relative to the CPU's LCDC-enable write"
     — a CGB enable-effect-delay / first-frame-after-enable length / enable→PPU-start offset family
     that mirrors the existing DMG constants. **A bare `apply_cgb_correction(4)` — or any
     `is_cgb_family()`-gated `+4` literal justified only by "it lands the SCY writes on DocBoy's
     dots" — is FORBIDDEN.** That is a curve-fit constant wearing a phase-model costume and would
     reintroduce exactly the seam culture this campaign exists to remove.
   - **It must hold at BOTH enable sites.** The phase is exposed here at the LCD re-enable only
     because `m3_scy_change` disables+re-enables the LCD (which is why exp d's boot-handoff
     correction had "no effect" — the restart masked it). "No effect on this ROM" ≠ "the boot phase
     is right". The corrected model must produce the hardware CPU↔PPU phase at the direct-boot
     handoff AND at every LCD re-enable, verified against the oracle at both — so the −4 is a real
     enable-phase model, not a restart-only patch.
   - **Constraints:** WITHOUT moving the DMG phase (DMG passes — change is `is_cgb_family()`-gated)
     and WITHOUT retuning the frozen #245 sprite penalty. Experiments d/e/g each moved ONE candidate
     (boot-handoff correction / LCDC-enable effect delay / direct-boot system counter) with no
     effect on the SCY-write dots, so P0 must instrument the enable→restart relationship directly
     (PPU restart/start dot vs CPU LCDC-enable-write dot) and name which physical quantity carries
     the −4 before any code lands.
2. **CGB register write-observation latency — port DocBoy's uniform `pending_write`, collapse the
   existing SCY mechanisms into it.** DocBoy models this as ONE uniform per-register deferred-write
   structure: `pending_write.{lcdc,scy,scx,wx,stat}`, each with a `countdown` decremented in
   `tick_pending_write` at the end of every `tick()` (ppu.cpp:703-726); SCY's countdown is 2 on CGB
   so the PPU observes the write 2 T-cycles late. This is `04-ppu-fix.md` M2-step-1, never
   implemented.
   - **Adopt the uniform structure, not a SCY-only special case.** Add a single CGB
     write-observation register (per-MMIO-reg countdown, serialized in save-state like DocBoy
     ppu.cpp:2938+) sized to feed LCDC/SCX/WX as well — because the SAME mechanism is what lets
     `04-ppu-fix.md` M2/M3 delete the LCDC.3/4/0/1/2 observation tables. A CGB-SCY-only delayed
     value would be a fourth seam; the uniform register is the foundation that pays forward.
   - **Collapse, do not accrete.** gb-cycle's SCY observation is today split across THREE
     overlapping mechanisms — the 2-deep `visible`/`pipeline` latch shift (registers.rs:25-30) with
     its `bg_scy_*_row_changed` / `current_scy_tile_data_row` helpers (mode3_latches.rs:183-204), the
     raw-live recompute via `current_scanline_tile_row()` (live scy at the output dot, state.rs:3430),
     and the landed seed-fix frozen row (`cgb_startup_frozen_tile_row`, state.rs:3429). The
     pending-write register must become the SINGLE source the fetcher reads; these three reduce to
     it. (The latch shift's deepest stage is only 1 dot old, so it physically *cannot* express the
     2-dot CGB delay — that is why a dedicated register is required, not an extension of the shift.)
   - **Sign bookkeeping:** applied alone the latency pushes the wrong way (exp h: +2), so it MUST
     land together with the phase model (1): phase gives −4, latency gives +2, net −2 = DocBoy
     (gb-cycle's raw observation must end up 2 dots *earlier* than today).
3. **CGB fetcher startup sampling = DocBoy's "latch SCY once at GetTile0".** Once (1)+(2) put the
   SCY schedule on the hardware dots, the BG fetcher samples SCY once per tile at GetTile0 (CGB) —
   reading the pending-write register from (2) — rather than via the per-pixel recompute + the
   `cgb_dmg_software_startup_visible_tile2/3_*` obj-phase retarget tables (mode3_policies.rs:983/1009,
   transfer.rs `compute_startup_visible_*`). DMG keeps its live-per-byte read (the bitplane desync
   that makes DMG pass); the GetTile0 latch is `is_cgb_family()`-gated because it is a real silicon
   difference, NOT a test patch. **These tables AND the landed seed fix
   (`cgb_startup_seed_get_tile_scy_row` / `cgb_startup_frozen_tile_row`) are DELETED here, not left
   beside the new path** — the seed fix is the embryo of this mechanism and its survival past P3
   means the redesign failed (hard gate in §9 P3). Do NOT extend the tables (forbidden curve-fit per
   TODO.md).

## 6. Scope, non-goals, hard constraints

- **In scope:** CGB LCD-re-enable restart phase, CGB SCY (then LCDC/SCX/WX) write-observation
  latency, CGB BG-fetcher GetTile0 SCY sampling, removal of the CGB SCY obj-phase retarget tables.
- **Non-goals / HARD CONSTRAINTS:**
  - **Never refit the frozen #245 sprite penalty** (`obj_fetch.rs:88` `alignment_stall_remaining`,
    6..11 per sprite). Prove per-sprite Mode 3 cost is byte-identical before/after via the probe.
  - **Do not move the DMG phase** — DMG `m3_scy_change` and all DMG timing ROMs pass; the change
    must be `console_model.is_cgb_family()`-gated.
  - **Do not retune the observation tables** to chase green; replace them with live reads only
    after (1)+(2) land dot-exact.
  - **No magic phase constant.** The −4 CPU↔PPU offset must emerge from a CGB enable→PPU-start model
    (§5.1); an `is_cgb_family()`-gated `+4` / `correction(4)` literal whose only justification is "it
    makes m3_scy_change pass" is FORBIDDEN — it is the same curve-fit seam in a new costume.
  - **Collapse, never accrete (net seam count must go DOWN).** Each component REPLACES an existing
    mechanism, it does not sit beside it: the pending-write register subsumes the 3 SCY mechanisms
    (§5.2); the GetTile0 latch + table removal subsumes the seed fix (§5.3). If a P-step adds a
    mechanism without deleting the one it supersedes, it has failed its purpose — revert.
  - Keep guardrail docs in sync (`docs/hardware/PPU-REIMPLEMENTATION.md`, `docs/TODO.md`) per the
    `04-ppu-fix.md` §6 coupling rule.

## 7. Risk + regression surface

This touches console-wide CGB timing, so the blast radius is the **entire CGB suite**, not just
mealybug:

- **Highest risk:** wilbertpol `intr_2_*` / `lcd_restart` (117) — they pin `LCD_REENABLE_LINE0_*`
  and the halt/dispatch grid. A CGB-aware restart phase must keep every wilbertpol case green
  (run the full suite, not a sample, each step).
- mooneye-acceptance CGB timing (LY/STAT/intr) — sensitive to CPU↔PPU phase.
- The SCY-observation latency touches every mode3 SCY read; verify the steady path (correct today)
  stays correct and that CGB readback semantics (CPU reading SCY right after writing) don't
  regress any ROM.
- Save-state/rewind: a new pending-write register and any new latch stage must be added to
  `PpuRuntimeState` Default + `capture/restore_save_state` (ppu.rs:643+) with `#[serde(default)]`
  to keep old snapshots loadable; machine-trace fixtures may need a CPU-invisible regen.

## 8. Validation plan + tooling (already built, revert at M5)

- **Primary oracle:** DocBoy CGB framebuffer == fixture. `build-trace-cgb`
  (`$HOME/workspace/docboy`, ENABLE_CGB=ON + `-DGBCYCLE_FETCH_TRACE`) with:
  - `GBDUMP_LY=<ly>` → per-line framebuffer-row RGB dump (added to `nogui/main.cpp`), diff vs the
    fixture per line.
  - `GBTRACE_LY=<ly>` → per-fetcher-state trace (`GBT …`) incl. `BG_GETTILE0` (observed scy) and a
    `GBT-TDATA` emit at `setup_bg_pixel_slice_fetcher_tile_data_address` (rendered `bwf.scy`/tile_y).
  - `CPUWR-SCY-DOCBOY` emit in `write_scy` (CPU SCY writes in DocBoy `dots`).
- **gb-cycle side:** sanctioned `crates/gb-core/examples/ppu_fetch_trace.rs` /
  `g3_sprite_grid.rs`; ephemeral probes (recreate + delete): per-pixel framebuffer diff (compare
  the rank-projected `actual.png` vs `expected-0.png` in the case artifact dir — NOT raw 2bpp), a
  CGB `cgb_scy_probe`/`cgb_objstall_probe` for per-tile SCY/obj-stall, and a `CPUWR-SCY` eprintln
  in the SCY write handler. CGB framebuffer is rank-normalized (color3→rank2; not linear).
- **Fast loop:** `cargo rom-suite mealybug-tearoom-tests --suite mealybug-tearoom-tests-cgb --case
  mealybug-cgb-m3-scy-change` (~0.7s, regenerates `actual.png`).
- **Full gates each step:** `cargo fmt-check`, `cargo lint`, `cargo tests`, then
  `rm -rf test/*/.status && cargo rom-report {blargg, mooneye, wilbertpol, gb-emulator-shootout,
  mealybug-tearoom-tests}` (mealybug runs BOTH DMG and CGB suites; CGB-compat is invisible to the
  shootout report — this command is mandatory).

## 9. Suggested phasing (each phase fully gated; revert on any regression)

- **P0 — pin the phase MECHANISM, not the magnitude.** Instrument the CGB LCD re-enable AND the
  direct-boot handoff in gb and DocBoy: capture the PPU restart/start dot vs the CPU's
  LCDC-enable-write dot, and the CPU SCY-write dots relative to it, at BOTH enable sites in both
  emulators. Identify which physical quantity ({CGB LCD-enable effect delay, CGB re-enable
  line-length, CGB direct-boot enable→PPU-start handoff phase}) carries the −4 such that ONE
  CGB-specific value explains the offset at boot AND re-enable. Gate: a written, oracle-grounded
  answer naming the mechanism and the CGB constant family it maps to (mirroring the DMG family at
  ppu.rs:58/62); no prod code. **If no single mechanism explains both sites, STOP and re-ground —
  do NOT proceed to a fitted constant.**
- **P1 — CGB enable→PPU-start phase model.** Implement the −4 as the CGB constant family P0
  identified (`is_cgb_family()`-gated), NOT a restart-only `correction(4)` literal. Gate: SCY writes
  land @76,84,92,100 (matching DocBoy `dots`) at the re-enable AND the boot-handoff phase agrees
  with the oracle, AND full CGB suite green (esp. wilbertpol 117). If wilbertpol regresses the model
  is in the wrong place — re-ground; do NOT patch the shared restart path to chase green.
- **P2 — uniform CGB write-observation register.** Port DocBoy's `pending_write` structure (per-reg
  countdown, SCY=2 on CGB), serialized in save-state, as the SINGLE observation-latency mechanism;
  route the fetcher's SCY read through it and COLLAPSE the 2-deep latch helpers + the raw-live
  recompute into it (§5.2). Gate: CGB observed SCY schedule matches DocBoy; mealybug CGB
  m3_scy_change px drops materially; full suite green; **no new SCY mechanism left beside the
  register.**
- **P3 — GetTile0 SCY sampling + MANDATORY seam removal.** Move CGB BG fetch to latch SCY once at
  GetTile0 (reading the P2 register). **Hard exit criteria — ALL required, not aspirational:**
  (a) CGB m3_scy_change **24/24** and full suite green; (b) the
  `cgb_dmg_software_startup_visible_tile2/3_*` SCY retarget tables (mode3_policies.rs:983/1009,
  transfer.rs `compute_startup_visible_*`) DELETED; (c) the recompute current-row override gone;
  (d) the landed seed fix (`cgb_startup_seed_get_tile_scy_row`, `cgb_startup_frozen_tile_row`,
  state.rs:2602/3000/3429) DELETED. **If the ROM passes but any of (b)–(d) survive, P3 has FAILED**
  — green-with-seams is the exact outcome this redesign exists to prevent; revert and re-ground.
- **P4 — close-out.** Update `docs/TODO.md` (strike `[PPU][MODE3-SCY-OBJ-PHASE-POLICY]` and the
  relevant fetcher-lead/observation-table notes), `docs/hardware/PPU-REIMPLEMENTATION.md`
  guardrails, this doc + `04-ppu-fix.md`; revert all DocBoy instrumentation. Gate: **net PPU seam
  count is LOWER than at campaign start** — the retarget tables + seed fix are gone, replaced by the
  uniform pending-write register + the CGB enable-phase constant family.

## 10. Entry pointers (file:line)

- Phase: `ppu/control/irq.rs` `enter_lcd_enabled_restart_state` (~678), `enter_lcd_enable_pending_state`
  (~718); `ppu.rs:61-68` (`LCD_REENABLE_LINE0_*`, `CPU_LCDC_ENABLE_EFFECT_DELAY_T_CYCLES=5`);
  `machine/access.rs:248` `apply_startup_configuration`; `boot.rs:1444` `direct_start_system_counter`,
  `1478` `cgb_direct_start_system_counter`, `1410-1412` system-counter constants.
- SCY observation: `ppu.rs:466` (`scy`), `control/registers.rs:12` `current_mmio_visible_registers`,
  `:42` refetch context, `:65` latch advance; `ppu/api.rs:809` SCY write handler.
- Fetcher / tables: `mode3/bg_fetch.rs` (TileIndex/TileData stages, the landed seed capture),
  `state.rs:3343` `recompute_live_background_cached_slice`, `helpers/mode3_policies.rs:866/983/1009`
  (SCY obj-phase + retarget tables), `mode3/transfer.rs` `compute_startup_visible_tile2/3_*`.
- Frozen #245 penalty (DO NOT refit): `mode3/obj_fetch.rs:88` `alignment_stall_remaining`.
