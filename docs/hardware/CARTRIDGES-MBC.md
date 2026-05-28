# CARTRIDGES / MBC

## Scope

Own cartridge-header parsing, cartridge classification and admission, cartridge-visible bus behavior for `0x0000-0x7FFF` and `0xA000-0xBFFF`, mapper runtime state, cartridge-local hardware such as RTC, rumble, sensors, EEPROM, flash, camera registers, and the typed persistence payloads consumed by the save layer.

Do not own boot-ROM overlay routing, generic bus arbitration, frontend file paths, host camera APIs, host rumble APIs, whole-machine save-state orchestration, or ROM-suite operations. Boot behavior lives in [`BOOT-ROM.md`](BOOT-ROM.md), bus access policy in [`BUS.md`](BUS.md), Pocket Camera frontend boundaries in [`GAME-BOY-CAMERA.md`](GAME-BOY-CAMERA.md), architecture and compatibility-policy shape in [`../ARCHITECTURE.md`](../ARCHITECTURE.md), test policy in [`../TESTING.md`](../TESTING.md), ROM-suite mechanics in [`../info/ROM-SUITES.md`](../info/ROM-SUITES.md), source ordering in [`../REFERENCES.md`](../REFERENCES.md), and open follow-up in [`../TODO.md`](../TODO.md) or [`../ROADMAP.md`](../ROADMAP.md).

## Design rule

A cartridge is an external bus device, not a ROM byte slice plus scattered mapper conditionals. The bus should route cartridge-owned address ranges to one typed cartridge device, and the device should own mapper commands, final bank selection, external-RAM apertures, cartridge-local registers, persistence metadata, and debug/trace summaries.

ROM-space writes are cartridge commands on the shared T-cycle timeline. They must never mutate ROM bytes, be deferred to instruction end, or be reinterpreted by the bus after the cartridge device has been selected.

## Header and loader contract

- The header at `0x0100-0x014F` is the source of truth for the base hardware declaration: entry point, logo, title bytes, preserved `0x013F-0x0142` suffix/manufacturer bytes, `cgb_flag`, licensee bytes, `sgb_flag`, `cartridge_type`, ROM size, RAM size, destination, old licensee, and checksum.
- Preserve raw metadata for diagnostics and future CGB/SGB compatibility work. Do not infer mapper behavior from filename, ROM size, title text, or frontend hints when `0x0147` already declares a type.
- Keep CGB-era title decoding conservative: when `0x0143.bit7` is set, preserve the ambiguous manufacturer/title-suffix bytes separately instead of guessing the newer `11`-character split from ASCII heuristics.
- Loader flow is `parse header -> classify raw type/signature/heuristic -> validate declared metadata -> apply compatibility policy -> construct one supported device or return a typed rejection`.
- Classification must preserve raw type, detected name, category, and reason so frontends and logs can explain both accepted special cases and rejected unsupported hardware.

## Compatibility policy

- `Strict` is the oracle/CI mode. It loads supported cartridges with validated metadata, rejects contradictions, disables heuristics, and must not hide manual overrides.
- `Permissive` may degrade unambiguous supported-hardware metadata problems to warnings, but runtime mapper behavior must stay identical to `Strict` for admitted hardware.
- `Experimental` may enable explicitly marked heuristic or partial paths, but results are non-oracle and must remain visible in diagnostics, save-state metadata, and replay/debug context.
- Execution mode changes admission, validation severity, heuristic enablement, overrides, and diagnostics. It must not change T-cycle-visible semantics for already supported cartridge families.
- Current narrow permissive cases are intentional: `NoMbc` legacy RAM-size mismatches can warn while keeping the fixed `8 KiB` RAM baseline, and explicit `MBC5` images with malformed size metadata can use an effective rounded ROM capacity padded with `0xFF` when the actual image still fits the supported MBC5 range.

## Supported and unsupported categories

