# SERIAL

## Scope

Own serial transfer registers, clocking behavior, and link-port-visible state. Do not own host networking or transport APIs.

## Hardware model

Keep hardware serial state explicit even if link support is stubbed initially.

## Responsibilities

- `SB` and `SC` behavior
- transfer progress state
- interrupt signaling at transfer completion

## Registers / MMIO

- `SB`
- `SC`

## Serial MMIO contract baseline

- `SB` and `SC` should remain owned by the serial subsystem rather than by a generic MMIO array.
- `SC.7` should express transfer requested / transfer in progress semantics, not a decorative latched bit.
- `SC.0` should select internal versus external clock semantics, and CGB-only speed-control bits should not act functional in DMG mode before that behavior is implemented.
- Transfer completion should clear `SC.7` and request the serial interrupt through the interrupt controller.
- When serial transfer is modeled with shifting precision, `SB` reads during an active transfer should be able to reflect the in-progress shifted value rather than a frozen pre-transfer byte.

## Timing / accuracy requirements

- Transfer timing and completion signaling should remain explicit.
- Serial progress should remain compatible with the shared T-cycle timing model of the core.

## Dependencies

- bus/MMIO wiring
- interrupt controller
- T-cycle scheduler or clock source

## Primary references

- Pan Docs serial sections

## Open-source emulator references

- SameBoy
- binjgb

## Tests

- register semantics tests
- completion and interrupt timing tests
- tests for `SC.7` start/in-progress and completion-clears behavior
- tests that serial completion requests the interrupt through `IF`

## Implementation notes for this repo

- Keep the hardware serial model separate from any eventual link backend.
- Let bus/MMIO wiring expose `SB` and `SC` at their mapped addresses while the serial subsystem owns transfer semantics.
- Request the serial interrupt through the shared interrupt-controller path instead of reaching into CPU interrupt state directly.
- Direct-boot startup values for `SB` and `SC` should come from the centralized post-boot snapshot rather than from serial-local guessed reset defaults.

## Known pitfalls

- treating serial as purely frontend-defined I/O

## Open questions

- what minimal stub behavior is acceptable before true link support exists
