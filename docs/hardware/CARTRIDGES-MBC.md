# CARTRIDGES / MBC

## Scope

Own cartridge ROM/RAM mapping, cartridge-header parsing, mapper state, cartridge-local persistence interfaces, and mapper-specific features such as RTC, rumble, sensors, or serial EEPROM.

## Hardware model

Keep MBC behavior decoupled from the rest of the core. Cartridge hardware should expose a clear mapper contract to the bus rather than leaking mapper rules everywhere.

The cartridge should not be modeled as "ROM bytes plus a few MBC conditionals." From the console's point of view it is an external bus device that owns `0x0000-0x7FFF` and `0xA000-0xBFFF`, exposes ROM bank `0`, switchable ROM, optional cartridge-visible RAM whether external or mapper-local, and any extra cartridge-local hardware declared by the header.

## Responsibilities

- parse the cartridge header at `0x0100-0x014F`
- derive cartridge model, capacity, and capability metadata from that header
- ROM banking and cartridge-visible RAM mapping
- mapper register writes
- RTC support and rumble support
- cartridge metadata and model capability handling
- ownership of cartridge-visible address ranges `0x0000-0x7FFF` and `0xA000-0xBFFF` once the bus has decoded the access into cartridge space
- validate declared ROM/RAM configuration against the loaded image with an explicit project policy

## Registers / MMIO

- cartridge header bytes in ROM bank `0` at `0x0100-0x014F`
- mapper-controlled ROM/RAM banking ranges
- cartridge RAM enable and control ranges

## Header-driven cartridge baseline

- The cartridge header in `0x0100-0x014F` is the architectural source of truth for the cartridge's base hardware description.
- The bus must not infer mapper behavior from ROM size, RAM size, filename, or frontend heuristics when the header already declares the cartridge type.
- A central cartridge-header parser should own decoding of at least:
  - `entry_point`
  - raw visible `title` bytes from `0x0134-0x0143`, with the documented split that legacy cartridges may use all `16` bytes while cartridges whose `0x0143` byte has `bit 7` set reduce the conservative decoded visible title to `15` bytes
  - raw `0x013F-0x0142` bytes preserved separately, for example as `raw_title_suffix_or_manufacturer_code`, because newer headers leave those bytes ambiguous between manufacturer code and title suffix
  - `cgb_flag` from `0x0143`, keeping canonical `0x80` / `0xC0` values distinct but also preserving any non-canonical `bit 7`-set values as explicit CGB-capable metadata rather than collapsing them into generic unknowns
  - `new_licensee_code` from `0x0144-0x0145`
  - `sgb_flag` from `0x0146`
  - `cartridge_type` from `0x0147`
  - `rom_size_code` from `0x0148`
  - `ram_size_code` from `0x0149`
  - `destination_code` from `0x014A`
  - `old_licensee_code` from `0x014B`
- The parser should preserve enough raw metadata for diagnostics and future compatibility work, including the Nintendo logo bytes, the raw visible title bytes, manufacturer and licensee bytes, and the raw header codes.
- Do not guess the `11`-character newer-title layout from ad hoc ASCII heuristics alone. The raw header does not expose a reliable discriminator between "manufacturer code is active" and "these four bytes are still part of a 15-character CGB-era title", so the parser should keep the decoded title conservative and leave the split available as explicit preserved metadata.
- The decoded result should live in a strongly typed structure such as `CartridgeHeader`, not in scattered ad hoc fields.
- Header-derived capability data should remain available even before the project implements all of the corresponding hardware, because future CGB, SGB, RTC, battery, rumble, and peripheral support depends on it.

## Cartridge-type baseline

- Byte `0x0147` is the source of truth for selecting the cartridge implementation.
- Cartridge classification should distinguish three layers rather than one flat "mapper kind":
  - supported runtime families such as `NoMbc`, `Mbc1`, `Mbc2`, `Mbc3`, and `Mbc5`
  - typed near-family variants such as `Mbc3Variant::Mbc30` and the supported `MBC1M` signature / variant path
  - structured unsupported or special classifications for everything else declared by the header
- The structured unsupported / special classification should distinguish at least:
  - `PlannedVariant`
  - `DocumentedButUnsupported`
  - `ExperimentalHeuristic`
  - `AccessorySpecialCase`
  - `UnknownCode`
- `NoMbc` should be the non-banked family covering header codes `0x00`, `0x08`, and `0x09`, while preserving the raw header type for RAM/battery distinctions and diagnostics.
- The cartridge type must drive more than bank switching. It also defines whether the cartridge has external RAM, mapper-local RAM such as MBC2 internal RAM, battery-backed persistence, RTC, rumble, or other mapper-local hardware.
- The classification result must preserve at least the raw `0x0147` type byte, the detected name, the category, and a concise reason suitable for diagnostics or frontend display.
- Less common types such as `MMM01`, `MBC6`, `MBC7`, `HuC1`, `HuC-3`, or sensor cartridges must remain explicitly identified rather than silently coerced into a nearby supported mapper. Several of those families now have dedicated supported paths, including `MBC7`; the rest should keep precise structured diagnostics until implemented.

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
- The same contract should cover cartridges with no RAM, ordinary external RAM, mapper-local RAM such as MBC2 internal RAM, banked RAM, RTC-mapped registers, and later extra cartridge-local hardware without requiring a new bus API.
- Real boot and post-boot execution should read the entry point, Nintendo logo, and header bytes through the same cartridge device rather than through a boot-only bypass.

## Factory and validation baseline

- Cartridge construction should be centralized in a loader or factory such as `load_cartridge(rom_bytes) -> Result<SupportedCartridge, UnsupportedCartridgeKind>`.
- That factory should:
  - parse the header
  - validate declared metadata
  - choose the cartridge implementation from `0x0147`
  - return either a supported device ready for bus ownership of `0x0000-0x7FFF` and `0xA000-0xBFFF`, or a typed unsupported classification
- Header validation should report at least:
  - supported kind versus structured special / unsupported kind
  - expected ROM size versus actual file size
  - declared RAM configuration
  - raw header type byte and detected cartridge name where known
  - suspicious or unsupported special size codes
- Validation should follow one explicit typed compatibility policy rooted in `ExecutionMode::{Strict, Permissive, Experimental}` plus sub-policies for validation, heuristics, overrides, and diagnostics.
- Unsupported or inconsistent cartridges should produce explicit diagnostics rather than a silent fallback mapper choice.
- Experimental heuristic detection for partially documented multicarts should remain an explicit opt-in loader policy and must be disabled by default in `Strict` and `Permissive`.

## Compatibility policy and execution modes

- `docs/ARCHITECTURE.md` is the authoritative home for the typed `CompatibilityPolicy` shape, execution-mode invariants, and cross-system metadata requirements.
- This handbook owns the cartridge-specific classification categories, decision matrix, loader diagnostics, and mapper-specific compatibility implications that plug into that shared policy shape.
- `docs/TESTING.md` owns CI/oracle usage of execution modes and the save/load determinism expectations attached to `Strict`.
- Compatibility policy is a stable loader/config layer that consumes typed cartridge classification; it is not a license to change the hardware truth of already supported cartridges.
- This project is T-cycle based. Once a supported cartridge device exists, cartridge-visible reads, writes, mapper commands, and mapper-side effects still happen on the access T-cycle regardless of execution mode.
- The loader should classify the cartridge once from the header and validated metadata, then hand that typed classification to one policy decision point.
- Mode-dependent code should not reparse `0x0147`, `0x0148`, or `0x0149` independently in the frontend, loader, or cartridge constructors.

### Supported-hardware invariant

- For already supported runtime families such as `NoMbc`, `Mbc1`, `Mbc2`, `Mbc3`, `Mbc5`, `Mbc6`, and `Mbc7`, switching between `Strict`, `Permissive`, and `Experimental` must not change T-cycle-visible cartridge semantics.
- That invariant covers at least ROM and RAM banking behavior, RTC register mapping, mapper-local enable state, persistence capability interpretation, and any later cartridge-local accessory or IRQ behavior once implemented.
- Mode differences belong in admission, validation severity, heuristic enablement, explicit overrides, diagnostics, and access to experimental or partial implementations.
- Any temporary case where a mode changes runtime behavior for already supported hardware should be documented as technical debt, not normalized as expected behavior.

### Mode definitions

- `Strict` is the reference mode for CI, differential comparison, DMG closure, and official accuracy claims.
- In `Strict`, only `Supported` cartridges should load by default; relevant header inconsistencies are fatal; heuristics are disabled; and manual mapper/model overrides should be disabled by default or require an explicit debug path with visible trace output.
- `Permissive` is the tolerant normal-use mode for cartridges that still map unambiguously to already supported hardware.
- In `Permissive`, all `Supported` cartridges should load with identical runtime semantics to `Strict`; only unambiguous header inconsistencies may degrade from error to warning; automatic heuristic mapper detection stays off; and manual overrides may be allowed, but never silently.
- Current repo decision: for already supported `NoMbc` cartridges, legacy or inconsistent RAM-size declarations that still map unambiguously to the fixed `8 KiB` always-enabled `ROM+RAM` baseline should degrade to warnings under `Permissive` / `Warn` instead of remaining fatal, while `Strict` still rejects them.
- `Experimental` is the research and bring-up mode.
- In `Experimental`, `Supported` cartridges still keep the same runtime semantics as the other modes, but explicit heuristic paths, partial planned-variant implementations, and clearly marked stub or placeholder paths for special hardware may be enabled behind explicit policy gates.
- `Experimental` results must be marked as non-oracle in diagnostics, tooling, and project claims.

### Category decision matrix

| Category | `Strict` | `Permissive` | `Experimental` |
| --- | --- | --- | --- |
| `Supported` | load | load | load |
| `PlannedVariant` | reject | reject by default | load only when an explicit partial or complete implementation path exists |
| `DocumentedButUnsupported` | reject | reject | optionally load only through explicit stub or partial feature paths with prominent diagnostics |
| `ExperimentalHeuristic` | reject | reject by default | allow only when heuristics are explicitly enabled |
| `AccessorySpecialCase` | reject | reject | allow only when a stub or partial accessory path exists explicitly |
| `UnknownCode` | reject | reject | reject unless an ultra-explicit development override bypasses classification |

- User-facing diagnostics should name both the detected category and the current execution mode when explaining why a cartridge was admitted, warned, or rejected.
- `Permissive` must not invent new mapper families; it only relaxes validation when the supported hardware mapping is still unambiguous.

### Manual overrides and diagnostics