| Category | Meaning |
| --- | --- |
| `Supported` | A runtime device exists and owns all cartridge-visible behavior for the declared/signed hardware. |
| `PlannedVariant` | Known shape reserved for a future explicit implementation; do not run through a nearby mapper. |
| `DocumentedButUnsupported` | Documented hardware without current runtime support. |
| `ExperimentalHeuristic` | Opt-in research classification such as broad `EMS`, `Bung`, or `Wisdom Tree` detection. |
| `AccessorySpecialCase` | Hardware requiring a dedicated accessory/device model, such as `Bandai TAMA5` until implemented. |
| `UnknownCode` | Raw type is not recognized; report it and stop. |

Supported runtime families are `NoMbc`, `Mmm01`, `M161`, `Huc1`, `Huc3`, `Mbc1`, `Mbc2`, `Mbc3`, `Mbc5`, `Mbc6`, `Mbc7`, and `PocketCamera`. Signature-backed variants such as `MBC1M`, `MBC30`, later-Mani `MMM01`, and known `M161` enter through explicit classifier paths, not broad guesses.

## Address contract

| Range | Cartridge-owned meaning |
| --- | --- |
| `0x0000-0x3FFF` | Fixed or mapper-defined low ROM window, plus mapper commands on writes where the family defines them. |
| `0x4000-0x7FFF` | Switchable ROM/flash window, or additional mapper-command ranges depending on family. |
| `0xA000-0xBFFF` | External RAM, mapper-local RAM, RTC registers, EEPROM/sensor registers, camera registers, flash apertures, or absent/open behavior depending on the active cartridge state. |

Every cartridge access remains ordered on the shared T-cycle timeline. Header parsing is load-time configuration, but post-boot reads of `0x0100-0x014F` must still come from the live cartridge ROM device after boot-ROM handoff.

## Core mapper families

- `NoMbc` covers header codes `0x00`, `0x08`, and `0x09`. Runtime is linear `32 KiB` ROM, optional fixed `8 KiB` RAM for `0x08`/`0x09`, ignored ROM-space writes, no mapper registers, and battery affecting persistence only.
- `MBC1` covers `0x01..=0x03` and keeps raw `rom_bank_low5`, `secondary_bank`, `banking_mode`, standard versus large-ROM wiring, RAM enable, and explicit effective-bank helpers. Apply the primary `0 -> 1` rule before final masking, and do not collapse the low/high ROM windows or RAM-bank path into one opaque active bank.
- `MBC1M` is a supported signature-backed MBC1-family variant for `1 MiB` multicarts with repeated valid subheaders. It uses explicit variant logic and must not be inferred from a generic multicart guess.
- `MBC2` covers `0x05` and `0x06`. It owns a `4`-bit ROM-bank register selected by address bit `8`, internal `512 x 4-bit` RAM, echoing over `0xA000-0xBFFF`, explicit high-nibble readback, and persistence as nibbles rather than byte-wide SRAM.
- `MBC3` covers `0x0F..=0x13`. It owns RAM/RTC enable, raw `7`-bit ROM bank, typed RAM/reserved/RTC selector state, live RTC state, latched RTC snapshot, and latch-edge tracking. Standard MBC3 keeps selector values `0x04..=0x07` reserved unless stronger evidence changes the policy.
- `MBC30` is a supported `MBC3`-family variant selected by RAM-bearing MBC3 headers with `64 KiB` SRAM. It extends ROM banking to `8` bits, allows selectors `0x00..=0x07` as SRAM banks, preserves RTC selectors `0x08..=0x0C`, and persists through the MBC3 RAM/RTC payload family with the larger RAM length.
- `MBC5` covers `0x19..=0x1E`. It owns raw low `8` and high `1` ROM-bank fields, valid switchable-window bank `0`, RAM-enable state, linear RAM banks, and optional rumble where `bit3` of the RAM-bank/rumble register is motor state and the remaining bank bits select RAM.

## Special cartridge families

