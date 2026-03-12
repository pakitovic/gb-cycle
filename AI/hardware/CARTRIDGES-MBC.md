# CARTRIDGES / MBC

## Scope

Own cartridge ROM/RAM mapping, cartridge-header parsing, mapper state, battery-backed persistence interfaces, and mapper-specific features such as RTC or rumble.

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
  - `NoMbc`
  - `Mbc1`
  - `Mbc2`
  - `Mbc3`
  - `Mbc5`
  - `Unsupported` or `Other`
- `NoMbc` should be the non-banked family covering header codes `0x00`, `0x08`, and `0x09`, while preserving the raw header type for RAM/battery distinctions and diagnostics.
- The cartridge type must drive more than bank switching. It also defines whether the cartridge has external RAM, mapper-local RAM such as MBC2 internal RAM, battery-backed save state, RTC, rumble, or other mapper-local hardware.
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
- The same contract should cover cartridges with no RAM, ordinary external RAM, mapper-local RAM such as MBC2 internal RAM, banked RAM, RTC-mapped registers, and later extra cartridge-local hardware without requiring a new bus API.
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

## Persistence baseline

- Battery-backed persistence must be modeled as cartridge-owned state, not as "dump the currently visible `0xA000-0xBFFF` window."
- `0xA000-0xBFFF` is only a mapper-controlled access window on the shared T-cycle runtime. The persistible payload is the cartridge's full backing store, not whichever bank or register file happens to be visible on one access.
- The decision that a cartridge should produce hardware-style persistent state must come from header byte `0x0147`, not from filename heuristics, game title, or RAM-size guesses.
- Loaded cartridge metadata should expose explicit capability data such as `has_battery`, `has_rtc`, and a typed distinction between persistent RAM, persistent RTC, non-persistent RAM, and no persistible storage.
- Persistible RAM sizing must come from validated `0x0147 + 0x0149` capability data rather than from `0x0149` alone.
- `MBC2` is a required exception: its persistible store is the internal `512 x 4-bit` nibble array, and `0x0149` must not size it.
- Validation must reject or warn on header combinations where cartridge type, battery capability, RTC capability, and declared RAM size are incoherent before the save layer creates any hardware-style payload.
- `ram_enabled` or equivalent access gating must never decide whether RAM exists or whether it should be saved. Disabled RAM is still cartridge-owned state.
- `NoMbc` persistence covers at most one linear `8 KiB` RAM store when the validated header type actually provides battery-backed RAM, such as `0x09`.
- `MBC1` persistence must cover the full validated SRAM backing, not only the currently visible bank in `0xA000-0xBFFF`.
- `MBC2` persistence must cover all `512` logical nibbles, not an invented `8 KiB` byte array and not only the currently addressed echo window.
- `MBC3` persistence must cover the full validated SRAM backing plus live RTC state when the header type includes timer and battery support.
- `MBC5` persistence must cover the full validated SRAM backing up to `128 KiB`; rumble state is not part of the ordinary battery-backed save payload.
- For `MBC3`, the persistible RTC payload must preserve at least seconds, minutes, hours, visible `9`-bit day counter, halt, carry, and enough elapsed-time bookkeeping to reconstruct powered-off advancement on reload.
- The persistible RTC payload must reflect live RTC state, not the latched snapshot exposed for reads through the `0x00 -> 0x01` latch sequence.
- Hardware-style cartridge saves and full emulator save states are different systems. Cartridge persistence must not serialize CPU, PPU, APU, WRAM, HRAM, or other console-owned state.
- Cartridges without battery support should not produce automatic hardware-style save files unless the emulator explicitly exposes a non-hardware-faithful opt-in policy.
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
  - a reserved future `Mbc1Variant::Mbc1M` or equivalent, because multi-cart MBC1M uses a different bank-selection formula from standard MBC1
