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
  - early visible-raster baseline: `daid/ppu_scanline_bgp.gb`
  - one sprite-coupled `STAT` timing case: `mooneye acceptance/ppu/intr_2_mode0_timing_sprites.gb`
  - remaining `Mode 3` live-write families: `m3_lcdc_bg_map_change`, `m3_lcdc_tile_sel_change`, `m3_lcdc_tile_sel_win_change`, `m3_scy_change`
- Last stable external measurements for the still-open live-write sentinels are: `m3_lcdc_bg_map_change: 674`, `m3_lcdc_tile_sel_change: 1410`, `m3_lcdc_tile_sel_win_change: 1232`, `m3_scy_change: 7819`.
- Visible-FIFO cached-slice ownership is explicit and survives `fill.pending -> FIFO` plus startup placeholders. Broad visible-FIFO retargeting was already tried and reverted: it regressed the external oracles instead of moving them. Treat that as settled evidence that the remaining debt is not solved by broad late visible-pop rereads.
- The important remaining left-edge/live-write evidence is stable:
  - `LCDC.3` late-tail ownership around `VisibleTile2/VisibleTile3` is already covered by repo-local tests and still does not move the external oracle.
  - `LCDC.4` / `SCY` visible-FIFO retargeting regressed the target families and was reverted.
  - current first mismatches stay at `m3_lcdc_bg_map_change -> x = 12, y = 0`, `m3_lcdc_tile_sel_change -> x = 0, y = 10`, `m3_scy_change -> x = 1, y = 0`.
  - working hypothesis remains: the unresolved debt sits earlier, around startup dummy / first-fetch / restart-lane behavior, not in another broad visible-FIFO retargeting pass.
- `PpuSnapshot` and the scheduler trace already export the useful re-entry state: transfer lane, source window, backing, readiness, startup seam, and visible-FIFO cached-slice metadata. Do not add new broad tracing until one of those existing fields fails to discriminate the next oracle.
- The Donkey Kong desktop gate remains stable after the refactor slices that landed the new ownership model. No current evidence says the remaining open PPU debt is the source of a desktop slowdown regression.
- `hacktix/strikethrough.gb`, `blargg oam_bug/4`, and `blargg oam_bug/5` are green again, so the current sprite-coupled work no longer carries those known regressions.
- **Highest-value next step:** keep `daid/ppu_scanline_bgp.gb` as the active early raster gate, but use testcase `36` of `intr_2_mode0_timing_sprites.gb` as the active mooneye oracle. Testcase `1` is no longer the frontier: the copied ROM-path probe already matches the real `STAT` arm and the remaining same-`X` right-edge work pushed the first failing testcase back to the mixed `5x X=0 + 5x X=160` case. The next useful move is a narrow testcase-`36` probe built from the current real round-count pattern, not another pre-arm setup retune.
- The new DMG maturity ladder in `PPU.md` shows the current PPU state is not monotonic. Strictly, the first still-open step is `order 2` (`daid/ppu_scanline_bgp.gb`), so the ladder says the emulator cannot yet be treated as "past the early raster baseline" even though many later and narrower families are already green.
- The DMG OAM-corruption path now matches the hardware/document baseline more closely too: trigger classification is address-family / trigger-family based, while the PPU remains the owner of the live `Mode 2` mode-and-row gate. That restores the last-row and first-scanline windows in `blargg oam_bug/4-scanline_timing.gb` and `oam_bug/5-timing_bug.gb` without reopening the phase-4 synthetic OAM-corruption fixtures.
- Current ladder snapshot:

| order | case | current state | likely gap | next action |
| --- | --- | --- | --- | --- |
| 2 | `daid/ppu_scanline_bgp.gb` | red | visible raster / per-scanline `BGP` baseline still differs from the fixture set; the captured framebuffer keeps the global face silhouette but introduces scanline-dependent inner stripes, so this is not a gross post-boot VRAM seed failure like `hacktix/bully.gb` | promote this case to an active gate and compare the three fixture variants against local scanline output before resuming broader hi-fi work |
| 15 | `mooneye acceptance/ppu/intr_2_mode0_timing_sprites.gb` | red | sprite-coupled `Mode 2 -> 0` timing still diverges even though `intr_2_mode0_timing.gb` is green; likely gap is OBJ stall / arbitration influence on variable `Mode 3` end and `mode0_start_dot` | treat this as the first sprite-coupled timing closure after the LCD restart lane, not as a generic `STAT` regression |
| 16 | `mooneye acceptance/ppu/lcdon_timing-GS.gb` | green | the dedicated CPU-path LCD-enable read probe is now green for `LY`, `STAT` with `LYC=0/1`, OAM, and VRAM; the last remaining mismatch was the coincidence bit on the first CPU-visible dot of a new line after restart, and closing that seam makes the external ROM pass | keep the repo-local read probe and first-dot coincidence seam explicit; move the next primary focus off the LCD restart lane |
| 17 | `mooneye acceptance/ppu/lcdon_write_timing-GS.gb` | green | the dedicated CPU-path `LCDC.7` write probe is now green and the external mooneye case passes after splitting CPU-visible OAM-write publication from the owner bus state and opening the OAM-only write window only at scanline start and the exact `Mode 2 -> 3` boundary | keep the new OAM-write publication seam fixed and use `lcdon_timing-GS` as the remaining LCD-restart oracle |
- Practical maturity reading:
  - Strict ladder maturity: blocked at `order 2`.
  - Real subsystem maturity: already beyond the early raster baseline in several later areas (`bully`, `mem_oam`, `sprite_priority`, most mooneye `STAT`, `oam_bug`, `strikethrough`, `m2_win_en_toggle`), but with two earlier holes still open (`2`, `15`).
  - Consequence: `27+` mealybug cases stay valuable as sentinels, but they should not be treated as the primary closure target until those earlier ladder blockers are closed or intentionally waived with evidence.

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

