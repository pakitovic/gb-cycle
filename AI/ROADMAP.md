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
   Acceptance criteria: CPU and DMA use the same arbitration route, decode/ownership and access-policy layers stay distinct, and DMG OAM DMA correctly leaves CPU with HRAM only.
3. **IRQ aggregation layer** (`Phase 2`)
   Goal: separate source request, `IF` visibility, and CPU acceptance.
   Acceptance criteria: PPU, timer, serial, and joypad only request; the CPU accepts by `IME/IE/IF` and fixed priority; timer keeps its delayed `4`-T-cycle (`1` M-cycle) request timing.
4. **Cycle logging** (`Phase 0`, expanded later)
   Goal: make the actual ordering visible per T-cycle.
   Acceptance criteria: traces can expose phase, bus owner, CPU micro-op, PPU mode, DMA activity, timer or serial events, and `IF/IE/IME`.
5. **Global-order regression tests** (`Phase 2` onward)
   Goal: lock down the scheduler invariants at cross-subsystem boundaries.
   Acceptance criteria: focused tests cover DMA versus CPU, delayed timer `IF`, serial completion plus IRQ, joypad visible `High -> Low` plus IRQ, `HALT` / IRQ priority, and `STAT`-versus-bus coherence.

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
- bus arbitration resolves decode/ownership before applying requester-specific restrictions, and DMA-versus-CPU precedence is centralized instead of duplicated
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
4. Close validation and diagnostics policy.
   Acceptance criteria: ROM-size and RAM-size metadata are checked explicitly, size mismatches produce useful warnings or errors, special ROM-size codes are not ignored silently, documented-but-unsupported cartridge types fail in a controlled way without mapper fallback, and the project exposes a configurable strict-versus-permissive policy with heuristics disabled by default in strict mode.

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
   Acceptance criteria: No MBC expects `32 KiB` ROM and at most `8 KiB` RAM, inconsistent headers report declared type, declared ROM size, declared RAM size, and actual file size, `0x08` and `0x09` may warn as rare but are not rejected solely for rarity, and strict/permissive/test modes remain configurable.
6. Close integration coverage.
   Acceptance criteria: skip-boot and post-`FF50` mapping tests use No MBC as the first closed cartridge baseline in this phase; once Phase 2 real-boot execution exists, the first real-boot cartridge coverage also lands on No MBC before any MBC-dependent validation.

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

#### Recommended sequencing inside Phase 3

1. Extract OAM DMA into the common controller.
   Acceptance criteria: OAM DMA no longer lives on an ad hoc path, visible DMG behavior does not regress, CPU still remains HRAM-only during active OAM DMA, and the PPU still sees the same OAM-conflict state.
2. Separate transfer mechanics from arbitration policy.
   Acceptance criteria: the bus consults one common DMA constraint API, the DMA subsystem does not reimplement bus decode, and the PPU can react to common OAM or VRAM impact state rather than to transfer-specific register knowledge.
3. Add common lifecycle and status visibility.
   Acceptance criteria: OAM DMA uses `Idle -> Starting -> Active -> Completed`, the common API can already represent future cancellation, and code can query active-versus-finished state without depending on one specific origin register.
4. Prepare block or windowed progression hooks.
   Acceptance criteria: fields such as `block_size` and `advance_condition` exist in the controller contract, including room for future `0x10`-byte HDMA-style blocks, they are not yet wired to real HDMA registers, and Phase 3 does not need another scheduler redesign when CGB transfer work starts.
5. Lock the infrastructure with focused tests.
   Acceptance criteria: tests cover OAM DMA on the new controller, published bus constraints, lifecycle visibility, completion, and at least one simulated `0x10`-byte block-style transfer shape that is not yet mapped to real CGB MMIO.

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

---

### Phase 6 — Banked cartridges, special cartridges, and persistence

25. **MBC1**
26. **MBC2**
27. **MBC3**
28. **MBC5**
29. **Special cartridges and unsupported policy**
30. **Banked external RAM, battery, RTC, persistence**

#### Goal

