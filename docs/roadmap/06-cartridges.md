# Phase 6 — Banked cartridges, special cartridges, and cartridge persistence

25. **MBC1**
26. **MBC2**
27. **MBC3**
28. **MBC5**
29. **Special cartridges and unsupported policy**
30. **Banked external RAM, battery, RTC, and cartridge persistence**

#### Goal

Extend `cartridge/` from the closed No MBC baseline to banked commercial cartridge families and generalized cartridge-local persistence without contaminating the rest of the core.
This phase closes cartridge-local persistence only; whole-machine save states remain dedicated Phase `8` work.

#### Modules involved

- `cartridge/`
- `bus/`
- `scheduler/`
- `debugger/`
- frontend/tooling persistence adapters

#### Deliverables

- standard MBC1 support with explicit wiring validation, immediate access-ordered bank effects, and a distinct MBC1M variant path
- standard MBC2 support with address-bit-`8` control decode, internal `512 x 4-bit` RAM, echo aliasing, and explicit header validation
- banking and RTC support for MBC3
- banking support for MBC5
- special-cartridge taxonomy and unsupported policy covering `MBC30`, multicarts, documented-but-unsupported mapper families, accessory cartridges, and optional heuristics
- DMG-relevant special-cartridge runtime follow-up for `MMM01`, `MBC1M`, `HuC1`, `HuC-3`, and `M161`, while keeping CGB-only special-cartridge runtime work explicitly deferred until the base CGB implementation exists
- functional mapper-controlled external RAM beyond the No MBC linear baseline
- typed cartridge persistence contracts for full backing stores such as linear SRAM, banked SRAM, MBC2 nibble RAM, and MBC3 SRAM plus RTC
- portable cartridge-persistence boundaries across frontends and tools
- clear separation between emulation logic and host storage APIs

#### MBC1 sequencing inside Phase 6

1. Establish the MBC1 register model and power-up state.
   Scope: `ram_enabled`, raw `rom_bank_low5`, raw `secondary_bank`, `banking_mode`, deterministic startup for both `RealBoot` and `SkipBoot`, and `0 -> 1` handling for the primary register field.
   Acceptance criteria: power-up state is `ram_enabled = false`, `rom_bank_low5 = 0`, `secondary_bank = 0`, and `banking_mode = 0`; `0x4000-0x7FFF` starts on bank `1`; and writes to `0x0000-0x7FFF` update the intended MBC1 register immediately for later accesses on the shared T-cycle timeline.
   Status: done in the current branch baseline. `MBC1` now loads as its own cartridge device, preserves explicit raw register state, starts the switchable ROM window on bank `1`, and applies RAM-enable plus ROM-control writes immediately on the shared bus timeline instead of rejecting the family as merely reserved.
2. Implement standard MBC1 ROM banking and size masking.
   Scope: high-region bank selection for `32 KiB`, `64 KiB`, `128 KiB`, `256 KiB`, and `512 KiB` ROMs, raw low-register preservation, `0 -> 1` before final size masking, and the documented special-bank behavior.
   Acceptance criteria: `0x4000-0x7FFF` selects the correct bank across the supported small-ROM sizes, the documented small-ROM case where bank `0` can appear in the high region after masking is reproducible, and dedicated tests cover banks `0x01` and `0x1F` plus the raw-register edge case.
   Status: done in the current branch baseline. Standard-wiring `MBC1` now preserves the raw low register, applies `0 -> 1` before the final size mask, reproduces the documented small-ROM high-window `bank 0` case after masking, and has dedicated unit plus bus-visible integration tests for banks `0x01` and `0x1F`.
3. Add large-ROM alternate wiring and mode-dependent low-region mapping.
   Scope: `1 MiB` and `2 MiB` standard MBC1 wiring, secondary-register high ROM bits, mode `0` versus mode `1`, and low-region bank selection for large cartridges.
   Acceptance criteria: banks `0x20`, `0x40`, and `0x60` are unreachable in the switchable high region while `0x21`, `0x41`, and `0x61` are reachable, `0x0000-0x3FFF` stays on bank `0` in mode `0`, mode `1` exposes the documented secondary-controlled low-region banks on large cartridges, and dedicated tests cover `0x21`, `0x41`, and `0x61` explicitly.
   Status: done in the current branch baseline. Large-ROM `MBC1` now keeps the documented `0x20` / `0x40` / `0x60` anomaly in the high window, reaches `0x21`, `0x41`, and `0x61`, and remaps `0x0000-0x3FFF` from the secondary register only in mode `1`.