- Header codes `0x01`, `0x02`, and `0x03` should be recognized explicitly as the MBC1 family.
- `0x01` means MBC1 with no external RAM, `0x02` means MBC1 + RAM, and `0x03` means MBC1 + RAM + battery.
- Battery presence changes persistence expectations only; it must not change live banking behavior.
- MBC1 configuration must be validated from header metadata and real image size rather than accepted indiscriminately.
- Treat `32 KiB` external RAM as the standard small-ROM wiring case.
- Treat `1 MiB` or `2 MiB` ROM as the alternate large-ROM wiring case, not as banked-`32 KiB` RAM cartridges.
- If declared ROM size, RAM size, real file size, and derived MBC1 wiring disagree, report an explicit diagnostic under the chosen validation policy instead of guessing silently.
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
- Keep standard MBC1 and future MBC1M formulas distinct rather than mixing them behind ad hoc conditionals.
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
- Reserve an explicit variant or flag for future MBC1M support instead of letting the first standard implementation accrete scattered special cases.

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
- The implementation must not collapse "RAM bank `0x00..=0x07`" and "RTC register `0x08..=0x0C`" into one undifferentiated bank number. They share an address window, not a meaning.
- The RTC must keep its own live state separate from any external-RAM backing and separate from bus-owned state; it is not a bus patch layered on top of SRAM.
- MBC3 should keep explicit live state for at least `ram_rtc_enabled`, a raw `7`-bit ROM-bank register such as `rom_bank`, an explicit `ram_or_rtc_select`, RTC live state, RTC latched snapshot state, latch-edge detection for the `0x00 -> 0x01` command sequence, battery / RTC capability flags, and header-derived metadata.
- Header codes `0x0F`, `0x10`, `0x11`, `0x12`, and `0x13` should be recognized explicitly as the MBC3 family.
- `0x0F` means MBC3 + timer + battery, `0x10` means MBC3 + timer + RAM + battery, `0x11` means plain MBC3, `0x12` means MBC3 + RAM, and `0x13` means MBC3 + RAM + battery.
- Header-derived capability state must distinguish plain MBC3 from RTC-backed MBC3 cartridges; not every MBC3 cartridge has timer hardware.
- Reserve an explicit future variant such as `Mbc3Variant::Mbc30` or equivalent instead of letting standard MBC3 quietly absorb the later MBC30 differences.
- Standard MBC3 must support up to `2 MiB` ROM, meaning up to `128` total `16 KiB` ROM banks with bank `0` fixed in `0x0000-0x3FFF` and banks `0x01..=0x7F` visible in `0x4000-0x7FFF`.
- Standard MBC3 external RAM support should stop at `32 KiB`; if header metadata implies the later `64 KiB` SRAM configuration associated with MBC30, keep that as explicit future validation / variant work rather than silently treating it as ordinary MBC3.
- Unlike standard MBC1, MBC3 high-region ROM banking must allow banks `0x20`, `0x40`, and `0x60` to be selected normally when present.
- The visible MBC3 memory map should be:
  - `0x0000-0x3FFF`: fixed ROM bank `0`
  - `0x4000-0x7FFF`: switchable ROM bank `0x01..=0x7F`
  - `0xA000-0xBFFF`: external RAM bank `0x00..=0x07` or RTC register `0x08..=0x0C`, depending on the current selector
