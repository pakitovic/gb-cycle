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

- The broad PPU refactor is structurally landed: explicit runtime/pipeline versus panel/live-output ownership, staged `Mode 3` MMIO live-write routing, ordered published-`STAT` rule evaluation, stage-split `Mode 3` helpers, typed `BgFifo` ownership, grouped `DmgWindowRestartState`, explicit `classify -> plan -> apply` palette-conflict handling, startup-alignment seam ownership, and cached-slice ownership across `Push -> fill -> FIFO`.
- The current external report snapshot is `.roms/test/test-report.md = 167/167`: `165` passed, `0` known failing, and `2` informational (`acid/which.gb`, `daid/rom_and_ram.gb`). `make ci` and `make test-roms` are green at this checkpoint; keep both green as the acceptance gate for follow-up PPU work.
- The full active Mealybug DMG window-mechanics ladder is green in the current tree (orders `40..49` in `docs/hardware/PPU.md`).

#### Remaining follow-ups

##### Cleanup debt

- [PPU][MODE3-SCY-OBJ-PHASE-POLICY] `m3_scy_change.gb` is green and the current closure already lives in `PpuMode3ScyObjPhasePolicy` plus `PpuMode3ObservedScyObjPhaseTable`. This is cleanup debt, not a blocker: if a later oracle resolves the exact BG/OBJ handoff phase, replace the remaining observed-table ranges with direct shared BG/OBJ fetcher arbitration instead of extending the table further.

##### Oracle follow-ups

- [PPU][WINDOW-GLITCH-ORACLE] The active window ROM block is now fully green (`m3_window_timing`, `m3_window_timing_wx_0`, `m3_lcdc_win_en_change_multiple*`, `m3_wx_*`), but the stricter oracle question remains open. Re-check whether `WX = 0`, `WX = 166`, `WX`/`WY`, and `LCDC.5` mid-frame glitch behavior still need an explicit hardware or trusted-oracle pass, especially the DMG-specific `WX = 0 && (SCX & 7) > 0` path. Does not block Phase `5`; needed for Phase `9`.
- [PPU][OAM-CORRUPTION-ORACLE] Deterministic unit/integration coverage is shipped for Mode `2` OAM access, `FEA0-FEFF` reads, `inc rr`, `[hli]`/`[hld]`, stack/interrupt paths, DMG variants, and the CGB negative path. The last-row and first-scanline blargg windows (`oam_bug/4`, `oam_bug/5`) are green again after moving trigger classification away from the coarse blocked-access flag and back to live `Mode 2` ownership in the PPU. Still lacks an independent oracle comparison. The curated `oam_bug` subset still excludes `oam_bug.gb` multi-ROM and `7-timing_effect.gb`. Needed for Phase `9`.
- [PPU][FF44-HBLANK-SEAM] The exact DMG `FF44` advance point inside late HBlank is still hypothesis-only. The docs prefer the "last machine cycle of HBlank" wording, but a direct retune to later dots regressed `mooneye acceptance/ppu/hblank_ly_scx_timing-GS` from green to red while leaving the rest of the model unchanged. Re-entry should start from a narrow trace or oracle comparison around the late-HBlank `LY/SCX` polling seam, not from another blind constant change. Hard gate: `mooneye acceptance/ppu/hblank_ly_scx_timing-GS` must stay green.

##### Phase `9` hardening

- [PPU][LCDC2-8X16-ARTIFACTS] Core `8x16` rules and the mid-frame `LCDC.2` shrink crash are fixed, and the active OBJ-toggle block (`m3_lcdc_obj_size_change`, `m3_lcdc_obj_size_change_scx`) is green. The closure is intentionally narrow: a line-start `OBJ`-height latch, observed per-phase bitplane selection, a queued-FIFO rewrite seam for future tail pixels, and a retroactive scanline repaint seam for already-emitted pixels when the shrink lands after the low-half rows have started drawing. With the Mealybug window block now green, re-check whether finer DMG-visible artifacts from mid-frame size changes still need targeted ROM or oracle coverage. Does not block Phase `5`; needed for Phase `9`.
- [PPU][SKIPBOOT-ORACLE] `SkipBoot` startup-mode latch is validated only against repo-local continuity tests. Before Phase `9` hardening, it still needs a trusted-oracle or hardware comparison proving that the first LCD-visible dots after `SkipBoot` are coherent with published `LCDC`, `STAT`, and `LY` state. Does not block Phase `5`.

#### If Phase 4 is reopened

##### Start here

