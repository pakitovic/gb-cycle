# Testing

## Testing strategy

Use multiple layers:

- focused unit tests
- subsystem integration tests
- ROM-based validation
- oracle comparisons where useful

## Authority and scope

- This document owns project-wide validation policy and cross-subsystem testing expectations.
- Detailed subsystem-specific checklists remain owned by the matching `AI/hardware/*.md` handbook.
- When this file repeats subsystem expectations for planning convenience, the subsystem handbook remains the behavioral authority and this file should be updated to match it.
- `AI/ROADMAP.md` may mention validation goals by phase, but it does not replace this testing policy.

## Validation priorities

Every subsystem change should aim to leave behind one of these:

- a focused automated test for the local invariant
- a ROM-based reproduction case
- a documented oracle comparison when timing or ordering is under review
- a characterization test before structural refactors in behavior-sensitive code

## ROM-based validation policy

Map tests to the subsystem they validate:

- CPU and instruction behavior
- bus and mapper behavior
- timer and interrupt ordering
- PPU and LCD timing
- DMA and access blocking
- APU sequencing
- boot-state and model differences
- CGB-specific features

For boot behavior, cover both real boot ROM execution and direct-boot presets when those modes exist.
Include coverage for explicit real-boot versus skip-boot modes, `FF50` handoff timing, boot-ROM overlay versus cartridge visibility, valid versus invalid logo/checksum outcomes, missing-cartridge or `0xFF` header behavior, and model-specific register state such as DMG versus MGB `A` at cartridge entry whenever suitable tests exist.
For direct-boot presets, include model-specific CPU state at `PC = 0x0100`, checksum-derived `F` on DMG/MGB, immediate I/O readback of the published post-boot snapshot, and continuity checks that the first timer, PPU, and APU ticks are coherent with that snapshot rather than restarting from zeroed hidden state.
Include explicit tests for unreliable post-boot state policy, such as WRAM, HRAM, cartridge RAM whether external or mapper-local, `OBP0`, and `OBP1`, without presenting those policy choices as proven hardware constants.
For PPU behavior, prioritize tests that expose dot timing, variable Mode 3 length, fetcher/FIFO correctness, STAT timing, and sprite interaction.
Include coverage for Mode 2 OAM blocking, OAM-order sprite selection, and the `10`-sprites-per-scanline limit when suitable tests exist.
Include sprite-selection tests that prove `Y` drives Mode 2 selection while `X` visibility does not prevent a sprite from consuming one of the `10` slots.
Include DMG OBJ/OBJ priority tests that distinguish selection priority from drawing priority and verify smaller `X` then earlier OAM order.
Include OBJ transparency tests that verify color index `0` is transparent rather than visible white output.
Include BG/OBJ mixing tests that verify the winning OBJ pixel is chosen before the BG-over-OBJ rule is applied.
Include DMG STAT quirk coverage and avoid assuming the same result on GBC-in-DMG-mode without validation.
Include coverage for Mode 3 startup cost, SCX-dependent timing, window-trigger timing, and sprite-induced stalls when suitable tests exist.
Include sprite-edge tests for top and bottom clipping, `8x8` versus `8x16`, and the `SCX & 7` plus `X = 0` timing-sensitive path when suitable tests exist.
Include mid-frame `LCDC.1` and `LCDC.2` toggle coverage when sprite fetch cancel and size-change behavior are implemented.
Include window tests that separate WY latch timing from WX trigger timing and verify WY is latched at Mode 2 start rather than recomputed continuously during the line.
Include tests for BG FIFO clear and fetcher restart when the window starts mid-scanline.
Include tests for the internal window line counter, including reset during VBlank and increment only on lines where window rendering really starts.
Include tests for `WX = 0`, `WX = 166`, and mid-frame `WX`/`WY`/`LCDC.5` glitches when suitable tests exist.
Include DMG tests that verify `LCDC.0 = 0` suppresses window rendering even if `LCDC.5 = 1`.
Include tests where window start or window glitches alter the BG/window stream seen by later sprite mixing without spuriously clearing OBJ FIFO state.
Include tests for live `STAT` readback composition, especially writable enable bits plus live mode and live coincidence bits.
Include tests that `LY` spans `0..=153`, including coincidence behavior during VBlank and across the `153 -> 0` transition.
Include tests that writing `LYC` reevaluates coincidence and the LCD STAT source immediately on the current dot.
Include tests for each LCD STAT mode-source enable path for Mode `0`, Mode `1`, and Mode `2`.
Include LCD STAT tests that verify one shared internal rising-edge source line, including STAT blocking when consecutive enabled sources keep that line high.
Include tests that Mode `3` never acts as a direct LCD STAT interrupt source.
Include tests where entering VBlank can request both VBlank and LCD STAT Mode `1` without collapsing them into one source.
Include DMG-family tests for the `STAT` write quirk in Mode `0`, Mode `1`, Mode `2`, coincidence-active cases, and a negative case for Mode `3`.
Include tests that the mode reported by `STAT` matches the same live state used by the bus for VRAM/OAM blocking decisions.
Include LCD off/on tests for `STAT.mode = 0`, release of ordinary LCD-mode VRAM/OAM restrictions, and re-enable without stale LCD STAT line or coincidence carry-over.
Include tests for `LCDC.7: 1 -> 0` causing immediate LCD/PPU disable, LCD-off white output, and release of ordinary VRAM/OAM mode restrictions.
Include tests for `LCDC.7: 0 -> 1` causing immediate internal PPU restart while visible output stays blank for the first full frame.
Include tests that LCD disable clears in-flight fetcher/FIFO/window/object state so re-enable does not resume a corrupted partial scanline.
Include tests that LCD-off accessibility and DMA-specific blocking still compose correctly instead of one silently erasing the other.
Include tests for one explicit LY policy at disable, during steady LCD-off state, and across LCD re-enable.
Include tests that mid-scanline `LCDC.7` writes take effect immediately rather than waiting for scanline or frame end.
Include DMG-family OAM corruption tests that distinguish ordinary Mode `2` OAM/`FEA0-FEFF` triggers from generic blocked OAM behavior in other modes.
Include tests that the current Mode `2` OAM row is exposed deterministically one row per `4` dots and that the first row remains immune to the basic corruption patterns.
Include tests for distinct read-corruption and write-corruption formulas and for the dedicated `read + inc/dec` versus `write + inc/dec` paths, including the previous-row mutation and copy behavior of the complex `read + inc/dec` case.
Include instruction-family tests for OAM corruption triggers covering `inc rr` / `dec rr`, `[hli]` / `[hld]`, `push` / `pop`, `call` / `ret` / `rst`, interrupt service, and executing from OAM.
Include model-gating tests where DMG-family models trigger the bug and CGB-family models do not.
For CPU execution behavior, include opcode fetch under boot-ROM/cartridge mapping, `imm8`/`imm16` fetch order, register-versus-`(HL)` timing differences, taken-versus-untaken conditional paths, stack byte order, CB-prefix double-fetch behavior, and instructions with internal no-bus steps whenever suitable tests exist.
For CPU interrupt-control behavior, include IE/IF register behavior, delayed `EI`, immediate `DI`, fixed interrupt priority, vector dispatch, `RETI`, `HALT` wake-up semantics, `HALT` bug activation/effect, and separate `STOP` coverage whenever suitable tests exist.
For joypad behavior, include `JOYP` mixed-register readback, high readback on bits `7-6`, active-low matrix semantics, separate button-row versus d-pad-row selection, simultaneous-row selection, visible `High -> Low` interrupt generation, repeated visible transitions, and the documented repo `STOP` wake policy whenever suitable tests exist.
For the current repo DMG-family baseline, treat that `STOP` wake policy as selection-independent button-press wake on the hardware-facing `8` buttons, while still keeping joypad-interrupt generation tied to visible `JOYP` low-nibble transitions.
For serial behavior, include `SB` / `SC` ownership and mixed-register semantics, forced-high DMG readback of the non-functional `SC` bits, DMG master-mode `8192` Hz transfer timing, slave-mode externally clocked progress, disconnected-peer `0xFF` reception, loopback or scripted-peer coverage, intermediate `SB` states during shifting, and serial IRQ request only on eighth-shift completion whenever suitable tests exist.
For timer behavior, include internal-counter-derived `DIV`, DIV-write glitches, TAC-write glitches, falling-edge TIMA increments, overflow-window behavior, separate TIMA/TMA write cases before/during/after reload, and timer interrupt timing through `IF` and CPU-visible servicing whenever suitable tests exist.
For bus behavior, include blocked-access cases, boot ROM remapping, next-fetch behavior after `FF50`, and DMA-related contention whenever suitable tests exist.
Include direct-boot routing checks that verify boot ROM is already unmapped, the ordinary cartridge ROM map is visible again across `0x0000-0x7FFF`, and DMG-mode reads of CGB-only registers return `0xFF` whenever suitable tests exist.
Include region-contract tests for fixed ROM, switchable ROM, VRAM, cartridge external space, WRAM, echo RAM, OAM, unusable space, MMIO, HRAM, and `IE`, including aliasing, blocked-access semantics, and ownership-by-device whenever suitable tests exist.
For cartridge loading and mapper selection, include tests for header parsing of `0x0143`, `0x0146`, `0x0147`, `0x0148`, and `0x0149`, explicit ROM-size versus file-size validation, unsupported-type diagnostics, and the `MBC2` internal-RAM special case whenever suitable tests exist.
For the `No MBC` family, include explicit coverage for `0x00`, `0x08`, and `0x09`, linear `0x0000-0x7FFF` reads with no bank state, ignored `0x0000-0x7FFF` writes, `32 KiB` ROM validation, and explicit diagnostics when No MBC declares impossible ROM or RAM sizes.
Include tests that `0xA000-0xBFFF` distinguishes absent RAM from present linear `8 KiB` RAM and that battery only changes persistence expectations rather than the visible map.
Use No MBC as the first closed cartridge baseline for boot-overlay and post-boot routing checks so `FF50` handoff, `0x0100-0x014F` visibility, and ordinary cartridge reads are validated before mapper banking enters the picture.
For MBC1, include explicit coverage for header types `0x01`, `0x02`, and `0x03`, deterministic power-up state, RAM-enable decode, raw `5`-bit primary bank-register behavior, and the rule that the primary-register `0 -> 1` translation happens before final ROM-size masking rather than after it.
Include MBC1 tests for `0x4000-0x7FFF` bank selection across `64 KiB`, `128 KiB`, `256 KiB`, `512 KiB`, `1 MiB`, and `2 MiB`, including dedicated access cases for banks `0x01`, `0x1F`, `0x21`, `0x41`, and `0x61`, the documented small-ROM case where bank `0` can appear in the high region after masking, and the large-ROM anomaly where `0x20`, `0x40`, and `0x60` resolve as `0x21`, `0x41`, and `0x61` in the switchable region.
Include MBC1 tests for mode `0` versus mode `1`, low-region bank changes on large-ROM cartridges, fixed `8 KiB` RAM versus banked `32 KiB` RAM behavior, disabled-RAM open-bus reads plus ignored writes, immediate visibility of `0x0000-0x7FFF` mapper writes to later accesses on the shared T-cycle timeline, and explicit diagnostics for impossible ROM-size / RAM-size / wiring combinations.
For MBC2, include explicit coverage for header types `0x05` and `0x06`, deterministic power-up state, address-bit-`8` decode across `0x0000-0x3FFF`, raw `4`-bit ROM-bank-register behavior, and the documented `0 -> 1` translation for the switchable ROM window.
Include MBC2 tests for `0x4000-0x7FFF` bank selection across the supported ROM sizes, explicit `256 KiB` maximum validation, and clear diagnostics when an MBC2 image exceeds that ROM limit or declares inconsistent RAM metadata.
Include MBC2 tests for internal `512 x 4-bit` RAM, low-nibble write masking, the chosen high-nibble readback policy, low-`9`-bit echo aliasing between `0xA000-0xA1FF` and `0xA200-0xBFFF`, disabled-RAM open-bus reads plus ignored writes, immediate visibility of MBC2 control writes to later accesses on the shared T-cycle timeline, battery-backed persistence on `0x06`, and warning/error policy when `0x0149 != 0x00` without reinterpreting the cartridge as ordinary external SRAM.
For MBC3, include explicit coverage for header types `0x0F`, `0x10`, `0x11`, `0x12`, and `0x13`, deterministic power-up state, raw `7`-bit ROM-bank-register behavior, typed RAM-versus-RTC selection, and the documented `0 -> 1` translation for the switchable ROM window.
Include MBC3 tests for `0x4000-0x7FFF` bank selection across supported ROM sizes up to `2 MiB`, ordinary access to banks `0x20`, `0x40`, and `0x60`, RAM banking up to standard `32 KiB`, and explicit reservation or diagnostics for MBC30-like `64 KiB` SRAM declarations.
Include MBC3 tests for RAM / RTC enable behavior, latch `0x00 -> 0x01`, live-versus-latched RTC state, halt/carry/day-counter behavior, disabled-RAM / RTC policy, and powered-off elapsed-time handling through an injected deterministic clock.
For MBC5, include explicit coverage for header types `0x19`, `0x1A`, `0x1B`, `0x1C`, `0x1D`, and `0x1E`, deterministic power-up state, raw low-`8` plus high-`1` ROM-bank-register behavior, and the rule that bank `0` remains valid in `0x4000-0x7FFF`.
Include MBC5 tests for `0x4000-0x7FFF` bank selection across supported ROM sizes up to `8 MiB`, including bank `0`, bank `0x1FF`, the `0xFF -> 0x100` boundary, and real-size masking without any MBC1/MBC3-style `0 -> 1` translation.
Include MBC5 tests for RAM-enable behavior, disabled-RAM open-bus reads plus ignored writes, linear `8 KiB` bank selection for `8 KiB`, `32 KiB`, and `128 KiB` SRAM configurations, the absence of any MBC1-style dual banking mode, and the rule that header variants without RAM do not expose fake SRAM semantics merely because the RAM-bank register exists.
Include rumble-capable MBC5 tests that prove `bit 3` of the control register in `0x4000-0x5FFF` updates observable `rumble_on`, that the state remains active until software clears it, and that rumble handling stays distinct from effective RAM-bank selection rather than collapsing both meanings into one opaque integer.
Include MBC5 validation tests for ROM sizes above `8 MiB`, impossible RAM declarations, no-RAM header variants with nonzero `0x0149`, and the failure case where a rumble-capable header is loaded without exposing observable rumble state.
For cartridge persistence, include tests that the saved hardware-style payload is the complete cartridge backing store rather than the currently visible `0xA000-0xBFFF` window, including linear SRAM on `No MBC`, banked SRAM on `MBC1`, `MBC3`, and `MBC5`, plus nibble RAM on `MBC2`.
Include persistence tests that `0x0147` capability decoding, not filename heuristics or `0x0149` alone, decides whether a cartridge auto-produces hardware-style saves, and that cartridges with non-persistent RAM do not do so by default.
Include persistence tests that `ram_enabled` gating does not affect the saved payload contents and that disabled-but-existing cartridge RAM can still round-trip through persistence.
Include `MBC3` persistence tests that serialize live RTC state plus elapsed-time bookkeeping, restore powered-off advancement from an injected deterministic clock, and do not confuse the latched RTC snapshot with the persistent live clock.
Include contract-level tests for in-memory and disk save backends, format versioning, explicit load/save APIs, save-on-close, forced save, optional auto-flush-after-write behavior, and atomic file replacement when storage robustness is under test.
Keep hardware-style cartridge persistence tests separate from full-emulator save-state tests; the former must not require CPU, PPU, APU, WRAM, or other console-state serialization.
For DMA behavior, include `FF46` source-page selection, full `160`-byte copy correctness, DMG total duration of `640` dots, transfer-progress timing, CPU blocking outside HRAM, HRAM accessibility during DMA, and OAM/LCD interaction whenever suitable tests exist.
For APU behavior, include tests that `NR52` power-off clears ordinary audio registers, preserves wave RAM accessibility, and does not reset the `DIV-APU` source relationship whenever suitable tests exist.
Include tests that `DIV-APU` advances from the falling edge of `DIV` bit `4`, including `DIV`-write-induced extra ticks when the edge is produced.
Include tests that the frame sequencer clocks length, envelope, and CH1 sweep without becoming the waveform timer for the channels themselves.
Include tests that `dac_enabled` and `channel_active` stay distinct, that DAC-off forces channel-off, and that `NR52` reports active channels rather than DAC-enabled channels.
Include tests that `NRx4` trigger writes act immediately on the shared timeline and do not activate a channel whose DAC is off.
Include tests for `NR51` stereo routing, `NR50` master-volume semantics including the documented "0 behaves like factor 1" rule, and HPF/DC-offset-sensitive mixer state changes whenever suitable tests exist.
Include APU output-path tests that resolved channel digital outputs feed per-channel DAC conversion before stereo mixing, including the documented negative-slope `0..15 -> -1..1` enabled-DAC mapping.
Include tests that DAC-off output remains distinct from "inactive channel with DAC still enabled" rather than collapsing both cases into one fake digital-`0` path.
Include tests that `NR51` routing changes, `NR50` volume changes, and DAC enable changes affect the live analog mix immediately and generate the documented pop-producing DC-offset transitions through the HPF.
Include tests that the left/right HPF state is persistent and stateful across captured samples instead of acting like a memoryless host post-process.
Include tests that host-facing sample-rate or buffer-size changes do not alter core APU timing, mixer semantics, HPF behavior, or pop generation, and that the core can be validated without a real audio backend.
Include CH1 tests for `NR10`-`NR14` ownership, `NR13` write-only behavior, and immediate `NR14` trigger/length-enable semantics.
Include CH1 tests for period timer cadence, duty-step progression, retrigger-not-resetting-duty-step behavior, and period-write delay until the current sample ends.
Include CH1 tests for length expiry, envelope progression, and the rule that envelope volume reaching `0` does not disable the channel.
Include CH1 sweep tests for trigger-time shadow copy, immediate overflow check, timed writeback, second overflow check, and the rule that `NR13` / `NR14` writes do not update the sweep shadow automatically.
Include dedicated CH1 quirk tests for envelope/sweep timer-reload semantics where programmed pace or period `0` behaves as `8`, extra length clocking, low frequency-timer bits on trigger, and the first-duty-step-after-power-on path whenever suitable tests exist.
Include CH2 tests for `NR21`-`NR24` ownership, `NR23` write-only behavior, and immediate `NR24` trigger/length-enable semantics.
Include CH2 tests for period timer cadence, duty-step progression, retrigger-not-resetting-duty-step behavior, and period-write delay until the current sample ends.
Include CH2 tests for DAC-off behavior, length expiry, envelope progression, and the rule that envelope volume reaching `0` does not disable the channel.
Include dedicated CH2 quirk tests for envelope timer-reload semantics where programmed pace or period `0` behaves as `8`, extra length clocking, low frequency-timer bits on trigger, and the first-duty-step-after-power-on path whenever suitable tests exist.
Include CH3 tests for `NR30`-`NR34` ownership, `NR31`/`NR33` write-only behavior, and wave RAM persistence across `NR52` power-off.
Include CH3 tests for period timer cadence at one tick every `2` dots, `32`-sample index progression, buffered sample fetch from wave RAM, and period-write delay until after the next wave-RAM read.
Include CH3 tests for DAC-off behavior, trigger-not-refilling-the-sample-buffer behavior, length expiry, and `NR32` digital output-level semantics distinct from DAC-off or analog mixer volume.
Include dedicated CH3 quirk tests for digital-`0` startup state, skipped-first-sample / first-buffer behavior, active-channel wave-RAM access policy, trigger-with-length-0 behavior, and DMG-family retrigger corruption keyed both to the exact byte-read position and to the affected aligned source block whenever suitable tests exist.
Include CH4 tests for `NR41`-`NR44` ownership, `NR41` write-only behavior, and immediate `NR44` trigger/length-enable semantics.
Include CH4 tests for `noise_timer` cadence, LFSR progression, and decoded `NR43` behavior including divider `0 -> 0.5`, live width-mode changes, and clock-shift `14` / `15` suppressing LFSR clocks.
Include CH4 tests for DAC-off behavior, trigger-time reset of envelope/LFSR/timer state, length expiry, envelope progression, and the rule that envelope volume reaching `0` does not disable the channel.
Include dedicated CH4 quirk tests for ordinary `15`-bit mode, ordinary `7`-bit mode, lock-up on `15 -> 7` transitions in the documented all-ones states, retrigger recovery from lock-up, and extra length clocking whenever suitable tests exist.

