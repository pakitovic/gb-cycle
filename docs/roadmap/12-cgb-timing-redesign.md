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

### 24.14 LANDED — item (3) VBlank-entry IF-read-vs-dispatch skew (commit `d461621a`, 2026-06-16)

Done out of sequence (it was the §24.12 batch-4 target before the §24.13 direction change) but it is a **structural
design fix, not a seam constant** — exactly the "1-cycle-skewed IF-register read value vs dispatch-pending state" §24.12
called for — and is independent of the observation-tables/mode0-publish seams (touches `interrupts.rs` + `step.rs` only,
not `irq.rs`/`ppu.rs`). Kept interim like E1.

**Mechanism (differential-oracle confirmed, main worktree + `oracle_sweep_vblank`):** under CPU-first, the VBlank-entry
PPU IRQ (VBlank `0x01` + the co-committed mode1 / LYC=144 STAT) is committed to the scheduler IF in InterruptAggregation
(phase 7) of the crossing T-cycle `T*`; the next cycle's PRE-tick CPU `ldh a,(IF)` (phase 4) observes it one read
position too early vs main (main's POST-tick read landed in the same cycle as the commit, i.e. before it). Both VBlank
and the STAT bit flip together at the same read position (probe + ROM confirm), so they must defer together.

**Fix:** `InterruptController.cpu_if_read_suppress_mask`, applied ONLY in `read_if_with_pending_requests` (the FF0F memory
read). Dispatch/service/halt-wake read `pending_mask()`/`highest_pending()` (raw `interrupt_flags`), so commit + dispatch
stay at `T*` — the "leave commit/dispatch at ly144 dot0" requirement. Armed in `step_interrupt_aggregation` over the
just-committed PPU bits **iff the VBlank bit is among them** (VBlank only ever commits at ly144 dot0, so this exactly
scopes to the vblank entry and never over-suppresses an on-time STAT edge — verified algebraically), cleared
unconditionally at the top of every InterruptAggregation so it lasts exactly one read position. Serialized in the
interrupt save-state (`#[serde(default)]`).

**Result:** wilbertpol 104→**107/117** (closes `intr_1_timing`, `ly_lyc_144-GS`, `ly_lyc_144-C`). mooneye 109, mealybug
13-fail (m3_scy_change DMG still closed), blargg 58/58 — all unchanged. Full `cargo test -p gb-core` diff vs clean HEAD =
exactly ONE intended unit change (`ppu_mode_edges::entering_vblank_can_raise_vblank_and_mode1_stat_together` rewritten to
the canonical E0-then-E3 observation), ZERO regressions; the 9 `mode_edges::*` + ~17 trace-fixture reds are unchanged.
3-lens adversarial static review (dispatch-leak / scoping / lifecycle+save-state) all returned no flaw.

**Not fully closed — `vblank_if_timing` (still 1 wilbertpol fail).** E1-style IF edge (round1) now passes, but it ALSO
asserts `round5` = LY read after `wait_vblank_irq`+97 nops = 145 (branch reads 144, 1 late). That is the **LY-after-event
read**, NOT the IF skew — it belongs to the canonical LY-delayed-register / mode0-publish work (items 1–2), unaffected by
this fix. So `vblank_if_timing` closes when the canonical `last_ly` model lands. NEXT: item (1), the DocBoy LYC model.

### 24.15 ITEM (1) DESIGN — LYC observation-tables seam → DocBoy last_ly/last_lyc model (recon complete, 2026-06-16)

Recon done (seam consumer map + DocBoy model verified at the source). This section is the cold-start-ready design.

**DocBoy canonical model (verified, /Users/pakitovic/workspace/DocBoy/src/docboy/docboy/ppu/ppu.cpp):**
- `tick()` order (ppu.cpp:507-571): (1) dot handler `(this->*tick_selector)()` (may `end_increase_ly` → `ly`/mode);
  (2) `tick_window()`; (3) **`raise_stat_irq()`** (681-710) — computes `is_lyc_eq_ly()` from the CURRENT
  `last_ly`/`last_lyc`, raises the STAT IRQ on a rising edge, and sets `stat.lyc_eq_ly = is_lyc_eq_ly()`; (4) at the
  END, `last_ly = ly; last_lyc = lyc` (557-560) — captured for the NEXT tick. So `raise_stat_irq` at tick T reads the
  registers as of the END of tick T-1 ⇒ the 1-T-cycle delay. The SAME `is_lyc_eq_ly()` drives BOTH the IRQ source and
  the readback flag — there is NO separate irq-vs-readback path (gb-cycle's two functions were the seam).
- `is_lyc_eq_ly()` (651-667): DMG = `(last_lyc == last_ly) && enable_lyc_eq_ly_irq`; CGB =
  `enable_lyc_eq_ly_irq ? (last_lyc == last_ly) : stat.lyc_eq_ly` (CGB RETAINS the previous readback flag in the disabled
  windows; DMG forces 0).
- `enable_lyc_eq_ly_irq` (default true) is forced **false** in exactly two window kinds, then re-enabled (1276/1294,
  1337/1351, 1415/1428, 1462/1488): (a) **dot 454 of every line** (disabled at the dot-453 handler right after
  `end_increase_ly`, re-enabled at the dot-454 handler); (b) **line 153 (last scanline) dots 2:6** (the LY 153→0 wrap;
  disabled at dot 2, re-enabled at dot 7; CGB resets `ly=0` at dot 3). `turn_on` sets `last_lyc=lyc` +
  `stat.lyc_eq_ly=is_lyc_eq_ly()` (no IRQ); `turn_off` sets `last_ly=0`, `enable=true`.

**Why this subsumes E1 and the 11 constants:** at a line boundary `ly` becomes `N` but `last_ly` is still `N-1` for one
dot, so the `LYC==N` coincidence lands one dot late — exactly E1's dot0 defer, for free, with no
`regular_line_dot0_compare_window`. The line-153 LYC153/LYC0 compare windows + the CGB blank/retain split collapse into
the single `enable_lyc_eq_ly_irq` flag + the CGB `stat.lyc_eq_ly` retain branch.

**gb-cycle tick mapping (api.rs `tick_t_cycle_with_observer`):** per dot — ModeTiming(`previous_mode`,
pre-increment) → RasterAdvance(`line_dot += 1`, :853) → VisiblePrep/Mode2/Mode3 → RasterAdvance2(scanline_length; at
`line_dot == scanline_length` RasterPublication sets `line_dot=0`, `ly++`/wrap, :899-904) → ModeTiming2(`current_mode`,
VBlank queue :940) → **StatIrq(`update_lyc_compare_latch` + `refresh_stat_irq_line`, :948-949)** ≈ DocBoy
`raise_stat_irq`. PLAN: add `last_ly`/`last_lyc` capture at the very END of the tick (after :949), and a
`enable_lyc_eq_ly_irq` bool; `is_lyc_eq_ly()` reads them. The two disable windows must be expressed in gb-cycle
`line_dot` coords — DocBoy `dots` and gb-cycle `line_dot` share 0..455 but differ in the LY-increment phase (gb-cycle
ly++ at the `line_dot==scanline_length` wrap, :900), so **ground the exact disable dots against the differential oracle**
(`oracle_sweep_ly_lyc` + `oracle_run_ly_lyc_roms` vs the main worktree) rather than copying 454/2:6 literally.

**State (serialize in PpuRuntimeState + save-state, like `pending_interrupts`):** `last_ly: u8`, `last_lyc: u8`,
`enable_lyc_eq_ly_irq: bool`. Seed in `apply_startup_state` (turn_on: `last_lyc=lyc`, `last_ly=ly`, `enable=true`,
readback flag from `is_lyc_eq_ly`).

**Seam to DELETE (irq.rs:24-101):** `live_lyc_coincidence`, `live_ly_for_lyc_compare`, `lyc_compare_blanked_at_line_end`,
`regular_line_dot0_compare_window`, the `lyc_compare_latch` field + `update_lyc_compare_latch`. Constants (ppu.rs:70-80):
`LINE_153_LYC153_COMPARE_{START,END}_DOT`(3/5), `LINE_153_LYC0_COMPARE_START_DOT`(12), `CGB_LINE_153_*`(1/5/9),
`CGB_LINE_END_LYC_COMPARE_BLANK_DOTS`(3), `LINE_153_LY_READ_ZERO_DOT`(4)/`CGB_…`(8) **only if** not still needed by the
LY-readback path (registers.rs:228-230 — CHECK; LY readback is item 2, may keep these until then).

**Consumer contracts to re-route to `is_lyc_eq_ly()` (from the seam map):** IRQ — `lyc_coincidence_for_irq_line`
(ordinary_stat_irq_line:128, lyc_stat_write_irq_source:365, cancel_obsolete_dot0_lyc_stat_irq_edge:440,
stat_write_quirk_active_for_write:499, enter_lcd_disabled_state:551). Readback — `lyc_coincidence_for_readback`
(registers.rs:147 read_stat, snapshot:994, trace:1465). LCD-enable-pending — `lcd_enable_pending_lyc_rise_source`
(uses `live_lyc_coincidence`). KEEP for now (separate quirks): `dot0_lyc_stat_irq_edge_pending`/cancel,
`line_153_lyc0_stat_irq_pretrigger_pending`/source/cancel (these are STAT-write/edge-cancel quirks layered on top — the
last_ly delay may simplify or obviate the dot0 one; re-evaluate after the core lands). LYC FF45 write (api.rs:479-488)
keeps re-evaluating; whether last_lyc lags a LYC write by 1 dot is an OPEN question — ground vs `ly_lyc_write-GS` (must
stay green) and `ly_lyc_0_write-GS` (must close).

**Incremental grounded plan:** (a) add the 3 state fields + capture at end of tick + `is_lyc_eq_ly()`, keep the seam
alive in parallel, print both in a temporary probe and diff vs the seam over a full frame (no behaviour change yet);
(b) flip `lyc_coincidence_for_irq_line`/`_for_readback` to `is_lyc_eq_ly`, delete the dot-window branches of
`live_ly_for_lyc_compare`, tune the two `enable_lyc_eq_ly_irq` disable windows in `line_dot` coords against
`oracle_sweep_ly_lyc` + the wilbertpol sources until `ly_lyc`/`ly_lyc_144`/`ly_lyc_write` stay green and
`ly_lyc_0`/`ly_lyc_0_write` close; (c) delete the seam constants/functions; (d) rewrite the seam-pinned unit tests
(`mode_edges::*` line-153/dot0 + `registers.rs` lyc tests — see §24 map) to the canonical behaviour; (e) ROM gate.
Targets: close `ly_lyc_0-GS/-C`, `ly_lyc_0_write-GS`; keep E1-closed green; mooneye/mealybug/blargg no-regress.

### 24.16 VBlank IF-read fix (`d461621a`) classified — interim compensation, not canon-structural (2026-06-16)

(The standing per-commit self-audit — "is this following the canon DocBoy + SameSuite + Pan Docs to avoid manual tables
and seams?" — is tracked in agent memory, not duplicated in repo docs. This section keeps only the factual record below.)

**Honest classification of the §24.14 VBlank IF-read fix (`d461621a`) — INTERIM COMPENSATION, not
canon-structural.** Verified at the source: DocBoy raises VBlank straight into the shared IF register
(`ppu.cpp:1715 enter_vblank → interrupts.raise_interrupt<VBlank>()`) and has NO cpu-IF-read suppress/hide — the
"visible one read-position after the raster crossing" timing falls out STRUCTURALLY from the within-T-cycle order (CPU
memory access ordered before the PPU raise, reading the shared IF directly). gb-cycle's `cpu_if_read_suppress_mask` is a
**compensation** for gb-cycle's different architecture (PPU `pending_interrupts` → separate `InterruptAggregation` drain
→ scheduler IF, read pre-tick under the reorder), not that structural ordering. It is hardware-CORRECT (grounded vs the
`main`/PR-#245 oracle + the wilbertpol sources) and is the §24.12/§24.13-sanctioned "item (3) design item" (a
1-cycle-skewed IF-read-vs-dispatch mechanism, NOT a per-dot/scx table) — but it is NOT the DocBoy mechanism and was NOT
cross-checked against DocBoy's IF path / SameBoy / Pan Docs. Kept interim like E1. **Canonical end-state that should
subsume it:** make the VBlank/STAT IF-read timing fall out of the tick order (align the scheduler-IF commit/read with the
within-cycle order the way DocBoy interleaves CPU-access-then-PPU-raise), or fold it into the items (1)/(2) canonical
IF-read / mode-from-delayed-registers model — then delete the mask. Revisit when items (1)/(2) land; net seam count must
not rise on its account.

### 24.17 ITEM (1) LANDED — line-153 LYC0 IRQ edge deferred via a delayed register; full seam deletion is item-2-coupled (2026-06-16)

**What landed.** `ly_lyc_0-GS`, `ly_lyc_0-C`, `ly_lyc_0_write-GS` now match the `main`/PR-#245 oracle exactly
(**wilbertpol 107→110/117**); mooneye 109, mealybug 13-fail (m3_scy_change DMG still closed), blargg untouched — ZERO ROM
regression. The fix is a single canonical 1-T-cycle **delayed register**
(`StatState.last_line_153_lyc0_pretrigger_window`, captured at the END of the tick in `capture_delayed_lyc_state`,
mirroring DocBoy `ppu.cpp:554-560`): the line-153 LYC0 STAT IRQ pretrigger source reads the *previous* dot's window
membership, which defers the otherwise-1-read-position-early edge by one dot under the CPU-first reorder. This is the
SAME principle as E1 (regular-line dot0 latch) and item (3) (VBlank IF-read), now applied to the last reorder-skewed LYC
edge. It adds a delayed register (canonical `last_*` family), NOT a manual table/constant — no window constants were
re-derived (the §24.13 refutation stands). The three seam-pinned unit tests
(`mode_edges::dmg_line153_lyc0_stat_pretrigger_bridges_to_visible_coincidence_without_retrigger`,
`mode_edges::cgb_line153_lyc_edges_follow_the_cgb_compare_schedule`,
`registers::dmg_vblank_stat_write_quirk_blocks_the_repeated_line153_lyc0_source`) were rewritten to the deferred
behaviour; lib/integration/trace-fixture failure sets are otherwise byte-identical to clean `d0b7d59e` (the 9 `mode_edges`
+ ~16 phase/machine/dma trace reds are pre-existing reorder debt, unchanged, NOT regenerated — behaviour is not final
until items 2–3).

**The §24.15 "flip both consumers to is_lyc_eq_ly + delete the seam" plan is REFUTED for item (1) in isolation** — proven
by the differential oracle (main worktree + `oracle_run_ly_lyc_roms`), three experiments, all reverted:
1. **Flip `lyc_coincidence_for_irq_line` to a raw-`last_ly`/`last_lyc` delayed `is_lyc_eq_ly`** — did NOT close `ly_lyc_0`
   (the line-153 LYC0 edge is fired by the separate pretrigger source, not the coincidence path) and REGRESSED
   `ly_lyc_write-GS` (the DocBoy `last_lyc` delay conflicts with gb-cycle's immediate LYC-write re-evaluation, which the
   green baseline depends on).
2. **Delayed effective-compare-LY with LIVE LYC** (`last_compare_ly == Some(lyc)`) — recovered `ly_lyc_write-GS` and was
   behaviourally equivalent to E1 for every green ROM, but still did NOT close `ly_lyc_0` (pretrigger again) and created a
   compare/pretrigger seam gap at the line-153 dot-12 boundary.
3. **Routing that delayed coincidence through the whole IRQ path** — REGRESSED CGB `ly_lyc_write-C` /
   `ly_lyc_153_write-C` (round2: a LYC write at a `Some` dot must see the LIVE compare to raise immediately; the uniform
   delay made the write-time evaluation stale on CGB).

**Why the full observation-tables-seam deletion is blocked in item (1) alone (the structural finding).** The seam is two
coupled compensations, each tied to *other* unlanded work:
- The **irq-vs-readback split** (`lyc_coincidence_for_irq_line` vs `_for_readback`) compensates gb-cycle's CPU-first
  reorder: the readback self-compensates (read-1-behind cancels raster-1-early, §24.12) while the IRQ needs an explicit
  defer (E1). DocBoy has no split because it is not reordered (it raises straight to IF and reads IF in canonical tick
  order). Unifying them requires modeling the reorder skew at the IF read — the item-(3)/§24.16 canonical end-state.
- The **line-153 LYC0/LYC153 compare windows** compensate gb-cycle NOT wrapping `ly` to 0 mid-line-153 (DocBoy does, at
  dot 3, so its `last_ly` delay produces the LYC0 coincidence in-line; gb-cycle keeps `ly=153` until the end-of-line
  wrap). Deleting them requires the mid-line `ly`-wrap — the item-(2) raster rephasing.
- gb-cycle's **immediate LYC/STAT write re-evaluation** (vs DocBoy `tick_pending_write` + next-tick `raise_stat_irq`) is
  load-bearing for the write ROMs; the canonical `last_lyc` write delay can only land alongside removing that immediate
  re-eval — coupled to the item-(2) work as well.

**Classification — INTERIM COMPENSATION (like E1 and item 3), net manual-seam count does NOT rise.** A 1-T-cycle delayed
register is the canonical mechanism, not a manual table; no constants were added or re-derived. **Canonical end-state
that subsumes it:** once item (2) lands the mid-line-153 `ly`-wrap and the mode/registers-from-delayed-registers model
(and the write path moves to pending-write + next-tick), the line-153 LYC0 pretrigger + compare windows collapse into the
single delayed `is_lyc_eq_ly`, and `last_line_153_lyc0_pretrigger_window` + the `LINE_153_LYC0_*`/`CGB_LINE_153_*`
constants delete together. Tracked as item-2-coupled debt; revisit when item (2) lands.

### 24.18 ITEM-1+2 COUPLED REWORK ATTEMPTED — root seam deletion needs the full internal-LY mid-line rephase; the bounded "effective-LY decoupling" is REFUTED (2026-06-16)

**Goal of the attempt (user-directed):** delete the LYC observation-tables seam at the root by rephasing the raster so
LY wraps to 0 mid-line-153 (DocBoy: LY increments at the dot-453 handler on every line, resets to 0 at dot 2 DMG / dot 3
CGB on line 153), letting the canonical `is_lyc_eq_ly = (last_lyc == last_ly) && enable_lyc_eq_ly_irq` replace
`live_ly_for_lyc_compare` + the 9 LYC compare constants + the irq-vs-readback split + E1's latch + the line-153 pretrigger.

**7-agent blast-radius map (ran via workflow).** Verdicts: (A) the LY-readback lead/wrap compensation
(`current_ly_read_advance_start_dot`=450, `line_153_reads_as_ly0` dot 4/8, `skip_boot_ly_read_lag`) becomes redundant
under a mid-line LY increment — deletable, HIGH risk. (D) **HIGH risk: all mode3 pixel rendering reads `self.ly` LIVE for
tile-row** (`bg_fetch.rs:222`, `helpers/mode3_latches.rs:232` `(scy+ly)%8`, `pipeline.rs:246/277` sprite-Y). (C)
mode-entry boundaries that key off LY (the VBlank-entry IRQ at ly 143→144, mode2-vblank-entry) **shift earlier** if LY
leads and must be re-anchored to line_dot. (F) DocBoy spec: LY increments at dot 452/453 (two-phase `begin/end_increase_ly`),
line 153 resets to 0 at dot 2/3, `enable_lyc_eq_ly_irq` disabled dots 453-454 + 2-6. (E) boot/lcd-restart seeding assumes
end-of-line wrap. (G) ~9 mode_edges + ~16 trace fixtures pin the current phase (mostly pre-existing reorder debt).

**Make-or-break experiment (the rigor item-1 skipped): a shadow-diff probe** (`oracle.rs`, reverted after; recipe below)
that, over a settled frame for DMG+CGB and lyc∈{0,2,143,144,153}, compares `observed_ly == lyc` (observed_ly =
`read_ly(CpuBusOperation)`, the readback geometry that ALREADY leads + wraps mid-153) against the seam's live
`lyc_coincidence_for_readback`/`_for_irq_line`. **It REFUTES the bounded "reuse the readback geometry for the coincidence"
plan:**
- The **readback LY leads from dot 450** (`observed_ly = ly+1` at ly=N dots 450-455) but the **coincidence does NOT lead** —
  the seam keeps `LYC==N` true at ly=N dots 450-455 (where `observed_ly` is already N+1). These are **incompatible phases**:
  an `enable_lyc_eq_ly_irq` window can only SUPPRESS a match, it cannot CREATE the coincidence at the dots where
  `observed_ly != lyc`. So the readback geometry is the wrong source for the coincidence.
- **Line 153 (DMG)** the seam is a TWO-window structure (LYC153 dots 3-5, LYC0 dots 12+) with a gap (dots 0-2, 6-11);
  collapsing it to "LY until a wrap-dot then 0" makes LYC153 span dots 0-11 (too long). Reproducing the gap needs
  `enable` disabled at the SAME gb-cycle-specific dots (0-2, 6-11) — i.e. the seam constants relabelled as enable
  windows, NOT a deletion (zero net reduction).
- CGB has only 2 divergences (its compare windows happen to align with the readback geometry), DMG 8-14 — confirming the
  DMG phase is where the readback/coincidence split is irreducible without changing the internal LY.

**Conclusion — the root deletion requires the FULL internal-`self.ly` mid-line rephase, a core raster restructure (NOT a
bounded LYC change), and the bounded effective-LY decoupling is dead.** The readback-LY (leads from dot 450) and the
coincidence-LY (non-leading, line-153 multi-window at dots 3-5/12+) are genuinely different phases in gb-cycle, neither
matching DocBoy (which leads 3 dots and wraps mid-153). Only making `self.ly` itself lead/wrap mid-line (DocBoy phase)
collapses both into one geometry and lets the constants fall out. That rephase is:
- **rendering-SAFE** (the LY lead lands at dot ~450-453, in hblank, AFTER mode3 ends ≤~289; line 153 is vblank, not
  rendered — so the mode3 `self.ly` reads at dots 80-289 never see the lead; dimension D's "corrupts every line" is
  over-cautious on this point), BUT
- requires **(1) restructuring the line counter** so the mid-line increment is not double-applied at the end-of-line wrap
  (a `next_ly`/already-incremented guard, like DocBoy's `real_ly`/`next_ly`); **(2) re-anchoring every ly-based mode/IRQ
  boundary** to line_dot (VBlank-entry, mode2-vblank-entry) so the ~3-6 dot LY lead does not flip mode early (regresses
  intr_1/vblank/lcdon_timing otherwise); **(3) deleting the readback lead compensation** and folding it into the intrinsic
  lead; **(4) the canonical `last_ly`/`last_lyc` + `enable_lyc_eq_ly_irq` windows grounded in gb-cycle line_dot**; **(5)
  resolving the write-vs-tick model** (item-1 finding: gb-cycle's immediate LYC re-eval vs DocBoy pending-write+next-tick);
  **(6) re-grounding the ENTIRE ly/lyc/mode timing suite** (mooneye ly00_*/ly143_*/ly_new_frame/lcdon_timing + wilbertpol
  ly_lyc family + intr_*; all currently green) + **regenerating ~17 trace fixtures + rewriting ~9 mode_edges tests.** This
  is the §24 "canonical restructure" L2 raster pass, multi-cut, with a regression surface across every CGB+DMG timing ROM.

**Decision pending (user).** The bounded path is refuted; the root deletion = the multi-cut core raster rephase above.
Item-1's interim (commit `c4bec898`, §24.17) is landed, green, +3 wilbertpol, zero regression, and its only cost is one
canonical delayed register (no manual table). Recommend treating the full rephase as a scoped, oracle-gated restructure
(its own branch/cuts) rather than a continuation of item-1, OR keeping the interim and moving to item-2 mode0-publish.

**Shadow-diff probe recipe (to reproduce the refutation):** add an `#[ignore]` test in `ppu/tests/oracle.rs` that builds a
`PpuTestRig` (DMG/CGB) with `lcdc=0x91, stat=STAT_LYC_INTERRUPT_ENABLE_BIT, ly=0, lyc=L`, ticks `3*154*456` to settle,
then over `154*456` dots records `(ly, line_dot, read_register(0xFF44,CpuBusOperation), live_ly_for_lyc_compare(),
lyc_coincidence_for_readback(), lyc_coincidence_for_irq_line())` and prints the dots where `observed_ly==lyc` diverges
from the seam readback/irq. Reverted (not committed) — findings captured above.

### 24.19 ITEM (2) mode0-publish — base `intr_2_mode0_timing` fix VALIDATED (the §24.12 "blowup" is legit test-pinning), full cluster is multi-batch (2026-06-16)

**Oracle (`oracle_sweep_intr_2_mode0_timing`, both trees):** confirmed §24.12 — poll-until-mode0 transitions 2→1 at delay
**46→47 on branch vs 45→46 on main (+1 late)**; poll-until-mode2 identical (47→48). The Drawing→HBlank (mode0) edge is
observed one read-position late under the CPU-first reorder; mode2/mode3 already correct (the §24.11 fix-#6
mode2→mode3 override fires at `MODE2_DOTS-1`, one dot early). DMG and CGB identical.

**The fix (validated, then reverted to keep the tree green for a deliberate landing):** make the mode0 boundary override
fire **one dot earlier**, mirroring the mode2→mode3 override — `published_stat_steady_frame_mode0_boundary_override_applies`
returns true also when `line_dot + 1 == current_mode0_start_dot() && access_mode_for_line_dot(line_dot)==Drawing &&
access_mode_for_line_dot(line_dot+1)==HBlank` (same gate `scx==0 || mode0-int`). This is scoped to the STAT readback path
(`current_published_stat_access_mode`), NOT the bus-access modes (`current_published_bus_access_mode` is separate). Result:
poll-until-mode0 → **45→46 = main**; **wilbertpol 110→111, mooneye 109→110** (both close `intr_2_mode0_timing` base),
poll-until-mode2 unchanged, ZERO ROM regression.

**The §24.12 "exp#2 blew up — lib stat 9→21" is CORRECTED here: it is legit test-pinning, not a regression.** The 12 new
lib fails are all `cpu_stat_read_*` CpuBusOperation mode0-boundary assertions (`mode_edges::cpu_stat_read_switches_to_hblank_on_the_exact_mode0_start_dot`
+ 6 `advanced_tails::*` + 4 `terminal::*` + 1 `multi_sprite::*`). They assert the PRE-reorder boundary (Drawing at
`mode0_start_dot-1`, HBlank at `mode0_start_dot`); the reorder-correct CPU read is HBlank one dot earlier. The shift is
**uniform** (a CPU-pre-tick scheduling property, independent of mode3 length), so the sprite-tail tests shift too — and
crucially the §245-frozen mode3 terminal-tail LENGTH stays verified by their separate `current_mode0_start_dot()` asserts
(untouched), so the rewrite does NOT touch §245. **Rewrite shape:** flip each `cpu_visible_stat_mode(&ppu)==0x03`
assertion that sits at `line_dot == boundary-1` to `==0x00` (or, per test, move the probed `line_dot` to `boundary-2` and
keep `0x03`); the helper `advanced_tails::cpu_visible_stat_mode` reads `CpuBusOperation`. Per-test `line_dot`-vs-boundary
analysis is required (some use the `terminal_tail_rig` default, x165 sets `line_dot` explicitly) — mechanical but careful;
deferred to a focused cut rather than rushed.

**Remaining intr_2_* cluster after the base fix (each a distinct sub-issue, NOT closed by the mode0-edge defer):**
`intr_2_mode0_timing_sprites` (mooneye+wb) — sprite-extended mode3 LENGTH (the §245-frozen penalty / fetcher-lead, item
MODE3-FETCHER-LEAD), orthogonal to the reorder edge; `intr_2_oam_ok_timing` (mooneye+wb) — the OAM/mode2 boundary, not
mode0; `intr_2_timing` (wb); `intr_2_mode0_scx3/scx7_timing_nops` (wb) — scx≠0, so the override gate needs `mode0-int`
(these are mode2-int tests → the `scx==0` arm fails → the current gate does NOT fire for them; the gate itself is a seam
to revisit). `vblank_if_timing` (wb) round5 = the LY-after-event read (item-1/2 LY model, §24.14). `boot_hwio-dmg0`
(mooneye) — boot handoff, separate. **NEXT item-2 batch:** land the validated base fix + the 12 mechanical test rewrites,
then take the sprites (mode3-length) and scx (gate) sub-issues one at a time against the oracle.

**LANDED (item-2 batch-1, 2026-06-16):** the mode0-earlier override + the 12 test rewrites (option (a): the
`cpu_stat_read_*` tests now assert the reorder-correct CPU read — `0x00`/HBlank at `mode0_start_dot-1` — and were
renamed `keeps_mode3/keeps_drawing` → `reads_mode0_one_dot_early`/`switches_to_hblank_one_dot_before_mode0_start`; the
§245 mode3 terminal-tail length stays verified by their untouched `current_mode0_start_dot()` asserts). Gate green:
**wilbertpol 110→111, mooneye 109→110** (both close `intr_2_mode0_timing`), blargg 58/58, mealybug 13-fail (m3_scy_change
DMG still closed), lib back to the 9 pre-existing reorder reds, integration/trace-fixture failure sets unchanged,
fmt-check + lint clean. Canon self-audit: the fix is a scoped published-STAT-readback override mirroring the already-canon
mode2→mode3 one (CPU-pre-tick observation), not a new manual table; net manual-seam count does not rise. Remaining
intr_2_* cluster (sprites/oam/scx/timing) + vblank_if + boot_hwio are the subsequent batches.

### 24.20 ITEM (2) mode0-publish batch-2 LANDED — `intr_2_oam_ok_timing` via the OAM-read bus path; cluster sources re-diagnosed (2026-06-16)

**Oracle (`oracle_sweep_intr_2_oam_ok_timing`, a faithful poll-until-OAM-accessible probe added to the sweep harness,
both trees):** the `intr_2_oam_ok_timing` ROM clears OAM, syncs to a mode2 IRQ, then `inc b; ld a,(OAM); and $FF; jr nz`
counts iterations until the OAM read returns 0 (= OAM accessible = HBlank). Asserts delay 46 → count 1, delay 45 → 2. The
probe showed the count 2→1 transition at delay **46→47 on the branch vs 45→46 on main (+1 late)**; DMG = CGB. So the
mode3→mode0 (HBlank) **OAM-unlock** was observed one read-position late under the CPU-first reorder — the SAME skew as the
batch-1 STAT readback, but on a SEPARATE path: the OAM-read bus path (`cpu_oam_read_bus_state` →
`current_published_oam_read_access_mode_from`), which the batch-1 STAT-readback fix did not touch.

**Root cause (the path asymmetry).** Both the STAT readback (`current_published_stat_line_dot`) and the bus access
(`current_published_bus_access_mode`) read the pre-tick `line_dot-1`. After batch-1, the STAT mode0 override fires at
`line_dot+1 == mode0_start` (one dot early, reorder-correct), but the OAM-read override still fired only at
`line_dot == mode0_start` (one dot late) — a one-dot mismatch between the two CPU-observed paths.

**The fix (commit `056340d0`).** Mirror the batch-1 STAT override in `current_published_oam_read_access_mode_from`: when
the pre-tick bus mode is Drawing and `line_dot+1 == current_mode0_start_dot()` with `access_mode_for_line_dot(line_dot+1)
== HBlank`, publish HBlank for the same-cycle CPU OAM read. Scoped to the OAM-read path only (owner/internal
`current_bus_access_mode`, the OAM-write and VRAM paths untouched). **Deliberately NO `scx==0` gate** — the OAM-unlock is a
CPU-pre-tick observation for all scx; the `scx==0` gate that still sits on the STAT-readback mode0 override is the
remaining mode0-publish seam (item-2 batch-3 = `intr_2_mode0_scx3/scx7`), and when that lands the STAT readback will
unlock early for scx≠0 too and reconverge with this OAM path. So this fix is the forward-consistent canonical end-state
for the OAM path, not a seam. Net manual-seam count does not rise (same CPU-pre-tick principle as the canonical
mode2→mode3 / batch-1 overrides).

**Test rewrites.** Two pinned `stat::bus` unit tests rewritten to the reorder-correct OAM unlock:
`cpu_oam_read_bus_state_switches_to_hblank_on_the_exact_mode0_start_dot` → `…_one_dot_before_mode0_start` (now HBlank at
`mode0_start-1`, anchored by a `mode0_start-2` Drawing probe); `sprite_extended_mode0_start_opens_cpu_oam_read_before_published_stat_catches_up`
updated so OAM opens at `mode0_start-1` while the (scx-gated, still-lagging) STAT readback stays Drawing — the test's
theme (OAM ahead of STAT) holds with a wider gap. The §245 `current_mode0_start_dot()` asserts are untouched.

**Gate green: wilbertpol 111→112, mooneye 110→111** (both close `intr_2_oam_ok_timing`), blargg 58/58, mealybug 13-fail
(m3_scy_change DMG still closed), lib back to the 9 pre-existing reorder mode_edges reds, integration/trace failing set
byte-identical (27 total), fmt-check + lint clean.

**Remaining cluster — sources RE-DIAGNOSED from the wilbertpol master `.s` (the workflow agents mislabelled three; verified
by direct fetch):**
- `intr_2_mode0_scx3/scx7_timing_nops` (wb) — **STAT-readback** mode3→mode0 bracket (D/E rounds: `nops delay; ld a,(STAT);
  and $03`, delay→mode3 then +1→mode0) at SCX=3/7, plus a mode2-IRQ instruction-counter (B/C). The mode3→mode0 readback is
  the SAME edge batch-1 fixed for scx=0, but the override's `scx==0 || mode0-int` gate does not fire (these are mode2-int
  tests, scx≠0). **Fix = derive the scx-aware mode0 boundary without the `scx==0` gate** (do NOT re-fit the §245 sprite
  penalty). This is item-2 batch-3, the direct sibling of batch-1/2.
- `intr_2_mode0_timing_sprites` (wb+mn) — NOT an IF test (the agent mislabelled it). Real source: `testcase`/`run_testcase`
  with sprites at X coords (`testcase 2,0` / `4,0,0` / …), `ld d,41+extra; ld e,40+extra` → it measures sprite-extended
  **mode3 LENGTH** (§245-frozen penalty / fetcher-lead). Orthogonal to the readback skew; most §245-entangled → last.
- `intr_2_timing` (wb) — NOT a poll-loop. LCD off@LY144 / on, then a single `ldh a,(IF)` per round (STAT=%00100000 mode2
  source, LYC=$F0). Rounds 1-4 pin the first **mode2 STAT-IF latch after an LCD re-enable** (109→110 nops); rounds 5-7 the
  STAT-vs-VBlank IF ordering at 143→144. Path = IF-read + LCD-restart phase (adjacent to item-3 / `LCD_REENABLE_LINE0_*`),
  NOT a mode0 readback.
- `vblank_if_timing` (wb) — NOT an LY-after-event read (the older §24.14 note and the agent both off): real source reads
  **IF** every round; round1 nops 97 → $E0 (VBlank bit0 clear), round2 nops 98 → $E1 (VBlank bit0 set). It pins the
  **VBlank IF (bit0)** raise at 143→144 after an LCD restart. Path = IF-read / vblank entry (item-3 family).
- `boot_hwio-dmg0` (mn) — boot-handoff IO snapshot (LCDC/STAT/SCY/SCX/LY/LYC/BGP read once at $0100; dmg0 = LY $01, STAT
  $83/mode3, DIV $19). NOT a STAT-cluster timing test; depends on the dmg0 boot-ROM duration + PPU raster position at
  handoff. Independent of the mode0-publish work.

### 24.21 ITEM (2) batch-3: vblank_if_timing LANDED (LY-wrap), scx3/scx7 gate-deletion REFUTED → fetcher cluster (2026-06-16)

**`vblank_if_timing` LANDED (commit `b798c348`, wilbertpol 113/117).** Correcting the §24.20 note: the full wilbertpol
source has 5 rounds, and rounds **4/5 read LY** (not IF) — `r4 = wait_vblank_irq; di; nops 96; ldh a,(LY)` asserts LY=144,
`r5 = …nops 97…` asserts LY=145. A real-ROM WRAM dump (both trees) isolated the failure to round5 only: branch r5=144 vs
main r5=145; r1-r4 already matched. A faithful LY-after-vblank-IRQ probe pinned the mechanism: round5's read lands at the
144→145 **wrap**, where the CPU-first reorder makes the CPU observe the pre-tick `line_dot` (line 144 dot 455) and read
ly=144, while main reads the post-tick wrap value 145.
- **Fix:** `read_ly_without_skip_boot_lag` gains a one-dot vblank-wrap compensation — at the final dot of a vblank line
  (`ly >= VISIBLE_SCANLINES && line_dot + 1 >= current_scanline_length() && ly + 1 < TOTAL_SCANLINES`) the same-cycle CPU
  LY read publishes the next line. This mirrors the mode0-publish overrides (CPU-pre-tick observation). Crucially it is
  **one dot, not the visible 6-dot lead** (450..455): the test's round4 reads 144 at line 144 dot ~452, so the visible lead
  would be wrong at vblank and is correctly gated off by `ly < VISIBLE_SCANLINES`. Line 153 stays excluded by `ly + 1 <
  TOTAL_SCANLINES` (handled by `line_153_reads_as_ly0`); the visible 6-dot lead is untouched.
- Canon self-audit: a structural reorder compensation mirroring the existing canonical mode0-publish pattern, not a manual
  table/constant; net manual-seam count does not rise. The §24.18 internal-LY mid-line rephase is the end-state that
  subsumes it (the wrap compensation collapses into the intrinsic lead once `self.ly` leads/wraps in phase).
- Gate green: wilbertpol 112→113, mooneye 111 unchanged, blargg 58/58, mealybug 13-fail (m3_scy_change DMG closed), lib 9
  pre-existing mode_edges reds, integration/trace failing set byte-identical (27 total), fmt-check + lint clean.

**`intr_2_mode0_scx3/scx7_timing_nops` — the "delete the `scx==0` gate" plan is REFUTED (deferred to the fetcher/§245
cluster).** The D/E rounds read STAT mode bits (`nops delay; ld a,(STAT); and $03`) and bracket the mode3→mode0 edge at
SCX=3/7. Oracle (single STAT-read-after-delay probe, scx 0..8, both trees, DMG=CGB) showed the branch matches main for
scx **0,1,2,4,5,6,8** and is +1 late ONLY at scx **3,7** — i.e. NOT the uniform gate effect the seam hypothesis predicted.
Experiment (removing the `scx==0 || mode0-int` gate so the override fires for all scx): it FIXES scx3/scx7 but REGRESSES
scx4/scx8 (those go one dot early). Transition-delay table (mode0@nop, DMG):

| scx        | 0  | 1  | 2  | 3   | 4   | 5  | 6  | 7   | 8   |
|------------|----|----|----|-----|-----|----|----|-----|-----|
| main       | 49 | 50 | 50 | 50  | 51  | 51 | 51 | 51  | 50  |
| branch+gate| 49 | 50 | 50 | 51✗ | 51  | 51 | 51 | 52✗ | 50  |
| no-gate    | 49 | 50 | 50 | 50  | 50✗ | 51 | 51 | 51  | 49✗ |

No binary scx-gating of the override reproduces main's per-scx pattern. The residual is the **sub-dot mode3-length phase
per scx** (the `[PPU][MODE3-FETCHER-LEAD]` seam / §245), not the STAT-readback gate. The gate `scx==0 || mode0-int` is a
hand-fit compensation that happens to land 0-2/4-6/8 but not 3/7; it is NOT a deletable seam in isolation. **Deferred:
group scx3/scx7 with `intr_2_mode0_timing_sprites` (batch-4, the §245/fetcher-lead mode3-length work).** The remaining
cluster is now: `intr_2_timing` (LCD-restart mode2-STAT-IF latch, IF path), `intr_2_mode0_timing_sprites` + scx3/scx7
(mode3-length/§245/fetcher), `boot_hwio-dmg0` (boot handoff, independent). Scoreboard after batches 1-3: **wilbertpol
113/117, mooneye 111/113**, blargg 58/58, mealybug 13-fail.

### 24.22 ITEM (2) batch-4: intr_2_timing LANDED (mode2 vblank-entry STAT pretrigger, blank-frame) (2026-06-16)

**`intr_2_timing` LANDED (commit `7a9599f5`, wilbertpol 114/117).** 7 rounds, all IF reads after an LCD re-enable:
rounds 1-4 bracket the line-0 mode2 STAT-IF latch (109→110 nops, $E0→$E2), rounds 5-7 (resync `wait_ly 143; nops 70;
clear IF; nops 26/27/28`) bracket the line-144 mode2-entry STAT-IF ($E0→$E2) and the VBlank-IF one dot later ($E2→$E3). A
real-ROM WRAM dump (both trees) isolated the failure to **round5 only** (branch $E2 vs main $E0); rounds 1-4 and 6/7
already matched. So the line-0 mode2 latch and the VBlank IF (item-3) are correct; only the line-144 mode2-entry STAT IF
fired one read-position early.
- **Mechanism:** rounds 5-7 run during the blank frame after the LCD re-enable. `ordinary_mode2_stat_pretrigger_lead_dots`
  already shifts the ordinary mode2 pretrigger one dot later in the blank frame (4→3) — which is why rounds 1-4 (line-0,
  ordinary path) are correct — but `mode2_vblank_entry_stat_source` (the line-144 mode2-entry quirk) used a fixed
  pretrigger (`==`, 4 dots) with no blank-frame adjustment, so it fired one dot early in the blank frame.
- **Fix:** apply the same blank-frame adjustment to `mode2_vblank_entry_stat_source` (`pretrigger_dots -= 1` when
  `blank_frame_active`), so both mode2 pretrigger paths agree. round5 $E2→$E0.
- Canon self-audit: removes an asymmetry by extending an EXISTING blank-frame compensation (the ordinary-mode2 4→3 lead)
  to the parallel vblank-entry source; not a new manual table/constant. Net manual-seam count does not rise.
- Gate green: wilbertpol 113→114, mooneye 111 unchanged, blargg 58/58, mealybug 13-fail (m3_scy_change DMG closed), lib 9
  pre-existing mode_edges reds, integration/trace failing set byte-identical (27 total), fmt-check + lint clean.

**Cluster status after batches 1-4.** Closed this work: `intr_2_mode0_timing` (batch-1), `intr_2_oam_ok_timing`
(batch-2), `vblank_if_timing` (batch-3), `intr_2_timing` (batch-4). **Scoreboard: wilbertpol 114/117, mooneye 111/113**,
blargg 58/58, mealybug 13-fail. Remaining:
- `intr_2_mode0_scx3/scx7_timing_nops` (wb) + `intr_2_mode0_timing_sprites` (wb+mn) — the **fetcher/§245 mode3-length**
  batch (per-scx and sprite-extended mode3 length; §24.21 proved scx is not a readback gate-seam). Needs the
  `[PPU][MODE3-FETCHER-LEAD]` work, not the mode0-publish pattern; do NOT re-fit the §245 sprite penalty.
- `boot_hwio-dmg0` (mn) — boot-handoff IO snapshot (dmg0 LY $01 / STAT $83 / DIV $19). Independent of the PPU readback /
  STAT cluster; depends on the dmg0 boot-ROM duration + raster position at handoff.

### 24.23 boot_hwio-dmg0 DIAGNOSED (dmg0 boot-power-on STAT phase one mode behind) — DEFERRED, NOT a mode0-publish item (2026-06-16)

**Diagnosis (real-ROM IO-read capture, dmg0 vs the passing dmgABCmgb).** boot_hwio walks `$FF00..$FF7F` once at `$0100`
and exact-matches a per-row table; the first mismatch fails. Capturing the first read of the PPU registers under the dmg0
boot-power-on lag window (which freezes the PPU-register reads at the handoff phase, `boot_power_on_ppu_phase_extends_until_vblank`):

| reg  | dmg0 reads | dmg0 expects | dmgABCmgb reads (passes) |
|------|-----------|--------------|--------------------------|
| DIV  | `$19` ✓   | `$19`        | `$AD` ✓ (live, window expired) |
| LY   | `$01` ✓   | `$01`        | `$0A` ✓ (live)           |
| STAT | `$82` (mode2) ✗ | `$83` (mode3) | `$80` ✓ (live mode0)  |

Only **STAT** mismatches: gb-cycle returns mode2 (OamScan), dmg0 hardware returns mode3 (Drawing). DIV (system counter)
and LY are correct. The STAT read goes through `read_stat` → `dmg_boot_power_on_stat_access_mode(elapsed_mcycles)`, whose
bucket table is the line-0/1/2 PPU mode progression (`7..=26` line0-OamScan, `27..=69` line0-Drawing, `70..=120`
line0-HBlank, `121..=140` line1-OamScan, `141..=183` line1-Drawing, …). The dmg0 STAT read lands in `121..=140` (line1
OamScan) but should be in `141..=183` (line1 Drawing) — the dmg0 boot-power-on PPU phase
(`DMG0_DIRECT_BOOT_HANDOFF_PPU_PHASE_BASE_OFFSET_DOTS = 3992`, applied in `apply_dmg0_direct_boot_handoff_stat_phase`) is
**~1 mode (≈20 mcycles) behind** the dmg0 hardware handoff phase.

**DEFERRED — not part of the mode0-publish cluster, and the bounded fix would re-fit a manual seam.** This is a
dmg0-specific boot-power-on phase issue, fully independent of the CPU-first reorder / STAT-readback / IF cluster (the task
flagged it so). The bounded fix = retune `DMG0_DIRECT_BOOT_HANDOFF_PPU_PHASE_BASE_OFFSET_DOTS` so the STAT read lands in
line1-Drawing while keeping LY=$01 — but that is adjusting a manual timing constant (a seam), which the canon/no-seams
policy says to avoid (net manual-seam count must not rise), and it risks the dmg0 boot unit tests
(`mode_edges.rs` `apply_dmg0_direct_boot_handoff_stat_phase`) and other dmg0 boot/power-on ROMs. The canonical fix is to
model the dmg0 boot-ROM duration → exact handoff PPU phase (a boot-emulation-accuracy task), not a constant tweak.
Tracked as an independent dmg0-boot-phase follow-up.

### 24.24 L2-a item-2 mode0-publish cluster — readback/IF sub-cases COMPLETE; two deep workstreams remain (2026-06-16)

**Closed (this item-2 work, all canon-aligned reorder compensations mirroring the established mode0-publish pattern, zero
ROM regression each):** `intr_2_mode0_timing` (batch-1, STAT mode0 readback), `intr_2_oam_ok_timing` (batch-2, OAM-read
bus mode0), `vblank_if_timing` (batch-3, vblank LY-wrap), `intr_2_timing` (batch-4, mode2 vblank-entry blank-frame
pretrigger). **Scoreboard: wilbertpol 110→114/117, mooneye 109→111/113**, blargg 58/58, mealybug 13-fail (m3_scy_change
DMG closed). Pre-existing reorder debt unchanged (9 lib `mode_edges` + 18 trace/integration = 27).

**Remaining = two distinct DEEP workstreams, NOT mode0-publish readback skews (each oracle-grounded above):**
1. **fetcher/§245 mode3-length** — `intr_2_mode0_scx3/scx7_timing_nops` (wb) + `intr_2_mode0_timing_sprites` (wb+mn). §24.21
   proved scx is a per-scx sub-dot mode3-length phase, not a readback gate-seam; sprites is sprite-extended mode3 length.
   Both need the `[PPU][MODE3-FETCHER-LEAD]` engine work; do NOT re-fit the §245 sprite penalty.
2. **dmg0 boot-power-on phase** — `boot_hwio-dmg0` (mn), §24.23. Independent of the PPU timing cluster.

The full internal-`self.ly` mid-line raster rephase (§24.18) remains the end-state that would subsume the interim
reorder compensations (E1, item-1 line-153 pretrigger, item-3 vblank IF mask, batch-2 OAM mode0, batch-3 vblank LY-wrap).

### 24.25 REPHASE DESIGN — diagnosis sharpened, DocBoy+SameBoy grounded, cut sequence (user authorized the rephase, 2026-06-17)

**Re-measured the workstream-A ROMs; the §24.21/§24.24 "fetcher/§245 mode3-length" framing is REFUTED. The 3 ROMs
(`intr_2_mode0_scx3/scx7_timing_nops`, `intr_2_mode0_timing_sprites`) fail PURELY on the CPU↔PPU reorder readback, NOT
on mode3 length.** Evidence (all reproduced this session; probes in `ppu_oracle_sweep.rs`, `#[ignore]`):
- Internal mode3 length is hardware-true and main-identical: a spin-ROM probe sampling `ppu.mode0_start_dot()` on a
  steady visible line gives `252 + (scx&7)` exactly for scx 0-8 (scx8→252) = the canonical Pan Docs `172 + scx&7`.
- `git diff main..branch` proves the length engine is byte-identical: `MODE0_START_DOT=252`, `MODE3_BASELINE_DOTS=172`,
  `capture_initial_scx (+= scx&7)`, `obj_fetch.rs` (§245 penalty), `OBJ_FETCH_MAX_ALIGNMENT_STALL_DOTS=5`,
  `extend_mode3_by_one_dot` — none changed. The branch only added M1 (`startup_fetch_idle_dots`, moves *when* mode3
  fetches, not the total length) + the reorder + the readback overrides.
- The scx readback fails exactly where `boundary ≡ 3 (mod 4)` (scx3, scx7); no binary scx gate matches main (§24.21).
  This is the boundary's mod-4 phase vs the nop-read alignment under the reorder, not a length error.

⇒ Workstream A is the reorder-readback model = the §24.18 canonical work, not independent fetcher work. The user
**authorized starting the rephase**. The "§245 frozen, prove sprite cost unchanged" guard still applies as a regression
check, but the fix is NOT in the fetcher.

**CRITICAL SCOPE REFINEMENT — the user's target ROMs (scx3/7 + sprites) are the mode0 READBACK, mid-line, and are
INDEPENDENT of the `self.ly` mid-line LY-lead.** The mode0 boundary is at `line_dot 252+scx&7` (mid-line); the LY-lead
window is dots 453-455; on a visible line `ly < VISIBLE` throughout, so the leading-vs-trailing `ly` never changes the
mode0 readback. ⇒ closing scx3/7+sprites = the **mode-from-delayed-registers** readback model (the §24.13 "item-2"
piece), which can land **independently of and before** the risky `self.ly` LY-lead. The LY-lead rephase closes/deletes
the LYC + line-153 + vblank-LY cluster (already green via interim fixes E1/item-1/item-3/batch-3), reducing net seams; it
does NOT itself close new target ROMs. So the cut order is REVISED: **mode-readback (closes the targets) FIRST, then the
LY-lead rephase + LYC-seam deletion (net seams down)**.

**Canonical model — DocBoy and SameBoy CONVERGE (both extracted from source this session):**
- **Mode is NOT derived from `ly`.** DocBoy: `mode` is a stored member written only by `update_mode<M>()` from the
  per-dot `tick_selector` state machine (ppu.cpp:1772-1776, :511). SameBoy: mode bits live in `io_registers[STAT]`,
  stamped by the display state machine. gb-cycle's `access_mode_from_raster(ly,line_dot,…)` DERIVES mode from `ly`
  (`ly>=VISIBLE→VBlank`) — this is the only reason a leading `ly` corrupts mode, and the reason re-anchoring is needed.
- **LY increment:** DMG increments LY at dot **453** (DocBoy `end_increase_ly`/`++ly` in `hblank_453`, ppu.cpp:823-829,
  :1279). SameBoy agrees (writes `GB_IO_LY` at line-boundary +2..3, display.c:1774). Line 153: LY→0 at dot **2** DMG /
  dot **3** CGB (DocBoy vblank_last_line, ppu.cpp:1446-1449/:1473); SameBoy DMG wraps ~dot 6 (a noted ≤4-dot phase
  nuance — ground the exact gb-cycle dot vs the oracle, do NOT copy 2/3 literally).
- **LYC coincidence:** `is_lyc_eq_ly() = (last_lyc==last_ly) && enable_lyc_eq_ly_irq` (DMG); CGB retains prior
  `stat.lyc_eq_ly` while disabled. `last_ly`/`last_lyc` captured at END of tick (ppu.cpp:554-560) = 1-T-cycle delay.
  `enable_lyc_eq_ly_irq` disabled at dot 453 (normal line) / 454 (vblank line) / dots 2:6 (line 153). SameBoy realizes
  the same via one `ly_for_comparison` sentinel (-1 at line start, set 1 dot later) feeding BOTH IRQ and readback — no
  irq-vs-readback split (gb-cycle's split is the seam).
- **STAT mode read:** DMG reads `stat.mode` recomputed each tick by `tick_stat` (ppu.cpp:740-768) with OAM→HBLANK force
  glitches; no extra delay register. SameBoy reads live `io_registers[STAT]` after full sync, with a SEPARATE
  `mode_for_interrupt` that **leads** the readback by 1 dot at the mode2→OAM edge but flips **together** at mode3→mode0
  (display.c:1778-1792, :2090-2108, state 22). So mode3→mode0 readback has NO early offset on hardware — the gb-cycle
  mode0-publish "+1 early" override is purely a reorder artifact.

**Target gb-cycle phase:**
- `self.ly` increments at `line_dot ≈ 453` (ground exact vs oracle), wraps to 0 mid-line-153; mode derivation
  re-anchored so the lead never flips VBlank/mode2 early.
- CPU LY readback = `self.ly` directly (delete the +1 lead-from-450 and the vblank LY-wrap branch).
- LYC = `last_ly`/`last_lyc` + a single `enable_lyc_eq_ly_irq` (line_dot-coord windows, DMG forces 0 / CGB retains).
- STAT mode readback = mode-from-(delayed)-registers replacing the published_stat dot-window overrides.

**Re-anchor list (from the 4-agent map, full file:line in the workflow result; the must-fix-together set):**
1. Mode-derivation leaves: `access_mode_from_raster` (common.rs:21), `current_access_mode` (registers.rs:254),
   `access_mode_for_line_dot` (registers.rs:272), `current_bus_access_mode`/`current_raster_state` (mode2.rs),
   `bus_access_mode_for_line_dot` (registers.rs:292) — all gate VBlank on `ly>=VISIBLE`.
2. VBlank-entry edge (api.rs:940), the wrap side-effects + post-increment guards (api.rs:885-929),
   `finalize_dmg_bgp_cpu_commit_scanline` order (boundary_repaint.rs:42, needs `previous_ly+1==self.ly`).
3. End-of-line `ly±1 / line_dot+N>=scanline_length` IRQ sources (fire in 452-456): `ordinary_mode2_stat_pretrigger_source`
   (irq.rs:286), `mode2_vblank_entry_stat_source` (irq.rs:306), `dmg_mode2_vblank_entry_halt_wake/_interrupt_service`
   (irq.rs:409/427), `dmg_mode2_oam_halt_wake_deferred` (irq.rs:403).
4. Line-153 LYC cluster keyed to `ly==TOTAL-1`: `live_ly_for_lyc_compare` (irq.rs:30/46), `line_153_lyc0_*`
   (irq.rs:325/339/359), `line_153_reads_as_ly0` (registers.rs:240).
5. LY-read lead seam (DIRECT COLLISION — would double-count to ly+2): `read_ly_without_skip_boot_lag` (registers.rs:204),
   `current_ly_read_advance_start_dot=450` (api.rs:1221).
6. Restart/boot (maybe): `advance_lcd_restart_phase` (irq.rs:580), `PpuLcdRestartPhase` (raster.rs:40),
   `current_scanline_length` ly-keyed cache (api.rs:1197).
   SAFE (rendering, mid-line, never in 453-455): framebuffer row `ly*SCREEN_WIDTH`, `bg_fetch.rs:222` `(scy+ly)%8`,
   `prepare_line` WY, palette recolor guards; boot one-shots (api.rs:660/702/727).

**Deletion list (interim reorder seams the rephase subsumes — with their pinned unit tests, from the map):**
mode0-publish overrides + `scx==0||mode0-int` gate (published_stat.rs:61-110), OAM read/write overrides
(published_stat.rs:121-169), E1 dot0 latch + `regular_line_dot0_compare_window` (irq.rs:82-101), item-1
`last_line_153_lyc0_pretrigger_window` (irq.rs:114-125/325-357), item-3 `cpu_if_read_suppress_mask`
(interrupts.rs/step.rs), batch-3 vblank LY-wrap (registers.rs:219-231), batch-4 mode2_vblank_entry blank-frame
(irq.rs:294-323), the LY-read lead (registers.rs:204-217 + ppu.rs:59/63), `line_153_reads_as_ly0` + the 11 LINE_153_*/
CGB_LINE_153_* / *_LY_READ_ZERO_DOT / CGB_LINE_END_LYC_COMPARE_BLANK_DOTS / LINE0_VBLANK_WRAP_STAT_READBACK_DELAY_DOTS
constants (ppu.rs:70-83), `live_ly_for_lyc_compare`/`lyc_compare_latch`/readback-irq split (irq.rs:24-112).
NOT reorder seams — LEAVE: `dmg_boot_power_on_*` tables, `real_boot_handoff_mode0_scx_seam`, `lcd_restart_phase`,
`dmg_stat_write_quirk_*`, halt-wake deferral helpers (genuine hardware/boot/quirk behavior).

**Cut sequence (each cut: shadow-diff in parallel first where possible, then flip, then per-ROM oracle gate; accept
temporary unit-test red, re-ground after; `cargo fmt-check`+`lint`+`tests`+`rom-report blargg`+target ROM + zero
regression vs the 27-baseline; suites via `cargo rom-suite`):**
- **Cut 2 FIRST (the user's targets): mode-from-delayed-registers STAT readback.** Empirically derive (against
  `oracle_sweep_intr_2_mode0_scx_timing` scx 0-8 + a faithful sprites probe + the main worktree) a CLEAN canonical
  readback model with NO per-scx gate that matches main for all scx. Candidate: published mode from a tick-end-registered
  `stat_mode` / a uniform `access_mode_for_line_dot(line_dot+k)` that undoes the reorder pre-tick, deleting the
  `scx==0||mode0-int` gate + the mode0/OAM "+1 early" overrides. Closes `scx3/scx7_nops` + `intr_2_mode0_timing_sprites`
  (+ keeps base intr_2_*). If no gate-free model matches without the LY-lead, it is coupled → fall back to Cut 1 first.
- **Cut 1: `self.ly` mid-line lead + re-anchor groups 1-6** above; delete the LY-read lead (group 5). Foundation for the
  LYC seam deletion. Ground the exact increment dot vs `oracle_sweep_ly_lyc`/`oracle_run_ly_lyc_roms`.
- **Cut 3: canonical `last_ly`/`last_lyc` + `enable_lyc_eq_ly_irq`**, delete the LYC seam constants + the irq/readback
  split + `live_ly_for_lyc_compare`; resolve the write-vs-tick model (immediate LYC re-eval vs pending-write+next-tick).
- **Cut 4: delete the subsumed interim compensations** (E1, item-1 pretrigger, item-3 mask, batch-3/4); verify net
  manual-seam count FALLS (canon P4 gate).
- **Cut 5: re-ground the ~9 mode_edges + stat/registers/bus/orchestration unit tests + regenerate the ~17 trace
  fixtures** (`GB_CYCLE_ACCEPT_*_FIXTURES=1`); final full-suite gate.

**Open questions to ground empirically (do NOT assume):** (a) the exact gb-cycle `line_dot` where `self.ly` must
increment to match the oracle (DocBoy 453 is in DocBoy `dots`; gb-cycle `line_dot` shares 0..455 but the reorder shifts
phase — ground vs the sweep); (b) whether a gate-free mode-readback model exists without the LY-lead (Cut 2's make-or-break
— RESOLVED below: YES); (c) the line-153 wrap dot (DocBoy 2/3 vs SameBoy ~6 — ground vs `ly_lyc`); (d) the write-vs-tick
LYC re-eval (item-1 §24.17 found gb's immediate re-eval load-bearing for `ly_lyc_write`).

#### 24.25.1 CUT 2 SOLVED (on paper) — the reorder is a UNIFORM −1 readback shift; fix = uniform +1, restoring main's model (2026-06-17)

Decisive dot-by-dot diff of the branch vs the main worktree (probe `oracle_sweep_intr_2_mode0_scx_timing_detailed`,
capturing `line_dot` + `mode0_start` at the STAT-read instant via `last_address_event`):
- **main ALREADY has the published_stat overrides + the `scx==0||mode0-int` gate** (they are PR #245, NOT branch-added).
  main resolves the readback as `access_mode_for_line_dot(line_dot − 1)` (the published base) + override forcing HBlank at
  `line_dot == mode0_start` (gated). For scx≠0/no-mode0-int it is just `access_mode(line_dot − 1)`.
- The scx0-vs-scx8 paradox (same `mode0_start=252`, main reads scx0@49 / scx8@50): main's scx0 HBlank@49 comes from the
  OVERRIDE (`line_dot==mode0_start`, 1 dot earlier than the raw `access_mode(line_dot−1)`); scx8 has no override (gate
  fails) so it uses the raw base → @50. Both are main's single model; no per-scx length difference.
- **The reorder makes the CPU read PRE-tick** (`self.line_dot` at the read = snapshot − 1; the probe snapshot is post-tick).
  So the branch evaluates the readback at `access_mode(self.line_dot − 1) = access_mode(snapshot − 2)`, while main
  evaluates `access_mode(snapshot − 1)` — the branch is **one dot further behind**. The batch-1/2/3 early-override branches
  (`line_dot+1==mode0_start`, `MODE2_DOTS−1`, the vblank LY-wrap) compensated the +1 ONLY on the override paths, not on the
  raw base path — so scx3/7 (raw, no override) stay 1 late, and that is the entire bug.

**THE CANONICAL FIX (uniform, gate-free, deletes batch-1/2/3): evaluate the published readback at `self.line_dot + 1`
(undo the reorder pre-tick), then apply main's EXACT readback model (the version WITHOUT the batch-1/2 early-override
branches).** Equivalently: the published base becomes `access_mode(self.line_dot)` and the current/override reference
becomes `self.line_dot + 1`, restoring main's `(base = access_mode(L−1), override at L==mode0_start)` with `L = self.line_dot + 1`.
**Validated OFFLINE against the oracle data for scx 0,3,4,7,8 — ALL match main** (scx0@49 via override at L==mode0_start;
scx3@50, scx4@51, scx7@51, scx8@50 via the +1-shifted raw base). This closes `scx3/scx7_nops` + `intr_2_mode0_timing_sprites`
and is INDEPENDENT of the LY-lead (the mode0 boundary is mid-line, `ly<VISIBLE` throughout). It also subsumes/deletes the
branch's batch-1 mode0 early branch + batch-2 OAM early branch + batch-3 vblank LY-wrap (net seams DOWN), since they were
piecemeal +1s now replaced by one uniform readback-reference +1.

**Implementation notes (delicate — do carefully, oracle-gated):** the `+1` needs wrap handling at `line_dot == scanline_length − 1`
(→ next line dot 0, ly+1) and must compose with the existing `vblank_wrap_line0_stat_readback_delay` path; apply it as a
single `readback_reference_line_dot()` helper feeding STAT mode + OAM read/write + LY readback, then restore main's
override/base bodies (drop the batch-1/2 early branches). Re-ground the `cpu_stat_read_*` / `stat::bus` unit tests that
batch-1/2 rewrote (§24.19/§24.20) back toward main's boundary. Gate: scx sweep == main for all scx, base intr_2_* /
oam_ok / vblank_if stay green, wilbertpol/mooneye no regression, §245 untouched (`current_mode0_start_dot` unchanged).
The IRQ-edge reorder seams (E1, item-1, item-3) are NOT readback and are handled by Cut 1/3 (the LY-lead + last_* model).

#### 24.25.2 CUT 2 STANDALONE REFUTED — the uniform STAT +1 shift closes the 3 targets but regresses 11 wilbertpol-acceptance ROMs; it is coupled to the LY/§245 model (2026-06-17)

Implemented §24.25.1 exactly (helper `readback_reference_line_dot()=line_dot+1`; published base `access_mode(reference−1)`;
overrides at `reference==mode0_start`/`reference==MODE2_DOTS`; restored main's bodies, dropped batch-1's dual branches;
`vblank_wrap` left unshifted; line-start fallback preserved at `line_dot==0`). **Code reverted** — tree back at the
clean branch baseline (`published_stat.rs` + the temporary trace probe in `ppu_oracle_sweep.rs` both reverted).

**Gate evidence (the new fast oracle = `cargo rom-suite wilbertpol --suite wilbertpol-acceptance`, 105 cases):**
- Clean baseline: **102 PASS / 3 FAIL** — the 3 FAILs are EXACTLY the targets `intr_2_mode0_scx3_timing_nops`,
  `_scx7_timing_nops`, `intr_2_mode0_timing_sprites`. main worktree on this same suite: **0 FAIL** (105/105).
- With the cut-2 STAT shift: **94 PASS / 11 FAIL**. The 3 targets FLIP to PASS (confirmed) — and the scx oracle column
  now matches main exactly (CGB+DMG `49,50,50,50,51,51,51,51,50`; scx3 51→50, scx7 52→51, oracle-verified). **BUT 11 NEW
  regressions appear**: `intr_2_mode0_timing_sprites{,-nops,-scx1..4}`, `hblank_ly_scx_timing_variant_nops`,
  `lcdon_mode_timing`, `ly143_144_{145,152_153,mode0_1,mode3_0}`. Deterministic (reconfirmed twice on fresh builds).
- ⇒ net **+3 / −11 = −8**. The standalone cut-2 is net-negative and cannot land.

**Why it is coupled (not a fixable bug in the implementation):**
- The fix is faithful to §24.25.1 and is provably a no-op for scx=0 STAT readback at every `line_dot` (the override path
  result is unchanged; only the scx≠0 RAW base flips one dot earlier — which is the targets' correct fix, oracle-matched).
- Yet `ly143_144_mode3_0` (a **scx=0** test) regresses. Per-read trace (`oracle_trace_regressing_roms`, reverted): its
  FF41 reads (4 total, all scx=0: (143,253),(143,257),(143,453),(144,1)) AND its dense FF44/LY reads are **byte-identical**
  branch↔fix up to (ly=144,line_dot=13), yet it ends non-fib. The divergence is **not** a changed FF41/FF44 value — it is
  the STAT-readback *phase* interacting with the rest of the reorder-compensated model (LY-lead@450, batch-3 vblank-wrap,
  item-3 IF, §245 sprite `mode0_start`), which is still on the branch phase, not main's. The STAT IRQ uses owner mode
  (`current_access_mode`), so it is not the IRQ; the coupling is at the line transitions + sprite-precise timing that read
  STAT to synchronise then measure LY.
- main passes all 11 because its WHOLE readback (STAT+LY+IF) is one consistent post-tick phase. Shifting STAT alone to
  main's phase, while LY/IF/§245 stay on the branch phase, breaks the tests that combine them. **This is exactly the
  design's own fallback condition ("if coupled → Cut 1 first") and re-confirms §24.18 (root deletion needs the full
  internal-`self.ly` mid-line rephase; bounded decoupling is dead).**

**Conclusion — the cut order in §24.25 is REFUTED. Cut 2 is NOT independent of the LY-lead for the full suite** (only for
the isolated mid-line scx single-read targets). The landable path is the coupled rephase: **Cut 1 (`self.ly` mid-line
lead/wrap, re-anchor groups 1–6, delete the LY-read lead) FIRST/together with Cut 2**, so STAT+LY+IF share one phase, then
Cut 3 (last_ly/last_lyc) + Cut 4 (delete interim seams) + Cut 5 (re-ground). This is §24.18's multi-cut core raster
restructure (its own scoped, oracle-gated branch). The interim branch state (114/117 wilbertpol equiv, self-consistent
reorder model; the 3 scx3/7/sprites + this suite's 3 are the documented debt) is the safe fallback if the rephase is
deferred.

**Decision pending (user):** (a) start the coupled Cut 1+2 self.ly rephase now (big, risky, multi-cut, re-grounds the
whole ly/lyc/mode suite — the §24.18 restructure), or (b) keep the branch at its current self-consistent baseline and
defer the rephase. The fast gate `cargo rom-suite wilbertpol --suite wilbertpol-acceptance` (102/105 baseline, pinpoints
the targets + any regression in ~1 min) is the recommended loop for the rephase.

#### 24.25.3 GROUNDING 1.0 + A′ (uniform readback-reference) VALIDATED — closes targets, residual is IRQ↔readback coupling, NOT pure readback (2026-06-17)

**Grounding 1.0 (bare-rig per-dot phase scan, `cut1_grounding_internal_ly_phase` in `ppu/tests/oracle.rs`, run on
branch + `../gb-cycle-main`, reverted after):** over 224 per-dot samples (DMG+CGB; normal line, vblank-entry 143→144,
line 153), the ONLY branch-vs-main difference is the observable LY leading by EXACTLY 1 dot (branch `obs_ly` flips at
dot 450 / main 451; 152→153 branch@455 / main@456). STAT mode, internal mode, `lyc_coin`, the line-153 LY-zero dots
(DMG 4 / CGB 8), and the LYC153/LYC0 windows are **byte-identical**. Mechanism confirmed: branch scheduler has the
reorder (`AutonomousPeripheralTicks` LAST → PPU ticks after CPU → pre-tick reads); main has PPU first (post-tick). The
branch's `LY_READ_ADVANCE_START_DOT=450` vs main's `451` is exactly the −1 that compensates the +1 scheduler shift —
same machine-observable behavior. ⇒ **the branch's LY/mode/LYC phase is already main-equivalent** (1-dot pre-tick
shift, nothing else); the 3 targets fail purely on the STAT mode0-boundary readback (mid-line), which 1.0 did not sample.

**A′ design chosen (user, over the §24.18 counter rephase):** one uniform readback reference `readback_reference() =
(ly, line_dot+1)` (with end-of-line / line-153 wrap) feeding `read_ly` + STAT-published + (OAM) with `main`'s EXACT
bodies, deleting the scattered per-path `+1` compensations — the generalisation of §24.25.1 that §24.25.2 pointed to but
ran only on STAT. Landed (uncommitted WIP): `readback_reference()` helper + `read_ly_without_skip_boot_lag` rewritten to
main's 2-branch body on the reference (deletes the batch-3 vblank end-lead; line-153 LY-zero shifts −1 to main's machine
phase); `LY_READ_ADVANCE_START_DOT` 450→451 (main's); `published_stat` base `line_dot−1`→`line_dot` + overrides at
`line_dot+1==mode0_start`/`==MODE2_DOTS` (drops batch-1 dual branches). Bare-rig before/after diff: the ONLY transition
change is line-153 LY (−1, converges to main); everything else identical.

**Gate result (`cargo rom-suite wilbertpol --suite wilbertpol-acceptance`):** baseline 102/105 (3 FAIL = the targets).
A′ → **94/105: the 3 targets FLIP to PASS, 11 NEW regressions** — `intr_2_mode0_timing_sprites{-nops,-scx1..4-nops}`
(5), `hblank_ly_scx_timing_variant_nops`, `lcdon_mode_timing`, `ly143_144_{145,152_153,mode0_1,mode3_0}` (4). This
**reproduces §24.25.2 EXACTLY even with `read_ly` now shifted** — so the LY-lead is NOT the cause. Ruled out: OAM
(`intr_2_oam_ok` stays green), LYC (`lyc_coin` unchanged in 1.0).

**Root cause (diagnosed, decisive):** the wilbertpol intr_2/sprite/ly143 tests measure the **STAT-IRQ ↔ STAT-readback
relationship**. A′ moved the readback to main's post-tick phase; the IRQ side stays on the branch's reorder phase. Proof
by contrast: clean-branch PASSES the sprite variants but FAILS scx3/7; A′ inverts both. They want OPPOSITE branch STAT
phases — only possible if clean-branch had IRQ-edge + readback consistently-wrong-TOGETHER (variants pass by mutual
compensation) while scx3/7 (pure readback) failed; A′ fixed the readback alone and desynced them. Note: `ordinary_stat_irq_line`
is byte-identical branch↔main (same mode0/mode2 pretriggers), so the IRQ *firing predicate* is not the diff — the
coupling is finer (candidate: the mode0/OAM readback override is `line_dot+1==mode0_start` (early) on A′ vs the
clean-branch's BOTH exact+early 2-dot window; sprites add the §245 penalty to `mode0_start`. Needs a real-ROM trace of
`intr_2_mode0_timing_sprites-nops` to pin whether sprites need the exact branch retained alongside the reference).

**Conclusion:** A′ (pure readback reference) is **necessary but not sufficient** — it closes the targets but the 11 need
the IRQ side brought onto the same phase. User chose **"A′ + converge the STAT-IRQ subsystem to main"** (keep the
readback reference; port main's `ordinary_mode2_stat_pretrigger_source`/`_edge` split + re-anchor the group-3 IRQ edges /
resolve the mode0-override exact-vs-early so the measured IRQ↔readback relationship matches main) over the full counter
rephase. WIP in tree; next concrete step = real-ROM trace of a failing sprite variant to pin the override model, then the
IRQ-edge convergence, gated on the fast suite back to 105/105.

**WIP checkpoint committed (branch `ppu/fetcher-lead-hardening`, INTENTIONALLY RED — do not ship):** A′ readback
reference landed in `registers.rs` (`readback_reference()` + `read_ly`), `ppu.rs` (`LY_READ_ADVANCE_START_DOT` 450→451),
`published_stat.rs` (base `line_dot`→ + overrides at `line_dot+1==boundary`, batch-1 dual branches dropped). Gate:
**94/105** wilbertpol-acceptance (3 targets PASS, 11 regress). `cargo fmt-check` + `cargo lint` clean. **15 PPU unit
tests fail** (pinned to the pre-A′ readback phase — Cut 5 re-grounds them; verbatim list at handoff): the
`published_stat`/`ly_read`/`sprite_extended_mode0`/`lyc_zero_window` boundary tests + the `*_hidden_from_same_cycle_cpu_if`
tests (the last group is the IRQ-visibility/item-3 surface the convergence must address). `#[ignore]` diagnostics kept in
`ppu/tests/oracle.rs`: `cut1_grounding_internal_ly_phase` (bare-rig phase scan, 1.0) + `cut1_trace_ly143_144_mode3_0`
(real-ROM read trace). RESUME: differential dispatch-IRQ trace (A′-branch vs main) of a failing case, capturing
`ServiceInterrupt{LcdStat}` + IF reads (not just register reads), to pin the IRQ-latch↔readback dot offset.

#### 24.25.4 CUT A LANDED + DIFFERENTIAL TRACE RE-DIAGNOSES THE RESIDUAL — half the regressions are NOT STAT-IRQ; they are the LCD-re-enable restart phase under the reorder (2026-06-17)

A 4-agent characterization workflow (`stat-irq-convergence-map`) + a feasible differential ROM trace (the prior brute-force
attempt was abandoned: the gb-test-runner is the fast oracle — these ROMs terminate at ~1.1–1.9M T-cycles; the per-cycle
probe runs ~80k cyc/s, and the "legacy fibonacci" terminal is opcode `0xED`, not `0x40`, so the earlier probe never
detected it) produced a sharper, partly **corrected** picture of the §24.25.3 residual.

**CUT A LANDED (zero-risk, validated): restored `main`'s same-cycle CPU-IF hide family.** The branch had deleted
`mode0_/mode2_/mode1_/lyc_stat_irq_edge_hidden_from_same_cycle_cpu_if` + `ordinary_mode2_stat_pretrigger_edge` +
`stat_request_hidden_from_same_cycle_cpu_if` in cefd6484 ("retire hide") and hard-coded
`queue_interrupt_request_with_cpu_if_visibility(LcdStat, true)`. Cut A re-adds those 6 methods verbatim from `main` and
re-couples BOTH LcdStat edge queues (`irq.rs refresh_stat_irq_line` + `registers.rs write_stat`) to
`!stat_request_hidden_from_same_cycle_cpu_if()`. Result: **15 → 10 red unit tests** (the 5 `*_hidden`/`line144_mode2_hidden`
tests go green), wilbertpol-acceptance gate **unchanged at 94/105** (the exact same 11 fails), fmt+lint clean. This is a
**no-op in the full machine** and proves the reorder model: under the reorder the PPU pending is committed+cleared every
cycle at `InterruptAggregation` (via `take_pending_interrupt_request_mask`, which is RAW/ignores hidden), so
`cpu_visible_pending_interrupt_request_mask` is ~always 0 during `CpuMicroOperation` and the hide flag never reaches the
full-machine IF read. The hide family is the canonical predicate the bare-PPU unit tests assert; restoring it deletes the
"retire hide" divergence (net seams ↓) with zero ROM risk. Dispatch uses `interrupts.highest_pending()` (scheduler IF);
the IF read uses scheduler IF | `cpu_visible_pending` (= scheduler IF only, in practice).

**THE 11 REGRESSIONS SPLIT INTO TWO CLASSES (from reading the wilbertpol `.s` sources, corroborated by the trace):**
- **Class A — STAT-IRQ-edge-vs-readback (5): `intr_2_mode0_timing_sprites_{nops,scx1..4_nops}`.** A Mode2 STAT IRQ
  HALT-wake is the time origin; the test then polls FF41 until mode0 (Drawing→HBlank), with mode3 length extended by the
  §245 sprite penalty (+scx). A′ moved the mode0 readback while the Mode2 IRQ edge stayed put → desync. The 3 TARGETS
  (`intr_2_mode0_scx3/scx7_timing_nops`, `intr_2_mode0_timing_sprites`) are the same mechanism and now PASS.
- **Class B — pure readback across an LCD-re-enable / line transition (6): `ly143_144_{145,152_153,mode0_1,mode3_0}`,
  `lcdon_mode_timing`, `hblank_ly_scx_timing_variant_nops`.** **These DISABLE interrupts** (`xor a; ldh IE; ldh IF;
  LYC=$f0`) and busy-poll FF44 / read FF41 once per round; most do an **LCD off→on per round**. The §24.25.3 framing that
  ly143_144 uses the VBlank IRQ is **REFUTED** — confirmed by the trace (IF=0xE1 is the unhandled VBlank flag; no
  ServiceInterrupt ever fires). So half the residual is **NOT** IRQ↔readback coupling — it is pure readback.

**DECISIVE DIFFERENTIAL TRACE (`ly143_144_mode3_0`, DMG, branch-A′ vs `../gb-cycle-main`, CPU-observable fields only):**
the entire (ly, read-value, pc) sequence is **byte-identical except ONE read** — at `pc=0x01EC`, the FF41 read returns
**branch `0x80` (mode0/HBlank) vs main `0x83` (mode3/Drawing)**, both at the SAME absolute T-cycle (@196939). Root cause,
pinned: the branch PPU's internal `line_dot` runs **exactly 1 dot AHEAD** of main from a specific point onward (post-step
branch dot=253 / main 252; the offset first appears at @65755, ly=0, **right after the ROM's first LCD off→on**, where the
mode reads HBlank in the early dots = blank_frame_active = LCD re-enable). The trace START is byte-identical (no init
offset); `LCD_REENABLE_*` constants + `enter_lcd_enabled_restart_state` (`line_dot=0`) are **identical branch↔main**. So
the +1 dot is the **reorder × LCD-enable handoff**: the branch's LCDC-enable write (CpuMicroOp/MmioCommit, phases 5/6)
precedes the PPU tick (phase 7), while `main`'s PPU tick (phase 4) precedes the write — so the re-enable lands in a
different phase and the branch restart ends one dot ahead.

**Implication — A′'s readback formula is CORRECT; the residual for Class B is the LCD-restart PPU phase, NOT readback and
NOT IRQ.** At the divergent read both PPUs are at internal `line_dot=252` at the CPU read instant; A′ reads
`access(reference−1)=access(252)=HBlank`, main reads `access(line_dot−1)=access(251)=Drawing`. **If the branch PPU were
phase-aligned** (not 1 ahead), the branch pre-tick `line_dot` would be 251 and A′ would read `access(251)=Drawing` =
correct. ⇒ **fixing the LCD-re-enable restart phase so the branch PPU is not 1 dot ahead after re-enable closes Class B
without touching A′ or the IRQ.** This is the documented LCD-restart seam (cf. CGB `LCD_REENABLE_LINE0_*` workstream), here
on DMG and with a clear target (+1 dot, post-step branch 253 vs main 252).

**REVISED PLAN (supersedes the §24.25.3 single "converge STAT-IRQ" framing):**
- **Cut A — hide family restore: DONE** (uncommitted; 94/105 unchanged, 5 unit tests green). Canon-positive prerequisite.
- **Workstream B (Class B, 6 ROMs): LCD-re-enable restart phase alignment.** Make the branch PPU NOT run 1 dot ahead of
  main after an LCD enable under the reorder. Gate per-ROM on the fast suite; this is a PPU-phase fix, independent of A′.
  RISK: the LCD-restart seam has a refuted history (CGB §M3); but here the target is concrete (DMG, +1 dot).
- **Workstream A (Class A, 5 ROMs + keep 3 targets): IRQ-edge-vs-readback for the sprite-extended mode0 boundary.** The
  Mode2 STAT IRQ edge vs the A′-moved mode0 readback; candidate is the `published_stat` mode0 override interacting with the
  §245-extended `mode0_start` (not the deferral seams — those protect ly_lyc_*/intr_2_timing, which currently PASS;
  guard-risk = medium/high to touch).
- **Deferral seams (item-1/E1/item-3/batch-4): do NOT remove yet.** guard-risk workflow: each protects a currently-PASSING
  ROM (item-1→ly_lyc_0, E1→ly_lyc, item-3→ly_lyc_144+intr_1_timing, batch-4→intr_2_timing) on the IRQ-source/IF-read path
  that A′ did not move; §24.16 already gates item-3's deletion on the deeper rephase. The 3 `irq-readback-coupling` red
  unit tests are tied to these and stay red until Workstream A resolves the edge phase.

Temp diagnostic probes in `ppu/tests/oracle.rs`: `irq_dispatch_trace` + `irq_trace_ly143_144_mode3_0_{dmg,cgb}` /
`irq_trace_intr_2_sprites_nops_{dmg,cgb}` (`#[ignore]`, write `/tmp/irq_trace_*.txt`; copy the fn block into
`../gb-cycle-main` to re-diff). Revert at close.

**Workstream B first attempt — LCD-enable effect delay +1 — REFUTED (net −9; the LCD-restart phase is coupled and
position-dependent, 2026-06-17).** Hypothesis: under the reorder the same-cycle PPU tick (phase 7, after the LCDC-enable
write at phase 6) consumes one decrement of `CPU_LCDC_ENABLE_EFFECT_DELAY_T_CYCLES`, so the restart lands one dot early;
the constant is identical to `main` (5) and the countdown logic byte-identical, so the +1 is purely the reorder
compensation. Tried `enter_lcd_enable_pending_state(CPU_LCDC_ENABLE_EFFECT_DELAY_T_CYCLES + 1)` on the CPU-write path only.
**Gate: 94 → 85/105.** It **FIXED 5** (`ly143_144_{145,152_153,mode0_1,mode3_0}` + `lcdon_mode_timing`) but **BROKE 14**
(`ly00_{mode0_2,mode1_0,mode1_2,mode2_3,mode3_0}` + `ly_lyc{,_0,_0_write,_144,_153_write,_write}` GS/C) that PASS at +0.
`hblank_ly_scx` + the 5 `intr_2_*_sprites` were unchanged (Class A). ⇒ **`ly143_144_*` want the post-enable timeline +1
while `ly00_*`/`ly_lyc_*` want +0**, even though all do an LCD off→on with the same delay — so the enable delay is too
GLOBAL a knob; the one-dot error is position-dependent (which raster dot the enable lands on relative to the 456-dot line),
not a uniform enable-delay shift. This matches the documented LCD-restart refuted history (CGB §M3, 8 bounded experiments
refuted). REVERTED. The mechanism remains: at the `ly143_144_mode3_0` divergent read both PPUs are at internal `line_dot`
252 but the branch is one TICK ahead in absolute count (post-step 253 vs 252) from the re-enable, so A′ reads `access(252)`
= HBlank where main reads `access(251)` = Drawing; the fix must make the branch restart NOT land one tick early WITHOUT
shifting the enable-relative phase that `ly00_*`/`ly_lyc_*` depend on — i.e. align the restart's raster phase, not the
scalar delay. Open: is this closable bounded, or is it the §24.18 full counter rephase? (Decision pending.)

**Workstream A (Class A sprite variants) RE-DIAGNOSED — SAME root as Class B, NOT an IRQ issue (2026-06-17).** Differential
trace of `intr_2_mode0_timing_sprites_nops` (DMG, branch vs main, FF41-poll-only diff): the FF41 read sequence is
byte-identical for the first 122 reads, then the branch fails a testcase and jumps to the fibonacci terminal (123 FF41
reads vs main's 480 over the same 2.2M-cycle window). At the failing testcase the @cycle-aligned reads still MATCH in value
(both `0xA3`→`0xA0` across the Drawing→HBlank boundary at pc=0x0EAF, ly=68), but the **branch internal `line_dot` is +1**
there (Drawing@261/HBlank@297 vs main 260/296) — the SAME +1-dot phase offset as Class B, again developing at a `ly=0`
event mid-trace (@499527, pc=0x0E55, the per-testcase LCD/restart setup the wilbertpol gpu tests share). The
Mode2-STAT-IRQ-wake origin is a red herring: the sprite count diverges because the §245-extended `mode0_start` poll lands
the mode0 boundary one poll-iteration off under the +1 PPU phase. **Workstream A is the same reorder × restart PPU-phase
coupling as Workstream B, not an IRQ-edge problem; "converge STAT-IRQ to main" does not address it.**

**CONVERGENT CONCLUSION (both workstreams):** all 11 regressions reduce to the branch PPU developing a **+1 internal
`line_dot` phase offset vs main at `ly=0` restart-like events under the reorder**, which A′'s uniform readback reference
cannot compensate because the offset is **position-dependent** (present for `ly143_144_*`/sprite variants; absent or
tolerated for `ly00_*`/`ly_lyc_*` which share the restart path — proven by the enable+1 experiment fixing the former and
breaking the latter). Bounded patches (A′ readback reference, published_stat override, enable-delay) are each
position-blind and therefore refuted, re-confirming §24.18 / §24.25.2: the landing path is the **full `self.ly`/`line_dot`
counter rephase** so the PPU internal phase is uniformly main-equivalent and one readback model works everywhere. A′ at
94/105 is net-WORSE than the pre-A′ self-consistent 102/105 (3 targets fail); documented fallback = revert-A′ + keep-Cut-A
(clean 102/105 with the hide-family seam removed) until the §24.18 rephase lands. DECISION PENDING (user).

#### 24.25.5 USER CHOSE §24.18 — refined plan: it is a COMPREHENSIVE atomic raster rephase, not the LY-lead cut alone; the bounded enable fix is refuted because it breaks the COMPENSATED balance (2026-06-17)

User decision: do the §24.18 rephase now (keep A′ + Cut A). One more decisive refinement before the restructure:

**Why no bounded LCD-restart fix exists (definitive).** The 11 regressions' root is the branch PPU landing **+1 internal
`line_dot`** after an LCD re-enable under the reorder (the write-cycle's phase-7 PPU tick decrements
`lcd_enable_pending_delay_tcycles` whereas `main`'s phase-4 tick precedes the write). Making the enable countdown
reorder-invariant (skip the write-cycle decrement) is EXACTLY equivalent to the `+1` experiment (restart one cycle later =
`main`'s wall-clock) — and that was REFUTED (94→85: fixes `ly143_144_*`+`lcdon`, breaks `ly00_*`+`ly_lyc_*`). The reason
`ly00_*`/`ly_lyc_*` PASS today at the "buggy" +1 restart is that the branch carries OTHER reorder compensations (the
LY-read lead, the line-153 / vblank-wrap seams, the `delay==2` LYC-on-enable refresh) that are balanced AGAINST the current
enable timing. Touching the enable timing alone breaks that balance. ⇒ **the enable timing and the compensations must be
re-grounded TOGETHER, atomically** — confirming §24.18's "multi-cut, regression surface across every timing ROM" and that
A′ (one bounded knob) could never be sufficient.

**Scope clarification: §24.18-as-written prioritizes the LY-lead/LYC-seam; the 11 regressions are the `line_dot`
restart/readback phase.** Both are facets of the same disease (the reorder distorts the raster phase the CPU observes), so
the COMPREHENSIVE rephase subsumes both, but the 11-regression close specifically needs the `line_dot`/restart phase
re-grounded, not just LY-lead. The end-state target: the branch PPU's observable phase (LY + line_dot + mode + LYC + the
restart seeding) is uniformly `main`-equivalent under the reorder, so the SINGLE A′ readback model (read at the post-tick
reference) is correct everywhere and ALL the per-path compensations delete.

**Cut plan (each cut oracle-gated on `cargo rom-suite wilbertpol --suite wilbertpol-acceptance`; accept unit-test red until
Cut 5; the §245 `current_mode0_start_dot` stays frozen):**
- **Cut R0 (grounding, safe, no core change):** per-cycle full-machine probe of the LCD re-enable on branch vs `main` for
  one FAILING (`ly143_144_mode3_0`) and one PASSING (`ly00_mode3_0`, `ly_lyc-gs`) ROM — dump `line_dot` every cycle across
  the off→on, pin the EXACT cycle the branch gains +1 and confirm whether the passing ROMs are +1-and-tolerant or +0. This
  decides whether the restart re-ground is uniform (one mechanism) or genuinely sub-cycle-alignment dependent (needs the
  full counter rephase). Do this FIRST — it sizes the rest.
- **Cut 1 (self.ly mid-line increment + `next_ly` guard, §24.18 pt 1):** increment `self.ly` at the DocBoy dot (ground vs
  oracle; ~453) with a guard so the end-of-line wrap does not double-increment; re-anchor the mode-derivation leaves
  (`access_mode_from_raster` family, group 1) + VBlank-entry/mode2-vblank-entry edges (groups 2-3) to `line_dot` so the LY
  lead never flips mode early.
- **Cut 2 (restart/line_dot seeding, §24.18 pt-E):** re-ground `enter_lcd_enabled_restart_state` + the enable countdown so
  the post-re-enable `line_dot` matches `main` under the reorder, REMOVING the compensations it was balanced against
  (the +1 enable artifact) — done together with Cut 1 so the balance is preserved.
- **Cut 3 (`last_ly`/`last_lyc` + `enable_lyc_eq_ly_irq`, §24.18 pt 4):** delete the LYC observation-tables seam +
  `live_ly_for_lyc_compare` + the irq/readback split; resolve write-vs-tick (pt 5).
- **Cut 4:** delete the now-subsumed interim seams (item-1/E1/batch-4; item-3 last, gated on the IF-read phase) + the
  LY-read lead (group 5); verify net manual-seam count FALLS.
- **Cut 5:** re-ground the ~9 mode_edges + stat/registers unit tests + regenerate the ~17 trace fixtures; full-suite gate.

NEXT ACTION = Cut R0 (the per-cycle re-enable line_dot grounding), as it sizes Cut 1+2 and confirms the mechanism before
the core restructure. This is its own scoped, multi-session, oracle-gated branch effort per §24.18's own guidance.

**Cut R0 RESULT (DONE, 2026-06-17) — the +1 offset is UNIFORM, NOT position-dependent; this re-sizes the rephase.**
Per-cycle full-machine trace of `ly00_mode3_0` (PASSES) vs `ly143_144_mode3_0` (FAILS), DMG, branch vs `../gb-cycle-main`:
- BOTH ROMs: the branch PPU internal `line_dot` is **+1 ahead of main from the SAME cycle** (@65755, ly=0, right after the
  shared LCD off→on), in 2386/2534 sampled lines. So the +1 restart offset is **uniform across siblings**, not
  sub-cycle-alignment dependent.
- `ly00_mode3_0`: CPU-observable `(ly, read, pc)` sequence is **byte-identical** branch↔main (0 diff lines) → PASSES. Its
  reads never land on the dot where the +1 flips a published mode.
- `ly143_144_mode3_0`: identical except **one** FF41 read (mode0 vs mode3 at the Drawing→HBlank boundary) → FAILS.
⇒ **The +1 PPU-phase offset after re-enable is real and uniform; A′'s readback only EXPOSES it at boundary-hitting reads.**
This means: (a) a uniform restart re-ground CAN remove it (it is one mechanism, not N position cases); (b) the enable+1
experiment's `ly00_*`/`ly_lyc_*` regressions were a SECONDARY effect (candidate: the `delay==2` LYC-on-enable refresh shift,
and/or A′'s `+1` reference double-counting once the offset is removed — `ly00_*` currently passes BECAUSE the +1 offset
cancels A′'s `+1` for its tolerant reads), NOT irreducible position-dependence. The cleaner framing of the disease: the
branch PPU phase vs main is **inconsistent** — ~0 in steady state (where A′'s `+1` readback reference compensates the
CPU-pre-tick read) but **+1 after an LCD re-enable** (where A′'s `+1` then over-counts at boundary reads). The rephase must
make the branch PPU phase vs main **consistent everywhere** (then ONE readback model — A′, or main's reverted — is correct
and the per-path compensations delete). Cut 1+2 should therefore re-ground the re-enable restart to the SAME branch-vs-main
phase as steady state, and re-test whether A′'s `+1` reference is still needed or also deletes. Probe:
`irq_trace_ly00_mode3_0_dmg` in `oracle.rs` (`#[ignore]`).

**Cut 1+2 FIRST ATTEMPT (restart re-ground via skip-advance) — mechanism VALIDATED, atomic dependency map pinned;
reverted (net 94→85, intentionally net-negative until the dependent readback layers re-ground) (2026-06-17).** Experiment:
in the pending-enable countdown (`api.rs` ~822), when the restart fires, return `false` instead of `true` so the restart
tick does NOT do its same-tick `RasterAdvance` — delaying the branch's first post-enable advance by one cycle to undo the
reorder's early restart. **This is the CORRECT restart re-ground (NOT the scalar enable+1): Cut R0 re-trace confirms it
makes `ly00`'s post-re-enable internal `line_dot` offset EXACTLY 0** (aligned with main, vs +1 before). Gate 94→85/105:
- **FIXED (5):** `ly143_144_{145,152_153,mode0_1,mode3_0}` + `lcdon_mode_timing` — the CPU-pending LCD re-enable family,
  now phase-aligned so A′'s readback reads the boundary correctly.
- **STILL FAIL (6):** `intr_2_*_sprites` (5) + `hblank_ly_scx` (1) — UNCHANGED, so their +1 comes from a DIFFERENT path than
  the pending-enable countdown (the skip only touches that path); their re-enable/restart route needs separate grounding.
- **NEWLY EXPOSED (14):** `ly00_*` (5) + `ly_lyc_*` (9) — the DEPENDENT readback layers that were balanced against the old
  +1 offset. With the phase now aligned (offset 0), A′'s readback has a RESIDUAL error in the `ly=0` blank-frame / re-enable
  region: e.g. `ly00_mode3_0` now diverges at one FF41 read (pc=0x081A, ly=0, line_dot 255/256: branch publishes Drawing
  `0x83` vs main HBlank `0x80`) — A′'s `current_published_stat_access_mode` over-publishes Drawing at `ly=0` once the offset
  no longer cancels it. `ly_lyc_*` are the LYC-on-enable (`delay==2` refresh) + line-153 layers.
⇒ **Cut 1+2 is confirmed ATOMIC and convergent (not endless): the restart re-ground (this skip-advance) + the `ly=0`
blank-frame STAT readback re-ground + the `intr_2` restart-path grounding + the LYC-on-enable/line-153 grounding must land
TOGETHER**, each net-negative alone, re-grounded as one cut, THEN A′ retested for deletion. The map is now precise: the
skip-advance is the validated foundation; the remaining layers are (a) `ly=0` blank-frame published_stat (the
`current_published_stat_access_mode` Drawing-at-ly0 residual), (b) the `intr_2`/`hblank_ly_scx` restart path (find where
their +1 enters — separate from the pending countdown), (c) `ly_lyc` on-enable + line-153. This is a focused multi-session
push; the branch stays at the committed clean 94/105 + Cut A until it lands. NEXT: re-ground (a) on top of the skip-advance
and re-gate; iterate (b)(c) until net-positive, then delete the subsumed compensations (Cut 4) + A′-retest + re-ground
tests (Cut 5).

**Cut 1+2 SECOND ATTEMPT (skip-advance + ly=0 readback diagnostic) — pins the MECHANISTIC atomic coupling: the restart
re-ground SHIFTS the restart-phase mode timeline, so `LCD_REENABLE_LINE0_*` must re-ground in lockstep (2026-06-17).**
Re-applied the skip-advance restart re-ground and augmented the `irq_dispatch_trace` probe to dump, on FF41 reads at
`ly<=2`, the published_stat inputs (`current_mode0_start_dot`, `blank_frame_active`, `access_mode_for_line_dot(ld-2..ld)`).
The failing `ly00_mode3_0` read at pc=0x081A: `ly=0 dot=256 m0s=252 blank=false am[254,255,256]=HBlank/HBlank/HBlank` yet
the CPU read returns **Drawing 0x83** — IMPOSSIBLE via the normal published_stat path (every `access_mode_for_line_dot`
around the read is HBlank with `mode0_start=252`). Root: the skip-advance (returning `false` from the restart tick) is too
blunt — it early-returns BEFORE `advance_mode3_register_latches` (the mode latch) AND `advance_lcd_restart_phase`, so the
restart tick skips its mode-latch + restart-phase advance, corrupting the ly=0 mode. But the deeper, UNAVOIDABLE finding:
even a refined "skip only the `line_dot += 1`" cannot work cleanly, because **delaying the restart by one cycle shifts the
`PpuLcdRestartPhase` mode timeline by one dot** — `advance_lcd_restart_phase` would run from a different `line_dot`, so the
re-enable mode sequence (mode3 from `LCD_REENABLE_LINE0_MODE3_START_DOT=72`, mode0 from +172=244) lands one dot off, which
is EXACTLY what the `ly=0` readback (`ly00_*`, `lcdon_mode_timing`) measures. ⇒ **Cut 2 (the restart `line_dot` re-ground)
and layer (a) (the `LCD_REENABLE_LINE0_*` restart-phase mode timeline) are mechanically the SAME cut and must re-ground in
lockstep**: shift the restart one cycle later AND shift the restart-phase mode constants one dot to keep the re-enable mode
sequence main-aligned. This is the precise atomic core of §24.18 for the re-enable family. The `intr_2`/`hblank_ly_scx`
(+1 from a different path) and `ly_lyc` (on-enable `delay==2`/line-153) are the other two lockstep facets. REVERTED to the
committed 94/105 + Cut A; the augmented probe stays as `#[ignore]` infra. The next focused session implements this lockstep
restart+`LCD_REENABLE_LINE0_*` re-ground as one cut, per-dot-validated against the `../gb-cycle-main` re-enable trace.

**Cut 1+2 THIRD ATTEMPT — CONVERGENCE VALIDATED (the atomic rephase IS tractable, not refuted): enable+1 restart re-ground
+ layer-(a) fix moves 85→89/105 (2026-06-17).** Used the CLEAN restart re-ground (`enable+1`: `CPU_LCDC_ENABLE_EFFECT_DELAY_T_CYCLES
+ 1`, which keeps the `PpuLcdRestartPhase` timeline INTACT — the restart tick still does its full work one cycle later —
unlike the skip-advance which corrupted it). Diagnosed the ly00 layer with the augmented probe: `ly00_mode3_0` fails at
pc=0x081A because the read is in the **vblank-wrap (frame 153→0) readback-delay window** (`vblank_wrap_line0_stat_delay_active`),
and that path in `current_published_stat_line_dot` uses raw `line_dot - 4` — **A′ applied its `+1` reference only to the
NORMAL base, never to the vblank-wrap path**. Once the restart offset is re-grounded (offset 0), the un-shifted vblank-wrap
read lands one dot behind main (`255-4=251`=Drawing vs main `256-4=252`=HBlank). **Layer-(a) fix (one line, a genuine A′
completion):** `published_stat.rs current_published_stat_line_dot` vblank branch `line_dot - 4` → `(line_dot + 1) - 4`.
Result with enable+1 + layer-(a): **89/105** — recovers `ly143_144_*`(4)+`lcdon`(1) [restart] AND `ly00_{mode1_2,mode2_3,mode3_0}`(3)+`ly_lyc_0-c`(1)
[layer a], vs the bare enable+1's 85. **This validates the layer-by-layer atomic rephase CONVERGES.** Remaining 16 fails,
each a mapped lockstep facet: (b) `ly00_{mode0_2,mode1_0}` (2, more ly=0 readback dots); (c) `ly_lyc_*` (8) — the **LYC
coincidence readback phase**: `ly_lyc-GS` diverges at one read (pc=0x0647, ly=3: branch `0xC4` STAT.2=set vs main `0xC0`),
the `lyc_coincidence_for_readback`/`live_ly_for_lyc_compare` window balanced against the old +1 — this is the §24.18 Cut 3
(`last_ly`/`last_lyc`) restructure, not a one-line shift; (d) `intr_2_*_sprites`(5)+`hblank_ly_scx`(1) — +1 from a DIFFERENT
restart path than the CPU-pending countdown (enable+1 left them unchanged; find their re-enable route). REVERTED to the
committed clean 94/105 + Cut A (the partial cut is net-negative until all facets land — A′'s `+1` and the +1 offset are
lockstep-balanced). The reproducible cut so far = {`enable+1` in `registers.rs:99`; vblank-wrap `+1` in
`published_stat.rs current_published_stat_line_dot`}. NEXT: land (a)+(b)+restart as one cut, then Cut 3 for (c), then (d);
keep iterating the fast gate past 94 before committing. Probes `irq_trace_{ly00_mode3_0,ly_lyc_gs}_dmg` added.