4. Implement external RAM enable and RAM-bank behavior.
   Scope: RAM-enable decode, disabled-RAM open-bus policy, ignored writes while disabled, fixed `8 KiB` RAM on large-ROM alternate wiring, and banked `32 KiB` RAM on compatible small-ROM cartridges.
   Acceptance criteria: disabled RAM reads follow an explicit policy and writes are ignored, mode `0` fixes RAM to bank `0`, mode `1` selects RAM banks `0..=3` on compatible cartridges, and large-ROM cartridges keep one fixed `8 KiB` visible RAM window.
   Status: done in the current branch baseline. `MBC1` now keeps disabled RAM under the explicit open-bus policy, ignores writes while disabled, selects RAM banks `0..=3` only for compatible small-ROM wiring in mode `1`, and holds large-ROM cartridges to one fixed `8 KiB` RAM window across mode changes.
5. Add MBC1 validation and diagnostics.
   Scope: consistency checks across `0x0147`, `0x0148`, `0x0149`, real ROM size, RAM size, and chosen MBC1 wiring / variant metadata.
   Acceptance criteria: impossible combinations produce clear diagnostics, large-ROM cartridges do not silently masquerade as `32 KiB` banked-RAM cartridges, and MBC1M is either detected explicitly or reserved through a first-class variant flag.
   Status: done in the current branch baseline. `MBC1` validation now rejects impossible ROM-size and RAM-size combinations for the selected wiring, keeps large-ROM cartridges from masquerading as `32 KiB` banked-RAM layouts, and reserves future `MBC1M` space through a first-class internal variant flag instead of ad hoc conditionals.
6. Close with dedicated MBC1 tests and oracle comparisons.
   Scope: unit tests, integration tests, ROM-based coverage, and at least one trusted oracle comparison for bank-selection edge cases.
   Acceptance criteria: tests cover RAM enable, `0 -> 1`, banks `0x01`, `0x1F`, `0x21`, `0x41`, `0x61`, the `0x20` / `0x40` / `0x60` anomaly, the small-ROM high-region bank-`0` case, mode `0` versus mode `1`, `8 KiB` versus `32 KiB` RAM behavior, and explicit configuration diagnostics.
   Status: done in the current branch baseline. Unit and integration tests now cover RAM enable, `0 -> 1`, banks `0x01`, `0x1F`, `0x21`, `0x41`, `0x61`, the `0x20` / `0x40` / `0x60` anomaly, the small-ROM high-region bank-`0` case, mode `0` versus `1`, `8 KiB` versus `32 KiB` RAM behavior, and configuration diagnostics. Phase `6` also now ships retained synthetic MBC1 ROM fixtures for standard banking and small-ROM masking plus RAM banking, `gb-test-runner` exposes the built-in `phase-6-cartridge-oracle` differential suite with portable `serial_hex` artifacts, and the repo-local SameBoy `case-bundle` materialization path now records matched oracle output for the anomaly, small-ROM, and mode-`1` edge cases instead of leaving that lane as harness-only.

#### MBC2 sequencing inside Phase 6

1. Establish the MBC2 control model and power-up state.
   Scope: `ram_enabled`, raw `rom_bank_low4`, address-bit-`8` decode inside the cartridge device, deterministic startup for both `RealBoot` and `SkipBoot`, and the documented `0 -> 1` behavior for the switchable ROM window.
   Acceptance criteria: power-up state is `ram_enabled = false` and raw `rom_bank_low4 = 0`, the effective `0x4000-0x7FFF` bank starts at `1`, writes with address bit `8 = 0` control RAM enable, and writes with address bit `8 = 1` control the ROM-bank register immediately on the shared T-cycle timeline.
   Status: done in the current branch baseline. `MBC2` now loads as its own cartridge device, keeps explicit `ram_enabled` plus raw `rom_bank_low4` state, starts the switchable ROM window on bank `1`, and decodes ROM-space control writes by address bit `8` on the access T-cycle.
2. Implement MBC2 ROM banking and ROM-size validation.
   Scope: switchable-region bank selection in `0x4000-0x7FFF`, raw `4`-bit bank-register preservation, documented `0 -> 1`, final masking by real ROM size, and explicit `256 KiB` maximum validation.
   Acceptance criteria: bank `0` translates to bank `1`, the effective high-region bank follows the real loaded ROM size without losing the raw-register semantics, and MBC2 cartridges that exceed `256 KiB` produce explicit diagnostics.
   Status: done in the current branch baseline. `MBC2` now preserves the raw `4`-bit ROM-bank register, applies `0 -> 1` before final size masking, and rejects ROM declarations above the documented `256 KiB` mapper limit with explicit diagnostics.
3. Implement internal `512 x 4-bit` RAM and echo aliasing.
   Scope: nibble-based internal RAM storage, low-nibble writes, explicit high-nibble read policy, disabled-RAM behavior, and low-`9`-bit address masking across `0xA000-0xBFFF`.
   Acceptance criteria: only `512` logical cells exist, writes preserve only the low nibble, the chosen high-nibble readback policy is explicit, RAM-disabled writes are ignored, RAM-disabled reads follow one explicit policy, and aliasing between `0xA000-0xA1FF` and `0xA200-0xBFFF` is correct.
   Status: done in the current branch baseline. `MBC2` now stores one logical `512 x 4-bit` internal RAM array, masks writes to the low nibble, returns `0xF0 | stored_nibble` under the repo policy, ignores writes while disabled, and aliases `0xA000-0xBFFF` through the low `9` address bits.
