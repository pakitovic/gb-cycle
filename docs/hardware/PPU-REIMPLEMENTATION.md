# PPU Reimplementation Notes

## Scope

This file keeps repo-local migration constraints and regression watch points for touching the current PPU implementation.

[PPU.md](./PPU.md) remains the hardware-facing contract.
[TODO.md](../TODO.md) remains the active ledger of open closure work.
Nothing in this file overrides those documents.

## How To Use This File

1. Implement or reason from [PPU.md](./PPU.md) first.
2. Use [TODO.md](../TODO.md) for the current frontier and active regression gates.
3. Use this file only to avoid reopening already-closed repo-local seams while reworking internals.

## Repo-Local Migration Constraints

### Scheduler, MMIO, and STAT

- Keep MMIO-owned storage separate from the register view visible to the active pixel pipeline.
- Keep previous-dot or pipeline-visible snapshots explicit where live-write-sensitive DMG behavior needs them.
- Keep the scheduler seam explicit: CPU phase, then PPU MMIO commit, then interrupt aggregation.
- Keep one internal LCD STAT line and request LCD STAT only on rising edges.
- Route both VBlank and LCD STAT through the shared interrupt-controller path.
- LCD off must enter one explicit disabled state; LCD on must restart from one explicit raster-start state.
- The first blank frame after LCD re-enable is panel behavior, not a delayed scheduler start.
- `STOP` blanking is distinct from LCD-off blanking.

### Mode 3 ownership and arbitration

- Keep BG/window as one shared fetcher-plus-FIFO pipeline.
- Keep pending OBJ hits explicit; do not rediscover them from transient X-position checks.
- Keep cached BG/window slices explicit across `Push -> fill.pending -> FIFO`.
- Keep push-side ownership explicit: entry delay, FIFO-empty wait, fill queue, OBJ handoff, and combined fill-plus-OBJ dots.
- Keep one transfer-dot model that exposes explicit context, readiness, and execution effect.
- Let output-side transfer service and FIFO-backed OBJ start consume that same transfer-dot model.
- Do not attempt isolated "strict push" or "push-state-only OBJ start" changes; those already regressed stable closures.

### Startup, left-edge, and window seams

- Startup transfer progress is driven by served transfer dots, not raw `line_dot`.
- Keep lane ownership, startup window (`AbstractStartup` versus `FifoBacked`), and effective BG FIFO occupancy as separate state.
- Keep the alignment/discard fetch distinct from the first real BG push.
- The first real BG/window push after startup still skips the ordinary one-dot push-entry delay.
- Keep `current_transfer_x`-style ownership explicit for Mode `3` arbitration.
- Keep the `WY` latch and runtime `WX` trigger distinct.
- Treat the activation dot as separate from the restarted window fetch.
- Turning `LCDC.5` off mid-window must finish the current window tile before BG resumes on a tile boundary.
- Keep `WX = 0`, `WX = 166`, and the `WX = 0 && (SCX & 7) > 0` shortening case explicit.

### DMA, OAM, and corruption

- Keep the live Mode `2` OAM row as `line_dot / 4`.
- Keep `OamCorruptionController` explicit and DMG-family only.
- Late Mode `3` OBJ metadata during OAM DMA must be able to see the current DMA destination word and in-flight byte.
- Use domain-specific OAM/VRAM bus views instead of raw backing slices at the PPU boundary.
- Keep the PPU as source of truth for mode, `LY`, current Mode `2` row, and VRAM/OAM accessibility; let the bus enforce blocked-access results.

### DMG panel-output and palette seams

- Keep the DMG BG palette-output model split from the raw current-scanline color pipeline.
- Keep the narrow CPU-path `BGP` previous-line boundary repaint seam explicit, panel-only, DMG-only, and fed only by the delayed pipeline-visible write class.
- Keep the DMG CPU-path `BGP` live-write seam explicitly bifurcated: the first visible-line CPU write stays retroactive while `visible_pixels_output == 0`, `current_transfer_x == 0`, and no sprites were selected; after that startup seam, retroactive panel recolor should only happen when the already-visible BG tail is all color `0`.
- Keep the sprite-coupled DMG `BGP` live-write follow-up explicit too: a single left sprite shifts the first two CPU-path write onsets by sprite phase and can expose a short transient left-edge range on the second write before the final palette becomes visible.
- Keep DMG palette-conflict handling asymmetric where repo-local evidence requires it; do not assume `BGP` and `OBP*` share the same retroactive span.

## Known Unstable Areas

Use [TODO.md](../TODO.md) for the exact still-red ROM list, active closure target, and current no-regression set.
This file only keeps the broader instability classes that remain easy to reopen during internal rewrites:

- Remaining `Mode 3` live-write families beyond the current `BGP` / `OBP0` closures.
- `SkipBoot` oracle closure.
- Exact late-HBlank `FF44` readback seam.
- Same-line window restart and `WX` edge behavior.
- Mid-frame `LCDC.2` sprite-size artifacts.

## Regressions To Watch

Use [TODO.md](../TODO.md) for the full active rerun set.
This watch list keeps the smaller set of repo-local closures that have already regressed during internal PPU refactors.

Timing and interrupt chronology:
- `mooneye acceptance/ppu/hblank_ly_scx_timing-GS.gb`
- `mooneye acceptance/ppu/lcdon_timing-GS.gb`
- `mooneye acceptance/ppu/lcdon_write_timing-GS.gb`
- `mooneye acceptance/ppu/intr_2_mode0_timing_sprites.gb`

Raster and panel-visible behavior:
- `acid/dmg-acid2.gb`
- `daid/ppu_scanline_bgp.gb`
- `hacktix/strikethrough.gb`
- `mealybug ppu/m3_bgp_change.gb`
- `mealybug ppu/m3_bgp_change_sprites.gb`
- `mealybug ppu/m3_obp0_change.gb`

OAM corruption:
- `blargg oam_bug/4-scanline_timing.gb`
- `blargg oam_bug/5-timing_bug.gb`

## Workflow Reminder

- Capture a baseline copy of `/.roms/test/test-report.md` before rerunning known external-ROM closures.
- Capture the final report after the run.
- Compare before and after before deciding to keep a timing-sensitive change.
