# INTERRUPTS

## Scope

Own interrupt request state, enable state, source tracking, fixed-priority pending selection, and the CPU-visible request/acknowledge interface.

## Hardware model

Interrupts are edge- and ordering-sensitive. Keep request, mask, and acceptance logic explicit rather than scattering it across subsystems.

## Responsibilities

- represent `IF` and `IE`
- track interrupt sources
- expose a centralized interrupt request path for hardware producers
- expose fixed-priority pending selection to the CPU
- provide clear acknowledge/consume behavior to the CPU

## Registers / MMIO

- `IF` at `FF0F`
- `IE` at `FFFF`

## Map-location baseline

- `IE` being located at `0xFFFF` instead of inside `0xFF00-0xFF7F` should stay explicit in bus decode and MMIO wiring.
- `IF` should remain part of the main MMIO range while `IE` is handled as its own high-memory decode case.

## Pending interrupt baseline

- Hardware devices should request interrupts by setting the relevant bit in `IF`, not by invoking CPU dispatch logic directly.
- The effective pending mask should be derived from `IE & IF`.
- When several interrupts are pending at once, the priority order must be `VBlank > LCD STAT > Timer > Serial > Joypad`.
- The interrupt controller should expose the highest-priority pending source as a single choice for CPU dispatch rather than encouraging ad hoc priority checks in multiple places.

## Timing / accuracy requirements

- Preserve ordering with CPU execution, `EI`, `DI`, `HALT`, and timer/PPU requests.
- Interrupt request and acknowledge behavior should be reasoned about on the shared T-cycle timeline.
- A pending request in `IF` should remain observable even when `IME = 0`; masking by `IME` affects CPU acceptance, not whether the request exists.
- Timer interrupt requests must remain aligned with the timer's real overflow/reload sequence rather than an oversimplified "overflow happened, so request now" shortcut.
- LCD/STAT timing should stay aligned with PPU mode transitions, including entry into Mode 2.
- When STAT behavior is implemented in detail, preserve the documented DMG-specific STAT write quirk and do not assume the same behavior on GBC running in DMG mode.

## Dependencies

- CPU
- bus/MMIO wiring
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
- tests for `IF`/`IE` read-write behavior at `FF0F` and `FFFF`
- tests for pending-request visibility with `IME = 0`
- tests for multiple simultaneous pending requests resolving in fixed priority order
- timer interrupt timing tests that verify IF request timing relative to TIMA overflow/reload
- timer interrupt integration tests that verify CPU-visible servicing order after the request becomes pending
- LCD/STAT timing tests, including mode transitions and STAT quirk coverage when available
- direct-boot readback tests for documented startup `IF`/`IE` values when firmware execution is bypassed

## Implementation notes for this repo

- Keep source signaling separate from CPU acknowledgement.
- A helper such as `request_interrupt(kind)` is preferred over handwritten bit-twiddling at each producer site.
- Keep the final decision to accept and dispatch an interrupt in CPU flow, even if priority selection and `IF`/`IE` ownership live here.
- Direct-boot startup values for `IF` and `IE` should be sourced from the centralized post-boot snapshot rather than inferred from CPU-local interrupt state.
- Keep the semantic ownership of `IF` and `IE` here even though bus decode must route `0xFF0F` and `0xFFFF` correctly.

## Known pitfalls

- conflating request with acceptance
- bypassing `IF` by letting hardware call directly into CPU interrupt dispatch
- hiding delayed effects from `EI`
- decoupling STAT/LCD interrupt timing from the real PPU mode schedule
- assuming the DMG STAT write quirk applies unchanged to GBC-in-DMG-mode

## Open questions

- what the narrowest MMIO-facing API is for exposing `IF`/`IE` through the bus without leaking ad hoc bit-twiddling across the codebase