4. Add persistence and header validation for MBC2.
   Scope: `0x05` versus `0x06`, battery-backed persistence for internal RAM, `0x0149` special-case validation, and explicit diagnostics for inconsistent header metadata.
   Acceptance criteria: `0x06` persists the internal RAM, `0x05` does not, `0x0149` is not reinterpreted as external SRAM size, and nonzero `0x0149` values on MBC2 cartridges produce clear warnings or errors according to the selected validation policy.
   Status: done in the current branch baseline. `0x05` versus `0x06` now remains explicit in mapper metadata, and nonzero `0x0149` produces clear warnings or errors without being reinterpreted as external SRAM. The shared Phase `6` cartridge-persistence block now exports and restores the full `0x06` nibble-RAM payload through the typed `PersistentCartState::Mbc2Ram` path instead of leaving MBC2 on a mapper-local side path.
5. Close with dedicated MBC2 tests and oracle comparisons.
   Scope: unit tests, integration tests, ROM-based coverage, and at least one trusted oracle comparison for MBC2 bank and RAM edge cases.
   Acceptance criteria: tests cover address-bit-`8` control decode, bank `0 -> 1`, ROM-size diagnostics, echo aliasing across `0xA000-0xBFFF`, low-nibble storage, chosen high-nibble readback policy, battery persistence, and `0x0149 = 0x00` validation.
   Status: done in the current branch baseline. Unit tests, integration tests, one retained synthetic Phase `6` ROM, and the shared cartridge-persistence round-trip coverage now cover address-bit-`8` control decode, bank `0 -> 1`, ROM-size diagnostics, echo aliasing, low-nibble storage, the chosen high-nibble readback policy, battery-backed nibble-RAM persistence, and `0x0149` validation. The built-in `phase-6-cartridge-oracle` differential suite now includes this retained `MBC2` case under the shared cartridge oracle lane, and the SameBoy `case-bundle` materialization path now records matched portable `serial_hex` output for the control-decode and nibble-RAM observables that are stable across both implementations.

#### MBC3 sequencing inside Phase 6

1. Establish the MBC3 control model and power-up state.
   Scope: `ram_rtc_enabled`, raw `rom_bank`, explicit typed `ram_or_rtc_select`, latch-sequence detection for `0x00 -> 0x01`, deterministic startup for both `RealBoot` and `SkipBoot`, and typed distinction between RAM-bank, reserved-selector, and RTC-register selection.
   Acceptance criteria: `0x0000-0x1FFF` enables RAM / RTC on low-nibble `0xA` and disables otherwise, raw ROM bank `0` maps to effective bank `1`, `0x4000-0x5FFF` distinguishes standard MBC3 RAM-bank targets `0x00..=0x03`, reserved selector values `0x04..=0x07`, and RTC-register targets `0x08..=0x0C`, and control writes become visible immediately on the shared T-cycle timeline.
   Status: done in the current branch baseline. `MBC3` now loads as its own cartridge device, keeps explicit `ram_rtc_enabled` plus raw `rom_bank` state, models the `0x4000-0x5FFF` selector as typed RAM / reserved / RTC targets, tracks the `0x00 -> 0x01` latch arm explicitly, and applies all control writes immediately on the shared T-cycle timeline.
2. Implement standard MBC3 ROM and RAM banking.
   Scope: fixed low ROM bank `0`, switchable high ROM bank `0x01..=0x7F`, raw `7`-bit ROM-bank register, real-size masking, standard external-RAM banking up to `32 KiB`, and explicit MBC30 reservation.
   Acceptance criteria: MBC3 supports up to `2 MiB` ROM, the switchable region honors raw `0 -> 1` while still masking by real ROM size, banks `0x20`, `0x40`, and `0x60` are reachable unlike MBC1, RAM banking is masked by real RAM size, and `64 KiB` SRAM configurations are reserved or diagnosed explicitly instead of being treated as standard MBC3.
   Status: done in the current branch baseline. Standard `MBC3` now supports ROM banking up to `2 MiB`, keeps banks `0x20`, `0x40`, and `0x60` reachable, masks RAM banking by real size up to `32 KiB`, and rejects MBC30-like `64 KiB` SRAM declarations as an explicit future variant rather than silently treating them as ordinary MBC3.
3. Implement live RTC registers and latched snapshots.
   Scope: RTC register mapping for `0x08..=0x0C`, live versus latched RTC state, and the `0x6000-0x7FFF` latch edge.
   Acceptance criteria: the RTC snapshot refreshes only on the `0x00 -> 0x01` sequence, repeated reads remain stable until the next latch, reads come from the latched snapshot, and writes go to the live RTC state.
   Status: done in the current branch baseline. `MBC3` now exposes typed RTC register selection for `0x08..=0x0C`, refreshes the latched snapshot only on the `0x00 -> 0x01` sequence, keeps repeated reads stable until the next latch, and routes writes to the live RTC state rather than the snapshot.
