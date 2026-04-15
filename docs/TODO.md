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
- The current external report snapshot is `.roms/test/test-report.md = 149/167`: `147` passed, `18` known failing, and `2` informational (`acid/which.gb`, `daid/rom_and_ram.gb`). `make ci` and `make test-roms` are green at this checkpoint.
- The baseline PPU smoke gates must stay green before and after any PPU advance:
  - `acid/dmg-acid2.gb` (`VERY LOW`, order `2`): base raster / smoke coverage for general `Mode 3` raster, BG/WIN/OBJ mixing, and left-edge startup behavior.
  - `daid/ppu_scanline_bgp.gb` (`MEDIUM`, order `41`): visible raster and post-boot state coverage for per-scanline `BGP`.
  - `hacktix/bully.gb` (`HIGH`, order `139`): visible raster and post-boot state coverage for visible VRAM / tilemap seed after boot.
- The current external PPU-green snapshot is:
  - `acid/dmg-acid2.gb`
  - curated `blargg oam_bug/{1-lcd_sync,2-causes,3-non_causes,4-scanline_timing,5-timing_bug,6-timing_no_bug,8-instr_effect}.gb`
  - `daid/ppu_scanline_bgp.gb`
  - `mooneye acceptance/ppu/{hblank_ly_scx_timing-GS,intr_1_2_timing-GS,intr_2_0_timing,intr_2_mode0_timing,intr_2_mode0_timing_sprites,intr_2_mode3_timing,intr_2_oam_ok_timing,lcdon_timing-GS,lcdon_write_timing-GS,stat_irq_blocking,stat_lyc_onoff,vblank_stat_intr-GS}.gb`
  - `hacktix/{bully,strikethrough}.gb`
  - `mealybug ppu/{m2_win_en_toggle,m3_bgp_change,m3_bgp_change_sprites,m3_obp0_change,m3_scx_low_3_bits,m3_scx_high_5_bits}.gb`
- The next strict ladder blocker is `m3_scy_change.gb` (`order 32`). This should be the next ROM test to attack because `m3_scx_low_3_bits.gb` and `m3_scx_high_5_bits.gb` are now green, leaving `SCY` as the last Tier B `SCX/SCY` FIFO-core case before moving into the broader `LCDC` and window live-write tiers.
- The DMG visible-transfer path now always consumes one BG FIFO pixel even when `LCDC.0 = 0` forces the presented BG/window color to white. That narrow change does not close `m3_lcdc_bg_en_change.gb`, but it lowers the framebuffer mismatch from `9398` pixels to `1556` while keeping the current green guards (`acid/dmg-acid2.gb`, `mealybug m3_bgp_change.gb`, `mealybug m3_bgp_change_sprites.gb`, `mealybug m3_obp0_change.gb`, `mooneye acceptance/ppu/hblank_ly_scx_timing-GS.gb`) green.
- A follow-up DMG pixel-path split now treats `LCDC.0` BG/window enable as live during visible output while leaving `LCDC.1` OBJ enable on the existing delayed-copy path. That moves `m3_lcdc_bg_en_change.gb` from `1556` to `1294` mismatching pixels and keeps the current green guards (`acid/dmg-acid2.gb`, `mealybug m3_bgp_change.gb`, `mealybug m3_bgp_change_sprites.gb`, `mealybug m3_obp0_change.gb`, `mooneye acceptance/ppu/hblank_ly_scx_timing-GS.gb`, `cargo test -q`) green.
- The main broad failure modes are already ruled out:
  - broad visible-FIFO cached-slice retargeting regressed the external oracles instead of moving them
  - `LCDC.4` / `SCY` visible-FIFO retargeting regressed the target families and was reverted
  - current first mismatches still stay at `m3_lcdc_bg_map_change -> x = 12, y = 0`, `m3_lcdc_tile_sel_change -> x = 0, y = 10`, `m3_scy_change -> x = 1, y = 0`
  - working hypothesis remains: the unresolved debt sits earlier, around startup dummy / first-fetch / restart-lane behavior, not in another broad visible-FIFO retargeting pass
