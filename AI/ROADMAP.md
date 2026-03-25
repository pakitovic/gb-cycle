# DMG ROADMAP — T-Cycle-Based Game Boy DMG Core

## Goal

This document defines the implementation roadmap for the project's **DMG** core, focused on an emulator that is:

- **T-cycle based**, with T-cycles as the system's base temporal unit
- **PPU dot-by-dot**
- **Mode 3 based on a pixel FIFO**
- architecturally prepared from the beginning for a future **CGB** extension, but **without implementing any CGB behavior yet** within this roadmap

This roadmap describes the recommended implementation order, the dependencies between blocks, and the completion criteria for each phase in order to minimize rework and avoid false foundations in timing, bus behavior, PPU, or CPU.

## Document status

This roadmap is a living project document.

Use it for:

- the recommended implementation order
- the dependency and phase context for ongoing work
- phase-level done criteria
- tracking concrete TODOs left behind by partial implementations, deferred fixes, incomplete validation, or postponed refactors

Update this document whenever:

- implementation order or phase structure changes
- a phase's done criteria or scope changes
- a task ships with known remaining work relevant to one of these phases
- an existing roadmap TODO is completed, invalidated, or superseded

Keep roadmap TODOs concrete and implementation-linked. Do not use this document as a general idea backlog.
If no single phase owns the remainder cleanly, place the TODO under `Cross-phase`.

---

## Authority boundaries

This roadmap sequences work, but it is not the behavioral source of truth.

- For crate layout, ownership, and subsystem boundaries, follow `AI/ARCHITECTURE.md`.
- For subsystem behavior, MMIO semantics, and timing contracts, follow the matching `AI/hardware/*.md` file plus `AI/TIMING-AND-ACCURACY.md`.
- For project-wide validation policy, follow `AI/TESTING.md`.
- For the boundary between cartridge persistence and whole-machine save states, follow `AI/ARCHITECTURE.md`; for cartridge-persistence semantics, follow `AI/hardware/CARTRIDGES-MBC.md`; for save/load determinism and oracle usage, follow `AI/TESTING.md`.

If roadmap prose ever drifts from those documents, update the roadmap to match the authoritative source instead of treating the mismatch as two valid policies.

---

## Scope of this roadmap

This roadmap covers only the **DMG** core.

Out of scope for this roadmap:

- functional CGB implementation
- CGB-specific features
- CGB-specific DMA
- double speed mode
- VRAM banking
- WRAM banking
- CGB palettes
- HDMA/GDMA

Future CGB compatibility must be considered **at the architectural level** from the beginning, but it is not part of the functional scope of this document.

---

## Core project principles

### 1. Single temporal model: T-cycle

The entire core is modeled around **T-cycles** as the fundamental temporal unit.

A model centered on M-cycles is not used for the main logic, in order to avoid ambiguity and misleading simplifications in:

- CPU fetch
- memory accesses
- bus arbitration
- DMA
- timer
- interrupt acceptance
- CPU/PPU/APU synchronization
- PPU modes
- LCD dot-by-dot behavior

The global scheduler must be able to advance the system **one T-cycle at a time**.

### 2. Dot-by-dot PPU

The PPU is not modeled using abstract scanlines or deferred full-line rendering.

The PPU must be modeled **dot-by-dot**, with special attention to:

- transitions between modes
- Mode 2 / Mode 3 / HBlank / VBlank timing
- fetcher
- pixel FIFO
- scroll
- window
- sprites
- progressive generation of visible output

### 3. Mode 3 with pixel FIFO

The PPU rendering pipeline must be built around an explicit model of:

- fetcher
- pixel FIFOs
- per-dot pixel production and consumption
- BG / window / OBJ mixing
- priorities and edge cases

### 4. Architecture prepared for growth

The architecture must allow the system to be extended later to CGB without breaking the DMG core foundations.

That implies:

- clean modeling of hardware variants
- clear subsystem separation
- avoiding magic constants or DMG-specific branches embedded in places that would later block extension
- designing bus, PPU, DMA, cartridge, and scheduler interfaces from the beginning so they do not require large rewrites later

### 5. Verification and observability from the start

A timing-faithful emulator cannot be sustained with only final functional tests.

From the very beginning there must be:

- a test strategy
- support for test ROMs
- internal tracing tools
- state inspection
- debugging utilities
- temporal comparison points between subsystems

---

## Reference architecture

For the authoritative structure and boundaries, see
[`AI/ARCHITECTURE.md`](./ARCHITECTURE.md), especially:

- `Recommended high-level layout`
- `Suggested subsystem boundaries`
- `Detailed module responsibility guide`
- `Ownership boundary notes`

---

## Cross-cutting scheduler foundation workstream

This workstream spans the early roadmap because the scheduler contract must be
fixed before later CPU, DMA, PPU, timer, serial, joypad, and APU work can stay
coherent without refactoring.

1. **Explicit scheduler phases** (`Phase 0`)
   Goal: refactor stepping around one visible `step_t_cycle()`-style entry point with fixed per-T-cycle phases.
   Acceptance criteria: the phase order is explicit and stable, there are no hidden cross-calls that bypass it, the scheduler owns ordering rather than reimplementing subsystem rules, and one `CycleContext`-style object or equivalent carries current-cycle events, derived signals, ownership facts, and queued side effects or IRQ requests.
2. **Central arbitration** (`Phase 1`)
   Goal: unify decode, ownership, and access policy behind one requester-aware bus path.
   Acceptance criteria: CPU and DMA-ready requesters use the same arbitration route, decode/ownership and access-policy layers stay distinct, and Phase `1` closes the requester-aware bus contract that Phase `3` later reuses for DMG OAM DMA source-bus-aware CPU blocking behavior.
3. **IRQ aggregation layer** (`Phase 2`)
   Goal: separate source request, `IF` visibility, and CPU acceptance.
   Acceptance criteria: PPU, timer, serial, and joypad only request; the CPU accepts by `IME/IE/IF` and fixed priority; timer keeps its delayed `4`-T-cycle (`1` M-cycle) request timing.
4. **Cycle logging** (`Phase 0`, expanded later)
   Goal: make the actual ordering visible per T-cycle.
   Acceptance criteria: traces can expose phase, bus owner, CPU micro-op, PPU mode, DMA activity, timer or serial events, and `IF/IE/IME`.
5. **Global-order regression tests** (`Phase 2` onward)
   Goal: lock down the scheduler invariants at cross-subsystem boundaries.
   Acceptance criteria: focused tests cover DMA versus CPU, delayed timer `IF`, serial completion plus IRQ, joypad visible `High -> Low` plus IRQ, `HALT` / IRQ priority, and `STAT`-versus-bus coherence.

## Cross-cutting state persistence workstream

This workstream spans cartridge bring-up, snapshot infrastructure, and final
hardening because the project intentionally separates cartridge-local
persistence from whole-machine save states.

1. **Cartridge persistence boundary** (`Phase 6`)
   Goal: persist only cartridge-owned backing stores such as SRAM, MBC2 nibble RAM, and MBC3 live RTC state.
   Acceptance criteria: payloads remain cartridge-owned, frontends and storage backends never infer mapper layout from the currently visible `0xA000-0xBFFF` window, and no CPU, PPU, APU, WRAM, or other console-owned state leaks into hardware-style saves.
2. **Whole-machine save-state boundary** (`Phase 8`)
   Goal: snapshot live machine state across scheduler, CPU, PPU, DMA, timer, APU, peripherals, bus-visible mapping, boot state, and cartridge runtime state.
   Acceptance criteria: restore recreates hidden temporal state coherently, whole-machine metadata records model plus execution mode and active overrides, and save states remain explicitly distinct from cartridge persistence.
3. **Determinism and closure integration** (`Phase 9`)
   Goal: use the Phase 8 save-state system as the foundation for replay, save/load determinism, and DMG closure evidence.
   Acceptance criteria: save/load continuation matches uninterrupted execution under the recorded mode and overrides, and Phase 6 cartridge persistence is never treated as a substitute for whole-machine save/load determinism.

Source-of-truth note: `AI/ARCHITECTURE.md` owns the top-level boundary between
cartridge persistence and whole-machine save states, `AI/hardware/CARTRIDGES-MBC.md`
owns cartridge-persistence semantics, and `AI/TESTING.md` owns save/load
determinism and oracle-usage policy.

---

## Recommended implementation order

### Phase 0 — Verification, debugging, and base architecture infrastructure

1. **Test model and test ROM strategy**
2. **Debugging infrastructure, tracing, and internal tools**
3. **General DMG model and architectural preparation for CGB**

#### Goal

Establish the project's methodological and architectural foundation before locking down detailed hardware behavior.

This phase must define:

- the global temporal model of the core
- the module structure and responsibilities
- the test strategy
- the minimum observability and debugging infrastructure
- the base design that allows future growth without blocking a later CGB expansion

#### Modules involved

- `model/`
- `scheduler/`
- `debugger/`
- `gb-test-runner/`
- `tests/`
- `lib.rs`

#### Deliverables

##### Tests and validation strategy

- initial unit and integration test structure
- test ROM strategy
- convention for golden traces and expected outputs
- classification of suites by subsystem
- reusable runners or helpers from `gb-test-runner/`

##### Debugging and tracing

- base infrastructure in `debugger/`
- initial trace format
- per-subsystem trace hooks
- core state snapshots
- inspection points connectable to the scheduler
- foundation for breakpoints and watchpoints

##### Base architecture

- base hardware types in `model/`
- T-cycle scheduler skeleton
- explicit per-T-cycle phase order with one `step_t_cycle()`-style top-level entry point
- definition of responsibilities by module
- initial interfaces between CPU, bus, PPU, DMA, timer, cartridge, and debugger
- a cycle-local context shape carrying external events, derived signals, ownership facts, and queued side effects or IRQ requests
- conventions to avoid mixing frontend logic with core logic

#### Done criteria

- there is a clear document or convention for the test strategy
- the project can run base tests against `gb-core`
- there is a reusable minimum tracing infrastructure
- the scheduler has a defined notion of T-cycle advancement
- the scheduler phase order is explicit in code and docs rather than implicit in subsystem call chains
- the responsibility split between modules is fixed
- cycle traces can expose enough per-T-cycle state to debug scheduler ordering issues
- the core does not depend on `gb-cli`, `gb-desktop`, or `gb-web` to function
- the architecture is prepared to incorporate CGB without contaminating DMG behavior yet

#### Risks if omitted or overly simplified

- massive rework when introducing tracing later
- the need to rewrite CPU or PPU just to inspect them properly
- tests that are not useful for debugging fine timing issues
- incorrect coupling between frontend and core
- DMG decisions that are too rigid and make a future CGB extension harder

Boundary note: Phase `0` fixes architecture, tracing, and the scheduler
skeleton. Phase `1` is where that skeleton becomes hardware-visible stepping,
arbitration, MMIO, and startup behavior.

Status note (`2026-03-15`): Phase `0` baseline is closed in the current repo.
The project now has the documented test layout, `gb-test-runner` typed
ROM-harness crate, typed debugger breakpoints/watchpoints, `Machine` plus
`step_t_cycle()`, explicit scheduler phases, stubbed subsystem boundaries,
typed debug snapshots, and deterministic scheduler-aligned subsystem trace
hooks. Remaining work moves to Phase `1`.

---

### Phase 1 — Temporal foundation and hardware access

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
- centralized MMIO metadata describing owner, access class, readable bits, writable bits, dynamic bits, reserved bits, read side effects, write side effects, and model-specific availability
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

Phase `1` should be delivered in five subphases.
The intent is to close one hardware-facing boundary at a time, with focused
tests and local done criteria before moving on.
Do not merge two adjacent subphases together unless the later one is blocked on
purely mechanical wiring that does not widen hardware scope.

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
   - unusable-space, MMIO, and cartridge ranges already have explicit routed placeholders instead of accidental generic RAM behavior
   Validation gate:
   - unit tests cover each DMG region boundary and decode result
   - integration tests prove echo-RAM aliasing in both directions
   - smoke tests prove memory traffic enters `bus/` rather than bypassing it
2. **Phase 1B — Requester-aware arbitration and blocked-access policy**
   Goal: separate decode/ownership from live access policy so PPU, DMA, boot, and model-specific constraints can attach without rewriting the bus.
   Scope:
   - requester identity for CPU, DMA, and future bus actors
   - decode result plus requester-aware access-policy evaluation
   - explicit blocked-read and blocked-write result handling
   - policy inputs for boot-ROM overlay, PPU visibility, DMA-published constraints, and model availability
   Note: this subphase closes the arbitration contract only; functional DMG OAM DMA timing remains Phase `3`.
   Done criteria:
   - decode/ownership and access-policy layers are distinct in both code and tests
   - CPU and a synthetic DMA requester already exercise the same arbitration entry point
   - blocked accesses have explicit observable results rather than falling through to normal storage semantics
   Validation gate:
   - focused tests cover requester-specific arbitration through one common path
   - tests cover VRAM, OAM, unusable-space, and HRAM policy decisions through injected hardware state
   - trace or snapshot tests lock the ordering between scheduler bus-arbitration phase and evaluated access policy
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
   - `No MBC` closes linear `32 KiB` ROM, optional linear `8 KiB` RAM, and ignored ROM-space writes with no hidden bank state
   Validation gate:
   - unit tests cover header parsing, size validation, and unsupported-type diagnostics
   - integration tests prove the bus reaches cartridge ROM and external-RAM ranges only through the cartridge interface
   - `No MBC` tests cover `0x0100-0x014F` visibility, optional RAM presence, and ignored ROM-space writes
4. **Phase 1D — MMIO contract table and mixed-register baseline**
   Goal: remove any possibility of treating `0xFF00-0xFF7F` and `0xFFFF` like generic RAM by routing every register through an explicit owner contract.
   Scope:
   - central MMIO descriptor table or equivalent routed-owner mechanism
   - mixed-register composition for latched, dynamic, forced, and unimplemented bits
   - first closed register set: `JOYP`, `DIV`, `TIMA`, `TMA`, `TAC`, `IF`, `IE`, `FF46`, and `FF50`
   - explicit DMG fallback policy for unavailable CGB-only registers
   Done criteria:
   - every MMIO address resolves to an explicit owner and access contract
   - mixed registers are represented per field rather than as coarse masked byte storage
   - immediate MMIO side effects are visible on the routed access path rather than in deferred cleanup code
   Validation gate:
   - completeness tests fail if any MMIO address falls back to generic storage
   - unit tests cover mixed-register readback and write masking behavior
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
   - the infrastructure is ready for Phase `2` real-boot execution without introducing a hidden "skip mode" routing path
   Validation gate:
   - integration tests cover pre- and post-`FF50` visibility at `0x0000`, `0x0100`, and cartridge-owned ranges
   - snapshot and startup tests cover model-aware post-boot visible state
   - trace tests lock the handoff ordering relative to MMIO side-effect commit

#### Subphase exit rule

Every Phase `1` subphase should end with:

- targeted unit and integration coverage for the newly closed contract
- updated golden traces or snapshots when observable ordering changes
- `make check` passing locally
- a roadmap TODO recorded immediately if the subphase ships with a concrete uncovered gap

#### MMIO contract sequencing

These steps define register-contract groundwork only.
They do not move full joypad, serial, audio, or timing-complete PPU implementation out of their later dedicated phases; those later phases still own complete functional behavior on top of the earlier MMIO contract baseline.

1. Define the central MMIO metadata table.
   Acceptance criteria: every address in `0xFF00-0xFF7F` and `0xFFFF` resolves to an explicit descriptor or dedicated handler, and no MMIO address falls back to accidental generic RAM behavior.
2. Add mixed-register composition infrastructure.
   Acceptance criteria: registers such as `JOYP`, `STAT`, `NR14`, and `NR52` can compose latched, dynamic, forced, and unimplemented bits without allowing read-only fields to be overwritten accidentally.
3. Close the first non-trivial register-contract baselines.
   Scope: `JOYP`, `DIV/TIMA/TMA/TAC`, `IF/IE`, `FF46`, and `FF50`.
   Acceptance criteria: read/write behavior and immediate side effects are observable through the routed MMIO path without duplicated logic in CPU or bus helpers.