- The bus should still delegate `0xA000-0xBFFF` completely to the cartridge device; it must not infer from the address alone whether the active target is SRAM or RTC.
- `0x0000-0x1FFF` is a write-only RAM / RTC-enable register. Any write whose low nibble is `0xA` enables both external RAM and RTC-register access; other values disable both.
- With RAM / RTC disabled, `0xA000-0xBFFF` reads and writes should follow one explicit project policy rather than accidental backing-store behavior. A default of `0xFF` is acceptable, but the policy should remain explicit and testable.
- `0x2000-0x3FFF` is a write-only raw `7`-bit ROM-bank register. Store `value & 0x7F`, ignore upper data bits, apply the documented `0 -> 1` translation for the switchable ROM window, and then mask the effective bank by the real number of loaded ROM banks.
- `0x4000-0x5FFF` is a write-only selector for the `0xA000-0xBFFF` window. Values `0x00..=0x07` select RAM banks, while `0x08..=0x0C` select RTC registers.
- Represent the `0x4000-0x5FFF` selector as an explicit mapping target such as `RamBank(u8)` versus `RtcRegister(RtcRegisterId)` instead of as one raw numeric field whose meaning is reconstructed ad hoc during each access.
- External RAM banking must be masked by the real number of available RAM banks declared by validated cartridge metadata; standard MBC3 should not silently treat a `64 KiB` RAM declaration as ordinary banked SRAM support.
- `0x6000-0x7FFF` is a write-only RTC latch command register. Latch only on the logical edge formed by writing `0x00` and then `0x01`; writing `0x01` without the preceding `0x00` must not refresh the snapshot.
- Keep RTC live state and RTC latched state as separate concepts. RTC register reads should come from the latched snapshot, while RTC register writes should target the live RTC state.
- The visible RTC register file must include seconds, minutes, hours, day low, and day high / flags.
- Seconds and minutes should stay within `0..=59`, hours within `0..=23`, and the visible day counter within `0..=511`.
- Day-counter state should be modeled as a `9`-bit value split across `DL` and `DH.bit0`.
- `DH.bit6` is the halt flag, and `DH.bit7` is the carry flag.
- When the live day counter overflows past `511`, it must wrap within the visible `9`-bit range and set the carry flag.
- The carry flag must remain set until software clears it through RTC register writes; it must not auto-clear merely because the clock continues to advance.
- `halt = 1` must stop progression of the live RTC state.
- Pan Docs' recommendation to set `halt` before writing RTC registers should be documented as a hardware-usage rule, but the emulator does not need to reject out-of-flow writes unless later hardware evidence demands that restriction.
- When the selector targets `0x08..=0x0C`, writes to `0xA000-0xBFFF` must update the live RTC register state, not the latched snapshot.
- MBC3 control writes are ordinary cartridge commands on the shared T-cycle timeline. Changes to ROM bank, RAM bank, RTC selector, RAM / RTC enable, and latch state must become visible on the access T-cycle for all later cartridge accesses; do not defer them to instruction or frame boundaries.
- Treat MBC3 bus-visible ordering as T-cycle based even though the RTC itself is driven by a `32.768 kHz` external oscillator in hardware. The long-term RTC progression should come from an injected time / persistence source, not from blindly counting executed CPU T-cycles as if the RTC were just another divider.
- The RTC design should separate three layers explicitly: visible RTC registers, live RTC counter state, and emulator-provided time / persistence infrastructure.
- Battery-backed persistence policy should cover RTC state as well as external RAM where the header declares battery support, while the save backend remains responsible only for storage and time-source integration rather than visible bus semantics.
- The RTC path must support a deterministic injected or simulated time source for tests; unit and ROM tests must not depend on the host wall clock.
- Pan Docs recommends leaving roughly `4 us` between separate RTC register accesses. For this project's timing vocabulary, that is `16` T-cycles at normal-speed DMG (`4` M-cycles). Document it as a current research / validation note rather than as an already enforced bus restriction unless the implementation closes that accuracy point explicitly.
- A concrete `Mbc3Cartridge` implementing `CartridgeDevice` is the intended implementation shape for this repo.
- It should contain at least `rom`, optional `ram`, `has_battery`, `has_rtc`, `ram_rtc_enabled`, `rom_bank`, `ram_or_rtc_select`, `rtc_live`, `rtc_latched`, latch-sequence state, and `header`.
- Prefer explicit helpers such as `effective_rom_bank()`, `effective_ram_bank()`, `current_a000_mapping()`, and `latch_rtc_if_needed()` so raw register state, target-selection state, live RTC state, latched RTC state, and final effective mapping remain inspectable.
- The first MBC3 implementation should close standard MBC3 before attempting MBC30. Leave the extension point in the cartridge factory and bank-resolution code, but do not bury MBC30 behind loose conditionals inside normal MBC3 behavior.

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
- MBC5 external RAM support should cover ordinary `8 KiB`, `32 KiB`, and `128 KiB` SRAM configurations, meaning up to `16` visible `8 KiB` RAM banks.
- MBC5 external RAM should be modeled as linear `8 KiB` banks selected directly by the RAM-bank register, with no MBC1-style dual banking mode.
- If a cartridge declares an MBC5 header type but validated ROM size exceeds `8 MiB`, the loader should emit an explicit diagnostic instead of guessing another mapper.
- If a cartridge declares an MBC5 header type with impossible RAM metadata, such as RAM omitted by `0x0147` while `0x0149 != 0x00`, or a declared MBC5 RAM size larger than `128 KiB`, the loader should emit an explicit diagnostic under the chosen validation policy.
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
- Unlike MBC1, MBC2, and MBC3, MBC5 must not apply a `0 -> 1` translation to the high ROM window. Writing bank `0` should really expose bank `0` in `0x4000-0x7FFF`.
- Effective MBC5 ROM-bank selection should combine `rom_bank_low8` plus `rom_bank_high1` into one `9`-bit value and then mask by the real number of loaded ROM banks without inventing a synthetic `0 -> 1` rule.
- Do not reuse MBC1 or MBC3 helper paths if they carry the `0 -> 1` rule, because that would make valid MBC5 high-window bank `0` unreachable.
- `0x4000-0x5FFF` is a write-only RAM-bank / rumble control register.
- On standard non-rumble MBC5, use the low `4` bits as the raw RAM-bank register and then mask by the real RAM-bank count.
- On rumble-capable MBC5, `bit 3` of that control register should update `rumble_on`, while the remaining RAM-bank-relevant bits should still resolve the effective RAM bank according to the validated cartridge wiring.
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

