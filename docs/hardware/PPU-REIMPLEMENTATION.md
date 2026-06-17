# PPU Reimplementation Guardrails

## Scope

This file is not a phase-progress ledger. The Phase `4` PPU work is considered closed against the current external DMG report (`167/167`: `165` passing, `2` informational). Keep this document only as repo-local guardrails for touching the current implementation without reopening already-closed seams.

[PPU.md](./PPU.md) remains the hardware-facing contract and owns the no-regression ROM catalog. [TODO.md](../TODO.md) owns concrete open follow-ups. Nothing in this file overrides those documents.

## Re-entry Rules

- Start from [PPU.md](./PPU.md) when changing hardware behavior; use this file only to preserve repo-local seams while editing the current implementation.
- Treat the PPU ROM table in [PPU.md](./PPU.md#tests) as the diagnostic no-regression catalog, not as an active roadmap. Keep the relevant rows green for accepted PPU behavior changes.
- For exploratory ROM-driven work, preserve baseline and final `/test/gb-emulator-shootout/` snapshots for promoted GB Emulator Shootout suites, `/test/docboy/` snapshots for the large DocBoy suites, `/test/gbmicrotest/` snapshots for gbmicrotest, or standalone exploratory report root snapshots such as `/test/mooneye/`, `/test/little-things-gb/`, `/test/magen/`, `/test/mealybug-tearoom-tests/`, `/test/samesuite/`, `/test/wilbertpol/`, or `/test/rtc3test/`, compare them before keeping the change, and isolate one failing family at a time.
- If an old broad fix looks tempting, first check the subsystem-specific guardrails below; most closed regressions depended on narrow ownership seams, not on global retiming or scanline-wide rewrites.

## Repo-Local Seams To Preserve

### Ownership and scheduling

- Keep top-level `Ppu` ownership split between runtime/pipeline state and panel/live-output state.
- Keep MMIO-owned storage separate from the register view visible to the active pixel pipeline, including previous-dot or pipeline-visible snapshots where live-write-sensitive DMG behavior needs them.
- Keep the scheduler seam explicit: CPU micro-op effects stage first, PPU MMIO commits on the same T-cycle, then interrupt aggregation observes the result.
- Keep one internal LCD STAT line and request LCD STAT only on rising edges.
- Keep published `STAT` / visible access-mode evaluation expressed as ordered named rule families rather than one large inline branch chain.
- Keep the current split between bus/readback access mode and the DMG-family STAT IRQ mode source; LCD restart and SCX-dependent HBlank tests rely on those seams not being collapsed back into one raw `current_access_mode()` decision.
- Keep the visible-line `FF44` early-read seam explicit at dot `451` for ordinary visible HBlank; dot `450` still reads the current line and VBlank/line-`153` seams keep their own helpers.
- Keep exact-boundary non-extended Mode `0` `STAT.mode` publication gated by the Mode `0` IRQ enable path; nonzero-`SCX` Mode `2`-only probes may still publish Drawing at the same internal HBlank start dot, while `SCX=0` keeps the Mooneye-compatible exact-boundary HBlank readback.
- Keep the LCD re-enable first-line Mode `0` HALT wake aperture separate from the non-HALT `IF` publication edge; moving the PPU request edge instead of gating the halted CPU wake regresses the running-CPU HBlank timing tests.
- Keep the steady-state Mode `0` HALT wake deferral separate from the PPU `IF` edge; CGB-family applies it for all `SCX` low bits, DMG-family applies it for `SCX&7` in `{1,2,5,6}`, and both are limited to the four-dot Mode `0` pretrigger aperture when Mode `0` STAT is enabled.
- Keep DMG Mode `2` STAT as an internal source that can lead readable/bus OAM mode, including the line `143 -> 144` STAT-only edge; it must not imply real OAM locking or sprite selection on line `144`.
- Keep the line `143 -> 144` DMG Mode `2` STAT `IF` pretrigger separate from CPU interrupt service priority: the `IF` edge stays early for gbmicrotest, but service is deferred across the last HBlank dots when VBlank is enabled so line `144` VBlank can win priority.
- Keep line-`153` LY readback, LYC comparison, and line-`0` post-wrap published `STAT.mode` seams as separate helpers; collapsing them back into one `LY=0` dot loses the gbmicrotest `line_153_*` timing windows, while applying the DMG dot-`4`/dot-`12` split to CGB regresses the CGB-family dot-`8` LY0 seam.
- Keep the line-`153` `LYC=0` STAT IRQ pretrigger separate from readable `STAT.2`: the request edge is dot `8`, visible coincidence is dot `12`, and same-dot CPU `LYC`/`STAT` writes can cancel the unaggregated edge before interrupt aggregation.
- Keep the DMG `STAT` write quirk in explicit line/dot write windows rather than deriving it from readable mode or bus access mode; ordinary HBlank, ordinary OAM, and the frame-start line-`0` exception intentionally differ.
- Keep DMG VBlank `STAT` write quirk effects separate from ordinary LYC sources: nonzero writes can still generate the quirk pulse in VBlank/coincidence windows, and the quirk can block the repeated line-`153` `LYC=0` source without disabling the ordinary line-`153` path when no VBlank quirk occurred.
- Do not let nonzero STAT enable writes reuse the zero-write OAM/HBlank/restart quirk windows; `gbmicrotest` LYC1 setup depends on `STAT=$40` during LCD restart line `0` not leaving IF STAT pending before the line-`1` coincidence edge.
- Keep the DMG `WX = 0 && (SCX & 7) == 3` terminal readback seam ahead of generic terminal-tail early-HBlank publication; this is a CPU-visible `STAT` seam, not a renderer-only detail.
- Keep the DMG reset-facing startup Mode `0` STAT IRQ phase explicit and boot-gated for `CustomBoot` and verified `RealBoot` handoff; do not leak that first-frame hidden phase into plain `SkipBoot`, PPU startup-state unit tests, pre-handoff `RealBoot`, or LCD off/on restart timing.
- Keep the DMG direct-boot first-frame `FF44`/`FF41` CPU-bus overlay separate from the internal synthetic raster line; changing the internal machine direct-start `LY=0` state to direct-boot `LY=153` would reopen unrelated startup seams.
- Keep DMG-family boot-facing `poweron_*` publication tables as CPU-bus overlays for early `FF41` / `FF44` reads and OAM/VRAM access; `CustomBoot` uses the synthetic frame-origin base, verified `RealBoot` uses its own handoff-relative base, and neither path should be satisfied by moving the internal raster, changing `BootController::direct_boot_state()`, changing LCD restart, or changing renderer / sprite-selection state.
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
- Keep the CGB-family `LCDC.5` line-local activation latch separate from DMG same-line window restart seams: Mode `2` writes affect the next scanline's latch, while Mode `3` writes can update the current latch and feed the active fetcher.
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
- If a scanline already has delayed `BGP` CPU-commit writes, a later write whose recent BG tail is color `0` must not automatically backdate that tail unless the line is one of the explicitly modeled early selected-retroactive seams; this keeps the retained Daid `ppu_scanline_bgp.gb` RealBoot framebuffer expectation aligned without broad tail backdating.
- Keep `BGP` and `OBP*` conflict handling separate; do not assume shared retroactive spans or conflict windows.
- Keep `LCDC.0` repaint rules BG-only. Dots already emitted as forced white are not palette-conflict candidates, and OBJ dots must not be repainted as BG.
- Do not use fill-only or materialized-slice-only `LCDC.0` overrides as generic fixes; keep onset rules localized per write class and per boundary.

## Performance Notes

- Use the strict `ppu_phase6` harness as the default before/after gate for PPU runtime experiments.
- Do not prioritize a broad per-dot `Mode 3` context cache without new profiling evidence; previous release sampling did not show `mode3_register_latches()` or `mode3_window_policy()` as standalone hotspots.
- If runtime work is revisited, transfer / raster-publication work (`current_transfer()` and related mode-boundary publication) is a better first target than a generic helper-view cache.
- PPU timing caches must remain memoization-only: the active cache is limited to scanline length because its key is stable across the line/restart phase, while Mode `3 -> 0` boundary helpers use conservative no-op fast paths instead of broad same-dot keys until profiling justifies explicit invalidation; any future cache state must stay out of save states and the uncached hardware calculation remains the source of truth.
- PPU bus-state fast paths may only replace snapshot/publication work on stable dots where owner, CPU-read, and CPU-write access modes are identical; they must fall back at LCD off/on/restart, blank-frame, VBlank, Mode `2 -> 3`, Drawing `->` HBlank, `line_dot = 0`, and scanline-end publication seams, and tests must continue comparing the fast result against the direct owner/read/write helpers.
- Release builds may skip pre-PPU owner recomputation that only feeds debug/test ownership validation, but the post-PPU owner snapshot remains the value synchronized back into the bus and debug/test builds must keep the ownership-validation path intact.
- PPU residual profiling buckets are observability-only seams: `Tick`, `Mode3Control`, `Mode3BgEdge`, `Mode3WindowEdge`, `Mode3ObjEdge`, and `RasterPublication` must not nest with existing PPU region observers or reorder T-cycle work, while `ppu_profile_gap` / `ppu_unbucketed` remains measurement overhead/residual rather than a semantic optimization target.
- Desktop `summary-lite` and `summary-overhead` profiling modes are instrumentation-only diagnostics: `summary-lite` disables PPU sub-region callbacks while keeping outer machine regions, and `summary-overhead` compares unobserved, core-only observer, and full observer replays from cloned frame-start states; neither mode may change PPU timing, publication order, RGB555 output, DMA/HDMA behavior, STOP, or speed-switch semantics.
- A shared OBJ FIFO write kernel may be acceptable as local deduplication, but previous benchmark evidence was noise-threshold; do not land it as a performance change alone.