4. Add day counter, halt, and carry behavior.
   Scope: `9`-bit visible day counter, `DH.bit0`, `DH.bit6`, `DH.bit7`, overflow behavior, sticky carry, and halted-versus-running live RTC progression.
   Acceptance criteria: visible days stay in `0..=511`, overflow sets carry and wraps the visible day counter, carry stays set until software clears it, `halt` freezes the live RTC, and writes to `DH` control day bit `8`, halt, and carry explicitly.
   Status: done in the current branch baseline. The live `MBC3` RTC now models seconds / minutes / hours / `9`-bit day state, `DH.bit0`, `DH.bit6`, and `DH.bit7`, wraps day overflow back into the visible range while setting sticky carry, and honors `halt` when the deterministic RTC-advance hook is used.
5. Add time-source separation and persistence.
   Scope: explicit separation between visible RTC registers, live RTC counter state, injected time source, and persistence backend; battery-backed elapsed-time handling across powered-off sessions; deterministic testing hooks.
   Acceptance criteria: battery-backed MBC3 cartridges can persist RTC state, elapsed powered-off time is applied through the chosen time-source policy without coupling RTC advancement to CPU cycle count, and tests can run against an injected deterministic clock rather than host wall time.
   Status: done in the current branch baseline. The runtime now has explicit separation between visible RTC registers, live RTC state, and a deterministic injected advance path for tests, so RTC behavior is no longer tied to CPU T-cycles or host wall time. The shared Phase `6` cartridge-persistence block plus `gb-persistence` now persist battery-backed RTC state, apply powered-off elapsed seconds through the configured time source before restore, and preserve live-versus-latched RTC separation across reload.
6. Close with dedicated MBC3 tests and validation follow-up.
   Scope: header-type coverage, RAM-versus-RTC selector behavior, latch sequencing, halt/carry/day overflow, stable snapshots, optional fine-delay research, and explicit future MBC30 tracking.
   Acceptance criteria: tests cover `0x0F`, `0x10`, `0x11`, `0x12`, and `0x13`, raw ROM-bank `0 -> 1`, RAM-bank versus RTC-register selection, latch `0x00 -> 0x01`, halt / carry / day overflow, stable RTC snapshots, and any deferred `16`-T-cycle / `4 us` access-spacing work is recorded explicitly in the roadmap rather than forgotten.
   Status: done in the current branch baseline. Unit tests, integration tests, and one retained synthetic Phase `6` ROM now cover header types `0x0F`, `0x10`, `0x11`, `0x12`, and `0x13`, raw ROM-bank `0 -> 1`, banks `0x20`, `0x40`, and `0x60`, RAM-versus-RTC selector behavior, latch sequencing, stable snapshots, halt / carry / day overflow, and MBC30 reservation diagnostics. The built-in `phase-6-cartridge-oracle` differential suite now carries this `MBC3` case with explicit pre-run RTC advancement through typed runner metadata, and the SameBoy `case-bundle` materialization path now records matched portable `serial_hex` output for the selected banking and RTC observables. The `16`-T-cycle / `4 us` RTC access-spacing recommendation is now represented explicitly as timed cartridge state (`rtc_access_ready_at`) on RTC-register accesses, but still remains advisory rather than enforced behavior pending stronger validation. Current compatibility notes: the curated external `cpp/latch-rtc-test.gb` framebuffer oracle from `GBEmulatorShootout` stays green under a deliberate legacy relatch rule that accepts follow-up non-zero writes after one valid RTC snapshot exists, even though the stricter `Pan Docs` reading would require re-arming with `0x00`. The same curated source also keeps `cpp/rtc-invalid-banks-test.gb` green only while selector values `0x04..=0x07` remain explicit reserved states rather than widened SRAM banks, despite the broader `$00-$07` wording in current `Pan Docs`.

#### MBC5 sequencing inside Phase 6

1. Establish the MBC5 control model and power-up state.
   Scope: `ram_enabled`, raw low `8` ROM-bank bits, raw high `1` ROM-bank bit, raw `ram_bank_raw`, deterministic startup for both `RealBoot` and `SkipBoot`, and explicit variant metadata for RAM / battery / rumble capability.
   Acceptance criteria: `0x0000-0x1FFF` enables RAM on low-nibble `0xA` and disables otherwise, the switchable ROM window really allows bank `0`, the low and high ROM-bank register pieces stay explicit, `0x4000-0x5FFF` updates raw RAM-bank state immediately, and control writes become visible immediately on the shared T-cycle timeline.
   Status: done in the current branch baseline. `MBC5` now loads as its own cartridge device, keeps explicit raw low / high ROM-bank state plus raw RAM-bank state, starts the switchable ROM window on bank `1` while still allowing explicit selection of bank `0`, and applies RAM-enable plus bank-control writes immediately on the shared bus timeline.
