# PPU Refactor Plan

## Scope

This file is a temporary execution guide for the current PPU refactor.
Delete or rewrite it once this refactor closes or the plan is superseded.

[PPU.md](./PPU.md) remains the hardware-facing contract.
[PPU-REIMPLEMENTATION.md](./PPU-REIMPLEMENTATION.md) remains the repo-local migration and regression handbook.
[TODO.md](../TODO.md) remains the active ledger of open closure work.
Nothing in this file overrides those documents.

## Current Decision

- Do not restart the PPU from zero.
- Do not treat the Donkey Kong slowdown as a blocking root cause to close first.
- Treat the next implementation pass as a scoped `Mode 3` refactor of the shared BG/window/OBJ fetcher and transfer pipeline.
- The focus is `Mode 3`, but adjacent internal refactors are allowed when they are required by the shared model, by ownership cleanup, or by integration with LCD/`STAT`, `Mode 2`, DMA/OAM, or startup seams.
- Keep the Donkey Kong desktop case as a hard no-regression performance gate while that refactor is in flight.
- The desktop profiler and frame diagnostics added in commit `8f31f46` are the baseline tool for separating PPU cost from pacing and host-side cost during this refactor.

## Hard Goals

- Keep the public `Ppu` contract, scheduler ordering, MMIO-commit seam, and interrupt-controller integration stable.
- Keep the external `ppu.rs` entry point stable while `Mode 3` is being reworked internally.
- Prefer to keep `Mode 2`, LCD/`STAT`, and DMA/OAM behavior stable, but allow narrowly scoped adjacent refactors when the shared pipeline model or ownership split requires them.
- Rebuild `Mode 3` around the canonical Pan Docs `pixel_fifo` contract:
  `TileIndex -> TileDataLow -> TileDataHigh -> Sleep -> Push`.
- Keep one shared transfer-dot model for output, `SCX` discard, window trigger eligibility, and FIFO-backed OBJ start.
- Keep repo-local seams explicit instead of rediscovering them from raw counters or FIFO length alone.
- Return to the highest-value remaining closure from [TODO.md](../TODO.md): cached-slice live-write timing for the second and especially third visible BG tile after startup, with focus on `x = 16..23`.

## Implementation Constraints

- Keep the external `Ppu` API unchanged. No public API reshaping is part of this refactor.
- Keep stepping on the existing `T-cycle` contract and preserve the existing MMIO-commit phase ordering.
- Keep the interrupt-controller path unchanged from the PPU side.
- Keep `VramBusView` and `OamBusView` as the domain-facing memory access boundary for PPU work.
- Split the current internal implementation into clearer ownership units:
  - raster and mode progression
  - `STAT`, `LY`, and `LYC`
  - LCD on, off, and restart
  - `Mode 2`
  - BG/window fetcher
  - transfer and pixel output
  - OBJ fetch and OBJ FIFO
  - OAM corruption
- Keep `ppu.rs` as the façade and integration point after that internal split.

## Hard Non-Goals

- Do not delete `ppu.rs` and restart the entire subsystem.
- Do not attempt another isolated "strict push" or "push-state-only OBJ start" change.
- Do not reopen broad startup realignment, broad tilemap rereads, or broad cached-slice retargeting before a new trace proves they are needed.
- Do not accept a change that improves one oracle while regressing the existing watchlist or the desktop performance gate.

## Authority And Precedence

Use the documents in this order:

1. [PPU.md](./PPU.md) for hardware behavior and the external PPU contract.
2. [PPU-REIMPLEMENTATION.md](./PPU-REIMPLEMENTATION.md) for migration constraints, unstable seams, and regression watch points.
3. [TODO.md](../TODO.md) for the active highest-value closure target and re-entry warnings.
4. [DESKTOP.md](../DESKTOP.md) for profiler and trace environment variables.
5. This file for the temporary rollout order and the no-regression performance gate.

## Fixed Per-Iteration Gates

Run this desktop case on every meaningful `Mode 3` iteration:

```bash
GB_CYCLE_DESKTOP_AUDIO_DISABLE_PACING_CORRECTION=1 \
GB_CYCLE_DESKTOP_EMU_PROFILE=summary:4 \
cargo run --release -p gb-desktop -- "/Users/pakitovic/workspace/gb-cycle/.roms/Donkey Kong (World) (Rev 1) (SGB Enhanced).gb"
```

Use that run as a hard no-regression gate, not as a mandatory-per-commit improvement target.

Keep these functional gates alongside it:

- `cargo test -p gb-core ppu -- --nocapture`
- Baseline and final `/.roms/test/test-report.md` whenever rerunning known external ROM closures

Capture an initial baseline for this desktop scenario and enforce no regression at least on:

- `speed`
- `core_est_ms`
- `ppu_ms`
- `ppu_mode3_startup_ms`
- `ppu_bg_ms`
- `ppu_win_ms`
- `ppu_push_ms`
- `ppu_obj_ms`
- `ppu_px_ms`
- `scanlines_over_456`
- `max_mode0_start_dot`
- `ly0_stall_*`
- `present_ms`
- `pac_ms`
- `audio_corr_ms`
- `late_ms`
- `oversleep_ms`

If the desktop summary worsens or shows a clearly dominant PPU bucket, rerun the same case with a trace:

```bash
GB_CYCLE_DESKTOP_AUDIO_DISABLE_PACING_CORRECTION=1 \
GB_CYCLE_DESKTOP_EMU_PROFILE=summary:4 \
GB_CYCLE_DESKTOP_TRACE_PATH=/tmp/gb-dk-trace.txt \
GB_CYCLE_DESKTOP_TRACE_T_CYCLES=32768 \
cargo run --release -p gb-desktop -- "/Users/pakitovic/workspace/gb-cycle/.roms/Donkey Kong (World) (Rev 1) (SGB Enhanced).gb"
```

## How To Read The Desktop Summary

- If `host_ms`, `present_ms`, `pac_ms`, `audio_corr_ms`, `late_ms`, or `oversleep_ms` dominate while `core_est_ms` and `ppu_ms` stay healthy, do not blame the PPU first.
- If `ppu_mode3_startup_ms` or `ly0_stall_*` dominate, investigate startup and left-edge ownership.
- If `ppu_push_ms` dominates, investigate `Push`, FIFO-empty wait, and BG/window/OBJ handoff.
- If `ppu_win_ms` dominates, investigate window activation, restart, and first-window-tile handling.
- If `ppu_obj_ms` dominates, investigate OBJ fetch, arbitration, and late OAM/DMA conflicts.
- If `ppu_px_ms` dominates, investigate transfer service, visible output, discard, and BG/OBJ mixing.
- Treat `scanlines_over_456` and `max_mode0_start_dot` as hard evidence that the frame is stretching, not as incidental diagnostics.

## Refactor Work Order

1. Capture a baseline Donkey Kong summary and a baseline `/.roms/test/test-report.md` before the first new slice.
2. Split the current monolithic PPU into clearer internal ownership boundaries without changing the public contract.
3. Rebuild the `Mode 3` substrate around the canonical BG/window fetcher, `Sleep`, and `Push` retry rules.
4. Keep one explicit transfer-dot source of truth carrying context, readiness, and execution effect.
5. Reintroduce repo-local seams in this order:
   - transfer-dot ownership and readiness
   - `Push -> fill.pending -> FIFO` ownership
   - window activation dot
   - first activated window tile
   - pending OBJ hits
   - cached slices
6. Return to the highest-value remaining closure:
   - cached-slice live-write timing for the second and especially third visible BG tile after startup, especially `x = 16..23`
7. After that closure, resume the remaining families in this order:
   - window and live-`LCDC.5`
   - mooneye LCD restart lane
   - sprite-coupled `STAT` timing

At the start of Phase 1, explicitly record which desktop buckets are currently hot and whether the slowdown looks PPU-side, pacing-side, or mixed.

## Current Ladder Gate

The DMG maturity ladder in `PPU.md` currently says the refactor state is not monotonic:

- `order 2` `daid/ppu_scanline_bgp.gb` is still red.
- `order 15` `mooneye acceptance/ppu/intr_2_mode0_timing_sprites.gb` is still red.
- `order 16` `mooneye acceptance/ppu/lcdon_timing-GS.gb` is still red.
- `order 17` `mooneye acceptance/ppu/lcdon_write_timing-GS.gb` is still red.

That means the project already has later closures in harder but narrower areas, but it should not treat `27+` mealybug `Mode 3` hi-fi cases as the primary closure target until those earlier ladder blockers are closed or explicitly waived with stronger evidence.

Practical interpretation:

- keep the `Mode 3` seams and mealybug reds as sentinels and diagnostic oracles
- prioritize the LCD restart lane first because it likely explains both the open `lcdon_*` mooneye cases and part of the remaining repeated left-edge debt
- then close the sprite-coupled `intr_2_mode0_timing_sprites` case
- promote `daid/ppu_scanline_bgp.gb` to an active gate so the refactor does not keep advancing while an earlier visible-raster baseline is still red

## Required Structural Constraints

- Keep BG and window on one shared fetcher-plus-FIFO pipeline.
- Let OBJ seize the shared fetcher only from an eligible BG/window `Push`, and only when the BG FIFO is not empty.
- Keep MMIO-owned storage separate from the register view visible to the active pixel pipeline.
- Keep previous-dot or pipeline-visible snapshots explicit where live writes need them.
- Keep `current_transfer_x`, startup state, lane ownership, effective BG FIFO occupancy, and cached-slice origin explicit instead of reconstructing them from `line_dot`.
- Keep the repo-local startup seam explicit as startup state rather than collapsing `AbstractStartup` and `FifoBacked` behavior back into generic counters.
- Keep OAM corruption, Mode `2`, LCD power, `STAT`, and DMA-facing ownership outside the main `Mode 3` rewrite focus unless the shared model or the ownership cleanup requires a narrow adjacent refactor.

## Sentinel Suite

Keep these cases green while the refactor is in progress:

- `dmg-acid2`
- `hacktix/strikethrough`
- `mealybug m3_bgp_change`
- `m3_lcdc_tile_sel_change`
- `m3_scy_change`
- `m3_lcdc_bg_map_change`
- `mooneye acceptance/ppu/hblank_ly_scx_timing-GS`
- `mooneye ppu/stat_lyc_onoff`
- `mooneye ppu/lcdon_timing-GS`
- `mooneye ppu/lcdon_write_timing-GS`
- `intr_2_mode0_timing`
- `intr_2_oam_ok_timing`

Any improvement that reopens this watchlist is not ready to keep.

## Acceptance Rule For Each Slice

A slice is ready only when all of these stay true:

- The scoped correctness target improves or becomes better isolated.
- The Donkey Kong desktop summary does not regress against the baseline in the relevant no-regression buckets.
- The sentinel suite does not reopen.
- The before/after `/.roms/test/test-report.md` delta is understood and acceptable.

If one of those fails, the slice is incomplete even if one local oracle turned greener.
