# PPU Hardening — Hardware-True Fetcher Convergence (working doc)

> **STATUS: TEMPORARY.** This is a working campaign doc for removing the three Mode 3 / Mode 0
> compensation seams recorded in [`docs/TODO.md`](../TODO.md) lines 20–23 (tags `[PPU][MODE3-FETCHER-LEAD]`,
> `[PPU][MODE3-OBSERVATION-TABLES]`, `[PPU][MODE0-PUBLISH-HALT-GRID]`). Delete it at **M5 close-out**;
> the durable record stays in `docs/TODO.md` and [`docs/hardware/PPU-REIMPLEMENTATION.md`](../hardware/PPU-REIMPLEMENTATION.md).
> Oracles: SameBoy (`$HOME/workspace/SameBoy`, read-only) and DocBoy (`$HOME/workspace/docboy`, modifiable for
> instrumentation, revert when done). Agent-agnostic (Claude + Codex).

## Verification status (anchors checked against source before this doc was written)

Confirmed **exactly** against the current tree:

- `published_stat.rs:28` (`line_dot.checked_sub(1)` — the structural `−1` publish lag), `:61-72` (steady Mode 0
  boundary override gated `scx == 0 || Mode0-IRQ`), `:74-84` (mode2→3 override at `line_dot == MODE2_DOTS`).
- `irq.rs:466-473` (`mode0_hblank_halt_wake_deferred`, `(cgb-family || scx&7 ∈ {1,2,5,6}) && Mode0-IRQ && lcd-enabled
  && ly<144 && line_dot < mode0_start && line_dot+4 >= mode0_start`).
- `mode3/core.rs:48` (SCX capture), `:143-146` (unconditional `advance_bg_fetcher`), `:93-97`
  (co-advance during obj alignment stall).
- `ppu.rs:85-96` constants (`MODE2_DOTS=80`, `MODE3_BASELINE_DOTS=172`, `MODE3_BG_FETCH_PRIMING_DOTS=12`,
  `MODE3_INITIAL_SCX_CAPTURE_DOT=3` → capture at dot 83, `MODE0_START_DOT=252`, `OBJ_FETCH_MAX_ALIGNMENT_STALL_DOTS=5`).
- Accessor `cpu_visible_stat_mode()` at `api.rs:1509`.
- **DocBoy `pixel_transfer_dummy_lx0` (`ppu.cpp:938`)**: mode3 enters at `dots==80` (**same origin as gb-cycle**),
  3 dummy dots, at `dots==83` `bg_fifo.fill()` (junk) + `to_discard = scx % 8` + start the real fetcher; the first
  real tile is **discarded**. The Mode 3 length formula (`80 + 3 + 8 + 20·8 + scx%8 + 6·win + (WX0&SCX>0?1) + 6..11·sprites`)
  is confirmed for recomputing `MODE0_START_DOT`.

**Open nuance — exact lead magnitude (4 vs 7 dots).** The anchor is `line_dot = SameBoy_dot − 3`, so SameBoy
`cycles_for_line==94` maps to gb-cycle `line_dot 91`, and DocBoy gives `80+3+8 = 91` for the first *visible* tile.
Our current first-visible step is `line_dot 87` ⇒ the real lead may be **~4 dots**, not 7 (the historical "~7" likely
compared a SameBoy dot to a gb-cycle dot without the `−3`). This does not change the plan: **M0 instrumentation runs
first and its gate is to reproduce the measured delta; if it does not reproduce, re-ground before editing.**

## M0 results — measured 2026-06-13 (lead resolved to ~4 dots)

DocBoy instrumented (nogui, DMG, `#ifdef GBCYCLE_FETCH_TRACE` in `ppu.cpp`, built in `build-trace/`) and
traced on `m3_lcdc_bg_map_change.gb` ly=0 (steady SCX=0 BG line). gb-cycle side from the authoritative
`startup_core` unit test `mode3_startup_keeps_dummy_occupancy_out_of_the_fifo_until_alignment_push`.

DocBoy steady SCX=0 schedule (empirical):
```
dots 80,81,82  DUMMY  (3 dummy dots; fetcher FSM not running)
dot  83        BG_GETTILE0 bwf.lx=0   (first/discarded tile)
dot  85 / 87   LOW0 / HIGH0 bwf.lx=0
dot  88-90     PUSH bwf.lx=8
dot  91        LX8_OUT lx=8  +  BG_GETTILE0 bwf.lx=8   (first visible pixel AND first visible tile-index read coincide)
dot  99,107…   BG_GETTILE0  (clean 8-dot cadence)
```

| Event | gb-cycle | DocBoy (HW-true) | Δ |
|---|---|---|---|
| 3 dummy dots before real fetch | **no** (fetcher pre-armed at dot 80) | yes (80–82) | — |
| first (discarded) tile GetTile0 | 80 | 83 | −3 |
| **second tile (first visible) GetTile0** | **87** | **91** | **−4 (the lead)** |
| first visible pixel output | ~92 | 91 | +1 |
| mode0 boundary (SCX=0) | 252 | 251 | +1 |

**Conclusions (the lead is ~4 dots, not 7):**
- Our fetcher skips the 3 dummy dots (pre-armed at dot 80) and runs an irregular 7-dot seed→second-tile gap
  (80→87) instead of DocBoy's clean 8-dot cadence (83→91).
- **Our output and mode0 boundary are already ~1 dot late (≈ hardware).** Therefore M1 must move the fetcher's
  register-sampling point ~4 dots later **without moving the output / boundary** — otherwise
  wilbertpol/mooneye/shootout (which pin the output/boundary timing) regress. This is the central M1 risk.
- DocBoy shares gb-cycle's dot origin (mode3 enters at dot 80 in both) → DocBoy is the primary numeric oracle.

Artifacts: DocBoy trace build `build-trace/docboy-nogui` (revert DocBoy edits at M5); gb-cycle probe
`crates/gb-core/examples/ppu_fetch_trace.rs` (real-ROM spot-checks; steady-line ground truth lives in the
`startup_core` unit tests). M0 gate met: no gb-cycle production code changed, baselines inherited green from HEAD.

---

# Hardware-True Fetcher Convergence Plan: Removing the Three Mode 3 / Mode 0 Compensation Seams

## 0. Diagnosis — one causal chain, not three bugs

The three seams in `docs/TODO.md` lines 20–22 are not independent. They are one root defect and two layers of compensation built on top of it.

**Root: the BG fetcher free-runs ~4–7 dots ahead of hardware.**
`start_line()` pre-arms the fetcher to `TileIndex/fetch_x=0` at `line_dot 80` (`state.rs:1703`), and `advance_mode3_pipeline` then steps it **unconditionally** every visible mode3 dot (`crates/gb-core/src/ppu/mode3/core.rs:143-146`). The priming/entry-delay budget (`startup_source_state = EntryDelay{4}`) gates only the *transfer/output* lane, never the fetcher (`core.rs:171-180`). So the deterministic schedule is: seed tile-index read at dot 80 → seed completes at dot 85 → one restart-idle dot at 86 (`bg_push.rs:170-174`) → the first **visible** tile (`fetch_x=8`) `TileIndex.0` step is entered at **`line_dot 87`**. DocBoy starts the *first discarded* tile at `dots==83` and the first *visible* tile ~8 dots later (≈ dot 91); SameBoy reaches the equivalent at `cycles_for_line ≈ 94` (= gb-cycle 91 after `−3`). The hardware-true target for the visible `fx=8` step is therefore **≈ dot 91 in gb-cycle coordinates** (M0 confirms the exact value).

**Layer 1 (tables exist *because* of the lead).** Because pixels are produced from a fetcher that is several dots early, any register write that lands mid-line on a sprite line is observed at the wrong fetch phase. The six observation tables + window masks in `helpers/mode3_policies.rs` and `mode3/window.rs` are a per-`sprite_x`/`scx`/`scy` correction surface that re-derives "what the correct pixel *would* have been if the fetcher were on the hardware dot." PR #245 refit these to the post-#245 schedule rather than deriving them from a corrected fetcher. They are pinned by the mealybug `m3_*` blobs (both DMG and CGB-DMG-software variants). **They cannot be deleted until the lead is gone**, or every mealybug `m3_*` ROM regresses.

