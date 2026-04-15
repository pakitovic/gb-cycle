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

#### Open TODOs

- [PPU][MEALYBUG-MODE3-LIVE-WRITES] Current report still-red follow-up families (implementation order per PPU.md ladder). These are the `18` red entries in the latest `make test-roms` report:
  - **Tier B — SCX/SCY (FIFO core)** `[orders 31-32]`: `m3_scy_change`.
  - **Tier C — LCDC BG toggles** `[orders 33-35]`: `m3_lcdc_bg_en_change`, `m3_lcdc_bg_map_change`, `m3_lcdc_tile_sel_change`.
  - **Tier D — LCDC OBJ toggles** `[orders 36-39]`: `m3_lcdc_obj_en_change`, `m3_lcdc_obj_en_change_variant`, `m3_lcdc_obj_size_change`, `m3_lcdc_obj_size_change_scx`.
  - **Tier E — Window mechanics** `[orders 40-49]`: `m3_window_timing`, `m3_window_timing_wx_0`, `m3_lcdc_win_map_change`, `m3_lcdc_tile_sel_win_change`, `m3_lcdc_win_en_change_multiple`, `m3_lcdc_win_en_change_multiple_wx`, `m3_wx_4_change`, `m3_wx_5_change`, `m3_wx_6_change`, `m3_wx_4_change_sprites`.
  - Immediate next target: `mealybug-tearoom-tests/ppu/m3_scy_change.gb`.

- [PPU][M3-LCDC0-LEFT-EDGE-ONSET] `m3_lcdc_bg_en_change.gb` is still red after fixing the broader bug where `LCDC.0 = 0` stopped consuming BG FIFO pixels. The remaining mismatch is now concentrated in the left edge and the first four live `FF40` toggles per tested scanline. Trace re-entry points:
  - On `LY = 1`, the ROM commits `FF40 <- {0x92, 0x93, 0x92, 0x93}` at `visible_pixels_output = {4, 16, 24, 32}` / `line_dot = {104, 116, 124, 132}`.
  - The first toggle lands while `StartupAlignmentFill` still fronts the FIFO (`pixel_index = 4`) and `VisibleTile2` is queued in `Push`; the second lands with `VisibleTile3` at the FIFO front; the third and fourth already hit ordinary tiles.
  - Two repo-local observability signatures now pin the same four-write cadence across distinct startup classes: a lower-mismatch class reaches the first write with `StartupAlignmentFill pixel_index = 3` and `startup_fifo_placeholders = 3`, then progresses through `VisibleTile2 pixel_index = 7`, `VisibleTile3 pixel_index = 7`, and `Ordinary pixel_index = 7`; the current worst band reaches the first write with `StartupAlignmentFill pixel_index = 6` and `startup_fifo_placeholders = 0`, then progresses through `VisibleTile3 pixel_index = 2` and `Ordinary pixel_index = 2`.
  - In that worst-band class, the external framebuffer mismatch already begins before the first `FF40` write on the line, so the next fix should not assume a pure per-write `LCDC.0` onset bug. Re-entry should treat the write cadence as a probe of an earlier startup-visible left-edge timing class.
  - A follow-up experiment that only pinned `StartupAlignmentFill` to the pre-write `LCDC.0` state regressed the target from `1556` to `1773` mismatching pixels.
  - A broader experiment that pinned already-materialized BG slices plus `push/fill pending` slices to the pre-write `LCDC.0` state regressed the target to `3617` mismatching pixels.

- [PPU][STARTUP-DUMMY-SEED-DEFERRED] A March 28 experiment moving the dummy-startup fill to discard-first-BG-fetch (docboy-style) improved `m3_lcdc_bg_map_change` (`978 -> 722`) but regressed raster tests, `acid/dmg-acid2.gb`, and `m3_scy_change` (`7266 -> 10099`). Confirms the remaining left-edge debt sits in the startup dummy/first-fetch seam, but the fix must preserve stable startup timing and `acid` baseline.