- [PPU][MOONEYE-STAT-TIMING] `hblank_ly_scx_timing-GS`, `intr_2_0_timing`, `vblank_stat_intr-GS` are now green. Remaining open case: `ppu/intr_2_mode0_timing_sprites` — sprite-coupled Mode `2 -> 0` timing is not fully closed.
  - Current re-entry state for `intr_2_mode0_timing_sprites.gb`:
    - testcase `0` (`1 sprite at X=0`) is locally closed again: the first `STAT` read after the IRQ sees `HBlank`, with `before_line_dot = 259` and `mode0_start_dot = 259`.
    - testcase `1` (`2 sprites at X=0`) is also locally closed: the first `STAT` read after the IRQ sees `HBlank`, with `before_line_dot = 267` and `mode0_start_dot = 267`.
    - testcase `2` (`3 sprites at X=0`) is closed too: the first `STAT` read after the IRQ now sees `HBlank`, with `before_line_dot = 271` and `mode0_start_dot = 271`.
    - `hacktix/strikethrough.gb` is green again, so the sprite-coupled closure can move independently without carrying a known Hacktix regression.
    - testcase `1` is no longer the frontier. The exact copied ROM-path probe for testcase `1` matches the real setup seam through the `STAT` arm itself at `ly = 67`, `line_dot = 136`, `mode = Drawing`, `pc = 0x0BFD`, which closes the earlier pre-arm mismatch before `FF41 = 0x20`.
    - the current same-`X` right-edge closure also pushed testcase `9` (`10 sprites at X=0`) out of the frontier. The local two-round probe for testcase `9` round B now gives `0xA3 -> 0xA0` instead of the previous `0xA3, 0xA3, 0xA3, 0xA3, 0xA0`.
    - the current full-ROM failure signature is now back at testcase `36`, the mixed `5x X=0 + 5x X=160` case. The useful real probe is `real_mooneye_intr_2_mode0_timing_sprites_logs_case36_round_counts`: round A first reads `0xA0` at `before_line_dot = 319`, round B first reads `0xA0` at `before_line_dot = 315`, and the remaining gap now lives in the right-edge mixed-cluster tail rather than in the old testcase-`1` setup path.
  - Do not retry these two directions first:
    - a one-dot-earlier CPU-visible `STAT` `Drawing -> HBlank` publication experiment did not move the earlier testcase-`2` oracle and regressed the local boundary unit test.
    - broader chained-fetch experiments that recurse into `advance_object_fetch()` from `Push1`, or that suppress chained `Push0` cost blindly, destabilized the local diagnostics and were reverted.
    - do not resume from the older testcase-`13` hidden-lane probes first; they are superseded by the testcase-`1` ROM-path oracle and the newer testcase-`36` mixed-cluster probes.
  - Highest-value next step from this checkpoint:
    - keep testcase `36` as the active oracle and derive a cheaper repo-local probe from the current real round-count pattern before touching runtime again. The next useful move is to discriminate the mixed right-edge cluster tail around the first round-B `FF41` read, not to reopen testcase-`1` pre-arm work or hidden-lane testcase-`13` diagnostics.

- [PPU][FF44-HBLANK-SEAM] The exact DMG `FF44` advance point inside late HBlank is still hypothesis-only. The docs prefer the "last machine cycle of HBlank" wording, but a direct retune of the current implementation threshold to later dots regressed `mooneye acceptance/ppu/hblank_ly_scx_timing-GS` from green to red while leaving the rest of the model unchanged. Re-entry should start from a narrow trace or oracle comparison around the late-HBlank `LY/SCX` polling seam rather than from another blind constant change. Hard gate: `mooneye acceptance/ppu/hblank_ly_scx_timing-GS` must stay green.

#### Re-entry rules

- Resume from one failing family at a time; prefer the smallest oracle-backed reproduction that distinguishes the suspected same-T-cycle window.
- Capture baseline and final `/.roms/test/test-report.md` for any exploratory rerun, especially `mealybug-tearoom-dmg-curated`, `acid-dmg-curated`, and `mooneye-acceptance-dmg-curated`.
- Treat cached background slices already in `Push`, `fill.pending`, or the visible FIFO as a likely suspect only for the remaining live-write families. Do not let that heuristic override a better active oracle such as the current testcase-`36` mixed right-edge seam in `intr_2_mode0_timing_sprites.gb`.
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
