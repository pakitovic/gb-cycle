# BUS

## Scope

Own address decoding, subsystem routing, and visible memory access ordering.

## Hardware model

The bus is not just a convenience table. It is where address ownership, access restrictions, and observable ordering become explicit.

## Responsibilities

- route reads and writes by address range
- expose MMIO ownership clearly
- keep arbitration and blocking rules visible

## Registers / MMIO

- full memory map routing
- shared access to cartridge, VRAM, WRAM, OAM, HRAM, and MMIO registers

## Timing / accuracy requirements

- Bus-visible ordering must remain explicit.
- Access restrictions from PPU and DMA must not be hidden.

## Dependencies

- memory/MMIO map
- cartridge/MBC
- PPU, DMA, timer, interrupt controller, joypad, serial, APU

## Primary references

- Pan Docs memory map sections
- AntonioND timing material
- Gekkio documentation and tests

## Open-source emulator references

Priority order:

1. SameBoy
2. binjgb
3. GameRoy
4. Mooneye GB
5. Gambatte

## Tests

- Mooneye memory and MMIO behavior tests
- subsystem-specific access restriction tests

## Implementation notes for this repo

- Keep cartridge logic decoupled from the rest of the bus.
- Favor explicit maps and handlers over opaque indirection.

## Known pitfalls

- accidental coupling between unrelated devices
- hiding blocked reads/writes behind generic memory helpers

## Open questions

- whether scheduler ownership should live above the bus or beside it
