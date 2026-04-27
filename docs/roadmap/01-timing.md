# Phase 1 — Temporal foundation and hardware access

4. **T-cycle / dot temporal foundation**
5. **Bus and arbitration**
6. **Complete memory map and special behavior by region**
7. **Base cartridge interface**
8. **General I/O registers and read/write rules**
9. **Boot ROM mapping, startup modes, and handoff infrastructure**

#### Goal

Build the real foundation of the emulated system on top of which CPU, timer, DMA, PPU, and peripherals will rest.

#### Modules involved

- `scheduler/`
- `bus/`
- `boot/`
- `cartridge/`
- `model/`
- `debugger/`

#### Deliverables

##### Temporal foundation

- global stepping by T-cycle
- explicit notion of dot compatible with the PPU
- stable stepping order for subsystems inside the scheduler
- explicit ordering between external-event ingress, shared-counter advance, derived edges, free-running peripherals, bus arbitration, CPU micro-ops, MMIO commit, IRQ aggregation, and CPU wake / accept

##### Bus and memory map

- memory region resolution
- unified access to ROM, VRAM, WRAM, external RAM, OAM, I/O, HRAM, and IE
- modeling of echo RAM and unusable areas
- access arbitration infrastructure
- one central decode path across `0x0000-0xFFFF` with explicit owner and access policy per region
- distinct decode / ownership and access-policy layers in that arbitration path
- centralized MMIO register routing instead of a generic RAM-like `FF00-FF7F` block

##### Base cartridge

- central header parser over `0x0100-0x014F`
- strongly typed cartridge metadata including `entry_point`, `cgb_flag`, `sgb_flag`, `cartridge_type`, `rom_size_code`, and `ram_size_code`
- base cartridge interface for the No MBC family and later MBC-backed devices
- `No MBC` as the first closed reference cartridge rather than a generic fallback path
- cartridge integration with the bus
- central cartridge factory that selects a supported device or a structured special / unsupported classification from `0x0147`
- explicit validation of declared ROM/RAM metadata against the loaded image with a configurable policy
- clean separation between bus logic and cartridge-specific logic

##### I/O and boot

- general I/O registers with base read/write rules
- centralized MMIO metadata describing owner, per-address register identity, access class, and model-specific availability, with detailed bit-level semantics remaining owned by the responsible subsystem
- MMIO infrastructure for mixed registers composed from latched, dynamic, forced, and unimplemented bits
- boot ROM integration into the memory map
- correct boot ROM unmapping
- system-visible startup configuration
- explicit startup-mode configuration, with a functional `SkipBoot` path in this phase and a first-class `RealBoot` path reserved for Phase 2 execution
- DMG-family boot-ROM kind selection for `DMG0`, `DMG`, and `MGB`
- `FF50`-driven handoff infrastructure in the bus mapping layer
- skip-boot initialization path with model-aware post-boot state entry
- centralized visible post-boot snapshot tables for CPU and I/O state
- direct-boot snapshot application that respects subsystem-owned mixed-register semantics such as `P1`, `STAT`, `DIV`, and `NR52` rather than raw byte injection
- explicit direct-boot policy for unreliable startup state such as WRAM, HRAM, and other non-deterministic regions

#### Done criteria