**Layer 2 (overrides + deferral exist *because* the published grid is off).** The published-STAT mode is structurally sampled one dot behind the raster (`published_stat.rs:28`, `line_dot − 1`). The dot-80 mode2→3 override and the steady-frame Mode 0 boundary override (`published_stat.rs:61-72`) cancel that lag at the two boundaries that matter. The Mode 0 halt-wake deferral table (`irq.rs:466-473`) defers a HALTed CPU's wake by up to 4 dots to reconcile the **halt-dispatch grid** (when a HALTed CPU services a STAT IRQ) with the **nop-dispatch grid** (when a running CPU's mid-line STAT read observes the boundary). The measured offset is 1–4 T-cycles and rides on the *dynamic* `current_mode0_start_dot()` — i.e. on PR #245's frozen sprite penalty, which itself is calibrated around the lead. So the grid offset is downstream of the same lead.

**Therefore the ordering is forced:** fix the lead first; the tables and the grid overrides are both compensations that become *removable* only once the fetcher lands on the hardware dot. Removing them before the lead is fixed would require re-deriving the very curve fit we are trying to delete.

The mapping that makes this tractable (cross-checked against both oracles):

| Counter | mode3 entry | first (discarded) tile fetch | first *visible* tile-index read |
|---|---|---|---|
| **gb-cycle `line_dot`** | 80 | 80 (seed) | **87** (current) → target ≈ 91 |
| **DocBoy `dots`** | 80 | 83 | ≈ 91 |
| **SameBoy `cycles_for_line`** | 84 (STAT pub) / mode2-tail | 89 (`mode_3_start`) | ≈ 94 |
| **anchor** | — | — | `line_dot = SameBoy_dot − 3`; `line_dot == DocBoy dots` (same origin) |

DocBoy and gb-cycle share the line origin (both start mode3 at dot 80, no offset). **DocBoy is therefore the primary numeric oracle** (directly comparable, and modifiable for instrumentation). SameBoy is the secondary cross-check (subtract 3). This is the single most important leverage point: the missing piece is a **dot-by-dot ground-truth fetch table**, and DocBoy can emit it directly.

---

## 1. Recommended ordering and atomicity

**Seam 1 (MODE3-FETCHER-LEAD) first — it is the root.** Everything else is compensation layered on it.

**Then Seam 2 (OBSERVATION-TABLES) — directly unblocked by Seam 1.** Once the fetcher lands on the hardware dot, mid-line writes resolve at the *actual* fetch dot and the tables collapse to direct fetcher arbitration. This seam is the largest LOC removal and the strongest regression surface (mealybug DMG + CGB-DMG-software), so it gets its own milestone after the lead is proven dot-exact.

**Seam 3 (MODE0-PUBLISH-HALT-GRID) last, and partly atomic with Seam 1.** Two sub-parts:
- **3a — the published-STAT boundary overrides** (`published_stat.rs:61-72`, `:74-84`) compensate the `−1` publication lag against the *raster* boundary. The raster boundary (`current_mode0_start_dot()`) moves when the lead is removed (Seam 1 changes the fetch schedule and therefore `MODE0_START_DOT` accounting). So 3a must be **re-derived in the same change as Seam 1's boundary recomputation** — they are atomic w.r.t. the mode0 boundary value. Do not leave the dot-80/mode0 overrides keyed to stale constants after Seam 1.
- **3b — the halt-wake deferral table** (`irq.rs:466-473`, `:247-263`) reconciles the halt vs nop dispatch grids. **HARD CONSTRAINT (from the campaign notes):** the variant-halt path that passes today *compensates the grid offset using the frozen #245 sprite penalty*. Fixing the grid must keep those penalties frozen and must **NOT refit them**. 3b is therefore done last, after 3a's boundary is dot-exact, by replacing the per-SCX deferral table with a uniform halt-wake dispatch convention + recalibrated apertures.

**Atomicity summary:**
- Seam 1 ⊗ Seam 3a (mode0 boundary recomputation): atomic — same PR.
- Seam 3b ⊗ frozen #245 sprite penalty: must stay frozen — never refit in 3b.
- Seam 2: strictly after Seam 1 lands green; independent PR(s), one table family at a time.

**Guardrail-doc coupling (HARD CONSTRAINT):** `docs/hardware/PPU-REIMPLEMENTATION.md` currently *preserves* these seams — line 29 ("Keep the steady-state Mode 0 HALT wake deferral…"), line 57 ("Keep sprite-phased live-write hypotheses declarative through observed policy tables…"), line 28 (LCD re-enable HALT aperture). Each guardrail must be rewritten **in the same change** that removes the seam it protects, or the next agent will treat the removal as a regression and revert it. Flag this in every PR description and in `docs/TODO.md`.

---

## 2. Instrumentation step (do this FIRST, before touching any production code)

The team's stated missing piece is a per-dot ground-truth fetch table. Build it from both oracles so the lead fix is verified dot-by-dot, not by ROM pass/fail alone.

### 2a. DocBoy instrumentation (primary — modifiable, same origin as gb-cycle)

Per `feedback_docboy_modifications`, modify freely and revert when done. DocBoy's per-dot fetcher FSM is the closest hardware-true reference and shares the `dots` origin.

- In `src/docboy/docboy/ppu/ppu.cpp`, add a compile-time-gated (`#ifdef GBCYCLE_FETCH_TRACE`) one-line emit at the **entry** of each fetcher state and at SCX capture:
  - `bgwin_prefetcher_get_tile_0` (`ppu.cpp:1946`): emit `ly, dots, lx (bwf.lx), tilemap_addr, "BG_GETTILE0"`.
  - `bg_pixel_slice_fetcher_get_tile_data_low_0` (`ppu.cpp:1992`): emit `dots, tile_data_addr_low`.
  - `bg_pixel_slice_fetcher_get_tile_data_high_0` (`ppu.cpp:2022`): emit `dots, tile_data_addr_high`.
  - `bgwin_pixel_slice_fetcher_get_tile_data_high_1` (`ppu.cpp:2267`): emit `dots, bwf.lx (post +=8)`.
  - `bgwin_pixel_slice_fetcher_push` (`ppu.cpp:2280`): emit `dots, bg_fifo.is_empty(), "PUSH_ATTEMPT"`.
  - `pixel_transfer_dummy_lx0` (`ppu.cpp:938`): emit the dot the 3-dummy stall ends (83) and `to_discard = scx % 8`.
  - `pixel_transfer_lx8` first emit (`ppu.cpp:1056`): emit `dots` at the first **visible** pixel push (lx leaves the priming/discard region).
  - obj fetch launch in `bgwin_pixel_slice_fetcher_push` (`ppu.cpp:2340`): emit `dots, sprite x, "OBJ_START"`; `obj_..._high_1_and_merge` (`ppu.cpp:2427`): emit `dots, "OBJ_DONE"`.
- Drive it with the mealybug `m3_*` ROMs and the mooneye/wilbertpol `intr_2_mode*_timing` ROMs to produce, **per line and per SCX&7 value**, the canonical table: `(event, dots)` for tile-index/data-low/data-high/first-visible-push/SCX-capture/obj-start/obj-done.
- **Deliverable: `docboy_fetch_dots.csv`** (kept out of the repo; an artifact for the campaign). Columns: `rom, ly, scx_low3, event, dots`. This is the oracle the gb-cycle probe is diffed against.
- Revert all DocBoy edits when the campaign closes (per memory note).

### 2b. SameBoy cross-check (secondary — read-only, subtract 3)

SameBoy is read-only. Do not patch it; instead capture from its existing structure:
- `advance_fetcher_state_machine` (`Core/display.c:916-1107`) and the `mode_3_start:` label (`display.c:1845-1855`). The first `GET_TILE_T1` address compute is at `cycles_for_line==89` (`display.c:949`). The `+4` STAT lag is documented at `display.c:1529-1532`.
- Use SameBoy's existing debugger/symbol output or a minimal `printf` in a *local throwaway build* (not committed) at `display.c:949` (`GET_TILE_T1`), `:957` (`GET_TILE_T2` read), `:996` (low read), `:1046` (high read), `:1084` (PUSH gate) emitting `cycles_for_line`. Convert to gb-cycle dots via `line_dot = cycles_for_line − 3`.
- Cross-check that DocBoy `dots == SameBoy cycles_for_line − 3` holds at each event. Where they disagree, **DocBoy wins** for gb-cycle (same origin) but record the divergence — it usually localizes to the +4 STAT-publication lag vs the internal raster, which matters for Seam 3.

### 2c. gb-cycle probe harness