- If a new PPU regression appears, resume from one failing family at a time. Prefer the smallest oracle-backed reproduction that distinguishes the suspected same-T-cycle window.
- Working hypothesis: the remaining Mode `3` debt is now oracle-hardening debt around startup dummy / first-fetch / restart-lane timing and live-write onset classes, not another broad visible-FIFO retargeting pass.
- Recent green Mode `3` seams are intentionally narrow: DMG-only `BGP`/`OBP0` live-write panel paths, sprite-coupled `STAT` publication seams, `SCX` startup carry handling, curated `SkipBoot` DMG boot-trademark seeding for the `LCDC.3` / `LCDC.4` closures, and startup-continuation overrides on `VisibleTile2` / `VisibleTile3`. Treat these as targeted hardware hypotheses, not generic FIFO rewrite permissions.

##### Do not retry first

- Do not reopen generic startup realignment, broad tilemap rereads, broad cached-slice / visible-FIFO retargeting, broad `SCX`/`SCY` retargeting, fill-only `LCDC.0` overrides, materialized-slice-only `LCDC.0` overrides, synthetic `visible_tile2_window` repaint windows, or isolated "strict push" experiments before a new oracle shows the fault starts there.
- Do not retry broad dummy-startup fill retiming without a new oracle; a previous discard-first-BG-fetch experiment improved one ROM but regressed baseline raster gates.
- Do not prioritize a broad per-dot `Mode 3` context cache as a first-line perf experiment. Recent release sampling on representative BG-only and OBJ-heavy scenes did not surface `mode3_register_latches()` or `mode3_window_policy()` as standalone hotspots; the measured hot leaf in that family was narrower transfer/raster-publication work instead.
- Do not land a shared `push_obj_pixels()` / `rewrite_obj_fifo_pixels()` kernel as a perf change alone. The measured OBJ-heavy hotspot gain was only borderline and still landed inside Criterion's noise threshold; keep that idea in reserve for a future maintainability-driven cleanup or stronger benchmark evidence.
- If a future `LCDC` or live-write regression appears, keep onset rules localized per write class and per boundary; do not retry a fill-only or generic materialized-slice override.

##### Validation minimum

- Capture baseline and final `.roms/test/test-report.md` for exploratory reruns, especially `mealybug-tearoom-dmg-curated`, `acid-dmg-curated`, and `mooneye-acceptance-dmg-curated`.
- Always rerun these baseline PPU smoke gates before accepting any PPU behavior change, even if the local target ROM improves:
  - `acid/dmg-acid2.gb` (`VERY LOW`, order `2`): base raster / smoke coverage for general `Mode 3` raster, BG/WIN/OBJ mixing, and left-edge startup behavior.
  - `daid/ppu_scanline_bgp.gb` (`MEDIUM`, order `41`): visible raster and post-boot state coverage for per-scanline `BGP`.
  - `hacktix/bully.gb` (`HIGH`, order `139`): visible raster and post-boot state coverage for visible VRAM / tilemap seed after boot.
- Keep the following focused no-regression set while touching panel-path palette behavior, startup/restart timing, sprite-coupled mode boundaries, `SCX/SCY`, or live-write behavior: `mealybug ppu/{m3_bgp_change,m3_bgp_change_sprites,m3_obp0_change,m3_scx_low_3_bits,m3_scx_high_5_bits,m3_scy_change}.gb`, `mooneye acceptance/ppu/{hblank_ly_scx_timing-GS,intr_2_mode0_timing_sprites,lcdon_timing-GS,lcdon_write_timing-GS}.gb`, `hacktix/strikethrough.gb`, and `blargg oam_bug/{4-scanline_timing,5-timing_bug}.gb`.
- When a candidate fix touches `STAT`, LCD restart, or sprite-coupled mode boundaries, rerun the narrow mooneye LCD timing slice before trusting any localized improvement.

### Phase 5 — Input and simple peripherals

- None currently.

### Phase 6 — Banked cartridges, special cartridges, and cartridge persistence

