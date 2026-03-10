# JOYPAD

## Scope

Own the joypad register view, button selection lines, and interrupt request signaling.

## Hardware model

Keep the hardware-visible register behavior separate from host input collection.

## Responsibilities

- `P1/JOYP` register behavior
- button matrix selection handling
- interrupt signaling on input transitions

## Registers / MMIO

- `JOYP`

## `JOYP` contract baseline

- `JOYP` should be implemented as a mixed register, not as a flat stored byte.
- The selection bits belong to the register's writable state, while the low input nibble is read-only and derived from the current button matrix state.
- The low nibble is active-low: a pressed button reads back as `0`.
- If neither button row nor d-pad row is selected, the low nibble should read back as all released, `0xF`.
- `JOYP` reads should be side-effect free unless later hardware evidence proves otherwise; software that reads repeatedly to stabilize input should see the current matrix state rather than a frontend-written echo.
- The frontend/input adapter should update a hardware-facing button state, not write bytes directly into `JOYP`.

## Timing / accuracy requirements

- Preserve hardware-visible register semantics even if host input arrives asynchronously.
- `JOYP` reads should observe the current selected rows and current input state at the instant of the MMIO read.

## Dependencies

- bus/MMIO wiring
- interrupt controller
- frontend input adapter boundary

## Primary references

- Pan Docs joypad sections

## Open-source emulator references

- SameBoy
- binjgb
- GameRoy

## Tests

- register behavior tests
- interrupt signaling tests
- separate tests for button-row selection and d-pad-row selection
- active-low readback tests
- tests that `0x30` selection reads the low nibble as no buttons pressed
- tests that interrupt generation is driven from the same underlying input-state transitions observed through `JOYP`

## Implementation notes for this repo

- Keep host key mapping outside the emulation core.
- Let bus/MMIO wiring expose `JOYP` at its mapped address while the joypad subsystem owns the register semantics.
- Request the joypad interrupt through the shared interrupt-controller path instead of mutating CPU interrupt state directly.
- Direct-boot startup values such as the documented post-boot `P1` snapshot should be injected through the centralized boot-state path rather than hard-coded as a local joypad reset default.

## Known pitfalls

- mixing frontend input API details into joypad logic

## Open questions

- whether input sampling should happen per T-cycle, per frame, or via latched events