- header parser tests for `entry_point`, `0x0143`, `0x0146`, `0x0147`, `0x0148`, and `0x0149`
- tests for standard `0x0148` ROM-size decoding and explicit handling of `0x52`, `0x53`, and `0x54`
- tests for `0x0149` RAM-size decoding, including the `MBC2` special case where internal RAM is not described by the ordinary RAM-size table
- tests for explicit diagnostics on unknown cartridge types and size mismatches
- explicit No MBC tests for `0x00`, `0x08`, and `0x09`, including warning-only handling for rare RAM variants
- tests that No MBC enforces or diagnoses `32 KiB` ROM and at most linear `8 KiB` RAM instead of silently accepting banked expectations
- tests that No MBC `0x0000-0x7FFF` reads are linear and bankless, and that `0x0100-0x014F` remains visible through ordinary reads after boot handoff
- tests that No MBC ROM-space writes are ignored without mutating ROM or mapper state, while still traveling through the normal cartridge command path
- tests that No MBC `0xA000-0xBFFF` behavior distinguishes absent RAM from present linear `8 KiB` RAM and that battery only changes persistence policy
- explicit MBC1 tests for `0x01`, `0x02`, and `0x03`, deterministic power-up state, register writes to each control range, and immediate visibility of mapper writes to later accesses
- tests that MBC1 reproduces raw-register `0 -> 1`, dedicated access to banks `0x01`, `0x1F`, `0x21`, `0x41`, and `0x61`, the documented small-ROM case where bank `0` becomes visible in `0x4000-0x7FFF` after size masking, and the large-ROM `0x20` / `0x40` / `0x60` to `0x21` / `0x41` / `0x61` anomaly
- tests that MBC1 distinguishes standard `32 KiB` banked-RAM wiring from large-ROM alternate wiring, including mode `0` versus mode `1`, disabled-RAM open-bus reads and ignored writes, fixed `8 KiB` RAM, banked `32 KiB` RAM, and explicit diagnostics for impossible ROM/RAM/wiring combinations
- explicit MBC2 tests for `0x05` and `0x06`, deterministic power-up state, address-bit-`8` control decode, immediate visibility of RAM-enable and ROM-bank writes, and the documented `0 -> 1` behavior for the `4`-bit ROM-bank register
- tests that MBC2 enforces its `256 KiB` ROM limit, keeps `0x0149` as a validation-only special case rather than external-SRAM sizing, and reports explicit diagnostics when ROM size or RAM-size metadata are inconsistent with MBC2 rules
- tests that MBC2 models one logical `512 x 4-bit` internal RAM array with low-`9`-bit echo aliasing across `0xA000-0xBFFF`, stores only the low nibble on writes, uses the documented repo high-nibble read policy explicitly, ignores writes while RAM is disabled, and persists that internal RAM only for `0x06`
- explicit MBC3 tests for `0x0F`, `0x10`, `0x11`, `0x12`, and `0x13`, deterministic power-up state, immediate visibility of RAM / RTC-enable and selector writes, and the documented `0 -> 1` behavior for the raw `7`-bit ROM-bank register
- tests that MBC3 supports up to `2 MiB` ROM, preserves access to banks `0x20`, `0x40`, and `0x60`, masks effective ROM and RAM banks by real cartridge size, and reserves or diagnoses MBC30-like `64 KiB` SRAM declarations explicitly
- tests that MBC3 distinguishes external RAM-bank selection from RTC-register selection in `0xA000-0xBFFF`, routes reads through the correct target, and keeps disabled RAM / RTC behavior under one explicit project policy
- tests that MBC3 latches RTC state only on the `0x00 -> 0x01` sequence, keeps snapshots stable across multiple reads while live RTC state can advance independently, and routes RTC writes to live state rather than to the latched copy
- tests that MBC3 implements seconds, minutes, hours, day low, and day high / flags correctly, including writes to `DH.bit0`, `DH.bit6`, and `DH.bit7`, sticky carry on day overflow, and `halt` freezing live RTC advancement
- tests that MBC3 RTC / persistence can run from an injected deterministic time source, including elapsed time across powered-off sessions without coupling the expected result to host wall-clock timing during tests
- if fine RTC-access delay emulation is implemented, tests that the chosen `16`-T-cycle access-spacing policy matches the documented model; until then, base MBC3 tests should not assume that fine delay is enforced
- explicit MBC5 tests for `0x19`, `0x1A`, `0x1B`, `0x1C`, `0x1D`, and `0x1E`, deterministic power-up state, immediate visibility of RAM-enable and bank-register writes, and explicit preservation of bank `0` in the switchable ROM window
- tests that MBC5 supports up to `8 MiB` ROM with full `9`-bit bank selection, including bank `0x1FF`, masks effective ROM and RAM banks by real cartridge size, and does not apply an MBC1/MBC3-style `0 -> 1` translation
- tests that MBC5 RAM banking covers the documented `8 KiB`, `32 KiB`, and `128 KiB` SRAM cases, respects disabled-RAM policy, uses linear `8 KiB` banks with no MBC1-style dual banking mode, and does not expose SRAM on header variants that do not actually provide RAM
- tests that rumble-capable MBC5 distinguishes effective RAM-bank selection from `rumble_on`, that `bit 3` of the `0x4000-0x5FFF` control register keeps the motor on until software clears it, and that rumble state is observable without moving that responsibility into the bus or frontend
- tests that MBC5 validation reports clear diagnostics for ROM sizes above `8 MiB`, impossible RAM declarations, and rumble-capable header types loaded without an observable rumble state
- tests that hardware-style persistence round-trips the complete cartridge backing store rather than the currently visible `0xA000-0xBFFF` window, including linear SRAM on `NoMbc`, banked SRAM on `MBC1`, `MBC3`, and `MBC5`, plus nibble RAM on `MBC2`
- tests that persistence eligibility comes from `0x0147` capability decoding, that `ram_enabled` does not gate save contents, and that non-battery cartridges do not auto-produce hardware-style saves by default
- tests that `MBC3` persistence serializes live RTC state plus elapsed-time bookkeeping, restores powered-off advancement from an injected clock, and does not confuse the latched RTC snapshot with the persistible live clock
- tests that save-backend versioning, in-memory adapters, disk adapters, manual save, save-on-close, optional auto-flush, and atomic replacement behavior are covered at the contract layer rather than through bus-side file I/O
- mapper-specific ROM tests
- cartridge RAM persistence behavior tests
- additional cross-session RTC persistence and integration tests once save/time-source plumbing is active
- tests that document startup behavior for cartridge RAM, whether external or mapper-local, when direct-boot presets bypass firmware execution
- tests that fixed-ROM, switchable-ROM, and external cartridge ranges are delegated through the cartridge interface rather than treated as internal console memory
- tests that ROM-space writes hit MBC control semantics instead of fake writable ROM
- tests that boot and post-boot paths both observe `0x0100-0x014F` through the ordinary cartridge device rather than through a shadow header copy