Extend `cartridge/` from the closed No MBC baseline to banked commercial cartridge families and generalized cartridge-local persistence without contaminating the rest of the core.

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
- portable persistence boundaries across frontends and tools
- clear separation between emulation logic and host storage APIs

#### MBC1 sequencing inside Phase 6

1. Establish the MBC1 register model and power-up state.
   Scope: `ram_enabled`, raw `rom_bank_low5`, raw `secondary_bank`, `banking_mode`, deterministic startup for both `RealBoot` and `SkipBoot`, and `0 -> 1` handling for the primary register field.
   Acceptance criteria: power-up state is `ram_enabled = false`, `rom_bank_low5 = 0`, `secondary_bank = 0`, and `banking_mode = 0`; `0x4000-0x7FFF` starts on bank `1`; and writes to `0x0000-0x7FFF` update the intended MBC1 register immediately for later accesses on the shared T-cycle timeline.
2. Implement standard MBC1 ROM banking and size masking.
   Scope: high-region bank selection for `64 KiB`, `128 KiB`, `256 KiB`, and `512 KiB` ROMs, raw low-register preservation, `0 -> 1` before final size masking, and the documented special-bank behavior.
   Acceptance criteria: `0x4000-0x7FFF` selects the correct bank across the supported small-ROM sizes, the documented small-ROM case where bank `0` can appear in the high region after masking is reproducible, and dedicated tests cover banks `0x01` and `0x1F` plus the raw-register edge case.
3. Add large-ROM alternate wiring and mode-dependent low-region mapping.
   Scope: `1 MiB` and `2 MiB` standard MBC1 wiring, secondary-register high ROM bits, mode `0` versus mode `1`, and low-region bank selection for large cartridges.
   Acceptance criteria: banks `0x20`, `0x40`, and `0x60` are unreachable in the switchable high region while `0x21`, `0x41`, and `0x61` are reachable, `0x0000-0x3FFF` stays on bank `0` in mode `0`, mode `1` exposes the documented secondary-controlled low-region banks on large cartridges, and dedicated tests cover `0x21`, `0x41`, and `0x61` explicitly.
4. Implement external RAM enable and RAM-bank behavior.
   Scope: RAM-enable decode, disabled-RAM open-bus policy, ignored writes while disabled, fixed `8 KiB` RAM on large-ROM alternate wiring, and banked `32 KiB` RAM on compatible small-ROM cartridges.
   Acceptance criteria: disabled RAM reads follow an explicit policy and writes are ignored, mode `0` fixes RAM to bank `0`, mode `1` selects RAM banks `0..=3` on compatible cartridges, and large-ROM cartridges keep one fixed `8 KiB` visible RAM window.
5. Add MBC1 validation and diagnostics.
   Scope: consistency checks across `0x0147`, `0x0148`, `0x0149`, real ROM size, RAM size, and chosen MBC1 wiring / variant metadata.
   Acceptance criteria: impossible combinations produce clear diagnostics, large-ROM cartridges do not silently masquerade as `32 KiB` banked-RAM cartridges, and MBC1M is either detected explicitly or reserved through a first-class variant flag.
6. Close with dedicated MBC1 tests and oracle comparisons.
   Scope: unit tests, integration tests, ROM-based coverage, and at least one trusted oracle comparison for bank-selection edge cases.
   Acceptance criteria: tests cover RAM enable, `0 -> 1`, banks `0x01`, `0x1F`, `0x21`, `0x41`, `0x61`, the `0x20` / `0x40` / `0x60` anomaly, the small-ROM high-region bank-`0` case, mode `0` versus mode `1`, `8 KiB` versus `32 KiB` RAM behavior, and explicit configuration diagnostics.

#### MBC2 sequencing inside Phase 6

1. Establish the MBC2 control model and power-up state.
   Scope: `ram_enabled`, raw `rom_bank_low4`, address-bit-`8` decode inside the cartridge device, deterministic startup for both `RealBoot` and `SkipBoot`, and the documented `0 -> 1` behavior for the switchable ROM window.
   Acceptance criteria: power-up state is `ram_enabled = false` and raw `rom_bank_low4 = 0`, the effective `0x4000-0x7FFF` bank starts at `1`, writes with address bit `8 = 0` control RAM enable, and writes with address bit `8 = 1` control the ROM-bank register immediately on the shared T-cycle timeline.
