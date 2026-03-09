# Timing and Accuracy

## Accuracy policy
This project aims for hardware-faithful behavior, with special care for timing-sensitive subsystems.

## Important distinction
Do not use "cycle accurate" loosely.
Track accuracy by subsystem:
- CPU execution timing
- bus access timing
- timer edge behavior
- interrupt ordering
- PPU mode timing
- pixel fetch / FIFO timing
- DMA timing
- APU sequencing and output timing

## Practical rule
A subsystem should only be described as cycle accurate when there is evidence for that claim.

## Sources of confidence
Use this order:
1. hardware documentation and research,
2. test ROM behavior,
3. comparison with trusted emulators,
4. project-local assumptions.

## Development policy
When implementing timing:
- document the level of confidence,
- document which source supports it,
- avoid claiming global cycle accuracy from local evidence.
