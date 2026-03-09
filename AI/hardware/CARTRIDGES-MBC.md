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

## Implementation notes for this repo

- Keep mapper traits or enums narrow and explicit.
- Avoid hard-coding cartridge logic into generic bus code.

## Known pitfalls

- leaking mapper knowledge into unrelated modules
- under-designing the cartridge boundary so later MBCs become invasive

## Open questions

- enum-based versus trait-based mapper organization for this codebase