4. Close LCD-facing MMIO contract baselines.
   Scope: `LCDC`, `STAT`, `LY`, `LYC`, `SCX`, `SCY`, `WX`, `WY`, `BGP`, `OBP0`, and `OBP1`.
   Acceptance criteria: dynamic bits, LCD side effects, and impossible writes such as `LY` stores are all handled by the PPU-owned contract.
5. Close serial and audio MMIO contract baselines.
   Scope: `SB/SC`, the `NRxx` family, and wave RAM ownership / visibility rules.
   Acceptance criteria: the routed MMIO contract already encodes correct read/write policy, immediate register-side effects, and non-RAM-like behavior for these ranges, including wave RAM's explicit ownership and non-reset-with-`NR52` policy; full transfer timing and full APU behavior remain owned by later subsystem phases.
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
   Scope: one real `ExecutionMode::{Strict, Permissive, Experimental}` type plus a central `CompatibilityPolicy`-style structure carrying validation, heuristic, override, and diagnostic policy, as defined authoritatively in `AI/ARCHITECTURE.md`.
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

This subsection operationalizes the cartridge-specific decision matrix defined
authoritatively in `AI/hardware/CARTRIDGES-MBC.md` on top of the Phase `1`
policy foundation above. `AI/ARCHITECTURE.md` remains the source of truth for
the policy shape and supported-hardware invariant, and `AI/TESTING.md` remains
the source of truth for CI/oracle usage of execution modes.

1. Centralize the category-by-mode decision table.
   Scope: resolve `Supported`, `PlannedVariant`, `DocumentedButUnsupported`, `ExperimentalHeuristic`, `AccessorySpecialCase`, and `UnknownCode` through one shared matrix driven by typed cartridge classification.
   Acceptance criteria: load / warn / reject behavior is decided centrally, the loader does not duplicate per-mode classification logic, and `Strict`, `Permissive`, and `Experimental` keep supported-hardware runtime semantics identical.
2. Close diagnostics and manual overrides.
   Scope: explicit rejection and warning reasons, visible heuristic and partial-path diagnostics, and manual overrides for model, mapper, mode, and validation policy.
   Acceptance criteria: loader messages report raw `0x0147`, detected name, category, current mode, and precise reason; overrides are visible in logs and tooling; and no silent mapper invention remains.
3. Integrate execution mode into save states, replays, CI, and tooling.
   Scope: persist execution-mode metadata, reject mismatched-mode restore by default, keep CI and oracle comparison on `Strict`, and segregate `Experimental` artifacts.
   Acceptance criteria: save states and replay logs record the originating mode and active overrides, strict-mode CI remains the official closure path, and experimental runs cannot be mistaken for oracle evidence.

---

### Phase 2 — CPU and real temporal control

10. **Exact CPU core at the fetch / execute / memory access level**
11. **Timer**
12. **Interrupts: IME, IE, IF, EI, DI, priority, acceptance timing**
13. **HALT, STOP, and HALT bug**

#### Goal

Build a truly temporal CPU core, where observable behavior emerges from internal steps compatible with the T-cycle scheduler.

#### Modules involved

- `cpu/`
- `timer/`
- `bus/`
- `scheduler/`
- `debugger/`

#### Deliverables

##### CPU core

- fetch / decode / execute at the T-cycle level
- internal micro-sequences per instruction when needed
- explicitly modeled reads and writes
- correct handling of relevant internal states

##### CPU / boot integration

- real boot-ROM execution through the same CPU core and scheduler used after startup
- real `FF50` write causing cartridge handoff on the next fetch
- logo/checksum outcomes emerging from executed boot-ROM code rather than emulator-side validation
- correct model-visible cartridge-entry state after real boot

##### Timer

- DIV
- TIMA
- TMA
- TAC
- edge timing integrated with the real system clock
- timer interrupt request generation
- direct-boot timer hidden-state synthesis coherent with the visible post-boot timer snapshot

##### Interrupts and CPU states

- IE and IF registers
- IME latch
- real temporal effect of EI and DI
- interrupt priority
- interrupt acceptance timing
- explicit separation between source request, `IF` aggregation, CPU wake, and CPU interrupt acceptance
- HALT
- STOP
- HALT bug

#### Done criteria

- the CPU core does not depend on an oversimplified full-instruction abstraction
- instructions generate their real bus accesses
- the timer advances with the global scheduler
- interrupts and HALT are integrated into the real execution flow
- source requests become visible in `IF` before the CPU accepts them, and timer keeps its delayed request timing instead of being flattened into same-cycle overflow service
- direct-boot timer state does not fake `DIV` or related registers through disconnected visible-only initialization
- real boot executes through the same CPU fetch/decode/execute engine used for the rest of the machine
- real boot reaches cartridge code only through an executed `FF50` write and next-fetch handoff
- invalid boot-logo or header-check cases remain in boot instead of handing off to the cartridge
- tracing can observe fetches, accesses, and IRQ acceptance

#### Recommended sequencing inside Phase 2

Phase 2 should be executed as narrow subphases. No subphase counts as closed
unless its local acceptance criteria land together with focused automated
coverage and move the phase-level done criteria forward without reintroducing
instruction-level shortcuts or hidden timing.

1. `Phase 2.1` - CPU execution plumbing and live register state.
   Acceptance criteria: the CPU stops being a startup-state-only stub, keeps a
   live register file plus explicit in-flight execution state, performs opcode
   fetch as a real bus read at `PC`, advances `PC` through explicit fetch flow,
   and exposes traceable per-T-cycle CPU state such as fetch, execute,
   service-interrupt, halted, and stopped without yet claiming broad opcode
   coverage.
   Validation gate: focused unit tests cover register-file initialization,
   opcode fetch at `PC`, explicit `PC` progression, deterministic micro-step
   traces, and scheduler-visible CPU state transitions under `SkipBoot`.
2. `Phase 2.2` - Memory-visible instruction bring-up.
   Acceptance criteria: the first instruction families run through ordered bus
   accesses rather than aggregate duration tables, `imm8` and `imm16` fetches
   remain explicit and correctly ordered, register-only and `(HL)` forms no
   longer share one flattened timing path, and memory read-modify-write cases
   keep separate read and write phases.
   Validation gate: unit and short integration tests cover `imm8`/`imm16`
   ordering, `(HL)` timing versus register timing, direct and indirect loads,
   ALU flag behavior for implemented families, and deterministic synthetic ROM
   execution for those instruction groups.
3. `Phase 2.3` - Control flow, stack traffic, prefixes, and boot-prerequisite
   opcode closure.
   Acceptance criteria: conditional taken and untaken paths execute through
   different temporal sequences, stack operations become byte-oriented bus
   traffic, `CALL`/`RET`/`RST` reuse that same stack model, CB-prefixed
   execution keeps the double-fetch explicit, and the project records one
   concrete boot-ROM prerequisite opcode matrix before attempting real boot.
   Validation gate: focused tests cover taken versus untaken timing for
   `JR`/`JP`/`CALL`/`RET`, stack byte order and `SP` updates, CB-prefix fetch
   sequencing, and short deterministic programs that cross branches, stack
   transfers, and prefixed instructions.
4. `Phase 2.4` - Real boot execution and `FF50` cartridge handoff.
   Acceptance criteria: `RealBoot` starts at `0x0000` on the same CPU core and
   scheduler used after startup, boot ROM overlay stays bus-owned, boot code
   reaches cartridge execution only through an executed `FF50` write, the next
   fetch after that write already comes from cartridge `0x0100`, invalid logo
   or checksum cases remain in boot, and `No MBC` is the first closed
   real-boot cartridge baseline.
   Validation gate: automated tests cover boot-ROM visibility before handoff,
   next-fetch cartridge visibility after `FF50`, valid handoff versus invalid
   header non-handoff, and DMG-family cartridge-entry state coming from
   executed firmware rather than direct-boot literals.
   Closure note: the first `2.4` landing may use a synthetic DMG boot ROM that
   performs representative header reads, conditional non-handoff, and an
   executed `FF50` write on `No MBC`; full production DMG boot-ROM opcode
   coverage remains tracked separately.
5. `Phase 2.5` - Interrupt-controller integration and CPU accept/service flow.
   Acceptance criteria: hardware producers request interrupts through the
   interrupt controller, `IF` visibility remains separated from CPU acceptance,
   `IME`, delayed `EI`, immediate `DI`, fixed priority, acknowledge, `RETI`,
   and the real `20` T-cycle service sequence are all represented explicitly,
   and scheduler step `8` versus step `9` remains visible in code and traces.
   Validation gate: focused tests cover `IF`/`IE` MMIO behavior, pending IRQ
   visibility with `IME = 0`, priority resolution, `EI ; NOP`, `EI ; DI`,
   `RETI`, and interrupt service timing as a real multi-step CPU sequence.
   Closure note: this phase closes the interrupt-controller plus CPU contract,
   including phase-`8` aggregation into `IF`, phase-`9` CPU acceptance, delayed
   `EI`, immediate `DI`, `RETI`, and bytewise `20` T-cycle servicing. Concrete
   request-generation rules for timer, PPU, serial, and joypad still land in
   their owning subsystem phases.
6. `Phase 2.6` - `HALT`, `STOP`, and the `HALT` bug.
   Acceptance criteria: `HALT`, `STOP`, wake-up, and later interrupt service
   remain distinct ordered events, the `HALT` bug is modeled as a next-fetch
   effect instead of a generic `PC` shortcut, and DMG `STOP` wake flows
   through the joypad-owned hardware path rather than a frontend-only resume.
   Validation gate: focused tests cover `HALT` with `IME = 1`, `HALT` with
   `IME = 0`, already-pending IRQ plus `HALT`, `HALT` bug fetch behavior,
   selection-independent DMG `STOP` wake, and the ordering between wake and
   later interrupt acceptance.
   Closure note: this phase closes the baseline control-state model for
   `HALT`, `STOP`, wake from joypad-owned input transitions, and a next-fetch
   `HALT` bug implementation on the shared scheduler timeline.
7. `Phase 2.7` - Timer edge model, overflow pipeline, and delayed timer IRQ.
   Acceptance criteria: timer state is driven by the shared internal divider on
   the global T-cycle timeline, `DIV` stays a derived view of the internal
   counter, `TAC` selection and enable feed falling-edge TIMA increments,
   overflow enters an explicit delayed reload/request pipeline, timer requests
   become visible in `IF` only after that delay, and `SkipBoot` synthesizes
   timer hidden state coherently with the visible post-boot snapshot.
   Validation gate: focused tests cover `DIV` reset behavior, `DIV` and `TAC`
   glitch cases, frequency-selection edge timing, TIMA overflow and reload
   windows, delayed timer request visibility, and timer-plus-interrupt
   integration without flattening request and service into one instant event.
   Closure note: this phase closes the timer baseline around the shared
   `system_counter`, falling-edge TIMA increments, `4` T-cycle delayed reload
   and request, plus CPU-visible integration with `IF` and later interrupt
   service ordering.
8. `Phase 2.8` - Phase closure, regression matrix, and oracle cross-check.
   Acceptance criteria: tracing can show opcode fetches, operand accesses,
   `IF` visibility, interrupt acceptance, and boot handoff on one shared
   timeline, Phase 2 local TODOs are either closed or explicitly documented,
   and the resulting CPU/timer/IRQ/boot stack is stable enough to stop being a
   moving target for later DMA and PPU work.
   Validation gate: the full unit and integration suite passes, the first Phase
   2 ROM automation targets land for CPU and interrupt timing, and
   timing-sensitive divergences are cross-checked against SameBoy before the
   phase is considered closed.
   Closure note: this phase closes with one shared trace timeline exposing
   phase-`5` CPU bus activity (`opcode_fetch`, `operand_read`, `data_read`,
   `data_write`), phase-`8` `IF` visibility, phase-`9` post-acceptance CPU and
   interrupt state, plus phase-`6` boot handoff visibility around `FF50`. The
   first Phase `2` ROM automation targets now exist as typed `gb-test-runner`
   suites for CPU and interrupt timing. Remaining local TODOs stay explicit
   under the Phase `2` section below, and the current SameBoy cross-check is a
   documented source-level comparison recorded in `AI/research/SAMEBOY.md`;
   full automated first-divergence tooling still belongs to Phase `9`.

#### Risks if done late or superficially

- inability to model HALT bug correctly
- incorrect interrupt acceptance
- timer that appears correct but is temporally false
- the need to rework much of the core when integrating PPU, DMA, or demanding test ROMs

---

### Phase 3 — Base DMA

14. **OAM DMA**

#### Goal

Integrate DMG OAM DMA as a real transfer mechanism inside the system architecture through a reusable DMA-controller foundation, coordinated with the scheduler and bus and already prepared for future CGB transfer families without implementing them yet.

#### Modules involved

- `dma/`
- `bus/`
- `scheduler/`
- `cpu/`
- `debugger/`

#### Deliverables

- common `DmaController`-style infrastructure with explicit active-transfer state
- writing to the DMA register triggers the transfer
- OAM DMA as the first concrete transfer kind inside that common infrastructure
- real temporal copy progression on the shared T-cycle timeline
- integration with bus arbitration
- separation between transfer progression and DMA-published arbitration state
- explicit lifecycle and status visibility for active transfers
- observability of DMA start, progress, and completion
- scheduler-visible DMA state that bus arbitration can use on the same T-cycle
- transfer fields that already leave room for block or windowed progression without wiring CGB MMIO yet

#### Done criteria

- the transfer is not implemented as an instantaneous memory copy
- OAM DMA is implemented as an instance of the common DMA transfer infrastructure rather than a one-off path outside it
- arbitration correctly reflects DMA effects on concurrent accesses
- CPU-versus-DMA precedence is decided centrally through bus arbitration instead of CPU-local blocking logic
- DMG OAM DMA still leaves the CPU with HRAM access only while active
- DMA lifecycle and active-state visibility are explicit and traceable
- the infrastructure can already represent future block or windowed transfers without requiring a later scheduler redesign, even though GDMA and HDMA remain out of scope here
- the system can trace DMA over time

Status note (`2026-03-19`): the current repo closes `Phase 3.5` and therefore
closes `Phase 3` as a whole. The DMA subsystem now exposes one common transfer
contract with explicit lifecycle/status queries independent of `FF46` readback,
published bus-impact state, and future-family hooks such as `block_size`,
`transfer_family`, and `advance_condition`. OAM DMA runs on the shared T-cycle
timeline as a real `160`-byte bus-routed transfer, and traces carry
start/progress/completion plus the published DMA bus-impact metadata from the
same cycle.

Timing refinement note (`2026-03-19`): source-analysis cross-check against
SameBoy commit `208ba4afabffab9edde416f2dbb8ae459e34adb8` (`Core/memory.c`,
`GB_IO_DMA` setup, `GB_dma_run`, and `is_addr_in_dma_use`) is now reflected in
the repo's DMG OAM-DMA model. The transfer keeps the `640`-T-cycle DMG burst
duration, but exposes an explicit `2`-T-cycle start-up seam before the first
byte commit. The first byte becomes visible at elapsed T-cycle `2`, later bytes
continue every `4` T-cycles, the last byte lands at elapsed T-cycle `638`, and
the final `Completed` transition remains visible after the remaining
`2`-T-cycle tail.

Oracle-validation note (`2026-03-19`): the same SameBoy cross-check also shows
that CPU-side non-HRAM conflict handling is not published during the internal
warm-up markers. The repo therefore now keeps the DMA transfer `in flight`
through the start-up seam while leaving the published CPU bus state
`Unrestricted` until that seam ends, and only then switches to the DMG
source-bus-specific restriction. This keeps bus-impact onset explicit in the DMA timing
contract instead of inferring it from "transfer armed" alone.

#### Recommended sequencing inside Phase 3

Phase `3` should be executed as narrow subphases. No subphase counts as closed
unless its local acceptance criteria land together with focused automated
coverage and move the phase-level DMA/bus/scheduler contract forward without
reintroducing CPU-local blocking logic, instant-copy shortcuts, or DMA-owned
bus decode.

