# Open TODOs

Concrete remaining work extracted from the DMG roadmap.
See [ROADMAP.md](ROADMAP.md) for phase context and implementation order.

## Guidelines

Keep this ledger lean in status noise and rich in re-entry context.

When an open item is non-trivial, make four things obvious:

1. What exact behavior or validation gap remains open.
2. What evidence is already in hand.
3. Which superseded directions should not be retried first.
4. What the highest-value next step is.

Remove TODOs when closed. Rewrite when the old wording points to a superseded path. Do not keep archival `Done` bullets.

### Cross-phase

- None currently.

### Phase 0 — Verification, debugging, and base architecture infrastructure

- None currently.

### Phase 1 — Temporal foundation and hardware access

- None currently.

### Phase 2 — CPU and real temporal control

- None currently.

### Phase 3 — Base DMA

- None currently.

### Phase 4 — Base PPU and visible pipeline

#### Current checkpoint

- The broad PPU refactor is structurally landed: explicit visible and pipeline register snapshots, explicit `Mode 3` transfer/readiness/execution state, push/fill ownership, startup-alignment seam, cached-slice ownership across `Push -> fill -> FIFO`, and typed cached-slice origins for the second and third visible post-startup BG tiles.
- Work stopped before closing the main oracle debt. Still-open families: `Mode 3` live-write cases (`m3_lcdc_bg_map_change`, `m3_lcdc_tile_sel_change`, `m3_lcdc_tile_sel_win_change`, `m3_scy_change`), mooneye LCD restart lane, and one sprite-coupled `STAT` timing case.
- Last stable measurements: `m3_lcdc_bg_map_change: 674`, `m3_lcdc_tile_sel_change: 1410`, `m3_lcdc_tile_sel_win_change: 1232`, `m3_scy_change: 7819`.
- Visible-FIFO cached-slice ownership is now explicit too, via a per-pixel sideband that survives `fill.pending -> FIFO` and startup placeholders. A first broad activation of live-write recompute on that visible FIFO regressed `m3_lcdc_tile_sel_change` (`1410 -> 1694`) while leaving `m3_lcdc_bg_map_change` at `674`, so keep the sideband but do not re-enable broad visible-FIFO retargeting without a narrower same-T-cycle oracle.
- A narrower follow-up path now exists for `LCDC.3` only: mark eligible visible-FIFO cached pixels and retarget on visible pop, with unit coverage for third-tile metadata and on-demand recompute. External oracle did not move (`m3_lcdc_bg_map_change` stayed `674`, `m3_lcdc_tile_sel_change` stayed `1410`), so do not keep pushing this seam blindly without a stronger trace showing the write lands after `fill.pending` and before visible pop.
- A matching visible-FIFO attempt for `LCDC.4` / `SCY` startup-continuation slices regressed both target families (`m3_lcdc_tile_sel_change: 1410 -> 1487`, `m3_scy_change: 7819 -> 8113`) and was reverted. Treat that as evidence that the remaining debt likely sits earlier in the startup dummy / first-fetch seam, not in late visible-pop retargeting.
- `PpuSnapshot` now exports typed visible-FIFO cached-slice metadata alongside raw `bg_fifo_pixels`, so future `snapshot.txt` captures can distinguish dummy occupancy from startup continuation slices and show whether late pixels still carry `needs_live_*` flags.
- The post-alignment startup seam now explicitly models the documented rule that the first real BG push after startup skips the ordinary one-dot push-entry delay. Repo-local coverage now locks the one-shot seam flag, the immediate first real queued fill, and the exported startup-seam snapshot fields.
- Targeted reruns of `mealybug-m3-lcdc-bg-map-change`, `mealybug-m3-lcdc-tile-sel-change`, and `mealybug-m3-scy-change` remained red after that seam fix, while the Donkey Kong desktop gate stayed stable after warm-up (`speed = 100%`, `ppu_mode3_startup_ms = 0`, `scanlines_over_456 = 0`, `ly0_stall_tcycles = 0`). Treat that as evidence that the remaining left-edge debt is not closed by the first-real-push skip alone.
- The live PPU scheduler trace now emits startup-seam state plus current fetch/push/fill origins and the front visible-FIFO cached origin. A local `info-trace` rerun of `m3_lcdc_bg_map_change` at `timeout_tcycles = 150000` showed the repeated `FF40 0x8B -> 0x83` writes hitting two distinct ownership edges on the same line: `VisibleTile2` late in `push.pending` with `entry_delay_remaining = 0`, and `VisibleTile3` still in the background fetcher at `TileDataHigh`.
- Repo-local coverage now locks those two edges too: late `VisibleTile2` push ownership for `LCDC.3`, and a fetcher-carried `needs_live_tilemap_refetch` handoff for `VisibleTile3` when the write lands during `TileDataLow` / `TileDataHigh`.
- A follow-up widening of the `LCDC.3` visible-FIFO mark from the third post-startup tile to the second and third post-startup tiles is also covered repo-locally, including on-demand visible-pop recompute for `VisibleTile2`. That still did not move the external oracles.
- External oracle still did not move after that narrower ownership fix: `mealybug-m3-lcdc-bg-map-change`, `mealybug-m3-lcdc-tile-sel-change`, and `mealybug-m3-scy-change` remained red, while the Donkey Kong desktop gate stayed stable after warm-up (`speed = 100%`, `core_est_ms ~= 10.5-10.6`, `ppu_ms ~= 5.77-5.84`, `scanlines_over_456 = 0`, `ly0_stall_tcycles = 0`).
- A normalized fixture diff against the repo-managed mealybug blobs no longer points `m3_lcdc_tile_sel_change` at the old `x = 8, y = 0` handoff. Current first mismatches are: `m3_lcdc_bg_map_change -> x = 12, y = 0`, `m3_lcdc_tile_sel_change -> x = 0, y = 10`, `m3_scy_change -> x = 1, y = 0`. Treat that as evidence that `tile_sel` has moved off the original first-visible-`VisibleTile2` oracle and still breaks on a repeated left-edge seam later in the frame.
- `PpuSnapshot` and the live PPU scheduler trace now export the current transfer context too: lane, source window, backing, readiness, and planned transfer kind. Use those fields on the next trace rerun before attempting another startup-dummy or first-fetch behavioral change.
- Two exact trace-derived repo-local tests now lock the `m3_lcdc_bg_map_change` `FF40` seam on both visible-tail variants seen in the `150000` trace: `t_cycle = 147715` (`VisibleTile2 pixel_index = 1`) and `t_cycle = 149995` (`VisibleTile2 pixel_index = 2`). In both cases, the current local model already keeps the remaining `VisibleTile2` tail pixels live and still retargets the fetched `VisibleTile3` slice. Treat those late visible-tail seams as covered; do not re-enter there first.
- The same `ly = 36` trace window now gives a tighter startup ownership map: `StartupAlignmentFill` still backs the first visible pixels (`x = 0..7`) while `VisibleTile2` is already becoming the next real cached slice, the first visible `VisibleTile2` pixel appears at `x = 8`, and traced `FF40` writes land both just after the first visible left-edge pixel (`visible_pixels_output = 1`) and later in the same span (`visible_pixels_output = 9`). Repo-local tests now lock both `LCDC.4` seams, and the fix required separating "tile-data selector changed" from "current row changed" so cached slices can reread the same row on `LCDC.4` but still use live `SCY/LY` on `SCY`.
- That new local closure did not move the external `m3_lcdc_tile_sel_change` oracle: the fixture diff stayed at `1475` mismatching pixels. Treat that as evidence that the remaining `tile_sel` debt is no longer the original `VisibleTile2` handoff itself, but another repeated left-edge/startup-continuation seam later in the frame.
- A new sprite-coupled repo-local regression now locks the `ly = 10` left-edge signature seen in the ROM trace (`selected_sprites = 1`, `current_transfer_x = 1`, `startup_dummy_pixels = 7`, `startup_fifo_placeholders = 7`). The same test also shows that once panel blanking is lifted, the startup tail itself renders the correct signed tile-data pixel at `x = 0`; the internal `Mode 3` seam is green there.
- Actual ROM traces still show `visible_output = ForcedBlank` deep into the restarted raster (`ly = 48` in the `153500` trace window), so the remaining `m3_lcdc_tile_sel_change` delta is now better explained by the LCD restart / panel-blank lane than by another cached-slice or visible-FIFO retargeting bug.
- **Highest-value next step:** pivot the next re-entry from cached-slice live-write closure to the LCD restart lane. Use the repeated `FF40 0x83 -> 0x93` scanline writes only as a locator for the problematic restarted lines, then audit `blank_frame_active`, `visible_output`, and the restart-frame boundary before retrying any broader `LCDC.4` / `SCY` or visible-FIFO change.
- The new DMG maturity ladder in `PPU.md` shows the current PPU state is not monotonic. Strictly, the first still-open step is `order 2` (`daid/ppu_scanline_bgp.gb`), so the ladder says the emulator cannot yet be treated as "past the early raster baseline" even though many later and narrower families are already green.
- Current ladder snapshot:

| order | case | current state | likely gap | next action |
| --- | --- | --- | --- | --- |
| 2 | `daid/ppu_scanline_bgp.gb` | red | visible raster / per-scanline `BGP` baseline still differs from the fixture set; the captured framebuffer keeps the global face silhouette but introduces scanline-dependent inner stripes, so this is not a gross post-boot VRAM seed failure like `hacktix/bully.gb` | promote this case to an active gate and compare the three fixture variants against local scanline output before resuming broader hi-fi work |
| 15 | `mooneye acceptance/ppu/intr_2_mode0_timing_sprites.gb` | red | sprite-coupled `Mode 2 -> 0` timing still diverges even though `intr_2_mode0_timing.gb` is green; likely gap is OBJ stall / arbitration influence on variable `Mode 3` end and `mode0_start_dot` | treat this as the first sprite-coupled timing closure after the LCD restart lane, not as a generic `STAT` regression |
| 16 | `mooneye acceptance/ppu/lcdon_timing-GS.gb` | red | LCD restart lane still mismatches the oracle on early restarted lines; rerun still ends in active raster with `visible_output=Driving`, consistent with the open `blank_frame_active` / restart-boundary debt | keep LCD restart / `visible_output` as the highest-value next re-entry |
| 17 | `mooneye acceptance/ppu/lcdon_write_timing-GS.gb` | red | same restart-lane family as `lcdon_timing-GS`, but now through `LCDC.7` write chronology | close together with `lcdon_timing-GS`; do not split the restart lane into two unrelated tasks |
- Practical maturity reading:
  - Strict ladder maturity: blocked at `order 2`.
  - Real subsystem maturity: already beyond the early raster baseline in several later areas (`bully`, `mem_oam`, `sprite_priority`, most mooneye `STAT`, `oam_bug`, `strikethrough`, `m2_win_en_toggle`), but with four earlier holes still open (`2`, `15`, `16`, `17`).
  - Consequence: `27+` mealybug cases stay valuable as sentinels, but they should not be treated as the primary closure target until those four ladder blockers are closed or intentionally waived with evidence.

