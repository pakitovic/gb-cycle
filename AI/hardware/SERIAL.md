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

## Implementation notes for this repo

- Keep the hardware serial model separate from any eventual link backend.
- Let bus/MMIO wiring expose `SB` and `SC` at their mapped addresses while the serial subsystem owns transfer semantics.
- Request the serial interrupt through the shared interrupt-controller path instead of reaching into CPU interrupt state directly.

## Known pitfalls

- treating serial as purely frontend-defined I/O

## Open questions

- what minimal stub behavior is acceptable before true link support exists
