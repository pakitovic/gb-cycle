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
For PPU behavior, prioritize tests that expose dot timing, variable Mode 3 length, fetcher/FIFO correctness, STAT timing, and sprite interaction.
Include coverage for Mode 2 OAM blocking, OAM-order sprite selection, and the `10`-sprites-per-scanline limit when suitable tests exist.
Include DMG STAT quirk coverage and avoid assuming the same result on GBC-in-DMG-mode without validation.
Include coverage for Mode 3 startup cost, SCX-dependent timing, window-trigger timing, and sprite-induced stalls when suitable tests exist.

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