- `MMM01` covers header codes `0x0B..=0x0D` plus the narrow later-Mani trailing-menu signature path. It owns unmapped versus mapped mode, menu-startup mapping to the last `32 KiB`, game-select masks, mapped-mode write restrictions, and optional battery-backed RAM instead of borrowing ordinary MBC1 state.
- `M161` is a supported signature-backed multicart path for the known Mani `4-in-1` shape. It switches whole `32 KiB` ROM banks with a one-time latch where the first ROM-space write selects bank bits `0..=2` until power-off, and it has no external RAM payload.
- `HuC1` is a supported mapper family for `0xFF`, not MBC1-with-IR. It owns RAM mode versus IR mode, a `6`-bit ROM-bank register, a `2`-bit RAM-bank register, no `0 -> 1` ROM-bank translation, mirrored small RAM payloads where applicable, and the documented IR transmitter/readback baseline.
- `HuC-3` is a supported mapper family for `0xFE`, not an MBC3 derivative. It owns a literal `7`-bit ROM-bank register where bank `0` is valid in the high window, banked RAM, select modes for RAM/RTC/IR/invalid states, mailbox/semaphore state, a `256`-nibble MCU window, synchronous documented RTC commands, and dedicated persistence for RAM plus MCU/RTC state.
- `MBC6` is supported for `0x20` and targets the documented CGB-era `1 MiB` ROM / `32 KiB` SRAM shape. It owns two independent `8 KiB` ROM/flash windows, two independent `4 KiB` SRAM windows, main flash, hidden flash, sector-0 protection, flash command decode, program buffers, and typed persistence for SRAM, main flash, hidden flash, and protection state.
- `MBC7` is supported for `0x22` and owns a `7`-bit ROM-bank register, two enable gates, accelerometer latch/read registers, a serial `93LC56`-style EEPROM protocol, and a raw `256`-byte EEPROM payload. The historical header name contains `RUMBLE`, but runtime rumble remains disabled until hardware evidence identifies a control route.
- `Pocket Camera` is supported for `0xFC` as dedicated cartridge-local hardware with camera registers, capture timing, `128 KiB` SRAM, host-frame seam, and persistence owned by the cartridge subsystem. The detailed contract lives in [`GAME-BOY-CAMERA.md`](GAME-BOY-CAMERA.md).
- `Bandai TAMA5` remains `AccessorySpecialCase` for `0xFD` until a dedicated device model exists. Do not run it as an approximate supported mapper.

## Heuristic and fallback policy

- Broad `EMS`, `Bung`, `Wisdom Tree`, or future multicart heuristics are disabled outside explicit `Experimental` policy unless they have become a supported signature-backed path.
- Internal code sharing is allowed only after classification selected the correct explicit family or variant. Code reuse must not become silent fallback.
- Never coerce `HuC1` to `MBC1`, `HuC-3` to `MBC3`, `MBC6` to `MBC3`/`MBC5`, `MBC7` to `MBC5`, `Pocket Camera` to an ordinary MBC, or `Bandai TAMA5` to any currently supported family.

## Persistence and external save boundaries

- Cartridge persistence stores full cartridge-owned backing state, not the currently visible `0xA000-0xBFFF` window.
- Hardware-style cartridge saves and whole-machine save states are different systems. Cartridge persistence must not serialize CPU, PPU, APU, WRAM, HRAM, scheduler state, or frontend state.
- Save eligibility comes from validated cartridge capability metadata such as battery, RTC, EEPROM, flash, and persistence profile; `ram_enabled` decides access, not existence or save eligibility.
- Persisted payloads currently cover `NoMbcRam`, `Mmm01Ram`, `Huc1Ram`, `Huc3` RAM/MCU/RTC state, `Mbc1Ram`, `Mbc2Ram`, `Mbc3Rtc`, `Mbc3Ram`, `Mbc3RamRtc`, `Mbc5Ram`, `Mbc6` RAM/flash/hidden/protection state, `Mbc7Eeprom`, and `PocketCameraRam`.
- MBC3 persistence stores live RTC state, not the latched read snapshot. Restoring persistence should refresh live RTC and clear runtime-local latch/edge/advisory state so stale snapshots do not survive reload.
- MBC6 hardware-style persistence must remain lossless for SRAM, main flash, hidden flash, and sector-0 protection. External `.sav` interchange is narrower and may represent only `SRAM || main flash` when hidden/protection state is default.
- External `.sav` import/export is a host-side conversion boundary. Linear RAM cartridges use raw bytes, MBC3 RAM/RTC uses the supported RTC suffix shapes, MBC2 accepts both common nibble layouts and exports the packed layout, MBC7 exports/imports raw EEPROM bytes, and unsupported/lossy mapper profiles must fail explicitly instead of dropping state.
- The persistence backend owns serialization, versioning, host paths, atomic replacement, time-source integration, and flush policy. The cartridge only exposes the typed payload and live persistence metadata.

