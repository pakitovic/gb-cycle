# Open TODOs

Concrete remaining work extracted from the DMG roadmap. See [ROADMAP.md](ROADMAP.md) for phase context and implementation order.

## Guidelines

Keep this ledger lean in status noise and rich in re-entry context.

When an open item is non-trivial, make four things obvious:

1. What exact behavior or validation gap remains open.
2. What evidence is already in hand.
3. Which superseded directions should not be retried first.
4. What the highest-value next step is.

Remove TODOs when closed. Rewrite when the old wording points to a superseded path. Do not keep archival `Done` bullets.

### Phase 4 — Base PPU and visible pipeline

- [PPU][MODE3-SCY-OBJ-PHASE-POLICY] `m3_scy_change.gb` is green through `PpuMode3ScyObjPhasePolicy` plus `PpuMode3ObservedScyObjPhaseTable`. This is cleanup debt, not a blocker: if a later oracle resolves the exact BG/OBJ handoff phase, replace the remaining observed-table ranges with direct shared BG/OBJ fetcher arbitration instead of extending the table further.
- [PPU][WINDOW-GLITCH-ORACLE] The active window ROM block is green (`m3_window_timing`, `m3_window_timing_wx_0`, `m3_lcdc_win_en_change_multiple*`, `m3_wx_*`). The stricter hardware/oracle question remains open for `WX = 0`, `WX = 166`, `WX`/`WY`, and `LCDC.5` mid-frame glitch behavior, especially `WX = 0 && (SCX & 7) > 0`.
- [PPU][OAM-CORRUPTION-ORACLE] Deterministic unit/integration coverage exists for Mode `2` OAM access, `FEA0-FEFF` reads, `inc rr`, `[hli]` / `[hld]`, stack/interrupt paths, DMG variants, and the CGB negative path. The curated `oam_bug` subset is green, but an independent oracle comparison is still missing and the curated set still excludes `oam_bug.gb` multi-ROM plus `7-timing_effect.gb`.
- [PPU][FF44-HBLANK-SEAM] The exact DMG `FF44` advance point inside late HBlank remains hypothesis-only. A direct retune to later dots regressed `mooneye acceptance/ppu/hblank_ly_scx_timing-GS`; re-entry must start from a narrow trace or oracle comparison around the late-HBlank `LY` / `SCX` polling seam, not another blind constant change. Hard gate: keep that ROM green.
- [PPU][LCDC2-8X16-ARTIFACTS] Core `8x16` rules and the mid-frame `LCDC.2` shrink crash are fixed through a narrow model: line-start OBJ-height latch, observed per-phase bitplane selection, queued-FIFO rewrite for future tail pixels, and retroactive scanline repaint when shrink lands after low-half rows started drawing. Re-check finer DMG-visible artifacts only with a targeted ROM or oracle.
- [PPU][SKIPBOOT-ORACLE] `SkipBoot` startup-mode latch has repo-local continuity coverage, but still needs a trusted-oracle or hardware comparison proving that first LCD-visible dots are coherent with published `LCDC`, `STAT`, and `LY` state.
- [PPU][PROJECT-OWNED-COVERAGE-GAPS] Keep these as concrete test gaps if the related code changes again: direct DMG `LCDC.0 = 0` suppression of window rendering when `LCDC.5 = 1`; end-to-end OAM-corruption fixtures for `pop`, `call`, `ret`, `rst`, and executing code from OAM; and an explicit project decision/test for whether the canonical fetcher `Sleep` phase is a named state or represented by the current push-entry / retry timing.

### Phase 6 — Banked cartridges, special cartridges, and cartridge persistence