1. `Phase 3.1` - Common DMA transfer contract and `FF46`-owned OAM descriptor.
   Acceptance criteria: the DMA subsystem replaces the ad hoc
   `OamStartRequested`-style placeholder with one typed active-transfer shape,
   `FF46` writes still latch the visible source page immediately, OAM DMA start
   normalization derives the effective `XX00-XX9F` source range plus fixed
   `FE00-FE9F` destination inside DMA-owned code, and the transfer record
   already carries explicit DMG properties such as total length, timing policy,
   CPU-impact policy, and memory-region impact.
   Validation gate: focused unit tests cover `FF46` readback, source-page
   normalization, fixed OAM destination, `160`-byte length, DMG timing-policy
   metadata, CPU-impact metadata, and direct-boot startup state without yet
   requiring whole-machine copy progression.
2. `Phase 3.2` - Scheduler-driven DMA timeline and current-cycle state
   publication.
   Acceptance criteria: DMA gains an autonomous-peripheral `tick` path on the
   shared scheduler timeline, transfer progression becomes explicit per T-cycle
   rather than implicit in one later bulk copy, `Starting -> Active ->
   Completed` becomes observable on that timeline, current-cycle DMA state is
   published before bus arbitration for the same T-cycle, and DMG OAM DMA
   duration is modeled as `640` dots with byte-phase visibility.
   Validation gate: focused unit and integration tests cover state progression
   across `Idle`, `Starting`, `Active`, and `Completed`, `1` byte every `4`
   dots progression metadata, `640`-dot total duration, deterministic stepping,
   and trace visibility for start and completion points.
3. `Phase 3.3` - Central arbitration closure and DMG source-bus-aware CPU behavior.
   Acceptance criteria: the bus consumes one common DMA constraint view instead
   of peeking at `FF46` or transfer internals, CPU-versus-DMA precedence stays
   centralized in arbitration rather than in CPU-local special cases, live DMG
   OAM DMA publishes source-bus-aware CPU blocking while active, and the
   PPU can consume one common OAM-impact signal rather than transfer-specific
   register knowledge.
   Validation gate: focused arbitration tests cover CPU access during active
   DMA for both external-bus and video-bus source shapes, blocked-read and
   ignored-write behavior on the conflicted bus family, unrestricted
   DMA requester access through the same arbitration path, DMA precedence over
   ordinary PPU region-policy checks, and same-cycle coherence between published
   DMA state and the bus decision the CPU observes.
4. `Phase 3.4` - Real OAM data movement through the shared bus model.
   Acceptance criteria: DMA source reads and OAM destination writes happen
   through the same central bus/arbitration model used by the rest of the
   machine, OAM DMA copies the full `160` bytes from the latched source page to
   OAM over time instead of by side effect, transfer-progress state and copied
   bytes remain separately observable on the timeline, and completion clears the
   in-flight transfer state without bypassing lifecycle visibility.
   Validation gate: integration tests cover source-page selection, correct
   `160`-byte copy contents, partial-progress snapshots before completion,
   OAM contents after completion, and completion ordering relative to the last
   transfer T-cycle.
5. `Phase 3.5` - Future transfer-family hooks, observability, and phase
   closure.
   Acceptance criteria: the common DMA API exposes lifecycle and status queries
   without depending on one origin register, the transfer contract already
   carries fields such as `block_size` and `advance_condition` for future
   block/windowed DMA families, traces expose DMA start/progress/completion plus
   published bus-impact state, and the phase closes with explicit TODOs only if
   a concrete remaining gap still blocks full Phase `3` done criteria.
   Validation gate: focused tests cover lifecycle/status visibility, current
   bus-impact publication, and at least one simulated `0x10`-byte block-style
   transfer shape that is not yet wired to real CGB MMIO; before closing the
   phase, the resulting DMA ordering is cross-checked against SameBoy at the
   source-analysis level when a timing-sensitive question remains.

#### Subphase exit rule

Every Phase `3` subphase should end with:

- targeted unit and integration coverage for the newly closed DMA contract
- updated traces or snapshots when observable DMA ordering changes
- `cargo test -q` passing locally at minimum, and `make check` when the
  subphase changes repo tooling or shared workflow-critical infrastructure
- a roadmap TODO recorded immediately if the subphase ships with a concrete
  uncovered gap

#### Risks if delayed too much

- the need to rewrite the bus
- problematic integration with sprites and OAM
- false positives in CPU or PPU behavior because real access conflicts were missing

---

### Phase 4 — Base PPU and visible pipeline

15. **General PPU**
16. **Mode 2 / OAM scan**
17. **Mode 3 / pixel pipeline**
18. **PPU: sprites in detail, priority, BG/OBJ mixing, edge cases**
19. **PPU: window in detail and associated glitches**
20. **PPU: STAT, LY==LYC coincidence, LCD IRQs, and DMG quirks**
21. **PPU: LCD on/off behavior and reactivation**
22. **OAM corruption bug**

#### Goal

Build a truly dot-by-dot PPU, where the visible image emerges from an explicit pipeline based on the fetcher and pixel FIFO.

#### Modules involved

- `ppu/`
- `bus/`
- `scheduler/`
- `dma/`
- `debugger/`

#### Deliverables

##### Base PPU

- general LCD state control
- mode sequencing connected to real timing
- LY and line/dot progression
- direct-boot PPU hidden-state synthesis coherent with the visible post-boot LCD snapshot

##### Mode 2

- OAM scan
- sprite selection for the line
- integration with real temporal restrictions

##### Mode 3

- pixel fetcher
- pixel FIFO
- per-dot pixel production and consumption
- scroll application
- BG, window, and OBJ integration

##### Fine PPU detail

- complete sprite rules
- priorities and mixing
- window and associated glitches
- STAT behavior
- LY==LYC coincidence
- LCD IRQs
- DMG quirks
- LCD on/off and reactivation
- OAM corruption bug

#### Recommended subphase rollout for the current implementation pass

1. `4.1` PPU scheduler spine and explicit state ownership.
   Scope: grow the current register-only PPU baseline into one explicit dot/line/mode state machine with a stable internal source of truth for LCD state, raster position, visible-output state, and direct-boot hidden-state synthesis. If the single-file layout starts obscuring ownership, split `ppu.rs` into focused child modules before timing logic accumulates further.
   Validation: unit tests for MMIO contract preservation, live bus-state derivation, startup-state import, and raster-state reset/snapshot behavior; integration tests that step the shared machine timeline and prove the PPU-visible state stays coherent with scheduler order.
   Exit criteria: one explicit PPU temporal state model exists, later Mode `2` / `3` work does not need to invent parallel counters, and no scanline-level renderer shortcut is introduced.
2. `4.2` Mode `2` scan and line candidate capture.
   Scope: land live Mode `2` timing, current scanline bookkeeping, and the per-line selected-sprite list driven by `Y`, live `LCDC.2`, OAM order, and the hard `10`-sprite limit.
   Validation: unit tests for sprite selection, `8x8` versus `8x16`, off-screen-`X` still counting, and OAM-order preservation; integration tests for OAM access restriction timing composed with existing DMA behavior.
   Exit criteria: the current line's sprite candidates are stable explicit state and the Mode `2` schedule is available for later fetch, mixing, and OAM-corruption work.
3. `4.3` BG-only Mode `3` fetcher, FIFO, and visible pixel output.
   Scope: implement the background fetcher, BG FIFO, per-dot pixel production, scroll discard/application, and a deterministic visible-output path without yet layering sprites or window over a finished scanline image.
   Validation: unit tests for fetch-step progression, FIFO fill/pop invariants, and scroll-driven startup behavior; integration tests with synthetic VRAM fixtures that assert visible pixel sequences and Mode `3`-driven VRAM blocking on the shared timeline.
   Exit criteria: a visible BG-only frame emerges from the real pipeline, Mode `3` is no longer a placeholder duration, and the design keeps a clean seam for later window restarts and OBJ fetch stalls.
4. `4.4` Window activation, fetcher restart, and internal window line counter.
   Scope: add WY latch timing, WX trigger timing, BG FIFO clear plus fetcher restart on window start, the dedicated window line counter, and the first explicit `WX = 0` / `WX = 166` edge paths.
   Validation: unit tests for WY latch semantics, WX trigger timing, and window-line-counter increment rules; integration tests for mid-scanline BG-to-window transition and status-bar style window usage without recomputing the whole line.
   Exit criteria: BG and window share one pipeline, the window starts as a temporal event rather than a scanline compositor, and later OBJ mixing can consume one BG/window stream instead of two ad hoc renderers.
5. `4.5` OBJ fetch, OBJ FIFO, priority, transparency, and BG/OBJ mixing.
   Scope: add object-fetch stalls, explicit OBJ FIFO state, DMG OBJ/OBJ priority, OBJ transparency, BG-over-OBJ handling, and the key clipping/size cases needed for the base DMG sprite model.
   Validation: unit tests for selection-versus-drawing priority, transparent OBJ color `0`, `8x16` row calculation, and partial top/bottom clipping; integration tests for Mode `3` lengthening, object-fetch cancellation boundaries, and window-plus-sprite interaction on the live pipeline.
   Exit criteria: sprites participate inside Mode `3` rather than after it, BG/OBJ mixing is resolved per popped pixel, and sprite timing remains explicit instead of collapsing into a scalar line penalty.
6. `4.6` STAT, `LY`, `LYC`, coincidence, and LCD IRQ closure.
   Scope: implement mixed `STAT` readback, live `LY` / `LYC` coincidence, the internal edge-detected LCD STAT line, real VBlank/LCD IRQ timing, and coherence between the exposed mode bits and bus-facing access policy now that variable Mode `3` timing exists.
   Validation: unit tests for mixed readback, immediate `LYC` reevaluation, rising-edge-only LCD STAT requests, and source blocking; integration tests at machine level for `IF` request timing, `STAT.mode`, and bus restriction coherence during mode transitions.
   Exit criteria: MMIO reads, IRQ requests, and bus policy all observe the same current PPU temporal state, without treating `STAT` sources as unrelated level-triggered checks.
7. `4.7` LCD disable/re-enable, raster restart, and blank-first-frame policy.
   Scope: model `LCDC.7` power transitions, the explicit LCD-disabled state, one documented raster restart state, clean pipeline reset, and the visible blank-first-frame rule after re-enable.
   Validation: unit tests for disabled-state readback, pipeline invalidation, restart-state initialization, and `LY` policy while LCD is off; integration tests for mid-scanline disable/enable, coexistence with DMA-side blocking, and the separation between internal draw restart and panel-visible blank output.
   Exit criteria: the PPU truly turns off and back on in hardware-facing terms, the implementation does not resume stale FIFOs or stale fetch state, and re-enable does not inherit stale `STAT` edge/coincidence state.
8. `4.8` DMG-family OAM corruption bug.
   Scope: expose the live Mode `2` OAM row, route bus and CPU micro-events into one corruption trigger model, implement the deterministic corruption formulas, include the unusable-area path, and keep DMG-family gating explicit.
   Validation: unit tests for row tracking, first-row immunity, read/write/combined corruption formulas, and DMG-versus-CGB family gating; integration tests with CPU-driven trigger sequences and direct bus accesses during live Mode `2`.
   Exit criteria: OAM corruption depends on the live Mode `2` row plus routed events rather than opcode blacklists or generic OAM-blocking shortcuts.

#### Phase 4 interleave policy with earlier open TODOs

- Phase `3` leaves no open TODOs, so DMA is not a sequencing blocker for entering Phase `4`.
- The Phase `2` CPU diagnostic TODO that previously turned unsupported opcodes into a silent non-retiring loop is now closed through one explicit unsupported-opcode diagnostic trap, so deeper Phase `4` ROM or trace debugging no longer fails silently on unknown opcodes.
- The shared Phase `2` CPU subset that Phase `4.8` depends on is now landed ahead of OAM-corruption closure: `[hli]` / `[hld]`, fetch-time `PC` increments, observable `inc rr` / `dec rr`, and the common address-bearing event model reused by stack/control-flow and interrupt-service paths. The remaining boot-facing MMIO transfer shapes stay deferred because they do not block `4.8`.
- The remaining Phase `2` HALT-edge verification and exact same-cycle `TIMA` / `TMA` reload-write arbitration stay deferred. They should not block early Phase `4` bring-up unless a concrete failing test proves a direct dependency.
- If a Phase `4` subphase lands with a deliberately isolated gap, record the remainder in `Open TODOs` immediately instead of carrying it informally into the next graphics task.

#### Subphase exit rule

Every Phase `4` subphase should end with:

- focused unit tests for the local state machine, pipeline step, or register contract that was introduced
- integration tests when the behavior only becomes meaningful across `ppu`, `bus`, `dma`, `interrupts`, or `machine`
- synthetic VRAM/OAM fixtures or retained trace/snapshot coverage when visible pixel order or timing changes
- `cargo test -q` passing locally at minimum, and `make check` whenever the subphase changes shared validation/tooling or other workflow-critical infrastructure
- at least one explicit note about remaining risk when external ROM or oracle validation is still intentionally deferred
- a roadmap TODO recorded immediately if the subphase ships with a concrete uncovered gap

#### Sprite sequencing inside Phase 4

1. Implement Mode 2 sprite selection.
   Scope: vertical match by `Y`, live `LCDC.2` size, OAM order, and the hard `10`-sprite-per-line limit.
   Acceptance criteria: horizontally off-screen sprites still count if their `Y` matches, `8x8` and `8x16` selection are both correct, and OAM discovery order is preserved.
2. Implement DMG OBJ/OBJ priority resolution.
   Scope: choose the winning visible OBJ pixel among overlapping sprite candidates before any BG mixing.
   Acceptance criteria: smaller `X` wins, equal `X` resolves by earlier OAM entry, and BG-over-OBJ does not participate in OBJ/OBJ priority.
3. Implement the object FIFO and transparency rules.
   Scope: object-pixel representation, transparent filler pixels, and OBJ color `0` semantics.
   Acceptance criteria: OBJ color `0` is transparent, transparent OBJ pixels do not block BG, and object FIFO fill behavior is explicit rather than implicit.
4. Integrate object fetch and sprite stalls into Mode 3.
   Scope: in-flight object-fetch state, BG-fetcher interaction, and real sprite-driven Mode 3 lengthening.
   Acceptance criteria: Mode 3 length can increase because of sprite work, BG fetch and object fetch interact on the dot timeline, and the `SCX & 7` plus `X = 0` special path exists as explicit timing-sensitive logic.
5. Implement per-pixel BG/OBJ mixing.
   Scope: popped BG and OBJ pixels, live `LCDC.0` and `LCDC.1`, and DMG BG-over-OBJ semantics.
   Acceptance criteria: BG/OBJ priority is resolved per pixel, BG-over-OBJ is applied only after the winning OBJ pixel is chosen, and DMG `LCDC.0 = 0` behavior remains correct.
6. Cover sprite edge cases and mid-frame toggles.
   Scope: `LCDC.1` fetch cancel, `LCDC.2` size changes, partial vertical clipping, and known `8x16` artifact work.
   Acceptance criteria: object-fetch cancel exists, top and bottom clipping cases are covered, and unresolved `8x16` leak/artifact behavior remains isolated as explicit follow-up work instead of hidden undefined behavior.

#### Window sequencing inside Phase 4

1. Implement basic window activation.
   Scope: WY latch at Mode 2 start, WX trigger during pixel output, and `LCDC.5`-controlled start conditions.
   Acceptance criteria: the window starts only when the WY latch, WX trigger, and window enable are all satisfied, and DMG `LCDC.0 = 0` suppresses window start.
2. Implement fetcher/FIFO reset on window start.
   Scope: BG FIFO clear, fetcher restart, window tilemap selection, and window-local coordinate source.
   Acceptance criteria: the window can begin in the middle of a scanline without recomputing the full line, the visible pixel sequence changes accordingly, and the `WX = 0 && (SCX & 7) > 0` one-dot shortening path is explicit.
3. Implement the internal window line counter.
   Scope: dedicated window Y counter, reset during VBlank, and increment only on scanlines where the window truly starts.
   Acceptance criteria: hiding the window mid-frame can prevent the increment, and status-bar style split usage does not break the chosen window row.