- Manual overrides are a separate layer from execution modes.
- Overrides should be able to force at least console model, mapper, execution mode, and validation policy.
- In `Strict`, overrides should require an explicit debug pathway and remain visible in logs, debugger UI, save states, and replay metadata.
- In `Experimental`, overrides may be used freely, but the run must be marked clearly as non-oracle.
- Loader failures should report at least raw `0x0147`, detected cartridge name, classification category, current execution mode, and the precise rejection reason.
- Warnings should distinguish at least:
  - inconsistent header but unambiguous supported mapper
  - heuristic classification active
  - partially supported or stub-backed hardware path
  - manual override active
- Avoid context-free messages such as "unsupported cartridge" when a more precise detected type exists.

## Special-cartridge taxonomy and unsupported policy

- Unsupported handling is part of the cartridge design, not an accidental loader fallback.
- Classification happens at load time, but runtime mapper behavior remains T-cycle based: once a supported cartridge device exists, cartridge-visible reads, writes, and mapper side effects still occur on the access T-cycle.
- The loader must not collapse every uncommon cartridge into one opaque `Unsupported` bucket and must not guess a nearby mapper merely because the ROM happens to boot.

### MBC30

- Treat `MBC30` as a real `MBC3`-family variant, not as an informal alias for ordinary `MBC3`.
- `MBC30` now classifies as a supported `MBC3`-family variant through an explicit typed entry such as `Mbc3Variant::Mbc30`.
- Pan Docs identifies "MBC3 with `64 KiB` of SRAM" as `MBC30`; gbdoc documents the practical extension as MBC3 behavior with up to `4 MiB` ROM and `64 KiB` SRAM.
- Detection should trigger only when the validated cartridge is in one of the RAM-bearing `MBC3` header shapes (`0x10`, `0x12`, or `0x13`) and header metadata implies the `64 KiB` SRAM configuration associated with `MBC30`.
- The factory must not silently construct ordinary standard `MBC3` with oversized SRAM when this case is detected.
- `Strict`, `Permissive`, and `Experimental` should all admit supported `MBC30` with identical runtime semantics; mode differences still belong only in validation severity, diagnostics, and explicit experimental paths for other hardware.
- `MBC30` reuses the MBC3 RTC and latch model, including the current relatch compatibility rule, advisory `rtc_access_ready_at`, and live-versus-latched persistence split.
- `MBC30` ROM banking extends the MBC3 ROM-bank register to `8` bits, keeps the `0 -> 1` switchable-window translation, and masks the final bank by the validated ROM size up to `4 MiB`.
- `MBC30` RAM banking maps selector values `0x00..=0x07` to the `64 KiB` SRAM backing, maps `0x08..=0x0C` to RTC registers when the header type provides RTC, and keeps `0x0D..=0x0F` as reserved selector states.
- `MBC30` persistence uses the same MBC3 RAM / RTC persistence payload family, but with a validated `64 KiB` RAM length when SRAM exists.

### Special-variant status and ordering

- Keep a design distinction between close derivatives of supported mapper families and cartridges that require new cartridge-local hardware.
- The DMG-family special-cartridge runtime path is closed for the currently targeted set: `MMM01`, the supported `MBC1M` signature path, `HuC1`, `HuC-3`, `M161`, and `Pocket Camera` all enter through dedicated supported cartridge paths rather than fallback mappers.
- `MBC30` is now the supported close `MBC3`-family variant, `MBC6` now enters through a dedicated supported cartridge path with its own split-window and flash model, and `MBC7` is now a supported dedicated sensor / EEPROM cartridge path. Do not fold any of these families into ordinary `MBC3` / `MBC5` logic.
- `Bandai TAMA5` remains an accessory-special unsupported path until a dedicated cartridge-local device model exists.
- Lower-confidence `EMS`, `Bung`, and `Wisdom Tree` paths remain experimental heuristics and must not participate in strict default loading.

### Multicarts: MMM01, MBC1M, and M161

- Treat `MMM01` as its own mapper family, not as a small `MBC1` patch.
- `MMM01` should now enter through its own supported cartridge path rather than through `DocumentedButUnsupported`.
- Header codes `0x0B`, `0x0C`, and `0x0D` should classify as `MMM01`.
- `MMM01` should live in a dedicated multicart path because Pan Docs documents additional game-selection state and an initial unmapped boot mode where the last `32 KiB` of the ROM is visible first.
- The loader must not assume that the only meaningful boot header for every cartridge necessarily lives at the physical start of the ROM image once `MMM01` support is under consideration.
- The loader should prefer the boot-visible MMM01 menu header at `(rom_size - 32 KiB) + 0x0100` when that trailing header resolves to one of the MMM01 type codes, declares the full loaded ROM size, and still looks like a coherent multicart menu rather than random tail bytes. In the current baseline that means the menu title is non-empty and the ROM still exposes at least two coherent embedded non-MMM01 game subheaders sharing the same Nintendo logo before the trailing menu block.
- The current baseline also admits one narrow second trailing-menu path for the later Mani `4-in-1` multicarts that Pan Docs attributes to `MMM01`: the boot-visible physical header may still look like plain `MBC1`, but a trailing `... SET` menu header with the observed `0x11` outer type and at least four coherent embedded no-RAM game subheaders (`NoMbc`/`MBC1` shapes in the local dumps) is treated explicitly as `MMM01`.
- That later-Mani path is intentionally narrow and signature-backed. It is not a generic "`SET` means MMM01" heuristic, and it does not reopen `M161`; the original `Tetris + Alleyway + Yakuman + Tennis` Mani cart remains on the dedicated `M161` path.
- The current repo baseline now includes a dedicated `MMM01` cartridge device with explicit `unmapped` versus `mapped` mode, menu-startup mapping to the last `32 KiB` of the ROM, dedicated game-select masks, and mapper-local ROM / RAM banking instead of routing MMM01 through ordinary `MBC1`.
- In the current baseline, the runtime path already covers the released non-CGB commercial shape: dedicated multicart selection, explicit ROM-bank mask locking, RAM-bank mask locking, mapped-mode write restrictions for the extended bits, and battery-backed RAM persistence when the header declares it.
- The current implementation has commercial validation through `Momotarou Collection 2 (Japan) (SGB Enhanced)`, plus the four later Mani local multicarts that enter through the explicit trailing-`... SET` signature path; the menu boots correctly and the included games run correctly after the menu-to-game transition on both the standard header-coded and later-Mani shapes.
- `MBC1M` should also be a distinct typed variant rather than being mixed into standard `MBC1` banking formulas with ad hoc conditionals.
- `MBC1M` now enters through an explicit supported `MBC1`-family signature path on `1 MiB` multicarts with repeated valid subheaders in banks such as `0x10`, `0x20`, and `0x30`, instead of being left as a future-only planned variant.
- `MBC1M` is still a near-family variant of `MBC1`, but it is not interchangeable with standard `MBC1`; the factory should reserve explicit variant space for it even though the current baseline now instantiates that variant.
- Treat `M161` as a separate multicart special case rather than as `NoMbc` or `MBC1`.
- `M161` now enters through an explicit supported signature path for the known Mani `4-in-1` multicart instead of falling back to ordinary `NoMbc`.
- The current signature path accepts both the synthetic baseline menu-header shape (`MANI 4 IN 1` with no-MBC header fields) and the known commercial outer-header shape (`TETRIS SET`, type `0x10`, ROM size `0x03`, RAM size `0x00`) because the real mapper hardware still behaves as `M161` even when the boot-visible menu header does not describe a standard no-MBC cartridge.
- The current baseline now includes a dedicated `M161` cartridge device with:
  - whole-`32 KiB` ROM switching across up to `8` banks
  - a literal one-time bank latch where the first ROM-space write selects bank bits `0..=2` and all later bankswitch writes are ignored until power-off
  - no external RAM aperture beyond the ordinary absent/open contract and no battery-backed persistence payload.
- The current implementation now has end-to-end commercial validation through `Mani 4 in 1 - Tetris + Alleyway + Yakuman + Tennis (China) (Ja)`: the menu boots correctly and all four embedded games run correctly after selection.
- `Strict` and `Permissive` must not enable broad multicart heuristics for `EMS` or `Bung`, but they may now use the explicit `MBC1M` subheader signature path because real commercial multicarts rely on it.
- `Experimental` may still enable the broader heuristic paths, but diagnostics must state that those classifications did not come from a standard header-backed or signature-backed supported path.

### HuC1 and HuC-3

- Treat `HuC1` as its own mapper family, not as "`MBC1` with IR."
- `HuC1` should now classify as its own supported mapper family, not as ordinary `MBC1`.
- Header code `0xFF` should classify as `HuC1 + RAM + BATTERY`.
- `HuC1` must keep cartridge-local IR-mode semantics explicit, because `0x0000-0x1FFF` selects RAM mode versus IR mode rather than acting as ordinary MBC1 RAM enable.
- The current baseline now includes a dedicated `HuC1` cartridge device with explicit `ram_mode` versus `ir_mode` state instead of inheriting standard `MBC1` behavior and overriding a few accesses.
- The current baseline routes `0x2000-0x3FFF` through a dedicated `6`-bit HuC1 ROM-bank register, `0x4000-0x5FFF` through a dedicated `2`-bit RAM-bank register, and ignores `0x6000-0x7FFF` writes instead of borrowing MBC1 banking-mode semantics.
- The current baseline keeps HuC1 ROM-bank writes literal rather than applying MBC1's `0 -> 1` translation, because Pan Docs documents a distinct HuC1 register map instead of MBC1-compatible bank-zero remapping.
- The current baseline supports ROM sizes up to `1 MiB` and RAM sizes up to `32 KiB`, matching the HuC1 limits documented by Pan Docs / Gekkio's `GB Complete Technical Reference`.
- `0xA000-0xBFFF` now routes either banked cartridge RAM or the IR register depending on the selected mode, and the persistent backing store lives under its own `HuC1` payload instead of being serialized as `MBC1`.
- When the validated HuC1 RAM payload is smaller than the visible `8 KiB` external aperture (for example the documented `2 KiB` case), the current baseline mirrors that smaller payload across `0xA000-0xBFFF` instead of falling off the end of the backing store.
- The current IR baseline exposes transmitter control and the documented `0xC0` / `0xC1` readback contract. For the current DMG roadmap, host-side light injection is intentionally out of scope unless a later concrete title investigation proves it necessary.
- Treat `HuC-3` as a documented but poorly understood special cartridge, not as a close `MBC3` derivative.
- `HuC-3` should now classify as its own supported mapper family, not as ordinary `MBC3`.
- Header code `0xFE` should classify as `HuC-3`.
- `HuC-3` must keep its cartridge-local RTC / IR / speaker protocol explicit and must not fall back automatically to ordinary `MBC3`.
- The current baseline now includes a dedicated `HuC-3` cartridge device with:
  - a literal `7`-bit ROM-bank register where bank `0` remains valid in `0x4000-0x7FFF`
  - a dedicated RAM-bank register for the banked `8 KiB` external-RAM aperture
  - explicit select modes for RAM read-only, RAM read/write, RTC command, RTC response, RTC semaphore, IR, and invalid-selector open bus
  - an explicit mailbox + `256`-nybble MCU-window model for the documented RTC protocol instead of `MBC3`-style RTC registers
  - dedicated persistence carrying battery-backed RAM plus HuC-3 RTC / MCU state, rather than reusing `MBC3` payload shapes.