2. Implement `9`-bit MBC5 ROM banking and size masking.
   Scope: fixed low ROM bank `0`, switchable high ROM bank `0x000..=0x1FF`, combined `rom_bank_low8 + rom_bank_high1` resolution, real-size masking, and explicit preservation of bank `0` semantics in the high region.
   Acceptance criteria: MBC5 supports up to `8 MiB` ROM, bank `0` is reachable in `0x4000-0x7FFF`, bank `0x1FF` is reachable on full-size images, banks above `0xFF` are reachable through the high ROM-bank bit, and final bank selection remains masked by real ROM size without introducing an MBC1/MBC3-style `0 -> 1` rule.
   Status: done in the current branch baseline. `MBC5` now resolves the full `9`-bit ROM-bank register, keeps bank `0` reachable in the switchable window, reaches bank `0x1FF` on full-size images, crosses the `0xFF -> 0x100` boundary through the explicit high bit, and masks the effective bank by the real ROM size without any `0 -> 1` translation.
3. Implement MBC5 SRAM enable and linear RAM banking.
   Scope: RAM-enable gating, linear `8 KiB` bank selection through `ram_bank_raw`, no MBC1-style dual banking mode, disabled-RAM policy, ignored writes while disabled, real-size masking, and the ordinary `8 KiB`, `32 KiB`, and `128 KiB` SRAM shapes.
   Acceptance criteria: MBC5 SRAM does not behave like normal RAM while disabled, `8 KiB`, `32 KiB`, and `128 KiB` RAM configurations map correctly, effective RAM banks are masked by the real RAM-bank count, no MBC1-style dual banking mode exists, and header variants without RAM do not expose fake SRAM behavior merely because the bank register exists.
   Status: done in the current branch baseline. `MBC5` now gates SRAM on the documented RAM-enable register, ignores writes while disabled, returns the explicit absent-RAM policy while disabled or absent, supports linear `8 KiB`, `32 KiB`, and `128 KiB` SRAM layouts, and masks the effective RAM bank by the real validated backing-store size without inventing any MBC1-style banking mode.
4. Implement rumble-capable MBC5 variants.
   Scope: explicit handling for `0x1C`, `0x1D`, and `0x1E`, observable `rumble_on`, `bit 3` ownership in the `0x4000-0x5FFF` control register, separation between effective RAM-bank selection and motor state, and cartridge-local ownership of rumble behavior.
   Acceptance criteria: `bit 3` of the `0x4000-0x5FFF` control register updates `rumble_on`, the motor state remains latched until software changes it, rumble handling does not break effective RAM-bank selection, and the bus / frontend do not own rumble semantics.
   Status: done in the current branch baseline. Rumble-capable `MBC5` variants now keep `rumble_on` as observable cartridge-local state, route `bit 3` ownership through the cartridge device instead of the bus, keep motor state latched until software changes it, and preserve effective RAM-bank selection separately from the rumble bit.
5. Add MBC5 validation, diagnostics, and persistence expectations.
   Scope: header-type coverage for `0x19..=0x1E`, battery-backed persistence expectations, ROM-size validation up to `8 MiB`, RAM-size validation up to `128 KiB`, and explicit diagnostics for impossible header combinations.
   Acceptance criteria: `0x19..=0x1E` are distinguished cleanly, battery variants persist RAM without changing live mapping rules, ROM sizes above `8 MiB` produce clear errors, type / RAM mismatches such as "no RAM type with nonzero `0x0149`" produce clear errors, and rumble-capable types are not accepted unless the implementation exposes observable rumble state.
   Status: done in the current branch baseline. `MBC5` header types `0x19..=0x1E` are now distinguished through explicit variant metadata, ROM sizes above `8 MiB` produce clear errors, no-RAM types with nonzero `0x0149` emit clear validation diagnostics, rumble-capable types now expose observable `rumble_on` state, and the shared Phase `6` cartridge-persistence block now treats battery-backed `MBC5` SRAM as a hardware-style persistent payload without changing the live mapper contract.
6. Close with dedicated MBC5 tests and oracle comparisons.
   Scope: unit tests, integration tests, ROM-based coverage, and at least one trusted oracle comparison for bank-selection and rumble edge cases.
   Acceptance criteria: tests cover header types `0x19..=0x1E`, bank `0` visibility in the switchable region, bank `0x1FF`, `9`-bit ROM-bank selection across the `0xFF -> 0x100` boundary, RAM-enable behavior, SRAM behavior for `8 KiB` / `32 KiB` / `128 KiB`, RAM-bank masking, rumble on/off, and size-validation diagnostics.
   Status: done in the current branch baseline. Unit tests, integration tests, and one retained synthetic Phase `6` ROM now cover header types `0x19..=0x1E`, bank `0` visibility in the switchable region, bank `0x1FF`, the `0xFF -> 0x100` boundary, RAM-enable behavior, SRAM behavior for `32 KiB` and `128 KiB`, RAM-bank masking, rumble on/off, and size-validation diagnostics. The built-in `phase-6-cartridge-oracle` differential suite now includes the retained `MBC5` rumble/banking case under the shared cartridge oracle lane, and the SameBoy `case-bundle` materialization path now records matched portable `serial_hex` output for the selected bank-selection and rumble observables.