4. Implement WX/WY/LCDC window glitches.
   Scope: glitch pixel behavior around `LCDC.5`, post-start `WX` change behavior, and special handling for `WX = 0` and `WX = 166`.
   Acceptance criteria: the documented glitches exist as pipeline behavior, not framebuffer post-processing, and each case has isolated tests.
5. Integrate window with sprite mixing.
   Scope: BG/window pixel-source transition before final OBJ mixing, without unintended OBJ FIFO resets.
   Acceptance criteria: window start changes the BG/window stream against which OBJ pixels compete, OBJ FIFO state is preserved appropriately, final LCD output still resolves per pixel, and focused tests cover window-plus-sprite interaction without hidden scanline compositing shortcuts.

#### STAT / coincidence / LCD IRQ sequencing inside Phase 4

1. Implement live `LY`, `LYC`, and coincidence state.
   Scope: live `LY` progression through `0..=153`, writable `LYC`, and continuous coincidence evaluation.
   Acceptance criteria: `STAT.2` reflects live `LY==LYC`, coincidence is reevaluated immediately after `LYC` writes, and VBlank lines `144..=153` remain part of the same comparison model.
2. Implement mixed `STAT` readback and enable bits.
   Scope: writable bits `6-3`, live coincidence flag, and live mode bits with LCD-off behavior.
   Acceptance criteria: `STAT` readback composes writable enables plus live bits correctly, mode reads as `0` when LCD is disabled, and software writes cannot overwrite read-only fields.
3. Implement the internal LCD STAT line and rising-edge behavior.
   Scope: OR-composed enabled sources for Mode `0`, Mode `1`, Mode `2`, and coincidence, plus previous-line tracking for edge detection.
   Acceptance criteria: LCD STAT requests occur only on `0 -> 1` transitions of the internal line, Mode `3` is not a direct source, and STAT blocking is reproduced for overlapping enabled conditions.
4. Integrate STAT with the real PPU scheduler and bus-facing access policy.
   Scope: real mode transitions, `LY` progression, Mode `1` entry, and synchronization with VRAM/OAM blocking.
   Acceptance criteria: LCD STAT timing follows the real mode-transition dot, entering VBlank can request both VBlank and LCD STAT Mode `1`, and the mode exposed through `STAT` matches the mode used by the bus for access restrictions.
5. Implement the DMG-family `STAT` write quirk.
   Scope: spurious LCD STAT interrupt behavior on `STAT` writes during Mode `0`, Mode `1`, Mode `2`, and coincidence-active situations.
   Acceptance criteria: the quirk is model-gated to DMG-family behavior, does not appear as a blanket "all `STAT` writes request IRQ" rule, and Mode `3` remains a negative case.
6. Integrate the `STAT`-facing subset of LCD off/on behavior.
   Scope: `LCDC.7` disable/enable effects on reported mode, LCD STAT sources, and VRAM/OAM accessibility, without replacing the broader LCD reactivation work item.
   Acceptance criteria: `STAT.mode = 0` while LCD is disabled, ordinary mode-source behavior is suspended while LCD is off, and re-enable remains compatible with the separate first-full-frame-blank rule handled by the later LCD reactivation block.

#### LCD on/off and reactivation sequencing inside Phase 4

1. Implement real LCD disable.
   Scope: `LCDC.7: 1 -> 0`, explicit LCD/PPU-disabled state, visible LCD-off blank output, and release of ordinary VRAM/OAM mode restrictions.
   Acceptance criteria: disabling LCD stops the active PPU mode scheduler, `STAT.mode = 0`, visible output becomes LCD-off white, ordinary VRAM/OAM mode restrictions are released again without erasing independent DMA-side blocking, and mode-driven LCD STAT requests stop.
2. Implement real LCD re-enable and raster restart.
   Scope: `LCDC.7: 0 -> 1`, one explicit raster-start state, and immediate internal PPU restart.
   Acceptance criteria: the PPU resumes on the shared timeline without an invented startup delay, the re-enable entry point has one documented line/dot/mode source of truth, and the implementation does not resume an old half-finished scanline.
3. Implement visible blank-first-frame behavior.
   Scope: separation between internal pixel generation and panel-visible output after LCD re-enable.
   Acceptance criteria: the internal PPU can start drawing immediately after re-enable while the visible LCD output remains blank for the first full frame, and normal visible output resumes only after that blank frame completes.
4. Reset the pixel pipeline cleanly across LCD power transitions.
   Scope: BG FIFO, OBJ FIFO, fetchers, object-fetch state, window state, and in-progress pixel-mixing state.
   Acceptance criteria: LCD disable invalidates in-flight pixel state explicitly, and LCD re-enable starts from a clean reproducible pipeline instead of resuming stale FIFOs or stale fetch state.
5. Integrate LCD power transitions with bus, LY policy, and mid-scanline writes.
   Scope: synchronized access policy, explicit LY-disabled/re-enable behavior, and immediate `LCDC.7` side effects even outside VBlank.
   Acceptance criteria: bus access policy, `STAT`, `LY`, and the PPU scheduler tell one coherent story during LCD off/on, re-enable does not inherit stale LCD STAT edge/coincidence state, mid-scanline writes remain immediate, and any optional out-of-VBlank warning stays observational only.

#### OAM corruption bug sequencing inside Phase 4

1. Expose the current Mode `2` OAM row.
   Scope: deterministic row tracking for the `20` Mode `2` rows, with one row per `4` dots (`1` M-cycle as a descriptive grouping only).
   Acceptance criteria: the current row is available as live state, matches the real Mode `2` scheduler, and can be consumed by other subsystems without re-deriving it ad hoc.
2. Detect trigger events from bus access and IDU activity.
   Scope: OAM and `FEA0-FEFF` accesses during Mode `2`, plus `16`-bit `inc/dec` activity in `FE00-FEFF`.
   Acceptance criteria: ordinary OAM accesses, `[hli]` / `[hld]`, stack/control-flow sequences, interrupt service, and `PC` increments in OAM all reach one common event model instead of an opcode blacklist.
3. Implement the basic deterministic corruption patterns.
   Scope: distinct read-corruption and write-corruption formulas over the current row and previous row.
   Acceptance criteria: row `0` stays intact, read and write paths remain separate, and the word-copy behavior matches the documented deterministic pattern.
4. Implement combined-event patterns.
   Scope: `write + inc/dec` and `read + inc/dec`.
   Acceptance criteria: `write + inc/dec` collapses to one effective write-corruption result, and `read + inc/dec` uses its dedicated row-restricted path that first mutates the previous row, copies it into the current row and the row two rows before, and then applies the ordinary read-corruption step.
5. Integrate the unusable-area path and model gating.
   Scope: `FEA0-FEFF` reads during Mode `2`, DMG-family enablement, and CGB-family exclusion.
   Acceptance criteria: DMG-family `FEA0-FEFF` reads during the Mode `2` OAM-scan blocked window feed the same controller, outside that window the range keeps its normal DMG readback behavior, and CGB-family models remain unaffected.
6. Close fine validation.
   Scope: row-by-row behavior, first-row immunity, instruction-family coverage, and model-family coverage.
   Acceptance criteria: tests exist for ordinary access triggers, IDU triggers, row exceptions, first-row immunity, and DMG-family positive versus CGB-family negative behavior.

#### Done criteria

- the PPU advances dot-by-dot
- Mode 3 is based on a pixel FIFO rather than deferred scanline rendering
- sprites and window participate inside the real pipeline
- Mode 2 sprite selection, object fetch, OBJ FIFO state, and BG/OBJ mixing are all represented as explicit pipeline behavior rather than scanline-level composition
- DMG sprite priority rules are respected separately for selection, OBJ/OBJ overlap, and BG/OBJ mixing
- window activation, BG-to-window fetch transition, and the internal window line counter are represented as explicit pipeline state rather than as global coordinate remapping
- LCD STAT interrupt generation is represented as one internal edge-detected line driven by live mode/coincidence state rather than as independent level-triggered source checks
- LCD power transitions enter and leave an explicit PPU-disabled state, restart from a defined raster state, and keep the first post-enable blank frame as visible-output behavior rather than a scheduler stall
- DMG-family OAM corruption is modeled from the live Mode `2` row plus bus/CPU micro-events rather than from opcode tables or generic OAM blocking
- STAT, LY, LYC, and LCD IRQs reflect the PPU's real temporal state
- direct-boot LCD-visible state is backed by a coherent internal PPU phase rather than an invented reset-mode shortcut
- bugs and quirks are added on top of an already stable base

#### Risks if oversimplified

- rendering that looks correct but is temporally false
- inability to support real glitches and edge cases
- the need to completely rebuild Mode 3
- incompatibility with tests that depend on FIFO, window, or LCD timings

---

### Phase 5 — Input and simple peripherals

23. **Joypad/input and its relation to interrupts**
24. **Serial port**

#### Goal

Complete basic system peripherals on top of an already consolidated bus, scheduler, and interrupt model.

#### Modules involved

- `joypad/`
- `serial/`
- `bus/`
- `scheduler/`
- `cpu/`
- `debugger/`

#### Deliverables

- joypad register reads/writes
- `JOYP` implemented as a mixed register at `FF00`, with latched row-selection bits and a dynamic active-low low nibble derived from a `2x4` button matrix
- explicit separation between frontend-provided abstract button state and emulated joypad/MMIO state
- visible-edge detection for joypad interrupt generation based on the low nibble actually exposed through `P1`
- input-driven `STOP` wake integration routed through the same joypad subsystem rather than a frontend bypass
- interrupt generation where appropriate
- `SB` and `SC` implemented as serial-owned MMIO state, with serial transfer modeled as a bit-level process rather than an instant byte exchange
- explicit serial-peer boundary for disconnected, loopback, scripted, and future real link peers
- serial interrupt generation driven by real transfer completion rather than by transfer start
- serial port functional at the emulated hardware level
- trace tools to observe both peripherals
- scheduler-visible regression coverage for their IRQ timing and CPU wake interactions

#### Done criteria

- joypad and serial are decoupled from the frontend
- writing `0x30` to `JOYP` makes the visible low nibble read back as `0xF`
- `JOYP` bits `7-6` read back high in the current DMG-family baseline instead of mirroring arbitrary storage
- selecting only one joypad row exposes only that row's buttons in the visible low nibble
- selecting both joypad rows follows one explicit combined-matrix rule for both readback and interrupt detection rather than an invented row priority
- joypad interrupt requests are generated only by visible `High -> Low` transitions on `P1` bits `0-3`, and only when the relevant row selection makes that transition visible
- repeated visible input transitions can request joypad interrupts repeatedly; the model does not assume one interrupt per press
- `STOP` wake-up is driven from the same joypad/input subsystem path used for hardware-visible input state, not by directly toggling CPU state from the frontend
- for the current repo DMG-family baseline, `STOP` wake is a selection-independent `released -> pressed` transition on any hardware-facing button, kept distinct from joypad-interrupt visibility rules
- `SB` changes progressively during active serial transfer instead of remaining frozen until the final byte arrives
- in DMG master mode, serial transfer advances through `8` internally generated clocks at `8192` Hz and clears `SC.7` only on completion
- in slave mode, serial transfer does not advance without externally injected clocks and does not time out internally
- disconnected serial-peer behavior is explicit and tends toward receiving `0xFF`
- serial interrupt requests occur only at transfer completion, when the eighth shift clears `SC.7`
- both are integrated through the bus and scheduler
- their interrupts and states are observable and testable
- their event timing does not depend on frame callbacks or host timers bypassing the T-cycle scheduler

#### Recommended sequencing inside Phase 5

Phase `5` should be executed as narrow subphases. No subphase counts as closed
unless its local acceptance criteria land together with focused automated
coverage and preserve the existing scheduler, bus, and interrupt-controller
contracts instead of reopening them through peripheral-local shortcuts.

1. `Phase 5.1` - Joypad register closure and hardware-facing input boundary.
   Acceptance criteria: `JOYP` remains joypad-owned as a mixed register, the
   frontend-facing API only updates hardware-facing button state instead of
   precomposed `FF00` bytes, bits `7-6` read back high, `0x30` reads back with
   low nibble `0xF`, and selecting one or both rows resolves the low nibble
   from one explicit `2x4` matrix rule rather than from row-priority shortcuts.
   Validation gate: focused unit and MMIO integration tests cover row
   selection, active-low semantics, simultaneous-row combination, direct-boot
   startup state, and the guarantee that selection writes affect readback on
   the same shared machine timeline.
2. `Phase 5.2` - Joypad visible-edge interrupt generation through the shared
   interrupt path.
   Acceptance criteria: joypad tracks the previously visible low nibble, raises
   a request only on visible `High -> Low` transitions after row selection is
   applied, repeated visible transitions can request multiple interrupts, and
   the request enters `IF` only through the shared interrupt controller.
   Validation gate: focused unit and integration tests cover selected-row,
   unselected-row, both-rows-selected, and selection-write-created edge cases,
   plus machine-level verification that `IF` changes only when the visible
   `JOYP` low nibble actually transitions.
   Status: done in the current branch baseline. `Joypad` now owns previous
   visible-low-nibble tracking, both `FF00` selection writes and hardware-side
   button transitions feed the same edge detector, and the resulting request is
   drained into `IF` only during scheduler phase `8` aggregation rather than by
   direct `FF00` or frontend-side mutation of the interrupt controller.
3. `Phase 5.3` - Joypad-driven `STOP` wake closure on the shared scheduler
   timeline.
   Acceptance criteria: the repo's current DMG-family `STOP` wake policy
   remains explicit as selection-independent `released -> pressed` wake on any
   hardware-facing button, that wake originates from the joypad subsystem path
   rather than from frontend or CPU bypasses, and wake ordering stays distinct
   from joypad-interrupt generation even when both happen around the same input
   change.
   Validation gate: focused CPU/joypad integration tests cover `STOP` wake with
   no visible joypad IRQ, `STOP` wake plus later interrupt servicing, repeated
   wake-producing input transitions, and one negative case proving that a
   non-transition or already-held button does not produce an extra wake event.
   Status: done in the current branch baseline. `STOP` wake continues to come
   only from the joypad-owned released-to-pressed path, remains selection
   independent across the `8` hardware-facing buttons, and stays temporally
   distinct from any same-input-change joypad interrupt request or later CPU
   interrupt service.
4. `Phase 5.4` - Serial MMIO closure and explicit transfer-state baseline.
   Acceptance criteria: `SB` and `SC` stay serial-owned, `SC.7` means
   transfer-requested or in-progress rather than instant completion, DMG
   non-functional bits still read high, the serial subsystem exposes one
   explicit in-flight transfer shape with bit count and clock-source state, and
   startup-state injection continues to come from the centralized boot path.
   Validation gate: focused unit and MMIO integration tests cover `SB` / `SC`
   readback, transfer arming without instant completion, internal versus
   external clock selection, direct-boot startup state, and snapshot/debug
   visibility of the new transfer state.
   Status: done in the current branch baseline. `SB` / `SC` remain
   serial-owned, `SC.7` still means transfer requested rather than completed,
   and the serial snapshot/debug surface now exposes one explicit pending
   transfer shape with selected clock mode plus `bits_shifted = 0` ahead of the
   later bit-level engine work in `Phase 5.5`.
5. `Phase 5.5` - Bit-level serial engine, peer boundary, and completion-driven
   IRQ timing.
   Acceptance criteria: DMG master mode advances one serial shift per internal
   clock pulse at `8192` Hz on the T-cycle timeline, slave mode does not
   advance without externally injected clocks, disconnected peers yield incoming
   `1` bits tending toward `0xFF`, `SB` evolves during transfer rather than
   jumping at the end, and the serial interrupt is requested only when the
   eighth shift clears `SC.7`.
   Validation gate: focused unit and integration tests cover intermediate `SB`
   states, master-mode timing, slave-mode pending state, disconnected-peer
   behavior, one loopback or scripted-peer case, and the same-cycle coherence
   of final `SB`, cleared `SC.7`, and serial `IF` request on transfer
   completion.
   Status: done in the current branch baseline. DMG master mode now shifts one
   bit every `512` T-cycles (`8192` Hz), slave mode remains pending without
   externally queued clocks, disconnected input tends toward `0xFF`, loopback
   is explicit through the serial peer boundary, and completion clears `SC.7`
   while requesting the serial interrupt in the same scheduler-visible cycle.