Extend `crates/gb-core/examples/g3_sprite_grid.rs` (the sanctioned fast-iteration harness) to dump the gb-cycle equivalents:
- Hook the fetcher dispatch in `mode3/bg_fetch.rs` (`advance_bg_fetcher_automaton_step`, `bg_fetch.rs:72-94`) behind an example-only trace flag to print `(line_dot, stage, fetch_x)` at each step entry, plus `capture_initial_scx` firing dot (`core.rs:48`).
- Use `cpu_visible_stat_mode()` (`ppu/api.rs:1509`) to sample the published STAT boundary dot per line for the Seam 3 grid checks.
- **Deliverable: `gbcycle_fetch_dots.csv`** with identical columns to 2a. The diff `gbcycle − docboy` per event is the seam, dot by dot. The MODE3-FETCHER-LEAD target is **diff == 0 at every event** for the steady line, and for sprite/SCX/window lines after Seam 1.

> Gate for proceeding to code changes: the two CSVs exist, and the current diff reproduces the documented lead at the first visible tile-index event (gb-cycle 87 vs DocBoy ≈ 91). If it does not reproduce, the model has drifted from the analysis and the plan must be re-grounded before editing.

---

## 3. Seam 1 — MODE3-FETCHER-LEAD

### Current mechanism (file:line)
- Fetcher pre-armed at entry: `start_line()` calls `fetcher.start_background()` setting `stage=TileIndex, fetch_x=0` at `line_dot 80` — `crates/gb-core/src/ppu/state.rs:1703` (invoked from `mode3/core.rs:43`).
- Fetcher stepped unconditionally every visible mode3 dot: `mode3/core.rs:143-146` (`advance_bg_fetcher`), confirmed not gated by the priming window.
- Co-advance during obj alignment stall: `mode3/core.rs:95-97`.
- Priming/transfer budget is a **separate** machine that never gates the fetcher: `startup_source_state` seeded `EntryDelay{4}` at `state.rs:1693-1695`, consumed only on the output path at `core.rs:171-180` / `state.rs:1778-1802`.
- Raw-dot constants that *should* hold the fetcher off but don't: `MODE3_BG_FETCH_PRIMING_DOTS=12`, `MODE3_PRE_VISIBLE_OBJ_MATCH_START_DOT=4`, `MODE3_INITIAL_SCX_CAPTURE_DOT=3`, `MODE3_FIFO_BACKED_HIDDEN_TRANSFER_START_DOT=8`, `MODE3_ABSTRACT_SOURCE_WINDOW_DOTS=8`, `MODE3_ABSTRACT_PREVISIBLE_TRANSFER_DOTS=4`, `MODE0_START_DOT=252`, `MODE3_BASELINE_DOTS=172` — all `crates/gb-core/src/ppu.rs:86-96` (verified).
- SCX capture pinned to raw `line_dot 83`: `mode3/core.rs:48`; body at `state.rs:1706-1715` (adds discard to `mode0_start_dot`).
- The PostAlignment `should_delay_background_tileindex_read` + 1-dot restart idle that pins the visible-tile landing: `bg_push.rs:170-174` (restart delay = 1), `bg_fetch.rs:46-62` (consume idle), `mode3_policies.rs:418-423` + `state.rs:2340-2349` (delayed read).

### Hardware-true target (DocBoy `ppu.cpp:938-972`, SameBoy `display.c:1845-1855`)
DocBoy is explicit and authoritative because it shares gb-cycle's origin:
1. **3 fixed dummy dots** at dots 80, 81, 82 during which the **real fetcher FSM does NOT run** (`tick_fetcher` is only called from the discard/lx0/lx8 states). DocBoy `pixel_transfer_dummy_lx0` (`ppu.cpp:938-972`).
2. At **dot 83**: fill BG FIFO with junk (`bg_fifo.fill()`), capture `to_discard = scx % 8`, then start the real fetcher. So the **first real BG tile fetch begins at dot 83**, not dot 80.
3. The first real tile is **also discarded** (it primes the FIFO); the first *visible* tile's `GetTile0` lands ~8 dots later (≈ dot 91), matching SameBoy ≈ 94 (= gb-cycle 91 after `−3`).

SameBoy reaches the same state differently (8 junk pixels pushed into the FIFO at `mode_3_start`, fetcher free-runs but its PUSH is blocked until junk drains — `display.c:1848-1851, 1084`) but the *net* first-visible-tile dot agrees with DocBoy.

**Computed delta to remove: the fetcher must start ~3 dots later (the dummy stall) AND not pre-fill the visible-tile pipeline early — net visible `fx=8` tile-index read must move from `line_dot 87` to `≈ 91` (M0 fixes the exact value).** The cleanest structural realization (per the analysis recommendation): **gate the fetcher start the same way the transfer is gated** — fold the 3-dummy stall + first-discarded-tile into the fetcher's own startup, and re-express the priming/transfer schedule as **fetch-step counters anchored to the seed/dummy slice** rather than absolute `line_dot` literals.

### Concrete code changes
1. **Insert the 3-dot dummy stall before the fetcher runs.** In `mode3/core.rs:143-146`, gate the unconditional `advance_bg_fetcher` behind a startup dummy-dot counter so the FSM does not step on `line_dot ∈ {80,81,82}`. Mirror DocBoy `pixel_transfer_dummy_lx0`: introduce `bg_fetch_dummy_dots_remaining` in `BgPipelineState`, seeded `=3` in `start_line()` (`state.rs:1680-1704`), decremented each visible dot until 0, fetcher held idle while > 0.
2. **Stop pre-arming the visible fetch at dot 80.** Keep `fetcher.start_background()` (`state.rs:1703`) but make the first real tile a **discarded prime tile** that only begins at dot 83. Replace the current seed/post-alignment machinery (`begin_post_alignment_followup`, `state.rs:2352-2361`; the `AlignmentSeedPending`→`PostAlignment` seam) with DocBoy's model: one junk FIFO fill at dot 83 + one discarded real tile, then steady cadence. This removes the lead rather than re-tuning it.
3. **Re-anchor SCX capture to the dummy-stall end, not raw dot 83-as-literal.** Change `mode3/core.rs:48` from the raw equality `line_dot == MODE2_DOTS + MODE3_INITIAL_SCX_CAPTURE_DOT` to fire when the dummy stall ends (fetch-step relative). The *value* (dot 83) is unchanged but it now tracks the fetcher, satisfying the "must become fetch-step-relative" requirement. Keep `state.rs:1713`'s `mode0_start_dot += discard` accounting.
4. **Re-express `ppu.rs:86-96` raw-dot constants as fetch-step counters.** Replace `MODE3_BG_FETCH_PRIMING_DOTS=12` and its `−8/−4/−1` derivatives with: `BG_FETCH_DUMMY_DOTS=3`, `BG_FETCH_FIRST_DISCARDED_TILE_DOTS=8`, and derive the transfer/SCX-capture anchors from those. Recompute `MODE0_START_DOT` from the **emergent** schedule: DocBoy's identity `dots == 80 + 3 + 8 + 160 + (SCX%8) + 6·win_triggers + (WX0&SCX>0?1:0) + (6..11)·sprites` (`ppu.cpp:1586-1626`). For the trivial steady line this is `80 + 3 + 8 + 160 = 251 + SCX%8` — cross-check against SameBoy's `mode3_batching_length` trivial case `167 + SCX&7` (`display.c:1471-1506`) and against the current `MODE0_START_DOT=252` baseline (the 251-vs-252 gap is exactly what M0 resolves). **This is where Seam 3a becomes atomic** (the boundary value the overrides key off changes here).
5. **Remove the PostAlignment 1-dot restart idle** (`bg_push.rs:170-174`, `bg_fetch.rs:46-62`) and the `should_delay_background_tileindex_read` PostAlignment branch (`mode3_policies.rs:418-423`, `state.rs:2340-2349`) — these only exist to land the lead at dot 87. With the dummy-stall + discarded-tile model they are obsolete.

### What gets DELETED in Seam 1
- The `AlignmentSeedPending` / `PostAlignment` seam machinery: `begin_post_alignment_followup` (`state.rs:2352-2361`), the post-alignment continuation budget retirement (`advance_startup_background_fetch_tile`, `state.rs:2381-2406`), the restart-delay literal (`bg_push.rs:170-174`), the idle consumer (`bg_fetch.rs:46-62`).
- The `should_delay_background_tileindex_read` PostAlignment path (`mode3_policies.rs:418-423`).
- The raw-dot priming derivatives in `ppu.rs:86-96` (replaced by fetch-step counters).