- [PPU][MODE3-PUSH-ARBITRATION-DEFERRED] A March 26 attempt at strict FIFO-empty BG push plus OBJ-start arbitration regressed multiple external families at once (`mooneye hblank_ly_scx_timing-GS`, `intr_2_mode0_timing`, `hacktix/strikethrough`, `mealybug m3_bgp_change`). Any future pass here needs a wider shared BG/window/OBJ fetcher contract rewrite, with external report comparison as a hard gate.

- [PPU][WINDOW-GLITCH-ORACLE] `WX = 0` and `WX = 166` paths are tested but remain provisional. Needs stricter validation for `WX`/`WY`/`LCDC.5` mid-frame glitch behavior, including the DMG-specific `WX = 0 && (SCX & 7) > 0` path. Does not block Phase `5`; needed for Phase `9`.

- [PPU][LCDC2-8X16-ARTIFACTS] Core `8x16` rules and mid-frame `LCDC.2` shrink crash are fixed, but finer DMG-visible artifacts from mid-frame size changes remain open. Needs targeted ROM or oracle coverage. Does not block Phase `5`; needed for Phase `9`.

- [PPU][OAM-CORRUPTION-ORACLE] Deterministic unit/integration coverage is shipped for Mode `2` OAM access, `FEA0-FEFF` reads, `inc rr`, `[hli]`/`[hld]`, stack/interrupt paths, DMG variants, and CGB negative path. The last-row and first-scanline blargg windows (`oam_bug/4`, `oam_bug/5`) are green again after moving trigger classification away from the coarse blocked-access flag and back to live `Mode 2` ownership in the PPU. Still lacks independent oracle comparison. Curated `oam_bug` subset excludes `oam_bug.gb` multi-ROM and `7-timing_effect.gb`. Needed for Phase `9`.

- [PPU][FF44-HBLANK-SEAM] The exact DMG `FF44` advance point inside late HBlank is still hypothesis-only. The docs prefer the "last machine cycle of HBlank" wording, but a direct retune of the current implementation threshold to later dots regressed `mooneye acceptance/ppu/hblank_ly_scx_timing-GS` from green to red while leaving the rest of the model unchanged. Re-entry should start from a narrow trace or oracle comparison around the late-HBlank `LY/SCX` polling seam rather than from another blind constant change. Hard gate: `mooneye acceptance/ppu/hblank_ly_scx_timing-GS` must stay green.

- [PPU][SKIPBOOT-ORACLE] `SkipBoot` startup-mode latch is validated only against repo-local continuity tests. Before Phase `9` hardening, needs comparison against a trusted oracle or hardware capture proving first LCD-visible dots after `SkipBoot` are coherent with published `LCDC`, `STAT`, and `LY` state. Does not block Phase `5`.

#### Current checkpoint

- The broad PPU refactor is structurally landed: explicit visible and pipeline register snapshots, explicit `Mode 3` transfer/readiness/execution state, push/fill ownership, startup-alignment seam, cached-slice ownership across `Push -> fill -> FIFO`, and typed cached-slice origins for the second and third visible post-startup BG tiles.
- The current external report snapshot is `.roms/test/test-report.md = 149/167`: `147` passed, `18` known failing, and `2` informational (`acid/which.gb`, `daid/rom_and_ram.gb`). `make ci` and `make test-roms` are green at this checkpoint.
- The strict PPU ladder is green through `m3_scx_low_3_bits.gb` and `m3_scx_high_5_bits.gb`. It is blocked at `m3_scy_change.gb` (`order 32`), the last Tier B `SCX/SCY` FIFO-core case before the broader `LCDC` and window live-write tiers.
- The current external PPU-green snapshot includes:
  - `acid/dmg-acid2.gb`
  - curated `blargg oam_bug/{1-lcd_sync,2-causes,3-non_causes,4-scanline_timing,5-timing_bug,6-timing_no_bug,8-instr_effect}.gb`
  - `daid/ppu_scanline_bgp.gb`
  - `mooneye acceptance/ppu/{hblank_ly_scx_timing-GS,intr_1_2_timing-GS,intr_2_0_timing,intr_2_mode0_timing,intr_2_mode0_timing_sprites,intr_2_mode3_timing,intr_2_oam_ok_timing,lcdon_timing-GS,lcdon_write_timing-GS,stat_irq_blocking,stat_lyc_onoff,vblank_stat_intr-GS}.gb`
  - `hacktix/{bully,strikethrough}.gb`
  - `mealybug ppu/{m2_win_en_toggle,m3_bgp_change,m3_bgp_change_sprites,m3_obp0_change,m3_scx_low_3_bits,m3_scx_high_5_bits}.gb`