6. `Phase 5.6` - Traceability, regression assets, and phase closure.
   Acceptance criteria: scheduler-visible traces expose joypad selection/input
   edges, joypad IRQ requests, `STOP` wake eligibility, serial start/progress /
   completion, and peer-driven external-clock events; the phase closes only
   once the resulting peripheral behavior is covered by targeted unit tests,
   subsystem integration tests, and retained artifacts where timing visibility
   matters.
   Validation gate: phase-level regression tests retain at least one
   joypad-and-`STOP` timing artifact and one serial timing artifact, and any
   timing-sensitive open question is either cross-checked against a trusted
   oracle or recorded immediately as a roadmap TODO instead of being carried
   informally.
   Status: done in the current branch baseline. Scheduler-visible traces now
   expose joypad state during interrupt aggregation and CPU wake evaluation,
   serial progress during autonomous-peripheral ticks, and retained Phase `5`
   trace fixtures now lock one joypad-plus-`STOP` chronology and one
   peer-driven external-clock serial chronology without introducing any new
   timer-driven open question that would force the deferred Phase `2.7`
   `TIMA` / `TMA` arbitration work into this phase.

#### Joypad implementation breakdown

1. **`JOYP` mixed-register baseline**
   Scope: `FF00` row selection, active-low low-nibble readback, and read/write ownership in `joypad/`.
   Acceptance criteria: `0x30` reads back with low nibble `0xF`, `JOYP` bits `7-6` stay high, selecting buttons versus directions changes which row is visible, and the frontend does not write precomposed `JOYP` bytes directly.
2. **Internal button-matrix state**
   Scope: one hardware-facing state model for all `8` buttons, separated from frontend host input details.
   Acceptance criteria: any button can be pressed or released without touching MMIO directly, and `JOYP` readback derives from that state plus current row selection.
3. **Joypad interrupt generation**
   Scope: visible `High -> Low` detection on `P1` low bits and request routing into `IF`.
   Acceptance criteria: the interrupt appears only when the relevant row is selected; multiple visible transitions can request multiple interrupts; joypad does not bypass the shared interrupt controller.
4. **`STOP` integration**
   Scope: route input-driven wake behavior through the joypad subsystem and CPU `STOP` state interface.
   Acceptance criteria: a `released -> pressed` transition on any hardware-facing button can wake `STOP` regardless of current `JOYP` row selection, and that wake path does not bypass the joypad subsystem.
5. **Focused validation**
   Scope: matrix selection, active-low semantics, simultaneous-row selection, visible-edge IRQ detection, and `STOP` wake behavior.
   Acceptance criteria: tests cover buttons and d-pad separately, both rows selected, visible `High -> Low` detection, repeated input transitions, and the documented repo policy that `STOP` wake is selection-independent while joypad IRQ generation is still selection-dependent.

#### Serial implementation breakdown

1. **`SB` / `SC` MMIO baseline**
   Scope: `FF01`, `FF02`, ownership in `serial/`, DMG control-bit semantics, and non-functional `SC.1` reservation for future CGB work.
   Acceptance criteria: `SB` and `SC` have clear serial ownership, `SC.7` means requested-or-in-progress transfer, `SC.0` selects internal versus external clock, DMG does not expose functional high-speed serial through `SC.1`, and the other non-functional DMG `SC` bits read back high through the routed MMIO contract.
2. **Bit-level master transfer**
   Scope: DMG internal-clock master mode, `8` serial shifts, live `SB` evolution, and completion-driven `SC.7` clear plus IRQ.
   Acceptance criteria: `SB` changes progressively during transfer, the DMG internal clock runs at `8192` Hz on the machine timeline, and transfer completion requests the serial interrupt only after the eighth shift.
3. **Peer boundary and disconnected behavior**
   Scope: explicit serial-peer interface, disconnected input policy, loopback, and scripted peers.
   Acceptance criteria: the core works without a real link peer, disconnected input yields incoming `1` bits and tends toward `0xFF`, and loopback or scripted peers can be attached without direct MMIO byte injection.
4. **Slave mode with external clock**
   Scope: externally driven serial clocks, pending transfer state, and non-uniform pulse timing.
   Acceptance criteria: arming slave mode does not advance transfer on its own, each externally injected clock performs one shift, and the transfer completes only on the eighth external pulse.
5. **Interrupt and scheduler closure**
   Scope: full `SB` / `SC` -> transfer -> `IF` route plus timing-visible reads and writes.
   Acceptance criteria: `IF` receives the serial request at the correct completion point, `SC.7` clears at that same point, and tests cover master mode, slave mode, disconnected peer, loopback or scripted peer, and intermediate `SB` states.

#### Phase 5 interleave policy with earlier open TODOs

- Phase `3` and Phase `5`'s own section currently leave no open TODOs, so DMA and cartridge work are not sequencing blockers for entering the input/peripheral phase.
- The resolved Phase `2.6` `EI ; HALT` pending-IRQ edge no longer blocks `Phase 5.3`; keep using that path as a regression target when extending joypad-driven wake and interrupt coverage so later refactors do not silently reopen it.
- The remaining Phase `2` exact reload-cycle `TIMA` / `TMA` arbitration is still deferred and should stay isolated unless a serial-completion or joypad-interrupt test proves that the shared interrupt timeline is modeled incorrectly for reasons broader than the timer itself.
- The remaining Phase `4` TODOs are validation-grade PPU follow-ups, not architectural blockers for Phase `5`; only interleave one of them if shared scheduler traces, oracle tooling, or retained artifact plumbing can be improved once and reused immediately by the active joypad or serial subphase.
- If a Phase `5` subphase depends on a missing helper, fixture pattern, or trace hook that also resolves a concrete earlier TODO, land that smallest reusable seam first instead of duplicating temporary peripheral-local scaffolding.
- If a Phase `5` subphase lands with a deliberately isolated gap, record the remainder in `Open TODOs` immediately instead of carrying it informally into later cartridge or APU work.

#### Subphase exit rule

Every Phase `5` subphase should end with:

- focused unit tests for the local register contract, edge detector, transfer state machine, or peer boundary that was introduced
- integration tests when the behavior only becomes meaningful across `joypad`, `serial`, `cpu`, `interrupts`, `bus`, or `machine`
- retained trace or snapshot coverage when timing visibility, `STOP` wake ordering, or serial progress would otherwise be hard to audit after a refactor
- `cargo test -q` passing locally at minimum, and `make check` whenever the subphase changes shared validation/tooling or other workflow-critical infrastructure
- at least one explicit note about remaining risk when oracle comparison or external-ROM validation is still intentionally deferred
- a roadmap TODO recorded immediately if the subphase ships with a concrete uncovered gap

---

### Phase 6 — Banked cartridges, special cartridges, and cartridge persistence

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

- standard MBC1 support with explicit wiring validation, immediate access-ordered bank effects, and reserved future MBC1M variant space
- standard MBC2 support with address-bit-`8` control decode, internal `512 x 4-bit` RAM, echo aliasing, and explicit header validation
- banking and RTC support for MBC3
- banking support for MBC5
- special-cartridge taxonomy and unsupported policy covering `MBC30`, multicarts, documented-but-unsupported mapper families, accessory cartridges, and optional heuristics
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
   Status: implementation and local coverage are done in the current branch baseline. Unit and integration tests now cover RAM enable, `0 -> 1`, banks `0x01`, `0x1F`, `0x21`, `0x41`, `0x61`, the `0x20` / `0x40` / `0x60` anomaly, the small-ROM high-region bank-`0` case, mode `0` versus mode `1`, `8 KiB` versus `32 KiB` RAM behavior, and configuration diagnostics. Phase `6` also now ships retained synthetic MBC1 ROM and trace fixtures for standard banking and small-ROM masking plus RAM banking. External oracle comparison for the bank-selection edge cases is still deferred and should be tracked as validation debt rather than as a behavior blocker for entering `MBC2`.

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
   Status: implementation and local coverage are done in the current branch baseline. Unit tests, integration tests, one retained synthetic Phase `6` ROM/trace, and the shared cartridge-persistence round-trip coverage now cover address-bit-`8` control decode, bank `0 -> 1`, ROM-size diagnostics, echo aliasing, low-nibble storage, the chosen high-nibble readback policy, battery-backed nibble-RAM persistence, and `0x0149` validation. External oracle comparison for the MBC2 control-decode and nibble-RAM edge cases is still deferred.

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
   Status: local implementation coverage is done in the current branch baseline. Unit tests, integration tests, and one retained synthetic Phase `6` ROM/trace now cover header types `0x0F`, `0x10`, `0x11`, `0x12`, and `0x13`, raw ROM-bank `0 -> 1`, banks `0x20`, `0x40`, and `0x60`, RAM-versus-RTC selector behavior, latch sequencing, stable snapshots, halt / carry / day overflow, and MBC30 reservation diagnostics. External oracle comparison is still deferred, and the optional `16`-T-cycle / `4 us` RTC access-spacing note remains recorded as future validation work rather than active runtime behavior.

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
   Status: local implementation coverage is done in the current branch baseline. Unit tests, integration tests, and one retained synthetic Phase `6` ROM/trace now cover header types `0x19..=0x1E`, bank `0` visibility in the switchable region, bank `0x1FF`, the `0xFF -> 0x100` boundary, RAM-enable behavior, SRAM behavior for `32 KiB` and `128 KiB`, RAM-bank masking, rumble on/off, and size-validation diagnostics. External oracle comparison is still deferred.

#### Special-cartridge and unsupported-policy sequencing inside Phase 6

1. Establish the special-cartridge taxonomy and unsupported categories.
   Scope: one central classification path for `Supported`, `PlannedVariant`, `DocumentedButUnsupported`, `ExperimentalHeuristic`, `AccessorySpecialCase`, and `UnknownCode`, plus stable names for `MBC30`, `MBC1M`, `MMM01`, `M161`, `HuC1`, `HuC-3`, `MBC6`, `MBC7`, `Pocket Camera`, `Bandai TAMA5`, `EMS`, `Bung`, and `Wisdom Tree`.
   Acceptance criteria: the loader produces stable classification for all of those cases, the frontend does not need to reparse headers to explain them, and the classification preserves raw `0x0147`, detected name, category, and reason.
   Status: partially done in the current branch baseline. The loader now owns one central classification path covering `Supported`, `PlannedVariant`, `DocumentedButUnsupported`, `ExperimentalHeuristic`, `AccessorySpecialCase`, and `UnknownCode`, with stable names and reasons for header-coded special families plus the currently wired experimental heuristics for `EMS`, `Bung`, and `Wisdom Tree`. Explicit `M161` identification is still open because no trusted detection rule is wired yet.
2. Add explicit `MBC30` detection.
   Scope: detect the `MBC3`-family plus `64 KiB` SRAM case as `MBC30`, return a typed planned variant or supported variant entry point instead of ordinary standard `MBC3`, and reserve matching persistence / banking work.
   Acceptance criteria: `MBC3 + 64 KiB SRAM` never falls through to standard `MBC3`, loader diagnostics name `MBC30` explicitly, and the code path is ready for future concrete `MBC30` implementation.
   Status: done in the current branch baseline. `MBC3`-family headers with `64 KiB` SRAM now classify as explicit `MBC30` planned variants, fail as known reserved variants rather than as invalid standard `MBC3`, and keep the future concrete implementation path explicit.
3. Add multicart and near-variant classification.
   Scope: classify `MMM01` from `0x0B..=0x0D`, reserve future `MBC1M` as a distinct `MBC1`-family variant, keep `M161` in a multicart-special path, and avoid assuming that `MMM01` boot/header handling is identical to standard cartridges.
   Acceptance criteria: `0x0B`, `0x0C`, and `0x0D` are emitted as `MMM01`, `MBC1M` remains a separate future variant instead of being merged into standard `MBC1`, and multicarts do not silently degrade to ordinary `MBC1` or `NoMbc`.
   Status: partially done in the current branch baseline. Header-coded `MMM01` variants already classify explicitly as multicart-special documented-but-unsupported hardware, and standard `MBC1` still reserves a distinct internal future `MBC1M` variant instead of merging those rules into ordinary `MBC1`. Explicit `M161` identification remains open and is tracked separately as a Phase `6` TODO.
4. Enforce controlled failure for documented special hardware.
   Scope: explicit diagnostics for `HuC1`, `HuC-3`, `MBC6`, `MBC7`, `Pocket Camera`, `Bandai TAMA5`, and `M161`, plus a hard rule against automatic fallback to `MBC1`, `MBC3`, `MBC5`, or other nearby supported mappers.
   Acceptance criteria: those types fail with clear messages naming the exact detected cartridge, `UnknownCode` reports the raw `0x0147` byte, and no silent degradations or fake "best effort" mapper substitutions remain.
   Status: partially done in the current branch baseline. `HuC1`, `HuC-3`, `MBC6`, `MBC7`, `Pocket Camera`, `Bandai TAMA5`, and generic `UnknownCode` values now fail with explicit typed diagnostics and without silent fallback to nearby supported mappers. `M161` is the remaining gap because its identification path is not wired yet.
5. Add optional experimental heuristic mode.
   Scope: isolate `EMS`, `Bung`, and `Wisdom Tree` detection behind an explicit dev / experimental loader policy, keep strict default behavior header-driven, and document that heuristic paths are lower priority than `MBC30`, multicarts, and documented special hardware.
   Acceptance criteria: heuristic detection is off by default, can be enabled explicitly for development and research, and diagnostics clearly state when a classification came from heuristics instead of a standard header mapping.
   Status: done in the current branch baseline. The loader now keeps heuristic detection disabled in `Strict` and `Permissive`, while `Experimental` with heuristics enabled can reclassify the currently wired `EMS`, `Bung`, and `Wisdom Tree` signatures into explicit `ExperimentalHeuristic` results whose rejection reasons say that the classification came from an experimental heuristic path.

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

---

### Phase 7 — Audio

31. **General APU architecture**
32. **APU frame sequencer**
33. **APU channel 1**
34. **APU channel 2**
35. **APU channel 3**
36. **APU channel 4**
37. **Mixing, output, DACs, power control, and audio edge cases**

#### Goal

Build the audio subsystem as a real temporal part of the hardware, integrated with the scheduler but decoupled from each frontend's concrete audio output.

#### Modules involved

- `apu/`
- `scheduler/`
- `bus/`
- `debugger/`
- frontend audio adapters

#### Deliverables

- base APU architecture
- separate implementation of each channel
- functional frame sequencer
- direct-boot APU hidden-state synthesis coherent with the visible post-boot audio snapshot
- final mixing
- DAC control
- power control
- audio edge cases
- clean interface between `gb-core` and frontend audio adapters

#### Base APU / frame sequencer sequencing inside Phase 7

1. Establish the master APU skeleton.
   Scope: `Apu` ownership of `NR50`, `NR51`, `NR52`, powered state, left/right internal outputs, and placeholder HPF state.
   Acceptance criteria: `NR52` power on/off behavior is centralized, wave RAM remains outside the ordinary power-reset path, and the live low `NR52` bits already represent channel-active state rather than DAC-enabled state.
2. Integrate `DIV-APU` / frame-sequencer timing.
   Scope: derive `div_apu` from the shared divider timeline, using the current DMG falling-edge source on `DIV` bit `4`, emit slow clocks for length, CH1 sweep, and envelope, and leave room for coherent direct-boot entry.
   Acceptance criteria: writes to `DIV` can produce the documented extra frame-sequencer tick when the edge occurs, the APU slow clocks remain derived from the same divider source as visible `DIV`, and direct-boot audio entry can synthesize a coherent `DIV-APU` / frame-sequencer phase instead of restarting audio timing from zero.
3. Separate DAC state from channel-active state and centralize trigger behavior.
   Scope: explicit `dac_enabled` versus `channel_active`, shared trigger handling from `NRx4` bit `7`, and DAC-off forcing channel-off.
   Acceptance criteria: triggers do not activate channels whose DAC is off, DAC-disable can deactivate a live channel immediately, and `NR52` reports live active channels rather than DAC-enabled channels.