- the system can advance by T-cycles consistently
- the scheduler phase order is stable, explicit, and shared across subsystems instead of encoded through call-site accidents
- all memory accesses go through `bus/`
- the memory map is modeled completely for DMG
- every DMG address region has an explicit owner, read behavior, write behavior, and blocked-access policy where applicable
- bus arbitration resolves decode/ownership before applying requester-specific restrictions, and the requester-aware path is already ready for later DMA integration instead of duplicating CPU-only checks
- every MMIO address in `0xFF00-0xFF7F` and `0xFFFF` has an explicit routed owner and contract rather than accidental default byte-storage behavior
- mixed MMIO registers are represented as per-field contracts rather than as plain read/write bytes plus a coarse mask
- the cartridge subsystem parses `0x0100-0x014F` into a typed header structure and preserves CGB/SGB compatibility flags for later work
- a functional No MBC cartridge family exists as the first closed reference cartridge, covering `0x00`, `0x08`, `0x09`, linear `32 KiB` ROM, optional linear `8 KiB` RAM, and ignored ROM-space writes with no hidden bank state
- cartridge implementation selection comes from `0x0147` rather than from ROM-size heuristics
- special and unsupported cartridge types are classified explicitly with raw `0x0147`, detected name, category, and reason rather than one opaque fallback bucket
- declared ROM size and RAM size are validated explicitly instead of being trusted silently
- the bus can access `0x0000-0x7FFF` and `0xA000-0xBFFF` through a base cartridge interface without knowing which MBC is active
- ROM-space writes are routed as cartridge commands instead of fake ROM mutations
- base I/O registers are connected to the bus
- the boot ROM can be mapped and unmapped correctly
- boot-ROM overlay versus cartridge visibility is controlled explicitly by bus-visible mapping state
- MMIO side effects occur on the access itself on the shared timeline rather than in an end-of-instruction cleanup pass
- `SkipBoot` reaches `0x0100` through explicit post-boot initialization rather than partial boot-ROM execution
- startup configuration already distinguishes direct post-boot entry from later executed boot-ROM flow rather than overloading both behind one ambiguous "boot" mode
- deterministic and cartridge-derived visible post-boot state are initialized through one documented path rather than scattered startup literals
- the infrastructure is ready for a later real-boot path to start CPU execution at `0x0000` with boot ROM mapped and hand off through a real `FF50` write once the CPU core exists

#### Risks if done late or incorrectly

- CPU built on unrealistic accesses
- PPU or DMA breaking later because real arbitration was missing
- boot ROM treated as a hack rather than real hardware
- rewriting the bus when introducing DMA, PPU, or MBCs
- scattering mapper detection and header parsing across bus, boot, and frontend code and having to undo that later

#### Recommended subphase breakdown

Phase `1` should be delivered in five subphases. The intent is to close one hardware-facing boundary at a time, with focused tests and local done criteria before moving on. Do not merge two adjacent subphases together unless the later one is blocked on purely mechanical wiring that does not widen hardware scope.

1. **Phase 1A — Bus skeleton and DMG region ownership**
   Goal: replace the current stub bus with a real DMG region-decode and storage baseline while preserving the Phase `0` scheduler contract.
   Scope:
   - ordered bus read/write entry points for the full `0x0000-0xFFFF` map
   - explicit region taxonomy and decode results
   - WRAM and HRAM backing storage plus echo-RAM alias behavior
   - explicit region ownership for ROM, VRAM, WRAM, external range, OAM, unusable area, MMIO, HRAM, and `IE`
   Done criteria:
   - every address resolves through one central decode path with an explicit region owner
   - WRAM, HRAM, and echo RAM behave through bus-owned routing rather than direct ad hoc storage access
   - unusable-space, MMIO, and cartridge ranges already have explicit routed placeholders instead of accidental generic RAM behavior Validation gate:
   - unit tests cover each DMG region boundary and decode result
   - integration tests prove echo-RAM aliasing in both directions
   - smoke tests prove memory traffic enters `bus/` rather than bypassing it
2. **Phase 1B — Requester-aware arbitration and blocked-access policy**
   Goal: separate decode/ownership from live access policy so PPU, DMA, boot, and model-specific constraints can attach without rewriting the bus.
   Scope:
   - requester identity for CPU, DMA, and future bus actors
   - decode result plus requester-aware access-policy evaluation
   - a pure address-router path that resolves nominal domain or region ownership without deciding timing
   - explicit blocked-read and blocked-write result handling
   - policy inputs for boot-ROM overlay, PPU visibility, DMA-published constraints, and model availability
   - bus-originated views for video domains such as VRAM and OAM so PPU-facing and DMA-facing access does not depend on unrelated raw slices
   Note: this subphase closes the arbitration contract only; functional DMG OAM DMA timing remains Phase `3`.
   Done criteria:
   - decode/ownership and access-policy layers are distinct in both code and tests
   - CPU and a synthetic DMA requester already exercise the same arbitration entry point
   - the code structure can evolve toward domain-oriented handlers or controllers without changing the shared T-cycle scheduler contract
   - blocked accesses have explicit observable results rather than falling through to normal storage semantics Validation gate:
   - focused tests cover requester-specific arbitration through one common path
   - tests cover VRAM, OAM, unusable-space, and HRAM policy decisions through injected hardware state
   - trace or snapshot tests lock the ordering between scheduler bus-arbitration phase and evaluated access policy Current implementation note:
  - the current repo baseline already has a pure router plus explicit `state`, `dispatch`, `meta`, `IoHram`, `Wram`, `Oam`, and `Vram` helpers, bus-originated PPU-facing video views, scheduler-visible video ownership synchronization, and a dedicated bus child module for DMG OAM-corruption trigger routing
   - remaining follow-up in this subphase is structural, not conceptual: continue shrinking `bus.rs` by moving more region-local policy into domain helpers where it improves clarity, while keeping one central arbitration entry point and without mixing in future CGB behavior yet