2. Implement MBC2 ROM banking and ROM-size validation.
   Scope: switchable-region bank selection in `0x4000-0x7FFF`, raw `4`-bit bank-register preservation, documented `0 -> 1`, final masking by real ROM size, and explicit `256 KiB` maximum validation.
   Acceptance criteria: bank `0` translates to bank `1`, the effective high-region bank follows the real loaded ROM size without losing the raw-register semantics, and MBC2 cartridges that exceed `256 KiB` produce explicit diagnostics.
3. Implement internal `512 x 4-bit` RAM and echo aliasing.
   Scope: nibble-based internal RAM storage, low-nibble writes, explicit high-nibble read policy, disabled-RAM behavior, and low-`9`-bit address masking across `0xA000-0xBFFF`.
   Acceptance criteria: only `512` logical cells exist, writes preserve only the low nibble, the chosen high-nibble readback policy is explicit, RAM-disabled writes are ignored, RAM-disabled reads follow one explicit policy, and aliasing between `0xA000-0xA1FF` and `0xA200-0xBFFF` is correct.
4. Add persistence and header validation for MBC2.
   Scope: `0x05` versus `0x06`, battery-backed persistence for internal RAM, `0x0149` special-case validation, and explicit diagnostics for inconsistent header metadata.
   Acceptance criteria: `0x06` persists the internal RAM, `0x05` does not, `0x0149` is not reinterpreted as external SRAM size, and nonzero `0x0149` values on MBC2 cartridges produce clear warnings or errors according to the selected validation policy.
5. Close with dedicated MBC2 tests and oracle comparisons.
   Scope: unit tests, integration tests, ROM-based coverage, and at least one trusted oracle comparison for MBC2 bank and RAM edge cases.
   Acceptance criteria: tests cover address-bit-`8` control decode, bank `0 -> 1`, ROM-size diagnostics, echo aliasing across `0xA000-0xBFFF`, low-nibble storage, chosen high-nibble readback policy, battery persistence, and `0x0149 = 0x00` validation.

#### MBC3 sequencing inside Phase 6

1. Establish the MBC3 control model and power-up state.
   Scope: `ram_rtc_enabled`, raw `rom_bank`, explicit typed `ram_or_rtc_select`, latch-sequence detection for `0x00 -> 0x01`, deterministic startup for both `RealBoot` and `SkipBoot`, and typed distinction between RAM-bank, reserved-selector, and RTC-register selection.
   Acceptance criteria: `0x0000-0x1FFF` enables RAM / RTC on low-nibble `0xA` and disables otherwise, raw ROM bank `0` maps to effective bank `1`, `0x4000-0x5FFF` distinguishes standard MBC3 RAM-bank targets `0x00..=0x03`, reserved selector values `0x04..=0x07`, and RTC-register targets `0x08..=0x0C`, and control writes become visible immediately on the shared T-cycle timeline.
2. Implement standard MBC3 ROM and RAM banking.
   Scope: fixed low ROM bank `0`, switchable high ROM bank `0x01..=0x7F`, raw `7`-bit ROM-bank register, real-size masking, standard external-RAM banking up to `32 KiB`, and explicit MBC30 reservation.
   Acceptance criteria: MBC3 supports up to `2 MiB` ROM, the switchable region honors raw `0 -> 1` while still masking by real ROM size, banks `0x20`, `0x40`, and `0x60` are reachable unlike MBC1, RAM banking is masked by real RAM size, and `64 KiB` SRAM configurations are reserved or diagnosed explicitly instead of being treated as standard MBC3.
3. Implement live RTC registers and latched snapshots.
   Scope: RTC register mapping for `0x08..=0x0C`, live versus latched RTC state, and the `0x6000-0x7FFF` latch edge.
   Acceptance criteria: the RTC snapshot refreshes only on the `0x00 -> 0x01` sequence, repeated reads remain stable until the next latch, reads come from the latched snapshot, and writes go to the live RTC state.
