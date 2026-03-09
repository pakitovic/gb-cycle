# Testing

## Testing strategy
Use multiple layers:
- focused unit tests,
- subsystem integration tests,
- ROM-based validation,
- oracle comparisons where useful.

## ROM-based validation policy
Map tests to the subsystem they validate:
- CPU / instruction tests
- timer / interrupt tests
- PPU / LCD tests
- DMA tests
- APU tests
- cartridge / mapper tests
- CGB-specific tests

## Recommended external validation sources
- blargg test ROMs
- Mooneye tests
- dmg-acid2 / cgb-acid2
- mealybug-tearoom-tests

## Behavioral cross-check policy
When a change affects observable timing or ordering:
- compare against SameBoy when possible,
- optionally compare against BGB or another trusted oracle externally,
- record any intentional deviation.

## Regression policy
Every fixed bug should leave behind:
- a focused automated test, or
- a documented ROM-based reproduction case.
