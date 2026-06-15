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