3. **Phase 1C — Cartridge foundation and No MBC closed baseline**
   Goal: replace the empty cartridge placeholder with typed header parsing, centralized classification, and `No MBC` as the first real device family.
   Scope:
   - strongly typed cartridge-header parser and metadata
   - base cartridge interface for `0x0000-0x7FFF` and `0xA000-0xBFFF`
   - central factory and compatibility-policy-driven load decision
   - `No MBC` support for `0x00`, `0x08`, and `0x09`
   Done criteria:
   - cartridge implementation selection comes from `0x0147` through one central factory
   - declared ROM and RAM metadata are validated explicitly against the loaded image
   - `No MBC` closes linear `32 KiB` ROM, optional linear `8 KiB` RAM, and ignored ROM-space writes with no hidden bank state Validation gate:
   - unit tests cover header parsing, size validation, and unsupported-type diagnostics
   - integration tests prove the bus reaches cartridge ROM and external-RAM ranges only through the cartridge interface
   - `No MBC` tests cover `0x0100-0x014F` visibility, optional RAM presence, and ignored ROM-space writes
4. **Phase 1D — MMIO contract table and mixed-register baseline**
   Goal: remove any possibility of treating `0xFF00-0xFF7F` and `0xFFFF` like generic RAM by routing every register through an explicit owner contract.
   Scope:
   - central MMIO descriptor table or equivalent routed-owner mechanism
   - per-address MMIO register identity plus routed access-class metadata
   - mixed-register composition for latched, dynamic, forced, and unimplemented bits inside the owning subsystem contracts
   - first closed register set: `JOYP`, `DIV`, `TIMA`, `TMA`, `TAC`, `IF`, `IE`, `FF46`, and `FF50`
   - explicit DMG fallback policy for unavailable CGB-only registers
   Done criteria:
   - every MMIO address resolves to an explicit owner, register identity, and access contract
   - mixed registers preserve per-field behavior in the owning subsystem rather than as coarse masked byte storage
   - immediate MMIO side effects are visible on the routed access path rather than in deferred cleanup code Validation gate:
   - completeness tests fail if any MMIO address falls back to generic storage
   - unit tests cover per-address MMIO descriptor accuracy plus representative mixed-register readback and write masking behavior
   - integration tests cover immediate side effects for `FF46` and `FF50` plus DMG `0xFF` readback on unavailable CGB-only registers
5. **Phase 1E — Boot mapping, startup presets, and handoff**
   Goal: connect boot-ROM overlay, `SkipBoot`, and future real-boot handoff infrastructure to the real bus, MMIO, and cartridge routing established earlier in the phase.
   Scope:
   - DMG-family boot-ROM kind selection and mapped/unmapped boot state
   - `FF50`-driven overlay handoff in the bus mapping layer
   - centralized visible post-boot snapshot tables and model-aware direct-boot entry, including serial-owned `SB` / `SC`, APU-owned `NRxx` readback, and PPU-owned LCD-visible register readback
   - explicit policy for unreliable startup state in WRAM, HRAM, and other non-deterministic regions
   Done criteria:
   - `SkipBoot` reaches `0x0100` through one documented post-boot initialization path rather than partial boot-ROM execution
   - boot-ROM overlay versus cartridge visibility is observable through the ordinary bus route before and after `FF50`
   - real-boot overlay bytes come from configured boot-ROM assets or an explicit "missing asset reads as `0xFF`" path, never from synthetic placeholder firmware
   - direct-boot visible state includes the published DMG-family audio-register snapshot, while wave RAM stays under an explicit startup policy rather than pretending to be a published hardware constant
   - the infrastructure is ready for Phase `2` real-boot execution without introducing a hidden "skip mode" routing path Validation gate:
   - integration tests cover pre- and post-`FF50` visibility at `0x0000`, `0x0100`, and cartridge-owned ranges
   - snapshot and startup tests cover model-aware post-boot visible state
   - trace tests lock the handoff ordering relative to MMIO side-effect commit

