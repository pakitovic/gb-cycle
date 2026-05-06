# PPU Reimplementation Guardrails

## Scope

This file is not a phase-progress ledger. The Phase `4` PPU work is considered closed against the current external DMG report (`167/167`: `165` passing, `2` informational). Keep this document only as repo-local guardrails for touching the current implementation without reopening already-closed seams.

[PPU.md](./PPU.md) remains the hardware-facing contract and owns the no-regression ROM catalog. [TODO.md](../TODO.md) owns concrete open follow-ups. Nothing in this file overrides those documents.

## Re-entry Rules

- Start from [PPU.md](./PPU.md) when changing hardware behavior; use this file only to preserve repo-local seams while editing the current implementation.
- Treat the PPU ROM table in [PPU.md](./PPU.md#tests) as the diagnostic no-regression catalog, not as an active roadmap. Keep the relevant rows green for accepted PPU behavior changes.
- For exploratory ROM-driven work, preserve baseline and final `/.roms/test/test-report.md` snapshots for promoted suites or `/.roms/test/test-report-extra.md` snapshots for extra/internal suites, compare them before keeping the change, and isolate one failing family at a time.
- If an old broad fix looks tempting, first check the subsystem-specific guardrails below; most closed regressions depended on narrow ownership seams, not on global retiming or scanline-wide rewrites.

## Repo-Local Seams To Preserve

### Ownership and scheduling

- Keep top-level `Ppu` ownership split between runtime/pipeline state and panel/live-output state.
- Keep MMIO-owned storage separate from the register view visible to the active pixel pipeline, including previous-dot or pipeline-visible snapshots where live-write-sensitive DMG behavior needs them.
- Keep the scheduler seam explicit: CPU micro-op effects stage first, PPU MMIO commits on the same T-cycle, then interrupt aggregation observes the result.
- Keep one internal LCD STAT line and request LCD STAT only on rising edges.
- Keep published `STAT` / visible access-mode evaluation expressed as ordered named rule families rather than one large inline branch chain.
- Keep the current split between bus/readback access mode and the DMG-family STAT IRQ mode source; LCD restart and SCX-dependent HBlank tests rely on those seams not being collapsed back into one raw `current_access_mode()` decision.
- Keep the LCD re-enable first-line Mode `0` HALT wake aperture separate from the non-HALT `IF` publication edge; moving the PPU request edge instead of gating the halted CPU wake regresses the running-CPU HBlank timing tests.
- Keep DMG Mode `2` STAT as an internal source that can lead readable/bus OAM mode, including the line `143 -> 144` STAT-only edge; it must not imply real OAM locking or sprite selection on line `144`.
- Keep line-`153` LY readback, LYC comparison, and line-`0` post-wrap published `STAT.mode` seams as separate helpers; collapsing them back into one `LY=0` dot loses the gbmicrotest `line_153_*` timing windows.
- Keep the line-`153` `LYC=0` STAT IRQ pretrigger separate from readable `STAT.2`: the request edge is dot `8`, visible coincidence is dot `12`, and same-dot CPU `LYC`/`STAT` writes can cancel the unaggregated edge before interrupt aggregation.
- Keep the DMG `WX = 0 && (SCX & 7) == 3` terminal readback seam ahead of generic terminal-tail early-HBlank publication; this is a CPU-visible `STAT` seam, not a renderer-only detail.
- Keep DMG `SkipBoot`'s startup Mode `0` STAT IRQ phase explicit and boot-gated; do not leak that first-frame hidden phase into ordinary PPU startup-state unit tests or into LCD off/on restart timing.
- Keep DMG `SkipBoot`'s first-frame `FF44` readback lag separate from the internal synthetic raster line; changing the internal machine skip-boot `LY=0` state to direct-boot `LY=153` would reopen unrelated startup seams.
- LCD off must enter one explicit disabled state; LCD on must restart from one explicit raster-start state. The first blank frame after re-enable is panel behavior, not a delayed internal scheduler start.

### Mode 3 and fetch arbitration

- Keep BG/window as one shared fetcher-plus-FIFO pipeline, with typed FIFO entries that carry output color plus cached BG/window sideband metadata.
- Keep cached BG/window slices explicit across `Push -> fill.pending -> FIFO`.
- Keep pending OBJ hits explicit; do not rediscover them from transient X checks.
- Keep one transfer-dot model that exposes context, readiness, and whether the dot consumed discard, hidden transfer, visible transfer, or a stall.
- Keep the stage-split `Mode 3` helpers (`BG` fetch, transfer service, `OBJ` fetch) unless the split itself proves observably wrong.
- Keep startup transfer progress driven by served transfer dots, not raw `line_dot`; lane ownership, startup window, and effective BG FIFO occupancy must remain distinct.
- Do not start timing-regression work from broad cached-slice / visible-FIFO retargeting, broad `SCX` / `SCY` retargeting, isolated "strict push" experiments, or broad dummy-startup fill retiming. First prove the affected transfer boundary with a narrow trace or oracle.

### Live writes, startup, and window seams

- Keep `LCDC.3` / `LCDC.4` startup-continuation owner state explicit in the BG pipeline; do not replace it with broad startup realignment, broad tilemap rereads, broad FIFO rewrites, or synthetic repaint windows.
- Treat the current green Mode `3` live-write seams as narrow DMG hypotheses: `BGP` / `OBP0` panel paths, sprite-coupled `STAT` publication, `SCX` startup carry handling, curated `SkipBoot` boot-trademark seeding for `LCDC.3` / `LCDC.4`, and startup-continuation overrides on `VisibleTile2` / `VisibleTile3`. Do not generalize them into wider rewrite permissions.
- Keep sprite-phased live-write hypotheses declarative through observed policy tables, not ad hoc imperative branches.
- Keep `current_transfer_x`-style ownership explicit for Mode `3` arbitration.
- Keep the `WY` latch and runtime `WX` trigger distinct.
- Keep the DMG same-line window restart / retarget seam grouped under one owner with `arm / clear / expire / followup` transitions.
- Treat the activation dot as separate from the restarted window fetch.
- Turning `LCDC.5` off mid-window must finish the current window tile before BG resumes on a tile boundary.
- Keep `WX = 0`, `WX = 166`, and `WX = 0 && (SCX & 7) > 0` as explicit edge-case paths.
- Keep the later-DMG `RealBoot` first-LCD-enable dot phase and `FF50`-armed Mode `0` SCX seam separate from ordinary LCD re-enable behavior; this is a boot/handoff hidden-state contract, not a generic `LCDC.7` restart rule.

### DMA, OAM, and corruption

- Keep the live Mode `2` OAM row as `line_dot / 4`.
- Keep `OamCorruptionController` explicit and DMG-family only.
- Late Mode `3` OBJ metadata during OAM DMA must be able to see the current DMA destination word and in-flight byte.
- Use domain-specific OAM/VRAM bus views at the PPU boundary rather than raw backing slices.
- Keep the PPU as source of truth for mode, `LY`, current Mode `2` row, and VRAM/OAM accessibility; let the bus enforce blocked-access results.

### Panel output and palette seams

- Keep the DMG BG palette-output model split from the raw current-scanline color pipeline.
- Keep DMG panel live-write owner state explicit for `LCDC.0`, CPU-path `BGP`, and recent panel-dot history.
- Keep palette-conflict handling as `classify -> plan -> apply`, with panel-history and CPU-commit history owned by the palette-conflict subsystem.
- Keep the narrow CPU-path `BGP` previous-line boundary repaint seam explicit, panel-only, DMG-only, and fed only by the delayed pipeline-visible write class.
- A delayed CPU-commit `BGP` write exposes the ORed transient palette for one panel dot at the delayed onset before the committed palette becomes visible; the same transient dot is used when repainting an eligible previous-line boundary.
- If a scanline already has delayed `BGP` CPU-commit writes, a later write whose recent BG tail is color `0` must not automatically backdate that tail unless the line is one of the explicitly modeled early selected-retroactive seams; this keeps Daid `ppu_scanline_bgp.gb` RealBoot aligned with the SameBoy `ppu_scanline_bgp_1.dmg.png` oracle.
- Keep `BGP` and `OBP*` conflict handling separate; do not assume shared retroactive spans or conflict windows.
- Keep `LCDC.0` repaint rules BG-only. Dots already emitted as forced white are not palette-conflict candidates, and OBJ dots must not be repainted as BG.
- Do not use fill-only or materialized-slice-only `LCDC.0` overrides as generic fixes; keep onset rules localized per write class and per boundary.

## Performance Notes

- Use the strict `ppu_phase6` harness as the default before/after gate for PPU runtime experiments.
- Do not prioritize a broad per-dot `Mode 3` context cache without new profiling evidence; previous release sampling did not show `mode3_register_latches()` or `mode3_window_policy()` as standalone hotspots.
- If runtime work is revisited, transfer / raster-publication work (`current_transfer()` and related mode-boundary publication) is a better first target than a generic helper-view cache.
- A shared OBJ FIFO write kernel may be acceptable as local deduplication, but previous benchmark evidence was noise-threshold; do not land it as a performance change alone.