- Important green seams are now deliberately narrow: DMG-only `BGP`/`OBP0` live-write panel paths, sprite-coupled `STAT` publication seams, `SCX` startup carry handling, and the single-placeholder Acid2 startup-tail cleanup. Keep these as targeted hardware hypotheses, not general FIFO rewrite permissions.
- The rejected broad-fix paths are captured below in `Re-entry rules`.
- Working hypothesis: the remaining Mode `3` debt sits around startup dummy / first-fetch / restart-lane timing and live-write onset classes, not another broad visible-FIFO retargeting pass.

#### Re-entry rules

- Resume from one failing family at a time. Prefer the smallest oracle-backed reproduction that distinguishes the suspected same-T-cycle window.
- Capture baseline and final `.roms/test/test-report.md` for exploratory reruns, especially `mealybug-tearoom-dmg-curated`, `acid-dmg-curated`, and `mooneye-acceptance-dmg-curated`.
- Always rerun the baseline PPU smoke gates before accepting any PPU behavior change, even if the local target ROM improves:
  - `acid/dmg-acid2.gb` (`VERY LOW`, order `2`): base raster / smoke coverage for general `Mode 3` raster, BG/WIN/OBJ mixing, and left-edge startup behavior.
  - `daid/ppu_scanline_bgp.gb` (`MEDIUM`, order `41`): visible raster and post-boot state coverage for per-scanline `BGP`.
  - `hacktix/bully.gb` (`HIGH`, order `139`): visible raster and post-boot state coverage for visible VRAM / tilemap seed after boot.
- Keep at least `acid/dmg-acid2.gb`, `daid/ppu_scanline_bgp.gb`, `mealybug ppu/m3_bgp_change.gb`, `mealybug ppu/m3_bgp_change_sprites.gb`, `mealybug ppu/m3_obp0_change.gb`, `mealybug ppu/m3_scx_low_3_bits.gb`, `mealybug ppu/m3_scx_high_5_bits.gb`, `mooneye acceptance/ppu/hblank_ly_scx_timing-GS.gb`, `mooneye acceptance/ppu/intr_2_mode0_timing_sprites.gb`, `mooneye acceptance/ppu/lcdon_timing-GS.gb`, `mooneye acceptance/ppu/lcdon_write_timing-GS.gb`, `hacktix/strikethrough.gb`, `blargg oam_bug/4-scanline_timing.gb`, and `blargg oam_bug/5-timing_bug.gb` as the minimum no-regression set while touching panel-path palette behavior, startup/restart timing, sprite-coupled mode boundaries, `SCX/SCY`, or remaining live-write families.
- Do not reopen generic startup realignment, broad tilemap rereads, broad cached-slice / visible-FIFO retargeting, broad `SCX`/`SCY` retargeting, fill-only `LCDC.0` overrides, materialized-slice-only `LCDC.0` overrides, or isolated "strict push" experiments before a new oracle shows the fault starts there.
- For `m3_scy_change.gb`, start from the existing bitplane-desync hypothesis, but do not retry pending `push` / `fill` tile-data refetch suppression as the complete fix.
- For `m3_lcdc_bg_en_change.gb`, localize the left-edge onset rules across the four `FF40` write points together; do not retry a fill-only or generic materialized-slice override.
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