#### Subphase exit rule

Every Phase `1` subphase should end with:

- targeted unit and integration coverage for the newly closed contract
- updated golden traces or snapshots when observable ordering changes
- `make ci` passing locally
- a roadmap TODO recorded immediately if the subphase ships with a concrete uncovered gap

#### MMIO contract sequencing

These steps define register-contract groundwork only. They do not move full joypad, serial, audio, or timing-complete PPU implementation out of their later dedicated phases; those later phases still own complete functional behavior on top of the earlier MMIO contract baseline.

1. Define the central MMIO metadata table.
   Acceptance criteria: every address in `0xFF00-0xFF7F` and `0xFFFF` resolves to an explicit descriptor or dedicated handler, the descriptor identifies the concrete register at that address, and no MMIO address falls back to accidental generic RAM behavior.
2. Add mixed-register composition infrastructure.
   Acceptance criteria: registers such as `JOYP`, `STAT`, `NR14`, and `NR52` can compose latched, dynamic, forced, and unimplemented bits in their owning subsystem without allowing read-only fields to be overwritten accidentally.
3. Close the first non-trivial register-contract baselines.
   Scope: `JOYP`, `DIV/TIMA/TMA/TAC`, `IF/IE`, `FF46`, and `FF50`.
   Acceptance criteria: read/write behavior and immediate side effects are observable through the routed MMIO path without duplicated logic in CPU or bus helpers.
4. Close LCD-facing MMIO contract baselines.
   Scope: `LCDC`, `STAT`, `LY`, `LYC`, `SCX`, `SCY`, `WX`, `WY`, `BGP`, `OBP0`, and `OBP1`.
   Acceptance criteria: dynamic bits, LCD side effects, and impossible writes such as `LY` stores are all handled by the PPU-owned contract.
5. Close serial and audio MMIO contract baselines.
   Scope: `SB/SC`, the `NRxx` family, and wave RAM ownership / visibility rules.
   Acceptance criteria: the routed MMIO contract already encodes correct register identity, read/write policy, immediate register-side effects, and non-RAM-like behavior for these ranges, including wave RAM's explicit ownership and non-reset-with-`NR52` policy; full transfer timing and full APU behavior remain owned by later subsystem phases.
6. Close absent and CGB-only register policy in DMG mode.
   Acceptance criteria: unavailable CGB-only MMIO registers do not behave like RAM, readback follows documented DMG fallback values, and writes follow an explicit ignored-or-stub policy.

#### Base cartridge sequencing

1. Parse the cartridge header.
   Acceptance criteria: `0x0147`, `0x0148`, `0x0149`, `0x0143`, and `0x0146` decode correctly; `entry_point` and logo bytes remain accessible; metadata lives in a strongly typed structure.
2. Define the base cartridge interface.
   Acceptance criteria: the bus can read `0x0000-0x7FFF` and `0xA000-0xBFFF` without knowing the active MBC, ROM-space writes route to cartridge commands, and header bytes remain visible through ordinary ROM bank `0` reads.
3. Add the cartridge factory.
   Acceptance criteria: the loader selects `NoMbc`, `Mbc1`, `Mbc2`, `Mbc3`, `Mbc5`, or a structured special / unsupported classification from `0x0147`, and unsupported types preserve raw `0x0147`, detected name, category, and reason for diagnostics.
