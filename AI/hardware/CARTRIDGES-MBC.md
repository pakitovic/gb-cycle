# CARTRIDGES / MBC

## Scope

Own cartridge ROM/RAM mapping, mapper state, battery-backed persistence interfaces, and mapper-specific features such as RTC or rumble.

## Hardware model

Keep MBC behavior decoupled from the rest of the core. Cartridge hardware should expose a clear mapper contract to the bus rather than leaking mapper rules everywhere.

## Responsibilities

- ROM and external RAM banking
- mapper register writes
- future RTC and rumble support
- cartridge metadata and model capability handling
- ownership of cartridge-visible address ranges `0x0000-0x7FFF` and `0xA000-0xBFFF` once the bus has decoded the access into cartridge space

## Registers / MMIO

- mapper-controlled ROM/RAM banking ranges
- external RAM enable and control ranges

## Timing / accuracy requirements

- Access behavior must remain compatible with bus ordering.
- Architecture should scale from ROM-only to MBC1, MBC3, MBC5, and later extensions.
- Direct-boot initialization should not assume external RAM starts clean unless that follows from persisted save data or an explicit uninitialized-memory policy.
- Writes in ROM address space should be interpreted as cartridge/MBC control behavior where applicable, not as attempts to mutate ROM contents.

## Dependencies

- bus
- persistence boundary for saves/RTC state
- model configuration where needed

## Primary references

- Pan Docs cartridge and MBC sections
- cartridge-specific research and test ROMs

## Open-source emulator references

Priority order:

1. SameBoy
2. binjgb
3. GameRoy
4. Mooneye GB
5. Gambatte

## Tests

- mapper-specific ROM tests
- save RAM behavior tests
- RTC behavior tests when implemented
- tests that document startup behavior for external RAM when direct-boot presets bypass firmware execution
- tests that fixed-ROM, switchable-ROM, and external cartridge ranges are delegated through the cartridge interface rather than treated as internal console memory
- tests that ROM-space writes hit MBC control semantics instead of fake writable ROM

## Implementation notes for this repo

- Keep mapper traits or enums narrow and explicit.
- Avoid hard-coding cartridge logic into generic bus code.
- Treat external RAM power-up contents as separate from deterministic post-boot CPU/MMIO state; if the emulator chooses a direct-boot initialization policy, keep it explicit and configurable.
- Keep active-ROM-bank selection, RAM enable, RAM banking, RTC mapping, and any bank-wrap quirks inside cartridge/MBC implementations rather than generic bus region logic.

## Known pitfalls

- leaking mapper knowledge into unrelated modules
- under-designing the cartridge boundary so later MBCs become invasive
- silently zeroing cartridge RAM during direct boot and then treating that as hardware-accurate startup behavior
- teaching the generic bus how a specific MBC banks ROM or RAM instead of delegating that behavior to the cartridge subsystem

## Open questions

- enum-based versus trait-based mapper organization for this codebase