## Timing and RTC policy

- Mapper writes, RAM-enable changes, bank changes, RTC selectors, flash commands, EEPROM pin changes, and camera register writes become visible in access order on the T-cycle that performs the bus transaction.
- MBC3 RTC progression is cartridge-owned wall-clock/RTC-domain state. The `16`-T-cycle RTC access-spacing rule remains advisory through explicit state until stronger evidence makes it an enforced failure mode.
- HuC-3 RTC protocol state is cartridge-local mailbox/MCU state, not MBC3 register reuse.
- MBC6 flash operations currently complete synchronously and expose done status immediately after the command-side effect. Treat this as the current baseline, not a claim about real erase/program latency.
- Pocket Camera capture busy timing is cartridge-local and documented in [`GAME-BOY-CAMERA.md`](GAME-BOY-CAMERA.md); live host camera acquisition remains outside core timing.

## Validation

- Keep focused Rust tests near the owning cartridge device for header parsing, classification, validation diagnostics, bank math, command side effects, persistence payloads, restore errors, and save conversion boundaries.
- Keep external ROM coverage and report mechanics in [`../TESTING.md`](../TESTING.md) and [`../info/ROM-SUITES.md`](../info/ROM-SUITES.md); this handbook should not duplicate the full test matrix.
- Required cartridge coverage: no fallback for unsupported/special types, T-cycle access ordering, boot/post-boot header visibility through cartridge ROM, disabled/absent RAM behavior, battery/persistence eligibility, mapper-specific bank registers, RTC/live-latch behavior, flash/EEPROM state, and explicit diagnostics for `Strict`, `Permissive`, and `Experimental` paths.
- Retain focused coverage for narrow compatibility choices: MBC3 follow-up relatch behavior, standard MBC3 reserved selectors `0x04..=0x07`, advisory RTC access spacing, MBC5 permissive size rounding, MBC6 representable external-save boundary, MBC7 no-runtime-rumble policy, and Pocket Camera official-shape validation.

## Deferred boundaries

- `Bandai TAMA5` needs a dedicated accessory device before it can move out of `AccessorySpecialCase`.
- Broad `EMS`, `Bung`, and `Wisdom Tree` support remains experimental/heuristic until each family has a typed runtime contract and validation evidence.
- Stronger hardware evidence may revise the current MBC3 relatch, reserved-selector, RTC access-spacing, MBC6 flash-latency, or MBC7 rumble policies; record such changes in [`../TODO.md`](../TODO.md) or the relevant roadmap before changing runtime behavior.
- Additional title-layout detection for CGB-era manufacturer-code versus title-suffix bytes remains deferred until a rule can avoid truncating valid software.

## Pitfalls

- Do not infer mapper family from ROM size, title, filename, or successful boot behavior.
- Do not make the bus understand individual mapper bank formulas.
- Do not let unsupported cartridges fall through to nearby supported mappers.
- Do not collapse raw mapper registers, effective bank numbers, and validated cartridge metadata into one `active_bank` integer.
- Do not treat `0x0149` alone as proof that RAM exists or should be persisted.
- Do not save the visible RAM aperture as if it were the whole cartridge payload.
- Do not let execution mode mutate supported hardware semantics.
- Do not hide manual overrides from logs, save states, replays, or debugger UI.
- Do not persist rumble state or host device state in hardware-style cartridge saves.
- Do not treat external `.sav` conversion as lossless unless the target mapper/profile has an explicit representation.