4. Introduce the typed compatibility-policy foundation.
   Scope: one real `ExecutionMode::{Strict, Permissive, Experimental}` type plus a central `CompatibilityPolicy`-style structure carrying validation, heuristic, override, and diagnostic policy, as defined authoritatively in `docs/ARCHITECTURE.md`.
   Acceptance criteria: execution modes are not represented as scattered booleans, one shared policy object exists for loader, tooling, and frontends, and the T-cycle core does not need to read ad hoc global compatibility flags.
5. Close initial validation and diagnostics plumbing against that shared policy.
   Acceptance criteria: ROM-size and RAM-size metadata are checked explicitly, size mismatches produce useful warnings or errors, special ROM-size codes are not ignored silently, documented-but-unsupported cartridge types fail in a controlled way without mapper fallback, and strict / permissive / experimental admission already goes through the shared policy foundation instead of per-call-site booleans.

##### No MBC milestone

1. Construct `NoMbcCartridge`.
   Acceptance criteria: the loader recognizes `0x00`, `0x08`, and `0x09` as the No MBC family; `0x00` builds without external RAM; `0x08` and `0x09` build with optional `8 KiB` RAM and preserve the raw type plus battery distinction for diagnostics and persistence policy.
2. Close fixed ROM reads.
   Acceptance criteria: `0x0000-0x7FFF` reads are linear and bankless, `0x0100-0x014F` plus the entry point stay visible through ordinary cartridge reads, and boot-ROM overlay still belongs to the bus mapping layer on the shared T-cycle timeline.
3. Close ROM-space write policy.
   Acceptance criteria: writes to `0x0000-0x7FFF` are still delegated through the cartridge interface as ordered T-cycle accesses, but `NoMbcCartridge` ignores them with no side effects and no fake ROM mutation.
4. Add optional external RAM.
   Acceptance criteria: `0xA000-0xBFFF` is either explicit "RAM absent" behavior or one linear `8 KiB` RAM window; there is no RAM enable, no RAM banking, no RTC, and battery only changes persistence policy.
5. Harden validation and diagnostics.
   Acceptance criteria: No MBC expects `32 KiB` ROM and at most `8 KiB` RAM, inconsistent headers report declared type, declared ROM size, declared RAM size, and actual file size, `0x08` and `0x09` may warn as rare but are not rejected solely for rarity, and strict / permissive / experimental mode handling stays centralized.
6. Close integration coverage.
   Acceptance criteria: skip-boot and post-`FF50` mapping tests use No MBC as the first closed cartridge baseline in this phase; once Phase 2 real-boot execution exists, the first real-boot cartridge coverage also lands on No MBC before any MBC-dependent validation.

#### Compatibility-policy sequencing across cartridge bring-up

This subsection operationalizes the cartridge-specific decision matrix defined authoritatively in `docs/hardware/CARTRIDGES-MBC.md` on top of the Phase `1` policy foundation above. `docs/ARCHITECTURE.md` remains the source of truth for the policy shape and supported-hardware invariant, and `docs/TESTING.md` remains the source of truth for CI/oracle usage of execution modes.

1. Centralize the category-by-mode decision table.
   Scope: resolve `Supported`, `PlannedVariant`, `DocumentedButUnsupported`, `ExperimentalHeuristic`, `AccessorySpecialCase`, and `UnknownCode` through one shared matrix driven by typed cartridge classification.
   Acceptance criteria: load / warn / reject behavior is decided centrally, the loader does not duplicate per-mode classification logic, and `Strict`, `Permissive`, and `Experimental` keep supported-hardware runtime semantics identical.
2. Close diagnostics and manual overrides.
   Scope: explicit rejection and warning reasons, visible heuristic and partial-path diagnostics, and manual overrides for model, mapper, mode, and validation policy.
   Acceptance criteria: loader messages report raw `0x0147`, detected name, category, current mode, and precise reason; overrides are visible in logs and tooling; and no silent mapper invention remains.
3. Integrate execution mode into save states, replays, CI, and tooling.
   Scope: persist execution-mode metadata, reject mismatched-mode restore by default, keep CI and oracle comparison on `Strict`, and segregate `Experimental` artifacts.
   Acceptance criteria: save states and replay logs record the originating mode and active overrides, strict-mode CI remains the official closure path, and experimental runs cannot be mistaken for oracle evidence.
