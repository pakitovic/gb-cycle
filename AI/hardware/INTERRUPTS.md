# INTERRUPTS

## Scope

Own interrupt request state, enable state, source tracking, and CPU-visible acknowledge behavior.

## Hardware model

Interrupts are edge- and ordering-sensitive. Keep request, mask, and acceptance logic explicit rather than scattering it across subsystems.

## Responsibilities

- represent `IF` and `IE`
- track interrupt sources
- provide clear acknowledge behavior to the CPU

## Registers / MMIO

- `IF`
- `IE`

## Timing / accuracy requirements

- Preserve ordering with CPU execution, `EI`, `DI`, `HALT`, and timer/PPU requests.
- Interrupt request and acknowledge behavior should be reasoned about on the shared T-cycle timeline.

## Dependencies

- CPU
- timer
- PPU
- joypad
- serial

## Primary references

- Pan Docs interrupt sections
- AntonioND timing material
- Gekkio/Mooneye interrupt edge-case research

## Open-source emulator references

- SameBoy
- binjgb
- Mooneye GB
- GameRoy

## Tests

- Mooneye interrupt timing tests
- focused tests for priority, masking, and delayed enable behavior

## Implementation notes for this repo

- Keep source signaling separate from CPU acknowledgement.

## Known pitfalls

- conflating request with acceptance
- hiding delayed effects from `EI`

## Open questions

- whether interrupt controller state should live in CPU-adjacent or bus-adjacent ownership
