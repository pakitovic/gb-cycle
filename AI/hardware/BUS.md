# BUS

## Scope

Own address decoding, subsystem routing, and visible memory access ordering.

## Hardware model

The bus is not just a convenience table. It is where address ownership, access restrictions, and observable ordering become explicit.

Even in a DMG-only implementation, treat VRAM, WRAM, OAM, cartridge space, HRAM, and MMIO as distinct controlled regions rather than one rigid flat memory block.

## Responsibilities

- route reads and writes by address range
- expose MMIO ownership clearly
- keep arbitration and blocking rules visible

## Registers / MMIO

- full memory map routing
- shared access to cartridge, VRAM, WRAM, OAM, HRAM, and MMIO registers
- startup mapping of boot ROM over `0000-00FF` and later cartridge handoff

## Timing / accuracy requirements

- Bus-visible ordering must remain explicit.
- Access restrictions from PPU and DMA must not be hidden.
- OAM access blocking during PPU Mode 2 must be represented as observable bus behavior, not as a render-only detail.
- During PPU Mode 3, both OAM and VRAM access restrictions must be represented as observable bus behavior.

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
- Design region ownership so future CGB additions can extend VRAM banking, WRAM banking, extra I/O registers, and HDMA without replacing the bus contract.
- Prefer region controllers or explicit handlers over hard-coded assumptions like "DMG only has one VRAM shape forever".
- The bus should model boot ROM mapping as a first-class routing rule, including the later `FF50`-controlled unmap to cartridge ROM.
- Avoid boot-ROM mapping code that assumes firmware always occupies exactly one small contiguous prefix of the address space.
- Leave room for model-specific boot firmware windows that are not a single contiguous DMG-style range.

## Known pitfalls

- accidental coupling between unrelated devices
- hiding blocked reads/writes behind generic memory helpers
- freezing the MMIO map behind abstractions that are hard to extend for CGB-only registers

## Open questions

- whether scheduler ownership should live above the bus or beside it