- The current RTC baseline implements the documented command subset:
  - `0x1` read + increment
  - `0x3` write + increment
  - `0x4` / `0x5` access-address setup
  - `0x6` extended commands `0x0`, `0x1`, and `0x2`
- The current RTC baseline keeps command execution synchronous when the semaphore is cleared, masks all MCU-window cells to `4`-bit values, ignores address offsets within `0xA000-0xBFFF` for HuC-3 register accesses, and treats undocumented commands (including extended `0xE`) as explicit unsupported protocol states surfaced through trace output rather than heuristically emulating `MBC3`.
- For the current DMG roadmap, host-side IR injection and audible tone generation remain out of scope unless a later concrete title investigation proves they are needed.
- Commercial oracle validation for the current baseline is now confirmed locally with `Pocket Family GB (Japan) (SGB Enhanced)`, so `HuC-3` is no longer only a unit-tested/runtime-baseline path in this repo.

### MBC6 and MBC7

- Treat `MBC6` and `MBC7` as documented special hardware, not as unknown codes and never as fallback aliases for nearby MBC families.
- Header code `0x20` now classifies as supported `MBC6`; header code `0x22` now classifies as supported `MBC7+SENSOR+RUMBLE+RAM+BATTERY`.
- `MBC6` must not fall back to `MBC3` or `MBC5`; Pan Docs documents split `8 KiB` ROM/flash windows, split `4 KiB` RAM windows, and on-cartridge Macronix MX29F008TC-compatible flash behavior.
- The current `MBC6` baseline targets the documented `Net de Get: Minigame @ 100` shape: cartridge type `0x20`, CGB-capable header, `1 MiB` ROM declaration, `32 KiB` SRAM declaration, `1 MiB` cartridge-local main flash initialized to `0xFF`, and a `256`-byte hidden flash region initialized to `0xFF`.
- `MBC6` power-up state is explicit: RAM disabled, flash disabled, flash write-enable disabled, ROM/flash window A bank `2`, ROM/flash window B bank `3`, RAM window A bank `0`, RAM window B bank `1`, both high windows selecting ROM, and sector `0` not protected unless restored from persistence.
- `MBC6` maps `0x0000-0x3FFF` to the fixed first `16 KiB` of ROM, `0x4000-0x5FFF` to independently selected ROM/flash bank A, `0x6000-0x7FFF` to independently selected ROM/flash bank B, `0xA000-0xAFFF` to SRAM window A, and `0xB000-0xBFFF` to SRAM window B.
- `MBC6` ROM/flash bank registers are `8 KiB` banks, not ordinary `16 KiB` MBC banks; SRAM registers are `4 KiB` banks, not ordinary `8 KiB` external-RAM banks.
- `MBC6` flash commands are decoded inside the cartridge device through the documented unlock offsets, including JEDEC ID mode, sector erase, chip erase, hidden-region read, hidden-region erase/program, sector-0 protect/unprotect, and `128`-byte aligned program buffers whose writes can only clear bits until an erase restores `0xFF`.
- `MBC6` flash write-enable models the flash `/WP` behavior rather than a generic bus-write gate: it protects sector `0` and hidden/protection commands, while sectors `1..=7` remain erasable/programmable when flash is otherwise enabled.
- MBC6 flash operations currently complete synchronously and expose a done status byte immediately after the command-side effect, because no stronger project-local timing evidence exists yet; this is an explicit timing baseline rather than a claim about real erase/program latency.
- `MBC6` persistence owns the full `32 KiB` SRAM, `1 MiB` main flash, `256`-byte hidden region, and non-volatile sector-0 protection bit. Raw external `.sav` import/export is intentionally narrower and only represents `SRAM || main flash` when hidden flash is default `0xFF` and sector-0 protection is clear.
- `MBC6` has two complementary validation signals in this branch: the repo-gated synthetic `phase-6-mbc6-oracle` fixture covers split ROM/SRAM windows plus main and hidden flash command behavior without using commercial data, while manual local smoke validation of `Net de Get: Minigame @ 100` confirms boot, minigame execution, and cartridge saving on the real documented title shape.
- `MBC7` must not fall back to `MBC5 + rumble + RAM`; Pan Docs documents a `7`-bit ROM-bank register, two independent enable gates, an `A000-AFFF` sensor / EEPROM register aperture, and `B000-BFFF` reads as `$FF`.
- MBC7 register access requires both enables: writes of `$0A` to `0000-1FFF` and `$40` to `4000-5FFF`; the switchable ROM window at `4000-7FFF` uses the low `7` bits written to `2000-3FFF`, while `0000-3FFF` remains ROM bank `0`.
- MBC7 accelerometer support models the documented latch protocol: writing `$55` to `Ax0x` clears the latched X/Y values to `$8000`, writing `$AA` to `Ax1x` captures deterministic host-provided X/Y raw values only after that clear, `Ax2x/Ax3x` expose X low/high, `Ax4x/Ax5x` expose Y low/high, `Ax6x` reads `$00`, and other undocumented selectors read `$FF`.
- MBC7 EEPROM support models the documented serial `93LC56`-style command path through `Ax8x`: `CS`, `CLK`, and `DI` are driven by writes, `DO` is visible on reads, commands are decoded as start bit plus `10` command bits, and the persistent backing store is exactly `256` raw bytes.
- The official header name for `$22` contains `RUMBLE`, but the current runtime intentionally exposes `has_rumble() == false` and `rumble_on() == false` for MBC7 because neither Pan Docs MBC7 nor the currently referenced hardware-board evidence documents a concrete MBC7 rumble motor or control bit. Do not invent a rumble register for MBC7 until hardware evidence identifies the route.
- The `RAM` and `BATTERY` words in the `$22` header name are treated as historical persistence labels for the `256`-byte EEPROM, not as generic SRAM mapping and not as proof of a physical battery-backed SRAM device.

### Accessory cartridges

- Treat `Pocket Camera` as dedicated cartridge-local hardware rather than as an ordinary MBC or an accessory-special unsupported placeholder.
- `Pocket Camera` should classify as a supported dedicated family, while `Bandai TAMA5` should remain `AccessorySpecialCase` until implemented.
- Header code `0xFC` should classify as `Pocket Camera`, and header code `0xFD` should classify as `Bandai TAMA5`.
- The `Pocket Camera` loader path should accept only the official `1 MiB` ROM / `128 KiB` RAM shape in `Strict`, keep the hardware inside the cartridge subsystem, and expose a host-frame seam instead of a frontend-owned fake mapper. See `GAME-BOY-CAMERA.md` for the detailed register, timing, and frontend-boundary notes.
- `Bandai TAMA5` diagnostics should continue to state explicitly that the type requires dedicated cartridge-local accessory hardware rather than only ROM / RAM banking.
- The loader must not try to execute `Pocket Camera` or `Bandai TAMA5` under an approximate supported mapper "just to see if they boot."

### Experimental and heuristic cases

- Some Pan Docs "Other MBCs" cases are partially documented or heuristic by nature; keep them in `ExperimentalHeuristic` rather than mixing them into ordinary validation.
- `EMS`, `Bung`, and `Wisdom Tree` should live in that category.
- `Wisdom Tree` in particular must stay separate from standard MBC families because it switches the whole `0x0000-0x7FFF` range and derives bank selection from written address bits rather than standard data-register semantics.
- Heuristic identification for `EMS`, `Bung`, or any new MBC1M-like multicart shape outside the supported signature path should remain disabled by default in `Strict` and `Permissive` and only activate under an explicit `Experimental` loader policy.

### Fallback policy

- Internal code sharing is allowed only for close variants that are architecturally derived from a supported base mapper.
- `MBC30` may share most of its backend with `MBC3`, but it must enter through an explicit typed variant.
- `MBC1M` may share substantial code with `MBC1`, but it must enter through an explicit typed variant or signature-backed path.
- Do not add automatic runtime or loader fallback from:
  - `HuC1` to `MBC1`
  - `HuC-3` to `MBC3`
  - `MBC6` to `MBC3` or `MBC5`
  - `MBC7` to `MBC5`
  - `Pocket Camera` to any ordinary MBC
- For `DocumentedButUnsupported`, fail with the precise detected type.
- For `PlannedVariant`, fail with a message that says the variant is known and reserved, not that the header is invalid.
- For `ExperimentalHeuristic`, keep default strict-mode behavior as "do not run heuristics automatically."
- For `UnknownCode`, report the raw `0x0147` value and stop rather than inventing a mapper.

## Persistence baseline

