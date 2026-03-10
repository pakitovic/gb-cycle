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

## Timing / accuracy requirements

- Preserve hardware-visible register semantics even if host input arrives asynchronously.

## Dependencies

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

## Implementation notes for this repo

- Keep host key mapping outside the emulation core.

## Known pitfalls

- mixing frontend input API details into joypad logic

## Open questions

- whether input sampling should happen per T-cycle, per frame, or via latched events
