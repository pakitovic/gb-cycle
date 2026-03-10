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
- Use T-cycles as the fundamental execution unit for the core timing model.
- Use dot or T-cycle level reasoning as the baseline timing vocabulary for the core.
- Treat M-cycles only as a descriptive grouping of four T-cycles, not as the project's primary timing abstraction.
- Keep the timing model clean enough that future CGB double-speed behavior can be expressed as an extension of the same temporal foundation rather than a separate clocking design.

## Practical timing rule

- CPU, PPU, timer, APU, DMA, and bus-visible effects should be expressible on a shared T-cycle timeline.
- Avoid architectures that execute a whole instruction or whole M-cycle and only then advance the rest of the hardware.
- If higher-level helpers exist for ergonomics, they must preserve the same per-T-cycle ordering semantics internally.