- Full emulator save states remain a separate whole-machine snapshot system owned by the global save-state boundary in `docs/ARCHITECTURE.md` and phased in `docs/ROADMAP.md` Phase `8`.
- Battery-backed persistence must be modeled as cartridge-owned state, not as "dump the currently visible `0xA000-0xBFFF` window."
- `0xA000-0xBFFF` is only a mapper-controlled access window on the shared T-cycle runtime. The persistible payload is the cartridge's full backing store, not whichever bank or register file happens to be visible on one access.
- The decision that a cartridge should produce hardware-style persistent state must come from header byte `0x0147`, not from filename heuristics, game title, or RAM-size guesses.
- Loaded cartridge metadata should expose explicit capability data such as `has_battery`, `has_rtc`, and a typed distinction between persistent RAM, persistent RTC, persistent EEPROM, non-persistent RAM, and no persistible storage.
- Persistible RAM sizing must come from validated `0x0147 + 0x0149` capability data rather than from `0x0149` alone.
- `MBC2` is a required exception: its persistible store is the internal `512 x 4-bit` nibble array, and `0x0149` must not size it.
- Validation must reject or warn on header combinations where cartridge type, battery capability, RTC capability, and declared RAM size are incoherent before the save layer creates any hardware-style payload.
- `ram_enabled` or equivalent access gating must never decide whether RAM exists or whether it should be saved. Disabled RAM is still cartridge-owned state.
- `NoMbc` persistence covers at most one linear `8 KiB` RAM store when the validated header type actually provides battery-backed RAM, such as `0x09`.
- `MBC1` persistence must cover the full validated SRAM backing, not only the currently visible bank in `0xA000-0xBFFF`.
- `MBC2` persistence must cover all `512` logical nibbles, not an invented `8 KiB` byte array and not only the currently addressed echo window.
- `MBC3` persistence must cover the full validated SRAM backing plus live RTC state when the header type includes timer and battery support.
- `MBC5` persistence must cover the full validated SRAM backing up to `128 KiB`; rumble state is not part of the ordinary battery-backed save payload.
- `MBC6` persistence must cover SRAM, main flash, hidden flash, and the non-volatile sector-0 protection bit; volatile flash command state belongs to whole-machine runtime save states, not hardware-style cartridge saves.
- Whole-machine runtime save states that capture an in-progress `MBC6` main-flash or hidden-flash program operation must preserve and validate the `128`-byte program buffer plus its `128`-entry written bitmap before restore, because those vectors model the pending aligned flash page commit rather than optional host metadata.
- `MBC7` persistence must cover the full `256`-byte serial EEPROM backing and export/import external `.sav` data as those raw `256` bytes rather than as banked SRAM.
- For `MBC3`, the persistible RTC payload must preserve at least seconds, minutes, hours, visible `9`-bit day counter, halt, carry, and enough elapsed-time bookkeeping to reconstruct powered-off advancement on reload.
- The persistible RTC payload must reflect live RTC state, not the latched snapshot exposed for reads through the `0x00 -> 0x01` latch sequence.
- Restoring hardware-style `MBC3` persistence into a live cartridge should refresh `rtc_live` while clearing runtime-local latch state (`rtc_latched`, latch-valid flag, edge-arming state, and advisory ready-at timing) so a stale read snapshot does not survive a persistence restore.
- Hardware-style cartridge saves and full emulator save states are different systems. Cartridge persistence must not serialize CPU, PPU, APU, WRAM, HRAM, or other console-owned state.
- Cartridges without battery support should not produce automatic hardware-style save files unless the emulator explicitly exposes a non-hardware-faithful opt-in policy or the validated cartridge family has a documented persistent non-SRAM medium such as MBC7 EEPROM.
- The cartridge side should expose an explicit typed persistence contract such as `PersistentCartState` or `CartridgePersistentPayload` that the storage backend consumes.
- The save backend should own physical serialization, format versioning, path policy, flush policy, and atomic write strategy, but it must not infer the mapper's persistible layout on its own.
- Supported save triggers should include save-on-close, manual or forced save, and optional auto-flush after writes to persistible cartridge state.
- Auto-flush and durable file I/O belong to the persistence layer around the cartridge contract, not to the bus. The shared runtime still observes mapper writes on the access T-cycle even though host-side flush timing is outside the emulated hardware timeline.

## No MBC family baseline

- Treat No MBC as the first closed cartridge implementation for this repo, not as a generic "no mapper" fallback path.
- This baseline should be the first reference cartridge used to validate bus decode, boot-ROM overlay and `FF50` handoff, header visibility, optional external RAM, and ROM-space write policy before `MBC1` work begins.
- Header codes `0x00`, `0x08`, and `0x09` should be recognized explicitly as the `NoMbc` family.
- `0x00` means ROM-only with no external RAM, `0x08` means ROM + RAM, and `0x09` means ROM + RAM + battery.
- `0x08` and `0x09` are rare and not well documented in licensed cartridges, but rarity is not grounds to reject them automatically; keep the raw type visible for diagnostics while instantiating the same no-banking family.
- For No MBC, the normal documented expectation is `0x0148 = 0x00`, meaning `32 KiB` total ROM with `2` visible `16 KiB` regions and no bank switching.
- If the header declares No MBC while `0x0148` or the real file size imply something other than `32 KiB`, emit an explicit diagnostic under the project's validation policy instead of silently coercing the image into another mapper.
- For `0x0147 = 0x00`, the expected `0x0149` value is `0x00`.
- For `0x0147 = 0x08` or `0x09`, the supported No MBC RAM case is `0x0149 = 0x02`, meaning one linear `8 KiB` external RAM window.
- If `Permissive` admits a legacy `ROM+RAM` header that declares `0x0149 = 0x01`, keep runtime behavior on the same fixed `8 KiB` always-enabled linear RAM window and surface the header mismatch as diagnostics instead of shrinking the device to a speculative `2 KiB` profile.
- If a No MBC header declares more than `8 KiB` of RAM or any banked-RAM expectation, report it as inconsistent or unsupported explicitly.
- At runtime, `0x0000-0x7FFF` reads should be linear ROM reads with no active bank state, subject only to boot-ROM overlay in the bus.
- The header bytes at `0x0100-0x014F` and the entry point at `0x0100` must come from that same cartridge device through ordinary reads after boot-ROM handoff.
- Writes to `0x0000-0x7FFF` remain routed to the cartridge on the shared T-cycle timeline, but No MBC should treat them as ignored writes with no side effects and no fake ROM mutation.
- `0xA000-0xBFFF` is either absent or one linear `8 KiB` RAM window; there is no RAM enable, no RAM banking, no RTC, no rumble, no sensor, and no creative pseudo-mapper behavior in this family.
- When No MBC has no external RAM, the `0xA000-0xBFFF` behavior should follow an explicit project policy for "RAM absent" rather than accidentally reading from zero-initialized backing storage.
- Battery presence changes only persistence expectations. Runtime mapping and live bus behavior stay the same for `0x08` versus `0x09`.
- Persistence belongs to the cartridge/save boundary, not to the bus.
- A concrete `NoMbcCartridge` is the intended implementation shape for this family.
- It should contain at least `rom: Vec<u8>`, `ram: Option<Vec<u8>>`, `has_battery: bool`, and `header: CartridgeHeader`.
- It should not carry `active_rom_bank`, `active_ram_bank`, RAM-enable latches, or similar mutable mapper state because No MBC has none.

## MBC1 baseline

- Treat MBC1 as a real mapper family with four pieces of live state: RAM enable, a `5`-bit primary ROM-bank register, a `2`-bit secondary register, and a `1`-bit banking-mode register. Do not collapse it to one `active_rom_bank`.
- The repo should distinguish at least three MBC1 configuration shapes:
  - standard wiring for up to `512 KiB` ROM with up to `32 KiB` banked external RAM
  - large-ROM / alternate wiring for `1 MiB` or `2 MiB` ROM, where the secondary register extends ROM selection and only one fixed `8 KiB` external RAM window remains practical
  - an explicit `Mbc1Variant::Mbc1M` path for the multicart wiring, because multi-cart MBC1M uses a different bank-selection formula from standard MBC1
- Header codes `0x01`, `0x02`, and `0x03` should be recognized explicitly as the MBC1 family.
- `0x01` means MBC1 with no external RAM, `0x02` means MBC1 + RAM, and `0x03` means MBC1 + RAM + battery.
- Battery presence changes persistence expectations only; it must not change live banking behavior.
- MBC1 configuration must be validated from header metadata and real image size rather than accepted indiscriminately.
- Treat `32 KiB` external RAM as the standard small-ROM wiring case.
- Treat `1 MiB` or `2 MiB` ROM as the alternate large-ROM wiring case, not as banked-`32 KiB` RAM cartridges.
- If declared ROM size, RAM size, real file size, and derived MBC1 wiring disagree, report an explicit diagnostic under the chosen validation policy instead of guessing silently.
- When degraded validation still admits a contradictory MBC1 header, keep the live RAM capability derived from `0x0147`: `0x01` must remain RAM-less, standard-wiring `0x02/0x03` should fall back to the explicit small-ROM RAM baseline, and large-ROM `0x02/0x03` should keep the fixed `8 KiB` RAM window baseline rather than silently inheriting a different size from the conflicting header code.
- When degraded validation admits that contradictory MBC1 header, effective RAM banking must follow the validated backing actually allocated by the loader rather than the contradictory `0x0149` bank-count metadata.
- If the cartridge is too small to observe the secondary register or banking mode, writes may still update the stored register state, but they must not invent visible effects.
- Power-up state must be deterministic for both `RealBoot` and `SkipBoot`: `ram_enabled = false`, `rom_bank_low5 = 0`, `secondary_bank = 0`, and `banking_mode = 0`.
- Even though `rom_bank_low5` powers up as `0`, the switchable ROM window at `0x4000-0x7FFF` must initially expose bank `1`, not bank `0`.
- Direct-boot setup must not depend on accidentally zeroed host memory or allocator behavior.
- `0x0000-0x1FFF` is a write-only RAM-enable register. Any write whose low nibble is `0xA` enables cartridge RAM; any other write disables it.
- When RAM is disabled, `0xA000-0xBFFF` reads should follow an explicit disabled-RAM open-bus policy rather than normal SRAM semantics. `0xFF` is a reasonable default, but the policy should stay explicit and ideally configurable for tests.
- When RAM is disabled, `0xA000-0xBFFF` writes must be ignored.
- `0x2000-0x3FFF` is a write-only `5`-bit primary ROM-bank register. Store `value & 0x1F` as raw register state; bits above bit `4` are discarded.
- The `0 -> 1` translation applies to the raw `5`-bit primary register field used by the high ROM window, not to a later final bank number after ROM-size masking.
- `0x4000-0x5FFF` is a write-only `2`-bit secondary register. Its meaning depends on cartridge wiring and banking mode: RAM bank on compatible `32 KiB` RAM cartridges, or high ROM bits on large-ROM cartridges.
- `0x6000-0x7FFF` is a write-only `1`-bit banking-mode register. Mode `0` fixes `0x0000-0x3FFF` to ROM bank `0` and `0xA000-0xBFFF` to RAM bank `0`. Mode `1` allows the secondary register to affect the low ROM region and/or external RAM selection when the cartridge wiring actually supports it.
- Keep raw register values, intermediate MBC1 bank calculations, and final size-masked bank numbers as separate concepts in code.
- The switchable region `0x4000-0x7FFF` should be resolved from the raw `5`-bit primary register after applying its `0 -> 1` rule, plus secondary bits where the cartridge wiring uses them, then masked to the real ROM size.
- Because the low `5`-bit field translates `0 -> 1` before final size masking, large standard MBC1 cartridges must reproduce the documented inaccessibility of banks `0x20`, `0x40`, and `0x60` in the high region, exposing `0x21`, `0x41`, and `0x61` instead.
- Small ROMs of `256 KiB` or less must still preserve the documented exception where the high region can end up on bank `0` after real-size masking even though the raw low register went through the `0 -> 1` translation.
- In mode `0`, or on cartridges too small to use the secondary register, `0x0000-0x3FFF` reads should resolve to ROM bank `0`.
- On large-ROM cartridges in mode `1`, `0x0000-0x3FFF` must be able to resolve the secondary-register-controlled low-region banks documented by Pan Docs.
- Keep standard MBC1 and MBC1M formulas distinct rather than mixing them behind ad hoc conditionals.
- The current baseline may promote a `1 MiB` `0x01` / `0x02` / `0x03` MBC1 image into `Mbc1Variant::Mbc1M` through an explicit repeated-subheader signature path when commercial multicart structure is present; broad heuristic detection must still remain disabled by default in `Strict` and `Permissive`.
- For `Mbc1Variant::Mbc1M`, the secondary register targets ROM bank bits `4-5` instead of `5-6`, while the primary register contributes only bits `0-3` after the documented raw-`5`-bit `0 -> 1` translation. That means mode `1` low-bank selection resolves to `0x00`, `0x10`, `0x20`, or `0x30`, and the high window can still expose bank `0` when the raw primary register is `0x10`.
- The bus must not implement any MBC1 bank math; all of it belongs inside the cartridge device.
- `0xA000-0xBFFF` should delegate to cartridge-owned external RAM only when the MBC1 configuration actually provides RAM.
- In mode `0`, visible RAM stays on bank `0`.
- On compatible `32 KiB` RAM cartridges in mode `1`, the secondary register selects RAM banks `0..=3`, masked by the real RAM size.
- On large-ROM alternate-wiring cartridges, visible RAM remains one fixed `8 KiB` window even when the secondary register changes.
- Effective RAM-bank selection should also be masked by the real RAM bank count without destroying the pre-mask MBC1 rules that produced it.
- ROM-space writes to MBC1 are ordered cartridge commands on the shared T-cycle timeline.
- A write to `0x0000-0x7FFF` must update mapper state immediately for later bus accesses; do not defer bank changes until the end of the instruction or frame.
- A concrete `Mbc1Cartridge` implementing `CartridgeDevice` is the intended implementation shape for this repo.
- It should contain at least `rom`, optional `ram`, `has_battery`, `ram_enabled`, `rom_bank_low5`, `secondary_bank`, `banking_mode`, header-derived size and capability metadata, and explicit wiring / variant metadata.
- Prefer explicit helpers such as `effective_low_region_rom_bank()`, `effective_high_region_rom_bank()`, and `effective_ram_bank()` so raw register state, wiring decisions, and final masked bank numbers stay inspectable.
- Keep an explicit variant or flag for MBC1M support instead of letting the standard `MBC1` implementation accrete scattered special cases.