4. Build the base stereo mixer.
   Scope: per-channel routing through `NR51`, left/right master-volume scaling through `NR50`, and internal left/right analog-output accumulation.
   Acceptance criteria: stereo routing is correct, `NR50` follows the documented "0 means factor 1, 7 means factor 8" behavior, and the architecture does not confuse master volume with mute.
5. Add the output HPF layer.
   Scope: left/right HPF state in the analog-output path after mixing and master-volume scaling.
   Acceptance criteria: the pipeline has an explicit place for DC-offset and pop-sensitive behavior, and HPF presence no longer depends on frontend audio code.
6. Prepare the channel blocks without collapsing the timing model.
   Scope: stable hooks for CH1-CH4 slow clocks and fast timers, plus follow-up placeholders for channel-specific quirks and edge cases.
   Acceptance criteria: each channel can later receive its own waveform timer without changing the master frame-sequencer architecture, and known follow-up work such as extra length clocking, CH3 wave-RAM quirks, CH4 lock-up, and envelope zombie-mode remains explicitly tracked rather than implicit.

#### CH1 sequencing inside Phase 7

1. Establish CH1 state ownership and MMIO routing.
   Scope: CH1-owned `NR10`-`NR14`, explicit channel state, and write-only/read-only field policy.
   Acceptance criteria: `NR13` remains write-only, `NR14` bit `7` acts as trigger, `NR14` bit `6` acts as immediate length enable, and CH1 ownership is not split informally across generic APU helpers.
2. Implement CH1 period timer and duty stepping.
   Scope: `11`-bit period value, fast period timer, selected duty waveform, and non-resetting duty-step counter.
   Acceptance criteria: the pulse timer advances once every `4` dots on DMG, the waveform is `8` steps long, retrigger resets the timer but not duty step, and period writes take effect only after the current sample ends.
3. Implement CH1 DAC state and general trigger behavior.
   Scope: `dac_enabled`, `channel_active`, trigger-time state reload, and `NR52` bit `0` integration.
   Acceptance criteria: DAC-off disables CH1 immediately, trigger does nothing if DAC is off, and CH1 trigger resets the documented period/envelope/sweep state in one explicit path.
4. Integrate CH1 length and envelope.
   Scope: `64`-step length counter, `256` Hz length clock, `64` Hz envelope clock, current-volume state, and immediate `NR14` length-enable behavior.
   Acceptance criteria: length expiry disables CH1, envelope changes current volume without mutating readable `NR12` bits, envelope volume reaching `0` does not disable CH1, and extra-length-clocking behavior is either implemented or isolated as explicit follow-up logic.
5. Implement full CH1 sweep behavior.
   Scope: shadow period, sweep timer, enabled flag, trigger-time setup, timed sweep iterations, writeback, and second overflow check.
   Acceptance criteria: trigger copies the shadow period and performs the immediate overflow check when required, sweep ticks perform writeback plus the second overflow check, and writes to `NR13` / `NR14` do not refresh the sweep shadow automatically.
6. Close CH1 quirks and fine validation.
   Scope: envelope/sweep timer-reload semantics where programmed pace or period `0` behaves as `8`, low frequency-timer bits on trigger, first-duty-step-after-power-on behavior, and any remaining documented CH1 trigger/length edge cases.
   Acceptance criteria: quirks are isolated behind explicit channel logic and tests, rather than leaking into the general APU architecture.

#### CH2 sequencing inside Phase 7

1. Establish CH2 state ownership and MMIO routing.
   Scope: CH2-owned `NR21`-`NR24`, explicit channel state, and write-only/read-only field policy without any sweep-only carryover.
   Acceptance criteria: `NR23` remains write-only, `NR24` bit `7` acts as trigger, `NR24` bit `6` acts as immediate length enable, and CH2 does not accumulate dummy sweep state just because it shares pulse-channel infrastructure with CH1.
2. Implement CH2 period timer and duty stepping.
   Scope: `11`-bit period value, fast period timer, selected duty waveform, and non-resetting duty-step counter.
   Acceptance criteria: the pulse timer advances once every `4` dots on DMG, the waveform is `8` steps long, retrigger resets the timer but not duty step, and period writes take effect only after the current sample ends.
3. Implement CH2 DAC state and general trigger behavior.
   Scope: `dac_enabled`, `channel_active`, trigger-time state reload, and `NR52` bit `1` integration.
   Acceptance criteria: DAC-off disables CH2 immediately, trigger does nothing if DAC is off, and CH2 trigger resets the documented period/envelope state in one explicit path.
4. Integrate CH2 length and envelope.
   Scope: `64`-step length counter, `256` Hz length clock, `64` Hz envelope clock, current-volume state, and immediate `NR24` length-enable behavior.
   Acceptance criteria: length expiry disables CH2, envelope changes current volume without mutating readable `NR22` bits, envelope volume reaching `0` does not disable CH2, and extra-length-clocking behavior is either implemented or isolated as explicit follow-up logic using the same infrastructure as CH1.
5. Close CH2 shared pulse quirks and fine validation.
   Scope: envelope timer-reload semantics where programmed pace or period `0` behaves as `8`, low frequency-timer bits on trigger, first-duty-step-after-power-on behavior, and any remaining documented CH2 trigger/length edge cases.
   Acceptance criteria: quirks are isolated behind explicit channel logic and tests, and CH2 remains architecturally simpler than CH1 because no sweep-specific state or flow leaked into it.

#### CH3 sequencing inside Phase 7

1. Establish CH3 state ownership, MMIO routing, and wave RAM.
   Scope: CH3-owned `NR30`-`NR34`, explicit channel state, write-only/read-only field policy, and explicit `16`-byte wave RAM ownership.
   Acceptance criteria: `NR31` and `NR33` remain write-only, `NR34` bit `7` acts as trigger, `NR34` bit `6` acts as immediate length enable, wave RAM is visible through its MMIO path, and wave RAM persists across `NR52` power-off.
2. Implement CH3 period timer, sample index, and sample buffer.
   Scope: `11`-bit period value, fast period timer, `32`-sample index progression, buffered sample fetch from wave RAM, and delayed application of period writes.
   Acceptance criteria: the timer advances once every `2` dots on DMG, the sample index traverses `32` logical samples, buffered output comes from fetched wave-RAM nibbles rather than direct live reads, and period writes take effect only after the next wave-RAM read boundary.
3. Implement CH3 DAC state and general trigger behavior.
   Scope: `dac_enabled`, `channel_active`, trigger-time timer/index reload, sample-buffer preservation, and `NR52` bit `2` integration.
   Acceptance criteria: DAC-off disables CH3 immediately, trigger does nothing if DAC is off, trigger resets the documented timer/index state in one explicit path, retrigger does not clear or refill the sample buffer automatically, and `NR52` bit `2` reflects live CH3 activity.
4. Integrate CH3 length and output level.
   Scope: `256`-step length counter, `256` Hz length clock, `NR32` digital attenuation rules, and immediate `NR34` length-enable behavior.
   Acceptance criteria: length expiry disables CH3, `NR32` mute and shift semantics are correct, `NR32` mute is not confused with DAC-off, and trigger-with-length-0 behavior remains either implemented or isolated as explicit follow-up logic.
5. Close CH3 quirks, active-wave-RAM policy, and DMG retrigger corruption.
   Scope: digital-`0` startup state, skipped-first-sample / first-buffer behavior, wave-RAM access policy while active, and DMG-family wave-RAM corruption on retrigger.
   Acceptance criteria: quirks remain isolated behind explicit CH3 state and tests, active-wave-RAM policy is not hidden behind generic RAM behavior, and retrigger corruption distinguishes the special first-byte overwrite case for reads in bytes `0..=3` from the aligned-`4`-byte block-copy cases for reads in bytes `4..=15`.

#### CH4 sequencing inside Phase 7

1. Establish CH4 state ownership and MMIO routing.
   Scope: CH4-owned `NR41`-`NR44`, explicit channel state, and write-only/read-only field policy.
   Acceptance criteria: `NR41` remains write-only, `NR44` bit `7` acts as trigger, `NR44` bit `6` acts as immediate length enable, and CH4 ownership is not split informally across generic APU helpers.
2. Implement CH4 LFSR, `noise_timer`, and `NR43` decoding.
   Scope: explicit `lfsr_state`, explicit fast timer, decoded clock shift / width mode / divider state, and the shared `15`-bit versus `7`-bit LFSR path.
   Acceptance criteria: the ordinary `15`-bit and `7`-bit paths are both correct, divider `0` is treated as `0.5`, clock-shift values `14` and `15` suppress CH4 clocks, and live `NR43` writes alter timer behavior without mutating CH4 into a texture-swap abstraction.
3. Implement CH4 DAC state and general trigger behavior.
   Scope: `dac_enabled`, `channel_active`, trigger-time state reload, lock-up recovery on retrigger, and `NR52` bit `3` integration.
   Acceptance criteria: DAC-off disables CH4 immediately, trigger does nothing if DAC is off, CH4 trigger resets the documented envelope/LFSR/timer state in one explicit path, retrigger exits LFSR lock-up, and `NR52` bit `3` reflects live CH4 activity rather than mere audibility.
4. Integrate CH4 length and envelope.
   Scope: `64`-step length counter, `256` Hz length clock, `64` Hz envelope clock, current-volume state, and immediate `NR44` length-enable behavior.
   Acceptance criteria: length expiry disables CH4, envelope changes current volume without mutating readable `NR42` bits, envelope volume reaching `0` does not disable CH4, and extra-length-clocking behavior is either implemented or isolated as explicit follow-up logic using the same infrastructure as CH1 / CH2.
5. Close CH4 lock-up and fine validation.
   Scope: width-mode transition quirks, documented lock-up on `15 -> 7` in the relevant all-ones states, retrigger recovery, and any remaining CH4 trigger/length edge cases.
   Acceptance criteria: lock-up remains a consequence of real LFSR state rather than an ad hoc mute flag, retrigger recovers sound by resetting the LFSR, and the remaining CH4 quirks are isolated behind explicit channel logic and tests.

#### Final output and host-boundary sequencing inside Phase 7

1. Introduce the explicit DAC layer.
   Scope: resolved channel digital outputs in the hardware `0..15` domain, per-channel DAC conversion, and an explicit DAC-off path distinct from ordinary enabled-DAC conversion.
   Acceptance criteria: enabled-DAC conversion follows the documented negative-slope `0..15 -> -1..1` mapping, DAC-off remains distinct from "inactive channel with DAC still enabled", and the master mixer now consumes analog channel outputs instead of raw digital values.
2. Build the stereo mixer and `NR51` routing.
   Scope: left/right analog buses, per-channel routing under `NR51`, and immediate routing changes on the shared timeline.
   Acceptance criteria: each channel can route to left, right, both, or neither; `NR51` writes are immediate; and routing is modeled as analog-bus inclusion rather than as an external mute shortcut.
3. Integrate `NR50` master-volume scaling and output-side power-state coherence.
   Scope: per-output master-volume scaling, explicit `VIN` slot, and the effect of `NR52` power-off on active mix contributions.
   Acceptance criteria: `NR50` level `0` does not mute, maximum volume follows the documented highest factor, and powering the APU off removes active channel contributions from the live mix while preserving wave RAM and `DIV-APU`.
4. Add the output HPF and DC-offset / pop behavior.
   Scope: one stateful HPF per stereo output after routing and `NR50`, plus documented pop behavior from DAC-enable, `NR51`, and `NR50` changes.
   Acceptance criteria: left/right HPF state persists across captured samples, output converges back toward neutral DC offset, documented pops emerge from the modeled signal path, and HPF absence remains at most a debug-only bypass.
5. Separate the T-cycle-accurate APU core from the host-facing sample/export boundary.
   Scope: explicit post-HPF analog-output exposure, sample-capture policy, host resampler/export boundary, and final normalization / format conversion outside the hardware model.
   Acceptance criteria: changing host sample rate does not change the core APU model, the core can run deterministically in tests without a real audio backend, and host-side conversion no longer owns hardware semantics such as mixing, HPF behavior, or pop generation.
6. Close final output-path integration and validation.
   Scope: end-to-end `DAC -> mixer -> NR50 -> HPF -> host-facing export boundary` behavior under dynamic routing, volume, DAC, and power changes.
   Acceptance criteria: `NR50`, `NR51`, `NR52`, and DAC-enable changes all affect the final stereo path coherently, pop-producing transitions are covered by tests, HPF behavior is deterministic, and the final host-facing export layer preserves rather than rewrites the hardware model.

#### Done criteria

- each channel is independently verifiable
- the frame sequencer coordinates the subsystem correctly
- mixing and DACs are implemented on top of a stable channel base
- direct-boot audio-visible state is backed by a coherent internal APU phase rather than by a disconnected visible-only snapshot
- the core does not depend on a concrete frontend audio backend

#### Risks if introduced too early

- effort dispersion while CPU/PPU/bus are not yet closed
- difficulty isolating bugs if the base system is still unstable

---

### Phase 8 — Full emulator save states and global serialization strategy

38. **Whole-machine snapshot contract and ownership**
39. **Global serialization envelope, versioning, and metadata**
40. **Core save/load restore path and validation**

#### Goal

Establish one explicit full-emulator save-state system, separate from cartridge persistence, only after the hardware subsystems already own their live runtime state and before final DMG closure depends on save/load determinism.
Phase `6` cartridge persistence is intentionally not a substitute for this whole-machine snapshot block.

#### Modules involved

- `scheduler/`
- `cpu/`
- `bus/`
- `ppu/`
- `dma/`
- `timer/`
- `apu/`
- `joypad/`
- `serial/`
- `cartridge/`
- `boot/`
- `debugger/`
- frontend/tooling persistence adapters

#### Deliverables

- typed whole-machine save-state contracts with explicit ownership by subsystem
- explicit capture and restore of hidden temporal state such as scheduler phase, CPU internal execution state, DMA lifecycle, PPU pipeline state, timer hidden counters, serial in-flight transfer state, and APU frame-sequencer / channel / HPF state
- cartridge runtime snapshot integration for continued execution, kept distinct from cartridge-local persistent-storage payloads
- one global versioned serialization envelope with mandatory model, execution mode, active override, and compatibility metadata
- portable core-facing save/load API boundaries that do not couple `gb-core` to disk, desktop, web, or tool-specific storage APIs
- debugger and tooling hooks for capture, restore, and structured snapshot inspection
- automated round-trip and continuation validation for full-machine save/load

#### Done criteria

- every subsystem that owns live machine state exposes an explicit save/restore contract rather than relying on ad hoc debugger dumps or MMIO readback reconstruction
- restoring a save state recreates in-flight temporal state coherently instead of replaying writes to rebuild hidden state from visible registers alone
- whole-machine save states remain clearly separate from cartridge persistence, with cartridge runtime state included only through cartridge-owned snapshot semantics
- save-state metadata records execution mode and active overrides, and mismatched-mode restore is rejected by default
- the serialization contract is versioned and portable across CLI, desktop, web, tests, and tooling without moving host storage policy into the emulation core
- tests cover exact round-trip restore plus save/load continuation versus uninterrupted execution, including at least one timing-sensitive mid-run restore case
- debugger and tooling capture or load states through the same core-owned contract instead of through parallel bespoke snapshot paths

#### Recommended sequencing inside Phase 8

1. Define the whole-machine snapshot boundary and invariants.
   Scope: separation between emulator save states, cartridge persistence, debugger snapshots, and later replay metadata, plus required metadata such as model, execution mode, overrides, and startup context.
   Acceptance criteria: ownership of every saved field is assigned to one subsystem, save states are explicitly distinct from battery persistence, and no host-storage concern leaks into subsystem snapshot semantics.
2. Add typed subsystem snapshot contracts.
   Scope: scheduler, CPU, bus-visible mapping, memory, DMA, PPU pipeline, timer hidden state, APU internals, joypad, serial, boot/startup context, and cartridge runtime state needed for continued execution.
   Acceptance criteria: subsystems export and import explicit typed state, in-flight temporal details are preserved, and restore does not depend on replaying MMIO writes to reconstruct hidden state.