- [CARTRIDGE][MBC3-LATCH-RELATCH-POLICY] MBC3 currently keeps a deliberate compatibility deviation for `cpp/latch-rtc-test.gb`: the first RTC latch still requires `0x00 -> 0x01`, but follow-up non-zero writes are also accepted once a valid snapshot exists because instrumentation of that ROM showed repeated non-zero relatch commands without re-arming zeros. Revisit that legacy relatch rule if curated oracle policy moves back toward the stricter `Pan Docs` model.
- [CARTRIDGE][MBC3-RTC-INVALID-BANKS] MBC3 keeps `0x04..=0x07` as explicit reserved selectors instead of widening standard SRAM banking to `$00-$07`. Current `Pan Docs` wording says `$00-$07` are RAM-bank selectors, but the retained curated `cpp/rtc-invalid-banks-test.gb` oracle only stays green when those selectors remain invalid. Revisit only if stronger hardware evidence or a better oracle closes that source conflict.
- [CARTRIDGE][MBC3-RTC-ACCESS-SPACING] MBC3 records the recommended RTC access-spacing state as `rtc_access_ready_at` on timed RTC-register reads and writes, but the emulator still treats that state as advisory only. `Pan Docs` recommends `4 us` spacing without defining an early-access penalty, and the current `SameBoy` cross-check does not expose one either. Keep enforcement deferred until a stronger dedicated oracle or hardware evidence exists.

### Phase 7 — Audio

- [APU][CGB-CH3-WAVE-RAM-ACTIVE-MMIO] The current CH3 active wave-RAM MMIO contract is only specified for the DMG family. DMG coverage already locks the fetch-window policy and DMG retrigger-corruption lane, but CGB-family active-access redirection remains intentionally deferred because the repo scope is still DMG-only. Do not treat the current `ConsoleModel::Cgb` fallback path as hardware-accurate or add tests/docs that claim a final CGB contract before the CGB APU lane exists.
- [APU][EXTRA-LENGTH-CLOCKING-CGB-REVISION] CH1/CH2/CH3/CH4 extra-length clocking is now wired through an explicit per-model policy seam, but the current `ConsoleModel` surface still cannot distinguish the documented `CGB-02` exception from later CGB revisions. The code therefore keeps the generic DMG/later-CGB rule as a conservative fallback even for `ConsoleModel::Cgb`. Do not claim revision-accurate CGB extra-length clocking until a revision-scoped model or stronger oracle closes that gap.
- [APU][SKIPBOOT-HIDDEN-STATE] Direct boot currently reconstructs the visible audio snapshot, powered state, wave-RAM startup policy, channel-active mask, and shared-divider-derived `DIV-APU` phase, but it still resets other hidden APU state from repo-local defaults. HPF history, pulse duty-step/timer continuation, CH3 sample-buffer/sample-index continuation, and CH4 LFSR/noise-timer continuation are not yet verified boot-handoff state. Keep docs/tests explicit about that narrower contract until a stronger oracle or hardware-backed startup model closes the gap.
- [APU][HOST-RESAMPLER-KERNEL-TUNING] The host-facing capture path now applies a causal windowed-sinc band-limited resampler to the post-HPF T-cycle stream, which is a large step up from simple interval averaging. However, SameBoy still uses a more specialized edge-driven synthesis path with tuned kernel/heuristic details, so the current resampler kernel width, cutoff margin, phase count, and transient behavior remain tuning targets if high-pitched commercial content still sounds harsher than the oracle.
- [APU][CH4-NR43-OUTPUT-ORACLE] CH4 DMG/pre-`CGB-D` now uses the SameBoy-guided hidden noise-counter path for ordinary LFSR stepping, delayed DMG trigger startup, divisor-`0` / `alignment == 3` visible `0x0055`, and staged `old -> FF -> new` live `NR43` writes. The old broad startup-handshake and Zelda-tail guard experiments are superseded; if this reopens, compare the explicit pass/action trace against a SameBoy isolated CH4 output oracle instead of retuning the hidden counter from commercial-audio symptoms.
- [APU][ZOMBIE-MODE-REVISION-MATRIX] CH1/CH2/CH4 now model the cross-revision-consistent manual increment path for live `NR12` / `NR22` / `NR42` writes (`increase` with pace `0` increments the current volume modulo `16`), but the broader zombie-mode write matrix still varies by hardware revision. Do not claim a fully solved DMG zombie-mode contract until a revision-scoped oracle or hardware-backed policy closes the rest of that matrix.
- [APU][DAC-OFF-FADE-TUNING] The APU now models DAC disable as a short explicit per-channel analog discharge toward `0` on the T-cycle timeline, which is a closer fit to Pan Docs than the previous instantaneous step. However, Pan Docs still says the exact fade varies by model, and the current repo policy uses one conservative SameBoy-aligned decay duration/curve across the supported models. Keep per-model tuning and hardware-backed fade-shape validation open until a stronger oracle closes that gap.