#### Special-cartridge and unsupported-policy sequencing inside Phase 6

1. Establish the special-cartridge taxonomy and unsupported categories.
   Scope: one central classification path for `Supported`, `PlannedVariant`, `DocumentedButUnsupported`, `ExperimentalHeuristic`, `AccessorySpecialCase`, and `UnknownCode`, plus stable names for `MBC30`, `MBC1M`, `MMM01`, `M161`, `HuC1`, `HuC-3`, `MBC6`, `MBC7`, `Pocket Camera`, `Bandai TAMA5`, `EMS`, `Bung`, and `Wisdom Tree`.
   Acceptance criteria: the loader produces stable classification for all of those cases, the frontend does not need to reparse headers to explain them, and the classification preserves raw `0x0147`, detected name, category, and reason.
   Status: done in the current branch baseline. The loader now owns one central classification path covering `Supported`, `PlannedVariant`, `DocumentedButUnsupported`, `ExperimentalHeuristic`, `AccessorySpecialCase`, and `UnknownCode`, with stable names and reasons for header-coded special families, the currently wired experimental heuristics for `EMS`, `Bung`, and `Wisdom Tree`, and one strict-mode explicit documented-special signature path for the known Mani `4-in-1` `M161` multicart.
2. Add explicit `MBC30` detection.
   Scope: detect the `MBC3`-family plus `64 KiB` SRAM case as `MBC30`, return a typed planned variant or supported variant entry point instead of ordinary standard `MBC3`, and reserve matching persistence / banking work.
   Acceptance criteria: `MBC3 + 64 KiB SRAM` never falls through to standard `MBC3`, loader diagnostics name `MBC30` explicitly, and the code path is ready for future concrete `MBC30` implementation.
   Status: done in the current branch baseline. `MBC3`-family headers with `64 KiB` SRAM now classify as explicit `MBC30` planned variants, fail as known reserved variants rather than as invalid standard `MBC3`, and keep the future concrete implementation path explicit.
3. Add multicart and near-variant classification.
   Scope: classify `MMM01` from `0x0B..=0x0D`, reserve future `MBC1M` as a distinct `MBC1`-family variant, keep `M161` in a multicart-special path, and avoid assuming that `MMM01` boot/header handling is identical to standard cartridges.
   Acceptance criteria: `0x0B`, `0x0C`, and `0x0D` are emitted as `MMM01`, `MBC1M` remains a separate future variant instead of being merged into standard `MBC1`, and multicarts do not silently degrade to ordinary `MBC1` or `NoMbc`.
   Status: done in the current branch baseline. Header-coded `MMM01` variants already classify explicitly as multicart-special documented-but-unsupported hardware, standard `MBC1` still reserves a distinct internal future `MBC1M` variant instead of merging those rules into ordinary `MBC1`, and the known Mani `4-in-1` multicart now classifies explicitly as `M161` through a deliberate signature path instead of falling through to ordinary `NoMbc` or generic unknown handling.
4. Enforce controlled failure for documented special hardware.
   Scope: explicit diagnostics for `HuC1`, `HuC-3`, `MBC6`, `MBC7`, `Pocket Camera`, `Bandai TAMA5`, and `M161`, plus a hard rule against automatic fallback to `MBC1`, `MBC3`, `MBC5`, or other nearby supported mappers.
   Acceptance criteria: those types fail with clear messages naming the exact detected cartridge, `UnknownCode` reports the raw `0x0147` byte, and no silent degradations or fake "best effort" mapper substitutions remain.
   Status: done in the current branch baseline. `HuC1`, `HuC-3`, `MBC6`, `MBC7`, `Pocket Camera`, `Bandai TAMA5`, `M161`, and generic `UnknownCode` values now fail with explicit typed diagnostics and without silent fallback to nearby supported mappers.
5. Add optional experimental heuristic mode.
   Scope: isolate `EMS`, `Bung`, and `Wisdom Tree` detection behind an explicit dev / experimental loader policy, keep strict default behavior header-driven, and document that heuristic paths are lower priority than `MBC30`, multicarts, and documented special hardware.
   Acceptance criteria: heuristic detection is off by default, can be enabled explicitly for development and research, and diagnostics clearly state when a classification came from heuristics instead of a standard header mapping.
   Status: done in the current branch baseline. The loader now keeps heuristic detection disabled in `Strict` and `Permissive`, while `Experimental` with heuristics enabled can reclassify the currently wired `EMS`, `Bung`, and `Wisdom Tree` signatures into explicit `ExperimentalHeuristic` results whose rejection reasons say that the classification came from an experimental heuristic path.