3. Define the global serialization envelope.
   Scope: versioned schema, canonical metadata for execution mode and overrides, and room for future compatibility evolution without silent breakage.
   Acceptance criteria: incompatible versions fail clearly, mode and override metadata are mandatory, and the format remains suitable for disk, in-memory, and web backends without changing core semantics.
4. Implement capture and restore through the core boundary.
   Scope: one authoritative save/load API, compatibility validation for model, cartridge, and mode metadata, and integration points for debugger and frontends.
   Acceptance criteria: save/load goes through one authoritative path, mismatched-mode restore is rejected by default, and restore resumes the shared T-cycle timeline from the recorded point rather than from a reconstructed approximation.
5. Lock round-trip and continuation validation.
   Scope: focused restore tests per subsystem, integration tests for continued execution equivalence, and at least one strict-mode end-to-end save/load case with banked-cartridge and active timing-sensitive subsystem coverage.
   Acceptance criteria: exact round-trip invariants are checked automatically, save/load continuation matches uninterrupted execution in covered scenarios, and failures retain enough metadata or snapshots to localize the first divergence quickly.

#### Risks if delayed or underspecified

- final hardening work lacks a stable save/load foundation
- frontend-specific storage decisions leak into core semantics
- restore paths reconstruct only visible registers and lose hidden temporal state
- cartridge persistence and whole-machine save states become conflated
- debugger or replay tooling grows a second incompatible serialization path

---

### Phase 9 — Final DMG hardening, differential validation, and closure

This phase is the roadmap home for the final DMG closure work. Parts of it should begin earlier, but the block only closes once the project can justify DMG correctness through layered evidence on the shared T-cycle model rather than through informal game compatibility.
It assumes the dedicated save-state and serialization infrastructure from Phase 8 already exists and uses it as part of closure evidence.

Status note (`2026-03-22`): the repo now starts a narrow early hardening lane
from this phase before APU work. The current early deliverables are:

- one explicit partial subsystem checklist in `AI/TESTING.md` that distinguishes
  repo-gated external evidence from internal-only evidence for the already-landed
  DMG subsystems
- one `gb-test-runner` catalog path,
  `cargo run -p gb-test-runner --bin run_rom_suite -- --list-detailed`, that
  exposes the built-in suite set together with oracle channel, capture, and
  retained-artifact policy
- one `gb-test-runner` checklist path,
  `cargo run -p gb-test-runner --bin run_rom_suite -- --early-checklist`, that
  exposes the current early hardening status per subsystem together with the
  evidence already landed and the still-open closure gaps
- one repo-gated PPU framebuffer-oracle suite,
  `cargo run -p gb-test-runner --bin run_rom_suite -- --suite acid-dmg-curated`,
  sourced from `GBEmulatorShootout` and now part of the supported external DMG
  block used by `make test` and the `external-roms` workflow
- one exploratory PPU framebuffer-oracle suite,
  `cargo run -p gb-test-runner --bin run_rom_suite -- --suite mealybug-tearoom-dmg-curated [--failure-artifact-root <dir>]`,
  which uses a curated DMG subset from `GBEmulatorShootout` and the same
  committed-PNG oracle contract as `dmg-acid2`, but is currently red under
  `Strict` and therefore remains outside the supported external DMG block
- one exploratory DMG acceptance suite,
  `cargo run -p gb-test-runner --bin run_rom_suite -- --suite mooneye-acceptance-dmg-curated [--failure-artifact-root <dir>]`,
  which follows the active `GBEmulatorShootout` `testroms/mooneye.py`
  acceptance list, uses the upstream `mooneye` breakpoint/register result
  protocol instead of framebuffer fixtures, and currently stays outside the
  supported external DMG block while its failures are triaged one by one
- one narrow differential end-of-test path,
  `cargo run -p gb-test-runner --bin run_differential -- --oracle sameboy [--oracle-layout <case-bundle|sameboy-tester>] [--oracle-artifact-root <dir>] --suite <suite-name>`,
  which compares the built-in suite's required-capture artifact against an
  imported oracle artifact bundle, enforces `Strict`, and archives local
  context plus the compared oracle artifact on divergence. The current path
  also reports the first differing byte or pixel inside the compared final
  artifact, even though full instruction-level or short-window first-divergence
  tooling is still deferred. The current `sameboy-tester` layout support is
  intentionally framebuffer-only and is aimed at PPU/image-oracle cases such as
  `dmg-acid2`. When the oracle root is omitted, the repo-local default is
  `/.oracles/<oracle>/<layout>/`
- one SameBoy Tester materialization path,
  `cargo run -p gb-test-runner --bin run_sameboy_tester -- --suite <suite-name> [--oracle-root <dir>] [--sameboy-root <dir> | --tester-binary <path>]`,
  which stages ROMs under the oracle root, runs SameBoy's internal `tester`
  target, and produces `.bmp` / `.tga` plus `.log` artifacts in the exact
  `sameboy-tester` layout consumed by `run_differential`. The repo-local
  default for this path is `/.oracles/sameboy/sameboy-tester/` for oracle
  outputs, and the wrapper intentionally leaves SameBoy's own boot-ROM path
  under SameBoy's control instead of trying to share local firmware selection
  with `gb-test-runner`

This does not count as closing Phase `9.2` or `9.3`; fuller SameBoy
differential launch automation, first-divergence windows, save/load determinism,
and the final DMG matrix still remain Phase `7/8/9` work.

#### Goal

Close the DMG core with a formal validation matrix, strong differential and determinism tooling, and explicit closure criteria that leave no major blind hardware areas behind.

#### Modules involved

- `tests/`
- `gb-test-runner/`
- `debugger/`
- `scheduler/`
- subsystem cores as needed for per-area traces and inspections
- frontend or tooling adapters only where they are needed for capture, visualization, or artifact export

#### Deliverables

- formal DMG hardening matrix with layers `A/B/C/D/E`, severity classes, and explicit `must-pass` areas
- automated external-ROM harness with timeout, pass/fail policy, framebuffer and serial capture, and retained failure artifacts
- differential comparison tooling for SameBoy with first-divergence reporting and short T-cycle windows
- deterministic replay, save/load determinism, and longer-running soak coverage
- minimum closure-ready debugging tooling: traces, breakpoints, watchpoints, snapshots, and targeted subsystem viewers
- explicit DMG closure checklist covering internal suites, external suites, differential comparison, determinism, save/load determinism, and primary cartridge families

#### Recommended sequencing inside Phase 9

1. Formalize the DMG hardening matrix and closure severity policy.
   Scope: define layers `A/B/C/D/E`, `must-pass` versus non-blocking categories, minimum DMG closure suites, and the rule that no single layer substitutes for another.
   Acceptance criteria: the project docs name the closure layers explicitly, identify the blocking hardware areas for DMG closure, and define a stable checklist instead of relying on informal compatibility claims.
2. Build the external ROM harness and minimum closure suites.
   Scope: automate CPU / interrupt ROMs, `dmg-acid2`, and `mealybug-tearoom-tests`; support framebuffer and serial capture; define timeouts, pass/fail rules, and retained artifacts; and keep explicit reserved follow-up slots for broader closure suites such as Mooneye / Gekkio coverage, SameSuite, GB Accuracy Tests, 144p Test Suite, and MBC3 RTC-focused ROMs.
   Acceptance criteria: the minimum DMG closure ROM suites run without manual screen inspection, every case has a timeout plus explicit pass/fail policy, and the harness can preserve enough output to debug failures offline.
3. Add differential comparison against SameBoy.
   Scope: end-of-test comparison, end-of-instruction comparison, short T-cycle-window comparison, and first-divergence localization with archived context.
   Acceptance criteria: SameBoy acts as the DMG oracle for the covered scenarios, and the tooling can report the first divergence instead of only a final mismatch.
4. Close the minimum debugging and inspection tooling.
   Scope: instruction / micro-op / short-window T-cycle tracing, breakpoints and watchpoints on `PC`, memory, MMIO, and cartridge-bank state, plus fast inspection of CPU, scheduler, bus owner, PPU mode / dot / `LY`, DMA, timer, APU, and cartridge / MBC state.
   Acceptance criteria: a blocking divergence can be localized without a long blind rerun, and the project has practical viewers or equivalent dumps for PPU, cartridge / MBC, APU, and IRQ state.
5. Lock determinism, replay, save/load determinism, soak, and regression retention.
   Scope: same-ROM replay with identical execution mode, explicit overrides, input stream, and injected time source, mid-run save/load equivalence, longer-running soak cases, and a permanent regression path for every important hardening bug.
   Acceptance criteria: repeated runs converge exactly under the same recorded mode, overrides, inputs, and injected time source; save/load continuation matches uninterrupted execution; mismatched-mode restore is rejected by default; soak coverage includes at least one real game plus long-running synthetic coverage; and fixed hardening bugs leave behind permanent regression assets.

#### Done criteria

- core unit and short integration suites for the blocking DMG areas are green
- the minimum external closure suites are green
- differential comparison either shows no unexplained divergence in the covered scenarios or records the remaining arbitrations explicitly
- deterministic replay and save/load determinism are green under `Strict`, with execution-mode metadata recorded in the relevant artifacts
- no severe open correctness bugs remain in `NoMbc`, `Mbc1`, `Mbc2`, `Mbc3`, or `Mbc5`
- the project has an explicit DMG closure checklist instead of relying on a general compatibility impression

#### Risks if omitted or overly simplified

- false confidence from a few booting games or one passing smoke suite
- unresolved blind spots in scheduler ordering, timing, or cartridge behavior
- repeated rediscovery of the same bugs because regressions were never turned into permanent assets
- inability to explain oracle divergences without expensive manual debugging sessions

---

## Final order summary

1. Test model and test ROM strategy  
2. Debugging infrastructure, tracing, and internal tools  
3. General DMG model and architectural preparation for CGB  

4. T-cycle / dot temporal foundation  
5. Bus and arbitration  
6. Complete memory map and special behavior by region  
7. Base cartridge interface  
8. General I/O registers and read/write rules  
9. Boot ROM mapping, startup modes, and handoff infrastructure  

10. Exact CPU core at the fetch / execute / memory access level  
11. Timer  
12. Interrupts: IME, IE, IF, EI, DI, priority, acceptance timing  
13. HALT, STOP, and HALT bug  

14. OAM DMA  

15. General PPU  
16. Mode 2 / OAM scan  
17. Mode 3 / pixel pipeline  
18. PPU: sprites in detail, priority, BG/OBJ mixing, edge cases  
19. PPU: window in detail and associated glitches  
20. PPU: STAT, LY==LYC coincidence, LCD IRQs, and DMG quirks  
21. PPU: LCD on/off behavior and reactivation  
22. OAM corruption bug  

23. Joypad/input and its relation to interrupts  
24. Serial port  

25. MBC1  
26. MBC2  
27. MBC3  
28. MBC5  
29. Special cartridges and unsupported policy
30. Banked external RAM, battery, RTC, and cartridge persistence

31. General APU architecture
32. APU frame sequencer
33. APU channel 1
34. APU channel 2
35. APU channel 3
36. APU channel 4
37. Mixing, output, DACs, power control, and audio edge cases

38. Whole-machine snapshot contract and ownership
39. Global serialization envelope, versioning, and metadata
40. Core save/load restore path and validation

41. Formal DMG hardening matrix, severity classes, and closure checklist
42. Automated external ROM harness and minimum closure suites
43. Differential comparison against SameBoy
44. Deterministic replay, save/load determinism, and soak coverage
45. Final DMG closure and regression-retention pass

---

## Open TODOs

Use this section to capture concrete remaining work when a change lands without
fully closing its relevant roadmap scope or done criteria.

Suggested entry style:

- In a phase section: `[subsystem] short remaining-work summary`
- In `Cross-phase`: `[Cross-phase][subsystem] short remaining-work summary`

### Cross-phase

- None currently.

### Phase 0 — Verification, debugging, and base architecture infrastructure

- None currently.

### Phase 1 — Temporal foundation and hardware access

- None currently.

### Phase 2 — CPU and real temporal control

- [CPU][BOOT] Full DMG boot ROM execution beyond the Phase `2.4` synthetic handoff baseline is still not closed against the production firmware path. Phase `9` official-ROM bring-up has already landed the broader ALU, accumulator-rotate, MMIO-transfer, `SP/HL`, and practical CB coverage that the earlier boot-facing TODO used to call out, so the remaining gap is no longer a missing-opcode list. What remains is end-to-end validation that a verified DMG boot ROM asset can run under `RealBoot` through the real `FF50` handoff and last pre-handoff fetch window without depending on the synthetic Phase `2.4` boot image. Phase dependency: the shared `[hli]` / `[hld]` subset that also fed Phase `4` OAM-corruption prep is already landed, so this remaining real-boot closure work does not block `Phase 4`.

#### Done:

- [CPU][DIAGNOSTICS] Unsupported decoded opcodes now enter one explicit unsupported-opcode diagnostic trap immediately after the real fetch retires, keeping the failure visible in CPU snapshots and scheduler traces instead of falling into a silent non-retiring execute loop.
- [CPU][HALT] Phase `2.6` now includes explicit `EI ; HALT` pending-IRQ verification and one targeted refinement: when `HALT` is the delayed-`EI` follower with an already pending interrupt, the interrupt is serviced once and returns to the `HALT` opcode instead of falling into the ordinary HALT-bug wake path or skipping ahead past `HALT`.
- [CPU][MMIO-BRIDGE] The minimum MMIO-facing opcode bridge is now landed through `LDH (a8),A`, `LDH A,(a8)`, `LD (C),A`, and `LD A,(C)`, with direct CPU integration tests and synthetic-ROM builder helpers so later joypad, serial, and boot-adjacent validation can target `FF00-FF7F` without raw-byte boilerplate.
- [CPU][OAM-PREP] The shared address-bearing event subset required by Phase `4.8` is already landed through `[hli]` / `[hld]`, fetch-time `PC` increments, stack/control-flow, interrupt service, and observable address-bearing `inc/dec` publication.
- [CPU][PHASE2-SYNTHETIC-ROMS] The first full synthetic Phase `2` asset family now ships: reproducible `NoMBC` ROMs and golden traces for fetch/immediate order, control-flow plus stack plus CB timing, `EI` delay plus priority, `HALT` / `STOP` / `HALT` bug chronology, and timer `IF` visibility plus interrupt service. `crates/gb-core/tests/phase2.rs` is now the source of truth for those builders, traces, and expected end states.
- [TEST-RUNNER][EXTERNAL-STIMULI] `gb-test-runner` metadata now supports deterministic external stimuli, and the shipped `phase2_halt_stop_and_halt_bug` contract uses one explicit joypad `A` press at `t_cycle = 380` so the `STOP` wake is part of the typed suite definition instead of a hidden local-harness assumption.
- [TIMER][RELOAD-WRITE-ARBITRATION] The reload-cycle timer write contract is now explicit and tested: `TIMA` writes on the reload T-cycle are ignored, `TMA` writes on that same cycle feed the reloaded `TIMA`, and the curated `mooneye` cases `timer/tima_write_reloading` and `timer/tma_write_reloading` now pass under `Strict`.
- [TIMER][MOONEYE-RAPID-TOGGLE] The curated DMG `mooneye` `timer/rapid_toggle` case now passes under `Strict`. The closing fix lives in the CPU-owned interrupt accept boundary rather than in new timer arithmetic: after scheduler phase `8` exposes the timer request in `IF`, the CPU may still accept that pending IRQ during the current opcode-fetch M-cycle as long as the next opcode byte has not been latched yet. That matches the Mooneye loop timing without changing the existing timer overflow/reload pipeline.

### Phase 3 — Base DMA

- None currently.

### Phase 4 — Base PPU and visible pipeline

