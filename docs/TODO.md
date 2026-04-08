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
- Remaining debt narrowed to cached background slices already in `Push` or `fill.pending` after startup. Broad startup retuning, broad tilemap rereads, and broad cached-slice `LCDC.3` retargeting all regressed the oracle — do not retry those first.
- **Highest-value next step:** cached-slice live-write timing for the second and especially third visible BG tile after startup (`x = 16..23`), using the current cached-slice origin and same-T-cycle window machinery.

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
- [APU][SKIPBOOT-HIDDEN-STATE] Direct boot currently reconstructs the visible audio snapshot, powered state, wave-RAM startup policy, channel-active mask, and shared-divider-derived `DIV-APU` phase, but it still resets other hidden APU state from repo-local defaults. HPF history, pulse duty-step/timer continuation, CH3 sample-buffer/sample-index continuation, and CH4 LFSR/noise-timer continuation are not yet verified boot-handoff state. Keep docs/tests explicit about that narrower contract until a stronger oracle or hardware-backed startup model closes the gap.
- [APU][ZOMBIE-MODE-REVISION-MATRIX] CH1/CH2/CH4 now model the cross-revision-consistent manual increment path for live `NR12` / `NR22` / `NR42` writes (`increase` with pace `0` increments the current volume modulo `16`), but the broader zombie-mode write matrix still varies by hardware revision. Do not claim a fully solved DMG zombie-mode contract until a revision-scoped oracle or hardware-backed policy closes the rest of that matrix.
- [APU][DAC-OFF-FADE-MODEL] The DMG baseline now models the Pan Docs "all DACs off" output disconnect by clamping post-HPF output to `0` and freezing HPF capacitor evolution until some DAC is re-enabled, but the per-channel DAC-off transition toward analog `0` is still treated as an immediate local step rather than as the documented model-dependent fade. Keep docs/tests explicit that only the all-DACs-off disconnect is claimed today until a stronger oracle or hardware-backed fade policy closes the remaining DAC-off analog path.

### Phase 8 — Full emulator save states and global serialization strategy

- None currently.

### Phase 9 — Final DMG hardening, differential validation, and closure

- None currently.
