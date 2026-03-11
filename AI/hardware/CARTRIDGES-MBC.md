# CARTRIDGES / MBC

## Scope

Own cartridge ROM/RAM mapping, cartridge-header parsing, mapper state, battery-backed persistence interfaces, and mapper-specific features such as RTC or rumble.

## Hardware model

Keep MBC behavior decoupled from the rest of the core. Cartridge hardware should expose a clear mapper contract to the bus rather than leaking mapper rules everywhere.

The cartridge should not be modeled as "ROM bytes plus a few MBC conditionals." From the console's point of view it is an external bus device that owns `0x0000-0x7FFF` and `0xA000-0xBFFF`, exposes ROM bank `0`, switchable ROM, optional external RAM, and any extra cartridge-local hardware declared by the header.

## Responsibilities

- parse the cartridge header at `0x0100-0x014F`
- derive cartridge model, capacity, and capability metadata from that header
- ROM and external RAM banking
- mapper register writes
- future RTC and rumble support
- cartridge metadata and model capability handling
- ownership of cartridge-visible address ranges `0x0000-0x7FFF` and `0xA000-0xBFFF` once the bus has decoded the access into cartridge space
- validate declared ROM/RAM configuration against the loaded image with an explicit project policy

## Registers / MMIO

- cartridge header bytes in ROM bank `0` at `0x0100-0x014F`
- mapper-controlled ROM/RAM banking ranges
- external RAM enable and control ranges

## Header-driven cartridge baseline

- The cartridge header in `0x0100-0x014F` is the architectural source of truth for the cartridge's base hardware description.
- The bus must not infer mapper behavior from ROM size, RAM size, filename, or frontend heuristics when the header already declares the cartridge type.
- A central cartridge-header parser should own decoding of at least:
  - `entry_point`
  - `cgb_flag` from `0x0143`
  - `sgb_flag` from `0x0146`
  - `cartridge_type` from `0x0147`
  - `rom_size_code` from `0x0148`
  - `ram_size_code` from `0x0149`
- The parser should preserve enough raw metadata for diagnostics and future compatibility work, including the Nintendo logo bytes and the raw header codes.
- The decoded result should live in a strongly typed structure such as `CartridgeHeader`, not in scattered ad hoc fields.
- Header-derived capability data should remain available even before the project implements all of the corresponding hardware, because future CGB, SGB, RTC, battery, rumble, and peripheral support depends on it.

## Cartridge-type baseline

- Byte `0x0147` is the source of truth for selecting the cartridge implementation.
- The first stable taxonomy for this repo should distinguish at least:
  - `RomOnly`
  - `Mbc1`
  - `Mbc2`
  - `Mbc3`
  - `Mbc5`
  - `Unsupported` or `Other`
- The cartridge type must drive more than bank switching. It also defines whether the cartridge has external RAM, battery-backed save state, RTC, rumble, or other mapper-local hardware.
- Less common types such as `MMM01`, `MBC6`, `MBC7`, `HuC1`, `HuC3`, camera, or sensor cartridges may begin life as `Unsupported`, but they should remain explicitly identified rather than silently coerced into a nearby supported mapper.

## ROM-size and RAM-size baseline

- Byte `0x0148` should decode to both `rom_size_bytes` and `rom_bank_count`.
- Standard ROM size codes `0x00..0x08` should follow the documented progression from `32 KiB` upward, using the normal `32 KiB * (1 << value)` interpretation.
- Special ROM size codes `0x52`, `0x53`, and `0x54` should remain explicit cases. They may be supported cautiously or rejected as unsupported, but they must not be ignored silently.
- Byte `0x0149` should decode to both `ram_size_bytes` and `ram_bank_count` for cartridges that use the ordinary external-RAM table.
- Presence of external RAM must be derived from both `0x0147` and `0x0149`. The RAM-size code alone is not enough, because the cartridge type decides whether that hardware exists at all.
- `MBC2` is a required special case: its internal `512 x 4-bit` RAM must not be modeled through the general `0x0149` external-RAM table.
- Declared ROM size should be validated against the actual loaded image size through an explicit project policy instead of an ad hoc best effort.

## Base device contract

- The bus should know one stable cartridge interface such as `CartridgeDevice`; it should not know per-MBC details.
- That interface should expose at least:
  - `read_rom(addr)`
  - `write_rom(addr, value)` for mapper-control commands issued through ROM-space writes
  - `read_ram(addr)`
  - `write_ram(addr, value)`
  - header or metadata accessors