- The most informative green regression gates inside that snapshot are:
  - `daid/ppu_scanline_bgp.gb` is closed through the narrow DMG CPU-path `BGP` previous-line boundary repaint seam; keep that seam panel-only and DMG-only
  - `mealybug m3_bgp_change.gb` is closed through an explicit DMG CPU-path split: the first visible-line `BGP` CPU write still uses the retroactive panel path while `visible_pixels_output == 0`, `current_transfer_x == 0`, and no sprites were selected; later writes only stay retroactive when the recent visible BG tail is all color `0`, and previous-line boundary repaint only comes from delayed pipeline-visible writes
  - `mealybug m3_bgp_change_sprites.gb` is now closed through a narrower DMG single-left-sprite `BGP` seam: the first two CPU-path writes use sprite-position-dependent visible onsets, and the second write keeps a short left-edge transient range before the final palette wins
  - `mealybug m3_obp0_change.gb` is closed through a separate DMG `OBP0` scanline-path seam: no retroactive OBJ recolor before `visible_x = 10`, then a scanline-anchored conflict window that preserves older isolated leading OBJ pixels while still recoloring the later live-write tail with the committed palette; removing the older first-pixel transient `OR` recolor kept the current guard-set report unchanged
  - `mooneye acceptance/ppu/intr_2_mode0_timing_sprites.gb` is closed through narrow CPU-visible `STAT` publication seams for the ten-sprite step-8 staggered families, not through another broad `Mode 3` rewrite
  - `hacktix/strikethrough.gb`, `mooneye acceptance/ppu/hblank_ly_scx_timing-GS.gb`, `mooneye acceptance/ppu/lcdon_timing-GS.gb`, `mooneye acceptance/ppu/lcdon_write_timing-GS.gb`, `blargg oam_bug/4`, `blargg oam_bug/5`, and `cargo test -p gb-core ppu -- --nocapture` are green again
