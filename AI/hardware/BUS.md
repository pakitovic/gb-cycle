# BUS

## Scope

Own address decoding, subsystem routing, and visible memory access ordering.

## Hardware model

The bus is not just a convenience table. It is where address ownership, access restrictions, and observable ordering become explicit.

Even in a DMG-only implementation, treat VRAM, WRAM, OAM, cartridge space, HRAM, and MMIO as distinct controlled regions rather than one rigid flat memory block.
Address alone is not enough: the bus must also consider the current temporal hardware state such as PPU mode, LCD enable state, DMA activity, boot ROM mapping, console model, and later CGB banking or speed mode.

## Responsibilities

- route reads and writes by address range
- expose MMIO ownership clearly
- keep arbitration and blocking rules visible
- apply access rules based on the current hardware state, not only the address
- let the CPU perform generic bus accesses without embedding device-specific lock rules in CPU code
- distinguish requester-specific access semantics when CPU, DMA, or other actors do not obey the same rules
- coordinate dynamic mapping between boot ROM, cartridge ROM, and later model-specific extensions
- consume subsystem-owned state such as PPU mode, DMA progress, and boot-ROM enable state when deciding the observable result of an access

## Registers / MMIO

- full memory map routing
- shared access to cartridge, VRAM, WRAM, OAM, HRAM, and MMIO registers
- startup mapping of boot ROM over `0000-00FF` and later cartridge handoff

## Timing / accuracy requirements

- Bus-visible ordering must remain explicit.
- Access restrictions from PPU and DMA must not be hidden.
- Boot-ROM overlay and cartridge handoff must be represented as observable routing behavior, not as a CPU-local switch or a post-boot jump shortcut.
- OAM decisions must consider address, LCD enable state, PPU mode, and OAM DMA state together rather than as unrelated checks.
- OAM access blocking during PPU Mode 2 must be represented as observable bus behavior, not as a render-only detail.
- During PPU Mode 3, both OAM and VRAM access restrictions must be represented as observable bus behavior.
- During DMG OAM DMA, CPU accesses should retain normal HRAM behavior while non-HRAM CPU accesses observe DMA-blocked semantics instead of normal memory-region behavior.
- With LCD disabled, access rules should return to the hardware state expected for LCD-off behavior.
- When an access is blocked, the bus should model the correct observable result for that situation instead of falling through to normal RAM semantics.
- CPU opcode fetch, immediate fetch, stack traffic, and read-modify-write memory operations should appear as ordinary ordered bus accesses, not as post-instruction aggregated effects.

## Dependencies

- memory/MMIO map
- boot subsystem state
- model/revision configuration
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
- tests for blocked reads returning the expected observable value and blocked writes being ignored where applicable
- tests for requester-specific behavior during OAM DMA, including CPU HRAM access and DMA-driven OAM writes
- tests for boot-ROM overlay before `FF50` and cartridge visibility after `FF50`
- tests that the next fetch after boot-ROM unmapping already observes cartridge routing

## Implementation notes for this repo

- Keep cartridge logic decoupled from the rest of the bus.
- Favor explicit maps and handlers over opaque indirection.
- Treat the bus as both an address decoder and an access arbiter.
- A bus context or equivalent state bundle is a good fit for carrying model, PPU mode, LCD enable, DMA activity, boot ROM mapping, and later CGB-specific selectors.
- A caller-aware access split or equivalent internal distinction between CPU-initiated and DMA-initiated accesses is recommended when the observable rules differ.
- Let subsystems define the state that causes restrictions or remapping, but keep the final blocked-access or routing decision in bus-facing handlers.
- Do not special-case CPU opcode fetch, operand fetch, or stack accesses outside the common bus contract; they should use the same routed access path as any other CPU-visible memory transaction.
- Treat `FF46` as the trigger that configures the DMA subsystem; do not implement OAM DMA by performing a direct `160`-byte copy inside the bus write path.
- Treat `FF50` as the trigger that changes boot-ROM mapping state; do not model real boot completion as a synthetic `PC = 0x0100` event outside the bus and CPU execution flow.
- Design region ownership so future CGB additions can extend VRAM banking, WRAM banking, extra I/O registers, and HDMA without replacing the bus contract.
- Prefer region controllers or explicit handlers over hard-coded assumptions like "DMG only has one VRAM shape forever".
- The bus should model boot ROM mapping as a first-class routing rule, including the later `FF50`-controlled unmap to cartridge ROM.
- The DMG-family next-fetch handoff after `FF50` should already be modeled in a way that can later extend to CGB's split boot-ROM mapping while keeping the cartridge header window visible.
- Avoid boot-ROM mapping code that assumes firmware always occupies exactly one small contiguous prefix of the address space.
- Leave room for model-specific boot firmware windows that are not a single contiguous DMG-style range.
- Keep blocked-access behavior inside bus-facing region handlers such as VRAM/OAM access paths rather than teaching the CPU about those rules.

## Known pitfalls

- accidental coupling between unrelated devices
- hiding blocked reads/writes behind generic memory helpers
- treating requester identity as irrelevant when CPU and DMA need different observable access rules
- freezing the MMIO map behind abstractions that are hard to extend for CGB-only registers
- treating the bus as a static memory map without temporal arbitration

## Open questions

- whether scheduler ownership should live above the bus or beside it