### Sub-tasks (sequenced)
1. Build the instrumentation CSVs (§2) and confirm the lead reproduces.
2. Add `bg_fetch_dummy_dots_remaining` + gate the fetcher (changes 1–2). Run probe; expect first-visible tile-index to move toward ≈ 91. Iterate until `gbcycle − docboy == 0` for the **steady, no-sprite, SCX&7==0** line.
3. Re-anchor SCX capture (change 3). Re-run probe across all SCX&7 ∈ {0..7}; expect diff 0 at SCX-capture and first-visible-push for every SCX phase.
4. Recompute `MODE0_START_DOT` / constants (change 4) and **simultaneously** re-derive Seam 3a overrides (see §5). Probe the Mode 0 boundary dot via `cpu_visible_stat_mode()`.
5. Delete the obsolete seam machinery (change 5). Re-run full probe.

### Verification gate at each sub-task
- Probe: `gbcycle_fetch_dots.csv` vs `docboy_fetch_dots.csv`, diff == 0 at every event for the line classes covered so far.
- ROM gates after each sub-task that compiles green: `cargo fmt-check`, `cargo lint`, `cargo tests`, `cargo rom-report blargg`, then the full green-baseline set (§7).
- **Critical**: at sub-task 2–4 the observation tables (Seam 2) are STILL PRESENT and were refit to the *old* schedule. Moving the fetcher will make them resolve at the new (correct) dot — this **will** perturb the mealybug `m3_*` ROMs. Expect them to **wobble** here; that is the signal that Seam 2 is now derivable. Do not refit the tables to chase green at this stage. Instead, gate Seam 1 "done" on the **probe diff == 0** plus wilbertpol/mooneye/blargg/shootout green, and accept that mealybug `m3_*` may go temporarily red — they are repaired by Seam 2. (If the team requires every ROM green at every commit, land Seam 1 + Seam 2 as a single squashed branch with intermediate commits, and only run the full mealybug gate at the branch tip.)

### Regression risks + early detection
- **Risk: the dummy-stall changes mode3 length, shifting `MODE0_START_DOT` and breaking mooneye `intr_2_mode3_timing` / `intr_2_oam_ok_timing`.** Detect early: after change 4, the `cpu_visible_stat_mode()` probe must show the Drawing→HBlank seam at the recomputed boundary for SCX&7 ∈ {0..7}; run those two mooneye ROMs immediately.
- **Risk: the co-advance during obj alignment stall (`core.rs:95-97`) double-counts now that the fetcher starts later.** Detect: probe `OBJ_START`/`OBJ_DONE` dots vs DocBoy on the sprite ROMs; the frozen #245 per-sprite cost (6 min, 11 max; `OBJ_FETCH_MAX_ALIGNMENT_STALL_DOTS=5 + 6`) must remain numerically identical to DocBoy's 6..11. If sprite penalty drifts, STOP — that violates the freeze constraint.
- **Risk: SCX&7 discard interacts with the new junk-fill.** Detect: the WX0&SCX7 special case (DocBoy `pixel_transfer_discard_lx0_wx0_scx7`, `ppu.cpp:1001-1015`) is a known edge; run `m3_wx_*` and `m3_window_timing_wx_0` against the probe specifically for SCX&7==7.

---

## 4. Seam 2 — MODE3-OBSERVATION-TABLES

> Prerequisite: Seam 1 landed, probe diff == 0 for steady/SCX/sprite/window lines.

### Current mechanism (file:line)
Six tables + window masks, all in `crates/gb-core/src/ppu/helpers/mode3_policies.rs` and `crates/gb-core/src/ppu/mode3/window.rs`:
- `PpuMode3ObservedScyObjPhaseTable` (`mode3_policies.rs:866-1023`) — master SCY/OBJ phase table; consumed via `PpuMode3ScyObjPhasePolicy` (`:1027-1126`), routed in `api.rs:688-763` / `transfer.rs:329-583`.
- `PpuMode3ObservedLcdc3PhaseTable` (`mode3_policies.rs:1255-1353`) — LCDC.3 BG-map; `control/live_writes.rs:80-103`.
- `PpuMode3ObservedLcdc4PhaseTable` (`mode3_policies.rs:1361-1490`) — LCDC.4 tile-data-sel; `control/live_writes.rs:160-178`.
- `PpuMode3ObservedLcdc0OnsetTable` (`mode3_policies.rs:1184-1228`) — LCDC.0 BG-enable onset; `control/live_writes.rs:259-280`.
- LCDC.1 disable-onset arrays (`mode3_policies.rs:1151-1169`) — `control/live_writes.rs:298-305`.
- `PpuMode3ObservedLcdc2ObjSizePhaseTable` (`mode3_policies.rs:1522-1659`) — LCDC.2 16→8 shrink; `helpers/mode3_lcdc2_obj_size.rs:20-32` / `transfer.rs:127`.
- Window masks: six const tables + reverse helpers in `mode3/window.rs:1199-1928`, consumed at pixel emit in `transfer.rs:244-249`.
- SCX coarse-bit gate: `bg_scx_tilemap_column_changed` (`helpers/mode3_latches.rs:179-181`), SCY companions `:183-194`; downstream SCX boundary routines `api.rs:455-655`.

### Hardware-true target (SameBoy + DocBoy: read registers LIVE per fetch sub-step)
Both oracles resolve mid-line writes **by reading the register live inside the fetcher sub-step that uses it**, not by a phase table:
- LCDC.3 BG-map base: SameBoy reads live in `GET_TILE_T1` (`display.c:927-932`); DocBoy in `setup_bg_pixel_slice_fetcher_tilemap_tile_address` (`ppu.cpp:2548-2555`).
- LCDC.4 tile-data-sel: SameBoy re-latches in both `GET_TILE_DATA_LOWER_T1` and `..._HIGH_T1` (`display.c:976, 1025`); DocBoy recomputes the address in low_0/high_0 (`ppu.cpp:1992-2037`). The mid-fetch desync (low/high bitplanes from different selects) is **emergent** from recomputing per byte.
- SCY: SameBoy `fetcher_y()` per `T1` (`display.c:819-822`); DocBoy `ly+scy` in GetTile0. The DMG/CGB-C-vs-CGB-D split (DMG re-reads SCY per byte, CGB-D caches) is at SameBoy `display.c:945-948` and DocBoy `ppu.cpp:1975-1982` — model it as a per-byte vs cached live read, not a table.
- LCDC.2 OBJ-size: read live in the obj tile-data address setup (DocBoy `ppu.cpp:2402-2418`; SameBoy `get_object_line_address` `display.c:1111`).
- Window activation / LCDC.4 window toggle: the per-pixel previous-vs-current bitplane mix is **emergent** from when the fetch sub-step lands relative to the window-activation dot — once the fetcher is on the hardware dot, the window masks collapse to "read the live select at the actual fetch dot."
- The 1-T-cycle (DMG) / 2-T-cycle (CGB) **register-write observation latency** that the tables encode as per-`x`/`scx` phase offsets is, in the oracles, a uniform write-pending delay: DocBoy `last_lcdc/last_wx/last_bgp` snapshot at end of `tick()` (`ppu.cpp:505-540`, DMG) and `pending_write` countdown of 2 (`ppu.cpp:683-699`, CGB). **This is the correct replacement for the phase tables**: a single console-keyed write-latency, applied before the live read, not a per-sprite-x column.

