# CPU

## Scope

Own the SM83 CPU execution model: registers, instruction flow, interrupt acceptance, `HALT`, `STOP`, `EI`, `DI`, and CPU-visible timing.

## Hardware model

Model opcode fetch, decode, and execute as explicit phases. Keep instruction semantics separate from timing/accounting decisions so timing refinements do not require rewriting instruction meaning.

For this project, the CPU timing model should be expressed in T-cycles as the fundamental unit. M-cycles may still be useful as a descriptive grouping, but not as the core execution granularity.
Interrupt acceptance, `EI` delay, `DI`, `HALT`, `HALT` bug, `RETI`, and `STOP` should be treated as explicit CPU control-flow states, not as ad hoc patches attached to unrelated bus or interrupt code.

## Responsibilities

- register file and flag behavior
- instruction decode and execution semantics
- IME state and delayed enable behavior
- interrupt acceptance and dispatch timing
- `HALT`, `HALT` bug, `STOP`, and `RETI` edge cases

## Registers / MMIO

- `AF`, `BC`, `DE`, `HL`, `SP`, `PC`
- `IME`, delayed-IME-enable state, and CPU halt/stop internal state

## Interrupt acceptance baseline

- A pending interrupt condition should be derived from `IE & IF`, not from device-specific flags scattered around the CPU.
- The fixed interrupt priority order is `VBlank > LCD STAT > Timer > Serial > Joypad`.
- The corresponding vectors are `0x40`, `0x48`, `0x50`, `0x58`, and `0x60`.
- The CPU should only accept maskable interrupts at defined points in the instruction-flow pipeline, effectively at instruction boundaries or an equivalent explicitly modeled acceptance point.
- When an interrupt is accepted, the CPU should clear `IME`, clear the selected bit in `IF`, push `PC`, and jump to the matching vector as part of one explicit service sequence.

## IME, HALT, and STOP baseline

- `IME` is a CPU-internal acceptance gate, distinct from the `IE` register mask.
- `DI` clears `IME` immediately.
- `EI` must not enable `IME` immediately; it should arm a delayed enable that becomes visible only after the following instruction completes.
- `HALT` should be represented as an explicit CPU state distinct from ordinary instruction execution.
- `STOP` should be represented distinctly from `HALT`; even before full DMG/CGB STOP behavior is implemented, the architecture must leave it as a separate CPU control state.
- The `HALT` bug must be represented explicitly as a pending effect on the next opcode fetch rather than flattened into a generic "PC did not increment" shortcut.

## Timing / accuracy requirements

- Use T-cycle stepping as the baseline execution granularity for this core.
- Treat M-cycles as a derived grouping of four T-cycles, not as the primary scheduling unit.
- Do not hide interrupt and halt behavior behind coarse instruction batching.
- Preserve the ordering between fetch, interrupt checks, and state transitions.
- Keep CPU memory access timing visible at the T-cycle level so VRAM/OAM locking, DMA interaction, and interrupt ordering can be modeled without later restructuring.
- `EI` delay must be tied to instruction completion, not to an unrelated timer or immediate write-back.
- The sequence `EI ; DI` must not leave a window where an interrupt is accepted between those two instructions.
- Interrupt dispatch must not be modeled as an instantaneous jump detached from the CPU timing flow; the service sequence should consume its real CPU-side steps.
- `HALT` wake-up and interrupt dispatch are related but distinct events; waking from `HALT` with `IME = 0` must not be collapsed into automatic interrupt service.
- The `HALT` bug condition is `HALT` executed with `IME = 0` and `IE & IF != 0`; it must alter the next fetch without pretending an interrupt was serviced.

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
- focused tests for `HALT`, `HALT` bug, `STOP`, `EI`, `DI`, `RETI`, and interrupt timing
- interrupt-priority tests with multiple simultaneous pending requests
- tests for correct push of `PC`, clearing of `IF`, and `IME -> 0` on interrupt service
- tests for `EI ; NOP`, `EI ; DI`, `DI ; EI ; NOP`, and pending-IRQ visibility around delayed `EI`
- tests for `HALT` wake-up with `IME = 1`, `IME = 0`, and `IME = 0` plus already-pending interrupt
- tests for `RETI` re-enabling interrupts and allowing later pending requests to be serviced

## Implementation notes for this repo

- Prefer APIs that expose hardware phases explicitly.
- Keep instruction semantics and timing data separable.
- If helper APIs summarize instruction timing, they should still expand into per-T-cycle execution internally.
- The CPU should own `IME`, delayed-IME-enable state, `halted`, `stopped`, and any `halt_bug_pending`-style fetch modifier state.
- The interrupt controller should own `IE` and `IF` as observable interrupt state, while bus/MMIO wiring exposes those registers at their mapped addresses.
- A clear split such as `request_interrupt(kind)`, `pending_interrupts()`, and `consume_interrupt(kind)` is preferred over implicit cross-module mutation.
- `RETI` should be implemented as a real instruction with return plus interrupt re-enable semantics, not as `RET` plus an informal external patch.

## Known pitfalls

- `HALT` bug behavior
- delayed `EI`
- implementing `DI` as delayed when it should be immediate
- treating `IME` as equivalent to `IE`
- interrupt acceptance ordering
- ignoring fixed interrupt priority when several requests are pending
- modeling `HALT` as "sleep until vector jump" instead of separating sleep, wake-up, and service
- assuming instruction-level stepping is always sufficient
- treating M-cycle totals as enough to model timing-sensitive hardware interaction

## Open questions

- which internal fetch-state representation best expresses the `HALT` bug without obscuring normal opcode fetch flow