- The bus must never "write to ROM contents." A write in `0x0000-0x7FFF` is a command routed to the cartridge device on the shared bus timeline.
- The same contract should cover cartridges with no RAM, ordinary external RAM, banked RAM, RTC-mapped registers, and later extra cartridge-local hardware without requiring a new bus API.
- Real boot and post-boot execution should read the entry point, Nintendo logo, and header bytes through the same cartridge device rather than through a boot-only bypass.

## Factory and validation baseline

- Cartridge construction should be centralized in a loader or factory such as `load_cartridge(rom_bytes) -> CartridgeDevice`.
- That factory should:
  - parse the header
  - validate declared metadata
  - choose the cartridge implementation from `0x0147`
  - return a device ready for bus ownership of `0x0000-0x7FFF` and `0xA000-0xBFFF`
- Header validation should report at least:
  - known versus unknown cartridge type
  - expected ROM size versus actual file size
  - declared RAM configuration
  - suspicious or unsupported special size codes
- Validation should follow an explicit project policy such as `Strict`, `PermissiveWithWarning`, or `TestMode`.
- Unsupported or inconsistent cartridges should produce explicit diagnostics rather than a silent fallback mapper choice.

## Timing / accuracy requirements

- Access behavior must remain compatible with bus ordering.
- Architecture should scale from ROM-only to MBC1, MBC3, MBC5, and later extensions.
- Direct-boot initialization should not assume external RAM starts clean unless that follows from persisted save data or an explicit uninitialized-memory policy.
- Writes in ROM address space should be interpreted as cartridge/MBC control behavior where applicable, not as attempts to mutate ROM contents.
- Cartridge-visible reads and writes should remain ordinary bus transactions on the shared T-cycle timeline; mapper side effects must occur in access order rather than in a deferred per-instruction batch.
- Header parsing is configuration work at load time, but runtime visibility of header bytes at `0x0100-0x014F` must still emerge from normal ROM bank `0` reads after boot-ROM handoff.

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

- header parser tests for `entry_point`, `0x0143`, `0x0146`, `0x0147`, `0x0148`, and `0x0149`
- tests for standard `0x0148` ROM-size decoding and explicit handling of `0x52`, `0x53`, and `0x54`
- tests for `0x0149` RAM-size decoding, including the `MBC2` special case where internal RAM is not described by the ordinary RAM-size table
- tests for explicit diagnostics on unknown cartridge types and size mismatches
- mapper-specific ROM tests
- save RAM behavior tests
- RTC behavior tests when implemented
- tests that document startup behavior for external RAM when direct-boot presets bypass firmware execution
- tests that fixed-ROM, switchable-ROM, and external cartridge ranges are delegated through the cartridge interface rather than treated as internal console memory
- tests that ROM-space writes hit MBC control semantics instead of fake writable ROM
- tests that boot and post-boot paths both observe `0x0100-0x014F` through the ordinary cartridge device rather than through a shadow header copy

## Implementation notes for this repo

- Prefer one typed `CartridgeHeader` plus decoded capability fields over raw-code lookups scattered throughout the codebase.
- Keep `0x0147`, `0x0148`, and `0x0149` decoding centralized in the cartridge loader instead of reinterpreting them inside the bus, boot code, or frontends.
- Preserve `cgb_flag` and `sgb_flag` now even if the current core is still DMG-only.
- A `CartridgeKind` plus device factory is a good fit for early bring-up, as long as unsupported raw type codes remain reportable.
- Keep mapper traits or enums narrow and explicit.
- Avoid hard-coding cartridge logic into generic bus code.
- Treat external RAM power-up contents as separate from deterministic post-boot CPU/MMIO state; if the emulator chooses a direct-boot initialization policy, keep it explicit and configurable.
- Keep active-ROM-bank selection, RAM enable, RAM banking, RTC mapping, and any bank-wrap quirks inside cartridge/MBC implementations rather than generic bus region logic.
- Keep header validation policy explicit and centralized rather than hiding it inside individual mapper constructors.

## Known pitfalls

- treating the cartridge as a raw ROM blob with ad hoc mapper `if` statements
- leaking mapper knowledge into unrelated modules
- under-designing the cartridge boundary so later MBCs become invasive
- silently zeroing cartridge RAM during direct boot and then treating that as hardware-accurate startup behavior
- teaching the generic bus how a specific MBC banks ROM or RAM instead of delegating that behavior to the cartridge subsystem
- inferring the mapper from ROM size or other heuristics instead of using `0x0147`
- using `0x0149` alone to decide whether external RAM exists
- modeling `MBC2` RAM as if it were ordinary banked SRAM
- dropping `cgb_flag` or `sgb_flag` because they are not immediately used by the DMG baseline
- silently coercing unsupported or inconsistent headers into a nearby supported configuration

## Open questions

- enum-based versus trait-based mapper organization for this codebase
- which validation mode should be the default for interactive use versus automated test runs