### Concrete code changes
Replace each table's consumption site with a **live read at the actual fetch dot**, plus a **uniform write-observation latency**:
1. Introduce a console-keyed register-write latency in the latch layer (`helpers/mode3_latches.rs`): DMG = 1 dot, CGB = 2 dots (mirroring DocBoy `ppu.cpp:505-540` / `:683-699`). Apply it to LCDC/SCX/SCY/WX before they reach the fetcher's live read. This subsumes the per-`x`/`scx` phase offsets.
2. **LCDC.3**: in the BG fetcher `TileIndex.0` (`bg_fetch.rs:107-189`, `compute_fetch_tile_index_address`), read the (latency-adjusted) live LCDC.3 to pick the tilemap base. Delete `apply_dmg_lcdc3_live_bg_tilemap_write` (`live_writes.rs:80-103`) and `PpuMode3ObservedLcdc3PhaseTable` (`mode3_policies.rs:1255-1353`).
3. **LCDC.4**: read live LCDC.4 in both `TileDataLow.0` and `TileDataHigh.0` (`bg_fetch.rs:211-305`), letting the low/high desync emerge. Delete `apply_dmg_lcdc4_live_bg_tiledata_write` (`live_writes.rs:160-178`) and `PpuMode3ObservedLcdc4PhaseTable` (`mode3_policies.rs:1361-1490`).
4. **SCY**: read live SCY per `T1` (DMG/CGB-C) or cached at GetTile0 (CGB-D+) in the fetcher. Delete `PpuMode3ObservedScyObjPhaseTable` (`mode3_policies.rs:866-1023`), its policy wrapper (`:1027-1126`), and the SCY routing carriers `PpuMode3LiveScyWriteRouting`/`PpuMode3CgbDmgLiveScyWriteRoute` (`:429-482`) plus the `transfer.rs:329-583` retarget/previous-row pixel helpers.
5. **LCDC.0 / LCDC.1 onsets**: replace the per-sprite-x onset arrays (`mode3_policies.rs:1184-1228`, `:1151-1169`) with the actual visible-x at which the live-read fetcher emits the toggled pixel. Delete the onset tables and `live_writes.rs:259-305`.
6. **LCDC.2 OBJ-size**: read live OBJ-size in the obj fetch tile-data setup. Delete `PpuMode3ObservedLcdc2ObjSizePhaseTable` (`mode3_policies.rs:1522-1659`) and `helpers/mode3_lcdc2_obj_size.rs` table construction.
7. **Window masks**: replace the six mask tables + helpers (`window.rs:1199-1928`) with live-select reads at the window fetch dots. Delete them.
8. **SCX coarse gating**: keep `bg_scx_tilemap_column_changed` (`mode3_latches.rs:179-181`) only if it still expresses a real coarse-vs-fine distinction after the live model; the per-SCX-value old-pixel/old-tail retargets in `api.rs:496-655` are curve fit — replace with live SCX read in the fetcher and delete the value-keyed bands.

### What gets DELETED in Seam 2
All six tables, the window mask tables, the SCY routing carriers, the LCDC0/1 onset arrays, and their consumption wrappers in `control/live_writes.rs`, `helpers/mode3_lcdc2_obj_size.rs`, and the `transfer.rs` per-pixel override helpers (`compute_startup_visible_tile2_*`, `compute_window_*`, `:329-583`). Plus the unit tests that pin the tables as tables (`tests/mode3/fetch.rs:221-650`, `tests/mode3/lcdc_obj_toggles/lcdc_obj_toggle_policy.rs:130-363`) — rewrite these to assert the live-read behavior instead of deleting coverage.

### Sub-tasks (one table family per step, each independently gated)
Order by regression surface, smallest first:
1. LCDC.0 onset (single ROM `m3_lcdc_bg_en_change`).
2. LCDC.1 onset (`m3_lcdc_obj_en_change[_variant]`).
3. LCDC.3 (`m3_lcdc_bg_map_change`).
4. LCDC.4 (`m3_lcdc_tile_sel_change`, `m3_lcdc_tile_sel_win_change`).
5. LCDC.2 obj-size (`m3_lcdc_obj_size_change[_scx]`).
6. Window masks (`m3_window_timing*`, `m3_lcdc_win_en_change_multiple*`, `m3_wx_*`).
7. SCY/OBJ phase table (`m3_scy_change`) — last, largest, and the `PpuMode3ScyObjPhasePolicy` is also referenced by `TODO.md:23`.

### Verification gate at each sub-task
- **Run `cargo rom-report mealybug-tearoom-tests`** (NOT just shootout — the CGB-compat fixtures are invisible to the shootout report; this is a HARD CONSTRAINT). Both the DMG suite and the CGB-DMG-software variant for the specific `m3_*` ROM must be green before deleting the next table.
- Probe diff == 0 for that write-type's fetch dots (extend §2's CSV to emit the live-read dot for the register under test).
- Full CI gates + green baselines (§7) at every commit.

### Regression risks + early detection
- **Risk: the DMG/CGB-C vs CGB-D SCY-desync split is subtle (per-byte vs cached).** Detect early: the CGB-DMG-software `m3_scy_change` blob is the canary — run it on CGB-compat *and* DMG; the split is at SameBoy `display.c:945-948`.
- **Risk: deleting window masks before the window fetch dot is dot-exact regresses CGB-C/D window captures.** Detect: probe window-activation fetch dots vs DocBoy `win_prefetcher_activating` (`ppu.cpp:2096-2112`) — the "first window dot is wasted" must reproduce before deleting masks.
- **Risk: the write-latency model is wrong (DMG 1 vs CGB 2).** Detect: a write-timing ROM will fail across *all* LCDC bits simultaneously rather than one — if multiple `m3_lcdc_*` ROMs regress together after change 1, the latency constant is wrong, not the per-bit live read.

---

## 5. Seam 3 — MODE0-PUBLISH-HALT-GRID

### Seam 3a — published-STAT boundary overrides (ATOMIC with Seam 1's boundary recompute)

**Current mechanism:** `published_stat.rs:28` (`line_dot − 1` global lag), `:61-72` (steady Mode 0 boundary override, gated `scx==0 || Mode0-IRQ`), `:74-84` (dot-80 mode2→3 override). Constants `MODE2_DOTS=80`, `MODE0_START_DOT=252` at `ppu.rs:85-96` (verified).

**Hardware-true target:** DocBoy publishes STAT mode with deliberate 1-dot pre-publish offsets that are *uniform*, not gated on SCX: PIXEL_TRANSFER published 1 dot early at dot 79 (`ppu.cpp:891-901`), OAM_SCAN 1 dot early at dot 455 (`ppu.cpp:1255-1271`), HBLANK exactly at the raster mode3 end (no early publish; `ppu.cpp:1642`). SameBoy: STAT mode bits "always late by 4 T-cycles" (`display.c:1529-1532`), mode2 IRQ fires 1 T-cycle before STAT changes except line 0 (`display.c:1778-1800`), mode0 published 1 T-cycle before the sleep (`display.c:2090-2108`).

**Concrete change (in the SAME PR as Seam 1 change 4):** once `current_mode0_start_dot()` is recomputed from the emergent fetch schedule, the `scx==0 || Mode0-IRQ` gate on the steady Mode 0 override (`published_stat.rs:71`) should no longer be necessary — the override existed to cancel the `−1` lag *at a boundary that was itself off by the lead*. Replace it with a **uniform publication-offset convention** matching DocBoy (publish HBlank exactly at the recomputed raster boundary; keep the dot-80→Drawing and dot-455→OAM 1-dot-early publishes, which are real hardware behavior). Verify the `−1` lag (`published_stat.rs:28`) is still the only structural offset and that the two boundary overrides reduce to the uniform convention.

**What gets DELETED:** the SCX/Mode0-IRQ gate clause at `published_stat.rs:71` and, if the uniform convention holds, the steady-frame `published_stat_steady_frame_mode0_boundary_override_applies` special case entirely (`:61-72`).

**Verification:** mooneye `intr_2_mode0_timing` (steady), `intr_2_mode3_timing`, `intr_2_oam_ok_timing`; unit `tests/stat/mode_edges.rs`. Probe the published boundary dot via `cpu_visible_stat_mode()` for SCX&7 ∈ {0..7}.

### Seam 3b — halt-wake deferral table (LAST; frozen #245 penalty MUST NOT be refit)

**Current mechanism:** `mode0_hblank_halt_wake_deferred` (`irq.rs:466-473`, verified): defers when `(CGB-family || SCX&7 ∈ {1,2,5,6}) && Mode0-IRQ-enabled && lcd-enabled && ly<144 && line_dot < mode0_start && line_dot+4 >= mode0_start`. LCD-reenable sibling `dmg_lcd_reenable_mode0_halt_wake_deferred` (`irq.rs:247-263`) with SCX-group-aligned aperture `((scx&7)+3)/4*4` (`irq.rs:238-241`). Dispatched from `machine/step.rs:1020-1028`.

**Root cause (per the analysis):** the 1–4 dot offset is the difference between the **halt-dispatch grid** and the **nop-dispatch grid**. SameBoy shows the mechanism: DMG advances HALT in **2-T-cycle** steps (`sm83_cpu.c:1625-1626, 1632`), CGB always **4** — this 2-vs-4 dispatch quantization is *exactly* the DMG `SCX&7 ∈ {1,2,5,6}` vs CGB-all-SCX split. The deferral table is a per-SCX patch over a coarse dispatch grid.

**Hardware-true target:** a **uniform halt-wake dispatch convention** — model the CPU halt-wake on the same dot grid as the running-CPU nop dispatch, with the console's native HALT advance quantization (DMG 2-cycle, CGB 4-cycle) applied *once* at the dispatch point, so the per-SCX deferral becomes emergent rather than tabulated.

