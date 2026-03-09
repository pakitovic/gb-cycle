# MEMORY

## Scope

Own internal memory regions and the documented map of WRAM, HRAM, echo behavior, and MMIO-facing storage that is not owned by another subsystem.

## Hardware model

Memory should reflect distinct hardware regions, not a single flat array with ad hoc exceptions.

## Responsibilities

- represent WRAM and HRAM explicitly
- model region-specific behavior and mirroring
- define ownership boundaries between plain memory and MMIO devices

## Registers / MMIO

- WRAM
- HRAM
- echo RAM behavior if modeled

## Timing / accuracy requirements

- Memory behavior must stay compatible with bus arbitration and subsystem locking rules.

## Dependencies

- bus
- model/revision configuration

## Primary references

- Pan Docs memory map
- Gekkio references for hardware differences

## Open-source emulator references

- SameBoy
- binjgb
- GameRoy

## Tests

- memory map tests
- mirror and access boundary tests

## Implementation notes for this repo

- Keep plain storage separate from device-backed MMIO.

## Known pitfalls

- flattening all address space into one abstraction
- mixing ownership between bus and memory modules

## Open questions

- how much of echo behavior should be explicit versus delegated through bus mapping