## Recommended external validation sources

- blargg test ROMs
- Mooneye tests
- dmg-acid2 / cgb-acid2
- mealybug-tearoom-tests

## Behavioral cross-check policy

When a change affects observable timing or ordering:

- compare against SameBoy when possible
- compare against another trusted oracle when that helps isolate behavior
- record intentional deviations and their reason

## Determinism policy

- Core execution should be deterministic for the same inputs and model configuration.
- Tests should prefer reproducible stepping and explicit expected state over fuzzy assertions.
- Instrumentation should not change hardware-visible behavior.
- Battery-backed RTC persistence tests must use an injected or otherwise explicit time source rather than the host wall clock.

## Regression policy

Every fixed bug should leave behind:

- a focused automated test, or
- a documented ROM-based reproduction case

## Test organization policy

- Prefer local module tests for unit-level coverage.
- Use top-level `tests/` for integration coverage only.
- When module tests outgrow an inline `tests` block, move them to a co-located test facade such as `foo/tests.rs`.

## Bug traceability policy

- Bug fixes should keep a reproducible description: ROM or test case, observed behavior, and expected behavior.
- For CPU, PPU, timer, interrupts, DMA, memory map, and boot behavior, prefer writing the failing test first when practical.

## Boot and startup policy

- When direct-boot presets are used in tests, document the assumed register and memory state explicitly.
- Document separately which parts of the direct-boot preset are deterministic, cartridge-derived, unreliable by hardware, or synthesized hidden state needed for temporal continuity.
- Keep tests that exercise real boot ROM execution separate from tests that start after boot.