- [CARTRIDGE][MBC3-LATCH-RELATCH-POLICY] MBC3 currently keeps a deliberate compatibility deviation for `cpp/latch-rtc-test.gb`: the first RTC latch still requires `0x00 -> 0x01`, but follow-up non-zero writes are also accepted once a valid snapshot exists because instrumentation of that ROM showed repeated non-zero relatch commands without re-arming zeros. Revisit that legacy relatch rule if curated oracle policy moves back toward the stricter `Pan Docs` model.
- [CARTRIDGE][MBC3-RTC-INVALID-BANKS] MBC3 keeps `0x04..=0x07` as explicit reserved selectors instead of widening standard SRAM banking to `$00-$07`. Current `Pan Docs` wording says `$00-$07` are RAM-bank selectors, but the retained curated `cpp/rtc-invalid-banks-test.gb` oracle only stays green when those selectors remain invalid. Revisit only if stronger hardware evidence or a better oracle closes that source conflict.
- [CARTRIDGE][MBC3-RTC-ACCESS-SPACING] MBC3 records the recommended RTC access-spacing state as `rtc_access_ready_at` on timed RTC-register reads and writes, but the emulator still treats that state as advisory only. `Pan Docs` recommends `4 us` spacing without defining an early-access penalty, and the current `SameBoy` cross-check does not expose one either. Keep enforcement deferred until a stronger dedicated oracle or hardware evidence exists.
- [CARTRIDGE][HEADER-CGB-TITLE-DISCRIMINATOR] The cartridge-header parser now preserves `0x013F-0x0142` separately but still decodes CGB-era titles conservatively as `15` visible characters. `Pan Docs` documents an additional `11`-character layout when those bytes are really a manufacturer code, but the raw header does not provide a reliable discriminator. Revisit only if stronger hardware evidence or a clearly scoped per-ROM metadata rule can separate the two layouts without truncating valid `15`-character titles.
- [CARTRIDGE][MMM01-UNMAPPED-SEAMS] The new MMM01 baseline now loads through the trailing menu header and models explicit `unmapped` versus `mapped` mode, but two hardware seams remain intentionally unresolved: whether external RAM is accessible at all while still in unmapped mode, and whether the `4000-7FFF` unmapped window ever reflects the transient low-bit note that Pan Docs still labels "to be verified". Evidence already in hand: Pan Docs documents both seams as unknown, and the current code keeps one explicit conservative policy (`unmapped` uses the last `32 KiB` menu window and blocks RAM access) instead of burying guesses in generic MBC1 logic. Do not replace that explicit policy with ad hoc MBC1-style behavior first. Highest-value next step: find a stronger oracle or hardware-backed trace for the unmapped menu phase.
- [CARTRIDGE][MMM01-ORACLE] MMM01 now has synthetic unit and integration coverage for trailing-header promotion, menu startup mapping, mapped-mode game selection, and non-multiplex RAM banking, but it still lacks an oracle comparison against a trusted commercial MMM01 implementation. Evidence already in hand: the current baseline passes the repo-local MMM01 cartridge tests and no longer falls back to MBC1. Do not claim full closure of point `2` until at least one trusted oracle or commercial ROM comparison confirms the selected startup and mapping rules. Highest-value next step: materialize one commercial MMM01 ROM or trusted external oracle and compare the menu-to-game transition behavior.
- [CARTRIDGE][CGB-GATE-SPECIAL-CARTS] The DMG-only special-cartridge roadmap intentionally stops after the DMG-relevant runtime block (`MMM01`, `MBC1M`, `HuC1`, `HuC-3`, `M161`). The remaining CGB-only follow-ups (`MBC30`, `MBC7`, `MBC6`) are blocked until the base CGB implementation can boot and validate CGB-only titles end to end. Evidence already in hand: header classification, typed variant space, and no-fallback diagnostics for all of those families already exist. Do not spend DMG-phase time trying to "close" those three mappers through unit-only or synthetic-only runtime work first. Highest-value next step when this reopens: close the base CGB bring-up gate described in `docs/hardware/CGB.md`.
- [CARTRIDGE][MBC30-AFTER-CGB] `MBC30` is still planned-variant work only: `MBC3 + 64 KiB SRAM` already classifies explicitly, but functional banking, persistence, and software validation remain open. Evidence already in hand: the loader reserves explicit `MBC30` variant space and standard `MBC3` validation already rejects the `64 KiB` SRAM case instead of silently accepting it. Do not fold `MBC30` into ordinary `MBC3` before the CGB gate closes. Highest-value next step after CGB bring-up: validate one explicit `MBC30` device against a real CGB software oracle.
- [CARTRIDGE][MBC7-AFTER-CGB] `MBC7` remains classification-only until CGB exists; EEPROM register semantics, accelerometer plumbing, and end-to-end software validation are still open. Evidence already in hand: header code `0x22` is already diagnosed explicitly and no fallback to `MBC5` remains. Do not start from `MBC5 + rumble + RAM` shortcuts. Highest-value next step after CGB bring-up: design a cartridge-local EEPROM + accelerometer device contract and validate it against a CGB-only title.
- [CARTRIDGE][MBC6-AFTER-CGB] `MBC6` remains classification-only until CGB exists; split ROM/RAM windows, flash semantics, and end-to-end software validation are still open. Evidence already in hand: header code `0x20` is already diagnosed explicitly and the loader never falls back to `MBC3` or `MBC5`. Do not start by coercing it into ordinary banked ROM / SRAM logic. Highest-value next step after CGB bring-up: model one explicit `MBC6` cartridge device with split-window and flash state before attempting title-specific behavior.

