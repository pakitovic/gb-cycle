# MEMORY

## Scope

Own internal memory regions and the backing storage behind WRAM, HRAM, and their documented alias relationships, plus only simple storage-backed MMIO fields whose semantics are not owned by another subsystem.

## Hardware model

Memory should reflect distinct hardware regions, not a single flat array with ad hoc exceptions.

DMG exposes simple WRAM behavior, while native CGB mode exposes banked WRAM through the same memory domain. VRAM access rules and VRAM/OAM visibility remain a bus-plus-PPU concern; this subsystem focuses on plain storage regions, aliasing, and backing storage rather than complex MMIO semantics.

## Responsibilities

- represent WRAM and HRAM explicitly
- provide storage-level support for internal RAM aliasing and mirroring
- define ownership boundaries between plain memory, simple storage-backed MMIO, and device-owned MMIO behavior
- provide the underlying storage reached through WRAM and HRAM address aliases without turning those aliases into duplicate buffers

## Registers / MMIO

- WRAM
- HRAM
- WRAM storage reached through the echo-RAM alias

## DMG and CGB memory baseline

- On DMG, `0xC000-0xDFFF` should behave as linear internal WRAM with no active banking.
- In native CGB mode, `0xC000-0xCFFF` maps fixed WRAM bank `0`, `0xD000-0xDFFF` maps the `SVBK`-selected bank, and an `SVBK` value of `0` effectively maps bank `1`.
- `0xE000-0xFDFF` must not be backed by a second RAM allocation; it should resolve to the same observable storage as `0xC000-0xDDFF`.
- Initialization policy for WRAM and HRAM contents is separate from the fact that subsequent access semantics are ordinary RAM behavior.

## Timing / accuracy requirements

- Memory behavior must stay compatible with bus arbitration and subsystem locking rules.
- Power-up and direct-boot initialization should not pretend WRAM or HRAM have one fixed hardware-defined value when the hardware leaves them unreliable.

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
- tests that document the chosen uninitialized WRAM/HRAM policy for direct-boot presets without confusing that policy with real hardware guarantees
- explicit alias tests that prove writes through echo RAM affect WRAM storage and vice versa

## Implementation notes for this repo

- Keep plain storage separate from device-backed MMIO.
- If a mapped field has mixed bits, timing-sensitive readback, or access side effects, its semantics belong to the owning subsystem or routed MMIO contract rather than to `memory/`.
- Avoid closed abstractions that assume WRAM and VRAM cannot be bank-selected; native CGB mode now keeps hidden WRAM banks in the same storage domain while DMG and CGB compatibility mode remain on the non-banked path.
- Treat extensibility of the I/O block and memory-backed regions as part of the baseline design, even before CGB functionality exists.
- Prefer designs where extra CGB banks can be disabled by machine mode rather than requiring a different memory architecture.
- Keep VRAM readiness as an architectural concern without moving VRAM locking or access rules out of the PPU/bus boundary.
- When `SkipBoot` is used, initialize WRAM and HRAM through an explicit uninitialized-memory policy rather than silently zero-filling them as if that were proven hardware behavior.
- Keep that uninitialized-memory policy reproducible for tests while remaining clearly separate from deterministic startup values owned by the boot snapshot.
- Let bus decode own the echo-RAM alias decision; the memory subsystem should expose the shared storage it aliases, not invent a second echo-specific backing store.
- Non-perturbing debug probes may expose the CPU-visible WRAM sample for a WRAM or echo-RAM address, including the effective CGB WRAM bank and bank-local offset, but this must remain a storage annotation path rather than a CPU bus read and must not participate in timing, arbitration, or MMIO side effects.

## Known pitfalls

- flattening all address space into one abstraction
- mixing ownership between bus and memory modules
- modeling storage so rigidly that later banked memory support becomes a rewrite
- treating WRAM or HRAM as documented zeroed memory at power-up because a convenient preset happened to choose zeroes
- allocating echo RAM as independent storage instead of as an alias of WRAM

## Open questions

- how much of echo behavior should be explicit versus delegated through bus mapping
- which default uninitialized-memory policy is most useful for direct-boot validation without overstating hardware certainty
