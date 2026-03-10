# AI Handbook Index

Read the matching file directly; this index is only a routing guide.

## Global docs

- `ARCHITECTURE.md`: project goals, crate layout, subsystem boundaries, and portability rules.
- `EXECUTION.md`: implementation workflow and change policy.
- `CODING-RULES.md`: Rust design rules, API style, and optimization discipline.
- `REFERENCES.md`: primary documentation, hardware research, and open-source consultation order.
- `TESTING.md`: unit, integration, ROM-based, and oracle-comparison strategy.
- `TIMING-AND-ACCURACY.md`: accuracy terminology, confidence levels, and timing expectations.

Subsystem handbooks may refine the generic reference consultation order from `REFERENCES.md` when a specific subsystem has a stronger oracle.

The project-wide timing baseline is T-cycle based; see `TIMING-AND-ACCURACY.md` and `hardware/CPU.md`.
The project-wide CPU baseline is a fine-grained fetch/decode/execute model with explicit bus-visible steps; see `hardware/CPU.md`.
The project-wide PPU baseline is dot-by-dot with explicit fetcher/FIFO behavior; see `hardware/PPU.md`.

## Hardware docs

- `hardware/CPU.md`
- `hardware/BUS.md`
- `hardware/MEMORY.md`
- `hardware/INTERRUPTS.md`
- `hardware/TIMER.md`
- `hardware/PPU.md`
- `hardware/DMA.md`
- `hardware/APU.md`
- `hardware/JOYPAD.md`
- `hardware/SERIAL.md`
- `hardware/CARTRIDGES-MBC.md`
- `hardware/BOOT-ROM.md`
- `hardware/CGB.md`
- `hardware/SGB.md`

Each hardware file should capture:

- what the subsystem owns
- which registers and timing rules matter
- what must remain explicit in code
- the best primary references
- the best emulator references for comparison
- the most relevant tests and pitfalls

## Research docs

- `research/SAMEBOY.md`
- `research/BINJGB.md`
- `research/GAMEROY.md`
- `research/ACCURATEBOY.md`
- `research/MOONEYE-GB.md`
- `research/DANGER-BOY.md`
- `research/GAMBATTE.md`

Use research docs as implementation cross-checks, not as the source of truth.
