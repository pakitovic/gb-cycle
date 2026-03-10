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

## Registers / MMIO

- mapper-controlled ROM/RAM banking ranges
- external RAM enable and control ranges

## Timing / accuracy requirements

- Access behavior must remain compatible with bus ordering.
- Architecture should scale from ROM-only to MBC1, MBC3, MBC5, and later extensions.
- Direct-boot initialization should not assume external RAM starts clean unless that follows from persisted save data or an explicit uninitialized-memory policy.

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

## Implementation notes for this repo

- Keep mapper traits or enums narrow and explicit.
- Avoid hard-coding cartridge logic into generic bus code.
- Treat external RAM power-up contents as separate from deterministic post-boot CPU/MMIO state; if the emulator chooses a direct-boot initialization policy, keep it explicit and configurable.

## Known pitfalls

- leaking mapper knowledge into unrelated modules
- under-designing the cartridge boundary so later MBCs become invasive
- silently zeroing cartridge RAM during direct boot and then treating that as hardware-accurate startup behavior

## Open questions

- enum-based versus trait-based mapper organization for this codebase