#### DMG-only execution split for special-cartridge follow-up

Within the current DMG-only but CGB-ready project plan, execute the special-cartridge follow-up in this order:

1. classification / no-fallback / typed variant space — in scope now and already closed in the current baseline
2. `MMM01` runtime support — in scope before CGB
3. `MBC1M` runtime support — in scope before CGB
4. `HuC1` runtime support — in scope before CGB
5. `HuC-3` runtime support — in scope before CGB
6. `M161` runtime support — in scope before CGB
7. base CGB bring-up gate — deferred to `TODO.md`
8. `MBC30` runtime support — deferred to `TODO.md` until point `7` is closed
9. `MBC7` runtime support — deferred to `TODO.md` until point `7` is closed
10. `MBC6` runtime support — deferred to `TODO.md` until point `7` is closed

The purpose of that split is to close the remaining DMG-relevant special-cartridge work first while keeping CGB-only specials explicit, typed, and impossible to misclassify. Before point `7` is closed, `MBC30`, `MBC7`, and `MBC6` may grow diagnostics, validation hooks, persistence shapes, or device skeletons, but they must not be counted as functionally supported runtime targets.

Current status for point `2`: done in the current branch baseline. `MMM01` now loads through the boot-visible trailing menu header, starts in explicit unmapped mode on the last `32 KiB` of the ROM, and supports mapped-mode ROM / RAM banking through a dedicated cartridge device instead of falling back to `MBC1`. The current implementation has already been validated against the commercial `Momotarou Collection 2 (Japan) (SGB Enhanced)` MMM01 cartridge path, including menu startup and both included games.

Current status for point `3`: done in the current branch baseline. `MBC1M` now enters through an explicit repeated-subheader signature path in strict mode for `1 MiB` multicarts, and the runtime path already covers both the no-RAM baseline and the fixed-`8 KiB` RAM commercial shape used by `Momotarou Collection`. The current implementation has already been validated against the commercial `Momotarou Collection (Japan) (SGB Enhanced)` cartridge path, including menu startup and both included games.

Current status for point `4`: in progress in the current branch baseline. `HuC1` now loads through its own supported mapper family, uses explicit `ram_mode` versus `ir_mode` state instead of MBC1 RAM gating, exposes dedicated HuC1 ROM / RAM banking plus ignored `0x6000-0x7FFF` writes, and persists battery-backed RAM through its own payload shape instead of masquerading as `MBC1`. The commercial validation set for this point is now explicitly `Daikaijuu Monogatari - The Miracle of the Zone II`, `Pokemon Card GB`, and `Pocket Bomberman`; the host-side IR light-input seam is intentionally out of scope for the current DMG roadmap, and the remaining blocker is the gameplay hang that still appears in `Pocket Bomberman` after some normal play.

#### Cartridge-persistence sequencing inside Phase 6

1. Define the cartridge persistent-state contract.
   Scope: a typed contract such as `PersistentCartState` that each supported cartridge family exposes, explicit per-mapper payload shapes, and strict separation from full-emulator save-state serialization.
   Acceptance criteria: `NoMbc`, `Mbc1`, `Mbc2`, `Mbc3`, and `Mbc5` expose cartridge-owned persistent payloads, `Mbc2` and `Mbc3` use dedicated payload shapes for nibble RAM and RTC state, and no CPU, PPU, APU, WRAM, or other console-owned state leaks into the contract.
   Status: done in the current branch baseline. `gb-core` now exposes a typed cartridge-persistence contract directly from `cartridge`, including explicit capability metadata, per-mapper payload shapes for `NoMbc`, `Mbc1`, `Mbc2`, `Mbc3`, and `Mbc5`, and restore validation that stays entirely inside cartridge-owned state without leaking CPU, PPU, APU, WRAM, or other console-owned data into the payload.
2. Build the save backend.
   Scope: disk and in-memory adapters, versioned save format, path / name mapping, load and save APIs, backend metadata separation from raw cartridge payloads, and portability across CLI, desktop, web, and tests.
   Acceptance criteria: the backend can round-trip complete cartridge backing stores rather than only visible windows, supports battery-backed RAM and RTC payloads, and can be tested without real file I/O.
   Status: done in the current branch baseline. The workspace now contains a host-side `gb-persistence` crate with a versioned cartridge-save envelope, explicit logical save keys, shared encode/decode helpers, in-memory and filesystem backends, injected time-source support for deterministic save timestamps, and safe-replacement disk writes. The backend round-trips the full cartridge-owned payload shapes exported by `gb-core`, including `MBC2` nibble RAM and `MBC3` RAM+RTC payloads, without pulling file I/O or path policy into the core. Battery-gated auto-save policy, powered-off RTC elapsed-time application, and higher-level flush policy still remain sequenced under the later cartridge-persistence subblocks.