## MBC2 baseline

- Treat MBC2 as its own mapper family, not as a cut-down MBC1.
- MBC2 should keep explicit live state for `ram_enabled`, a raw `4`-bit ROM-bank register such as `rom_bank_low4`, internal `512 x 4-bit` RAM, and header-derived metadata.
- MBC2 must not grow MBC1 concepts that do not exist here: no banking mode, no secondary bank register, no banked external SRAM, no RTC, and no separate enable register outside the ROM-space control range.
- Header codes `0x05` and `0x06` should be recognized explicitly as the MBC2 family.
- `0x05` means MBC2 with no persistence, while `0x06` means MBC2 + battery-backed persistence for its internal RAM.
- Battery presence changes persistence expectations only; it must not change visible ROM banking or RAM-enable behavior.
- Standard MBC2 must support at most `256 KiB` ROM, meaning at most `16` total `16 KiB` ROM banks with bank `0` fixed in `0x0000-0x3FFF` and banks `0x01..=0x0F` visible in `0x4000-0x7FFF`.
- If a cartridge header declares MBC2 but the validated ROM size exceeds that limit, the loader should emit an explicit diagnostic rather than guessing a nearby mapper.
- MBC2 RAM is a mapper-local special case: it is internal `512 x 4-bit` RAM inside the MBC, not ordinary external SRAM described by the generic `0x0149` RAM-size table.
- For MBC2, the expected `0x0149` value is `0x00`; do not reinterpret a nonzero `0x0149` value as ordinary external SRAM capacity.
- If an MBC2 cartridge declares `0x0149 != 0x00`, emit a warning or error according to the configured validation policy while still keeping MBC2 RAM modeled as internal mapper RAM.
- The visible MBC2 memory map should be:
  - `0x0000-0x3FFF`: fixed ROM bank `0`
  - `0x4000-0x7FFF`: switchable ROM bank `0x01..=0x0F`
  - `0xA000-0xA1FF`: internal MBC2 RAM
  - `0xA200-0xBFFF`: echoes of the same internal RAM because only the low `9` address bits participate in RAM indexing
- Do not allocate `8 KiB` of byte-wide RAM for MBC2. The implementation should model one logical `512`-cell nibble array and use masked addressing for the echoes.
- MBC2 RAM cells are `4` bits wide. Writes should store only the low nibble of the written value.
- The upper nibble returned by MBC2 RAM reads is not a documented hardware constant. Keep it under an explicit emulator policy rather than accidental host-memory behavior.
- For the current repo default, MBC2 RAM reads should return `0xF0 | stored_nibble`; that is an explicit emulator policy for the undefined high nibble, not a claim that real hardware always reads back that exact pattern.
- Functional tests should treat the low nibble as the hardware-significant part of MBC2 RAM unless the chosen high-nibble policy itself is under test.
- MBC2 ROM-space control writes are multiplexed inside `0x0000-0x3FFF` and must be decoded by address bit `8`, not by fixed `0x0000-0x1FFF` versus `0x2000-0x3FFF` subranges.
- When address bit `8` is `0`, the write is a RAM-enable command. When address bit `8` is `1`, the write is a ROM-bank command.
- Keep that address-bit decode inside `Mbc2Cartridge`; the bus should still delegate the whole ROM-space write to the cartridge device rather than inspecting MBC2-specific address rules.
- For RAM-enable writes, MBC2 should enable RAM only when the written low nibble is `0xA`; any other low nibble should disable RAM.
- Power-up state must be deterministic for both `RealBoot` and `SkipBoot`: `ram_enabled = false` and raw `rom_bank_low4 = 0`, while the effective switchable-ROM bank must start at bank `1`.
- For ROM-bank writes, keep only the written low nibble as raw MBC2 bank-register state and ignore the upper `4` bits of the data byte.
- The switchable ROM window at `0x4000-0x7FFF` must apply the documented `0 -> 1` translation to that raw `4`-bit bank field.
- Keep raw `rom_bank_low4`, the documented `0 -> 1` translation, and final ROM-size masking as explicit separate concepts in code.
- Effective MBC2 ROM-bank selection should be masked by the real number of loaded ROM banks without losing the documented raw-register `0 -> 1` semantics.
- MBC2 must distinguish RAM-enabled from RAM-disabled behavior explicitly.
- With RAM disabled, writes to `0xA000-0xBFFF` must be ignored.
- With RAM disabled, reads from `0xA000-0xBFFF` should use an explicit project open-bus policy rather than pretending the internal RAM remains normally readable. `0xFF` is a reasonable default, but keep the policy explicit and testable.
- RAM indexing for `0xA000-0xBFFF` should use only the low `9` address bits, such as through a helper like `effective_ram_index(addr) = addr & 0x01FF`.
- `0xA000-0xA1FF` and every echo in `0xA200-0xBFFF` must therefore alias the same `512` logical cells rather than a duplicated backing store.
- Persistence for `0x06` should cover the internal `512 x 4-bit` RAM contents; `0x05` should not persist those contents automatically.
- Persistence must remain a cartridge/save-layer concern rather than bus logic.
- ROM-space writes to MBC2 are ordered cartridge commands on the shared T-cycle timeline.
- A write that changes MBC2 RAM-enable or ROM-bank state must become visible on the access T-cycle for all later cartridge accesses; do not defer mapper changes to instruction or frame boundaries.
- A concrete `Mbc2Cartridge` implementing `CartridgeDevice` is the intended implementation shape for this repo.
- It should contain at least `rom`, `ram_nibbles` with `512` logical cells, `has_battery`, `ram_enabled`, `rom_bank_low4`, and `header`.
- Prefer explicit helpers such as `is_ram_enable_write(addr)`, `is_rom_bank_write(addr)`, `effective_high_region_rom_bank()`, and `effective_ram_index(addr)` so address decode, raw register state, and final effective mapping remain inspectable.
- Do not add `banking_mode`, `secondary_bank`, or generic external-RAM-bank state to `Mbc2Cartridge`.

## MBC3 baseline

- Treat MBC3 as its own mapper family, not as "MBC1 plus RTC."
- MBC3 has three coupled cartridge-local behaviors that must stay explicit in code: ROM banking, external-RAM banking, and an RTC register file that shares the `0xA000-0xBFFF` window through a selector.
- The implementation must not collapse standard MBC3 RAM-bank selection `0x00..=0x03`, reserved selector values `0x04..=0x07`, and RTC-register selection `0x08..=0x0C` into one undifferentiated bank number. They share an address window, not a meaning.
- The RTC must keep its own live state separate from any external-RAM backing and separate from bus-owned state; it is not a bus patch layered on top of SRAM.
- MBC3 should keep explicit live state for at least `ram_rtc_enabled`, a raw `7`-bit ROM-bank register such as `rom_bank`, an explicit typed `ram_or_rtc_select` state that can represent RAM banks, reserved selectors, and RTC registers, RTC live state, RTC latched snapshot state, latch-edge detection for the `0x00 -> 0x01` command sequence, battery / RTC capability flags, and header-derived metadata.
- Header codes `0x0F`, `0x10`, `0x11`, `0x12`, and `0x13` should be recognized explicitly as the MBC3 family.
- `0x0F` means MBC3 + timer + battery, `0x10` means MBC3 + timer + RAM + battery, `0x11` means plain MBC3, `0x12` means MBC3 + RAM, and `0x13` means MBC3 + RAM + battery.
- Header-derived capability state must distinguish plain MBC3 from RTC-backed MBC3 cartridges; not every MBC3 cartridge has timer hardware.
- Keep `Mbc3Variant::Mbc30` or equivalent as an explicit supported near-family variant instead of letting standard MBC3 quietly absorb the later MBC30 differences.
- Standard MBC3 must support up to `2 MiB` ROM, meaning up to `128` total `16 KiB` ROM banks with bank `0` fixed in `0x0000-0x3FFF` and banks `0x01..=0x7F` visible in `0x4000-0x7FFF`.
- Standard MBC3 external RAM support should stop at `32 KiB`; if header metadata implies the later `64 KiB` SRAM configuration associated with MBC30, route that cartridge into the explicit MBC30 variant rather than silently treating it as ordinary MBC3.
- For ordinary MBC3 RAM validation, accept the standard external-RAM size codes that appear in the generic table for small SRAM-backed cartridges, including `0x01` (`2 KiB`), `0x02` (`8 KiB`), and `0x03` (`32 KiB`); only `0x05` should trigger the explicit MBC30 variant path.
- Unlike standard MBC1, MBC3 high-region ROM banking must allow banks `0x20`, `0x40`, and `0x60` to be selected normally when present.
- The visible MBC3 memory map should be:
  - `0x0000-0x3FFF`: fixed ROM bank `0`
  - `0x4000-0x7FFF`: switchable ROM bank `0x01..=0x7F`
  - `0xA000-0xBFFF`: external RAM bank `0x00..=0x03`, reserved selector states `0x04..=0x07`, or RTC register `0x08..=0x0C`, depending on the current selector
