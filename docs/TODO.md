# Open TODOs

Concrete remaining work extracted from the roadmap. See [ROADMAP.md](ROADMAP.md) for phase context and implementation order.

Keep this ledger lean: only open, actionable work stays here. Closed work belongs in git history, roadmap status, or the owning subsystem doc, not as archival `Done` bullets.

## Entry format

Each non-trivial TODO should make these clear in one compact bullet:

1. The exact behavior or validation gap that remains open.
2. The evidence already in hand.
3. Superseded directions that should not be retried first.
4. The highest-value next step.

## Phase 4 — Base PPU and visible pipeline

PPU hardening toward a hardware-true engine (post PR #245): the sprite Mode 3 penalty and STAT publication are now emergent rather than curve-fitted, but three coupled compensation seams remain. Reference implementations for dot-by-dot cross-checks: SameBoy (`$HOME/workspace/SameBoy`, read-only reference) and DocBoy (`$HOME/workspace/docboy`, modifiable for instrumentation, revert after). Fast iteration harness: `crates/gb-core/examples/g3_sprite_grid.rs`.

- [PPU][MODE3-FETCHER-LEAD] The BG fetcher runs ~7 dots ahead of hardware before the first sprite fetch (visible tile `fx=8` tile-index read lands at dot `87` vs hardware ~`94`); this lead is the root reason the mid-line write observation tables below have to exist. PR #245's hardware-true sprite penalty is calibrated around the lead, not by removing it. Highest-value next step: pull the BG fetch start to the hardware dot, cross-checked dot-by-dot against SameBoy/DocBoy. Do not paper over the lead by retuning individual observation tables.
- [PPU][MODE3-OBSERVATION-TABLES] Mid-line register-write landing on sprite lines is still resolved through per-sprite-`x`/`scx` phase-fitted tables (`PpuMode3ObservedLcdc3PhaseTable`, `Lcdc4PhaseTable`, `Lcdc2ObjSizePhaseTable`, `ScyObjPhaseTable` in `helpers/mode3_policies.rs`; the window-activation tilemap masks in `mode3/window.rs`; the `SCX` coarse-bit refetch gating) which PR #245 refit to the corrected schedule rather than deriving from the fetcher. Evidence: mealybug DMG + CGB-compat suites are green. Superseded direction: do not extend these tables for new cases. Highest-value next step: once [MODE3-FETCHER-LEAD] is fixed, resolve tilemap / tile-data / `SCY` / tile-select at the actual fetch dots and delete the tables.
- [PPU][MODE0-PUBLISH-HALT-GRID] The steady-frame published-`STAT` Mode 0 boundary override gated on `scx == 0 || Mode 0 IRQ enabled`, plus the Mode 0 halt-wake deferral table (CGB-family all `SCX` low bits, DMG-family `SCX&7` in `{1,2,5,6}`), compensate a measured 1-4 T-cycle offset between the halt-dispatch and nop-dispatch grids — wilbertpol `intr_2_mode0_timing_sprites_nops` measures on post-LCD-enable frames, mooneye halt variants on steady frames. Superseded direction: never refit sprite penalties (PR #245 froze them hardware-true). Highest-value next step: a uniform halt-wake dispatch convention plus recalibrated Mode 0 / Mode 2 pretrigger apertures, validated against SameBoy/DocBoy.
- [PPU][MODE3-SCY-OBJ-PHASE-POLICY] `m3_scy_change.gb` is green on DMG (shootout 264/264) but the CGB-compat case is the only `mealybug-tearoom-tests-cgb` failure (23/24, 121px after the landed seed fix). Root cause is NOT a mode3 fetcher bug: it is a CGB CPU↔PPU phase offset of ~4 dots (CPU writes SCY 4 dots earlier relative to the PPU on CGB hardware) entangled with the CGB SCY 2-dot write-observation latency. 8 scoped-fix experiments refuted. The fix is a coordinated CGB timing redesign, fully planned in [`docs/roadmap/12-cgb-timing-redesign.md`](roadmap/12-cgb-timing-redesign.md) (phases P0–P4). HARD CONSTRAINT carried into that plan: the −4 must EMERGE from a CGB enable→PPU-start phase model (a CGB constant family mirroring the DMG one), never a magic `is_cgb_family()`-gated `+4`/`correction(4)` literal; the SCY observation latency must be DocBoy's uniform `pending_write` register collapsing gb-cycle's 3 existing SCY mechanisms (not a 4th); and P3 must DELETE the `cgb_dmg_software_startup_visible_tile2/3_*` retarget tables + the seed fix (`cgb_startup_frozen_tile_row`) — green-with-seams is an explicit failure. Net PPU seam count must go DOWN. Do not retry the scoped levers or extend the observed-table ranges.
- [PPU][WINDOW-GLITCH-ORACLE] The active window ROM block is green (`m3_window_timing`, `m3_window_timing_wx_0`, `m3_lcdc_win_en_change_multiple*`, `m3_wx_*`), but stricter hardware/oracle evidence is still missing for `WX = 0`, `WX = 166`, `WX`/`WY`, and `LCDC.5` mid-frame glitches, especially `WX = 0 && (SCX & 7) > 0`.
- [PPU][OAM-CORRUPTION-ORACLE] Deterministic unit/integration coverage exists for Mode `2` OAM access, `FEA0-FEFF` reads, `inc rr`, `[hli]` / `[hld]`, stack/interrupt paths, DMG variants, and the CGB negative path; the promoted GB Emulator Shootout `oam_bug` subset is green, and the standalone `blargg` report now keeps `oam_bug.gb` plus `7-timing_effect.gb` green with pinned framebuffer fixtures, but independent oracle comparison is still missing.
- [PPU][FF44-HBLANK-SEAM] The exact DMG `FF44` advance point inside late HBlank remains hypothesis-only; a direct later-dot retune regressed `mooneye acceptance/ppu/hblank_ly_scx_timing-GS`, so re-entry must start from a narrow trace or oracle comparison around the late-HBlank `LY` / `SCX` polling seam while keeping that ROM green.
- [PPU][LCDC2-8X16-ARTIFACTS] Core `8x16` rules and the mid-frame `LCDC.2` shrink crash are fixed through a narrow model; re-check finer DMG-visible artifacts only with a targeted ROM or oracle, not by broadening the current line-start latch / queued-FIFO / repaint policy speculatively.
- [PPU][SKIPBOOT-ORACLE] `SkipBoot` startup-mode latch has repo-local continuity coverage, but still needs a trusted-oracle or hardware comparison proving first LCD-visible dots are coherent with published `LCDC`, `STAT`, and `LY` state.
- [PPU][PROJECT-OWNED-COVERAGE-GAPS] Keep these as concrete test gaps if related code changes again: direct DMG `LCDC.0 = 0` suppression of window rendering when `LCDC.5 = 1`; end-to-end OAM-corruption fixtures for `pop`, `call`, `ret`, `rst`, and executing code from OAM; and a project decision/test for whether canonical fetcher `Sleep` is a named state or represented by current push-entry / retry timing.

## Phase 5 — Input and simple peripherals

- [SERIAL][DOCBOY-SC00-LINK-ORACLE] DocBoy's `serial_two_players_basic_transfer_slave_sc_00.gb` linked row asserts that a slave-side transfer can complete after writing `SC = 0`, while the serial handbook follows Pan Docs in requiring external-clock receivers to arm with `SC.7 = 1`; keep the DocBoy row visible as a blocking extra-suite failure, but resolve the source conflict with hardware-facing evidence before changing serial gating.

## Phase 6 — Banked cartridges, special cartridges, and cartridge persistence

- [CARTRIDGE][MBC3-LATCH-RELATCH-POLICY] MBC3 deliberately accepts follow-up non-zero RTC relatch writes once a valid snapshot exists because `cpp/latch-rtc-test.gb` repeatedly relatches without re-arming zero; revisit this compatibility deviation only if curated oracle policy moves back toward the stricter Pan Docs `0x00 -> 0x01` model.
- [CARTRIDGE][MBC3-RTC-INVALID-BANKS] MBC3 keeps `0x04..=0x07` as reserved selectors instead of widening standard SRAM banking to `$00-$07`; Pan Docs wording conflicts with the retained `cpp/rtc-invalid-banks-test.gb` oracle, so revisit only with stronger hardware evidence or a better oracle.
- [CARTRIDGE][MBC3-RTC-ACCESS-SPACING] MBC3 records recommended RTC access spacing as `rtc_access_ready_at`, but enforcement remains advisory because Pan Docs recommends `4 us` without defining early-access penalties and current cross-check evidence exposes no penalty; defer enforcement until a dedicated oracle or hardware evidence exists.

## Phase 7 — Audio

- [APU][CGB-CH3-WAVE-RAM-ACTIVE-MMIO] CH3 active wave-RAM MMIO is specified for DMG-family behavior only; do not treat the current `ConsoleModel::GameBoyColor` fallback as hardware-accurate or claim a final CGB contract before CGB APU evidence exists.
- [APU][EXTRA-LENGTH-CLOCKING-CGB-REVISION] CH1/CH2/CH3/CH4 extra-length clocking has an explicit per-model policy seam, but `ConsoleModel` still cannot distinguish the documented `CGB-02` exception from later CGB revisions; keep the generic DMG/later-CGB fallback until revision-scoped modeling or stronger oracle evidence closes the gap.
- [APU][SKIPBOOT-HIDDEN-STATE] Direct boot reconstructs visible audio state, powered state, wave-RAM policy, channel-active mask, and shared-divider-derived `DIV-APU` phase, but HPF history, pulse duty/timer continuation, CH3 sample-buffer/index continuation, and CH4 LFSR/noise-timer continuation remain unverified boot-handoff state.
- [APU][HOST-RESAMPLER-KERNEL-TUNING] The host-facing capture path uses a causal windowed-sinc resampler; revisit kernel width, cutoff margin, phase count, and transient behavior only if high-pitched commercial content still sounds harsher than the chosen reference capture.
- [APU][CH4-NR43-OUTPUT-ORACLE] CH4 DMG/pre-`CGB-D` uses a retained hidden noise-counter path; if this reopens, compare the explicit pass/action trace against an isolated CH4 output oracle instead of retuning from broad commercial-audio symptoms.
- [APU][ZOMBIE-MODE-REVISION-MATRIX] CH1/CH2/CH4 now model the cross-revision-consistent manual increment path for live `NR12` / `NR22` / `NR42` writes, but the broader zombie-mode write matrix remains revision-dependent; do not claim a fully solved DMG zombie-mode contract without revision-scoped oracle or hardware-backed policy.
- [APU][DAC-OFF-FADE-TUNING] DAC disable is modeled as a short per-channel analog discharge, but Pan Docs says fade varies by model and the repo currently uses one conservative curve; keep per-model fade-shape validation open until stronger evidence exists.

## Phase 9 — Final DMG hardening, differential validation, and closure

- [TESTING][LINKED-SESSION-MARKDOWN-REPORT] `docboy-dmg-link` now runs through the `docboy` report with `cargo rom-suite-link`; if linked aggregate Markdown persistence is needed, design participant-aware report rows instead of flattening linked sessions into single-machine ROM rows.

## Phase 10 — CGB implementation roadmap

- [BOOT][CGB-DIRECT-DIV-PREDICTOR] CGB direct-start has validated header-aware timer buckets for missing/DMG-compatible headers (`0x2674`), native CGB non-Nintendo old-licensee headers (`0x1E84`), and native CGB old-licensee `$33` headers with binary-zero new-licensee bytes (`0x1E98`), but a full DocBoy-style predictor for other Nintendo, new-licensee, and DMG-compatibility checksum-table buckets remains deferred; re-entry should start from generated revision-derived CGB RealBoot measurements, not ROM-specific runner timer overrides or blind DocBoy phase constants.

## Phase 11 — SGB/SGB2 implementation roadmap

- [SGB][EXTERNAL-ORACLES] Mooneye `acceptance/boot_regs-sgb.gb` and `acceptance/boot_regs-sgb2.gb` pin SGB/SGB2 direct-start fingerprints in the extra report, but public-ROM oracle gaps remain for `ATRC_EN`, `TEST_EN`, `ICON_EN`, `OBJ_TRN`, `_TRN` timing, and packet busy; synthetic coverage validates those internally, so add external coverage only from a public ROM with an explicit expected signal for one of those gaps.