#### Open TODOs

- [PPU][SKIPBOOT-ORACLE] `SkipBoot` startup-mode latch is validated only against repo-local continuity tests. Before Phase `9` hardening, needs comparison against a trusted oracle or hardware capture proving first LCD-visible dots after `SkipBoot` are coherent with published `LCDC`, `STAT`, and `LY` state. Does not block Phase `5`.

- [PPU][MEALYBUG-MODE3-LIVE-WRITES] Still-red follow-up families:
  - Low-`X` sprite/live-OBJ timing: `m3_bgp_change_sprites`, remaining `m3_obp0_change` delta.
  - Window/live-`LCDC.5` timing: `m2_win_en_toggle`, `m3_window_timing*`, `m3_lcdc_win_en_change_multiple*`, `m3_wx_4_change*`, `m3_wx_5_change`, `m3_wx_6_change`.
  - Live `LCDC` map/enable/tile-select: `m3_lcdc_bg_en_change`, `m3_lcdc_bg_map_change`, `m3_lcdc_win_map_change`, `m3_lcdc_tile_sel_change*`, `m3_lcdc_obj_en_change*`.
  - `SCX/SCY` live-scroll: `m3_scx_high_5_bits`, `m3_scx_low_3_bits`, `m3_scy_change`.
  - Live sprite-size: `m3_lcdc_obj_size_change`, `m3_lcdc_obj_size_change_scx`.