## Implementation notes for this repo

- Prefer one typed `CartridgeHeader` plus decoded capability fields over raw-code lookups scattered throughout the codebase.
- Keep `0x0147`, `0x0148`, and `0x0149` decoding centralized in the cartridge loader instead of reinterpreting them inside the bus, boot code, or frontends.
- Preserve `cgb_flag` and `sgb_flag` now even if the current core is still DMG-only.
- A `CartridgeKind` plus device factory is a good fit for early bring-up, as long as unsupported raw type codes remain reportable.
- Model No MBC as its own concrete device, not as a generic blob-reader fallback.
- Keep mapper traits or enums narrow and explicit.
- Avoid hard-coding cartridge logic into generic bus code.
- Keep explicit cartridge capability data for battery-backed RAM, RTC, and non-persistent RAM instead of re-deriving persistence policy from ad hoc conditions later.
- Keep the typed persistence contract attached to the cartridge layer, and make its payload describe full cartridge-owned backing stores rather than bus-visible windows.
- Treat cartridge RAM power-up contents, whether external or mapper-local, as separate from deterministic post-boot CPU/MMIO state; if the emulator chooses a direct-boot initialization policy, keep it explicit and configurable.
- Keep No MBC absent-RAM behavior under an explicit and configurable policy instead of accidental zero-backed memory.
- Keep active-ROM-bank selection, RAM enable, RAM banking, RTC mapping, and any bank-wrap quirks inside cartridge/MBC implementations rather than generic bus region logic.
- Keep header validation policy explicit and centralized rather than hiding it inside individual mapper constructors.
- For MBC1, keep raw register fields, resolved effective bank helpers, and validated wiring / variant metadata as separate layers instead of one opaque "current bank" state blob.
- For MBC1, derive an explicit standard-versus-large-ROM wiring classification, plus a reserved future MBC1M variant, from validated cartridge metadata instead of branching ad hoc during each access.
- Keep disabled external-RAM open-bus behavior explicit and configurable enough that tests can pin the chosen policy.
- For MBC2, keep address-bit-`8` control decode, raw `rom_bank_low4`, internal nibble RAM organization, effective ROM-bank helpers, and explicit disabled-RAM / high-nibble readback policies as separate layers instead of hiding them in generic byte-RAM helpers.
- For MBC2, treat `0x0149` as validation metadata only; runtime RAM capacity comes from the mapper family itself rather than from the ordinary external-RAM table.
- For MBC3, derive `has_rtc` from the header type itself, not from battery presence or RAM presence alone.
- For MBC3, keep `ram_or_rtc_select` as a typed mapping target rather than one raw bank number whose meaning changes implicitly.
- For MBC3, keep `rtc_live` and `rtc_latched` as separate state objects, and route RTC reads versus writes intentionally.
- For MBC3, keep the visible RTC model inside the cartridge device while the save / persistence layer only owns serialization, storage, and time-source integration.
- For MBC3, persist live RTC state plus elapsed-time bookkeeping, not the latched snapshot shown through the read latch.
- For MBC3, restate the optional RTC access-spacing note in T-cycles when it becomes behaviorally relevant: the Pan Docs `4 us` recommendation corresponds to `16` T-cycles at normal-speed DMG.
- For MBC3, reserve MBC30 as a first-class future extension point rather than folding it into standard MBC3 conditionals.
- For MBC5, keep the raw low `8` ROM-bank bits and raw high `1` ROM-bank bit separate instead of flattening them into one opaque field too early.
- For MBC5, remember that bank `0` is valid in the switchable ROM window; do not cargo-cult the MBC1/MBC3 `0 -> 1` rule here.
- For MBC5, keep `ram_bank_raw`, `effective_ram_bank()`, and `rumble_on` distinct instead of pretending the rumble-capable RAM-bank register is identical to standard non-rumble MBC5.
- For MBC5, keep rumble state out of the ordinary battery-backed save payload unless the project intentionally adds a separate transient-state feature beyond normal cartridge persistence.
- Keep save flush policy, file replacement strategy, and path naming out of the bus; they belong to the persistence backend around the cartridge contract.
- For MBC5, emit explicit validation diagnostics when `0x0147`, `0x0148`, and `0x0149` describe an impossible MBC5 combination instead of silently coercing the cartridge into a nearby supported shape.

