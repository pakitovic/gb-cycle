# PPU Reimplementation Notes

## Scope

This file captures repo-local migration constraints, rollout order, unstable seams, and regression watch points for reworking the current PPU.

[PPU.md](./PPU.md) remains the hardware-facing contract.
Nothing in this file overrides that handbook.

## How To Use This File

1. Implement or reason from [PPU.md](./PPU.md) first.
2. Use this file when reworking the current repo without reopening already-closed tests.
3. Treat emulator cross-checks and test-driven notes here as compatibility constraints, not as hardware truth.

## Current Rollout Spine

- Keep the PPU dot-by-dot and mode-explicit.
- Keep Mode `2` as a fixed `80`-dot phase with one sprite entry examined every `2` dots.
- Keep BG/window as one shared fetcher-plus-FIFO pipeline.
- Keep window start as a fetcher/FIFO event, not as a late compositor switch.
- Keep OBJ work as an explicit fetch path with overlapping-OBJ priority resolved before BG/OBJ mixing.
- Keep LCD power, `STAT`, and interrupt timing explicit on the shared scheduler.
- Keep OAM corruption explicit, row-based, and DMG-family gated.
- Start any rewrite from the Pan Docs `pixel_fifo` contract before layering repo-local refinements.

## Repo-Local Migration Constraints

### MMIO And Live-Write Seams

- Keep MMIO-owned storage separate from the register view visible to the active pixel pipeline.
- Keep a second previous-dot or pipeline-visible snapshot for live-write-sensitive Mode `3` behavior.
- Window activation in the current repo still depends on delayed `LCDC.5` and `WX` visibility rather than only on newest MMIO state.
- BG/window fetch still needs localized `LCDC.4` and tile-data-selector seams.
- Visible-pixel transfer and DMG palette handling still need explicit previous-dot seams for `LCDC`, `BGP`, `OBP0`, and `OBP1`.
- DMG palette conflicts remain asymmetric: repo evidence allows a slightly wider retroactive window for `OBP*` than for `BGP`.

### LCD, STAT, And Power Closure

- Keep one internal LCD STAT line and request LCD STAT only on rising edges.
- Route both VBlank and LCD STAT through the shared interrupt-controller path.
- LCD off must enter one explicit disabled state; LCD on must restart from one explicit raster-start state.
- The first blank frame after LCD re-enable is panel behavior, not a delayed scheduler start.
- `STOP` blanking is distinct from LCD-off blanking.
- The finest `LY/STAT` timing around `lcdon_timing-GS`, `lcdon_write_timing-GS`, and some LCD on/off coincidence edges is still open.
- Keep the scheduler seam explicit: CPU phase, PPU MMIO commit, then interrupt aggregation.

### Mode 3, Push, And OBJ Arbitration

- Keep pending OBJ hits explicit; do not rediscover them from transient X-position checks.
- Keep cached BG/window slices explicit across `Push -> fill.pending -> FIFO`.
- Keep push-side ownership explicit: entry delay, FIFO-empty wait, fill queue, OBJ handoff, and combined fill-plus-OBJ dots.
- Current repo behavior assumes strict "BG/window push only when the BG FIFO is empty".
- Do not tighten push rules or move OBJ start wholesale onto `Push` in isolation.
- Keep one transfer-dot model that exposes explicit context, readiness, and execution effect.
- Let output-side transfer service and FIFO-backed OBJ start consume that same transfer-dot model.
- Cached-slice live-write closure is intentionally narrow; broad tilemap retargeting regressed stable cases.
- The remaining `LCDC.3` left-edge closure issue is concentrated on the third visible BG tile after startup.

### Window-Specific Closure

- Keep the `WY` latch and runtime `WX` trigger distinct.
- Treat the activation dot as separate from the restarted window fetch.
- The first activated window tile keeps its own push seam.
- The first activated window tile must not immediately hand its push dot to OBJ fetch.
- Turning `LCDC.5` off mid-window must finish the current window tile before BG resumes on a tile boundary.
- `WX = 0`, `WX = 166`, and same-line restart behavior remain explicit edge paths.
- The `WX = 0 && (SCX & 7) > 0` shortening case must stay explicit.

### Startup And Left-Edge Closure

- Startup transfer progress is driven by served transfer dots, not raw `line_dot`.
- Keep lane ownership, startup window (`AbstractStartup` versus `FifoBacked`), and effective BG FIFO occupancy as separate state.
- Visible output still requires a real BG FIFO pixel, even if startup uses placeholder-backed occupancy earlier.
- Keep the alignment/discard fetch distinct from the first real BG push.
- The first real BG/window push after startup still skips the ordinary one-dot push-entry delay.
- Pending OBJ hits may survive the `pre-visible -> hidden` transition while `current_transfer_x` is unchanged.
- Keep one explicit `current_transfer_x`-style owner for Mode `3` arbitration.
- Keep one explicit transfer-dot result carrying served-dot class and final `SCX` discard information.
- Window trigger eligibility should come from that served transfer-dot model, not from counters alone.

### DMA, OAM, And Corruption

- Keep the live Mode `2` OAM row as `line_dot / 4`.
- Keep `OamCorruptionController` explicit and DMG-family only.
- Late Mode `3` OBJ metadata during OAM DMA must be able to see the current DMA destination word and in-flight byte.
- Use domain-specific OAM/VRAM bus views instead of raw backing slices at the PPU boundary.

### Ownership And State Shaping

- Keep the PPU as source of truth for mode, `LY`, current Mode `2` row, and VRAM/OAM accessibility.
- Let the bus enforce the observable blocked-access result.
- Let the interrupt controller own `IF`; the PPU only raises requests.
- `SkipBoot` should synthesize a coherent hidden PPU phase rather than only a visible MMIO snapshot.
- Keep direct-boot `OBP0` and `OBP1` under an explicit uninitialized policy instead of baking fixed values.

## Deferred Work And Known Unstable Areas

- Late-HBlank `FF44` readback handoff.
- LCD on/off coincidence and initial post-enable `STAT` timing.
- Left-edge startup, cached-slice live writes, and `LCDC.3` tilemap retargeting.
- Same-line window restart and some `WX` edge behavior.
- Mid-frame `LCDC.2` sprite-size artifacts.
- The strict hidden-startup-dot model needed before full docboy-style transfer closure.

## Regressions To Watch

General timing and interrupt chronology:
- `mooneye acceptance/ppu/hblank_ly_scx_timing-GS`
- `mooneye ppu/stat_lyc_onoff`
- `mooneye ppu/lcdon_timing-GS`
- `mooneye ppu/lcdon_write_timing-GS`
- `intr_2_mode0_timing`
- `intr_2_oam_ok_timing`

Framebuffer and raster-visible behavior:
- `dmg-acid2`
- `hacktix/strikethrough`
- `mealybug m3_bgp_change`
- `m3_lcdc_tile_sel_change`
- `m3_scy_change`
- `m3_lcdc_bg_map_change`

## Workflow Reminder

- Capture a baseline copy of `/.roms/test/test-report.md` before rerunning known external-ROM closures.
- Capture the final report after the run.
- Compare before and after before deciding to keep a timing-sensitive change.