4. Add day counter, halt, and carry behavior.
   Scope: `9`-bit visible day counter, `DH.bit0`, `DH.bit6`, `DH.bit7`, overflow behavior, sticky carry, and halted-versus-running live RTC progression.
   Acceptance criteria: visible days stay in `0..=511`, overflow sets carry and wraps the visible day counter, carry stays set until software clears it, `halt` freezes the live RTC, and writes to `DH` control day bit `8`, halt, and carry explicitly.
5. Add time-source separation and persistence.
   Scope: explicit separation between visible RTC registers, live RTC counter state, injected time source, and persistence backend; battery-backed elapsed-time handling across powered-off sessions; deterministic testing hooks.
   Acceptance criteria: battery-backed MBC3 cartridges can persist RTC state, elapsed powered-off time is applied through the chosen time-source policy without coupling RTC advancement to CPU cycle count, and tests can run against an injected deterministic clock rather than host wall time.
6. Close with dedicated MBC3 tests and validation follow-up.
   Scope: header-type coverage, RAM-versus-RTC selector behavior, latch sequencing, halt/carry/day overflow, stable snapshots, optional fine-delay research, and explicit future MBC30 tracking.
   Acceptance criteria: tests cover `0x0F`, `0x10`, `0x11`, `0x12`, and `0x13`, raw ROM-bank `0 -> 1`, RAM-bank versus RTC-register selection, latch `0x00 -> 0x01`, halt / carry / day overflow, stable RTC snapshots, and any deferred `16`-T-cycle / `4 us` access-spacing work is recorded explicitly in the roadmap rather than forgotten.

#### MBC5 sequencing inside Phase 6

1. Establish the MBC5 control model and power-up state.
   Scope: `ram_enabled`, raw low `8` ROM-bank bits, raw high `1` ROM-bank bit, raw `ram_bank_raw`, deterministic startup for both `RealBoot` and `SkipBoot`, and explicit variant metadata for RAM / battery / rumble capability.
   Acceptance criteria: `0x0000-0x1FFF` enables RAM on low-nibble `0xA` and disables otherwise, the switchable ROM window really allows bank `0`, the low and high ROM-bank register pieces stay explicit, `0x4000-0x5FFF` updates raw RAM-bank state immediately, and control writes become visible immediately on the shared T-cycle timeline.
2. Implement `9`-bit MBC5 ROM banking and size masking.
   Scope: fixed low ROM bank `0`, switchable high ROM bank `0x000..=0x1FF`, combined `rom_bank_low8 + rom_bank_high1` resolution, real-size masking, and explicit preservation of bank `0` semantics in the high region.
   Acceptance criteria: MBC5 supports up to `8 MiB` ROM, bank `0` is reachable in `0x4000-0x7FFF`, bank `0x1FF` is reachable on full-size images, banks above `0xFF` are reachable through the high ROM-bank bit, and final bank selection remains masked by real ROM size without introducing an MBC1/MBC3-style `0 -> 1` rule.
3. Implement MBC5 SRAM enable and linear RAM banking.
   Scope: RAM-enable gating, linear `8 KiB` bank selection through `ram_bank_raw`, no MBC1-style dual banking mode, disabled-RAM policy, ignored writes while disabled, real-size masking, and the ordinary `8 KiB`, `32 KiB`, and `128 KiB` SRAM shapes.
   Acceptance criteria: MBC5 SRAM does not behave like normal RAM while disabled, `8 KiB`, `32 KiB`, and `128 KiB` RAM configurations map correctly, effective RAM banks are masked by the real RAM-bank count, no MBC1-style dual banking mode exists, and header variants without RAM do not expose fake SRAM behavior merely because the bank register exists.