- The bus should still delegate `0xA000-0xBFFF` completely to the cartridge device; it must not infer from the address alone whether the active target is SRAM or RTC.
- For this repo's public observability contract, the cartridge should expose a typed descriptor for the currently selected `0xA000-0xBFFF` aperture so `Bus::resolve_access()` can report whether the live target is RAM, RTC, a reserved selector, disabled, or absent without duplicating MBC3 logic in the bus layer.
- `0x0000-0x1FFF` is a write-only RAM / RTC-enable register. Any write whose low nibble is `0xA` enables both external RAM and RTC-register access; other values disable both.
- With RAM / RTC disabled, `0xA000-0xBFFF` reads and writes should follow one explicit project policy rather than accidental backing-store behavior. A default of `0xFF` is acceptable, but the policy should remain explicit and testable.
- `0x2000-0x3FFF` is a write-only raw `7`-bit ROM-bank register. Store `value & 0x7F`, ignore upper data bits, apply the documented `0 -> 1` translation for the switchable ROM window, and then mask the effective bank by the real number of loaded ROM banks.
- `0x4000-0x5FFF` is a write-only selector for the `0xA000-0xBFFF` window. Standard MBC3 values `0x00..=0x03` select RAM banks, values `0x08..=0x0C` select RTC registers, and values `0x04..=0x07` remain explicit reserved or invalid selector states; MBC30 gives `0x04..=0x07` the distinct meaning of extended SRAM banks.
- Current source divergence note: the latest public `Pan Docs` wording says `$00-$07` select RAM banks, but the retained curated oracle `cpp/rtc-invalid-banks-test.gb` actively writes to selectors `0x04..=0x07`, re-reads through the same selectors, and only stays green when those states remain invalid or reserved rather than exposing banked SRAM semantics. Until stronger hardware evidence settles the conflict, keep the project model explicit about that compatibility choice instead of silently widening standard MBC3 RAM banking.
- The MBC3 RAM / RTC selector should decode from the low nibble of the written value. Upper data bits must not create a different selector namespace.
- Represent the `0x4000-0x5FFF` selector as an explicit mapping target such as `RamBank(u8)`, `ReservedSelector(u8)`, or `RtcRegister(RtcRegisterId)` instead of as one raw numeric field whose meaning is reconstructed ad hoc during each access.
- External RAM banking must be masked by the real number of available RAM banks declared by validated cartridge metadata; standard MBC3 must not silently treat a `64 KiB` RAM declaration as ordinary banked SRAM support, while MBC30 uses the validated `8` SRAM banks explicitly.
- When the validated MBC3 SRAM backing is smaller than the visible `8 KiB` aperture, such as the accepted `2 KiB` case, reads and writes through `0xA000-0xBFFF` should wrap within the real SRAM payload instead of exposing out-of-range holes.
- `0x6000-0x7FFF` is a write-only RTC latch command register. The project keeps the first accepted latch on the logical edge formed by writing `0x00` and then `0x01`, so a first post-reset `0x01` does not latch until software has armed the edge with `0x00`.
- Keep RTC live state and RTC latched state as separate concepts. RTC register writes should update only the live RTC state; reads must continue to observe the currently latched snapshot until software issues the next latch command accepted by the controller.
- Before the first accepted latch, the current project policy is explicit: RTC register reads observe a zeroed snapshot rather than live RTC state. That keeps the pre-latch state deterministic and avoids relying on whatever bytes happened to sit in the latched-storage field at reset.
- The visible RTC register file must include seconds, minutes, hours, day low, and day high / flags.
- Seconds and minutes should stay within `0..=59`, hours within `0..=23`, and the visible day counter within `0..=511`.
- Day-counter state should be modeled as a `9`-bit value split across `DL` and `DH.bit0`.
- `DH.bit6` is the halt flag, and `DH.bit7` is the carry flag.
- When the live day counter overflows past `511`, it must wrap within the visible `9`-bit range and set the carry flag.
- The carry flag must remain set until software clears it through RTC register writes; it must not auto-clear merely because the clock continues to advance.
- `halt = 1` must stop progression of the live RTC state.
- The live MBC3 RTC keeps an explicit subsecond phase derived from the cartridge clock domain, not from the CPU timer or `DIV`. The current deterministic runner injection advances that domain at `32.768 kHz` by feeding one RTC clock tick per `128` normal-speed CPU-visible runner T-cycles, scaling to `256` CPU-visible runner T-cycles in CGB double speed so RTC wall-clock progression does not inherit the CPU speed multiplier, while host frontends should derive the same `32.768 kHz` tick budget from suspend-aware elapsed host wall-clock time and release it on the live emulation T-cycle cadence instead of jumping MBC3 by whole seconds or batching subsecond ticks at frame boundaries.
- Writing the seconds RTC register resets only the MBC3 RTC subsecond phase; writing minutes, hours, day low, or day high preserves the current subsecond phase. This keeps subsecond write timing observable without normalizing unrelated register writes into a new time origin.
- Setting `DH.bit6` (`halt = 1`) freezes both whole-second progression and the subsecond phase; clearing halt resumes from the preserved phase instead of restarting the clock.
- Pan Docs' recommendation to set `halt` before writing RTC registers should be documented as a hardware-usage rule, but the emulator does not need to reject out-of-flow writes unless later hardware evidence demands that restriction.
- When the selector targets `0x08..=0x0C`, writes to `0xA000-0xBFFF` must update the live RTC register state, not the latched snapshot.
- The current implementation records the next recommended RTC-ready point as explicit cartridge state on timed `0x08..=0x0C` accesses: each RTC-register read or write updates `rtc_access_ready_at = current_t_cycle + 16`. This keeps the `4 us` / `16`-T-cycle spacing recommendation observable and testable without yet enforcing an early-access penalty.
- That advisory `rtc_access_ready_at` state should be surfaced through the public cartridge-external aperture descriptor and cartridge snapshot APIs, rather than remaining inspectable only from mapper-internal tests.
- Current cross-check note: `Pan Docs` documents the spacing as a recommendation, not a defined early-access failure mode, and a direct read of `SameBoy`'s public `MBC3` memory path also does not show an explicit early-access penalty. Keep this project policy advisory-only until hardware evidence or a stronger dedicated oracle demonstrates an observable penalty.
- Preserve the architecturally visible bits of each written RTC register for later readback through the latched register file: seconds and minutes keep their low `6` bits, hours keeps its low `5` bits, day low keeps all `8` bits, and day high keeps bit `0` plus halt/carry. Only normalize those visible values into valid running-clock ranges when advancing elapsed time in the live RTC model.
- When software writes non-canonical visible RTC values, elapsed-time advancement first ticks those visible bit-width fields by hardware-shaped rollover instead of immediately decimal-normalizing them: seconds/minutes roll within their `6` visible bits, hours roll within `5` visible bits, and the valid `59 -> 00`, `23 -> 00`, and day-carry paths remain the normal running-clock behavior once the fields are canonical.
- After one valid snapshot exists, the current curated-compatibility model also accepts follow-up non-zero `0x6000-0x7FFF` writes as relatch commands. This is an intentional compatibility deviation from the stricter `Pan Docs` reading, kept to match the retained `cpp/latch-rtc-test.gb` oracle after instrumenting that ROM and observing an initial `0x00 -> 0x01` latch followed by repeated non-zero relatch writes without re-arming zeros. Record this explicitly so it can be revisited later.
- MBC3 control writes are ordinary cartridge commands on the shared T-cycle timeline. Changes to ROM bank, RAM bank, RTC selector, RAM / RTC enable, and latch state must become visible on the access T-cycle for all later cartridge accesses; do not defer them to instruction or frame boundaries.
- Treat MBC3 bus-visible ordering as T-cycle based even though the RTC itself is driven by a `32.768 kHz` external oscillator in hardware. The long-term RTC progression should come from an injected time / persistence source, not from blindly counting executed CPU T-cycles as if the RTC were just another divider.
- Frontends or tools that run a live real-time session must keep feeding elapsed wall-clock seconds into the cartridge RTC while the session stays open; applying elapsed time only when loading a save is not sufficient for games that expect the clock to tick during play.
- The RTC design should separate three layers explicitly: visible RTC registers, live RTC counter state, and emulator-provided time / persistence infrastructure.
- Battery-backed persistence policy should cover RTC state as well as external RAM where the header declares battery support, while the save backend remains responsible only for storage and time-source integration rather than visible bus semantics.
- The RTC path must support a deterministic injected or simulated time source for tests; unit and ROM tests must not depend on the host wall clock.
- Pan Docs recommends leaving roughly `4 us` between separate RTC register accesses. For this project's timing vocabulary, that is `16` T-cycles at normal-speed DMG (`4` M-cycles). Keep it as a current research / validation note rather than as an already enforced bus restriction unless the implementation closes that accuracy point explicitly with stronger hardware evidence or a dedicated oracle.
- A concrete `Mbc3Cartridge` implementing `CartridgeDevice` is the intended implementation shape for this repo.
- It should contain at least `rom`, optional `ram`, `has_battery`, `has_rtc`, `ram_rtc_enabled`, `rom_bank`, `ram_or_rtc_select`, `rtc_live`, `rtc_latched`, latch-sequence state, subsecond RTC phase state, and `header`.
- Prefer explicit helpers such as `effective_rom_bank()`, `effective_ram_bank()`, `current_a000_mapping()`, and `latch_rtc_if_needed()` so raw register state, target-selection state, live RTC state, latched RTC state, and final effective mapping remain inspectable.
- MBC30 support must stay variant-owned inside the MBC3-family cartridge path: code sharing with standard MBC3 is allowed, but ROM-bank width, RAM selector range, validation, diagnostics, and persistence sizing must remain explicitly tied to the typed variant rather than loose conditionals.

## MBC5 baseline