## Known pitfalls

- treating the cartridge as a raw ROM blob with ad hoc mapper `if` statements
- treating No MBC as too trivial to deserve a real cartridge device
- leaking mapper knowledge into unrelated modules
- under-designing the cartridge boundary so later MBCs become invasive
- silently zeroing cartridge RAM during direct boot and then treating that as hardware-accurate startup behavior
- teaching the generic bus how a specific MBC banks ROM or RAM instead of delegating that behavior to the cartridge subsystem
- dumping the currently visible `0xA000-0xBFFF` contents as if that were always the full save payload
- deciding save eligibility from `ram_enabled`, filename heuristics, or `0x0149` alone instead of validated cartridge capabilities
- inferring the mapper from ROM size or other heuristics instead of using `0x0147`
- adding fake active-bank or latch state to No MBC
- using `0x0149` alone to decide whether external RAM exists
- modeling `MBC2` RAM as if it were ordinary banked SRAM
- dropping `cgb_flag` or `sgb_flag` because they are not immediately used by the DMG baseline
- silently accepting No MBC headers that declare more than `32 KiB` ROM or more than `8 KiB` RAM without diagnostics
- silently coercing unsupported or inconsistent headers into a nearby supported configuration
- collapsing MBC1 to one `active_rom_bank` and losing the raw register semantics that drive its quirks
- applying the MBC1 `0 -> 1` rule after final ROM-size masking instead of on the raw `5`-bit primary register field
- assuming bank `0` can never appear in `0x4000-0x7FFF` on small MBC1 ROMs
- treating all MBC1 cartridges as if the secondary register always means the same thing regardless of wiring and banking mode
- folding future MBC1M behavior into standard MBC1 bank math with scattered conditionals
- modeling MBC2 as "MBC1 with fewer bits" instead of as its own mapper with address-bit-`8` control decode
- splitting MBC2 control writes into arbitrary `0x0000-0x1FFF` / `0x2000-0x3FFF` subranges instead of decoding address bit `8`
- modeling MBC2 RAM as ordinary byte-wide `8 KiB` SRAM instead of one `512 x 4-bit` internal array with echoes
- forgetting that `0xA200-0xBFFF` aliases the same MBC2 RAM cells as `0xA000-0xA1FF`
- letting MBC2 high-nibble readback or disabled-RAM behavior emerge accidentally from host memory instead of one explicit project policy
- modeling MBC3 as if it were just MBC1 plus a decorative clock register
- treating MBC3 RAM-bank values `0x00..=0x07` and RTC-register values `0x08..=0x0C` as one interchangeable selector namespace
- reading MBC3 RTC registers from live state instead of the latched snapshot
- writing MBC3 RTC register updates into the latched snapshot instead of the live RTC state
- deriving MBC3 RTC progression directly from executed CPU cycles instead of from an explicit time / persistence source
- persisting the latched MBC3 RTC snapshot as if it were the live battery-backed clock
- assuming MBC3 cannot select ROM banks `0x20`, `0x40`, or `0x60` because MBC1 cannot
- silently treating `64 KiB` SRAM declarations as ordinary standard MBC3 instead of reserving MBC30 explicitly
- deferring MBC3 bank, selector, enable, or latch effects until instruction end instead of applying them on the access T-cycle
- applying the MBC1/MBC3 `0 -> 1` bank rule to MBC5 and accidentally making bank `0` unreachable in `0x4000-0x7FFF`
- collapsing MBC5's `9`-bit ROM-bank register into one lossy `8`-bit field and losing banks above `0xFF`
- silently treating rumble-capable MBC5 header types as if their RAM-bank register were identical to standard non-rumble MBC5
- loading `0x1C`, `0x1D`, or `0x1E` as plain non-rumble MBC5 and thereby hiding cartridge-local rumble state from the rest of the system
- persisting `rumble_on` or full-console state inside the cartridge save payload instead of keeping hardware-style saves limited to cartridge-owned persistent state

## Open questions

- enum-based versus trait-based mapper organization for this codebase
- which validation mode should be the default for interactive use versus automated test runs
- which explicit default policy should govern MBC3 RAM / RTC-disabled reads and writes at `0xA000-0xBFFF`
- whether the Pan Docs RTC access-spacing recommendation should remain documented-only or later become an enforced `16`-T-cycle timing rule
- what persisted RTC serialization shape best separates visible RTC state, elapsed-time bookkeeping, and frontend-specific storage adapters
- which default flush policy should be enabled for hardware-style saves: close-only, manual plus close, or optional auto-flush after persistible writes
