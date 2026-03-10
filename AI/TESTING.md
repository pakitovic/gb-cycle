# Testing

## Testing strategy

Use multiple layers:

- focused unit tests
- subsystem integration tests
- ROM-based validation
- oracle comparisons where useful

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
For direct-boot presets, include model-specific CPU state at `PC = 0x0100`, checksum-derived `F` on DMG/MGB, immediate I/O readback of the published post-boot snapshot, and continuity checks that the first timer and PPU ticks are coherent with that snapshot rather than restarting from zeroed hidden state.
Include explicit tests for unreliable post-boot state policy, such as WRAM, HRAM, external RAM, `OBP0`, and `OBP1`, without presenting those policy choices as proven hardware constants.
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
Include LCD off/on tests for `STAT.mode = 0`, LCD-off VRAM/OAM accessibility, and re-enable without stale LCD STAT behavior.
For CPU execution behavior, include opcode fetch under boot-ROM/cartridge mapping, `imm8`/`imm16` fetch order, register-versus-`(HL)` timing differences, taken-versus-untaken conditional paths, stack byte order, CB-prefix double-fetch behavior, and instructions with internal no-bus steps whenever suitable tests exist.
For CPU interrupt-control behavior, include IE/IF register behavior, delayed `EI`, immediate `DI`, fixed interrupt priority, vector dispatch, `RETI`, `HALT` wake-up semantics, `HALT` bug activation/effect, and separate `STOP` coverage whenever suitable tests exist.
For timer behavior, include internal-counter-derived `DIV`, DIV-write glitches, TAC-write glitches, falling-edge TIMA increments, overflow-window behavior, separate TIMA/TMA write cases before/during/after reload, and timer interrupt timing through `IF` and CPU-visible servicing whenever suitable tests exist.
For bus behavior, include blocked-access cases, boot ROM remapping, next-fetch behavior after `FF50`, and DMA-related contention whenever suitable tests exist.
Include direct-boot routing checks that verify boot ROM is already unmapped, the ordinary cartridge ROM map is visible again across `0x0000-0x7FFF`, and DMG-mode reads of CGB-only registers return `0xFF` whenever suitable tests exist.
Include region-contract tests for fixed ROM, switchable ROM, VRAM, cartridge external space, WRAM, echo RAM, OAM, unusable space, MMIO, HRAM, and `IE`, including aliasing, blocked-access semantics, and ownership-by-device whenever suitable tests exist.
For DMA behavior, include `FF46` source-page selection, full `160`-byte copy correctness, DMG total duration of `640` dots, transfer-progress timing, CPU blocking outside HRAM, HRAM accessibility during DMA, and OAM/LCD interaction whenever suitable tests exist.

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