**Concrete change:** In `machine/step.rs:1020-1028`, replace the `mode0_hblank_halt_wake_deferred` predicate with a halt-wake that resolves the STAT IRQ on the recalibrated Mode 0 pretrigger aperture using the native HALT advance step (mirror SameBoy `sm83_cpu.c:1625-1662`). Recalibrate the Mode 0 / Mode 2 pretrigger apertures (`irq.rs:159-160` mode0 pretrigger, `:285-286` hidden edge, `:471-472` the 4-dot aperture) so the same aperture serves both the running-CPU STAT read and the HALTed-CPU wake. **The frozen #245 sprite penalty (the dynamic `current_mode0_start_dot()`) is the input to the aperture and must not change.**

**What gets DELETED:** the `(CGB-family || SCX&7 ∈ {1,2,5,6})` table clause (`irq.rs:467`), and if the uniform convention holds, `mode0_hblank_halt_wake_deferred` collapses to the plain pretrigger aperture. The LCD-reenable SCX-group aperture (`irq.rs:238-263`) should likewise reduce to the same convention.

**Verification:** wilbertpol `intr_2_mode0_timing_sprites_nops` + the `scxN_nops` variants (post-LCD-enable frames — these pin the halt-dispatch grid AND ride on the frozen sprite penalty); mooneye `intr_2_mode0_timing` (steady); unit `tests/stat/mode_edges.rs:162-209` (the 4-dot aperture + DMG/CGB split), `:499-509`, `:544-559`; integration `tests/ppu/ppu_lcd_restart.rs:519-526` (the `0x62/0x63/0x64` SCX-group staircase).

**Regression risks + early detection:**
- **Risk (the named HARD CONSTRAINT): touching the halt grid silently refits the sprite penalty.** Detect: before and after 3b, dump the per-sprite mode3 cost via the probe; it must remain identical (6..11 per sprite, matching DocBoy `ppu.cpp:1573-1626`). If `intr_2_mode0_timing_sprites_nops` only passes after the sprite cost moved, the freeze is violated — revert.
- **Risk: steady (mooneye) vs post-LCD-enable (wilbertpol) frames diverge.** Detect: run both ROM families together at every 3b commit; they exercise different frame types and the uniform convention must serve both.
- **Risk: the DMG 2-cycle halt advance interacts with the CPU step loop.** Detect: unit `mode_edges.rs:200-208` asserts the exact `SCX ∈ {1,2,5,6}` split — if the emergent model produces a different SCX set, the dispatch quantization is mismodeled.

---

## 6. Guardrail-doc updates (HARD CONSTRAINT — same change as each removal)

Each seam removal must rewrite the matching guardrail in `docs/hardware/PPU-REIMPLEMENTATION.md` (verified line refs):
- **Seam 1**: no direct guardrail line, but add a note that the fetcher now starts via the 3-dummy-dot + first-discarded-tile convention (DocBoy-aligned) and that priming is fetch-step-relative.
- **Seam 2**: rewrite **line 57** ("Keep sprite-phased live-write hypotheses declarative through observed policy tables, not ad hoc imperative branches") → live register reads at the actual fetch dot with a uniform console write-latency; tables removed.
- **Seam 3a/3b**: rewrite **line 29** ("Keep the steady-state Mode 0 HALT wake deferral separate…") and **line 28** (LCD re-enable HALT aperture) → uniform halt-wake dispatch convention + recalibrated apertures.
- Update `docs/TODO.md:20-23` (strike the closed seams; the SCY-OBJ-PHASE-POLICY note at `:23` closes with Seam 2 step 7).

---

## 7. Green baselines that must hold at EVERY milestone (HARD CONSTRAINT)

Run before declaring any milestone done:
- `cargo fmt-check`, `cargo lint`, `cargo tests`, `cargo rom-report blargg` (CI gates).
- wilbertpol **117/117**, mooneye **113/113**, blargg **58/58**, gb-emulator-shootout **264/264**.
- `cargo rom-report mealybug-tearoom-tests` — **both** the DMG suite and the CGB-compat suite (the CGB-compat fixtures are invisible to the shootout report; this command is mandatory before declaring any mode3-timing change done).
- Clear suite caches before a full report: `rm -rf test/*/.status` (a `--case` run overwrites the suite status with only that case).

---

## 8. Milestone checklist

- **M0 — Instrumentation.** DocBoy fetch-trace (§2a) + SameBoy cross-check (§2b) + gb-cycle probe (§2c) produce `docboy_fetch_dots.csv` / `gbcycle_fetch_dots.csv`. Current diff reproduces the lead at first-visible tile-index (gb-cycle 87 vs DocBoy ≈ 91) **and resolves the 4-vs-7-dot ambiguity**. *Gate: all §7 baselines green (no production code changed yet); DocBoy edits behind `#ifdef`, to be reverted at campaign close.*

- **M1 — Lead removed on steady lines. ✅ IMPLEMENTED 2026-06-13.** Mechanism: a `startup_fetch_idle_dots` counter (= `MODE3_BG_FETCH_STARTUP_DUMMY_DOTS = 3`) seeded in `start_line` (state.rs) and consumed at the top of `advance_bg_fetcher` (mode3/bg_fetch.rs) before the special-stage dispatch. The 3 dummy idle dots consume the fetcher's slack: the first (discarded) tile-index read moves dot 80→83, the first VISIBLE tile read moves dot 87→~90-91 (matches DocBoy's 91; cadence tiles 2+ exact: 99/107/115), while **output (first visible pixel still dot 92) and the mode0 boundary (still 252) are unchanged**. Verified CPU-invisible: phase2 + phase4 machine-trace fixtures regenerated change ONLY `bg_*` fetcher-internal fields — `mode`/`ly`/`stat_irq_line`/`line_dot`/OAM-state/CPU-regs all identical. *Gate status: blargg 58/58, mooneye 113/113, wilbertpol 117/117 GREEN; gb-core unit+integration GREEN; fmt/lint clean. Expected Seam-2 wobble: exactly ONE ROM red — `m3_scy_change` (DMG in shootout 263/264, and CGB-compat 23/24). **DMG side CLOSED 2026-06-14** (startup-SCY latch now arms on the pending `fill`, not only the FIFO → shootout 264/264; see the m3_scy_change close-out note under M3). **CGB-compat side stays 23/24** — it is not a standalone startup-SCY bug but the M2 obj-stall-coupled sampling cluster (sprite-at-column-0 stalls the BG fetcher across the SCY change).*

- **M2 — Lead removed across SCX/sprite/window + Seam 3a.** Probe diff 0 for all SCX&7, sprite, and window line classes; `MODE0_START_DOT` recomputed from the emergent schedule; published-STAT overrides reduced to the uniform convention. *Gate: full §7 green INCLUDING `cargo rom-report mealybug-tearoom-tests` (DMG + CGB-compat) is the target — if mealybug is still red here, it is repaired in M3 and M1/M2/M3 land as one squashed branch. wilbertpol/mooneye/blargg/shootout green at every commit.*

