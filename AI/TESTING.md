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
