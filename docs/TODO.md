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
- Work stopped before closing the main oracle debt. Still-open families are:
  - remaining `Mode 3` live-write families: `m3_lcdc_bg_map_change`, `m3_lcdc_tile_sel_change`, `m3_lcdc_tile_sel_win_change`, `m3_scy_change`
- Last stable external measurements for the still-open live-write sentinels are: `m3_lcdc_bg_map_change: 674`, `m3_lcdc_tile_sel_change: 1410`, `m3_lcdc_tile_sel_win_change: 1232`, `m3_scy_change: 7819`.
- Visible-FIFO cached-slice ownership is explicit and survives `fill.pending -> FIFO` plus startup placeholders. Broad visible-FIFO retargeting was already tried and reverted: it regressed the external oracles instead of moving them. Treat that as settled evidence that the remaining debt is not solved by broad late visible-pop rereads.
- The important remaining left-edge/live-write evidence is stable:
  - `LCDC.3` late-tail ownership around `VisibleTile2/VisibleTile3` is already covered by repo-local tests and still does not move the external oracle.
  - `LCDC.4` / `SCY` visible-FIFO retargeting regressed the target families and was reverted.
  - current first mismatches stay at `m3_lcdc_bg_map_change -> x = 12, y = 0`, `m3_lcdc_tile_sel_change -> x = 0, y = 10`, `m3_scy_change -> x = 1, y = 0`.
  - working hypothesis remains: the unresolved debt sits earlier, around startup dummy / first-fetch / restart-lane behavior, not in another broad visible-FIFO retargeting pass.
- The early visible-raster baseline `daid/ppu_scanline_bgp.gb` is now green again. The winning slice stayed narrow:
  - keep the DMG CPU-path `BGP` panel delay (`4` visible BG pixels) for same-line output
  - additionally repaint the previous visible boundary line when the next visible line starts a new per-scanline `BGP` row family, using that next-line `BGP` sequence over the previous line's already-composed mixed pixels
  - keep that seam panel-only and DMG-only; raw `current_scanline_pixels`, fetcher timing, and the shared `Mode 3` ownership model stay unchanged
  - hard regression gates remained green through the closure: `hacktix/strikethrough.gb`, `mooneye acceptance/ppu/intr_2_mode0_timing_sprites.gb`, and `cargo test -p gb-core ppu -- --nocapture`
- `PpuSnapshot` and the scheduler trace already export the useful re-entry state: transfer lane, source window, backing, readiness, startup seam, and visible-FIFO cached-slice metadata. Do not add new broad tracing until one of those existing fields fails to discriminate the next oracle.
- The Donkey Kong desktop gate remains stable after the refactor slices that landed the new ownership model. No current evidence says the remaining open PPU debt is the source of a desktop slowdown regression.
- `hacktix/strikethrough.gb`, `blargg oam_bug/4`, `blargg oam_bug/5`, and `mooneye acceptance/ppu/intr_2_mode0_timing_sprites.gb` are green again. The `intr_2_mode0_timing_sprites` closure came from narrower CPU-visible `STAT` publication seams for the ten-sprite step-8 staggered families, not from another broad `Mode 3` runtime rewrite.
- The repo-local wide gate is aligned again: `cargo test -p gb-core ppu -- --nocapture` is green after pruning stale startup/right-edge same-`X` local oracles and retuning the remaining expectations to the current contract.
- **Highest-value next step:** the early raster baseline is closed again, so move the active frontier back to the still-red live-write families (`m3_lcdc_bg_map_change`, `m3_lcdc_tile_sel_change`, `m3_lcdc_tile_sel_win_change`, `m3_scy_change`) with `daid/ppu_scanline_bgp.gb`, `intr_2_mode0_timing_sprites.gb`, and `strikethrough.gb` preserved as hard regression gates.
- The new DMG maturity ladder in `PPU.md` is now monotonic up to the live-write tranche. With `daid/ppu_scanline_bgp.gb` green again, the first strict ladder blocker moves to the hi-fi `Mode 3` group at `order 27`.
- The DMG OAM-corruption path now matches the hardware/document baseline more closely too: trigger classification is address-family / trigger-family based, while the PPU remains the owner of the live `Mode 2` mode-and-row gate. That restores the last-row and first-scanline windows in `blargg oam_bug/4-scanline_timing.gb` and `oam_bug/5-timing_bug.gb` without reopening the phase-4 synthetic OAM-corruption fixtures.
- Current ladder snapshot:

| order | case | current state | likely gap | next action |
| --- | --- | --- | --- | --- |
| 2 | `daid/ppu_scanline_bgp.gb` | green | closed through the DMG CPU-path `BGP` previous-line boundary repaint seam; keep it as the early raster regression gate while touching panel-path palette behavior | do not reopen broad `BGP` carry experiments without rerunning this ROM plus `strikethrough` and `intr_2_mode0_timing_sprites` |
| 15 | `mooneye acceptance/ppu/intr_2_mode0_timing_sprites.gb` | green | sprite-coupled `Mode 2 -> 0` timing is closed again; the winning slice was a narrow CPU-visible `STAT` publication model for the ten-sprite step-8 staggered reduced-caller families (`x00..x07`, `x48..x49`) that the full ROM depends on | keep this ROM plus `hacktix/strikethrough.gb` as hard regression gates while moving back to the raster baseline and live-write debt |
| 16 | `mooneye acceptance/ppu/lcdon_timing-GS.gb` | green | the dedicated CPU-path LCD-enable read probe is now green for `LY`, `STAT` with `LYC=0/1`, OAM, and VRAM; the last remaining mismatch was the coincidence bit on the first CPU-visible dot of a new line after restart, and closing that seam makes the external ROM pass | keep the repo-local read probe and first-dot coincidence seam explicit; move the next primary focus off the LCD restart lane |
| 17 | `mooneye acceptance/ppu/lcdon_write_timing-GS.gb` | green | the dedicated CPU-path `LCDC.7` write probe is now green and the external mooneye case passes after splitting CPU-visible OAM-write publication from the owner bus state and opening the OAM-only write window only at scanline start and the exact `Mode 2 -> 3` boundary | keep the new OAM-write publication seam fixed and use `lcdon_timing-GS` as the remaining LCD-restart oracle |
- Practical maturity reading:
  - Strict ladder maturity: blocked at `order 27`.
  - Real subsystem maturity: the early raster baseline is now closed alongside `bully`, `mem_oam`, `sprite_priority`, most mooneye `STAT`, `oam_bug`, `strikethrough`, `intr_2_mode0_timing_sprites`, and `m2_win_en_toggle`.
  - Consequence: the remaining hi-fi mealybug/live-write tranche is now the primary closure target rather than just a sentinel set.

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

- [PPU][OAM-CORRUPTION-ORACLE] Deterministic unit/integration coverage is shipped for Mode `2` OAM access, `FEA0-FEFF` reads, `inc rr`, `[hli]`/`[hld]`, stack/interrupt paths, DMG variants, and CGB negative path. The last-row and first-scanline blargg windows (`oam_bug/4`, `oam_bug/5`) are green again after moving trigger classification away from the coarse blocked-access flag and back to live `Mode 2` ownership in the PPU. Still lacks independent oracle comparison. Curated `oam_bug` subset excludes `oam_bug.gb` multi-ROM and `7-timing_effect.gb`. Needed for Phase `9`.

- [PPU][FF44-HBLANK-SEAM] The exact DMG `FF44` advance point inside late HBlank is still hypothesis-only. The docs prefer the "last machine cycle of HBlank" wording, but a direct retune of the current implementation threshold to later dots regressed `mooneye acceptance/ppu/hblank_ly_scx_timing-GS` from green to red while leaving the rest of the model unchanged. Re-entry should start from a narrow trace or oracle comparison around the late-HBlank `LY/SCX` polling seam rather than from another blind constant change. Hard gate: `mooneye acceptance/ppu/hblank_ly_scx_timing-GS` must stay green.

#### Re-entry rules

- Resume from one failing family at a time; prefer the smallest oracle-backed reproduction that distinguishes the suspected same-T-cycle window.
- Capture baseline and final `/.roms/test/test-report.md` for any exploratory rerun, especially `mealybug-tearoom-dmg-curated`, `acid-dmg-curated`, and `mooneye-acceptance-dmg-curated`.
- Treat cached background slices already in `Push`, `fill.pending`, or the visible FIFO as a likely suspect only for the remaining live-write families. Do not let that heuristic override a better active oracle such as `daid/ppu_scanline_bgp.gb` or the still-red mealybug live-write sentinels.
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
