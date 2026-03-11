# AI Handbook Index

Read the matching file directly; this index is a routing guide plus a summary of document authority boundaries, but detailed behavioral rules still live in the owning file.

## Global docs

- `ARCHITECTURE.md`: project goals, crate layout, subsystem boundaries, and portability rules.
- `EXECUTION.md`: implementation workflow and change policy.
- `CODING-RULES.md`: Rust design rules, API style, and optimization discipline.
- `REFERENCES.md`: primary documentation, hardware research, and open-source consultation order.
- `ROADMAP.md`: living implementation order, phase goals, done criteria, and open TODO tracking for deferred work.
- `TESTING.md`: unit, integration, ROM-based, and oracle-comparison strategy.
- `TIMING-AND-ACCURACY.md`: accuracy terminology, confidence levels, and timing expectations.

## Authority map

- `ARCHITECTURE.md` owns crate/module layout, subsystem boundaries, and ownership rules.
- `EXECUTION.md` owns implementation workflow, change-scope discipline, and roadmap-follow-up recording policy.
- `CODING-RULES.md` owns Rust-facing code style, API clarity expectations, and optimization discipline.
- `REFERENCES.md` owns the generic source-consultation order and open-source reference tier unless a subsystem handbook overrides it explicitly.
- `TIMING-AND-ACCURACY.md` owns shared timing vocabulary and project-wide temporal constraints.
- `TESTING.md` owns project-wide validation policy and cross-subsystem testing expectations.
- `ROADMAP.md` owns implementation sequencing, phase context, and carried TODOs; it does not redefine subsystem behavior.
- `hardware/*.md` own subsystem-specific behavior, MMIO semantics, timing expectations, and subsystem-specific validation detail.

When guidance overlaps, the more specific document wins:

- `hardware/*.md` over generic docs for subsystem behavior
- `ARCHITECTURE.md` over `README.md` or roadmap prose for layout and ownership
- `TIMING-AND-ACCURACY.md` over `README.md` or roadmap prose for shared timing claims
- `REFERENCES.md` over generic prose for consultation order unless a subsystem handbook explicitly refines it
- `TESTING.md` over roadmap prose for generic validation policy
- `ROADMAP.md` only for implementation order and remaining work tracking

Subsystem handbooks may refine the generic reference consultation order from `REFERENCES.md` when a specific subsystem has a stronger oracle.

The project-wide timing baseline is T-cycle based; see `TIMING-AND-ACCURACY.md` and `hardware/CPU.md`.
The project-wide CPU baseline is a fine-grained fetch/decode/execute model with explicit bus-visible steps; see `hardware/CPU.md`.
The project-wide PPU baseline is dot-by-dot with explicit fetcher/FIFO behavior; see `hardware/PPU.md`.
Use `ROADMAP.md` when a task needs phase context, when resuming incomplete work, or when documenting known remaining gaps after an implementation.

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