4. Implement rumble-capable MBC5 variants.
   Scope: explicit handling for `0x1C`, `0x1D`, and `0x1E`, observable `rumble_on`, `bit 3` ownership in the `0x4000-0x5FFF` control register, separation between effective RAM-bank selection and motor state, and cartridge-local ownership of rumble behavior.
   Acceptance criteria: `bit 3` of the `0x4000-0x5FFF` control register updates `rumble_on`, the motor state remains latched until software changes it, rumble handling does not break effective RAM-bank selection, and the bus / frontend do not own rumble semantics.
5. Add MBC5 validation, diagnostics, and persistence expectations.
   Scope: header-type coverage for `0x19..=0x1E`, battery-backed persistence expectations, ROM-size validation up to `8 MiB`, RAM-size validation up to `128 KiB`, and explicit diagnostics for impossible header combinations.
   Acceptance criteria: `0x19..=0x1E` are distinguished cleanly, battery variants persist RAM without changing live mapping rules, ROM sizes above `8 MiB` produce clear errors, type / RAM mismatches such as "no RAM type with nonzero `0x0149`" produce clear errors, and rumble-capable types are not accepted unless the implementation exposes observable rumble state.
6. Close with dedicated MBC5 tests and oracle comparisons.
   Scope: unit tests, integration tests, ROM-based coverage, and at least one trusted oracle comparison for bank-selection and rumble edge cases.
   Acceptance criteria: tests cover header types `0x19..=0x1E`, bank `0` visibility in the switchable region, bank `0x1FF`, `9`-bit ROM-bank selection across the `0xFF -> 0x100` boundary, RAM-enable behavior, SRAM behavior for `8 KiB` / `32 KiB` / `128 KiB`, RAM-bank masking, rumble on/off, and size-validation diagnostics.

#### Special-cartridge and unsupported-policy sequencing inside Phase 6

1. Establish the special-cartridge taxonomy and unsupported categories.
   Scope: one central classification path for `Supported`, `PlannedVariant`, `DocumentedButUnsupported`, `ExperimentalHeuristic`, `AccessorySpecialCase`, and `UnknownCode`, plus stable names for `MBC30`, `MBC1M`, `MMM01`, `M161`, `HuC1`, `HuC-3`, `MBC6`, `MBC7`, `Pocket Camera`, `Bandai TAMA5`, `EMS`, `Bung`, and `Wisdom Tree`.
   Acceptance criteria: the loader produces stable classification for all of those cases, the frontend does not need to reparse headers to explain them, and the classification preserves raw `0x0147`, detected name, category, and reason.
2. Add explicit `MBC30` detection.
   Scope: detect the `MBC3`-family plus `64 KiB` SRAM case as `MBC30`, return a typed planned variant or supported variant entry point instead of ordinary standard `MBC3`, and reserve matching persistence / banking work.
   Acceptance criteria: `MBC3 + 64 KiB SRAM` never falls through to standard `MBC3`, loader diagnostics name `MBC30` explicitly, and the code path is ready for future concrete `MBC30` implementation.
3. Add multicart and near-variant classification.
   Scope: classify `MMM01` from `0x0B..=0x0D`, reserve future `MBC1M` as a distinct `MBC1`-family variant, keep `M161` in a multicart-special path, and avoid assuming that `MMM01` boot/header handling is identical to standard cartridges.
   Acceptance criteria: `0x0B`, `0x0C`, and `0x0D` are emitted as `MMM01`, `MBC1M` remains a separate future variant instead of being merged into standard `MBC1`, and multicarts do not silently degrade to ordinary `MBC1` or `NoMbc`.
4. Enforce controlled failure for documented special hardware.
   Scope: explicit diagnostics for `HuC1`, `HuC-3`, `MBC6`, `MBC7`, `Pocket Camera`, `Bandai TAMA5`, and `M161`, plus a hard rule against automatic fallback to `MBC1`, `MBC3`, `MBC5`, or other nearby supported mappers.
   Acceptance criteria: those types fail with clear messages naming the exact detected cartridge, `UnknownCode` reports the raw `0x0147` byte, and no silent degradations or fake "best effort" mapper substitutions remain.
