# CGB CPU↔PPU Timing Redesign — Design Doc (working)

> **STATUS: P0 done (§11). P1 done 2026-06-15 — phase model REFUTED, re-grounded (§13).** P1 proved
> the §5.1 "−4 enable→PPU-start phase" is NOT the m3_scy_change carrier (the −4 is a benign internal
> CPU↔PPU-phase difference; shifting the CGB STAT phase regressed 10+ other mealybug `m3_*` tests). The
> fix is SCY-observation only: §5.2 (uniform write latency) + §5.3 (CGB-D GetTile0-once sampling). **§5.2/§5.3
> attempt 1 reverted (§14): the §5.2 latency register is sound, but routing observed SCY through the universal
> `current_mmio_visible_registers` poisons the output-time recompute (121→2760px). Attempt 2 must feed observed SCY
> ONLY to the GetTile0 fetch latch, per-tile validated against DocBoy `GBT-TDATA tile_y`.** **ATTEMPT 2 RESULT
> (§15): the per-tile validation did its job — §5.2 is rebuilt and oracle-dot-exact on the steady path, but §5.3 is
> NECESSARY-BUT-INSUFFICIENT (it mismatches the oracle at exactly the obj-stalled startup tile, where gb's BG fetcher
> reaches GetTile0 ~4 dots early). The real carrier is the BG-fetcher obj-stall LEAD, not SCY observation; §5.3
> wiring HELD, re-ground to fetcher-lead (touches the #245-frozen penalty region — a phase decision).** This is the plan for the
> coordinated CGB timing rework that the PPU-hardening campaign (`04-ppu-fix.md`) proved is
> required to close `mealybug-cgb-m3-scy-change`. It supersedes the scoped-fix directions for
> that ROM. Durable open-work ledger stays in [`docs/TODO.md`](../TODO.md); hardware contract in
> [`docs/hardware/PPU.md`](../hardware/PPU.md). **Reference order is [`docs/REFERENCES.md`](../REFERENCES.md):
> primary hardware docs (Pan Docs, TCAGBD, Gekkio GBCTR) + the hardware-derived mealybug fixture FIRST;
> SameBoy/DocBoy are emulator CROSS-CHECKS (tier 2/4), never the authority.** SameBoy
> (`$HOME/workspace/SameBoy`, read-only) and DocBoy (`$HOME/workspace/docboy`, modifiable for
> instrumentation, revert at close) are the per-dot cross-checks; every claim they produce MUST be
> reconciled with documented hardware behavior (or flagged as doc-silent) — see the grounding ledger in
> §12. Agent-agnostic (Claude + Codex).

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
   *(**⚠️ REFUTED as the m3_scy_change carrier — P1, §13.** The ~4-clock CGB-vs-DMG phase is real and TCAGBD-documented (§8.1/§8.9.2), but it is a BENIGN internal-phase difference, NOT the bug: every other `m3_*` mealybug CGB test writes its register via the SAME STAT-Mode-2-IRQ handler at gb's dots and PASSES, and shifting the CGB STAT phase to match DocBoy regressed 10+ of them. gb's CPU↔PPU phase is correct; m3_scy_change is a SCY-observation bug (§5.2 + §5.3). Do NOT implement this component.)*
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
   existing SCY mechanisms into it.** *(Grounding: CORROBORATED by primary hardware research —
   mealybug comprehensive PPU doc: "On CGB and AGB devices, [SCY/SCX] writes appear to take effect 2
   T-cycles later" vs DMG immediate — §12.)* DocBoy models this as ONE uniform per-register deferred-write
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
3. **CGB fetcher startup sampling = DocBoy's "latch SCY once at GetTile0".** *(Grounding: CORROBORATED
   by primary hardware research — mealybug comprehensive PPU doc: DMG/CGB-C read SCY across the `B,0,1`
   fetch stages (per-byte → bitplane mixing), AGB/**CGB-D** read SCY ONLY in the `B` stage (latched once,
   no mixing); our fixture is CGB-D — §12.)* Once (1)+(2) put the
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
- **Official-doc cross-check each step (per `docs/REFERENCES.md`):** before landing, reconcile the model
  against Pan Docs (`raw.githubusercontent.com/gbdev/pandocs/master/src/*.md` — the site 403s automated
  fetchers), TCAGBD, Gekkio GBCTR, and the mealybug comprehensive PPU doc; record CORROBORATED/CONTRADICTED/
  DOC-SILENT in the §12 ledger. A DocBoy/SameBoy-only behavior is acceptable only when doc-silent + fixture-backed.
- **Full gates each step:** `cargo fmt-check`, `cargo lint`, `cargo tests`, then
  `rm -rf test/*/.status && cargo rom-report {blargg, mooneye, wilbertpol, gb-emulator-shootout,
  mealybug-tearoom-tests}` (mealybug runs BOTH DMG and CGB suites; CGB-compat is invisible to the
  shootout report — this command is mandatory).

## 9. Suggested phasing (each phase fully gated; revert on any regression)

**Cross-cutting rule (per `docs/REFERENCES.md` order): every phase must cross-check its model against the
primary hardware docs (Pan Docs, TCAGBD, Gekkio GBCTR) + the hardware-derived mealybug fixture BEFORE landing,
and record the result in the §12 grounding ledger as CORROBORATED / CONTRADICTED / DOC-SILENT.** DocBoy/SameBoy
are cross-checks, not the authority; a DocBoy-only behavior that the docs are silent on (e.g. the −4 enable
phase) is permitted ONLY when it is reconciled with the hardware-derived fixture AND no documented behavior
contradicts it — and it is flagged DOC-SILENT, not asserted as fact.

- **P0 — pin the phase MECHANISM, not the magnitude. ✅ DONE 2026-06-15 (see §11).** Re-enable site
  fully characterized: the −4 is a CGB CPU↔PPU **enable→PPU-start phase** at the LCD re-enable restart
  (oracle-grounded, adversarially verified). Surviving carrier candidates: {CGB `LCD_REENABLE_INITIAL_LINE_DOT`≠0,
  CGB re-enable line0 length}; the **`CPU_LCDC_ENABLE_EFFECT_DELAY` and boot-system-counter carriers are
  STRUCK** (exp e/g + mechanism). STOP does NOT fire (no boot↔reenable contradiction). **Open: the boot-handoff
  phase is UNVERIFIED — tooling-blocked (all CGB mealybug ROMs mask boot via a disable+VBlank-wait+re-enable
  harness; DocBoy is `ENABLE_BOOTROM=OFF`); carried into P1 as a flagged entry risk (§11.5).**
- **P1 — CGB enable→PPU-start phase model. ❌ REFUTED 2026-06-15 (see §13).** The ROM source shows the SCY
  storm runs in the STAT Mode 2 IRQ handler (not enable-gated). Shifting the CGB STAT phase to match DocBoy's SCY
  dots reproduced @76 but REGRESSED 10+ other mealybug `m3_*` tests that share the handler and pass at gb's timing
  ⇒ gb's CPU↔PPU phase is correct; the −4 is benign; there is no phase term to implement. Reverted.
- **P2/P3 (now the active work) — SCY-observation only.** Implement §5.2 (uniform CGB write-observation latency,
  countdown=2, collapsing the 3 SCY mechanisms) + §5.3 (CGB-D GetTile0-once SCY sampling), NO phase/STAT change;
  then delete the seed fix + retarget tables (P3 exit criteria, §9 below). **Regression gate: the mealybug `m3_*`
  register-change tests are the sensitive set (shared SCY/STAT harness) alongside wilbertpol 117 / mooneye 113 —
  run the FULL CGB suite each step.** If the `m3_*` tests regress, the SCY model is leaking into the shared path —
  re-ground.
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

## 11. P0 RESULT — diagnostic complete (2026-06-15)

**Gate: MET for the re-enable site; boot-handoff verification tooling-blocked and DEFERRED to P1 (§11.5).
STOP does NOT fire.** Mechanism named, carriers narrowed, no prod code. Established by fresh oracle measurement
(DocBoy `build-trace-cgb`, ROM `m3_scy_change.gb` md5 `5f0f33d2…`, byte-identical across both trees) and a
5-claim adversarial refutation pass (C1/C5 survived; C2/C4 forced the corrections below; C3 adjudicated STOP).

### 11.1 Measurement (gb-cycle `line_dot` ≡ DocBoy `dots`: zero offset, both per-line, OAM-dot-0 origin)

| Quantity (CGB, m3_scy_change) | gb-cycle | DocBoy (oracle) | Δ |
|---|---|---|---|
| mode3 start (ly = 1, 24, 40) | 80 | 80 | 0 |
| SCY-write dots, ly=1 | 80,88,96,104,112,120 (v 0,1,2,254,72,254) | 76,84,92,100,108,116 (same values) | **+4** |
| SCY-write dots, ly=24 | 80,88,96,104,112,120 (v 0,1,2,3,4,3) | 76,84,92,100,108,116 (same values) | **+4** |
| re-enable LCDC.7 0→1 | `line_dot=0` (LCD off, frozen); effect +5 t-cyc (`CPU_LCDC_ENABLE_EFFECT_DELAY=5`) | `dots=0` (frozen); `turn_on()` instant (0) | — |
| re-enable line0 length | 448 (mode3@72; `LCD_REENABLE_LINE0_*`) | 456 (mode3@80; full glitched line) | −8 |
| boot-epoch LCD disable | ly=144 `line_dot=67` | ly=144 `dots=48` | +19 (**CONFOUNDED**) |

### 11.2 Findings (adversarially verified)

- **The +4 is a genuine CPU↔PPU enable-phase offset at the LCD RE-ENABLE** — DocBoy's CPU reaches each SCY
  write 4 dots *before* its own mode3@80; gb-cycle's CPU reaches it *at* mode3@80. It is **NOT SCY observation
  latency** (raw store-dot vs store-dot on both sides: gb stores SCY immediately with no register pending-write,
  `api.rs:809`; DocBoy emits `CPUWR-SCY` *before* setting `countdown=2`; exp h confirmed latency-alone goes +2
  wrong-way) and **NOT a mode3 geometry difference** (mode3-start identical at 80 across ly=1/24/40).
- **Sign:** to match the oracle (SCY at 76, 4 dots earlier on a co-anchored axis), gb's PPU must be **RETARDED
  ~4 dots relative to the CPU at the re-enable** (the restart begins later in CPU-time, so fewer PPU dots have
  accrued when the CPU executes each write). Net rendering sign incl. the separate +2 SCY pending-write latency:
  phase −4 + latency +2 = **net −2** (gb's observation must end 2 dots earlier than today), matching §5.2.

### 11.3 Mechanism + constant family (the P0 deliverable)

**Mechanism: the CGB CPU↔PPU enable→PPU-start phase at the LCD re-enable restart.** gb-cycle has NO
CGB-specific enable-phase term — the CGB re-enable falls through to the SHARED `LCD_REENABLE_INITIAL_LINE_DOT=0`
(`irq.rs:683-687` `else` arm) and the SHARED `LCD_REENABLE_LINE0_*` (448/72), neither `is_cgb_family()`-gated
and both pinned only by DMG-mode wilbertpol. That absence is the defect, mirroring how DMG carries a dedicated
family (`DMG0_DIRECT_BOOT_HANDOFF_PPU_PHASE_BASE_OFFSET_DOTS=3992` `ppu.rs:58`,
`DMG_REAL_BOOT_POWER_ON_LCD_ENABLE_INITIAL_LINE_DOT=92` `ppu.rs:62`).

**Surviving carrier candidates for P1 (one mechanism; choose by oracle-match, do not stack):**
1. A CGB-specific **`LCD_REENABLE_INITIAL_LINE_DOT` ≠ 0** (the restart `line_dot` seed) — the cleanest DMG analog.
2. A CGB-specific **re-enable line0 length** (`LCD_REENABLE_LINE0_*`). **NOT exonerated** (corrects the earlier
   "ruled out" claim): `mode3-start=80` at ly≥1 is a per-line LOCAL coordinate reset at every wrap, so it is
   structurally BLIND to a line0-length error; gb's current 448 already shifts ly≥1 phase by −8 dots. The current
   448 is the wrong sign to be the *active* cause, but the candidate remains live as a *fix* carrier.

**STRUCK carriers (do NOT pursue):** CGB **`CPU_LCDC_ENABLE_EFFECT_DELAY` ≠ 5** — refuted (exp e "+4 no effect"
+ mechanism: during the countdown `line_dot` is frozen and the restart resets it to `LCD_REENABLE_INITIAL_LINE_DOT=0`
regardless of delay length; lengthening the delay only postpones the whole restart, leaving the CPU↔PPU phase
invariant). Also STRUCK: the **boot system-counter** (exp g "+4 no effect" — decoupled from the re-enable phase).

**Hardware-doc grounding (per `docs/REFERENCES.md`; full ledger §12; TCAGBD + GBCTR read locally 2026-06-15):**
three of the four model claims are now documentation-grounded. §5.2 the CGB 2-T-cycle write-observation latency
and §5.3 the CGB-D "latch SCY once at the `B` stage" are CORROBORATED by the mealybug comprehensive PPU doc. **The
~4-clock CPU↔PPU phase is now CORROBORATED by TCAGBD** — §8.1 ("LCD timings… CGB… in another way *even in DMG
compatibility mode*") and §8.9.2 (CGB-in-DMG-mode) vs §8.9.1 (DMG), which document the CGB holding LY ~4 clocks
longer (153→0 at clock 8 vs DMG clock 4) in the exact mode our ROM runs (CGB-flag 0x00). **The only residual
DOC-SILENT item is the dot-exact first-line fetch phase** that discriminates the `LCD_REENABLE_INITIAL_LINE_DOT`
vs line0-length carrier (TCAGBD is 4-clock-granular and omits the sprite-dependent mode3→mode0 transition; GBCTR
is silent) — that is what P1's fetch-domain + §11.5 boot-handoff measurement still owes.

### 11.4 STOP adjudication — does NOT fire

The STOP trigger is a *proven contradiction* between the boot and re-enable sites. None exists: the only
boot-site signal — the LCD-disable dot (Δ=+19) — is non-diagnostic. The ROM gates the disable behind a
wait-for-VBlank busy-loop (`@0x4800: LDH A,(LY); CP 0x90; JR NZ,-6; RET`, then `XOR A; LDH (LCDC),A`); the loop
period is 32 dots and 19 < 32, so the gap is one-iteration poll-exit quantization, not phase. exp g independently
shows the boot system-counter is decoupled from the re-enable SCY dots. A confounded, non-diagnostic boot signal
cannot establish a contradiction → no STOP. The re-enable defect (the actually-exposed one) is fully characterized.

### 11.5 FLAGGED P1-ENTRY RISK — boot-handoff phase is UNVERIFIED (tooling-blocked)

The §5.1/§9 requirement that the mechanism be "verified against the oracle at BOTH the direct-boot handoff AND the
re-enable" is **NOT yet met for boot**:
- m3_scy_change — and all 31 CGB mealybug ROMs (byte-scanned) — use the same `disable + wait-LY==144 + re-enable`
  harness; the SCY storm is post-re-enable only and the boot epoch emits ZERO in-mode3 SCY writes. No CGB mealybug
  ROM gives a clean boot-handoff CPU↔PPU phase signal.
- DocBoy is built `ENABLE_BOOTROM=OFF`, so neither emulator exercises a real CGB boot-ROM handoff; the boot handoff
  is synthetic on both sides.

**Before any prod code lands, P1 MUST obtain a confound-free boot-handoff measurement** (§9 P1 already gates on
"boot-handoff phase agrees with the oracle"): either (a) a tiny custom CGB ROM that runs the mode3 SCY storm on the
first frame after a clean direct boot WITHOUT any LCD disable/re-enable, run through the same probes; or (b)
instrument BOTH emulators to emit the PPU restart/start dot relative to the CPU's first instruction-fetch dot
DIRECTLY at the SkipBoot/direct-boot handoff (read off the state machine, not via a poll-gated ROM write). Confirm
the SAME CGB constant that lands SCY @76/84/92/100 at the re-enable also reproduces the oracle boot-handoff phase.
Until then the family is NOT certified not-curve-fit, and the fix MUST be re-enable-scoped (irq.rs
`enter_lcd_enabled_restart_state`) and structurally isolated from the boot dispatch.

### 11.6 Tooling (reproducible)

- **gb-cycle probe** — `crates/gb-core/examples/cgb_scy_phase_probe.rs` (ephemeral, DELETED at P0 close; recreate
  per §11.7). Raw event tracer: `Machine::new_summary(MachineConfig::new(ConsoleModel::GameBoyColor)
  .with_startup_mode(StartupMode::SkipBoot))`, step `step_t_cycle()`, and per tick read `ppu.{ly(), line_dot(),
  access_mode(), lcd_state().is_enabled(), read_register(0xFF40), read_register(0xFF42), mode0_start_dot()}`
  (accessors `api.rs:207/1493/1497/1501/1505/1522`). Print edges of LCDC.7, LCD enable/disable, `OamScan→Drawing`
  on the target ly, and SCY value-changes, tagged by enable-epoch (epoch 0 = boot, epoch 1 = first re-enable).
  Run: `cargo run --release -p gb-core --example cgb_scy_phase_probe -- <m3_scy_change.gb> <ly>`.
- **DocBoy emits** — kept until P4 (§8/§9-P4), `#ifdef GBCYCLE_FETCH_TRACE`-guarded, copy the `GBTRACE_LY` idiom
  at `ppu.cpp:3538-3545`: `CPUWR-LCDC-DOCBOY` (in `write_lcdc`, the `if (en != lcdc.enable)` block, before
  `turn_on()/turn_off()`, prints `ly dots val en`) and `PPU-MODE3-START-DOCBOY` (top of `enter_pixel_transfer`,
  prints `ly dots`). Rebuild `cmake --build build-trace-cgb -j8`. Run:
  `GBTRACE_LY=<ly> build-trace-cgb/docboy-nogui <rom> -t 400000 2>&1 | grep -E 'CPUWR|PPU-MODE3'`.

## 12. Hardware-doc grounding ledger (per `docs/REFERENCES.md`)

Cross-check of every model component against primary hardware docs. DocBoy/SameBoy are cross-checks, not the
authority; CORROBORATED = documented hardware behavior agrees; DOC-SILENT = no documented behavior found
(permitted only when reconciled with the hardware-derived fixture and uncontradicted, and flagged as such).

| Component / claim | Status | Source + note |
|---|---|---|
| §5.2 CGB SCY/SCX write observed **2 T-cycles late** (DMG immediate) | **CORROBORATED** | mealybug comprehensive PPU doc: *"On CGB and AGB devices, writes appear to take effect 2 T-cycles later."* = DocBoy `pending_write.scy.countdown=2`. |
| §5.3 **CGB-D latches SCY once at the `B` stage** (GetTile0); DMG/CGB-C read across `B,0,1` (per-byte → bitplane mixing) | **CORROBORATED** | mealybug comprehensive PPU doc. Our fixture is `cgb_dmg_mode` = CGB-D. |
| First frame after LCD enable is blank; LCD disable only during VBlank | **CORROBORATED (qualitative)** | Pan Docs `LCDC.7`. No dot-level first-line timing given. |
| §5.1 / P0 **~4-clock CPU↔PPU phase, CGB-vs-DMG, present in DMG-compat mode** | **CORROBORATED** | **TCAGBD §8.1**: "LCD timings are a bit different… CGB, AGB, AGS in another way (**even in DMG compatibility mode**)." **TCAGBD §8.9.2 (CGB in DMG mode) vs §8.9.1 (DMG)**: CGB holds LY ~4 clocks longer (153→0 at clock **8** vs DMG clock **4**) and its LY=LYC compare is correspondingly offset — a documented ~4-clock CGB phase shift in DMG-compat mode. Our ROM runs there (CGB-flag byte 0x143 = **0x00** = DMG-compat). Direction coheres with P0's +4: CGB line boundary ~4 clocks later in CPU-time ⇒ CPU writes land ~4 dots earlier in the PPU line (SCY @76 vs gb-DMG @80). |
| Exact first-line-after-enable mode3-fetch dot / re-enable line0 length (gb 448/72 vs DocBoy 456/80) | **DOC-SILENT (residual)** | TCAGBD documents the ~4-clock phase at 4-clock granularity in the LY/LYC/STAT domain but does **not** tabulate the mode3→mode0 fetch transition (sprite-count-dependent, §8.9 caveat); GBCTR is silent. The dot-exact fetch phase that discriminates the `LCD_REENABLE_INITIAL_LINE_DOT`-seed vs line0-length carrier still needs the P1 fetch-domain + boot-handoff measurement (§11.5). |

**Method note (2026-06-15):** Pan Docs blocks automated fetchers (HTTP 403) — use the GitHub source mirror
`raw.githubusercontent.com/gbdev/pandocs/master/src/*.md`. **TCAGBD and Gekkio GBCTR were read locally**
(`$HOME/Downloads/{TCAGBD,gbctr}.pdf` via `pdftotext`/poppler): TCAGBD §8.1/§8.8/§8.9 are the authoritative
corroboration of the CGB-vs-DMG phase; **GBCTR's PPU chapter (ch.9, p.139+) is DMG-register-focused and silent on
the CGB phase**. The mealybug comprehensive PPU doc corroborates §5.2/§5.3. Net: 3 of the 4 model claims are now
documentation-grounded; only the dot-exact carrier choice remains an emulator+fixture measurement (P1).

## 13. P1 RESULT — phase model REFUTED; re-grounded to SCY-observation (2026-06-15)

**Outcome: the §5.1 "CGB enable→PPU-start phase" / −4 carrier is REFUTED as the m3_scy_change cause. The −4 is a
benign internal CPU↔PPU-phase difference between gb-cycle and DocBoy (each self-consistent), NOT the bug. The fix
is §5.2 (uniform CGB write-observation latency) + §5.3 (CGB-D GetTile0-once SCY sampling) ONLY — no phase/STAT
change. The "net −2 = phase −4 + latency +2" sign bookkeeping (§5.2) is WITHDRAWN; there is no phase term.**

How P1 got here (the measurement that the doc's P1 gate demanded, plus the regression that refuted the model):
1. **The ROM source settles the mechanism.** `mealybug-tearoom-tests/src/ppu/m3_scy_change.asm`: after re-enabling
   the LCD the ROM sets `STAT=$20` (Mode 2 OAM IRQ), `ei`, and runs a NOP slide — **the SCY writes execute inside
   the STAT Mode 2 IRQ handler (vector 0x48), re-armed by each line's Mode 2 entry.** It does NOT poll LY/STAT and
   does NOT depend on the enable geometry — which is exactly why exp d/e/g (enable-phase levers) had "no effect".
2. **Measured (CGB):** gb dispatches the handler at ly=1 line_dot 16 → SCY writes @80,88,96; DocBoy dispatches it
   4 dots earlier (entry 12) → @76,84,92. The handler is byte-identical, so the +4 is purely "when the STAT-Mode-2
   handler runs relative to the PPU fetch" — i.e. the CGB STAT/CPU↔PPU phase (TCAGBD §8.9.2).
3. **Experiment:** `ordinary_mode2_stat_pretrigger_dots` 4→8 for `is_cgb_family()` (irq.rs:304) reproduced DocBoy's
   SCY dots @76 exactly (handler entry → 12).
4. **REGRESSION (the refutation):** that change broke **10+ other mealybug CGB tests** — `m3_bgp_change`,
   `m3_bgp_change_sprites`, `m3_obp0_change`, `m3_lcdc_obj_en_change_variant`, `m3_lcdc_obj_size_change_scx`,
   `m3_lcdc_tile_sel_change`, `m3_lcdc_tile_sel_win_change`, `m3_lcdc_win_en_change_multiple(_wx)`,
   `m3_lcdc_win_map_change`, … (mooneye 113/113 and wilbertpol 117/117 stayed green; only the mealybug
   register-change tests broke). Reverted → mealybug CGB back to 23/24.
5. **Decisive conclusion:** every other `m3_*` test writes its register via the SAME STAT-Mode-2-IRQ handler at gb's
   dots (80,88,…) and PASSES against the hardware fixture. So gb's STAT/write timing is CORRECT, and the +4 vs
   DocBoy is a benign internal-phase difference. m3_scy_change fails at the SAME write timing that renders
   BGP/OBP0/LCDC correctly ⇒ the bug is **SCY-observation-specific** (gb samples SCY live per-plane during fetch;
   CGB-D samples it ONCE at the `B`/GetTile0 stage with a 2-T-cycle write latency — §12 CORROBORATED).

**Revised plan (supersedes §9 P0/P1 phase model):** skip the §5.1 phase component entirely. Implement §5.2 (the
uniform CGB write-observation latency, countdown=2, collapsing the 3 overlapping SCY mechanisms) and §5.3 (CGB-D
GetTile0-once SCY sampling), WITHOUT touching STAT/phase timing, then delete the seed fix + retarget tables (P3
exit criteria). Hard regression gate stays: full CGB suite green at each step — the mealybug `m3_*` register-change
tests are the sensitive set this round (they share the SCY harness), alongside wilbertpol 117 / mooneye 113.

**Lesson:** P0 measured the +4 correctly but misattributed it as the cause; the missing P0 check was "do the OTHER
m3_* tests share the same +4 write-dot offset and still pass?" (they do). The P1 gated experiment + full-suite
regression is what caught it. Keep the "prove it on the whole suite before believing a single-ROM fix" discipline.

## 14. §5.2/§5.3 IMPLEMENTATION — attempt 1 reverted; the plan's "single-source" Step 2 is WRONG (2026-06-15)

**Reverted to baseline (m3_scy_change 121px, full suite green). The mapping plan's Step 2 — "route the observed SCY
into `current_mmio_visible_registers` so it feeds all three consumers" — is WRONG and must NOT be done.**

What was implemented (all reverted, clean tree):
- A `PpuPendingWrite { observed, pending_value, countdown }` register on `Ppu` (`scy_observation`), armed with
  `countdown=2` on CGB SCY writes (api.rs Scy write), ticked once per dot (beside `advance_mode3_register_latches`),
  serialized with `#[serde(default)]`. **This §5.2 mechanism is SOUND** — compiles, save-state round-trips, fmt/lint
  clean, and the mealybug **suite canary stayed 23/24** (only m3_scy_change; no other `m3_*` regressed).
- Routed `scy_observation.observed()` into `current_mmio_visible_registers().scy` on CGB (the plan's Step 2).

**Result: m3_scy_change 121 → 2760 px (far worse).** Root cause: `current_mmio_visible_registers` feeds the
OUTPUT-time recompute (`recompute_live_background_cached_slice` → `current_scanline_tile_row()` at push/pop,
state.rs:3427-3430). The recompute already samples SCY at the output dot, which is too LATE (doc exp h: feeding it
the 1-dot-delayed `pipeline.scy` → 228px worse). Adding the 2-dot observation latency there COMPOUNDS the error
(→2760). Generalizing the GetTile0 frozen-row latch into the recompute (Step 3a) made no difference — the 2760 was
already there from Step 2. (Also: MECH 1c's dmg-software SCY routing reads SCY and broke 3 unit tests when the
observed value shifted its marker — confirming MECH 1c must be collapsed together with the SCY model.)

**Corrected design for attempt 2:**
1. **Keep `current_mmio_visible_registers().scy = self.scy` RAW** — never route observed SCY through the universal
   helper (it poisons the output-time recompute). The §5.2 register stays as built.
2. **Observed SCY (2-dot latency) feeds ONLY the GetTile0 latch (fetch time).** At bg_fetch TileIndex/1, for ALL CGB
   Background tiles, latch the tile row computed from the OBSERVED value (read `scy_observation.observed()`, not the
   raw helper) — generalize the seed capture.
3. **The GetTile0-latched row (frozen) feeds BOTH the fetch (TileDataLow/High, replacing the live per-plane read)
   AND the recompute's CGB Background row (replacing `current_scanline_tile_row()`).** DMG keeps live per-byte.
4. Remove MECH 1c (`cgb_dmg_scy_high_plane_uses_low_row`) + its 3 unit tests together — the latched-row-for-both-
   planes replaces it.
5. **MANDATORY per-tile oracle check BEFORE wiring the recompute:** with `build-trace-cgb`, diff gb's per-tile
   GetTile0-latched row against DocBoy's emitted `GBT-TDATA … bwfscy=.. tile_y=..` per tile. They must match per
   tile. Attempt 1 skipped this and wired blind — that is the missing rigor. Do the row-match measurement first,
   then wire fetch, then recompute, then delete MECH 1c/seed/tables, full CGB suite at each step.

Net: the observation-latency MECHANISM is correct and built; the error was applying it at the wrong site (universal
helper / output recompute) instead of the GetTile0 fetch latch. Attempt 2 = GetTile0-local observed SCY, per-tile
DocBoy-validated.

## 15. ATTEMPT 2 — per-tile validation PASSED its job: §5.2 confirmed, §5.3 INSUFFICIENT; residual is the BG-fetcher obj-stall lead (2026-06-15)

**Outcome: the §5.2 observation register is rebuilt and VALIDATED dot-exact against the oracle, but the mandatory
per-tile measurement (§14 step 5 — the step attempt 1 skipped) proves §5.3 (GetTile0-once-from-observed) does NOT
match DocBoy per tile, and so cannot close m3_scy_change on its own. The residual carrier is the BG-fetcher
startup/obj-stall TIMING (the "fetcher-lead"), NOT SCY observation. Per the gate's own rule ("if they don't match
per tile, re-ground; do NOT wire blind"), §5.3 wiring is HELD. No behavior-changing code landed; the §5.2 register
+ an env-gated GetTile0 candidate probe are in the working tree (uncommitted).**

### 15.1 What landed (behavior-neutral, validated)
- **§5.2 register, rebuilt the right way.** `PpuScyObservation { observed, pending_value, countdown }` lives in
  `PpuRuntimeState` (ppu.rs, beside `visible_registers`/`pipeline_registers`); armed `countdown=2` on CGB SCY write
  (`api.rs` `PpuRegister::Scy`), ticked once per dot beside `advance_mode3_register_latches_from_mmio` (api.rs:~1178),
  resynced to `self.scy` in `reload_mode3_register_latches_from_mmio` (covers startup + every LCD enable/disable/
  restart), serialized `#[serde(default)]`. `current_mmio_visible_registers().scy` kept RAW (§14 step 1). fmt/lint/
  tests clean; mealybug CGB stays **23/24** (no consumer yet) ⇒ provably behavior-neutral.
- **Env-gated GetTile0 candidate probe** (ephemeral, `GBCYCLE_SCY_PROBE_LY=<ly>`) at bg_fetch TileIndex/dot1 for CGB
  Background tiles, emitting `GBT-CANDIDATE ly dot fetch_x observed row live_scy` — the row gb WOULD freeze under §5.3.

### 15.2 The measurement (gb `GBT-CANDIDATE` vs DocBoy `GBT-TDATA bwfscy/tile_y`, 4 bands)
DocBoy emit is at tile-data-address setup (≈ gb GetTile0 + 2 dots, the stage offset); compared on GetTile0-aligned
(first) emit. Per-tile **value** match:

| band (rep ly) | candidate vs oracle | mismatching tiles |
|---|---|---|
| 72–79 (ly 74) | **all 21 tiles match** (value-exact) | none |
| 136–143 (ly 140) | **all 21 tiles match** (value-exact) | none |
| 23–31 (ly 24, 28) | match EXCEPT one startup tile | **x=8 only** (gb obs=1, oracle=2) |
| 0–7 (ly 4) | diverges broadly | confounded by the landed seed fix (gb renders via seed, not the raw candidate) |

⇒ The §5.2 latency is correct (steady path + 2 whole bands are dot-exact). The candidate fails ONLY at the
obj-stalled startup tile, and (separately) the seed-fix band is not modelled by the raw candidate.

### 15.3 Root cause, nailed (ly=28, GetTile0 dots side-by-side)
| x | gb GetTile0 dot (scy) | DocBoy GetTile0 dot (scy) | Δ |
|---|---|---|---|
| 0 | 84 (0) | 85 (0) | +1 |
| **8** | **97 (1)** | **101 (2)** | **+4** |
| 16 | 109 (3) | 109 (3) | 0 |
| 24…160 | aligned | aligned | 0 |

gb: x0→x8 = 13 dots, x8→x16 = 12 dots. DocBoy: x0→x8 = **16** dots, x8→x16 = **8** dots. Same total (both reach
x16@109), but **gb places its startup/obj-stall delay one tile too LATE** — it fetches tile x=8's index/SCY at dot 97
(BEFORE the stall), hardware fetches it at dot 101 (AFTER the stall). A SCY write lands in that 4-dot gap, so gb
samples scy=1 where hardware samples scy=2. That single misplaced stall window is the whole bug. The failing
tile tracks the per-line sprite (obj fetch stalls the BG fetcher); in bands 72–79/136–143 the stall lands where the
local SCY value is identical across the gap, so it is invisible there.

### 15.4 Verdict + re-grounded hypothesis
- **§5.2 (CGB 2-dot SCY write-observation latency): CONFIRMED correct** (doc-CORROBORATED in §12; now also
  oracle-dot-exact on the steady path). Keep it as the foundation.
- **§5.3 (GetTile0-once-from-observed): NECESSARY but INSUFFICIENT.** It would render bands 72–79/136–143 correctly,
  but leaves the obj-stalled startup tile wrong in bands 0–7/23–31, and the raw candidate does not reproduce the
  seed-fix band — so wiring it now would NOT reach 24/24 and would regress ly0–7. HELD per the gate.
- **Re-grounded carrier: the BG-fetcher startup/obj-stall lead.** gb's startup fetch distributes its obj-stall delay
  one tile later than hardware; the post-sprite startup tile's GetTile0 fires ~4 dots early. This is the
  campaign's known "fetcher ~7 dots adelantado" debt (branch `ppu/fetcher-lead-hardening`). Closing m3_scy_change
  needs the obj-stall/startup BG-fetch timing made hardware-true so GetTile0 for that tile lands at the oracle dot;
  THEN §5.2+§5.3 sample the correct SCY automatically (and the seed fix + retarget tables can be deleted).
- **⚠️ This residual lives in the obj-stall/sprite-penalty region** (`obj_fetch.rs:88` `alignment_stall_remaining`,
  the #245-frozen penalty) and the startup-fetch idle/alignment machinery — i.e. the §6 HARD CONSTRAINT
  "never refit the frozen #245 sprite penalty" is adjacent. The fix must move the *position* of the startup delay
  relative to the tile-fetch boundary WITHOUT re-tuning per-sprite Mode-3 cost (prove byte-identical mode3 duration
  before/after). This is a phase decision for the next step, not a free lever.

### 15.5 Tooling (reproducible)
- gb probe: `GBCYCLE_SCY_PROBE_LY=<ly> cargo rom-suite mealybug-tearoom-tests --suite mealybug-tearoom-tests-cgb
  --case mealybug-cgb-m3-scy-change 2>&1 | grep GBT-CANDIDATE` (bg_fetch.rs `scy_probe_target_ly`; ephemeral).
- oracle: `GBTRACE_LY=<ly> build-trace-cgb/docboy-nogui <m3_scy_change.gb> -t 250000 2>&1 | grep GBT-TDATA`.
- side-by-side comparison harness: `/tmp/scy_cmp.sh` (per-tile join, flags value mismatches).

## 16. BREAKTHROUGH — the §5.3 latch point is TileDataLow/0, not TileIndex/0 (2026-06-15)

**The "fetcher-lead" framing in §15 was a measurement artifact of WHERE the candidate sampled. The full per-dot
stage trace (gb `GBT-STAGE`) shows gb's BG fetcher and DocBoy's reconverge correctly; the bug was that the §5.3
candidate (and the landed seed fix) sample observed-SCY at `TileIndex/dot1`, but the tile-data ROW is consumed one
stage later at `TileDataLow/dot0`. At that later dot the §5.2-observed SCY has settled correctly relative to gb's own
SCY-write phase, and the candidate then matches the oracle EXACTLY.**

Evidence (ly=28, the failing band): gb writes scy=2 at line_dot 96; the §5.2 countdown=2 settles it at dot 98. gb's
x8 `TileIndex/1` is at dot 97 (observed still =1, unsettled) but its `TileDataLow/0` is at dot 98 (observed=2). DocBoy
latches bwf.scy at its `B`/GETTILE0 (dot 99) and uses it at LOW0 (dot 101), reading scy=2. So sampling at
`TileDataLow/0` (dot 98) yields 2 = oracle. The fetcher dots were never wrong — the obj-stall (#245) is irrelevant.

Re-validated per-tile across the 4 bands with the candidate moved to `TileDataLow/dot0`:

| band (rep ly) | result |
|---|---|
| 23–31 (ly 24, 28) | **ALL tiles match the oracle** (x8 now obs=2, was 1) |
| 72–79 (ly 74) | **ALL tiles match** |
| 136–143 (ly 140) | matches (steady) |
| 0–7 (ly 4) | still diverges — but this band already PASSES via the landed seed fix (the special stable-scy region right after LCD re-enable; `TileDataLow/0` crosses a write boundary there) |

**Implication:** §5.3 = "latch observed-SCY → tile_data_row ONCE at `TileDataLow/dot0`, freeze for both planes + the
recompute." This closes the currently-failing bands (23–31, 72–79, 136–143 = the 121px) with NO change to fetcher /
obj-stall / #245 timing. The `is_cgb_family()` GetTile0 latch is the §5.3 mechanism; the only open question for the
P3 seam-deletion is band 0–7, currently held by the seed fix (`TileIndex/0` capture in the stable post-re-enable
region) — keep it until the §5.3 path is proven to also cover band 0–7, then delete.

## 17. §5.3 WIRED + REGRESSION-FREE — 121→41px, 3 of 4 bands closed; residual = band-0-7 left-edge obj-stall (2026-06-15)

**Milestone (uncommitted WIP): the §5.3 model is wired and the FULL suite is regression-free. m3_scy_change CGB
121px → 41px. The remaining 41px is band 0–7 (~38px) + 3 stray px (rows 22, 69).**

What is wired (the §5.3 + net-0 latency model):
- §5.2 `CGB_SCY_OBSERVATION_DELAY_DOTS = 0` (the register stays, but the empirically-correct latency is **0**, not 2 —
  see below). Frozen row = `(live scy + ly) % 8` sampled at `TileDataLow/dot0`.
- `advance_bg_fetcher_tile_data_low_dot0`: for every non-seed CGB Background tile, latch the row into
  `cgb_startup_seed_get_tile_scy_row` and compute the low-plane address from it.
- `advance_bg_fetcher_tile_data_high_dot0`: reuse that latched row for the high plane (replaces MECH 1c on this path).
- recompute consumes it via the existing `cgb_startup_frozen_tile_row` override.
- The CGB `compute_startup_visible_tile2/3_scy_*` retarget tables are NEUTRALIZED (return None on CGB).

**Why latency must be 0, not the doc-CORROBORATED 2 (the §13 "net −2" bookkeeping, restored):** gb's SCY writes
land +4 dots vs hardware (P0). DocBoy writes @76/84/92 + 2-dot latency settles BEFORE its (later) data-stage read; gb
writes @80/88/96, and at gb's data-stage read the write is only ~1 dot old, so applying the 2-dot latency makes gb
observe one write STALE (ly=68 x16: latency=2 → obs 3, oracle 4). gb's +4 write phase already supplies the freshness
DocBoy gets from the latency, so the NET correct model for gb is latency 0 = "use live SCY at the data stage." This
is exactly §5.2's withdrawn "phase −4 + latency +2 = net −2" — restored, and oracle-confirmed per-tile on ly 28/68/132.

**Full-suite gate (all green, no regressions):** wilbertpol **117/117** (incl. `intr_2_mode0_timing_sprites`),
gb-emulator-shootout **264/264**, mooneye green, blargg green, mealybug DMG **24/24**, mealybug CGB **23/24**
(m3_scy_change the only fail, now 41px). The #245 sprite penalty is provably untouched (shootout + wilbertpol green).

**The residual (band 0–7, ~38px) is the left-edge-sprite obj-stall — and it IS the #245 region.** For ly 0–7 the
test's per-line sprite sits at the left edge (sprite x ≈ ly/8 ≈ 0), where the obj-stall is large; DocBoy's x8 GetTile0
lands ~8 dots later (ly=4: gb @98 vs DocBoy @106) and gb samples one SCY write early. The retarget tables + seed fix
were the curve-fit for exactly this band; neutralizing them (to let §5.3 close the mid bands) re-exposed it. So:
- The mid bands (sprite mid-screen, small/no obj-stall offset) → §5.3 closes them, tables not needed.
- Band 0–7 (left-edge sprite, large obj-stall) → §5.3 samples at the wrong dot; needs either the #245 left-edge
  obj-stall timing made hardware-true, OR the seed/tables kept for that band only (a residual seam).

**Net:** §5.3 (data-stage latch, latency-net-0) is the validated, regression-free core that closes 3 of the 4 bands.
Closing band 0–7 cleanly (and deleting the seed fix + tables per P3) requires the left-edge-sprite obj-stall fix in the
#245 region — the one HARD-CONSTRAINT lever, now precisely isolated to a single per-line tile.

**LANDED: commit `2bcb48e9`** (§5.3 data-stage latch + CGB retarget-table neutralization; 121→41px; full suite green).

## 18. BAND ly0-7 DIAGNOSIS — left-edge-sprite obj-stall under-positions x8 GetTile0 (point-1 start, 2026-06-15)

Per-tile oracle measurement for the storm frame at ly=4 (band 0-7), gb candidate (TileDataLow/0, live SCY) vs DocBoy:

| x | gb scy/row @dot | DocBoy bwfscy/y @BG_GETTILE0 | |
|---|---|---|---|
| 8 | 2/6 @98 | 3/7 @102 | **mismatch — gb 4 dots early** |
| 16…64 | 4/0, 3, 2, 1, 0, 1, 2 | 4/0, 3, 2, 1, 0, 1, 2 | all match |

Same shape as the (now-closed) mid bands: **only the x8 tile is wrong.** gb's x8 GetTile0 fires at line_dot 98 on
EVERY line; DocBoy's BG_GETTILE0 dot VARIES with the per-line sprite (ly=28 → 99, ly=4 → 102) because the obj-stall
tracks the sprite. For the left-edge sprite (ly 0-7, OAM x=0, `obj_fetch.rs:88` `stall=OBJ_FETCH_MAX_ALIGNMENT_STALL_DOTS=5`
at match_tile=0) DocBoy pushes x8 ~4 dots later than gb; gb's x8 stays at 98 and samples one SCY write early
(gb writes land +4 vs hardware, so the scy=3 write that DocBoy reads by dot 102 only reaches gb at ~104). gb's
startup-seam overhead sits AFTER x8 (ly=4 gb x8→x16 = 15 dots vs DocBoy's normal 8), so x8 fires before it.

**Point-1 fix (the clean P3 close):** reposition the left-edge-sprite startup/obj-stall overhead so x8's data-stage
read lands ~4 dots later (at the oracle dot), WITHOUT changing total mode3 length (#245 hard constraint: per-sprite
Mode-3 cost byte-identical, wilbertpol 117 + shootout 264 must stay green — prove it). i.e. move ~4 dots of the
x8→x16 startup-seam overhead to before x8. **The naive entry-delay-skip lever (`first_real_push_skips_entry_delay`,
state.rs:2362) was measured to have NO effect on x8's dot — REFUTED.** The carrier is elsewhere in the PostAlignment
seam (the seed-fetch / `delayed_background_tileindex_read` / fill timing). Once x8 lands at the oracle dot, §5.3
samples band 0-7 correctly and the seed fix + retarget tables can be DELETED (P3 exit criteria b/d), reaching CGB
24/24 with a net seam-count DECREASE. Residual after band 0-7 closes: rows 22 & 69 (3 stray px) — re-measure then.

## 19. BAND ly0-7 ROOT CAUSE — the obj-alignment-stall lets the lead BG fetcher race through x8; §18 hypothesis REFUTED (2026-06-15)

Per-dot reproduction (gb `GBCYCLE_SCY_PROBE_LY` per-dot trace at `advance_mode3_pipeline` + GetTile0/TileDataLow0
emits, vs DocBoy `build-trace-cgb GBTRACE_LY`; baseline px-diff `actual.png` vs `expected-0.png` = **41px = band
ly0-7 38px + strays row22:1, row69:2**). The §18 framing ("startup-seam overhead / seed-fetch / delayed reads /
fill timing") is **REFUTED** — the carrier is the **obj-alignment-stall × fetcher-lead interaction**, not the seam
counters.

### 19.1 The two contrasting bands (storm frame, scy writes gb @80,88,96,104,…; latency-net-0 ⇒ §5.3 reads LIVE scy at TileDataLow/0)

| | gb x0 GT0 / DL0 | gb x8 GT0 / DL0 (scy) | DocBoy x8 BG_GETTILE0 (bwfscy) | obj-fetch start (sx, stall) |
|---|---|---|---|---|
| **ly=28** (mid, PASSES) | 83 / 85 (clean) | 96 / **98 (scy=2 ✓)** | @99 (scy=2) | @87, sx=3, stall=2 |
| **ly=4** (band 0-7, FAILS) | 83 / **91** (obj-stalled) | 96 / **98 (scy=2 ✗)** | **@102 (scy=3)** | **@80, sx=0, stall=5** |

gb's x8 DL0 is **@98 on BOTH lines**; DocBoy's x8 GETTILE0 **varies with the sprite** (99 vs 102). To read scy=3 gb
needs x8 DL0 **@104** (gb writes are +4 vs DocBoy and gb reads live, so the scy=3 write lands @104; DocBoy reads it by
@102 with its −4 phase + 2-dot latency). So ly=4's x8 is **6 dots early**, ly=28's is correct.

### 19.2 Mechanism (per-dot, nailed)
- The test sprite is at **OAM x = 0** for ly0-7 (`sprite_trigger_x = sprite.x = 0`), and at x≥3 for the mid bands.
- For **x=0** the obj-fetch hit is pending at `match_x=0` from the FIRST mode3 dot, so gb starts it **@80** — before
  the BG seed tile (x0) is even fetched. Its alignment stall (`sprite.x==0 ⇒ OBJ_FETCH_MAX_ALIGNMENT_STALL_DOTS=5`,
  `obj_fetch.rs:88`) therefore lands on **x0** (gb stalls x0 @85-91), NOT x8. For x=3 (ly=28) the hit fires @87 (after
  x0), stalling **x8** — which is why the mid bands are already oracle-correct.
- DocBoy fetches BG tile x0 FIRST (GETTILE0@83, push@88), THEN does the x=0 obj fetch during the FIFO-drain wait
  (dots 88-101), THEN x8 GETTILE0@102. The x=0 stall always delays **x8** on hardware.
- **The fetcher-lead is why scoped levers fail:** during the obj alignment stall the BG fetcher is **advanced**
  (`core.rs:95-96` `if handled && in_alignment_stall { advance_bg_fetcher }`) so it "catches up" and finishes 160px by
  `mode0_start_dot`. With the **large x=0 stall (5)** the lead fetcher races through x8's ENTIRE GetTile0→TileDataLow/0
  during the stall, latching SCY ~6 dots early. The mid bands' small stall (≤2) only advances x0's tail, so x8 lands right.

### 19.3 Refuted scoped experiments (all measured per-dot + px-diff, all reverted; tree clean)
| # | Experiment | Result |
|---|---|---|
| A | Defer obj-fetch start during `AlignmentSeedPending` (FifoBackedTransfer only) | obj re-fires via `QueuedBgFill` @88; the alignment stall still advances BG through x8 → x8 DL0 **@92 scy=1** (WORSE) |
| B | Defer obj-fetch start during `AlignmentSeedPending` (all sources) | same: x8 DL0 **@92 scy=1**, alignment-stall races BG through x8 (WORSE) |
| C | Suppress the BG catch-up during the obj alignment stall in the CGB startup seam (`core.rs:96`) | x8 moves the right way (98→**103**) but **undershoots by 1** (needs 104), and BREAKS the mid bands → **162px** (new fails ly64-71, ly128-135 that DEPEND on the catch-up) |

### 19.4 Design tension (why this is the structural fetcher-lead rework, not a lever)
The `core.rs:96` BG-catch-up-during-alignment-stall is **REQUIRED** twice over: (1) for `mode0_start_dot`
conservation (the #245 per-sprite Mode-3 cost — `extend_mode3_by_one_dot` runs per obj dot regardless, but the BG must
finish 160px by then), and (2) the mid bands rely on it to land x8 correctly. But that same catch-up is exactly what
lets the large x=0 stall race the lead fetcher through x8's data stage, sampling SCY early. **You cannot toggle the
catch-up to fix band 0-7 without breaking the mid bands / length.** The hardware-true fix must **decouple the startup
continuation tile's tile-data READ (the §5.3 SCY sample) from the lead fetcher's stage-advance during the obj stall**:
the fetcher may advance its stage (length-conserving), but the SCY/tile-data commit for that tile must occur at the
post-stall (true-fetch) dot — i.e. a deferred SCY sample fired `alignment_stall_remaining` dots after the lead crosses
TileDataLow/0, equivalently "the lead-fetched startup tile re-samples SCY at the dot it would have reached without the
lead." This is the **fetcher-lead hardening** the branch is named for; it touches the #245-adjacent obj-stall region
and must prove `mode0_start_dot` byte-identical for every sprite-x + wilbertpol 117 + shootout 264 green.

**Status: root cause nailed, 3 scoped levers refuted, structural approach specified. Tree clean (probes/experiments
reverted). Next = the deferred-SCY-sample-for-the-lead-startup-tile model (a phase decision, flagged HARD per §6).**

## 20. CANONICAL MODEL — how SameBoy + DocBoy structure the startup BG/OBJ interleave (the §19 fix spec, 2026-06-15)

Read SameBoy `Core/display.c` (`render_pixel_if_possible` / `advance_fetcher_state_machine` / the object-fetch loop)
and DocBoy `src/docboy/docboy/ppu/ppu.cpp` (the `bgwin_*`/`obj_*` fetcher tick selectors). **Both converge on the SAME
model, and it differs structurally from gb-cycle.** This is the spec for the §19 fix.

**Canonical fetcher (SameBoy lines 1951-2025; DocBoy `bgwin_pixel_slice_fetcher_push` ~2322-2398):**
1. SCY is latched **once at GetTile0** (DocBoy `bwf.scy = scy` @ `bg_prefetcher_get_tile_0` ~2021; consumed for the
   tile-data address at `setup_bg_pixel_slice_fetcher_tile_data_address` ~2617). (= §12 CGB-D corroboration.)
2. After fetching a tile (GetTile0→…→HIGH1) the fetcher enters **PUSH, which BLOCKS until the BG FIFO has room**
   (DocBoy: `can_push_to_bg_fifo = bg_fifo.is_empty()`; SameBoy: FIFO has space). **The next tile's GetTile0 does NOT
   start until that push succeeds.** The fetcher does NOT pre-fetch — it idles at PUSH holding the fetched tile.
3. The **object fetch starts only after** the BG fetcher has fetched the first tile (SameBoy gate line 1956:
   `while (fetcher_state < GET_TILE_DATA_HIGH_T2 || fifo_size == 0) advance…` ⇒ obj waits until fetcher ≥ HIGH **and**
   FIFO primed; DocBoy: obj fetch is launched from inside the PUSH handler, `is_obj_ready_to_be_fetched()`). During the
   obj fetch the **BG fetcher is FROZEN** (SameBoy line 943 the tile-x calc only advances `!during_object_fetch`; DocBoy
   the bwf stays in PUSH). It does NOT run ahead.
4. ⇒ For an x=0 sprite: x0 fetched (dots 83-88), PUSH-blocks (FIFO full from the startup placeholders), obj fetch runs
   FROZEN during the drain (dots ~88-93), FIFO drains lx0-7 (dots 94-101), **x8 GetTile0 @102** (push finally succeeds).
   The x=0 obj fully delays x8. Length is conserved because the fetcher had a FULL FIFO while frozen (no lost pixels).

**gb-cycle divergence (the three coupled defects):**
- (a) **Pre-fetch / fetcher-lead:** gb's fetcher does GetTile0→data eagerly right after the seed push and BLOCKS at the
  Push stage *after* fetching (dots 102-110), so x8's TileDataLow/0 SCY sample fires ~6 dots early (@98). Canonical:
  GetTile0 is gated on push-success, so the sample is late.
- (b) **Obj-start gate missing:** gb starts the x=0 obj fetch @dot80 (before the BG fetched tile x0; gb's FIFO has
  startup placeholders so it looks "primed"), charging the stall to x0. Canonical gate requires fetcher ≥ data-high
  (first REAL tile fetched) first, so the obj always lands on x8.
- (c) **Freeze vs advance:** gb ADVANCES the BG during the obj alignment stall (`core.rs:96`) for length conservation;
  canonical FREEZES it (and conserves length via the full-FIFO-at-PUSH surplus). gb can't simply freeze (exp C: the BG
  isn't at a full-FIFO PUSH when frozen, so it falls behind → mid bands' right edge breaks, 162px).

**Why no scoped lever works (confirmed):** the correct SCY-sample dot must track the **canonical GetTile0**, which is
gated on push-success + frozen-obj. That dot is early for mid sprites (x≥3, gb @98 ≈ oracle @99 — already right) and
late for x=0 (oracle @102). A fixed deferral, a refetch-on-push, or toggling the catch-up each break one side. The
sample dot is intrinsically a function of the obj position, reproducible only by the canonical interleave.

**Fix spec (the option-1 implementation):** restructure the CGB startup BG/OBJ interleave to the canonical model,
`is_cgb_family()`-gated and confined to the startup seam (AlignmentSeedPending/PostAlignment), leaving the steady path
and DMG untouched:
- (i) gate the startup continuation tile's GetTile0 (and its SCY latch, §5.3) on the previous tile's push succeeding
  (FIFO has room) instead of eager pre-fetch;
- (ii) gate the startup obj-fetch start on the BG having fetched its first real tile (≥ data stage), so the x=0 stall
  lands on x8;
- (iii) freeze the BG fetcher during the startup obj fetch, conserving `mode0_start_dot` via the full-FIFO surplus
  (NOT via the `core.rs:96` advance).
- **Gate every step:** `mode0_start_dot` byte-identical for every sprite-x (probe), CGB m3_* register-change tests +
  wilbertpol 117 + shootout 264 + mooneye + mealybug DMG all green. On success, delete the seed fix + retarget tables
  (P3 b/d). This is the fetcher-lead hardening; it is large and touches the #245-adjacent region — land incrementally.

**Refuted experiment D (the §20 (ii)+(iii) flag-toggle combination, 2026-06-15):** defer the obj-start past the seed
push AND freeze the BG during the startup obj alignment stall (both `is_cgb_family()` + seam-gated). Result: band ly0-7
still **38px** (x8 DL0 lands @103, scy=2 — STILL 1 dot short of the @104 needed for scy=3), the mid bands re-break
(127px total: ly64-71, ly128-135, ly136-143), and `mode0_start_dot` shifts (ly=4 m0 263→268, **length NOT conserved**).
⇒ The canonical push-blocks/freeze model is **not reachable by toggling `core.rs:96` + the obj-start gate** on top of
gb's pre-fetch pipeline: gb's BG fetcher is not at a full-FIFO PUSH when frozen, so freezing both loses pixels (mid
bands) AND miscounts the length. The startup BG-fetch state machine itself must be restructured to the canonical
"fetch → PUSH-blocks-on-FIFO-room → obj-during-PUSH-frozen → next GetTile0" shape (§20 (i)). Confirmed: 4 flag-level
experiments (§19.3 A/B/C + D) exhausted; the fix is the state-machine restructure, a dedicated incremental effort.

## 21. CANONICAL STARTUP REFACTOR — gb target + increment plan (AUTHORIZED 2026-06-15)

The user authorized rebuilding the mode3 STARTUP to the canonical model (§20), accepting that unit/integration tests —
and temporarily ROM tests — may go red, to be re-greened after. **ROM tests are the priority gate.** gb's steady state
is already canonical (push cadences the next GetTile0 at 8 dots, no lead); the **fetcher-lead is startup-only** (the
abstract placeholder/seed/QueueFill model lets the seed push succeed early), so the rewrite is scoped to the mode3
startup. See [[project_ppu_canonical_refactor]].

**Regression dashboard (baseline @ commit 2bcb48e9, this branch):** blargg 58, mooneye 113, wilbertpol 117,
gb-emulator-shootout 264, mealybug DMG 24/24, mealybug CGB **23/24** (m3_scy_change 41px = band ly0-7 38 + rows 22/69).

**gb-target canonical startup (replaces the abstract model):**
- The first BG tile fetch fills the FIFO with 8 real pixels (no `startup_fifo_placeholders` pre-fill); SCX&7 are
  discarded from the FIFO front before the first visible output.
- After each tile, the fetcher enters PUSH which BLOCKS until the FIFO has room; the NEXT tile's GetTile0 (and its SCY
  latch) does not start until the push succeeds — no pre-fetch.
- The obj fetch starts only after the BG has fetched its first real tile (FIFO primed) and FREEZES the BG fetcher
  for its duration; pixel output is paused during the fetch. Length conserved by the full-FIFO surplus, NOT by the
  `core.rs:96` BG-advance-during-stall.

**Increment sequence (each: full ROM dashboard, accept temporary red, log deltas):**
1. **Gate the startup continuation GetTile0 on FIFO-room** (kill the pre-fetch) — the core fetcher-lead removal.
2. **Gate the startup obj-fetch start on first-real-tile-fetched** + freeze BG during it (remove `core.rs:96` for the
   startup, conserve length via the now-full FIFO).
3. **Collapse `startup_fifo_placeholders` / `Mode3StartupSourceState` (EntryDelay/Abstract)** into the canonical
   "first real fetch fills FIFO + SCX discard".
4. **Delete the seams** (P3 b/d): `BgStartupFetchSeamState` continuation slices, `cgb_startup_seed/frozen_tile_row`
   seed fix, `cgb_dmg_software_startup_visible_tile2/3` retarget tables, the recompute current-row override.
5. **Re-green** unit/integration tests against the new canonical structure; regen CPU-invisible machine-trace fixtures.

**DELETION LIST (the seams this refactor must remove — success = these gone, dashboard restored + CGB 24/24):**
`startup_fifo_placeholders`, `Mode3StartupSourceState`, `startup_fetch_idle_dots`(MODE3_BG_FETCH_STARTUP_DUMMY_DOTS),
`post_alignment_fetch_restart_delay_dots`, `BgStartupFetchSeamState`+`BgStartupContinuationSlice`, the seed fix,
the `cgb_dmg_software_startup_visible_tile2/3` tables, `cgb_dmg_scy_startup_retarget_active`.

### 21.1 INCREMENT 1 LANDED (WIP) + the critical blast-radius finding (2026-06-15)

Increment 1 implemented: `cgb_startup_continuation_fetch_blocked_on_fifo_room` (bg_fetch.rs) holds a CGB startup
PostAlignment continuation tile's GetTile0 while `fifo.len() > BG_TILE_WIDTH` — the canonical no-pre-fetch primitive
(the continuation tile waits for the previous tile's push to have FIFO room, exactly like the steady-state push the
abstract `QueueFill` startup bypassed). `is_cgb_family()`-gated, startup-scoped. fmt/lint clean.

**Effect (per-dot + px-diff, full ROM dashboard):**
- m3_scy_change band ly0-7: x8 TileDataLow/0 moved **98 → 104** (the canonical-ish dot), and the row-22 stray closed
  (41 → 40px). x8 still reads scy=2 not 3 — it lands exactly on gb's SCY write-dot 104, where the CPU write applies
  AFTER the PPU mode3 step, so the live read is one write behind (needs dot 105). This residual is the SCY-phase /
  §5.2-observation-at-the-write-boundary, NOT the fetcher timing (now correct). `mode0_start_dot` CONSERVED (263/260).
- **BLAST RADIUS IS CONTAINED TO CGB MEALYBUG** — the de-risking finding: wilbertpol **117/117**, gb-emulator-shootout
  **264/264** (incl. its DMG mealybug 24/24), mooneye **113/113**, mealybug DMG **24/24** ALL STAY GREEN. The only
  regression is mealybug **CGB 23→22/24**: `m3_lcdc_tile_sel_change` broke because the continuation tile's LCDC sample
  moved with the (now-canonical) fetch timing — and in the canonical model LCDC is latched at the SAME GetTile0 as SCY,
  so it RE-GREENS once the register latch point follows the canonical GetTile0. ⇒ **the CGB startup can be rewritten
  freely, gating only on the ~11-case CGB mealybug suite; the broad suites are safe.**

**Refuted en route:** the `>= BG_TILE_WIDTH` threshold (+1 dot) closes band ly0-7 (x8→105, scy=3) but breaks the
no-startup-obj mid bands ly64-71/ly128-135 (they need the `>8` dot) — the +1 is sprite-dependent (only the x=0-obj
band needs it). Increment 1 + the §20(ii) obj-start gate together = catastrophe (1247px, sprite-position coupling in
the #245 region + length blew to 268). ⇒ the +1 for band ly0-7 must come from the canonical obj-pausing-the-drain
(increment 2: obj-start-after-first-tile + register latch at canonical GetTile0 + sprite-position accounting), not a
threshold tweak.

**Next (increment 2):** move the CGB register latches (SCY §5.3, LCDC, the tables' targets) to fire at the canonical
GetTile0 produced by increment 1, and gate the startup obj-fetch start on first-real-tile-fetched with the
sprite-position accounting updated — re-greens `m3_lcdc_tile_sel_change` and supplies band ly0-7's +1. Gate: CGB
mealybug back to 23/24 then 24/24, broad suites stay green.

### 21.2 INCREMENT 2 ATTEMPT — the breakage is the +4 register-write phase, NOT the fetch timing (2026-06-15)

Tried gating the fetch-policy pipeline-snapshot reads (`startup_background_tilemap/tiledata/tileindex…`) off for CGB
so the continuation reads live: **no effect** (lcdc_tile_sel still 436px). The LCDC.4 tile-data-select is read live at
the data stage already, not via that snapshot. Reverted.

**Oracle measurement settles it:** DocBoy `m3_lcdc_tile_sel_change` storm x8 BG_GETTILE0 is at **dot 102 — IDENTICAL to
m3_scy_change**. So the canonical continuation fetch dot is 102 for both, and increment 1 (gb's continuation GetTile0
now ≈102, data stage 104) has the **CORRECT canonical fetch timing**. The lcdc_tile_sel regression is therefore NOT a
fetch-timing error — it is gb's **+4 register-write phase (§13, P0)**: gb writes its mode3 registers 4 dots later than
hardware, so reading at the canonical dot (102/104) yields the wrong value. The pre-inc1 read dot (98) happened to
satisfy LCDC.4 by coincidence of the write schedule; inc1 (104) breaks it. Band ly0-7's SCY needs read@105 for the
same reason (write@104 applies after the PPU step).

**Consequence — the two are coupled:** the canonical startup (read registers at the canonical GetTile0) only yields
correct values if the **register writes are observed with the canonical phase**. So increment 2 is the **§5.2 CGB
write-observation** (the `pending_write` countdown, doc-CORROBORATED 2 T-cycles) made canonical, so a read at the
canonical GetTile0 sees the write hardware would have applied by then. inc1 (canonical fetch timing) + §5.2 (canonical
write observation) together close the CGB mealybug register-change tests at the canonical dot; neither alone does.
§13 refuted shifting the whole CGB STAT phase, but the §5.2 per-register observation latency (NOT a phase shift) is the
remaining lever — apply it at the GetTile0 register latch only, validated per-register against DocBoy across
m3_scy_change AND m3_lcdc_tile_sel_change (and the other m3_* register-change tests).

## 22. INCREMENT 2 MEASURED — §5.2-as-positive-latency REFUTED at the per-register gate; the +1 obj-aware lever closes the x=0 band only (2026-06-15)

**The mandatory per-register oracle gate (§14 step 5 / §15.4 — the step attempts 1/§21.2 skipped) was finally RUN
with `bwfscy`/`lcdc4` emitted per tile. It re-grounds §21.2: a positive write-observation latency (the `pending_write`
countdown=2) is the WRONG DIRECTION and cannot be the lever.** Tooling: DocBoy `build-trace-cgb` now emits
`lcdc4=%u` (`lcdc.bg_win_tile_data`) alongside `GBT-TDATA`; gb side via an ephemeral data-stage probe (removed).

### 22.1 The measurement (gb data-stage read vs DocBoy `GBT-TDATA`, last frame, per tile)
| ROM / band | tile | gb @dot / value | oracle @dot / value | direction |
|---|---|---|---|---|
| m3_scy_change ly=4 (band 0-7) | x8 | **104 / scy=2** | **104 / scy=3** | gb reads 1 write STALE at the same dot |
| m3_scy_change ly=28 (mid, PASSES) | x8 | 101 / scy=2 | 101 / scy=2 | match (read dot not on a write boundary) |
| m3_lcdc_tile_sel ly=4 (band 0-7) | x8 | 104 / lcdc4=0 | 104 / lcdc4=1 | gb reads STALE |
| m3_lcdc_tile_sel ly=20 (band 8-39) | x8 | 102 / lcdc4=0 | 102 / lcdc4=1 | gb reads STALE at the same dot |

**Decisive finding:** the failing continuation tile reads its register at the *same dot* as the oracle but gets a
**staler** value — gb's CPU writes land +4 (and apply *after* the PPU mode3 step within a dot), so a read landing on a
gb write boundary misses that write. A positive observation latency makes the read *staler still* (exp h / §17 already
found "latency must be 0, not 2"). The fix must make the read see the **contemporaneous** write (fresher), i.e. land
the read one dot *off* the write boundary — a fetch-timing move, NOT a `pending_write` latency.

### 22.2 The landed fix (the user-approved "measured +1 obj-aware lever")
- **`cgb_startup_seed_obj_stall_extra_continuation_dot`** (`state.rs`): set when a left-edge sprite (`sprite.x == 0`)
  obj-fetch pays its alignment stall on the seed tile during `AlignmentSeedPending` (`obj_fetch.rs`), i.e. the obj
  starts before the seed is pushed and delays it. It holds the first PostAlignment continuation tile's GetTile0 one
  extra dot of FIFO drain (`cgb_startup_continuation_fetch_blocked_on_fifo_room` threshold `BG_TILE_WIDTH-1`), so the
  data-stage read lands at **oracle+1** (the steady-tile cadence) where gb's +4-phase write is visible. Consumed at the
  first PostAlignment `TileIndex/0` so it never extends later tiles. THIS is the whole landed change.
- **Tried + REVERTED — live LCDC.4 select at the CGB frozen-row data stage** (`bg_fetch.rs` low/high `dot0`): reading
  `self.lcdc` live for the tile-data base (consistent with the live-SCY row) closed lcdc_tile_sel rows 0-7 (436→372px)
  but it broadly changed the tile-data SELECT source from the latched pipeline LCDC to live for EVERY CGB-dmg-software
  BG tile (broke unit test `cgb_dmg_software_bg_fetcher_ignores_native_cgb_bg_attributes`, which pins the latched
  source) and did NOT close lcdc_tile_sel. A live LCDC.4 read is the right hardware-true target (§4) but belongs to the
  dedicated write-observation / Seam-2 effort (per-table, HALLAZGO-M2-aware), not a bolt-on here. Reverted.

### 22.3 Results (CGB-scoped, broad suites GREEN — #245 intact)
- **m3_scy_change CGB 40px → 2px**: band ly0-7 CLOSED. Residual 2px = row-69 cols 14-15, a **sprite-overlay artifact**
  (x8 SCY read MATCHES the oracle @93 scy=1 — NOT the continuation mechanism; the pre-existing row-22/69 stray family).
- **m3_lcdc_tile_sel CGB: unchanged at 436px** (inc1's regression; the live-LCDC.4 partial step was reverted — see §22.2).
- Broad gates unchanged: **wilbertpol 117/117, gb-emulator-shootout 264/264** (incl. DMG mealybug 24/24). CGB **22/24**
  (neither m3_scy_change nor m3_lcdc_tile_sel fully passes yet). fmt/lint clean; `cargo tests` green (no unit-test break).

### 22.4 Why the lever stops here (the §19.4/§20 entanglement, now per-register-proven)
The +1 lands the read off the write boundary **only when the write is exactly +1 away** (SCY ly0-7). For
**m3_lcdc_tile_sel rows 8-39** the LCDC write lands **+2** beyond the canonical dot (gb x8 moved 102→103 still read
lcdc4=0), and these bands are NOT obj-stalled-on-the-seed (sprite.x≠0), so the flag does not even fire. Broadening the
flag to "obj paid on tile 0" moved those x8 +1 but they still read stale (write is +2) — neutral, reverted. A single
fetch-timing offset cannot satisfy two ROMs whose register writes land at different dots; and the mid bands read
correctly at oracle+0, so a uniform +1 over-shifts them (§21.1). **Closing the non-x=0 LCDC bands needs the
write-OBSERVATION made canonical** — gb reading the contemporaneous CPU write at the fetch dot — which is the CGB
CPU↔PPU write-phase problem (§5.1/§13 territory, but per-register not a global STAT shift), i.e. the §20/§21 canonical
restructure, NOT a scoped lever. This re-confirms §19.4/§20 at the per-register/per-dot level.

**Status: the approved +1 obj-aware lever is landed (SCY band ly0-7 closed, m3_scy_change 40→2px), broad suites green,
`cargo tests` green, no regression. m3_lcdc_tile_sel stays at inc1's 436px (the live-LCDC.4 partial was reverted as a
broad/incomplete semantic change). The remaining bands (lcdc rows 8-39/129-143; scy row-69 sprite stray) are the
write-observation core — gb reads the register STALE at the same fetch dot as the oracle, a +4 CPU↔PPU write-phase
difference that a fetch-timing lever cannot fix per-register (proven §22.1/§22.4). Next = the canonical write-observation
(§20/§21 restructure / §5.x done in the fresher direction), which also subsumes the live-LCDC.4 read. Tree: only the
`cgb_startup_seed_obj_stall_extra_continuation_dot` flag lives; ephemeral gb probes removed; DocBoy `lcdc4` trace emit
kept (revert at P4/M5).**

## 23. WRITE-OBSERVATION CORE — the machine-level mechanism + why a scoped causal observation cannot close it (2026-06-15)

Grounded the "write-observation" against the actual machine scheduler (`machine/step.rs`, `scheduler.rs`), which
settles HOW to model it — and shows the scoped form is causally blocked.

### 23.1 The mechanism (already present, used today for LYC)
CPU writes to PPU MMIO are NOT applied inline. `step_cpu_micro_operation` detects a PPU-MMIO write, buffers it in
`pending_ppu_mmio_write { address, value, commit_delay_t_cycles }`, and queues a `CommitMmioWrite` side-effect
(`step.rs:806-818`). The commit runs in `step_mmio_side_effect_commit` (`step.rs:904-936`) via
`commit_pending_ppu_mmio_write` → `ppu.write_register_with_source(.., CpuMmioCommit)`. A `commit_delay_t_cycles`
already exists — set to 1 for **LYC (0xFF45) on CGB at normal speed** (`step.rs:808-812`) — i.e. the codebase already
models a CGB-specific deferred PPU-register commit. This is the natural home for any CGB write-observation latency.

### 23.2 The phase order (the causal constraint)
`SchedulerPhase::ORDER` (`scheduler.rs:69-79`) per T-cycle: … `AutonomousPeripheralTicks` (**PPU tick**, phase 4) →
`BusArbitration` → `CpuMicroOperation` (**CPU executes + queues the MMIO write**, phase 6) → `MmioSideEffectCommit`
(**commit to the PPU**, phase 7) → … So within a T-cycle the **PPU ticks BEFORE the CPU's write is even queued**, and
the commit lands AFTER. A write queued at T-cycle N commits at N phase 7 and is first visible to the PPU tick at
**N+1**. The PPU therefore observes every CPU MMIO write **one T-cycle late, structurally — identical on DMG** (which
passes m3_scy_change). `commit_delay_t_cycles` can only push this **later** (staler), never earlier.

### 23.3 Why a scoped write-observation cannot close the non-x=0 LCDC bands
The failing tiles read the register **stale at the same fetch dot the oracle reads it fresh** (§22.1). To match, gb's
fetch read must see a write that gb's CPU commits ~2-3 dots in the FUTURE of the fetch dot (the net −2 of §13, a
look-ahead from gb's frame). The phase order makes that **causally impossible** for a PPU-side observation: at the
PPU tick the contemporaneous write is not yet queued, and `commit_delay` only defers. The only causal levers are:
1. **Move the CPU's write earlier** (shift the CGB STAT/handler phase) — **REFUTED (§13/P1)**: it broke 10+ m3_*
   CGB tests, because the OUTPUT-time registers (BGP/OBP0, read at the later output dot) match gb's current phase.
2. **Move the fetch read later** (the +1/+2 timing lever) — **position-dependent, does not generalize** (§22.4): SCY
   needs +1, LCDC +2-3, mid bands need +0; one offset cannot serve all.
3. **Reorder the PPU tick after CPU+commit** (globally, or a CGB mode3 sub-tick) — broad, changes VRAM/OAM access and
   every register observation; would regress the DMG-passing model.

### 23.4 The real asymmetry (why bgp/obp0 pass but scy/lcdc.4 don't)
All CGB m3_* tests write via the same handler at the same gb dots (80,88,96,…). **OUTPUT-time** registers
(BGP/OBP0/palettes) are consumed at the *output* dot — late, off gb's write boundary — so gb matches the CGB fixture.
**FETCH-time** registers (SCY, LCDC.4 tile-data-select) are consumed at the *fetch* dot. inc1 moved the CGB startup
continuation FETCH to the oracle's canonical dot, which for these registers lands **on gb's write boundary** → stale.
Pre-inc1 the fetch sat earlier (off the boundary) and lcdc_tile_sel PASSED. So inc1's canonical fetch timing is correct
for the oracle's phase but wrong for gb's phase; a single CPU↔PPU phase can't make BOTH the fetch-time and output-time
reads land correctly (that is the §13 dead-end restated structurally).

### 23.5 Consequence — paths (decision needed; none is a scoped lever)
- **(A) Defer lcdc to the full canonical restructure** (§20/§21 increments 3-5): rebuild the startup so the
  continuation read lands at gb's natural off-boundary dot AND the obj-interleave is canonical, handling the
  fetch-vs-output phase holistically. Large; the user-authorized direction. Land the clean SCY +1 win now (CGB 22/24,
  scy 40→2px, no regression), accept lcdc at inc1's 436px as a tracked WIP regression.
- **(B) Narrow inc1's continuation block to the x=0 obj-stall case**: non-x=0 lines keep pre-inc1 (off-boundary) fetch
  timing → lcdc rows 8-39 re-pass, but the row-22 SCY stray re-opens (scy 2→4px) and rows 0-7 lcdc stay — a trade, not
  a close.
- **(C) A CGB mode3 PPU-tick / register-observation reorder** confined to fetch-time registers — the only causal way to
  get "fresher", but it is a console-timing change with broad regression surface (must keep DMG + every other m3_* CGB
  green); essentially component §5.1 re-attempted at the observation layer rather than the STAT layer.

## 24. CANONICAL RESTRUCTURE — full design (oracle maps complete; BOTH layers authorized, NO ROM-gating during the structural pass, 2026-06-15)

The user authorized the **full canonical restructure of BOTH layers** to make gb-cycle match SameBoy + DocBoy + Pan
Docs as closely as possible, and explicitly directed: **do NOT measure ROM tests during the structural pass / stop
chasing the CGB ROM tests — go toward canon first, re-green/refine later.** This supersedes the scoped-lever framing of
§22-§23. Five parallel oracle/code maps (DocBoy `core/core.cpp`+`ppu/ppu.cpp`, SameBoy `Core/display.c`, gb-cycle
`mode3/*`+`state.rs`+`scheduler.rs`+`machine/step.rs`) settled the canonical model and the exact gb divergence.

### 24.1 The canonical model (DocBoy + SameBoy converge — this is the target)

**BG fetcher** (DocBoy 8 states `bgwin_prefetcher_get_tile_0`→…→`bgwin_pixel_slice_fetcher_push`, ppu.cpp:449-471;
SameBoy 7 states `GB_FETCHER_GET_TILE_T1`→…→`GB_FETCHER_PUSH`, display.c:862-872):
- SCY latched **once at GetTile0** on CGB (`bwf.scy = scy`, ppu.cpp:2021; consumed for tile-data row at
  `setup_bg_pixel_slice_fetcher_tile_data_address`, 2606-2640). DMG re-reads live `scy` per GetTileDataLow (bitplane
  desync). Doc-CORROBORATED (mealybug PPU doc, CGB-D latches once at the `B` stage).
- After GetTileDataHigh1, **PUSH BLOCKS until the BG FIFO is empty** (DocBoy `can_push_to_bg_fifo = bg_fifo.is_empty()`,
  2344; SameBoy `if (fifo_size(&gb->bg_fifo) > 0) break`, 1084). The FREEZE is structural: DocBoy `tick_fetcher` only
  advances the fetcher when `!enable_pixel_slice_fetcher_push` (1747-1752); `enable_push` is set true after
  GetTileDataHigh1 (2319) and cleared only when PUSH succeeds (2368) or an obj fetch launches (2392). **The next
  GetTile0 does NOT start until the push succeeds — no pre-fetch / no fetcher-lead.**

**OBJ fetcher** (DocBoy 6 states `obj_prefetcher_get_tile_0`→…→`obj_..._merge_with_obj_fifo`, 2402-2565):
- Obj fetch launches from **inside the PUSH handler** only when `is_obj_ready_to_be_fetched()` (oam_entries[lx] not
  empty && lcdc.obj_enable, 1819-1824) — i.e. after the BG has fetched its first real tile and reached PUSH. If the
  FIFO is full, the in-flight BG tile is **cached** (`cache_bg_win_fetch`, 2857-2862) and restored after the obj
  (2864-2869). SameBoy gate: `while (fetcher_state < GET_TILE_DATA_HIGH_T2 || fifo_size == 0) advance` (1956) — obj
  waits until the BG fetcher passed the data-high fetch AND the FIFO primed.
- During the obj fetch the **BG is FROZEN** (DocBoy stays in PUSH with `enable_push=false` so only obj states tick;
  SameBoy `during_object_fetch` removes the +1 from the tile-x calc, 943). Multiple sprites at the same lx loop
  (2548). After the last, restore cached BG + re-enable PUSH (2563).
- **Length is conserved** because the obj runs DURING the PUSH stall, which would have stalled anyway (the FIFO is
  full). Frame length stays fixed regardless of sprite count (ppu.cpp:2322-2397 length-conservation note).

**Startup** (SameBoy `mode_3_start`, display.c:1845-1872): FIFOs cleared, **8 junk pixels pushed** into the BG FIFO,
fetcher = GET_TILE_T1. **SCX&7 discarded from the FIFO front** during `render_pixel_if_possible` (687-704:
`(position_in_line & 7) == (SCX & 7)` ends the discard). The first REAL tile fetch then PUSH-blocks on those 8 junk
pixels until they drain — there is NO separate "abstract" transfer source; the junk pixels ARE real FIFO entries.

**CPU↔PPU phase** (DocBoy `core/core.cpp`): within EVERY T-cycle (t0/t1/t2/t3) the CPU ticks, THEN the PPU ticks
(`cpu.tick_t*` at 55/96/154/195 before `ppu.tick()` at 63/115/162/217). So DocBoy's PPU observes a CPU MMIO write the
SAME T-cycle. Per-register CGB write-observation latency is then layered LOCALLY in the PPU (`pending_write.{lcdc,scy,
…}.countdown`, ppu.cpp:703-726; SCY/LCDC countdown=2 on CGB). LCD enable is instant (`turn_on()`), re-enable line0 is a
full glitched 456-dot line.

### 24.2 gb-cycle divergence (the exact deltas to fix)

| Aspect | gb-cycle (now) | Canonical | Layer |
|---|---|---|---|
| Startup FIFO | abstract `startup_fifo_placeholders` count overlaying the FIFO (`effective_fifo_is_empty`/`fifo_contains_real_pixels`/`consume_effective_fifo_pixel`, state.rs:1842-1857) | 8 REAL junk pixels in the FIFO, SCX&7 discarded from the front | 1 |
| Transfer source | `Mode3StartupSourceState{EntryDelay,Abstract,FifoBacked}` abstract window (state.rs:565,1787-1834) + `startup_pre_visible_transfer_dots_remaining` + `transfer_phase=Priming` | none — the FIFO drains naturally; real fetch fills it | 1 |
| First fetch | `startup_fetch_idle_dots = MODE3_BG_FETCH_STARTUP_DUMMY_DOTS(3)` (state.rs:1712) | first GetTile0 fires immediately; cadence comes from PUSH-blocks | 1 |
| Push gate | `current_bg_push_dot_ownership` has `WaitingForEmptyFifo` (canonical) but startup uses `QueueFill`/`EntryDelay`/seed (bg_push.rs:36-139) | always PUSH-blocks on real FIFO-empty; obj-from-push when primed | 1 |
| Seed/continuation | `BgStartupFetchSeamState{AlignmentSeedPending,PostAlignment+VisibleTile2/3 slices}` (state.rs:2552) + `post_alignment_fetch_restart_delay_dots` | none — uniform canonical fetch from the first tile | 1 |
| Obj-start gate | starts at `match_x` even before the seed is pushed (FIFO "looks primed" via placeholders), charging the x=0 stall to x0; BG **advanced** during the stall (`core.rs:96` catch-up) | obj launches from PUSH after first real tile; BG **frozen**; length via full-FIFO surplus | 1 |
| CGB SCY sampling | seed fix `cgb_startup_seed_get_tile_scy_row`/frozen row (bg_fetch.rs:294-310) + `cgb_dmg_software_startup_visible_tile2/3` retarget tables (transfer.rs:329-462, mode3_policies.rs) + `cgb_dmg_scy_startup_retarget_active` + inc1/inc2 levers | plain SCY-once-at-GetTile0 latch, no obj-phase retarget | 1 (read point) + 2 (freshness) |
| CPU↔PPU order | `scheduler.rs` ORDER: PPU `AutonomousPeripheralTicks`(idx3) → `BusArbitration`(4) → CPU `CpuMicroOperation`(5) → `MmioSideEffectCommit`(6). PPU sees writes 1 dot late, structurally | CPU+commit BEFORE PPU tick (same-T-cycle observation) | 2 |
| Write-observation | immediate store (api.rs SCY) + the `pending_ppu_mmio_write.commit_delay_t_cycles` (LYC=1 CGB) machinery in step.rs:806-936 | DocBoy-style per-register countdown=2 (CGB SCY/LCDC), local to the PPU | 2 |
| LCD enable | `CPU_LCDC_ENABLE_EFFECT_DELAY_T_CYCLES=5` + re-enable line0 length 448 (ppu.rs:61-68) | instant `turn_on`; re-enable line0 = 456 (glitched full line) | 2 |

### 24.3 DELETION LIST (success = these gone; from §21 + the maps)

M1 `startup_fifo_placeholders` + `effective_fifo_is_empty`/`fifo_contains_real_pixels`/`consume_effective_fifo_pixel`/
`pop_visible_fifo_pixel` placeholder special-cases. M2 `Mode3StartupSourceState` (+ `startup_source_state`,
`consume_startup_transfer_entry_delay_dot`, `consume_startup_source_window_dot`, `startup_pre_visible_transfer_dots_remaining`,
`consume_startup_pre_visible_transfer_dot`, `Mode3TransferPhase::Priming`). M3 `startup_fetch_idle_dots`
(`MODE3_BG_FETCH_STARTUP_DUMMY_DOTS`, `consume_startup_fetch_idle_dot`). M4 `BgStartupFetchSeamState` +
`BgStartupContinuationSlice` + `post_alignment_fetch_restart_delay_dots` + the alignment-seed push path
(`queue_bg_startup_alignment_seed_from_fetcher`, `queue_startup_alignment_from_push`, `begin_post_alignment_followup`,
`advance_startup_background_fetch_tile`, `peek_startup_background_fetch_origin`, `StartupAlignmentSeed/Fill/Continuation`
origins). M5 — none. M6 the CGB SCY seam: `cgb_startup_seed_get_tile_scy_row`/frozen-row override,
`cgb_dmg_software_startup_visible_tile2/3` tables (transfer.rs:329-462,530-..., mode3_policies.rs:983/1009),
`cgb_dmg_scy_startup_retarget_active`, `scy_obj_phase_owner/policy`, `apply_startup_scy_tiledata_latch_to_fill`,
`startup_scy_tiledata_latch`, `compute_startup_visible_*` family, the `startup_visible_tile3_scx_boundary_*` fields. M7
the inc1/inc2 levers `cgb_startup_continuation_fetch_blocked_on_fifo_room`, `cgb_startup_seed_obj_stall_extra_continuation_dot`.
(The `recompute_live_background_cached_slice` current-row override and the DMG-window seams are touched but largely
retained — they serve DMG live-write semantics; only the CGB-startup-SCY portions go.)

### 24.4 Execution plan (canon first, re-green later; each step compiles)

The increments are COUPLED through the M1 placeholder abstraction (the transfer model reads the effective-FIFO
interface), so there is no tiny safe lever — Capa 1 is a coherent rewrite landed across a few compiling checkpoints.

- **L1-a — canonical startup FIFO + SCX discard (replace M1).** Push 8 real junk pixels at mode3 entry; route the
  transfer model + obj arbitration to the REAL `fifo.is_empty()`/`fifo.len()`; SCX&7 discard pops from the FIFO front.
  Delete the `effective_*`/placeholder interface. Checkpoint: compiles; DMG startup uses the real FIFO.
- **L1-b — canonical PUSH-blocks + no pre-fetch for the startup (replace M2/M3/M4).** Route the startup through the
  steady-state `WaitingForEmptyFifo` push ownership; first GetTile0 immediate; next GetTile0 gated on push success.
  Delete the abstract source window, the dummy idle dots, the seed/continuation seam.
- **L1-c — canonical obj interleave (replace `core.rs:96` catch-up).** Obj launches from PUSH after the first real
  tile; FREEZE the BG during the obj fetch; conserve `mode0_start_dot` via the full-FIFO surplus (cache/restore the
  in-flight tile like DocBoy). Delete the catch-up + the inc1/inc2 levers (M7).
- **L1-d — plain CGB SCY-once-at-GetTile0 (replace M6).** Latch SCY once at GetTile0 reading the live value (Layer 2
  will make it fresh); delete the retarget tables + seed fix + obj-phase policy.
- **L2-a — scheduler CPU→PPU order.** Move `CpuMicroOperation`+`MmioSideEffectCommit` before
  `AutonomousPeripheralTicks` (re-home `BusArbitration` accordingly), so the PPU observes CPU writes same-T-cycle, as
  DocBoy. Carefully re-validate the bus-owner/DMA/IRQ interlock against the oracle trace (this is the foundational,
  highest-blast-radius step).
- **L2-b — DocBoy per-register write-observation.** Replace the immediate SCY store + the bespoke `commit_delay`
  machinery with a uniform per-register countdown (CGB SCY/LCDC=2), local to the PPU, serialized in save-state.
- **L2-c — canonical LCD enable.** Instant `turn_on`; re-enable line0 length 456. Re-validate the wilbertpol
  `lcd_restart`/`intr_2_*` model against the oracle (was pinned to 448).

Re-green (unit/integration tests + fixture regen + the full ROM dashboard) happens AFTER the structural pass, per the
user's directive. Layers 1 and 2 are orthogonal (fetcher interleave vs write observation) and may land independently.

### 24.5 PROGRESS LOG + resume brief (durable; task list does NOT persist across restarts)

**Committed (branch ppu/fetcher-lead-hardening, all green: 616/616 ppu + integration, fmt/clippy clean):**
- `31b78213` docs §24 design.
- `1b8197d7` **L1-a**: BG FIFO pre-filled with 8 real junk pixels at `start_line` (state.rs); seed fill no longer
  re-materializes dummies (bg_push.rs). `startup_fifo_placeholders` now counts leading junk still in the FIFO.
  Behavior-equivalent (dot-89 FIFO + first-visible@92 preserved).
- `22178c38` **L1-b step 1**: removed `Mode3TransferBacking` + `effective_fifo_is_empty`; `current_transfer` readiness
  reads the real FIFO (`fifo.is_empty()`); snapshot `bg_current_transfer_backing` kept as a diagnostic derived from the
  source window. Behavior-equivalent.

**SameBoy startup-latency finding (display.c, 2026-06-15 — reframes the remaining L1-b):** the 12-dot mode3 fill latency
is NOT purely emergent in SameBoy either. Before the `mode_3_start` rendering loop SameBoy adds EXPLICIT setup delays:
`cycles_for_line = MODE2_LENGTH + 4` (line 1824) then `+= 3` (`GB_SLEEP(10,3)`, 1839) then `+= 2` (`GB_SLEEP(32,2)`,
1843) — ~9 dots of explicit setup before the fetcher starts; at `mode_3_start` it pushes 8 junk
(`fifo_push_bg_row(...,0,0,0,...)`, 1851) and sets `fetcher_state = GET_TILE_T1` (1855). ⇒ gb's entry-delay(4) +
dummy-fetch-dots(3) are the CANONICAL-EQUIVALENT of SameBoy's setup `GB_SLEEP` delays and should be KEPT (cleanly), **NOT
eliminated**. The actual non-canonical accretion to remove is the **SEED-SEAM (M4)** — `BgStartupFetchSeamState`
(AlignmentSeedPending/PostAlignment + VisibleTile2/3 continuation slices) + `post_alignment_fetch_restart_delay_dots` +
`queue_bg_startup_alignment_seed_from_fetcher`/`queue_startup_alignment_from_push`/`begin_post_alignment_followup` —
which has NO SameBoy analog (it is gb's CGB-retarget machinery), plus the residual placeholder COUNT (M1:
`fifo_contains_real_pixels`, `pop_visible_fifo_pixel` special-case) once the seam is gone.

**Remaining L1-b plan (do as one coherent pass; gate on `cargo test -p gb-core --lib ppu::` + `--test snapshots ppu`):**
1. Route the first real BG tile push through the SAME canonical path as continuation tiles (kill the alignment-seed
   special push): delete `queue_bg_startup_alignment_seed_from_fetcher` + the `startup_alignment_seed_pending()` branch
   in `advance_bg_fetcher_tile_data_high_dot1` (bg_fetch.rs) so the first tile uses `queue_bg_push_from_fetcher`.
2. Collapse `BgStartupFetchSeamState` to a simple "startup until first real push completed" flag (or remove it if the
   continuation-slice/delayed-read bookkeeping turns out CGB-only and folds into L1-d). Remove
   `post_alignment_fetch_restart_delay_dots` + `begin_post_alignment_followup` + `advance_startup_background_fetch_tile`.
3. Remove the `startup_fifo_placeholders` COUNT: `fifo_contains_real_pixels` becomes a positional/real-FIFO check (the
   push obj-start gate is L1-c), `pop_visible_fifo_pixel` drops its count special-case, `consume_effective_fifo_pixel`
   becomes a plain pop. KEEP the explicit startup output delay (entry-delay) — it is canonical (SameBoy `GB_SLEEP`).
4. Keep `mode0_start_dot` byte-identical (probe the per-sprite Mode-3 cost) and first-visible@92 / mode0=252 for DMG.
   Rewrite the startup unit tests ONCE to assert canonical state (push real junk; no seam snapshots). Fixture regen via
   `FIXTURE_ACCEPT_ENV` if the snapshot format changes.

**To resume in a fresh session:** the durable state is the 3 commits above + [[project_ppu_canonical_refactor]] +
this §24. Say "continúa con el trozo estructural de L1-b" — read §24.4/§24.5 + the SameBoy finding, then execute the
4-step plan against the unit-test oracle. The abstract startup is ONE interlocked mechanism — remove the seam whole,
do not peel pieces (peeling churns the same startup tests twice, proven this session).

**Status: write-observation core mapped to the machine scheduler; a scoped causal observation is proven blocked by the
PPU-before-CPU phase order. The clean SCY +1 increment stands; closing lcdc_tile_sel needs (A) the canonical restructure
or (C) a fetch-time observation reorder — a console-timing decision, not a lever.**

### 24.6 L1-b structural — BLAST-RADIUS MAP + SCOPE DECISION (5-agent map, 2026-06-15)

A 5-agent blast-radius map (bg_fetch / state / bg_push / transfer-policies-snapshot / tests) settled the exact
edit points. Constant check: `MODE3_ABSTRACT_SOURCE_WINDOW_DOTS = MODE3_BG_FETCH_PRIMING_DOTS(12) -
MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT(4) = 8` (start_line pushes 8 real junk pixels; snapshot tests that expect 7 are
"8 minus 1 consumed", NOT the constant). Junk pixels are real FIFO entries with `cached = None`; real pixels carry
`cached = Some(..)` — so the COUNT has a positional replacement (leading `cached==None` run).

**KEY FINDING — the seam is NOT cleanly separable from M6 (CGB SCY seed-fix = L1-d).** Four entanglement points:
(1) `StartupContinuation(VisibleTile2/3)` origin feeds `scy_uses_startup_visible_tile2_tilemap_row` in
`mode3_policies::for_push/for_fill_pending_slice`, reached by BOTH the CGB and the **non-CGB** `live_scy_write_routing`
path (api.rs:703+) — i.e. it touches DMG live-write SCY retarget, not only CGB; (2) `StartupAlignmentFill` origin is
read by `mark_live_scy_write_while_startup_alignment_fifo_visible` + `apply_startup_scy_tiledata_latch` (the M6 SCY
latch); (3) `cgb_startup_seed_get_tile_scy_row` has use-A (seed-specific freeze, gated `AlignmentSeedPending`, goes
with the seam) and use-B (canonical CGB-D data-stage row latch, gated `!AlignmentSeedPending`, the §16-§17 latch that
STAYS — when the seam is gone its guard simplifies to always-on = the canonical "latch SCY once at GetTile0");
(4) `scy_obj_phase_owner/policy` (M6) reads `startup_fetch_seam != Inactive` and the count.

**SCOPE DECISION (user, 2026-06-15): "remove whole + adelantar L1-d".** Combined L1-b+L1-d pass — delete the seed-push
(M4) + the COUNT (M1) + the CGB SCY seed-fix tables (M6) + M7 levers together. Accept CGB `m3_scy_change` churn +
observability-unit-test rewrite; **preserve ONLY the DMG static gates** (next paragraph). NO ROM-gating during the pass
(per §24). The canonical end state: 8 junk drain via the kept entry-delay (`startup_fetch_idle_dots=3` +
`startup_source_state=EntryDelay{4}` + per-push `entry_delay_remaining=1`), first real tile = Ordinary canonical
`queue_bg_push_from_fetcher`, no seam enum, no count, plain CGB SCY-once-at-GetTile0 (the kept use-B latch).

**DMG static gates that MUST stay byte-identical (the oracle; all in tests/mode3/startup_core.rs unless noted):**
- dot-89 BG FIFO == `[0,0,0,1,2,3,0,1,2,3]`; dot-80 FIFO == `[0;8]` (`:403/:417`).
- first visible pixel @ `line_dot == 92`, SCX=0 (`:430`).
- `mode0_start_dot == 252` SCX=0 (`:396`); SCX=N → 252+N; SCX=3 → 255 (terminal.rs:88).
- 3 visible tiles in order 0,1,2 (`:467`); SCX shift SCX=2 → `[2,3,0,1,2,3,0,1]` (`:507`).
- fetcher 3-dummy-dot idle then first tile-index read @ dot 83 (`:397/:407`).
- per-sprite Mode-3 cost: LCDC1-off-mid-objfetch keeps timing (obj/render.rs:268); x160 terminal sprite still +1
  (obj/arbitration.rs:470); obj fetch start window (obj/fetch.rs:112).

**CADENCE RISK (the one empirical unknown):** removing the seed's `entry_delay=0`+immediate-advance, the +1
`post_alignment_fetch_restart_delay_dots`, and the first-continuation `take_startup_first_real_push_skip_entry_delay`
(entry_delay=0) nets to a tuned cadence; naive removal drifts ~+1 dot and would break dot-89/first-visible@92. Resolve
against the oracle: re-distribute the kept 12-dot setup (e.g. trim a dummy/entry-delay dot, or land the first push
immediate) so the static gates stay byte-identical. mode0_start_dot is a CONSTANT (80+172), unaffected by fetch cadence.

**DELETION LIST (confirmed by the map; success = all gone):** M4 — `BgStartupFetchSeamState` (whole enum) +
`AlignmentSeedPending`/`PostAlignment` + `BgStartupContinuationSlice` + `begin_post_alignment_followup` +
`advance_startup_background_fetch_tile` + `take_startup_first_real_push_skip_entry_delay` +
`maybe_finish_startup_fetch_seam` + `peek_startup_background_fetch_origin` + `startup_background_tile{index,map,data}_*`
+ `startup_alignment_seed_pending` + `post_alignment_fetch_restart_delay_dots`
(+`consume_bg_fetcher_post_alignment_restart_delay_dot`) + `queue_bg_startup_alignment_seed_from_fetcher` +
`queue_startup_alignment_seed_from_fetcher` + `queue_startup_alignment_from_push` + `is_startup_alignment_seed` +
origins `StartupAlignmentSeed`/`StartupAlignmentFill`/`StartupContinuation` + `queued_fill_origin` +
`startup_dummy_pixels`. M1 — `startup_fifo_placeholders` + `fifo_contains_real_pixels` (→`!fifo.is_empty()`) +
`consume_effective_fifo_pixel` (→`pop_real_fifo_pixel`) + `pop_visible_fifo_pixel` skip-block + the bg_push.rs:105
`+placeholders` window + mode2.rs:147 + the `PpuMode3ScyObjPhaseContext.startup_fifo_placeholders` field. M6 —
`cgb_startup_seed_get_tile_scy_row` use-A + `cgb_dmg_software_startup_visible_tile2/3` tables (transfer.rs ~329-462,
mode3_policies ~983/1009) + `cgb_dmg_scy_startup_retarget_active` + `scy_obj_phase_owner/policy` +
`apply_startup_scy_tiledata_latch*` + `startup_scy_tiledata_latch` + `mark_live_scy_write_while_startup_alignment_fifo_visible`
+ `compute_startup_visible_*` + `startup_visible_tile3_scx_boundary_*`. M7 —
`cgb_startup_continuation_fetch_blocked_on_fifo_room` + `cgb_startup_seed_obj_stall_extra_continuation_dot`. KEEP —
`push_dummy_fifo_pixels` (the canonical 8-junk pre-fill), the entry-delay machinery (`startup_fetch_idle_dots`,
`startup_source_state=EntryDelay`, per-push `entry_delay_remaining`), the CGB-D use-B SCY data-stage latch (now
always-on), `bg_current_transfer_backing` diagnostic.

**Tests:** delete pure-seam tests (startup_post_alignment_seam_*); rewrite ~30 files that set
`startup_fifo_placeholders`/`startup_fetch_seam`/`PostAlignment`/`StartupAlignmentFill` to canonical (real junk via
`push_dummy_fifo_pixels`, FIFO-length-driven ownership, Ordinary origin). Snapshot/trace schema changes
(`PpuBgStartupFetchSeamSnapshot`, `bg_startup_fifo_placeholders`, `bg_fetcher_post_alignment_restart_delay_dots`,
`startup_dummy_pixels`) → regen 18 trace fixtures + snapshot via `FIXTURE_ACCEPT_ENV`. Gate at the end:
`cargo test -p gb-core --lib ppu::` + `--test snapshots ppu` (CGB m3_scy churn allowed; re-green later).

### 24.7 LANDED — L1-b + L1-d remove-whole pass (2026-06-16, branch ppu/fetcher-lead-hardening)

The combined pass is DONE and the full CI is GREEN: `cargo fmt-check` clean, `cargo lint` clean, `cargo tests` 0
failed across all 48 binaries (14 ignored = 5 new canonical-pending below + 9 pre-existing), `cargo rom-report
blargg` 58/58. The mode3 startup is now the canonical model: 8 real junk pixels pre-filled at `start_line`, the first
real BG tile pushes through the ordinary `queue_bg_push_from_fetcher` path (entry_delay=1, no seed), the junk drains
one per dot, no seam / no `BgCachedSliceOrigin` / no placeholder count / no CGB SCY seed-fix retarget / no continuation
labeling / no scx-boundary-on-tile3. The kept canonical pieces: the explicit startup entry-delay
(`startup_fetch_idle_dots=3` + `startup_source_state=EntryDelay{4}` + per-push `entry_delay_remaining=1`), the 8-junk
pre-fill, the CGB-D "SCY-once at the data stage" latch (`cgb_startup_seed_get_tile_scy_row`, now always-on for
cgb+dmg-software, replacing the deleted seed retarget), and `bg_current_transfer_backing` (diagnostic).

**DMG static gates held byte-identical** (validated, untouched): dot-80 FIFO `[0;8]`, **first visible @ dot 92**,
`mode0_start_dot=252` (SCX-shifted +N), the 3 visible tiles in order, the SCX low-bit pixel-phase shift, fetcher
3-dummy-dot idle. The cadence self-compensated: removing the seed's immediate-advance + the +1 restart + the
entry-delay skip shifted the FIRST real fill from dot 89→90, but the junk drain absorbs it so first-visible stays @92.
The only intermediate change is the dot-89 FIFO snapshot (`[0,0,0,1,2,3,0,1,2,3]` → `[0,0]`) — updated in the gate
test. Fixture regen verified behavior-neutral: across all 18 trace fixtures the ONLY differing retained fields are the
internal BG-fetcher/FIFO mode3-startup state (`bg_fifo_len`/`bg_stage`/`bg_push_pending`/`bg_fill_pending` — the
canonical junk-drain) plus the dropped diagnostic fields; NO mode-boundary / `line_dot` / `mode0_start_dot` /
`visible_pixels_output` / bus / CPU field changed (so the CPU/OAM-bug fixtures these traces serve are unaffected).

**`PpuMode3ScyObjPhasePolicy`/`scy_obj_phase_*` were KEPT** (not deleted): they are live beyond the seam — the surviving
`pending_refetch_prefers_*` obj-match-phase routing flows through them. Only the seam-specific bits
(`startup_visible_tile2_*`, the `startup_fifo_placeholders` context field, the dead route accessors) were removed.

**5 tests `#[ignore]`'d as canonical-pending (re-pin via ROM after L1-d/L2 fresh value, NOT regressions):**
`cgb_fetch::cgb_dmg_software_bg_high_plane_reuses_low_plane_scy_tiledata_row` +
`..._low_dot_scy_write_reuses_low_plane_row_for_high_plane` (CGB-DMG live-SCY high-plane row-reuse, subsumed by /
removed with the use-B latch & M6); `lcdc_bg_toggles::sprite_coupled_tile_sel_replay_matches_curated_background_windows`
+ `..._line10_tile_sel_replay_matches_trace_signature` (observed LCDC3/4 startup phase-table replay removed with M6);
`terminal::saturated_placeholder_backed_terminal_bg_tail_holds_one_extra_dot_after_push_entry_delay` (the
`terminal_placeholder_tail_extra_hold` mechanism has had NO production trigger since before this pass — vestigial).

**Remaining minor cleanup (deferred, harmless, noted for the re-green/polish pass):** two now-vestigial fields survive
as always-false dead state — `BgFetcherState::cgb_dmg_scy_high_plane_uses_low_row` (its only setter,
`maybe_latch_cgb_dmg_scy_low_row_for_high_plane`, went with M6; still read at bg_fetch.rs but the branch is never taken)
and `BgPushState::terminal_placeholder_tail_extra_hold_remaining` (vestigial). Remove them + their reads when
re-greening the 5 ignored tests. Layer 2 (scheduler CPU→PPU order, per-register write observation, canonical LCD
enable) is unstarted.

### 24.8 L2-a blast-radius MAP + EXECUTION PLAN (5-agent map, 2026-06-16, user chose "map first")

Five parallel oracle/code maps (DMA+bus interlock, pinned timing constants, IRQ/bus-owner timing, tests+fixtures,
DocBoy/SameBoy oracle grounding) settled the L2 surface. The reorder itself is data-driven: edit
`SchedulerPhase::ORDER` (scheduler.rs:69); dispatch is a pure `match context.phase()` (step.rs:332), so changing ORDER
reorders both execution AND the emitted trace lines with no dispatch edit. Target order: `ExternalEventIngress,
MasterClockTick, DerivedEdgeResolution, BusArbitration, CpuMicroOperation, MmioSideEffectCommit,
AutonomousPeripheralTicks, InterruptAggregation, CpuWakeInterruptEvaluation` (CPU+commit move ahead of the PPU/DMA/APU
tick).

**KEY REFRAMING — L2-a is NOT a standalone scheduler reorder.** Moving CPU before the PPU tick ALONE would *double-delay*
the Mode2 STAT IRQ and regress the ~10 `m3_*` tests sharing the Mode2-STAT handler, because the
`*_hidden_from_same_cycle_cpu_if` family (irq.rs:265 mode0, :377 mode2, :387 mode1, :395 lyc; + the
`pending_interrupts_hidden_from_cpu_if` mask ppu/api.rs:1539, set via `stat_request_hidden_from_same_cycle_cpu_if`
irq.rs:579) is a same-cycle-VISIBILITY correction that EXISTS to compensate for PPU-before-CPU. Under CPU-first it is
redundant/inverted and must be RETIRED in the same pass. The STAT pretrigger constants (the hard-coded `+4` mode0/mode2
lead irq.rs:286/308/315, `DMG_MODE2_VBLANK_ENTRY_STAT_PRETRIGGER_DOTS`/`CGB_COMPAT_…`=4, `LINE_153_LYC0_STAT_IRQ_PRETRIGGER_DOT`=8
via −4, `CGB_…`=8 via −1) collapse by ~1 dot. So **L2-a bundles four moves:** (1) ORDER reorder; (2) retire the
hidden-from-cpu-if visibility layer; (3) re-derive the pretrigger / LY-read-advance / line-153 LYC-compare constants
against the oracle; (4) a DMA "armed-this-cycle → tickable-next-cycle" latch so OAM-DMA startup delay survives the
peripheral tick now running AFTER the CPU write.

**Oracle target CONFIRMED (DocBoy `/Users/pakitovic/workspace/DocBoy/src/docboy/docboy/`; design-note line numbers were
stale, mechanism TRUE):** within each T-cycle `Core::cycle()` drives `tick_t0..t3`, each `gb.cpu.tick_t*()` BEFORE
`gb.ppu.tick()` (core.cpp:55/63, 96/114, 154/161, 195/216), DMA after both (`gb.dma.tick()` tick_t1:131/tick_t3:234,
+t0/t2 on CGB double-speed). SameBoy corroborates (SM83 access, then `GB_advance_cycles`→`GB_display_run` flushes the
PPU clock; timing.c:513).

**L2-b spec CORRECTED — NOT a uniform countdown=2** (§24.1/§24.2 stated it incompletely). DocBoy truth
(ppu.cpp `tick_pending_write` :712-738 runs at the END of `Ppu::tick()` :569 → committed value visible NEXT dot;
setters write_scy/scx/wx/lcdc :3516-3585; bus wiring cpubus.cpp:180-198):
- **DMG:** SCY/SCX/WX/LCDC-non-enable-bits/BGP/OBP/WY/LYC are IMMEDIATE (direct bus pointers / `write_*_real`). ONLY
  STAT is delayed (`pending_write.stat.countdown=1`, with STAT IE bits forced high for that 1 T-cycle).
- **CGB:** countdown=2 for LCDC-non-enable bits, SCY, SCX, WX. STAT immediate. LCDC.enable ALWAYS immediate
  (turn_on/turn_off applied before the latch). WY/BGP/OBP/LYC immediate.
- No double-speed branch — the countdown constant is fixed (2, or DMG-STAT 1) regardless of speed.
- CGB additionally freezes SCY into `bwf.scy` at GetTile0 (ppu.cpp:2021) — gb-cycle ALREADY keeps this as the
  always-on use-B latch (§24.7). So L2-b's job is the per-register countdown=2 store-latch, NOT the GetTile0 freeze.
⇒ gb-cycle's `commit_delay_t_cycles` (LYC=FF45 CGB normal-speed only, step.rs:808) is the WRONG shape: replace with a
per-register PpuRegisterWrite latch {LCDC-bits/SCY/SCX/WX → countdown=2 on CGB; STAT → countdown=1 on DMG}, decremented
at end of PPU tick, local to the PPU and serialized in save-state.

**Per-frente findings (condensed; cite the agent maps in session log):**
- *DMA/bus interlock.* Caches `cached_cpu_bus_arbitration_states`/`cached_ppu_bus_state_snapshot` memoize the FIRST
  read per T-cycle (reset step.rs:1146/1185). Today AutonomousPeripheralTicks reads first (post-tick PPU mode); under
  L2-a CpuMicroOperation reads first (PRE-tick mode) — this one-dot-earlier CPU view of PPU mode is the INTENDED effect
  (#1, leave it, validate). HIGH-RISK side-effects: HDMA HBlank-window (`VramDmaRuntimeContext` step.rs:462,
  dma.rs:616) and OAM-DMA FF46 arm (dma.rs:1049) — a transfer armed THIS cycle now ticks one T-cycle earlier,
  collapsing the startup delay; `dma.cpu_stall_active()` (step.rs:744/1005) stall edge shifts. Containment: freeze
  ppu.owner_bus_state/ly/dma.bus_state/cpu_stall_active in a per-cycle PRE-CPU snapshot + DMA armed-this-cycle latch.
  Gate on mooneye `oam_dma_*`, HDMA timing, VRAM/OAM-block.
- *Pinned constants.* WILL-SHIFT (re-derive vs oracle): the whole STAT-pretrigger family (above);
  `CPU_LCDC_ENABLE_EFFECT_DELAY_T_CYCLES`=5→almost-certainly 4 (countdown armed by CPU commit, decremented in PPU tick;
  reorder gives it one extra decrement); `LY_READ_ADVANCE_START_DOT`=451 + `LCD_REENABLE_LINE0_LY_READ_ADVANCE_START_DOT`=444;
  all `LINE_153_LYC*`/`CGB_LINE_153_LYC*` compare windows + `*_LY_READ_ZERO_DOT` (4/8) + `CGB_LINE_END_LYC_COMPARE_BLANK_DOTS`=3;
  `LINE0_VBLANK_WRAP_STAT_READBACK_DELAY_DOTS`=4. NEEDS-ORACLE-CHECK: boot/LCD-restart `line_dot` seeds
  (`LCD_REENABLE_INITIAL_LINE_DOT`, `DMG_REAL_BOOT…`=92, `CGB_BOOT_ENTRY_LINE_DOT`=173, boot bases 36/3992/235),
  `MODE3_INITIAL_SCX_CAPTURE_DOT`=3, and **speed.rs:52 double-speed tick parity** `t_cycle & 1 == 0` (may need to flip —
  the one item plain dot-offset reasoning doesn't cover; needs a CGB double-speed oracle run). STABLE: pure mode
  lengths/geometry (DOTS_PER_SCANLINE, MODE2/MODE3 lengths, MODE0_START_DOT base, internal fetcher windows).
- *IRQ/bus-owner.* Two channels: A = deferred scheduler buffer (Timer DerivedEdge:2 stays before CPU → NO shift;
  Serial AutonomousPeripheral → moves after CPU → IF-visible one cycle later). B = direct PPU state read (VBlank/STAT):
  commit-to-IF in InterruptAggregation (stays index 7, after PPU) is unchanged, but the CPU's LIVE in-cycle IF read
  (`cpu_visible_pending_interrupt_request_mask` step.rs:135 / ppu/api.rs:1537) now runs BEFORE this cycle's PPU tick →
  same-cycle visibility flips. Timer + Joypad are control anchors (no shift). The Mode2-STAT handler is the crux
  (m3_scy_change). `step_cpu_wake_interrupt_evaluation` deferral tables (step.rs:1016-1025) read post-PPU-tick state in
  BOTH orders → unaffected by the reorder, but may over/under-correct once the STAT-edge dot shifts (validate
  wilbertpol lcd_restart / mooneye intr_2_*).
- *Tests/fixtures.* Hand-edit: `scheduler/tests.rs:6-18` (order array), `machine/tests.rs:732-741` regions
  `[Timer,Apu,Ppu,Cpu,Cpu]`→`[Cpu,Cpu,Timer,Apu,Ppu]`, rename `machine/tests.rs:2758`
  (`…during_phase_7…`), `scheduler_cycle_trace.txt` (NOT auto-blessed; reorder its phase lines). Auto-bless (env
  `GB_CYCLE_ACCEPT_{MACHINE,PHASE2,PHASE4,PHASE5,PHASE6,PRINTER}_FIXTURES`, helper tests/common/fixtures.rs:16): all
  machine_*/phase2/4/5/6 traces (expect substantive PPU/DMA-column diffs, not just line moves). Save-state: NO impact
  (only `next_t_cycle` serialized; `pending_ppu_mmio_write` reset to None on restore machine.rs:715). `tests/scheduler.rs`
  order assertion is self-referential (passes after reorder).

**EXECUTION PLAN (each checkpoint compiles; ROM-gated this time — Layer 2's goal IS to re-green, unlike Layer 1):**
- **L2-a.0 — pre-CPU peripheral snapshot + DMA armed-latch (containment FIRST, BEFORE the reorder, behaviour-neutral).**
  Freeze the per-cycle pre-CPU PPU/DMA picture and add the DMA "armed-this-cycle → tickable-next-cycle" latch while the
  order is still PPU-first (so it's a no-op now), so step L2-a.1 doesn't collapse OAM-DMA startup. Gate: full `cargo
  tests` still green (no behaviour change yet).
- **L2-a.1 — reorder ORDER + retire hidden-from-cpu-if + re-derive pretrigger/LY/LYC constants, together.** This is the
  coherent structural cut. Land it, then re-derive each WILL-SHIFT constant against the DocBoy/SameBoy oracle trace
  (NOT by chasing red tests blindly). Gate: `cargo fmt-check` + `cargo lint` + `cargo tests` + `cargo rom-report
  blargg`; then the dashboards — DMG `m3_scy_change` stays closed, the ~10 shared `m3_*` do not regress, wilbertpol
  117/117, mooneye 113/113. Regen fixtures last (env vars above) once behaviour is settled.
- **L2-b — per-register write-observation latch** (corrected spec above): replace `commit_delay_t_cycles` with the
  per-register countdown. Re-pins the fresh CGB SCY value → re-greens the 5 §24.7 `#[ignore]` tests and closes the CGB
  `m3_scy_change` VisibleTile2 bands (121px). Gate: the 5 ignored tests un-ignored + green, m3_* register-change green.
- **L2-c — canonical LCD enable** (instant turn_on; re-enable line0 = 456, not 448). Re-validate wilbertpol
  `lcd_restart`/`intr_2_*` against the oracle (currently pinned to 448).

Orthogonality: L2-a is foundational for L2-b (same-cycle observation) but L2-c can land independently. **To resume:**
read §24.8 + the 5 agent maps; start at L2-a.0 (containment), then the L2-a.1 coherent cut. Do NOT attempt the reorder
without retiring the hidden-from-cpu-if layer in the same change — proven (this map) to double-delay the Mode2 STAT IRQ.

### 24.9 L2-a.1 IN PROGRESS — reorder LANDED + damage measured (2026-06-16, branch ppu/fetcher-lead-hardening, UNCOMMITTED WIP, tree RED on purpose)

**L2-a.0 was DROPPED as a separate step** — closer reading proved it is not cleanly separable (mirrors the L1-b "not
separable" finding): (1) the "pre-CPU snapshot unification" IS the L2-a.1 behavioural change (it flips the CPU's view of
PPU mode from post-tick to pre-tick), not a no-op; (2) the DMA armed-latch is only verifiable post-reorder
(`elapsed_t_cycles` startup-delay is explicit at dma.rs:333, but the `pending_restart` path is order-coupled), so
pre-installing it blind gives false comfort. ⇒ L2-a is a single coherent verde→rojo→verde cut; the right first action
is "reorder + measure" to convert the §24.8 predictions into the real damage list.

**LANDED so far (uncommitted):**
- Reorder: `SchedulerPhase::ORDER` (scheduler.rs:69) now `…DerivedEdge, BusArbitration, CpuMicroOperation,
  MmioSideEffectCommit, AutonomousPeripheralTicks, Interrupt…` (enum decl left intact — only ORDER drives execution, so
  Ord/Serialize unchanged). Updated `scheduler/tests.rs` order array + `machine/tests.rs` regions
  `[Timer,Apu,Ppu,Cpu,Cpu]`→`[Timer,Cpu,Apu,Ppu,Cpu]`.
- **Cache-coherence bug FIXED (real, not cosmetic):** the end-of-PPU-tick snapshot (step.rs:683,
  `sync_video_domain_ownership` with the POST-tick owner) was hitting `cached_ppu_bus_state_snapshot` now PRE-populated
  by the CPU phase with the PRE-tick view. Added `refresh_ppu_bus_state_snapshot_with_observer` (resets the cache, takes
  a fresh post-tick snapshot). No-op under the old order (cache was None there); required under the new order. The
  `step_t_cycle_with_observer_reports_regions_*` ppu_regions list confirms it (BusSnapshot×2+PublishedAccess restored).

**MEASURED DAMAGE (full `cargo test -p gb-core --tests --no-fail-fast`):** lib 1472/0 GREEN; snapshots GREEN (mode3
internals unaffected by the reorder — the §24.7 fetcher work holds). Integration failures = exactly the §24.8 prediction:
- *Mechanical (trace-fixture regen — phase lines reorder):* phase2 (5), phase4 (5; verify OAM-state cols unchanged),
  phase5 (2), machine (2), dma (2 arbitration-trace), scheduler_cycle_trace.txt (1, hand-edit). ~17 tests.
- *Behavioural (the real fix work) — ppu.rs 11:* `ppu_lcd_restart::*` (7, incl. the literal
  `..._mode0_edge_is_not_visible_to_same_cycle_if_reads` / `..._mode2_pretrigger_is_not_visible_to_same_cycle_if_reads`
  — the hidden-from-cpu-if family), `ppu_mode_edges::{mode2_to_mode3_stat_probe, hblank_ly_scx_probe_*}` (3, the
  pretrigger/mode-edge constants), `ppu_oam_dma::...mode2_corruption_controller` (1). ROM suites
  (mooneye/wilbertpol/m3_*) NOT yet run.

**NEXT (the L2-a.1 fix phase, the substantive part):** (1) retire the `*_hidden_from_same_cycle_cpu_if` family
(irq.rs:265/377/387/395 + `pending_interrupts_hidden_from_cpu_if` api.rs:1539) — addresses the 7 lcd_restart visibility
tests + the Mode2-STAT handler dot; (2) re-derive the pretrigger / LY-read-advance / line-153 LYC constants vs the
DocBoy oracle (addresses ppu_mode_edges); (3) handle OAM-DMA startup (latch vs +1 constant, verified against
`oam_dma_*`); (4) regen the trace fixtures (env vars above) once behaviour settles; (5) ROM-gate
fmt/lint/tests/blargg + m3_* no-regress + wilbertpol 117 + mooneye 113. Then L2-b, L2-c.

**FIX-PHASE DIAGNOSIS (2026-06-16, still at commit 4f0b04e4 — recon only, no further code landed):**
- *Hide mechanism (confirmed):* the hide ONLY sets `pending_interrupts_hidden_from_cpu_if` (the same-cycle CPU IF-read
  mask, irq.rs:559-577); it does NOT change committed real IF (`pending_interrupts`, drained in InterruptAggregation)
  nor dispatch. CPU consumes it via `cpu_interrupt_mask_for_if_read`→`cpu_visible_pending_interrupt_request_mask`
  (step.rs:135). Under the new order the CPU IF read (phase 5) is BEFORE the PPU tick (phase 7), so the hide now
  OVER-persists by a cycle ⇒ retire it: both call sites `irq.rs:503` + `registers.rs:185` → `true`; delete the 4
  `*_stat_irq_edge_hidden_from_same_cycle_cpu_if` predicates + `stat_request_hidden_from_same_cycle_cpu_if`. KEEP the
  shared source helpers `ordinary_mode2_stat_pretrigger_{edge,source}`/`mode2_vblank_entry_stat_source`/
  `line_153_lyc0_stat_irq_pretrigger_source` (used by the real IRQ-line calc at irq.rs:130/161-162/401/454/499/541).
- *Retirement is NOT independently validatable:* the 7 `ppu_lcd_restart::*` tests ASSERT the removed mechanism → they
  must be REWRITTEN to canonical, not "made green". The retirement's gate is the ROM suites (mooneye intr_2_*,
  wilbertpol lcd_restart), not the existing unit tests.
- *Quantified shift (oracle-proxy probes ppu_mode_edges.rs):* `mode2_to_mode3_stat_probe` count (2,2) vs expected
  (1,2); `hblank_ly_scx` fails scx 3-8 but PASSES scx 0-2 ⇒ NOT a naive universal −1 on `LY_READ_ADVANCE_START_DOT`
  (451) — there is an scx-dependent interaction with the mode0-start dot. Shift is ~1 dot in the "CPU now observes PPU
  line_dot one behind" direction; exact per-constant value/direction needs DocBoy-oracle grounding per case (the scx
  asymmetry is the whack-a-mole trap that broke the 8 prior bounded fixes). Probes = fast oracle proxy (~4s); DocBoy
  `build-trace-cgb` + `GBCYCLE_SCY_PROBE_LY` = ground truth. **Do NOT guess constants — ground each vs the oracle.**

**FIX-PHASE PROGRESS (2026-06-16, grind started — "vamos con A"):**
- ✅ **LY-read advance = clean −1, VALIDATED.** `LY_READ_ADVANCE_START_DOT` 451→450 (ppu.rs:59) + the unit-test
  assertion (`DOTS_PER_SCANLINE - 5`→`- 6`, ppu/tests/stat/registers.rs:793). All 3 `hblank_ly_scx` probes GREEN
  (incl. scx 0-2, which already passed — so the −1 restores the exact pre-reorder observation without breaking the
  slack cases). Confirms the model: the reorder makes the CPU observe the PPU `line_dot` exactly one behind for a DIRECT
  register read, and the canonical fix is −1 on that CPU-comparison threshold (matching how DocBoy's constants are
  defined relative to its CPU-first order). KEPT uncommitted→ committed as a small WIP.
- ⚠️ **The STAT-visibility / pretrigger cases are NOT a uniform −1 — they need per-case work.** Two concrete signals:
  `mode2_to_mode3_stat_probe` count (2,2) vs (1,2) [STAT-edge-to-mode3 spacing]; `lcd_reenable_mode0_if_probe(59)`
  returns 0xE2 (visible) but should be 0xE0 — the mode0 STAT edge becomes IF-visible ONE delay-unit EARLIER. So the
  same-cycle IF-visibility STILL matters under the new order (the earlier "hide is redundant/no-op" reasoning is NOT
  confirmed — these probes prove same-cycle visibility shifted, not vanished). These involve the pretrigger leads
  (`+4`), the hide, and the IRQ-service phase together, and shift in directions that need grounding per case. ⇒ retire
  the hide and re-derive the pretrigger constants TOGETHER, validating each against its probe, NOT by a global rule.
- *Method confirmed working:* the curated ppu.rs probes ARE the fast oracle proxy — empirical loop (change → probe →
  read delta) is the right inner loop; full ROM suites are the outer gate. Remaining: STAT pretrigger family (mode2/
  mode0/vblank-entry), the LCD-reenable family (off-by-1 in the IF-visibility boundary), line-153 LYC, hide cleanup,
  OAM-DMA startup, then rewrite the 7 lcd_restart tests, regen fixtures, ROM-gate.

### 24.10 RESUME BRIEF — STAT-pretrigger + hide batch (cold-start handoff, 2026-06-16)

**State:** branch `ppu/fetcher-lead-hardening`, HEAD `a37d7779`, working tree CLEAN. Two L2-a.1 commits landed:
`4f0b04e4` (scheduler CPU→PPU reorder + `refresh_ppu_bus_state_snapshot_with_observer` cache fix), `a37d7779`
(`LY_READ_ADVANCE_START_DOT` 451→450). Tree is INTENTIONALLY red — this batch finishes L2-a.1. Read §24.8 (blast-radius
map) + §24.9 (diagnosis + progress) first.

**EXACT RED SET to clear (run `cargo test -p gb-core --test ppu --no-fail-fast`):** 9 behavioural ppu.rs tests:
- 7 `ppu_lcd_restart::*` — `lcd_reenable_{first_frame_mode0_stat_suppresses_pretrigger_and_keeps_scx_seams,
  arming_mode2_stat_during_oam_waits_for_the_next_oam_edge, line0_mode0_stat_uses_scx_grouped_irq_dots,
  line0_mode0_halt_wake_uses_the_scx_aligned_aperture, mode0_edge_is_not_visible_to_same_cycle_if_reads,
  mode2_pretrigger_is_not_visible_to_same_cycle_if_reads, prearmed_mode2_stat_services_on_the_first_oam_pretrigger}`.
- `ppu_mode_edges::mode2_to_mode3_stat_probe_matches_mooneye_counts`.
- `ppu_oam_dma::cpu_inc_hl_inside_fe_range_reaches_the_same_mode2_corruption_controller`.
PLUS ~17 MECHANICAL trace-fixture regens (phase2 5 / phase4 5 / phase5 2 / machine 2 / dma 2 / `scheduler_cycle_trace.txt`
hand-edit) — these are NOT behaviour, do them LAST via the env-var bless (below).

**VALIDATED METHOD (transferable):** the reorder makes the CPU observe the PPU `line_dot` exactly ONE behind for a
DIRECT register read (CPU runs in CpuMicroOperation, before the PPU tick) ⇒ −1 on that CPU-comparison threshold
(PROVEN on LY-read; restores the exact pre-reorder/oracle observation, doesn't break the slack cases). BUT the
STAT-visibility/pretrigger cases are NOT a uniform −1 and the hide is NOT a no-op — each needs per-case grounding
against its probe. The curated probes encode the mooneye thresholds = the oracle; inner loop = `cargo test -p gb-core
--test ppu <probe>` (~4s). DO NOT guess constants globally (the scx asymmetry + the 8 prior refutations are the trap).

**STARTING PROBE DELTAS (evidence captured this session):** `mode2_to_mode3_stat_probe` count actual (2,2) vs expected
(1,2) — the STAT-edge→mode3 spacing is 1 too wide at delay3. `lcd_reenable_mode0_if_probe(59)` returns 0xE2 (IF-visible)
but must be 0xE0 — the mode0 STAT edge goes IF-visible 1 delay-unit too EARLY (so same-cycle visibility shifted, not
vanished). `lcd_reenable_mode2_if_probe(109/110)` analogous (expect 0xE0/0xE2).

**EDIT POINTS (ready to act):**
- *Retire the hide* (do FIRST — structural before constants, or you tune twice): set both call sites to `true` —
  irq.rs:503 (`refresh_stat_irq_line`) + registers.rs:185 (the STAT-write edge path); then DELETE the 4 predicates
  `mode0/mode2/mode1/lyc_stat_irq_edge_hidden_from_same_cycle_cpu_if` (irq.rs:265-302 / 377-385 / 387-393 / 395-420) +
  `stat_request_hidden_from_same_cycle_cpu_if` (irq.rs:579-584). **KEEP** the shared source helpers
  `ordinary_mode2_stat_pretrigger_{edge,source}` / `mode2_vblank_entry_stat_source` / `line_153_lyc0_stat_irq_pretrigger_source`
  (used by the REAL IRQ-line calc at irq.rs:130/161-162/401/454/499/541, not just the hide). NOTE: after retiring, the
  cpu-if-visibility mask `pending_interrupts_hidden_from_cpu_if` may itself become dead → check `cpu_visible_pending_interrupt_request_mask`
  (ppu/api.rs:~1537) and `cpu_interrupt_mask_for_if_read` (step.rs:135) and simplify if so.
- *STAT pretrigger leads* (the `+4` family — re-derive per-case vs the probes): irq.rs:286 (mode0 `line_dot + 4 >=
  current_mode0_start_dot`), irq.rs:308/315 (`ordinary_mode2_stat_pretrigger_{source,edge}` `line_dot + 4`), irq.rs:339
  (vblank-entry via `mode2_vblank_entry_stat_pretrigger_dots`), the halt-wake deferreds irq.rs:454/463/472/481 (also
  `+4`). Constants: `DMG_MODE2_VBLANK_ENTRY_STAT_PRETRIGGER_DOTS`/`CGB_COMPAT_…` (ppu.rs:82-83),
  `LINE0_VBLANK_WRAP_STAT_READBACK_DELAY_DOTS` (ppu.rs:81).
- *Line-153 LYC*: `LINE_153_LYC0_STAT_IRQ_PRETRIGGER_DOT` + `CGB_LINE_153_LYC0_STAT_IRQ_PRETRIGGER_DOT` and the compare
  windows (ppu.rs:72-79).
- *LCD-reenable family*: `LCD_REENABLE_LINE0_LY_READ_ADVANCE_START_DOT` (ppu.rs:63, likely −1 like its sibling) + the
  reenable mode0/mode2 IRQ dots (`lcd_reenable_line0_mode0_irq_dot`, irq.rs).
- *LCD enable delay* (L2-c-adjacent but cheap): `CPU_LCDC_ENABLE_EFFECT_DELAY_T_CYCLES` 5→4 (ppu.rs:67) — armed by the
  CPU commit, decremented inside the PPU tick; the reorder gives it one extra decrement. Validate vs lcd-on-timing.

**ORDER OF ATTACK:** (1) retire hide → re-measure all 9; (2) re-derive STAT pretrigger constants per-case vs
`mode2_to_mode3` + the lcd_reenable_if probes; (3) LCD-reenable family vs the other lcd_restart tests; (4) line-153 LYC;
(5) OAM-DMA mode2-corruption (`ppu_oam_dma::cpu_inc_hl_inside_fe_range_…`); (6) REWRITE the 7 lcd_restart tests to the
canonical behaviour (they ASSERT the removed mechanism — rewrite, don't force-green); (7) regen the ~17 trace fixtures:
`GB_CYCLE_ACCEPT_{MACHINE,PHASE2,PHASE4,PHASE5,PHASE6,PRINTER}_FIXTURES=1 cargo test -p gb-core` + hand-edit
`crates/gb-core/tests/fixtures/traces/scheduler_cycle_trace.txt` (reorder its phase lines); (8) ROM-GATE: `cargo
fmt-check` + `cargo lint` + `cargo tests` + `cargo rom-report blargg` + mooneye 113 / wilbertpol 117 / m3_* no-regress.
Then L2-b (per-register write latch, §24.8 corrected spec) + L2-c (LCD enable 456). Oracle for hard cases: DocBoy
`build-trace-cgb` + `GBCYCLE_SCY_PROBE_LY` at /Users/pakitovic/workspace/DocBoy.

### 24.11 L2-a.1 FIX-PHASE BATCH 2 — unit tests closed (8/9) + CRITICAL ROM-scope finding (2026-06-16)

**State:** branch `ppu/fetcher-lead-hardening`, working tree DIRTY (uncommitted, net-positive, fmt+lint clean). Files
touched this batch: `ppu/control/irq.rs`, `ppu/control/published_stat.rs`, `ppu/control/registers.rs`,
`tests/ppu/ppu_oam_dma.rs`. (`ppu.rs` enable-delay was tried at 6 then reverted to 5 — see below.)

**CRITICAL FINDING — the reorder's ROM damage was never measured and is ~3× the brief's "9 unit tests".** §24.10/§24.9
said "ROM suites NOT yet run". They have now been run. The reorder commit `4f0b04e4` (+ LY −1 `a37d7779`), BEFORE this
batch, already regressed the ROM oracle hard:
- **Baseline at HEAD a37d7779 (hide still active): mooneye 108/113, wilbertpol 93/117.** (vs PR #245 main = 113/113,
  117/117.) So the reorder broke ~5 mooneye + ~24 wilbertpol ROMs — the 9 unit tests were the tip of the iceberg.
- Baseline mooneye fails (5): `boot_hwio-dmg0`, `intr_2_mode0_timing`, `intr_2_mode0_timing_sprites`,
  `intr_2_mode3_timing`, `intr_2_oam_ok_timing`.
- Baseline wilbertpol fails (24): `hblank_ly_scx_timing_variant_nops`, `intr_0_timing`, `intr_1_timing`,
  `intr_2_mode0_scx3/scx7_timing_nops`, `intr_2_mode0_timing`, `intr_2_mode0_timing_sprites(+_nops, +_scx1/2/3/4_nops)`,
  `intr_2_mode3_timing`, `intr_2_oam_ok_timing`, `intr_2_timing`, `ly_lyc-C/GS`, `ly_lyc_0-C/GS`, `ly_lyc_0_write-GS`,
  `ly_lyc_144-C/GS`, `ly_lyc_write-GS`, `vblank_if_timing`.

**THIS BATCH'S WORK (net-positive, ZERO regressions vs baseline):**
- **Unit tests: 8 of the 9 behavioural ppu.rs tests now GREEN** (was 0/9). The 1 remaining = `ppu_lcd_restart::
  lcd_reenable_line0_mode0_halt_wake_uses_the_scx_aligned_aperture` (scx1 only: 0x63 vs expected 0x62 — analysis below).
- **ROMs improved: mooneye 108→109, wilbertpol 93→101.** Fixed (8 wilbert + 1 mooneye): `hblank_ly_scx_…_nops`,
  `intr_0_timing`, `intr_2_mode0_timing_sprites_nops`, `…_scx1/2/3/4_nops`, `intr_2_mode3_timing` (both suites).
- The fixes (all reorder-compensation, delay stays 5):
  1. **Retired the hide** (§24.10 plan): both call sites `irq.rs`/`registers.rs` → `true`; deleted the 4
     `*_stat_irq_edge_hidden_from_same_cycle_cpu_if` predicates + `stat_request_hidden_from_same_cycle_cpu_if` +
     `ordinary_mode2_stat_pretrigger_edge` (only the hide used it). NOTE: retiring the hide changed NOTHING in the 9
     unit probes (proving it was dead FOR THOSE), and net-IMPROVED the ROMs (93→101) — so the "hide is dead" call was
     right for the cases it covered. `pending_interrupts_hidden_from_cpu_if` is now always 0 → the
     `cpu_if_visible` param of `queue_interrupt_request_with_cpu_if_visibility` + the mask field are dead-but-harmless;
     left for a follow-up simplification (not done — risk).
  2. **mode2 reenable pretrigger lead** `ordinary_mode2_stat_pretrigger_lead_dots()` = 3 during `blank_frame_active`, 4
     otherwise (fixes prearmed/arming/if_probe mode2; `blank_frame_active` guard keeps steady-state `mode2_to_mode0`
     green — DO NOT make it unconditional).
  3. **mode0 line0 reenable** `lcd_reenable_line0_mode0_irq_dot()` += 1 (counter + if_probe).
  4. **mode0 line0 halt-wake** `lcd_reenable_line0_mode0_halt_wake_dot()` += 1 (deferred-wake dot).
  5. **first-frame line1** `current_stat_irq_access_mode()` suppress-deferral now applies to ALL scx (dropped the
     `matches!(scx,3|7)` guard) + `current_mode0_stat_irq_start_dot()` suppress arm += 1.
  6. **mode2→mode3 published-STAT override** (`published_stat.rs`): fires one dot earlier (`line_dot == MODE2_DOTS - 1`
     with a post-tick `access_mode_for_line_dot(line_dot+1)==Drawing` check) so a CPU `ld a,(FF41)` observes mode3 at
     the canonical dot under the pre-tick read. (Fixed `intr_2_mode3` in BOTH suites — proves these unit-probe fixes
     DO propagate to ROMs when the probe genuinely mirrors the ROM.)
  7. **oam_dma test** (`ppu_oam_dma.rs`): the corruption uses the PRE-tick scan row (canonical = DocBoy CPU-then-PPU);
     the test captured the POST-tick snapshot row. Fixed the TEST to capture the row before the step (no production
     change — the pre-tick row is correct).

**halt-wake scx1 (the 1 remaining unit test) — root-caused, likely a genuine post-reorder value:** with edge `+1`
(needed by the counter/if_probe, confirmed via the mooneye `intr_2_mode0_*` family) scx1's mode0 STAT IRQ *pends* at
line_dot 253; the halt-wake deferral can only DELAY, so it cannot wake before 253 → b=0x63. The expected 0x62 came from
the old order's {scx0,scx1} grouping, which is unachievable once scx1's edge sits 4 dots after scx0's (the counter
REQUIRES that spacing: scx0→0x3D, scx1→0x3E). Measured wake-dot→count map (TIMA sum): `[249,252]→0x62, [253,256]→0x63,
[257,260]→0x64`. Do NOT chase scx1 with another `+1` — it cascades into the counter. Resolve only once the `intr_2_*`
ROM cluster is canonical; then either accept 0x63 (update the test) or the deeper fix falls out.

**DEAD END recorded:** `CPU_LCDC_ENABLE_EFFECT_DELAY_T_CYCLES` 5→6 fixes ALL reenable STAT IRQ + oam_dma in one shot
but BREAKS the `cpu_path_lcd_enable_read_{ly,stat}` probes (raster moves 1 late for the READ path). The read path
self-compensates the reorder (read-1-behind cancels raster-1-early) at delay 5; the IRQ path does not. So the enable
delay is a SHARED knob that can't satisfy both — keep it 5 and compensate the IRQ path per-case. This is why the fixes
are scattered, not one constant.

**REMAINING ROM FAILURES (the real L2-a.1 fix work — 4 mooneye + 16 wilbertpol):**
- `boot_hwio-dmg0` (mooneye) — boot HWIO; may be separate from STAT timing (verify on main/branch history).
- `intr_2_mode0_timing`, `intr_2_mode0_timing_sprites`, `intr_2_oam_ok_timing`, `intr_2_timing` (the base, non-`_nops`
  variants) — STAT mode0/mode2/oam IRQ timing. The `_nops` variants are FIXED; the base ones differ in CPU phase
  alignment. No fast unit proxy (the `mode2_to_mode0` unit probe PASSES yet `intr_2_mode0_timing` ROM FAILS — the probe
  is an INCOMPLETE proxy). Must ground vs the ROM directly (slow) or build a tighter probe.
- `intr_2_mode0_scx3/scx7_timing_nops` (wilbert) — scx-seam mode0; related to `current_mode0_stat_irq_start_dot` seam.
- `intr_1_timing`, `vblank_if_timing` (wilbert) — mode1/vblank STAT IF-visibility. Were failing at baseline too (NOT
  caused by the hide retirement). Need the vblank-entry / line-153 STAT pretrigger constants re-derived.
- `ly_lyc{,_0,_0_write,_144,_write}-{C,GS}` (8-9 wilbert) — LY-LYC coincidence STAT IRQ + readback. UNTOUCHED this
  batch. Likely needs the `live_ly_for_lyc_compare` windows / `regular_line_dot0_compare_window` (line_dot==0) / the
  `LINE_153_LYC*` + `CGB_LINE_153_*` compare constants shifted for the pre-tick CPU observation. Biggest single cluster.

**METHODOLOGY LEARNING:** the curated unit probes are NOT a complete oracle for these ROMs. `intr_2_mode3` and
`hblank_ly_scx` probes DID mirror their ROMs (fixing the probe fixed the ROM). But `mode2_to_mode0` passes while
`intr_2_mode0_timing`/`intr_2_oam_ok` ROMs fail — so for the remaining cluster there is no fast proxy; iterate against
the ROM suite (`cargo rom-suite {mooneye,wilbertpol}` then `cargo rom-report …`; ~2-3 min each) or first WRITE a tighter
unit probe that reproduces the ROM's exact CPU read/IRQ sequence. The trace-fixture regens (§24.10 step 7) are NOT done
yet (the ~17 phase2/4/5/machine/dma fixtures still need `GB_CYCLE_ACCEPT_*_FIXTURES=1`); do them only once the ROM
cluster is closed and behaviour is final.

**RESUME:** read §24.8–24.10 + this §24.11. Net-positive WIP is in the working tree (uncommitted, fmt+lint clean,
1472 lib + 45/46 ppu integration green). Next: pick the `ly_lyc` cluster (biggest, untouched) or the base `intr_2_*`
cluster; ground each against the ROM oracle. Target: mooneye 113/113, wilbertpol 117/117, m3_* no-regress, blargg green.

### 24.12 L2-a.1 FIX-PHASE BATCH 3 — LYC dot0 edge closed + differential-oracle method + full per-cluster diagnosis (2026-06-16)

**State:** branch `ppu/fetcher-lead-hardening`, working tree CLEAN. Commits this batch (on top of `567cdcd5`):
`f2e66727` (E1 — LYC dot0 IRQ defer, the real fix) and `685893d8` (TEMPORARY diagnostic sweep harness). ROM scoreboard
now **mooneye 109/113, wilbertpol 104/117, mealybug 13 fails (IDENTICAL to pre-batch baseline — zero m3 regression,
m3_scy_change DMG still closed), blargg untouched.** E1 fixed `ly_lyc-GS`, `ly_lyc-C`, `ly_lyc_write-GS` (wilbertpol
16→13) with zero unit/ROM regression.

**THE WINNING METHOD (use this — it is the missing piece §24.11 lacked).** The curated probes are an INCOMPLETE oracle;
the reliable loop is a **differential probe vs a `main` (PR #245) worktree**, because main passes every one of these ROMs
and is therefore the exact CPU-observable truth:
1. `git worktree add ../gb-cycle-main main` (HEAD 9956eb3b; it builds independently).
2. Fetch the real ROM source to learn what each round measures + the literal pass values — they are NOT in the repo:
   `curl -fsSL https://raw.githubusercontent.com/wilbertpol/mooneye-gb/master/tests/acceptance/gpu/<name>.s` (and
   `tests/common/common.s` for the `wait_ly`/`nops` macros). WebFetch paraphrases — use curl.
3. The sweep harness (`crates/gb-core/tests/ppu/ppu_oracle_sweep.rs`, commit `685893d8`, `#[ignore]`) has: a faithful
   reenable+readback probe (`run_reenable_readback`: di/IE=0/IF=0/LYC/STAT; wait_ly 144; LCD off;nop;LCD on; IF=0;
   wait_ly V; nops N; `ldh a,(target)` — captures the value the ROM stores), a vblank/mode1-entry sweep, a faithful
   `intr_2_mode0_timing` poll-until-mode probe, and `run_real_rom_capture_wram` which RUNS the actual `.gb` and dumps
   the first WRAM writes = the `round1..N` table (so you read the measured value the assert checks, no disassembly).
   NOTE: synthetic-cartridge `build_test_rom` writes the program at 0x0100 and the program byte at offset 0x47 lands on
   the cartridge-type header (0x147); a long program that puts 0x20 there is rejected as MBC6 — use
   `build_nom_bc_test_rom_with_program_entry(.., 0x0150, ..)` (entry jump + program at 0x150) and rebase the PC math.
4. Run `cargo test -p gb-core --test ppu ppu_oracle_sweep::<probe> -- --ignored --nocapture --test-threads=1` in BOTH
   trees, diff. The reorder shows up as a per-edge dot/read-position shift; CRITICAL distinction the data proved:
   **register READBACK (STAT mode bits, LY) already matches main; only IF/IRQ EDGES are shifted** (the §24.11
   "read path self-compensates, IRQ path does not", now confirmed mechanically).

**E1 — the LYC fix (LANDED, principled):** under the CPU-first order the CPU observes the raster one dot ahead, so a
regular-line LYC coincidence IRQ that rises at `line_dot 0` (via `regular_line_dot0_compare_window`) is seen one
read-position too early. Fix = defer it to `line_dot 1` by returning the `lyc_compare_latch` instead of the live dot0
compare in `lyc_coincidence_for_irq_line` (irq.rs:87-96). Readback path untouched. Proof: `ly_lyc-GS` round5 IF read
went `0xE2`→`0xE0` (= assert), `ly_lyc`/`ly_lyc_144`/`vblank_if` STAT+LY readbacks already matched main, mealybug
identical. The 9 pre-existing red `ppu::tests::stat::mode_edges::*_hidden_from_same_cycle_cpu_if*` lib tests (they assert
the retired hide / old dot0 timing) were ALREADY red at clean HEAD — §24.11's "1472 lib green" was inaccurate; they
still need the rewrite from the §24.10 plan and are unchanged by E1.

**REMAINING 13 wilbertpol + 4 mooneye, fully diagnosed (all reenable+measure tests). Each needs DEDICATED design, not a
constant tweak — two bounded fixes were REFUTED this batch (below). Order by tractability:**

- **vblank-entry cluster — `intr_1_timing`, `vblank_if_timing` (wb), `ly_lyc_144-GS/-C` (wb).** Ground truth: vblank +
  mode1 IF edge is observed exactly **1 read-position early** (main E0→E1/E3 between nops 105/106; branch 104/105). The
  STAT *mode* readback is already correct (separate published-stat path, lags via `line_dot-1`). **REFUTED EXPERIMENT
  #1:** deferring the internal queue by 1 dot (a `vblank_entry_irq_deferred` flag in api.rs:940 + returning Drawing from
  `current_stat_irq_access_mode` at ly==144,dot==0) DID align the read (fixed all 4) but REGRESSED
  `vblank_stat_intr-GS/-C` and `intr_1_2_timing-GS` (mooneye 4→7). ROOT CAUSE: deferring the queue defers DISPATCH too,
  but `vblank_stat_intr` measures dispatch (ei + serviced IRQ) which is ALREADY correct (dispatch reads post-PPU-tick
  state in both orders, §24.8). The real need: defer ONLY the CPU **IF-read** visibility by 1 read-position, leave the
  commit/dispatch at line 144 dot 0. The hide (`pending_interrupts_hidden_from_cpu_if`) cannot do it — it is cleared by
  the InterruptAggregation drain every cycle and the read sees the committed scheduler IF afterwards. This needs a real
  design: a 1-cycle-skewed "IF-register read value vs dispatch-pending state" for PPU interrupts under the reorder.

- **base `intr_2_*` cluster — `intr_2_mode0_timing`, `intr_2_mode0_timing_sprites`, `intr_2_oam_ok_timing`,
  `intr_2_timing` (mooneye+wb).** Faithful probe (mode2 STAT IRQ → nops → poll STAT until mode0, counting): main count
  transitions 2→1 at delay **45/46** (= asserts d=1@46, e=2@45); branch transitions at **46/47** (+1). The
  poll-until-**mode2** count MATCHES main (7→6 @47/48) — so the start (mode2 IRQ wake) and the mode2 readback are fine;
  only **poll-until-mode0 is +1**, i.e. the Drawing→HBlank (mode0) edge is observed one read-position late vs the
  mode2/mode3 edges. **REFUTED EXPERIMENT #2:** mirroring the mode2→mode3 published-stat "publish one dot earlier"
  override onto the mode0 boundary (`published_stat_steady_frame_mode0_boundary_override_applies` at
  `mode0_start_dot-1`) blew up — lib stat 9→**21** failures (the published mode0 readback is load-bearing across the
  suite). The mode0-vs-mode2/3 read asymmetry is the crux; the published-stat `line_dot-1` lag + the two existing
  overrides need re-derivation per-edge against the probe, not a blanket earlier-publish. `intr_2_mode0_scx3/scx7_nops`
  (wb) are the scx-seam siblings (same mode0-edge machinery, `current_mode0_stat_irq_start_dot` seam).

- **`ly_lyc_0-GS/-C`, `ly_lyc_0_write-GS` (wb) — line-153 LYC0 wrap.** Reenable; wait_ly 152; nops; read LY/STAT/IF
  across the 152→153→0 boundary with LYC=0 (asserts e.g. -GS a=153,b=$15,c=$13,d=$54,e=$C6,h=$40,l=$C2). Touches the
  intricate `LINE_153_LYC0_*`/`CGB_LINE_153_*` compare windows + `vblank_wrap_line0_stat_readback_delay` together — NOT
  a clean E1-style single-window defer; CGB-sensitive. Lowest priority.

- **`boot_hwio-dmg0` (mooneye)** — boot HWIO at the DMG0 handoff; reorder-caused (passed on main, red since the reorder)
  but mechanism is the boot/handoff phase, likely independent of the STAT clusters. Investigate separately.

**REFUTED (do NOT retry):** (1) vblank/mode1 internal-queue 1-dot defer — breaks dispatch-based `vblank_stat_intr`;
(2) mode0-boundary published-stat earlier-publish — cascades to 21 lib fails. Both reverted. Also still dead-ends from
§24.11: enable-delay 5→6 (breaks read probes), chasing scx1 halt_wake with another +1 (cascades the counter).

**NEXT:** the vblank-entry cluster is the cleanest target IF the IF-read-vs-dispatch skew is solved as a small,
contained mechanism (the read of FF0F for PPU-sourced bits lags dispatch by one read-position under CPU-first). Then the
base `intr_2_*` mode0-edge readback. Gate unchanged: fmt-check + lint + tests + rom-report blargg + mooneye 113 +
wilbertpol 117 + m3 no-regress; THEN rewrite the (now ~9) `*_hidden_from_same_cycle_cpu_if*` + the §24.10 lcd_restart
unit tests to the post-reorder behaviour, regen the ~17 trace fixtures, and DELETE `ppu_oracle_sweep.rs` (+ its `mod`
line in `tests/ppu.rs`, commit `685893d8`).

### 24.13 DIRECTION CHANGE — close the remaining STAT/LYC clusters CANONICALLY (delete the seam), not by re-deriving constants (user decision, 2026-06-16)

**Decision:** the user reviewed batch 3 and chose to stop patching the observation-table seam constants and instead
**replace it with the canonical DocBoy model.** Rationale: the remaining L2-a.1 failures ARE the known seams
(`ly_lyc_0`/line-153 = the *observation-tables* seam; `intr_2_*` mode0 / `scx_nops` = the *mode0-publish* seam; vblank =
the IF-visibility variant). §24.8's "re-derive the pretrigger / line-153 LYC-compare constants" plan = patching the
seam; it entrenches it and pins more tests to it (proven by the refuted 21-test cascade in §24.12). **Re-greening must
not rebuild what the canonical refactor exists to delete.** The scheduler reorder (L2-a) and E1 STAY — both are
canonical-aligned (E1 actually removed a dot0 special case and matches DocBoy's delayed-LY effect); E1's branch is later
subsumed by the replacement.

**The canonical model (DocBoy `src/docboy/docboy/ppu/ppu.cpp`, verified):**
- `last_ly` = LY delayed by 1 T-cycle; `last_lyc` = LYC delayed by 1 T-cycle (ppu.cpp:554-560,366). The coincidence is
  `is_lyc_eq_ly() = (last_lyc == last_ly) && enable_lyc_eq_ly_irq` (ppu.cpp:651-667). The 1-T-cycle register delay is
  what makes the coincidence/IRQ land one dot after the LY increment — i.e. E1's effect falls out for FREE, with no
  `regular_line_dot0_compare_window`.
- `enable_lyc_eq_ly_irq` is a SINGLE flag toggled at the few edge dots instead of gb-cycle's 11 window constants: on DMG
  LYC_EQ_LY is forced 0 at dot 454 and at last-scanline (LY 153→0) dots ~2:6; on CGB the flag RETAINS its previous state
  over those windows (ppu.cpp:653-667 documents this DMG/CGB divergence — the same split gb-cycle encodes as separate
  `LINE_153_*` vs `CGB_LINE_153_*` constants). Cross-check against SameBoy + Pandocs LYC timing before landing.
- STAT mode readback: DocBoy derives it from the current dot with the delayed registers, not from
  `published_stat` dot-window overrides — that is the separate *mode0-publish* seam (handle with the same philosophy).

**The seam to DELETE (gb-cycle):** `live_ly_for_lyc_compare()` + `lyc_compare_latch` + `regular_line_dot0_compare_window`
(irq.rs:24-101) + the 11 `LINE_153_LYC*`/`CGB_LINE_153_*`/`*_LY_READ_ZERO_DOT`/`CGB_LINE_END_LYC_COMPARE_BLANK_DOTS`
constants (ppu.rs:70-80). Replace with `last_ly`/`last_lyc` (delayed registers, serialized in save-state) + an
`enable_lyc_eq_ly_irq`-style flag. This should close `ly_lyc_0-GS/-C`, `ly_lyc_0_write-GS` and keep the E1-closed ones
green WITHOUT the dot windows. Ground every step with the §24.12 differential method (main worktree + the sweep harness +
the wilbertpol sources) AND DocBoy `build-trace-cgb`.

**Sequencing:** (1) observation-tables seam → DocBoy LYC model (LYC + line-153 clusters); (2) mode0-publish seam → DocBoy
mode-from-delayed-registers (base `intr_2_*` + `scx_nops`); (3) vblank IF-read-vs-dispatch skew (still a design item,
§24.12). The unit tests pinned to the old seam (the ~9 `mode_edges::*` reds + others) get rewritten to the canonical
behaviour as part of each replacement, not before. Keep the reorder; keep E1 until the LYC model lands.
