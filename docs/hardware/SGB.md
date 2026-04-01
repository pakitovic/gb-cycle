# SGB

## Scope

Track future Super Game Boy and Super Game Boy 2 support boundaries without polluting the DMG/CGB core prematurely.

## Hardware model

SGB should be treated as a future extension layer with explicit boundaries, not as ad hoc special cases inside unrelated subsystems.

## Responsibilities

- document likely integration points
- capture future command, border, and timing concerns
- protect current APIs from blocking later SGB work

## Registers / MMIO

- mainly shared through existing DMG interfaces and external command interpretation

## Timing / accuracy requirements

- Do not make DMG/CGB architectural shortcuts that would prevent later SGB support.

## Dependencies

- joypad/serial-like command paths
- model/revision configuration
- frontend/display integration boundaries

## Primary references

- Pan Docs SGB sections
- future subsystem-specific research

## Open-source emulator references

- SameBoy for architectural comparison when needed

## Tests

- add once SGB work starts

## Implementation notes for this repo

- Keep SGB concerns out of the core until a clear design is needed, but leave clean extension points.

## Known pitfalls

- hard-coding DMG assumptions that later block SGB behavior

## Open questions

- how to stage SGB support without destabilizing DMG/CGB correctness