- Treat MBC5 as its own mapper family, not as "MBC3 with more ROM bits."
- MBC5 is the large and comparatively direct classic Game Boy mapper family: up to `8 MiB` ROM, up to `128 KiB` banked external RAM, a raw `9`-bit ROM-bank selector split as low `8` bits plus high `1` bit, fixed ROM bank `0` in `0x0000-0x3FFF`, and a switchable ROM window that legitimately allows bank `0`.
- Pan Docs also notes that MBC5 was the first MBC family that behaved correctly with CGB double-speed mode. For this repo, that should translate to one T-cycle-ordered cartridge model that can later sit under double-speed scheduling without introducing mapper-local timing shortcuts.
- MBC5 should keep explicit live state for `ram_enabled`, raw `rom_bank_low8`, raw `rom_bank_high1`, raw `ram_bank_raw`, optional external RAM, `has_battery`, `has_rumble`, `rumble_on`, and header-derived metadata.
- Header codes `0x19`, `0x1A`, `0x1B`, `0x1C`, `0x1D`, and `0x1E` should be recognized explicitly as the MBC5 family.
- `0x19` means MBC5 with no external RAM.
- `0x1A` means MBC5 + RAM.
- `0x1B` means MBC5 + RAM + battery.
- `0x1C` means MBC5 + rumble.
- `0x1D` means MBC5 + rumble + RAM.
- `0x1E` means MBC5 + rumble + RAM + battery.
- Loader-visible variant metadata should distinguish at least no-RAM, RAM, RAM+battery, rumble-only, rumble+RAM, and rumble+RAM+battery shapes rather than flattening them into one generic `Mbc5`.
- Battery presence changes persistence expectations only. Rumble changes cartridge-local motor state and must not change ROM banking semantics.
- MBC5 must validate up to `8 MiB` ROM, meaning up to `512` total `16 KiB` ROM banks with bank `0` fixed in `0x0000-0x3FFF` and banks `0x000..=0x1FF` visible in `0x4000-0x7FFF`.
- Standard non-rumble MBC5 external RAM support should cover ordinary `8 KiB`, `32 KiB`, `64 KiB`, and `128 KiB` SRAM configurations, meaning up to `16` visible `8 KiB` RAM banks.
- Rumble-capable MBC5 external RAM support should cover `8 KiB`, `32 KiB`, and `64 KiB` SRAM configurations. Because `bit 3` of the RAM-bank control register is wired to the motor rather than the RAM chip, only `3` bank-select bits remain for external RAM on those variants.
- MBC5 external RAM should be modeled as linear `8 KiB` banks selected directly by the RAM-bank register, with no MBC1-style dual banking mode.
- If a cartridge declares an MBC5 header type but validated ROM size exceeds `8 MiB`, the loader should emit an explicit diagnostic instead of guessing another mapper.
- If a cartridge declares an MBC5 header type with impossible RAM metadata, such as RAM omitted by `0x0147` while `0x0149 != 0x00`, a standard non-rumble MBC5 RAM size larger than `128 KiB`, or a rumble-capable MBC5 RAM size larger than `64 KiB`, the loader should emit an explicit diagnostic under the chosen validation policy.
- The visible MBC5 memory map should be:
  - `0x0000-0x3FFF`: fixed ROM bank `0`
  - `0x4000-0x7FFF`: switchable ROM bank `0x000..=0x1FF`
  - `0xA000-0xBFFF`: external RAM bank `0x00..=0x0F`, if present and enabled
- The bus must not know MBC5's `9`-bit ROM-bank math or rumble wiring. It should continue delegating cartridge-owned regions to the cartridge device through the stable cartridge interface.
- `0x0000-0x1FFF` is a write-only RAM-enable register. Any write whose low nibble is `0xA` enables external RAM; other values disable it.
- With RAM disabled, `0xA000-0xBFFF` reads and writes should follow one explicit project policy rather than accidental backing-store behavior. A default of `0xFF` is acceptable, but the policy should remain explicit and testable.
- `0x2000-0x2FFF` should store the low `8` bits of the ROM-bank register.
- `0x3000-0x3FFF` should store the high `1` bit of the ROM-bank register.
- Keep those raw ROM-bank fields separate in code instead of collapsing them immediately into one opaque current-bank value.
- Hardware still appears to power up MBC5 with bank `1` visible in `0x4000-0x7FFF`, even though later writes may legitimately select bank `0` there.
- Unlike MBC1, MBC2, and MBC3, MBC5 must not apply a `0 -> 1` translation to the high ROM window. Writing bank `0` should really expose bank `0` in `0x4000-0x7FFF`.
- Effective MBC5 ROM-bank selection should combine `rom_bank_low8` plus `rom_bank_high1` into one `9`-bit value and then mask by the real number of loaded ROM banks without inventing a synthetic `0 -> 1` rule.
- Do not reuse MBC1 or MBC3 helper paths if they carry the `0 -> 1` rule, because that would make valid MBC5 high-window bank `0` unreachable.
- `0x4000-0x5FFF` is a write-only RAM-bank / rumble control register.
- On standard non-rumble MBC5, use the low `4` bits as the raw RAM-bank register and then mask by the real RAM-bank count.
- On rumble-capable MBC5, `bit 3` of that control register should update `rumble_on`, while the remaining RAM-bank-relevant bits should still resolve the effective RAM bank according to the validated cartridge wiring.
- In practice, that means current rumble-capable MBC5 validation should accept RAM size codes `0x02` (`8 KiB`), `0x03` (`32 KiB`), and `0x05` (`64 KiB`), but reject `0x04` (`128 KiB`) because bank bits `0..=2` can select at most `8` external RAM banks once `bit 3` is reserved for rumble.
- Keep `ram_bank_raw`, `effective_ram_bank()`, and `effective_rumble_state()` as distinct concepts. Do not collapse rumble-capable MBC5 behavior into one integer whose meaning is reconstructed ad hoc.
- For the current project scope, rumble modeling should stop at the digital hardware-visible motor state. No physical inertia or analog intensity model is required yet.
- The cartridge should expose `rumble_on` as observable cartridge-local state. A frontend may translate that state into host vibration, but it must not authoritatively set rumble state without going through cartridge writes.
- In cartridges without external RAM, `0xA000-0xBFFF` must not behave like SRAM merely because `ram_bank_raw` exists. RAM presence still comes from validated header capabilities.
- MBC5 control writes are ordinary cartridge commands on the shared T-cycle timeline. RAM-enable, ROM-bank, and RAM-bank changes must become visible on the access T-cycle for all later cartridge accesses; do not defer them to instruction or frame boundaries.
- A concrete `Mbc5Cartridge` implementing `CartridgeDevice` is the intended implementation shape for this repo.
- It should contain at least `rom`, optional `ram`, `has_battery`, `has_rumble`, explicit MBC5 variant metadata, `ram_enabled`, `rom_bank_low8`, `rom_bank_high1`, `ram_bank_raw`, `rumble_on`, and `header`.
- Prefer explicit helpers such as `effective_rom_bank()`, `effective_ram_bank()`, and `effective_rumble_state()` so raw register state, variant decisions, final masked bank numbers, and observable rumble state remain inspectable.

## Timing / accuracy requirements

- Access behavior must remain compatible with bus ordering.
- Architecture should scale from the No MBC family to MBC1, MBC2, MBC3, MBC5, and later extensions.
- Direct-boot initialization should not assume cartridge RAM starts clean unless that follows from persisted save data or an explicit uninitialized-memory policy.
- Writes in ROM address space should be interpreted as cartridge/MBC control behavior where applicable, not as attempts to mutate ROM contents.
- Cartridge-visible reads and writes should remain ordinary bus transactions on the shared T-cycle timeline; mapper side effects must occur in access order rather than in a deferred per-instruction batch.
- For MBC1, MBC2, and later banked mappers, RAM-enable and bank-select writes should become visible on the access T-cycle for all later cartridge reads and writes.
- Header parsing is configuration work at load time, but runtime visibility of header bytes at `0x0100-0x014F` must still emerge from normal ROM bank `0` reads after boot-ROM handoff.
- For No MBC specifically, linear ROM reads, ignored ROM-space writes, and optional external-RAM accesses should still happen through the same ordered T-cycle cartridge transactions rather than through a bus-local fast path.

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

`docs/TESTING.md` owns the detailed cartridge test matrix. Keep this handbook focused on hardware contracts and avoid duplicating every closed unit, integration, synthetic-ROM, or commercial-oracle case here.

Cartridge verification should continue to cover these boundaries:

- header parsing, size decoding, central classification, execution-mode admission, diagnostics, and explicit no-fallback handling for unsupported or CGB-gated cartridges
- mapper-visible behavior for each supported device family and variant: `NoMbc`, `MBC1`, `MBC2`, standard `MBC3`, `MBC30`, `MBC5`, `MMM01`, the supported `MBC1M` signature path, `M161`, `HuC1`, `HuC-3`, and `Pocket Camera`
- T-cycle ordering for ROM-space mapper commands, RAM / RTC aperture reads and writes, and boot/post-boot visibility of `0x0100-0x014F` through the cartridge device rather than through a shadow header copy
- cartridge-owned persistence for full backing stores, battery-gated save eligibility, `MBC2` nibble RAM, `MBC3` live RTC state plus powered-off elapsed time, and external `.sav` import/export boundaries
- retained oracle coverage for intentionally narrow compatibility choices, especially the current MBC3 relatch rule, reserved selector values `0x04..=0x07`, and advisory-only RTC access spacing

New cartridge tests should be added to the concrete owning test module or ROM oracle lane, then summarized in `docs/TESTING.md` or the relevant roadmap entry only when the coverage changes project policy.

## Implementation notes for this repo

### Ownership and loader shape

- Prefer one typed `CartridgeHeader` plus decoded capability fields over raw-code lookups scattered throughout the codebase.
- Keep `0x0147`, `0x0148`, and `0x0149` decoding centralized in the cartridge loader instead of reinterpreting them inside the bus, boot code, persistence backend, or frontends.
- Preserve `cgb_flag` and `sgb_flag` even while the current functional core is DMG-family focused; future CGB/SGB mode selection depends on those header bits.
- Use one central special-cartridge classifier that converts raw header metadata into either a supported cartridge build path or a typed unsupported classification carrying `header_type_code`, detected name, category, and reason.
- Use one central compatibility-policy evaluator that consumes typed cartridge classification plus explicit overrides. Execution mode controls admission, validation severity, heuristics, and diagnostics; it must not change T-cycle-visible runtime semantics for already supported cartridges.
- Keep manual mapper/model overrides visible in logs, save states, replays, and debugging UI. Hidden overrides make oracle comparisons and persistence diagnosis unreliable.

### Device boundary

- Model each supported cartridge as a cartridge-owned device behind the stable bus-facing contract. The bus should delegate fixed ROM, switchable ROM, and external cartridge ranges rather than embedding mapper-specific bank math.
- ROM-space writes are cartridge commands. They must hit the mapper/device on the access T-cycle instead of mutating ROM bytes or being deferred to an instruction/frame boundary.
- Keep raw mapper registers, resolved effective-bank helpers, aperture descriptors, and validated cartridge metadata as separate layers. Avoid one opaque `active_bank` field that hides the quirk being modeled.
- Treat cartridge RAM power-up contents, whether external or mapper-local, as separate from deterministic direct-boot CPU/MMIO state. Any deterministic direct-boot policy must remain explicit and configurable.
- Disabled or absent cartridge RAM behavior must be an explicit project policy, not accidental zero-backed host memory.

### Current supported mapper/device contracts