5. Add optional experimental heuristic mode.
   Scope: isolate `EMS`, `Bung`, and `Wisdom Tree` detection behind an explicit dev / experimental loader policy, keep strict default behavior header-driven, and document that heuristic paths are lower priority than `MBC30`, multicarts, and documented special hardware.
   Acceptance criteria: heuristic detection is off by default, can be enabled explicitly for development and research, and diagnostics clearly state when a classification came from heuristics instead of a standard header mapping.

#### Persistence sequencing inside Phase 6

1. Define the cartridge persistent-state contract.
   Scope: a typed contract such as `PersistentCartState` that each supported cartridge family exposes, explicit per-mapper payload shapes, and strict separation from full-emulator save-state serialization.
   Acceptance criteria: `NoMbc`, `Mbc1`, `Mbc2`, `Mbc3`, and `Mbc5` expose cartridge-owned persistent payloads, `Mbc2` and `Mbc3` use dedicated payload shapes for nibble RAM and RTC state, and no CPU, PPU, APU, WRAM, or other console-owned state leaks into the contract.
2. Build the save backend.
   Scope: disk and in-memory adapters, versioned save format, path / name mapping, load and save APIs, backend metadata separation from raw cartridge payloads, and portability across CLI, desktop, web, and tests.
   Acceptance criteria: the backend can round-trip complete cartridge backing stores rather than only visible windows, supports battery-backed RAM and RTC payloads, and can be tested without real file I/O.
3. Integrate battery-gated hardware persistence.
   Scope: binding persistence eligibility to validated `0x0147` capability data such as `has_battery`, preserving non-persistent RAM semantics, and avoiding automatic hardware-style saves for cartridges that do not provide battery-backed storage.
   Acceptance criteria: only battery-backed cartridges generate hardware-style persistence by default, `MBC2` restores nibble RAM correctly, `MBC3` restores SRAM plus RTC correctly, and non-battery cartridges do not silently produce hardware-save payloads unless an explicit non-faithful option is added.
4. Fix the off-session RTC time policy.
   Scope: explicit elapsed-real-time handling for battery-backed `MBC3`, support for both real clocks and injected deterministic clocks, preservation of live-versus-latched RTC separation, and clear T-cycle versus powered-off-time boundaries.
   Acceptance criteria: real and injected clocks are both supported, elapsed powered-off time is applied without coupling RTC advancement to accumulated CPU cycles alone, and day counter, halt, and carry survive persistence round-trips correctly.
5. Harden save writes and flush policy.
   Scope: save-on-close, manual or forced save, optional auto-flush after writes to persistible cartridge state, atomic replacement or equivalent corruption-avoidance strategy, format versioning, and clear error reporting.
   Acceptance criteria: the save backend exposes explicit flush policy choices outside the bus, versioned payloads are written atomically or through an equivalent safe strategy, and persistence errors surface clearly instead of failing silently.

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

30. **General APU architecture**
31. **APU frame sequencer**
32. **APU channel 1**
33. **APU channel 2**
34. **APU channel 3**
35. **APU channel 4**
36. **Mixing, output, DACs, power control, and audio edge cases**

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
30. Banked external RAM, battery, RTC, persistence

31. General APU architecture
32. APU frame sequencer
33. APU channel 1
34. APU channel 2
35. APU channel 3
36. APU channel 4
37. Mixing, output, DACs, power control, and audio edge cases

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

- None currently.

### Phase 3 — Base DMA

- None currently.

### Phase 4 — Base PPU and visible pipeline

- None currently.

### Phase 5 — Input and simple peripherals

- None currently.

### Phase 6 — Banked cartridges, special cartridges, and persistence

- None currently.

### Phase 7 — Audio

- None currently.

---

## Final notes

- This document defines the recommended implementation order, not necessarily the exact merge order if work happens in parallel.
- Whenever a later block requires additional observability, the `debugger/` infrastructure should be expanded incrementally without changing its transversal role.
- Any local simplification that contradicts the T-cycle model or the dot-by-dot PPU must be treated as explicit and documented technical debt.
- If a conflict appears between ease of implementation and temporal fidelity, this roadmap prioritizes temporal fidelity as long as the design remains maintainable.