- Strict ladder maturity is blocked at `order 32`, but the early raster, restart, and sprite-coupled `BGP` live-write baselines are closed again.
- The next primary closure target is the remaining `SCX/SCY` FIFO core tranche starting at `m3_scy_change.gb`.
- A narrow April 14 correction removed `SCY` live tile-data refetch from already-materialized background `push` / `fill` slices. DMG still re-samples `SCY` naturally on the active BG fetcher between `TileDataLow` and `TileDataHigh`, but pending cached slices now keep the bytes they already fetched. The minimum guard set stayed green, while `m3_scy_change.gb` remained red, so the remaining debt is not just "pending-slice refetch erases SCY bitplane desync".
- A second April 14 correction delayed the initial `SCX & 7` capture from `Mode 3` entry to the post-dummy-fetch startup dot, matching the narrower `m3_scx_low_3_bits` hypothesis better than the old "capture immediately at line start" model. The local startup/transfer tests and the external guard set stayed green, but `m3_scx_low_3_bits.gb` remained red, so the remaining debt is not just "SCX low bits were sampled too early".
- April 14 follow-up diagnostics on the real `m3_scx_low_3_bits.gb` ROM showed two distinct `FF43 <- 0x02` CPU-commit windows: rows `0..=71` write at `line_dot = 88` while the startup seam is already `PostAlignment` (`bg_current_transfer_x = 5`, `bg_fetcher_stage = TileDataLow`), and rows `72..=143` write at `line_dot = 84` while the seam is still `AlignmentSeedPending` (`bg_current_transfer_x = 1`, `bg_fetcher_stage = TileDataHigh`). A repo-local reclassification fix now lets early `SCX` retunes map consumed startup progress back from `hidden` into `discard`, matching the `SCX = 2` startup baseline more closely in unit tests, but the real ROM framebuffer still stays all-white on the right edge. That leaves the main blocker as a CPU-commit / startup-seam timing gap, not a generic `SCX` low-bit discard formula anymore.
- A later April 14 re-entry closed `m3_scx_low_3_bits.gb` without another PPU timing change: the curated runner now seeds the DMG boot trademark tile (`tile $19`) for that ROM under `SkipBoot`, matching the ROM's actual dependency on the post-boot logo tile while it writes only the tilemap entry itself. The previous all-white right edge was therefore a startup-memory contract gap in the runner, not the remaining `SCX` FIFO-core blocker.
- A follow-up April 14 pass on `m3_scx_high_5_bits.gb` added explicit `SCX` tile-column live-retarget coverage for background `push` / `fill` cached slices and for the startup/ordinary current BG fetch carry path. Local fetch tests now pin the intended distinction between `SCX` low-bit-only writes and tile-column changes, and the narrow guard set stayed green, but the real ROM still fails. The first framebuffer mismatch did move from `x = 16, y = 8` to `x = 16, y = 24`, so the remaining debt is narrower than the original "SCX high bits are never retargeted" hypothesis.
- An April 14 re-entry then tested the tempting next seam, visible-FIFO `SCX` retarget, against the real `m3_scx_high_5_bits.gb` write chronology and rejected it. The real ROM writes `FF43` on the failing rows at `line_dot = 112` with `visible_pixels_output = 8`, `bg_fetcher_stage = TileDataHigh`, `bg_fetcher_stage_dot = 0`, and the FIFO front still tagged as `StartupContinuationVisibleTile2`; however, the external framebuffer mismatch still begins at `x = 16`, not at the second visible startup tile. A narrow visible-FIFO retarget experiment therefore worsened the target (`204 -> 213` mismatching pixels, first mismatch `x = 15, y = 24`) and was reverted. Keep the new repo-local unit that proves the current-fetcher `VisibleTile3` carry path can recompute its `push.cached` slice after an `SCX` tile-column change, but do not retry another visible-FIFO `SCX` retarget without a stronger oracle. The remaining suspect is the startup `push -> fill -> output` boundary around the carried third visible tile, not the already-visible FIFO slice itself.
- A later April 14 re-entry narrowed that startup-boundary hypothesis to a specific DMG `SCX` class in `m3_scx_high_5_bits.gb`: rows `24..=30` write `FF43` at `line_dot = 112` with `bg_current_transfer_x = 16`, `visible_pixels_output = 8`, `bg_fetcher_stage = TileDataHigh`, `bg_fetcher_stage_dot = 0`, and a `PostAlignment` seam whose next continuation slice is `VisibleTile3`. Carrying a `+1 tile` full-refetch offset globally for `VisibleTile3` fixed that band but opened earlier rows; keeping it as a narrow write-time seam only for that late `High0` window reduced the ROM from `204` to `189` mismatching pixels while keeping `mealybug/m3_bgp_change.gb` and `mooneye/hblank_ly_scx_timing-GS.gb` green. The remaining debt is whatever classes still produce the residual mismatches on rows `25..=38`, not the already-ruled-out broad `VisibleTile3` rule.
- A final April 14 pass closed `m3_scx_high_5_bits.gb`. The retained fix is not another broad FIFO rewrite: it keeps the late `High0` `VisibleTile3` old-tail seam for rows `24..=30`, and adds a second narrow DMG-only low-band carry correction for rows `8..=14` (`line_dot = 112`, `visible_pixels_output = 13`, `bg_current_transfer_x = 21`, FIFO front still `StartupContinuationVisibleTile2`). In that low-band class, only the carried third visible tile is adjusted: low bits `{0,6}` reuse the next pixel already cached in that carried slice, low bits `{1,5}` reuse the next two-bit position from the same carried slice, and low bit `3` reuses the already-validated old-`SCX` next-tile retarget one pixel earlier. `m3_scx_high_5_bits.gb`, `mealybug/m3_bgp_change.gb`, `mealybug/m3_bgp_change_sprites.gb`, `mealybug/m3_obp0_change.gb`, `daid/ppu_scanline_bgp.gb`, `mooneye/hblank_ly_scx_timing-GS.gb`, `mooneye/lcdon_timing-GS.gb`, `mooneye/lcdon_write_timing-GS.gb`, `hacktix/strikethrough.gb`, and the local `ppu::tests::mode3::fetch` / `ppu::tests::palette` suites stayed green with that closure.
- An April 15 Acid2 guard re-entry found a materialized startup placeholder leaking into the first visible BG pixel on the footer lines (`SCX = $F3`, `mode0_start_dot = 255`). The retained fix only drops the single-placeholder residual FIFO case whose cached sideband is `None` before visible BG output; multi-placeholder startup tails remain timing-visible for the `SCX=6/7` Mooneye threshold. This preserves synthetic live-write handoff states and keeps `dmg-acid2.gb`, `m3_scx_low_3_bits.gb`, `m3_scx_high_5_bits.gb`, and local `ppu::tests::mode3` green.
- Validation checkpoint: `make ci` passes; `make test-roms` passes after materializing all curated DMG families. The generated report has no unexpected red outside the known Mealybug Mode 3 live-write ladder.

