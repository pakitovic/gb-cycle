# BOOT ROM

## Scope

Own boot ROM assets, boot-ROM enable/disable state, power-up sequencing, direct-boot state definitions, and hardware revision differences visible at startup.

## Hardware model

Distinguish clearly between running through boot ROM code and starting from an already initialized state. DMG and CGB must not share assumed initial state without evidence.

Within the DMG family, do not collapse `DMG0`, later `DMG`, and `MGB` into one generic startup model if observable differences matter.
For DMG-family support, prefer one shared hardware core with different boot ROM images rather than separate emulator implementations per model.
When CGB support arrives, treat its boot ROM as a larger and structurally different firmware flow, not as a simple DMG boot ROM variant with a few extra writes.
The boot subsystem should own firmware selection and boot-ROM enable state, while the bus consumes that state when routing accesses.
Real boot should start CPU execution at `0x0000` with the internal boot ROM mapped over the low cartridge region, and hand off to cartridge code only after a real write to `FF50` changes the mapping. Skip-boot should be a separate explicit initialization path rather than a partially executed or silently shortened boot ROM flow.

## Responsibilities

- select between explicit real-boot and skip-boot startup modes
- boot ROM enable/disable behavior
- boot ROM mapping policy and state exposed to the bus
- boot ROM kind selection for the active console model
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

## Boot mode baseline

- The project should support two explicit startup modes: `RealBoot` and `SkipBoot`.
- `RealBoot` must execute the selected boot ROM on the real CPU core, through the real bus, and on the shared T-cycle scheduler.
- `SkipBoot` must initialize a model-specific post-boot state directly, start execution at `0x0100`, and leave boot ROM mapping disabled from the beginning.
- The rest of the system should not care whether execution reached cartridge code through real boot or skip boot; only the configured startup path should differ.

## DMG-family boot baseline

- DMG-family boot ROM selection should remain explicit through a `BootRomKind`-style concept covering at least `DMG0`, `DMG`, and `MGB`.
- Those DMG-family boot ROMs should run on the same DMG hardware core without scattered model-specific CPU branches.
- During real DMG-family boot, the boot ROM should read the cartridge logo/header bytes, perform its documented checks, drive the visible animation through ordinary CPU and bus activity, and withhold cartridge handoff when those checks fail.
- The visible boot logo should come from the cartridge header bytes at `0x0104-0x0133`, not from a frontend animation script or emulator-side asset.
- The "no cartridge / reads as `0xFF`" boot behavior should emerge from cartridge and bus behavior rather than a special visual hack.

## `FF50` handoff baseline

- `FF50` must behave as a real boot-ROM mapping-control register.
- Real boot completion should happen because boot-ROM code executes a real write to `FF50`, not because the emulator detects a conceptual "boot is done" state.
- The mapping change caused by `FF50` must affect the next fetch, not previous accesses retroactively.
- On DMG-family real boot, the first opcode fetched from the cartridge after handoff should be the byte at `0x0100`.
- Register state visible at cartridge entry must come from the executed boot ROM of the selected model; do not hard-code DMG and MGB as sharing one identical final `A` value.

## Timing / accuracy requirements

- Boot ROM transition behavior must remain explicit.
- Direct-boot helpers must document what state they assume and why.
- The boot ROM should execute as real CPU code from `0000-00FF` while mapped.
- Unmapping of the boot ROM and handoff to cartridge ROM must happen through the documented hardware-visible mechanism, not through an implicit emulator shortcut.
- Future CGB support must account for the fact that the boot ROM mapping is not the same shape as DMG-family boot ROM mapping.
- Real boot must use the same CPU core, bus, and shared T-cycle scheduler as the rest of emulation rather than a special startup interpreter or frontend animation path.
- Boot ROM reads from the cartridge header and boot-ROM writes to VRAM/LCD/MMIO should use the same bus and arbitration rules as ordinary execution.
- The duration of the boot process should emerge from executed instructions and subsystem timing, not from an external startup timer.
- Skip-boot must remain a distinct initialization path; do not partially execute the boot ROM and cut it short.

## Dependencies

- CPU
- T-cycle scheduler or clock source
- bus and memory
- cartridge/MBC
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

- real-boot versus skip-boot entry-path tests
- boot ROM presence/disable tests
- model-specific startup state tests
- tests for `FF50` handoff from boot ROM to cartridge ROM
- tests that `0x0000-0x00FF` read from boot ROM before `FF50` and from cartridge ROM after `FF50`
- tests that the next fetch after `FF50` already comes from the cartridge and that DMG-family real boot enters cartridge code at `0x0100`
- tests for valid header/logo/checksum handoff versus invalid-logo or invalid-checksum no-handoff behavior
- tests for missing-cartridge or `0xFF`-filled header behavior
- tests for model-specific visible `A` at cartridge entry, especially DMG versus MGB
- direct-boot preset tests that document assumed register state

## Implementation notes for this repo

- Keep "after boot" presets separate from real boot ROM execution paths.
- Leave extension points for hardware revisions and variants.
- The first core may target the DMG family, but the boot path must still depend on an explicit console model enum or equivalent typed descriptor.
- Boot ROM loading should be configurable so the emulator can use real dumps, custom firmware, or no boot ROM at all.
- A dedicated `BootRom` component with bytes, selected kind, and mapped/unmapped state is the intended baseline.
- Keep boot-ROM asset ownership and boot enable/disable state in the boot subsystem even if the bus performs the actual address routing.
- Keep real-boot and skip-boot as explicit modes such as `RealBoot` and `SkipBoot`; the rest of the emulator should see only the resulting machine state and bus mapping.
- A `SkipBoot` or equivalent explicit direct-boot mode is useful for tests, tooling, and differential validation, but it must remain distinct from verified boot ROM execution.
- DMG-family observable differences should initially be assumed to come from firmware and startup state unless a proven hardware-level difference matters to the emulator.
- `FF50` should integrate with system or bus mapping control, not as a CPU-local shortcut.
- Real-boot header validation should emerge from executed boot-ROM code reading cartridge bytes, not from a parallel emulator-side validator.
- Do not hard-code boot ROM support around a fixed 256-byte assumption; CGB boot ROM is larger and uses a split mapped layout.
- When CGB is implemented, boot should be able to inspect cartridge header compatibility information and choose CGB mode or DMG-compatibility mode accordingly.

## Known pitfalls

- assuming DMG and CGB initial state are interchangeable
- mixing convenience startup state with verified boot behavior
- hard-coding one boot ROM path into the core
- forcing real boot to jump to `0x0100` without a real `FF50` write and next-fetch handoff
- validating logo or checksum outside the executed boot ROM path
- faking the Nintendo logo or boot animation in a frontend layer instead of letting VRAM/LCD writes emerge from execution
- silently jumping to post-boot state without making the mode explicit

## Open questions

- which models and revisions to support in the first direct-boot API
- which DMG-family differences are treated as required from day one versus deferred behind documented limitations
- how the boot mapping abstraction should represent non-contiguous firmware windows cleanly once CGB support begins