- [PPU][STARTUP-DUMMY-SEED-DEFERRED] A March 28 experiment moving the dummy-startup fill to discard-first-BG-fetch (docboy-style) improved `m3_lcdc_bg_map_change` (`978 -> 722`) but regressed raster tests, `acid/dmg-acid2.gb`, and `m3_scy_change` (`7266 -> 10099`). Confirms the remaining left-edge debt sits in the startup dummy/first-fetch seam, but the fix must preserve stable startup timing and `acid` baseline.

- [PPU][MODE3-PUSH-ARBITRATION-DEFERRED] A March 26 attempt at strict FIFO-empty BG push plus OBJ-start arbitration regressed multiple external families at once (`mooneye hblank_ly_scx_timing-GS`, `intr_2_mode0_timing`, `hacktix/strikethrough`, `mealybug m3_bgp_change`). Do not attempt another isolated "strict push" or "push-state-only OBJ start" change — the next slice must rewrite more of the shared BG/window/OBJ fetcher contract at once, with external report comparison as a hard gate.

- [PPU][WINDOW-GLITCH-ORACLE] `WX = 0` and `WX = 166` paths are tested but remain provisional. Needs stricter validation for `WX`/`WY`/`LCDC.5` mid-frame glitch behavior, including the DMG-specific `WX = 0 && (SCX & 7) > 0` path. Does not block Phase `5`; needed for Phase `9`.

- [PPU][LCDC2-8X16-ARTIFACTS] Core `8x16` rules and mid-frame `LCDC.2` shrink crash are fixed, but finer DMG-visible artifacts from mid-frame size changes remain open. Needs targeted ROM or oracle coverage. Does not block Phase `5`; needed for Phase `9`.

- [PPU][OAM-CORRUPTION-ORACLE] Deterministic unit/integration coverage is shipped for Mode `2` OAM access, `FEA0-FEFF` reads, `inc rr`, `[hli]`/`[hld]`, stack/interrupt paths, DMG variants, and CGB negative path. Still lacks independent oracle comparison. Curated `oam_bug` subset excludes `oam_bug.gb` multi-ROM and `7-timing_effect.gb`. Needed for Phase `9`.

- [PPU][MOONEYE-LCD-RESTART] `stat_lyc_onoff` is closed. `ppu/lcdon_timing-GS` and `ppu/lcdon_write_timing-GS` remain red — fine `LY/STAT` boundary mismatch around early restarted lines. LCD restart timing is not yet oracle-validated.

- [PPU][MOONEYE-STAT-TIMING] `hblank_ly_scx_timing-GS`, `intr_2_0_timing`, `vblank_stat_intr-GS` are now green. Remaining open case: `ppu/intr_2_mode0_timing_sprites` — sprite-coupled Mode `2 -> 0` timing is not fully closed.

- [PPU][FF44-HBLANK-SEAM] The exact DMG `FF44` advance point inside late HBlank is still hypothesis-only. The docs prefer the "last machine cycle of HBlank" wording, but a direct retune of the current implementation threshold to later dots regressed `mooneye acceptance/ppu/hblank_ly_scx_timing-GS` from green to red while leaving the rest of the model unchanged. Re-entry should start from a narrow trace or oracle comparison around the late-HBlank `LY/SCX` polling seam rather than from another blind constant change. Hard gate: `mooneye acceptance/ppu/hblank_ly_scx_timing-GS` must stay green.