3. Integrate battery-gated hardware persistence.
   Scope: binding persistence eligibility to validated `0x0147` capability data such as `has_battery`, preserving non-persistent RAM semantics, and avoiding automatic hardware-style saves for cartridges that do not provide battery-backed storage.
   Acceptance criteria: only battery-backed cartridges generate hardware-style persistence by default, `MBC2` restores nibble RAM correctly, `MBC3` restores SRAM plus RTC correctly, and non-battery cartridges do not silently produce hardware-save payloads unless an explicit non-faithful option is added.
   Status: done in the current branch baseline. `gb-persistence` now exposes explicit host-side hardware-persistence helpers that gate default cartridge save/load behavior through validated cartridge capability metadata instead of filename heuristics or RAM-size guesses. The default helper path only persists battery-backed `PersistentRam`, `PersistentRtc`, or `PersistentRamAndRtc` profiles, skips `NonPersistentRam` cartridges without creating hardware-save files, and already covers `MBC2` nibble RAM plus `MBC3` SRAM+RTC round-trips through the public helper layer. Powered-off RTC elapsed-time application and higher-level flush triggers remain sequenced under the later persistence subblocks.
4. Fix the off-session RTC time policy.
   Scope: explicit elapsed-real-time handling for battery-backed `MBC3`, support for both real clocks and injected deterministic clocks, preservation of live-versus-latched RTC separation, and clear T-cycle versus powered-off-time boundaries.
   Acceptance criteria: real and injected clocks are both supported, elapsed powered-off time is applied without coupling RTC advancement to accumulated CPU cycles alone, and day counter, halt, and carry survive persistence round-trips correctly.
   Status: done in the current branch baseline. The cartridge-side RTC arithmetic is now exposed through `Mbc3RtcPersistentState`, and the battery-gated hardware-persistence load helper applies powered-off elapsed seconds before restore using the save backend's time source rather than CPU T-cycles. Both system-time and injected deterministic clocks are supported through the backend time-source abstraction, halted RTC state still blocks progression, and overflow / carry behavior survives reload with elapsed-time application. The remaining persistence work is now focused on higher-level save triggers and flush policy.
5. Harden save writes and flush policy.
   Scope: save-on-close, manual or forced save, optional auto-flush after writes to persistible cartridge state, atomic replacement or equivalent corruption-avoidance strategy, format versioning, and clear error reporting.
   Acceptance criteria: the save backend exposes explicit flush policy choices outside the bus, versioned payloads are written atomically or through an equivalent safe strategy, and persistence errors surface clearly instead of failing silently.
   Status: done in the current branch baseline. `gb-persistence` now exposes an explicit host-side persistence manager with `Manual`, `SaveOnClose`, and `AutoFlushAfterPersistibleWrite` policies, plus explicit `flush`, `force_save`, and `close` entrypoints. Dirty-state tracking lives entirely in the host-side persistence layer rather than the bus, repeated disk saves use the existing safe-replacement path, and filesystem failures now surface synchronously through the returned error path instead of failing silently. With this step closed, the cartridge-persistence block of Phase `6` is fully implemented in the current branch baseline.

#### Done criteria

- the bus uses a clean interface toward the cartridge
- each MBC lives inside `cartridge/` without polluting the rest of the system
- standard MBC1 behavior is modeled inside cartridge devices with explicit wiring / variant metadata rather than bus-side heuristics or one opaque active-bank field
- standard MBC2 behavior is modeled inside cartridge devices with explicit address-bit-`8` control decode, internal nibble RAM semantics, and mapper-local validation rather than generic external-SRAM assumptions
- standard MBC3 behavior is modeled inside cartridge devices with explicit RAM-bank versus RTC-register selection, live-versus-latched RTC state, and a reserved future MBC30 extension point
- MBC5 behavior is modeled inside cartridge devices with explicit `9`-bit ROM-bank state, valid switchable-region bank `0`, explicit RAM / battery / rumble variant handling, and observable cartridge-local rumble state
- special cartridges and unsupported cases are classified explicitly, fail in a controlled way, and do not silently fall back to nearby supported mappers
- RTC and persistence are properly encapsulated
- hardware-style persistence stores full cartridge backing stores rather than whichever `0xA000-0xBFFF` mapping happened to be visible
- only battery-backed cartridges auto-persist by default, while full emulator save states remain a separate system
- the save backend supports versioned payloads, an injected RTC time source, and atomic or equivalent safe writes
- persistence does not break portability between CLI, desktop, and web

#### Risks if integrated poorly

- cartridge logic spread throughout the bus
- persistence coupled directly to the core
- MBC3 treated as "MBC1 with a clock" and split across bus, cartridge, and persistence layers
- visible `0xA000-0xBFFF` windows mistaken for full save payloads
- `ram_enabled` or latched RTC state mistaken for persistence truth
- difficulty extending to more mappers or future variants
