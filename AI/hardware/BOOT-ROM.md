# BOOT ROM

## Scope

Own boot ROM mapping, power-up sequencing, direct-boot state definitions, and hardware revision differences visible at startup.

## Hardware model

Distinguish clearly between running through boot ROM code and starting from an already initialized state. DMG and CGB must not share assumed initial state without evidence.

Within the DMG family, do not collapse `DMG0`, later `DMG`, and `MGB` into one generic startup model if observable differences matter.
For DMG-family support, prefer one shared hardware core with different boot ROM images rather than separate emulator implementations per model.
When CGB support arrives, treat its boot ROM as a larger and structurally different firmware flow, not as a simple DMG boot ROM variant with a few extra writes.
The boot subsystem should own firmware selection and boot-ROM enable state, while the bus consumes that state when routing accesses.

## Responsibilities

- boot ROM enable/disable behavior
- model-specific initial register and memory state
- revision-aware startup configuration
- DMG-family differentiation where boot ROM or startup-visible behavior differs
- configurable boot ROM source selection
- direct-boot configuration for tests and tooling
- future CGB boot ROM mapping and compatibility-mode entry rules

## Registers / MMIO

- boot ROM mapping control
- startup-visible register state
- `FF50` boot ROM disable behavior
- future CGB boot ROM overlays across `0000-00FF` and `0200-08FF`

## Timing / accuracy requirements

- Boot ROM transition behavior must remain explicit.
- Direct-boot helpers must document what state they assume and why.
- The boot ROM should execute as real CPU code from `0000-00FF` while mapped.
- Unmapping of the boot ROM and handoff to cartridge ROM must happen through the documented hardware-visible mechanism, not through an implicit emulator shortcut.
- Future CGB support must account for the fact that the boot ROM mapping is not the same shape as DMG-family boot ROM mapping.

## Dependencies

- CPU
- bus and memory
- model/revision configuration

## Primary references

- Pan Docs boot process sections
- Gekkio hardware documentation and revision material
- Mooneye documentation and tests

## Open-source emulator references

Priority order:

1. SameBoy
2. Mooneye GB
3. binjgb
4. GameRoy
5. Gambatte

## Tests

- boot ROM presence/disable tests
- model-specific startup state tests
- tests for `FF50` handoff from boot ROM to cartridge ROM
- direct-boot preset tests that document assumed register state

## Implementation notes for this repo

- Keep "after boot" presets separate from real boot ROM execution paths.
- Leave extension points for hardware revisions and variants.
- The first core may target the DMG family, but the boot path must still depend on an explicit console model enum or equivalent typed descriptor.
- Boot ROM loading should be configurable so the emulator can use real dumps, custom firmware, or no boot ROM at all.
- Keep boot-ROM asset ownership and boot enable/disable state in the boot subsystem even if the bus performs the actual address routing.
- A `skip_bootrom` or equivalent direct-boot mode is useful for tests, tooling, and differential validation, but it must remain distinct from verified boot ROM execution.
- DMG-family observable differences should initially be assumed to come from firmware and startup state unless a proven hardware-level difference matters to the emulator.
- Do not hard-code boot ROM support around a fixed 256-byte assumption; CGB boot ROM is larger and uses a split mapped layout.
- When CGB is implemented, boot should be able to inspect cartridge header compatibility information and choose CGB mode or DMG-compatibility mode accordingly.

## Known pitfalls

- assuming DMG and CGB initial state are interchangeable
- mixing convenience startup state with verified boot behavior
- hard-coding one boot ROM path into the core
- silently jumping to post-boot state without making the mode explicit

## Open questions

- which models and revisions to support in the first direct-boot API
- which DMG-family differences are treated as required from day one versus deferred behind documented limitations
- how the boot mapping abstraction should represent non-contiguous firmware windows cleanly once CGB support begins