#### Re-entry rules

- Resume from one failing family at a time; prefer the smallest oracle-backed reproduction that distinguishes the suspected same-T-cycle window.
- Capture baseline and final `/.roms/test/test-report.md` for any exploratory rerun, especially `mealybug-tearoom-dmg-curated`, `acid-dmg-curated`, and `mooneye-acceptance-dmg-curated`.
- Treat cached background slices already in `Push`, `fill.pending`, or the visible FIFO as the first suspect for remaining second/third-tile live-write failures.
- Do not reopen generic startup realignment, broad tilemap rereads, or broad cached-slice retargeting before a new trace proves the fault starts earlier than the cached-slice seam.
- When a candidate fix touches `STAT`, LCD restart, or sprite-coupled mode boundaries, rerun the narrow mooneye LCD timing slice before trusting any localized mealybug improvement.

### Phase 5 — Input and simple peripherals

- None currently.

### Phase 6 — Banked cartridges, special cartridges, and cartridge persistence

- None currently.

### Phase 7 — Audio

- [APU][CGB-CH3-WAVE-RAM-ACTIVE-MMIO] The current CH3 active wave-RAM MMIO contract is only specified for the DMG family. DMG coverage already locks the fetch-window policy and DMG retrigger-corruption lane, but CGB-family active-access redirection remains intentionally deferred because the repo scope is still DMG-only. Do not treat the current `ConsoleModel::Cgb` fallback path as hardware-accurate or add tests/docs that claim a final CGB contract before the CGB APU lane exists.
- [APU][EXTRA-LENGTH-CLOCKING-CGB-REVISION] CH1/CH2/CH3/CH4 extra-length clocking is now wired through an explicit per-model policy seam, but the current `ConsoleModel` surface still cannot distinguish the documented `CGB-02` exception from later CGB revisions. The code therefore keeps the generic DMG/later-CGB rule as a conservative fallback even for `ConsoleModel::Cgb`. Do not claim revision-accurate CGB extra-length clocking until a revision-scoped model or stronger oracle closes that gap.
- [APU][SKIPBOOT-HIDDEN-STATE] Direct boot currently reconstructs the visible audio snapshot, powered state, wave-RAM startup policy, channel-active mask, and shared-divider-derived `DIV-APU` phase, but it still resets other hidden APU state from repo-local defaults. HPF history, pulse duty-step/timer continuation, CH3 sample-buffer/sample-index continuation, and CH4 LFSR/noise-timer continuation are not yet verified boot-handoff state. Keep docs/tests explicit about that narrower contract until a stronger oracle or hardware-backed startup model closes the gap.
- [APU][ZOMBIE-MODE-REVISION-MATRIX] CH1/CH2/CH4 now model the cross-revision-consistent manual increment path for live `NR12` / `NR22` / `NR42` writes (`increase` with pace `0` increments the current volume modulo `16`), but the broader zombie-mode write matrix still varies by hardware revision. Do not claim a fully solved DMG zombie-mode contract until a revision-scoped oracle or hardware-backed policy closes the rest of that matrix.
- [APU][DAC-OFF-FADE-MODEL] The DMG baseline now models the Pan Docs "all DACs off" output disconnect by clamping post-HPF output to `0` and freezing HPF capacitor evolution until some DAC is re-enabled, but the per-channel DAC-off transition toward analog `0` is still treated as an immediate local step rather than as the documented model-dependent fade. Keep docs/tests explicit that only the all-DACs-off disconnect is claimed today until a stronger oracle or hardware-backed fade policy closes the remaining DAC-off analog path.

### Phase 8 — Full emulator save states and global serialization strategy

- None currently.

### Phase 9 — Final DMG hardening, differential validation, and closure

- None currently.
