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
- Timer interrupt requests must remain aligned with the timer's real overflow/reload sequence rather than an oversimplified "overflow happened, so request now" shortcut.
- LCD/STAT timing should stay aligned with PPU mode transitions, including entry into Mode 2.
- When STAT behavior is implemented in detail, preserve the documented DMG-specific STAT write quirk and do not assume the same behavior on GBC running in DMG mode.

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
- timer interrupt timing tests that verify IF request timing relative to TIMA overflow/reload
- timer interrupt integration tests that verify CPU-visible servicing order after the request becomes pending
- LCD/STAT timing tests, including mode transitions and STAT quirk coverage when available

## Implementation notes for this repo

- Keep source signaling separate from CPU acknowledgement.

## Known pitfalls

- conflating request with acceptance
- hiding delayed effects from `EI`
- decoupling STAT/LCD interrupt timing from the real PPU mode schedule
- assuming the DMG STAT write quirk applies unchanged to GBC-in-DMG-mode

## Open questions

- whether interrupt controller state should live in CPU-adjacent or bus-adjacent ownership
