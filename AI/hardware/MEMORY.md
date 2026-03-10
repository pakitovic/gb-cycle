# MEMORY

## Scope

Own internal memory regions and the documented map of WRAM, HRAM, echo behavior, and MMIO-facing storage that is not owned by another subsystem.

## Hardware model

Memory should reflect distinct hardware regions, not a single flat array with ad hoc exceptions.

DMG may expose only simple WRAM behavior today, but the overall memory architecture should leave room for future bank-selectable regions where the hardware family later requires them.
VRAM access rules and VRAM/OAM visibility remain a bus-plus-PPU concern; this subsystem focuses on plain storage regions and their ownership boundaries.

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
- Avoid closed abstractions that assume WRAM and VRAM can never gain bank-selection behavior.
- Treat extensibility of the I/O block and memory-backed regions as part of the baseline design, even before CGB functionality exists.
- Prefer designs where extra CGB banks can be disabled by machine mode rather than requiring a different memory architecture.
- Keep VRAM readiness as an architectural concern without moving VRAM locking or access rules out of the PPU/bus boundary.

## Known pitfalls

- flattening all address space into one abstraction
- mixing ownership between bus and memory modules
- modeling storage so rigidly that later banked memory support becomes a rewrite

## Open questions

- how much of echo behavior should be explicit versus delegated through bus mapping