### Phase 7 — Audio

- [APU][CGB-CH3-WAVE-RAM-ACTIVE-MMIO] The current CH3 active wave-RAM MMIO contract is only specified for the DMG family. DMG coverage already locks the fetch-window policy and DMG retrigger-corruption lane, but CGB-family active-access redirection remains intentionally deferred because the repo scope is still DMG-only. Do not treat the current `ConsoleModel::Cgb` fallback path as hardware-accurate or add tests/docs that claim a final CGB contract before the CGB APU lane exists.
- [APU][EXTRA-LENGTH-CLOCKING-CGB-REVISION] CH1/CH2/CH3/CH4 extra-length clocking is now wired through an explicit per-model policy seam, but the current `ConsoleModel` surface still cannot distinguish the documented `CGB-02` exception from later CGB revisions. The code therefore keeps the generic DMG/later-CGB rule as a conservative fallback even for `ConsoleModel::Cgb`. Do not claim revision-accurate CGB extra-length clocking until a revision-scoped model or stronger oracle closes that gap.
- [APU][SKIPBOOT-HIDDEN-STATE] Direct boot currently reconstructs the visible audio snapshot, powered state, wave-RAM startup policy, channel-active mask, and shared-divider-derived `DIV-APU` phase, but it still resets other hidden APU state from repo-local defaults. HPF history, pulse duty-step/timer continuation, CH3 sample-buffer/sample-index continuation, and CH4 LFSR/noise-timer continuation are not yet verified boot-handoff state. Keep docs/tests explicit about that narrower contract until a stronger oracle or hardware-backed startup model closes the gap.
- [APU][HOST-RESAMPLER-KERNEL-TUNING] The host-facing capture path now applies a causal windowed-sinc band-limited resampler to the post-HPF T-cycle stream, which is a large step up from simple interval averaging. However, SameBoy still uses a more specialized edge-driven synthesis path with tuned kernel/heuristic details, so the current resampler kernel width, cutoff margin, phase count, and transient behavior remain tuning targets if high-pitched commercial content still sounds harsher than the oracle.
- [APU][CH4-NR43-LIVE-GLITCH-MATRIX] CH4 DMG/pre-`CGB-D` now ports SameBoy's hidden noise-counter state more literally: the running hidden counter/divider countdown now serves as the authoritative phase source for ordinary CH4 LFSR stepping, the DMG trigger path honors SameBoy's delayed-start seam and divisor-`0` / `alignment == 3` visible-`0x0055` quirk, and the live-write path evaluates one actual `old -> FF` write plus one actual `FF -> new` write (with the optional low-shift follow-up) against that preserved counter state instead of against a repo-local synthetic seed. The countdown/delayed-start bookkeeping now also lives in an explicit `2 MHz` subdomain rather than in doubled T-cycle counters, CH4's hidden `alignment` timebase now keeps advancing while `NR52` is powered off just like SameBoy's APU run loop, and `NR52` power-on now explicitly re-seeds the CH4 startup phase while leaving the rest of the hidden counter state intact. A follow-up startup trace also found and fixed a repo bug where powered-on CH4 advanced `alignment` twice per `2 MHz` step. With those fixes, the first Zelda thunder trigger at `pc=0x7BB6` and the later retrigger at `pc=0x4136` now line up with SameBoy's immediate-vs-delayed trigger choice, counter value, delayed-start seam, and `alignment mod 4`; the remaining work is therefore no longer the broad startup handshake, but the later CH4 audio revalidation against SameBoy's isolated output.
- [APU][ZOMBIE-MODE-REVISION-MATRIX] CH1/CH2/CH4 now model the cross-revision-consistent manual increment path for live `NR12` / `NR22` / `NR42` writes (`increase` with pace `0` increments the current volume modulo `16`), but the broader zombie-mode write matrix still varies by hardware revision. Do not claim a fully solved DMG zombie-mode contract until a revision-scoped oracle or hardware-backed policy closes the rest of that matrix.
- [APU][DAC-OFF-FADE-TUNING] The APU now models DAC disable as a short explicit per-channel analog discharge toward `0` on the T-cycle timeline, which is a closer fit to Pan Docs than the previous instantaneous step. However, Pan Docs still says the exact fade varies by model, and the current repo policy uses one conservative SameBoy-aligned decay duration/curve across the supported models. Keep per-model tuning and hardware-backed fade-shape validation open until a stronger oracle closes that gap.

### Phase 8 — Full emulator save states and global serialization strategy

- None currently.

### Phase 9 — Final DMG hardening, differential validation, and closure

- None currently.
