# CPU

## Scope

Own the SM83 CPU execution model: registers, instruction flow, interrupt acceptance, `HALT`, `STOP`, `EI`, `DI`, and CPU-visible timing.

## Hardware model

Model opcode fetch, decode, and execute as explicit phases. Keep instruction semantics separate from timing/accounting decisions so timing refinements do not require rewriting instruction meaning.

For this project, the CPU timing model should be expressed in T-cycles as the fundamental unit. M-cycles may still be useful as a descriptive grouping, but not as the core execution granularity.

## Responsibilities

- register file and flag behavior
- instruction decode and execution semantics
- interrupt acceptance points and delayed enable behavior
- `HALT`/`STOP` edge cases

## Registers / MMIO

- `AF`, `BC`, `DE`, `HL`, `SP`, `PC`
- `IME` and CPU halt/stop internal state

## Timing / accuracy requirements

- Use T-cycle stepping as the baseline execution granularity for this core.
- Treat M-cycles as a derived grouping of four T-cycles, not as the primary scheduling unit.
- Do not hide interrupt and halt behavior behind coarse instruction batching.
- Preserve the ordering between fetch, interrupt checks, and state transitions.
- Keep CPU memory access timing visible at the T-cycle level so VRAM/OAM locking, DMA interaction, and interrupt ordering can be modeled without later restructuring.

## Dependencies

- bus access API
- interrupt controller state
- model/revision configuration

## Primary references

- Pan Docs
- AntonioND cycle-accurate docs
- Gekkio CPU/material where applicable

## Open-source emulator references

Priority order:

1. SameBoy
2. binjgb
3. GameRoy
4. Danger Boy
5. Gambatte

## Tests

- blargg CPU instruction tests
- Mooneye CPU and interrupt edge-case tests
- focused tests for `HALT`, `STOP`, `EI`, `DI`, and interrupt timing

## Implementation notes for this repo

- Prefer APIs that expose hardware phases explicitly.
- Keep instruction semantics and timing data separable.
- If helper APIs summarize instruction timing, they should still expand into per-T-cycle execution internally.

## Known pitfalls

- `HALT` bug behavior
- delayed `EI`
- interrupt acceptance ordering
- assuming instruction-level stepping is always sufficient
- treating M-cycle totals as enough to model timing-sensitive hardware interaction

## Open questions

- where to draw the boundary between CPU timing and scheduler ownership