#### Open TODOs

- [PPU][SKIPBOOT-ORACLE] `SkipBoot` startup-mode latch is validated only against repo-local continuity tests. Before Phase `9` hardening, needs comparison against a trusted oracle or hardware capture proving first LCD-visible dots after `SkipBoot` are coherent with published `LCDC`, `STAT`, and `LY` state. Does not block Phase `5`.

- [PPU][MEALYBUG-MODE3-LIVE-WRITES] Current report still-red follow-up families (implementation order per PPU.md ladder). These are the `18` red entries in the latest `make test-roms` report:
  - **Tier B — SCX/SCY (FIFO core)** `[orders 31-32]`: `m3_scy_change`.
  - **Tier C — LCDC BG toggles** `[orders 33-35]`: `m3_lcdc_bg_en_change`, `m3_lcdc_bg_map_change`, `m3_lcdc_tile_sel_change`.
  - **Tier D — LCDC OBJ toggles** `[orders 36-39]`: `m3_lcdc_obj_en_change`, `m3_lcdc_obj_en_change_variant`, `m3_lcdc_obj_size_change`, `m3_lcdc_obj_size_change_scx`.
  - **Tier E — Window mechanics** `[orders 40-49]`: `m3_window_timing`, `m3_window_timing_wx_0`, `m3_lcdc_win_map_change`, `m3_lcdc_tile_sel_win_change`, `m3_lcdc_win_en_change_multiple`, `m3_lcdc_win_en_change_multiple_wx`, `m3_wx_4_change`, `m3_wx_5_change`, `m3_wx_6_change`, `m3_wx_4_change_sprites`.
  - Immediate next target: `mealybug-tearoom-tests/ppu/m3_scy_change.gb`. Start from the existing `SCY` bitplane-desync hypothesis, but do not retry pending `push` / `fill` tile-data refetch suppression as the whole fix; that was already applied narrowly and the ROM stayed red.

- [PPU][M3-LCDC0-LEFT-EDGE-ONSET] `m3_lcdc_bg_en_change.gb` is still red after fixing the broader bug where `LCDC.0 = 0` stopped consuming BG FIFO pixels. The remaining mismatch is now concentrated in the left edge and the first four live `FF40` toggles per tested scanline. Trace re-entry points:
  - On `LY = 1`, the ROM commits `FF40 <- {0x92, 0x93, 0x92, 0x93}` at `visible_pixels_output = {4, 16, 24, 32}` / `line_dot = {104, 116, 124, 132}`.
  - The first toggle lands while `StartupAlignmentFill` still fronts the FIFO (`pixel_index = 4`) and `VisibleTile2` is queued in `Push`; the second lands with `VisibleTile3` at the FIFO front; the third and fourth already hit ordinary tiles.
  - Two repo-local observability signatures now pin the same four-write cadence across distinct startup classes: a lower-mismatch class reaches the first write with `StartupAlignmentFill pixel_index = 3` and `startup_fifo_placeholders = 3`, then progresses through `VisibleTile2 pixel_index = 7`, `VisibleTile3 pixel_index = 7`, and `Ordinary pixel_index = 7`; the current worst band reaches the first write with `StartupAlignmentFill pixel_index = 6` and `startup_fifo_placeholders = 0`, then progresses through `VisibleTile3 pixel_index = 2` and `Ordinary pixel_index = 2`.
  - In that worst-band class, the external framebuffer mismatch already begins before the first `FF40` write on the line, so the next fix should not assume a pure per-write `LCDC.0` onset bug. Re-entry should treat the write cadence as a probe of an earlier startup-visible left-edge timing class.
  - A follow-up experiment that only pinned `StartupAlignmentFill` to the pre-write `LCDC.0` state regressed the target from `1556` to `1773` mismatching pixels. Do not retry another fill-only override. The next slice should localize the onset rules across those four write points together, not just across the alignment-fill tail.
  - A broader experiment that pinned already-materialized BG slices plus `push/fill pending` slices to the pre-write `LCDC.0` state regressed the target to `3617` mismatching pixels. Do not retry a generic "materialized slices keep old BG enable" rule.

