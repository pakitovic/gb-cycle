# CPU

## Scope

Own the SM83 CPU execution model: registers, instruction flow, interrupt acceptance, `HALT`, `STOP`, `EI`, `DI`, and CPU-visible timing.

## Hardware model

Model opcode fetch, decode, and execute as explicit phases. Keep instruction semantics separate from timing/accounting decisions so timing refinements do not require rewriting instruction meaning.

## Responsibilities

- register file and flag behavior
- instruction decode and execution semantics
- interrupt acceptance points and delayed enable behavior
- `HALT`/`STOP` edge cases

## Registers / MMIO

- `AF`, `BC`, `DE`, `HL`, `SP`, `PC`
- `IME` and CPU halt/stop internal state

## Timing / accuracy requirements

- Make the stepping granularity explicit: instruction, M-cycle, or T-cycle.
- Do not hide interrupt and halt behavior behind coarse instruction batching.
- Preserve the ordering between fetch, interrupt checks, and state transitions.

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

## Known pitfalls

- `HALT` bug behavior
- delayed `EI`
- interrupt acceptance ordering
- assuming instruction-level stepping is always sufficient

## Open questions

- what stepping granularity the first implementation should target
- where to draw the boundary between CPU timing and scheduler ownership