- `NoMbc` is a concrete device, not a blob-reader fallback. It has no mapper bank state, ignores ROM-space writes through the normal command path, and exposes only absent RAM or one validated linear RAM store.
- `MBC1` keeps raw register fields, standard versus large-ROM wiring metadata, the explicit `MBC1M` variant path, and helpers that apply the primary-register `0 -> 1` rule before final ROM-size masking.
- `MBC2` is its own mapper with address-bit-`8` control decode, a raw `4`-bit ROM-bank register, one `512 x 4-bit` internal RAM array, low-`9`-bit echo aliasing, and the repo's explicit `0xF0 | nibble` readback policy. `0x0149` is validation metadata only for this family.
- `MBC3` keeps RAM/RTC enable state, typed RAM / reserved / RTC selector state, separate live and latched RTC state, and explicit latch-edge tracking. Standard `MBC3` uses a raw `7`-bit ROM-bank register and stops at `32 KiB` SRAM, while `MBC30` uses the explicit variant path for an `8`-bit ROM-bank register and `64 KiB` SRAM.
- `MBC5` keeps raw low `8` and high `1` ROM-bank fields, valid switchable-window bank `0`, linear RAM banks, explicit rumble-capable variant metadata, and observable `rumble_on` state separate from effective RAM-bank selection.
- `MBC6` is a dedicated device with two independent `8 KiB` ROM/flash windows, two independent `4 KiB` SRAM windows, cartridge-local main flash plus hidden flash state, synchronous status-mode flash side effects, and typed persistence for SRAM, main flash, hidden flash, and sector-0 protection.
- `MMM01`, the supported `MBC1M` signature path, and `M161` are first-class multicart paths. Do not redistribute their boot-header, menu, mask-locking, or latch-until-power-off behavior into ordinary `MBC1` or `NoMbc` code.
- `HuC1` and `HuC-3` are dedicated supported mapper families. `HuC1` owns its RAM/IR mode split; `HuC-3` owns its mailbox, semaphore, MCU-window RTC model, IR mode, and dedicated persistence shape. Neither should fall back to `MBC1` or `MBC3`.
- `Pocket Camera` is a dedicated cartridge-local hardware family with camera registers, capture timing, SRAM, and host-frame state inside the cartridge subsystem. See `GAME-BOY-CAMERA.md` for the detailed camera contract.
- `MBC30` is implemented as a supported `MBC3`-family variant. `MBC6` is a dedicated supported split-window SRAM/ROM/flash cartridge family, and `MBC7` is a dedicated supported sensor / EEPROM cartridge family with explicit no-rumble runtime policy.

### Persistence and external save boundaries

- Keep the typed persistence contract attached to the cartridge layer. Payloads describe full cartridge-owned backing stores, not whichever `0xA000-0xBFFF` bank or register is currently visible.
- Hardware-style cartridge saves and whole-machine save states are different systems. Cartridge persistence must not serialize CPU, PPU, APU, WRAM, HRAM, or other console-owned state.
- Save eligibility comes from validated cartridge capability metadata such as battery, RTC, and persistence profile. `ram_enabled` never decides whether a backing store exists or should be saved.
- The host-side persistence backend owns serialization, format versioning, path policy, flush policy, time-source integration, and safe file replacement. None of that belongs in the bus or mapper runtime.
- Default host-side save keys preserve the active ROM's exact filename stem. Path separators, control characters, and portable-filesystem reserved characters require explicit frontend/tool overrides; frontends and tools keep a read/export fallback for the older underscore-sanitized key so existing saves are not orphaned.
- External `.sav` interchange is an explicit host-side conversion boundary. Linear RAM-backed cartridges import/export raw bytes; `MBC3` RAM/RTC uses the common `48`-byte little-endian RTC suffix with elapsed RTC time applied through the persistence time source; `MBC2` import accepts both SameBoy's `512`-byte one-byte-per-nibble layout and mGBA's `256`-byte packed layout, while export defaults to the mGBA packed form; `MBC6` import/export uses `SRAM || main flash` only when hidden flash is still all `0xFF` and sector-0 protection is clear; `MBC7` EEPROM import/export is exactly the raw `256`-byte EEPROM payload. Mapper/profile combinations without a safe shared external mapping, including HuC-3-specific RTC/MCU state, protected/non-default MBC6 hidden state, or Bandai TAMA5-style accessory data unless a compatibility contract is documented, must fail explicitly instead of silently dropping state.

### Compatibility and research seams

- Keep heuristic `EMS`, `Bung`, and `Wisdom Tree` detection outside the supported-mapper fast path. Strict and permissive loading must leave those heuristics disabled unless an explicit experimental policy enables them.
- For MBC3, preserve the current documented compatibility seams until better evidence exists: first latch requires `0x00 -> 0x01`, follow-up non-zero relatches are accepted after one valid snapshot, selector values `0x04..=0x07` remain reserved, and RTC access spacing remains advisory-only via explicit `rtc_access_ready_at` state.
- When a future mapper shares code with an existing family, it must still enter through an explicit typed variant or device path. Code reuse must not become silent runtime fallback.

## Known pitfalls

- treating the cartridge as a raw ROM blob with ad hoc mapper `if` statements
- treating No MBC as too trivial to deserve a real cartridge device
- leaking mapper knowledge into unrelated modules
- under-designing the cartridge boundary so later MBCs become invasive
- silently zeroing cartridge RAM during direct boot and then treating that as hardware-accurate startup behavior
- teaching the generic bus how a specific MBC banks ROM or RAM instead of delegating that behavior to the cartridge subsystem
- dumping the currently visible `0xA000-0xBFFF` contents as if that were always the full save payload
- exporting `.sav` files by truncating mapper-owned state to whichever raw RAM layout is convenient instead of requiring an explicit SameBoy/mGBA-compatible mapping for that cartridge profile
- collapsing every non-mainline cartridge into one opaque `Unsupported` bucket and then forcing the frontend to rediscover what was already known at load time
- deciding save eligibility from `ram_enabled`, filename heuristics, or `0x0149` alone instead of validated cartridge capabilities
- inferring the mapper from ROM size or other heuristics instead of using `0x0147`
- adding fake active-bank or latch state to No MBC
- using `0x0149` alone to decide whether external RAM exists
- modeling `MBC2` RAM as if it were ordinary banked SRAM
- dropping `cgb_flag` or `sgb_flag` because they are not immediately used by the DMG baseline
- silently accepting No MBC headers that declare more than `32 KiB` ROM or more than `8 KiB` RAM without diagnostics
- silently coercing unsupported or inconsistent headers into a nearby supported configuration
- letting `Strict`, `Permissive`, or `Experimental` change the runtime banking, timing, or register semantics of already supported cartridges
- collapsing MBC1 to one `active_rom_bank` and losing the raw register semantics that drive its quirks
- applying the MBC1 `0 -> 1` rule after final ROM-size masking instead of on the raw `5`-bit primary register field
- assuming bank `0` can never appear in `0x4000-0x7FFF` on small MBC1 ROMs
- treating all MBC1 cartridges as if the secondary register always means the same thing regardless of wiring and banking mode
- folding MBC1M behavior into standard MBC1 bank math with scattered conditionals
- modeling MBC2 as "MBC1 with fewer bits" instead of as its own mapper with address-bit-`8` control decode
- splitting MBC2 control writes into arbitrary `0x0000-0x1FFF` / `0x2000-0x3FFF` subranges instead of decoding address bit `8`
- modeling MBC2 RAM as ordinary byte-wide `8 KiB` SRAM instead of one `512 x 4-bit` internal array with echoes
- forgetting that `0xA200-0xBFFF` aliases the same MBC2 RAM cells as `0xA000-0xA1FF`
- letting MBC2 high-nibble readback or disabled-RAM behavior emerge accidentally from host memory instead of one explicit project policy
- modeling MBC3 as if it were just MBC1 plus a decorative clock register
- treating standard MBC3 RAM-bank values `0x00..=0x03`, reserved selector values `0x04..=0x07`, and RTC-register values `0x08..=0x0C` as one interchangeable selector namespace
- reading MBC3 RTC registers from live state instead of the latched snapshot
- writing MBC3 RTC register updates into the latched snapshot instead of the live RTC state
- deriving MBC3 RTC progression directly from executed CPU cycles instead of from an explicit time / persistence source
- persisting the latched MBC3 RTC snapshot as if it were the live battery-backed clock
- assuming MBC3 cannot select ROM banks `0x20`, `0x40`, or `0x60` because MBC1 cannot
- silently treating `64 KiB` SRAM declarations as ordinary standard MBC3 instead of routing them through the explicit MBC30 variant
- deferring MBC3 bank, selector, enable, or latch effects until instruction end instead of applying them on the access T-cycle
- treating `MMM01` as ordinary `MBC1` and assuming the only relevant boot header always sits at physical ROM offset `0x0100`
- treating `HuC1` as "good enough" `MBC1`, `HuC-3` as "good enough" `MBC3`, `MBC6` as "good enough" `MBC3` / `MBC5`, or `MBC7` as "good enough" `MBC5`
- auto-enabling heuristic `EMS` / `Bung` / `Wisdom Tree` detection in strict mode and then mistaking heuristic guesses for header-backed truth
- spreading compatibility-policy decisions across loader, frontend, debugger, and persistence code instead of routing them through one central matrix
- allowing manual mapper or mode overrides to become invisible in logs, save states, replays, or debugging UI
- applying the MBC1/MBC3 `0 -> 1` bank rule to MBC5 and accidentally making bank `0` unreachable in `0x4000-0x7FFF`
- collapsing MBC5's `9`-bit ROM-bank register into one lossy `8`-bit field and losing banks above `0xFF`
- silently treating rumble-capable MBC5 header types as if their RAM-bank register were identical to standard non-rumble MBC5
- loading `0x1C`, `0x1D`, or `0x1E` as plain non-rumble MBC5 and thereby hiding cartridge-local rumble state from the rest of the system
- persisting `rumble_on` or full-console state inside the cartridge save payload instead of keeping hardware-style saves limited to cartridge-owned persistent state

## Open questions

- whether stronger hardware or oracle evidence should replace the current MBC3 curated-compatibility relatch rule
- whether stronger hardware or oracle evidence should change the current MBC3 reserved-selector policy for `0x04..=0x07`
- whether MBC3 RTC access spacing should remain advisory-only or later become an enforced `16`-T-cycle timing rule
- whether future MBC7 hardware evidence should add a documented rumble control route or EEPROM program busy timing; until then MBC7 exposes no runtime rumble
- whether a clearly scoped metadata rule can ever distinguish CGB-era `11`-character titles plus manufacturer code from valid `15`-character titles without truncating real software
- whether stronger MBC6 hardware or oracle evidence should replace the current immediate flash status-completion baseline with explicit erase/program latency
