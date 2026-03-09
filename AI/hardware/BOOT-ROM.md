# BOOT ROM

## Scope

Own boot ROM mapping, power-up sequencing, direct-boot state definitions, and hardware revision differences visible at startup.

## Hardware model

Distinguish clearly between running through boot ROM code and starting from an already initialized state. DMG and CGB must not share assumed initial state without evidence.

## Responsibilities

- boot ROM enable/disable behavior
- model-specific initial register and memory state
- revision-aware startup configuration

## Registers / MMIO

- boot ROM mapping control
- startup-visible register state

## Timing / accuracy requirements

- Boot ROM transition behavior must remain explicit.
- Direct-boot helpers must document what state they assume and why.

## Dependencies

- CPU
- bus and memory
- model/revision configuration

## Primary references

- Pan Docs boot process sections
- Mooneye documentation and tests
- SameBoy revision handling for comparison

## Open-source emulator references

Priority order:

1. SameBoy
2. Mooneye GB
3. binjgb
4. GameRoy
5. Gambatte

## Tests

- boot ROM presence/disable tests
- model-specific startup state tests

## Implementation notes for this repo

- Keep "after boot" presets separate from real boot ROM execution paths.
- Leave extension points for hardware revisions and variants.

## Known pitfalls

- assuming DMG and CGB initial state are interchangeable
- mixing convenience startup state with verified boot behavior

## Open questions

- which models and revisions to support in the first direct-boot API
