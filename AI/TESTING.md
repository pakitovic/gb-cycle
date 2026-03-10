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
For PPU behavior, prioritize tests that expose dot timing, variable Mode 3 length, fetcher/FIFO correctness, STAT timing, and sprite interaction.
Include coverage for Mode 2 OAM blocking, OAM-order sprite selection, and the `10`-sprites-per-scanline limit when suitable tests exist.
Include DMG STAT quirk coverage and avoid assuming the same result on GBC-in-DMG-mode without validation.
Include coverage for Mode 3 startup cost, SCX-dependent timing, window-trigger timing, and sprite-induced stalls when suitable tests exist.
For CPU execution behavior, include opcode fetch under boot-ROM/cartridge mapping, `imm8`/`imm16` fetch order, register-versus-`(HL)` timing differences, taken-versus-untaken conditional paths, stack byte order, CB-prefix double-fetch behavior, and instructions with internal no-bus steps whenever suitable tests exist.
For CPU interrupt-control behavior, include IE/IF register behavior, delayed `EI`, immediate `DI`, fixed interrupt priority, vector dispatch, `RETI`, `HALT` wake-up semantics, `HALT` bug activation/effect, and separate `STOP` coverage whenever suitable tests exist.
For timer behavior, include internal-counter-derived `DIV`, DIV-write glitches, TAC-write glitches, falling-edge TIMA increments, overflow-window behavior, separate TIMA/TMA write cases before/during/after reload, and timer interrupt timing through `IF` and CPU-visible servicing whenever suitable tests exist.
For bus behavior, include blocked-access cases, boot ROM remapping, next-fetch behavior after `FF50`, and DMA-related contention whenever suitable tests exist.
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
- Keep tests that exercise real boot ROM execution separate from tests that start after boot.