- [PPU][STARTUP-DUMMY-SEED-DEFERRED] A March 28 experiment moving the dummy-startup fill to discard-first-BG-fetch (docboy-style) improved `m3_lcdc_bg_map_change` (`978 -> 722`) but regressed raster tests, `acid/dmg-acid2.gb`, and `m3_scy_change` (`7266 -> 10099`). Confirms the remaining left-edge debt sits in the startup dummy/first-fetch seam, but the fix must preserve stable startup timing and `acid` baseline.

- [PPU][MODE3-PUSH-ARBITRATION-DEFERRED] A March 26 attempt at strict FIFO-empty BG push plus OBJ-start arbitration regressed multiple external families at once (`mooneye hblank_ly_scx_timing-GS`, `intr_2_mode0_timing`, `hacktix/strikethrough`, `mealybug m3_bgp_change`). Do not attempt another isolated "strict push" or "push-state-only OBJ start" change — the next slice must rewrite more of the shared BG/window/OBJ fetcher contract at once, with external report comparison as a hard gate.

- [PPU][WINDOW-GLITCH-ORACLE] `WX = 0` and `WX = 166` paths are tested but remain provisional. Needs stricter validation for `WX`/`WY`/`LCDC.5` mid-frame glitch behavior, including the DMG-specific `WX = 0 && (SCX & 7) > 0` path. Does not block Phase `5`; needed for Phase `9`.

- [PPU][LCDC2-8X16-ARTIFACTS] Core `8x16` rules and mid-frame `LCDC.2` shrink crash are fixed, but finer DMG-visible artifacts from mid-frame size changes remain open. Needs targeted ROM or oracle coverage. Does not block Phase `5`; needed for Phase `9`.

- [PPU][OAM-CORRUPTION-ORACLE] Deterministic unit/integration coverage is shipped for Mode `2` OAM access, `FEA0-FEFF` reads, `inc rr`, `[hli]`/`[hld]`, stack/interrupt paths, DMG variants, and CGB negative path. The last-row and first-scanline blargg windows (`oam_bug/4`, `oam_bug/5`) are green again after moving trigger classification away from the coarse blocked-access flag and back to live `Mode 2` ownership in the PPU. Still lacks independent oracle comparison. Curated `oam_bug` subset excludes `oam_bug.gb` multi-ROM and `7-timing_effect.gb`. Needed for Phase `9`.

- [PPU][FF44-HBLANK-SEAM] The exact DMG `FF44` advance point inside late HBlank is still hypothesis-only. The docs prefer the "last machine cycle of HBlank" wording, but a direct retune of the current implementation threshold to later dots regressed `mooneye acceptance/ppu/hblank_ly_scx_timing-GS` from green to red while leaving the rest of the model unchanged. Re-entry should start from a narrow trace or oracle comparison around the late-HBlank `LY/SCX` polling seam rather than from another blind constant change. Hard gate: `mooneye acceptance/ppu/hblank_ly_scx_timing-GS` must stay green.

#### Re-entry rules