- **M3 — Observation tables deleted.** Each table family (smallest regression surface first; SCY last) replaced by live reads + uniform write-latency; unit tests rewritten to assert live-read behavior. Guardrail doc line 57 updated. *Gate: full §7 green at every sub-step, with `cargo rom-report mealybug-tearoom-tests` (both suites) green before deleting the next table.*

  **M2 SCOPE FINDING (2026-06-13) — the tables are NOT redundant cleanup.** Experiment: neutralized `dmg_single_selected_sprite_phase_policy()` (live_writes.rs:363, the shared gate for the LCDC.0/1/3/4 observation tables) to fall back to the fetcher's live LCDC read. Result: 5 sprite-coupled mealybug ROMs regressed (`m3_lcdc_bg_en_change`, `m3_lcdc_bg_map_change`, `m3_lcdc_obj_en_change`, `m3_lcdc_obj_en_change_variant`, `m3_lcdc_tile_sel_change`) on BOTH DMG and CGB-compat. So these tables encode GENUINE sprite-coupled fetch-timing behavior: when a single selected sprite stalls the BG fetcher, the dot at which the BG fetcher samples LCDC/SCY relative to a mid-line write shifts, and the live-read path does NOT yet model that obj-stall-coupled register sampling. **Implication:** removing any observation table hardware-truly requires the BG fetcher's live register reads to land at the hardware-correct dots EVEN DURING obj-fetch stalls (the OBJ/BG arbitration timing — tied to the PR #245 frozen sprite-penalty model). M2 is therefore a per-table fetcher-modeling effort (DocBoy-trace-driven, verified per mealybug ROM), NOT mechanical table deletion. The tables remain in place and green (except `m3_scy_change`). Reverted the experiment; mealybug back to 23/24.

  **M2/M3 SCY investigation (2026-06-13, in progress):** Mapped the SCY path. KEY FINDINGS: (1) the BG fetcher ALREADY reads live SCY per plane at tile-data low (bg_fetch.rs:259) and high (bg_fetch.rs:295) — the DMG bitplane-desync works without any table (test fetch.rs:910). (2) `PpuMode3ObservedScyObjPhaseTable` + `scy_obj_phase_policy()` (transfer.rs:505) are **sprite-gated** — the policy only returns Some when a sprite is near the fetch. (3) `m3_scy_change` is **BG-only**, so the sprite-gated obj-phase table does NOT apply to it; its regression comes from the GENUINE live-SCY path / startup-SCY handling (`startup_scy_tiledata_latch`, cached-slice `scy_tile_data_row_changed`) now reading at the M1-shifted startup dots. (4) M1 left tiles 2+ dot-exact (99/107/115) but tile1 at 90 vs DocBoy 91 (1 dot early) — the residual startup wobble is the likely culprit if the `m3_scy_change` write lands in the early-tile region. NEXT STEP: extend the DocBoy fetch-trace to emit the SCY value/row per BG tile fetch, run `m3_scy_change.gb`, and diff our per-tile SCY-read dot/row vs DocBoy to pin the exact divergence — then either close the tile1 residual or align the startup SCY latch. The sprite-gated obj-phase table deletion (api.rs:688 `live_scy_write_routing`, transfer.rs:329-614 helpers) is separate cleanup that should stay green (no current ROM depends on it once verified).

  **m3_scy_change DMG CLOSED + CGB reclassified (2026-06-14).** Empirical close-out of the investigation above (DocBoy DMG fetch-trace + a gb-cycle per-dot SCY/seam probe + a per-pixel framebuffer diff harness, all reproduced then removed). The ROM is a **diagonal-skew test**: it rewrites SCY once per tile during mode 3 so each tile reads a different `(scy+ly)%8` data row.
  - **DMG root cause (now fixed).** Defect = column 0 (first visible tile) only. `mark_live_scy_write_while_startup_alignment_fifo_visible` (state.rs) gated the startup-SCY latch on an unlatched `StartupAlignmentFill` pixel being present **in the FIFO**. The M1 dummy dots delay the alignment seed so it sits in the **`fill`-pending stage one SCY write longer** before reaching the FIFO; the latch therefore missed the scy write the first visible tile must observe (e.g. ly=8: row should be 1/scy=1 at the seed's data read ≈ dot 88, but the FIFO-only gate first fired on the *next* write → row 2/scy=2). Fix: also treat the seed as latchable while it is the pending `fill` slice (`fill.pending && includes_real_tile_pixels && StartupAlignmentFill && !needs_live_tile_data_refetch`). DMG → 0 px diff; **shootout 264/264**. CPU-invisible: `cargo tests` green with no machine-trace fixture regen.
  - **CGB-compat reclassified — NOT BG-only, it is the M2 obj-stall cluster.** The "BG-only" premise was WRONG: `m3_scy_change` keeps **LCDC.1 obj-enable set with a sprite at column 0 on every line** (`nsel=1`). The obj-fetch stalls the BG fetcher across the mid-line SCY change. On CGB-compat the SCY-write path (api.rs:297) returns early via `scy_obj_phase_policy`, and the column row comes from the cached-slice recompute driven by the **sprite-coupled live-SCY routing** (CGB-D samples SCY *once* at the read, not per byte like DMG). Failures span the first **two** tiles: col 0 (seed, obj-stalled at TileDataLow → reads SCY after the stall) and col 1 (`VisibleTile2` continuation → reads SCY one write late under M1's +3-dot startup shift vs the unshifted CPU writes + CGB's 2-cycle write-observation latency). A principled "freeze the startup tile's SCY row at the read/stall + retire the obsolete `VisibleTile2/3` SCY obj-phase retarget overrides" reduced CGB 156→41 px, but the residual (col 1, ly 0–7, off-by-one row) needs the **CGB write-observation-latency model** and obj-phase-table collapse — i.e. the documented M2 per-table obj-stall modeling, entangled with the frozen #245 sprite penalty. Reverted those attempts to keep the DMG fix minimal. **Conclusion: CGB `m3_scy_change` belongs to the M2 obj-stall-coupled register-sampling cluster (mealybug CGB stays 23/24 until M2), not a standalone startup-SCY bug.**

  **CGB m3_scy_change — full empirical model (2026-06-14, oracle VALIDATED).** Built a CGB `#ifdef GBCYCLE_FETCH_TRACE` DocBoy nogui (`build-trace-cgb`, ENABLE_CGB=ON) + framebuffer-row dump (`GBDUMP_LY` in nogui main) + `bwf.scy`/`tile_y` emit at `setup_bg_pixel_slice_fetcher_tile_data_address`; gb-cycle ephemeral `examples/cgb_scy_probe.rs` (SCY-write schedule + per-tile FIFO-cached rows) + VRAM/bitmap decode. **All reproduced; revert DocBoy edits + delete gb-cycle example at M5.** Findings:
    - **DocBoy CGB framebuffer == the mealybug CGB fixture EXACTLY** (verified ly 0/24/28/74, x0–23). DocBoy is a faithful CGB oracle for this ROM. Its model: on CGB the global `scy` lags CPU writes by **2 dots** (`pending_write.scy.countdown=2`, `tick_pending_write` ppu.cpp:703/3526); the fetcher latches `bwf.scy = scy` **once at GetTile0** (ppu.cpp:2004) and uses it for **both** tile-data planes (ppu.cpp:2600). No mid-tile SCY change on CGB.
    - **All 156 failing px are the first two screen tiles (x0–15); steady tiles (x≥16) are already CGB-correct.** Per line, exactly ONE startup tile is wrong — whichever the obj fetch stalls just before. The sprite x **moves per line** with the diagonal (ly0→x0, ly24→x3, ly74→x9): ly0–7 fail tile0, ly23–31/72–79/136–143 fail tile1. So the failing tile = the tile whose GetTile0 lands right after the obj stall.
    - **Error = wrong tile-data ROW by ±1**, sign set by the SCY-triangle direction at the stall: ly0 tile0 gb row1 vs correct row0 (+1); ly24 tile1 gb row3 vs correct row2 (+1, `0x45` r2=`KGGKKKKK`=fixture); ly28 tile1 gb row7 vs row6 (+1); ly74 tile1 gb row2/4 vs correct row3 (`0x4B` r3=`KGGGGKKW`=fixture) (−1). gb-cycle samples SCY **live at the (M1-delayed + stall-shifted) tile-data read** with **no** write-observation latency and allows mid-tile refetch; DocBoy samples the **2-dot-delayed** value **once at GetTile0**. The net offset crosses one ~7–8-dot SCY-write boundary → ±1 row on exactly the stalled tile. (NOTE: `cgb_scy_probe`'s "final cached row" is unreliable — a refetch fires at/after the output dot; trust actual.png + VRAM decode, which is what the ±1 above is measured from.)
    - **Fix target:** make the CGB startup/stalled tile latch SCY **once at its GetTile0 (TileIndex) dot with the 2-dot write-observation latency**, frozen for the whole tile (no mid-tile refetch) — i.e. DocBoy's `bwf.scy` rule scoped to the startup seam. The 2-dot latency is roadmap §4 M2-step-1; gb-cycle's latch layer (`PpuMode3RegisterLatches{visible,pipeline}`, registers.rs:65 `advance_mode3_register_latches_from_mmio`) is a 2-deep shift (visible=this dot, pipeline=1 dot ago) — CGB needs a value 2 dots old, not currently available, so a dedicated CGB SCY pending-write (countdown=2) or a 3rd latch stage is required. Scope strictly to CGB SCY startup tiles: a global SCY delay risks the steady path (correct today via live-at-tile-data-read, a *different* mechanism) and the frozen #245 obj-stall timing. **Oracle for iteration: `GBDUMP_LY` framebuffer dump from `build-trace-cgb` == fixture, diff per line.**
    - **IMPLEMENTATION SITE PINNED (2026-06-14, two revertible attempts, both inert → reverted to green).** The seed's FINAL rendered row is NOT set at fetch time. (1) Freezing the startup-SCY latch (`mark_live_scy_write_while_startup_alignment_fifo_visible`, AlignmentSeedPending branch) had ZERO effect: that branch never fires for the seed on `m3_scy_change` because `scy_obj_phase_policy()` returns None at the SCY-write dots, so the whole `cgb_dmg_software_scy` route (api.rs:294) returns early — nothing is routed. (2) Capturing the GetTile0 SCY-row on the fetcher (`BgFetcherState`, at `advance_bg_fetcher_tile_index_dot1`) and using it at TileDataLow/High dot0 ALSO had ZERO effect. **Reason (matches the api.rs:297 note above): the seed's output row is (re)computed by `recompute_live_background_cached_slice` (state.rs:3343) via `context.current_scanline_tile_row()` = `(live raw scy + ly) % 8` whenever the cached slice carries `needs_live_tile_data_current_row_refetch`, which fires at push/output — overwriting any fetch-time row.** ⇒ The seed fix MUST live in the recompute path: carry the frozen GetTile0 SCY-row on `BgCachedSlice` and, for CGB startup-origin slices, use it instead of `current_scanline_tile_row()`. That fixes ONLY the seed band (ly0–7, ~35px). **SEED FIX LANDED 2026-06-14:** `BgFetcherState.cgb_startup_seed_get_tile_scy_row` captured at `advance_bg_fetcher_tile_index_dot1` (CGB family + `AlignmentSeedPending`, raw `tile_data_row()` at the GetTile0 dot), carried to `BgCachedSlice.cgb_startup_frozen_tile_row` via `from_fetcher`, and consumed in `recompute_live_background_cached_slice` (overrides `current_scanline_tile_row()` for BG). **CGB m3_scy_change 156→121px, ly0–7 band CLOSED.** CPU-invisible (full `cargo tests` green, no machine-trace fixture regen needed), CGB-scoped, ZERO regression: blargg 58, mooneye 113, wilbertpol 117, shootout 264, DMG mealybug green; CGB mealybug stays 23/24 (only m3_scy_change, now the VisibleTile2 bands). **The VisibleTile2/Tile3 bands (ly23–31/72–79/136–143, ~121px) are the harder half:** gb-cycle does NOT model the obj stall delaying the startup BG GetTile0 (obj=Idle through startup), so VisibleTile2's GetTile0 lands ~too early and is compensated *at output* by the `cgb_dmg_software_startup_visible_tile2/3_*` obj-phase retarget tables (mode3_policies.rs:983/1009) — correct for sprite x=0 (ly0) but wrong for the moved sprite (x=3/9). Closing them hardware-truly = roadmap §4 M2-step-2 (model obj-stall-delayed startup GetTile0, tied to the FROZEN #245 penalty) then collapse those SCY retarget tables. Extending/retuning the tables = forbidden curve-fit. **Net: full CGB close needs both halves; the seed half is LANDED (above), the VisibleTile2 half is large/§245-entangled and remains the open M2-step-2 work. DocBoy `build-trace-cgb` + `GBDUMP`/`GBT-TDATA` instrumentation left mounted, revert at M5.**

  **M2 STEP-2 CHARACTERIZED (2026-06-14, 3 revertible experiments, all WORSE → reverted; tree stays at seed-fix 121px).** The remaining bands map to a per-line sprite (sprite x = ly/8, 8 lines per x; obj probe `examples/cgb_objstall_probe.rs`, recreated+deleted). Failing = sprite x ∈ {2(ly23), 3(ly24-31), 9(ly72-79), 17(ly136-143)}. gb-cycle DOES obj-stall the startup BG fetcher (NOT obj=Idle as previously thought): e.g. ly24 obj fetch dot87-93, BG held at TileIndex/0 dot88-95, VisibleTile2 GetTile0 at dot96. Per-tile rows measured (gb FETCHED vs DocBoy CORRECT, tile1): ly0 fetched r2 / correct r3; ly24 fetched r2 / correct r2; ly74 fetched r3 / correct r3. So **gb's FETCHED row matches DocBoy on the failing bands (x3/x9), but the output recompute (`needs_live_tile_data_current_row_refetch` → live SCY at the output dot, ~dot108) corrupts it**; on the PASSING lines (x0 etc.) the fetched row is WRONG (gb's startup obj-stall lands ~2 dots early vs DocBoy: ly0 gb fetch dot100/scy2 vs DocBoy GetTile0 dot102/scy3) and the recompute ACCIDENTALLY fixes it. **Dead-ends (do not repeat):** (1) broaden the seed GetTile0-capture gate to all startup tiles → 272px (the startup seam has multiple TileIndex passes per tile — delayed reads + obj-stall re-entry — so a later pass overwrites the seed's capture). (2) keep the fetched row for `StartupContinuation` slices (skip the recompute override) → 225px (breaks the majority of lines where the fetched row is wrong). Conclusion: neither the fetched row nor the output recompute is uniformly correct; the correct row is the **obj-stall-delayed GetTile0 SCY**, which gb-cycle gets right only for some sprite-x. The principled fix is to correct gb-cycle's **startup fetch timing per sprite-x** (the obj-stall length for low-x/off-screen sprites is ~2 dots short, compounded by the M1 fetcher-lead residual "~1 dot early") so the GetTile0/fetched row is uniformly DocBoy-exact, THEN drop the recompute current-row override + the SCY retarget tables for CGB startup tiles. The obj-stall length lives in the FROZEN #245 penalty (`obj_fetch.rs:88` `alignment_stall_remaining`, shared startup+steady) so any change must be proven not to move the per-sprite mode3 cost (wilbertpol `intr_2_*_sprites`); likely the fix is in the startup obj path / M1-lead, not the steady penalty. This is the genuine §245-entangled structural work — not an output-layer tweak.

- **M4 — Halt grid unified.** Per-SCX halt-wake deferral replaced by uniform dispatch convention + recalibrated apertures; frozen #245 sprite penalty proven unchanged (probe per-sprite cost identical before/after). Guardrail doc lines 28–29 updated. *Gate: full §7 green; wilbertpol `intr_2_mode0_timing_sprites_nops` + `scxN_nops`, mooneye `intr_2_mode0_timing`, and the `mode_edges.rs` / `ppu_lcd_restart.rs` unit+integration tests green.*

- **M5 — Close-out.** DocBoy instrumentation reverted; `docs/TODO.md:20-23` struck; `docs/hardware/PPU-REIMPLEMENTATION.md` guardrails fully rewritten; `project_ppu_hardening` memory updated; **this working doc deleted**. *Gate: full §7 green; clean `git status`; agent-agnostic (no Claude/Codex-specific artifacts).*

---

## Key file:line index (for the executing engineers)

- Fetcher unconditional step / co-advance: `crates/gb-core/src/ppu/mode3/core.rs:143-146`, `:95-97`; SCX capture `:48`; mode3 entry gate `:23-31`.
- Fetcher pre-arm + priming budget: `crates/gb-core/src/ppu/state.rs:1703`, `:1693-1695`, `:1706-1715`; PostAlignment `:2352-2361`, `:2381-2406`, `:2340-2349`.
- Raw-dot constants (→ fetch-step counters): `crates/gb-core/src/ppu.rs:85-96`.
- Restart-idle / delayed read: `crates/gb-core/src/ppu/mode3/bg_push.rs:170-174`; `crates/gb-core/src/ppu/mode3/bg_fetch.rs:46-62`, `:72-94`, `:107-189`; `crates/gb-core/src/ppu/helpers/mode3_policies.rs:418-423`.
- Observation tables: `helpers/mode3_policies.rs:866-1023, 1151-1169, 1184-1228, 1255-1353, 1361-1490, 1522-1659`; window masks `mode3/window.rs:1199-1928`; SCX coarse gate `helpers/mode3_latches.rs:179-194`; consumers `control/live_writes.rs:80-305`, `helpers/mode3_lcdc2_obj_size.rs`, `mode3/transfer.rs:127, 244-249, 329-583`, `api.rs:293-320, 387-763`.
- Published-STAT / halt grid: `control/published_stat.rs:28, 61-84`; `control/irq.rs:159-160, 238-263, 285-286, 451-482, 466-473`; `machine/step.rs:1014-1028`.
- Oracles: SameBoy `Core/display.c:916-1107, 1471-1532, 1760-2150`, `Core/sm83_cpu.c:1625-1662`; DocBoy `src/docboy/docboy/ppu/ppu.cpp:938-972, 1533-1658, 1708-1791, 1946-2522`.
- Harness: `crates/gb-core/examples/g3_sprite_grid.rs`; `cpu_visible_stat_mode()` at `crates/gb-core/src/ppu/api.rs:1509`.
- Guardrail doc: `docs/hardware/PPU-REIMPLEMENTATION.md:28, 29, 57`; ledger `docs/TODO.md:20-23`.