- [PPU][SKIPBOOT-ORACLE] The Phase `4.1` `SkipBoot` startup-mode latch is still validated only against repo-local continuity tests and the documented post-boot snapshot contract. Before Phase `9` hardening treats the direct-boot PPU handoff as externally validated, this path still needs comparison against a trusted oracle or hardware-derived capture proving that the first LCD-visible dots after `SkipBoot` remain coherent with the published `LCDC`, `STAT`, and `LY` state rather than with an overfit local latch assumption. Phase dependency: this does not block Phase `5`, but later DMG closure should not claim externally validated direct-boot PPU continuity until this check lands.
- [PPU][MEALYBUG-MODE3-LIVE-WRITES] The exploratory `mealybug-tearoom-dmg-curated` suite now has one concrete closure point: `m3_bgp_change` passes under `Strict` after refining two explicit DMG-family behaviors in the PPU model, namely the non-line-`0` Mode `2` LCD STAT pretrigger path and DMG palette-write conflict handling for `BGP/OBP*` writes during late Mode `3` / early HBlank. Follow-up work in this lane has also clarified a harness-side precondition shared by multiple sprite-driven cases: ROMs such as `m3_obp0_change` and `m3_bgp_change_sprites` assume the DMG boot trademark tile is already present in VRAM, so the runner now supports typed startup-memory writes and those curated cases seed the boot-derived tile `0x19` under `SkipBoot` instead of misclassifying missing-logo output as a PPU regression. With that boot-derived seed in place, `m3_obp0_change` no longer fails as a blank-output startup artifact and the remaining mismatch shrinks to a small real PPU delta. The next useful reductions in this lane are now landed too: the PPU no longer clamps early OBJ scheduling to visible `X = 0`, and instead keeps a distinct pre-visible raw-`X` match path for left-edge sprites; for those low-`X` startup requests, the OBJ fetch no longer freezes the whole line immediately on match and instead lets the BG side finish priming the FIFO and leave the initial `TileIndex` phase before the sprite path starts stealing dots; and the DMG palette-conflict approximation now keeps `OBP*` writes slightly wider than `BGP` writes in the retroactive window, which trims `m3_obp0_change` further without regressing `m3_bgp_change_sprites` or the existing internal PPU coverage. That keeps the general DMG OBJ-priority tests green, preserves the smaller `m3_obp0_change` delta, and reduces `m3_bgp_change_sprites` further without broadening the change into a whole-pipeline refactor. It still does not close either case yet. The remaining `m3_bgp_change_sprites` error is no longer a broad whole-frame failure; after this reduction it is mostly the residual sprite/live-write timing space rather than the earlier "all low-X sprites collapse to one trigger" bug. The still-red follow-up space therefore narrows down further instead of leaving the whole lane amorphous: low-`X` sprite/live-OBJ timing (`m3_bgp_change_sprites`, the remaining `m3_obp0_change` timing delta), window/live-`LCDC.5` timing (`m2_win_en_toggle`, `m3_window_timing*`, `m3_lcdc_win_en_change_multiple`, `m3_wx_4_change_sprites`), `SCX` low-bit timing (`m3_scx_low_3_bits`), and live sprite-size timing (`m3_lcdc_obj_size_change`) still need dedicated closure. Phase dependency: this does not block entering Phase `7`, but Phase `9` should not mark the mealybug PPU oracle lane as closed until those remaining Mode `3` live-write families are rechecked against the new baseline.
- [PPU][WINDOW-GLITCH-ORACLE] The current Phase `4.4` window baseline includes explicit tested paths for `WX = 0` and `WX = 166`, but they remain provisional baseline behavior rather than oracle-backed glitch closure. The project still needs stricter validation and, if necessary, refinement for `WX` / `WY` / `LCDC.5` mid-frame glitch behavior, including the DMG-specific `WX = 0 && (SCX & 7) > 0` path and the special `WX = 166` continuation behavior. Phase dependency: this does not block entering Phase `5`, but Phase `9` hardening should not mark detailed DMG window-glitch behavior as closed until this oracle pass is finished.
- [PPU][LCDC2-8X16-ARTIFACTS] The Phase `4.5` sprite baseline already treats `LCDC.2` as live state and covers the core `8x16` row-selection rules, and the current baseline now also guards one concrete live-size edge case: if a mid-frame `LCDC.2` shrink leaves the current row outside the new OBJ height, the fetch path no longer underflows or panics and instead resolves that request as empty OBJ data. That removes the `m3_lcdc_obj_size_change` crash, but the finer DMG-visible artifacts and leaks caused by mid-frame `LCDC.2` size changes, especially around the lower half of `8x16` sprites, remain only documented follow-up work and the mealybug framebuffer mismatch is still open. Before Phase `9` hardening claims detailed sprite-size behavior as externally validated, this path still needs targeted ROM or oracle coverage and, if needed, refinement that keeps those artifacts explicit instead of leaving them as accidental baseline behavior. Phase dependency: this does not block Phase `5`, but later DMG closure should not claim fully hardened `LCDC.2` / `8x16` edge behavior until this validation lands.
- [PPU][OAM-CORRUPTION-ORACLE] Phase `4.8` now has deterministic unit/integration coverage, a shipped synthetic ROM/trace family for direct Mode `2` OAM access, `FEA0-FEFF` reads, `inc rr`, `[hli]` / `[hld]`, stack plus interrupt-service paths, DMG-family model variants, and the CGB negative path, but it still lacks comparison against an independent trusted oracle or hardware-derived capture before the bug can be treated as externally validated across instruction families and hardware revisions. As of March 21, 2026, the built-in official `retrio/blargg oam_bug` automation is intentionally curated to the `GBEmulatorShootout`-listed subset, the repository-gated external DMG block now runs that curated subset by default, and the automation still excludes the upstream multi-ROM `oam_bug.gb` plus `7-timing_effect.gb`; Phase `9` hardening should not treat that curated subset as complete OAM-corruption closure without a later independent oracle pass for the remaining timing-sensitive coverage.
- [PPU][MOONEYE-LCD-RESTART] The curated DMG `mooneye` acceptance lane now closes `ppu/stat_lyc_onoff`: LCD-off `STAT` coincidence readback is latched from the last active-LCD comparison, `LYC` writes while LCD-off update storage without recomputing that retained result, and LCD re-enable now exposes one provisional DMG-family restart state with a short early-dot `STAT.mode = 0` startup window instead of reporting Mode `2` immediately after the enable write. That closes the specific `stat_lyc_onoff` acceptance contract, but it does not yet close the broader restarted-line timing. `ppu/lcdon_timing-GS` and `ppu/lcdon_write_timing-GS` remain red, and the current diagnostic reduction still leaves a fine `LY/STAT` boundary mismatch around the early restarted lines, so Phase `4` should not treat LCD restart timing as fully oracle-validated yet.
- [PPU][MOONEYE-STAT-TIMING] The same curated DMG `mooneye` lane still has open PPU timing and LCD STAT cases beyond the now-closed LCD-off coincidence path: `ppu/hblank_ly_scx_timing-GS`, `ppu/intr_2_0_timing`, `ppu/intr_2_mode0_timing_sprites`, and `ppu/vblank_stat_intr-GS` remain red. Phase dependency: this does not block unrelated subsystems, but Phase `9` should not mark DMG STAT-mode timing as closed until those acceptance cases are either fixed or reconciled against a trusted oracle.

#### Done:

- [PPU][BASELINE] The implementation side of Phase `4.1` through `4.8` is landed: scheduler spine, Mode `2`, BG/window/OBJ Mode `3`, `STAT/LY/LYC/IRQ`, LCD power transitions, and the routed DMG-family OAM-corruption model. The remaining Phase `4` work is validation-grade closure rather than another missing baseline implementation block.
- [PPU][OAM-CORRUPTION-CONTRACT] The external-validation skeleton for OAM corruption is now explicit in `gb-test-runner`, including reserved Phase `4` ROM/trace targets for direct Mode `2` OAM access, `FEA0-FEFF` reads, `inc rr` / `dec rr`, `[hli]` / `[hld]`, stack plus interrupt-service paths, DMG-family model coverage, and one CGB negative case.
- [PPU][OAM-CORRUPTION-DIRECT-ROM] One first locally generated `NoMBC` ROM plus golden trace now ships for the direct Mode `2` OAM-write path, proving the reserved Phase `4` names can be backed by real executable assets and locking the baseline write-corruption timing to one reproducible machine trace.
- [PPU][OAM-CORRUPTION-SYNTHETIC-ROMS] The full first synthetic Phase `4` asset family now ships: dedicated `NoMBC` ROMs and golden traces for direct Mode `2` OAM access, `FEA0-FEFF` reads, DMG-family and CGB `inc rr` model coverage, one `[hli]` / `[hld]` combined-event ROM, and one stack-plus-interrupt-service ROM. The checked-in builders in `crates/gb-core/tests/phase4.rs` are now the reproducible source of truth for those assets.
- [PPU][MOONEYE-STAT-LYC-ONOFF] The curated DMG `mooneye` acceptance case `ppu/stat_lyc_onoff` now passes under `Strict` after making LCD-off coincidence retention and LCD re-enable `STAT` readback timing explicit in the PPU model instead of recomputing coincidence from reset `LY = 0` or reporting Mode `2` immediately on re-enable.

### Phase 5 — Input and simple peripherals

- None currently.

### Phase 6 — Banked cartridges, special cartridges, and cartridge persistence

- [MBC1][ORACLE-BANKING] The shipped `MBC1` implementation, integration coverage, and retained synthetic ROM/trace fixtures now cover the documented bank-selection edge cases locally, but Phase `6` still lacks one trusted external oracle comparison for the `0x20` / `0x40` / `0x60` anomaly, the small-ROM high-window bank-`0` case, and the large-ROM mode-`1` low-window remap. Phase dependency: this does not block entering `MBC2`, but Phase `6` should not be treated as externally validated for cartridge banking until the differential harness covers at least one trusted oracle pass.
- [MBC2][ORACLE-RAM] The shipped `MBC2` implementation, integration coverage, and retained synthetic ROM/trace fixture now cover address-bit-`8` control decode, `0 -> 1` banking, nibble RAM, and low-`9`-bit echo aliasing locally, but Phase `6` still lacks one trusted external oracle comparison for those edge cases. Phase dependency: this does not block entering `MBC3`, but `MBC2` should not be treated as externally validated until the cartridge differential harness includes at least one oracle pass for control decode and internal-RAM behavior.
- [MBC3][ORACLE-RTC] The shipped `MBC3` implementation, integration coverage, and retained synthetic ROM/trace fixture now cover standard banking, RAM-versus-RTC selection, latch sequencing, and the live-versus-latched RTC contract locally, but Phase `6` still lacks one trusted external oracle comparison for those bank and RTC edge cases. Phase dependency: this does not block entering `MBC5`, but `MBC3` should not be treated as externally validated until the cartridge differential harness includes at least one oracle pass for banking and RTC behavior.
- [MBC5][ORACLE-RUMBLE] The shipped `MBC5` implementation, integration coverage, and retained synthetic ROM/trace fixture now cover bank-`0` visibility, `9`-bit ROM banking across the `0xFF -> 0x100` boundary, linear SRAM, and rumble-capable register behavior locally, but Phase `6` still lacks one trusted external oracle comparison for bank-selection and rumble edge cases. Phase dependency: this does not block entering the special-cartridge policy block, but `MBC5` should not be treated as externally validated until the cartridge differential harness includes at least one oracle pass for high-bank selection and rumble-capable control writes.
- [SPECIAL][M161-DETECTION] The special-cartridge taxonomy is now explicit for header-coded families plus the opt-in experimental heuristics currently wired for `EMS`, `Bung`, and `Wisdom Tree`, but Phase `6` still lacks a trusted explicit identification rule for `M161`. Phase dependency: this does not block entering the cartridge-persistence block, but the special-cartridge policy should not be treated as fully closed until `M161` can be classified deliberately instead of falling through generic unknown handling.

### Phase 7 — Audio

- None currently.

### Phase 8 — Full emulator save states and global serialization strategy

- None currently.

### Phase 9 — Final DMG hardening, differential validation, and closure

- None currently.

#### Done:

- [TEST-RUNNER][EXECUTION] `gb-test-runner` is no longer contract-only. It now executes typed ROM suites end to end against `gb-core`, supports deterministic external stimuli, timeout policy, serial / framebuffer / snapshot capture, retained failure artifacts, and opt-in external ROM roots without requiring a frontend.
- [TEST-RUNNER][OFFICIAL-ROM-SMOKE] The first opt-in official external suite contract now ships as `retrio-blargg-cpu-smoke`, reserving the full `retrio/blargg cpu_instrs/individual` block (`01` through `11`) under `GB_CYCLE_RETRIO_GB_TEST_ROMS_ROOT` and using serial output as the machine-readable pass channel. All `11` individual CPU cases are now green against the real external assets in `release`, so Phase `9` already has an official serial-based CPU bring-up path that runs end to end without frontend help for the whole individual `cpu_instrs` set.
- [TEST-RUNNER][OFFICIAL-CPU-INSTRS-FULL] The official `retrio/blargg cpu_instrs/cpu_instrs.gb` multi-ROM is now integrated as a separate typed external suite contract and also passes in `release`, using the ROM's own final serial report `Passed all tests` as the machine-readable success condition. That gives the repo both granular coverage from the `11` individual CPU ROMs and the full upstream aggregate CPU run.
- [TEST-RUNNER][OFFICIAL-INSTR-TIMING] The official `retrio/blargg instr_timing` ROM is now integrated as its own typed external suite contract under the same repo-managed asset store and also passes in `release`. The repo-managed `GBEmulatorShootout` Blargg DMG curated suite now carries the matching `instr_timing.gb` case again as part of the default external DMG block, so the timing-focused CPU path is no longer confined to the opt-in official lane. That gives Phase `9` one timing-focused CPU oracle on top of the functional `cpu_instrs` coverage without needing a frontend or ad hoc local asset wiring.
- [TEST-RUNNER][OFFICIAL-MEM-TIMING] The official `retrio/blargg mem_timing` pair is now integrated as typed external suite metadata and both cases pass in `release`, but not through one hard-coded output channel: the original `mem_timing` ROM still uses serial, while `mem_timing-2` now runs through the ROM's own cartridge-RAM text/status contract at `A000..A004`. That means the runner can already distinguish between serial-driven and RAM-driven official ROMs without inventing ad hoc case-local harness code.
- [TEST-RUNNER][OFFICIAL-HALT-BUG] The official `retrio/blargg halt_bug` ROM now passes in `release` too. The key missing piece was not CPU behavior but output capture: this ROM reports its self-validated result through the upstream `console.s` LCD text console rather than serial or cartridge RAM. `gb-test-runner` now has a typed Blargg-console text extractor based on `BGMAP0` plus `SCY`, so the test is automated without relying on manual framebuffer inspection or a circular screenshot fixture.
- [TEST-RUNNER][EXTERNAL-ASSET-STORE] External official ROM assets now have a repo-managed acquisition path instead of ad hoc local clones. The workspace ships `crates/gb-test-runner/data/sources.toml` as the pinned source manifest, `fetch_external_roms` populates the gitignored `/.roms/external-test/` store after hash verification, `gb-test-runner` can fall back to that default store when the suite-specific environment variable is unset, and CI uses the same fetch path for the current smoke job. Private commercial ROMs remain segregated under `/.roms/local-commercial/` and are not part of the public automation contract.
- [CPU][OFFICIAL-ROM-BRINGUP] Bringing up the official individual `cpu_instrs` ROMs has already forced concrete runtime fixes in the current branch baseline, including the `32 KiB` strict-loading admission for header-coded `MBC1` images, a broader ALU / flag / rotate / `ADD HL,rr` CPU block, the signed-`SP` arithmetic path `LD HL,SP+r8` / `ADD SP,r8`, the missing `SP/HL` transfer block `LD (a16),SP` / `LD SP,HL`, and the remaining practical CB matrix needed by the official `09` / `10` / `11` cases (`RRC`, `SLA`, `SRA`, `SWAP`, `RES`, and `SET` for both register and `(HL)` targets).

---

## Final notes

- This document defines the recommended implementation order, not necessarily the exact merge order if work happens in parallel.
- Whenever a later block requires additional observability, the `debugger/` infrastructure should be expanded incrementally without changing its transversal role.
- Any local simplification that contradicts the T-cycle model or the dot-by-dot PPU must be treated as explicit and documented technical debt.
- If a conflict appears between ease of implementation and temporal fidelity, this roadmap prioritizes temporal fidelity as long as the design remains maintainable.
