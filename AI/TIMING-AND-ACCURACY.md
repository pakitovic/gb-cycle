# Timing and Accuracy

## Accuracy policy

This project aims for hardware-faithful behavior, with special care for timing-sensitive subsystems.

## Important distinction

Do not use "cycle accurate" loosely. Track accuracy by subsystem:

- CPU execution timing
- bus access timing
- timer edge behavior
- interrupt ordering
- PPU mode timing
- pixel fetch and FIFO timing
- DMA timing
- APU sequencing and output timing

## Practical rule

A subsystem should only be described as cycle accurate when there is evidence for that claim.

## Sources of confidence

Use this order:

1. hardware documentation and research
2. test ROM behavior
3. comparison with trusted emulators
4. project-local assumptions

## Development policy

When implementing timing:

- document the expected ordering of events
- document the level of confidence
- state which source supports the model
- avoid claiming global cycle accuracy from local evidence

## Modeling policy

- Prefer explicit clock ownership.
- Make temporal edges visible in code and tests.
- Do not replace hardware ordering with convenience batching unless the behavior is proven equivalent for the target accuracy.
- Favor clarity and testability over shortcuts that obscure the phase model.