- Resume from one failing family at a time; prefer the smallest oracle-backed reproduction that distinguishes the suspected same-T-cycle window.
- Capture baseline and final `/.roms/test/test-report.md` for exploratory reruns, especially `mealybug-tearoom-dmg-curated`, `acid-dmg-curated`, and `mooneye-acceptance-dmg-curated`.
- Always rerun the baseline PPU smoke gates (`acid/dmg-acid2.gb`, `daid/ppu_scanline_bgp.gb`, `hacktix/bully.gb`) before accepting any PPU behavior change, even if the local target ROM improves.
- Keep at least `acid/dmg-acid2.gb`, `daid/ppu_scanline_bgp.gb`, `mealybug ppu/m3_bgp_change.gb`, `mealybug ppu/m3_bgp_change_sprites.gb`, `mealybug ppu/m3_obp0_change.gb`, `mealybug ppu/m3_scx_low_3_bits.gb`, `mealybug ppu/m3_scx_high_5_bits.gb`, `mooneye acceptance/ppu/hblank_ly_scx_timing-GS.gb`, `mooneye acceptance/ppu/intr_2_mode0_timing_sprites.gb`, `mooneye acceptance/ppu/lcdon_timing-GS.gb`, `mooneye acceptance/ppu/lcdon_write_timing-GS.gb`, `hacktix/strikethrough.gb`, `blargg oam_bug/4-scanline_timing.gb`, and `blargg oam_bug/5-timing_bug.gb` as the minimum no-regression set while touching panel-path palette behavior, startup/restart timing, sprite-coupled mode boundaries, `SCX/SCY`, or remaining live-write families.
- Do not reopen generic startup realignment, broad tilemap rereads, broad cached-slice / visible-FIFO retargeting, or isolated "strict push" experiments before a new oracle shows the fault starts there.
- When a candidate fix touches `STAT`, LCD restart, or sprite-coupled mode boundaries, rerun the narrow mooneye LCD timing slice before trusting any localized improvement.

### Phase 5 — Input and simple peripherals

- None currently.

### Phase 6 — Banked cartridges, special cartridges, and cartridge persistence

- [CARTRIDGE][MBC3-LATCH-RELATCH-POLICY] MBC3 currently keeps a deliberate compatibility deviation for `cpp/latch-rtc-test.gb`: the first RTC latch still requires `0x00 -> 0x01`, but follow-up non-zero writes are also accepted once a valid snapshot exists because instrumentation of that ROM showed repeated non-zero relatch commands without re-arming zeros. Revisit that legacy relatch rule if curated oracle policy moves back toward the stricter `Pan Docs` model.
- [CARTRIDGE][MBC3-RTC-INVALID-BANKS] MBC3 keeps `0x04..=0x07` as explicit reserved selectors instead of widening standard SRAM banking to `$00-$07`. Current `Pan Docs` wording says `$00-$07` are RAM-bank selectors, but the retained curated `cpp/rtc-invalid-banks-test.gb` oracle only stays green when those selectors remain invalid. Revisit only if stronger hardware evidence or a better oracle closes that source conflict.
- [CARTRIDGE][MBC3-RTC-ACCESS-SPACING] MBC3 records the recommended RTC access-spacing state as `rtc_access_ready_at` on timed RTC-register reads and writes, but the emulator still treats that state as advisory only. `Pan Docs` recommends `4 us` spacing without defining an early-access penalty, and the current `SameBoy` cross-check does not expose one either. Keep enforcement deferred until a stronger dedicated oracle or hardware evidence exists.
- [CARTRIDGE][HEADER-CGB-TITLE-DISCRIMINATOR] The cartridge-header parser now preserves `0x013F-0x0142` separately but still decodes CGB-era titles conservatively as `15` visible characters. `Pan Docs` documents an additional `11`-character layout when those bytes are really a manufacturer code, but the raw header does not provide a reliable discriminator. Revisit only if stronger hardware evidence or a clearly scoped per-ROM metadata rule can separate the two layouts without truncating valid `15`-character titles.

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
