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
- definition of responsibilities by module
- initial interfaces between CPU, bus, PPU, DMA, timer, cartridge, and debugger
- conventions to avoid mixing frontend logic with core logic

#### Done criteria

- there is a clear document or convention for the test strategy
- the project can run base tests against `gb-core`
- there is a reusable minimum tracing infrastructure
- the scheduler has a defined notion of T-cycle advancement
- the responsibility split between modules is fixed
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

##### Bus and memory map

- memory region resolution
- unified access to ROM, VRAM, WRAM, external RAM, OAM, I/O, HRAM, and IE
- modeling of echo RAM and unusable areas
- access arbitration infrastructure
- one central decode path across `0x0000-0xFFFF` with explicit owner and access policy per region
- centralized MMIO register routing instead of a generic RAM-like `FF00-FF7F` block

##### Base cartridge

- base interface for ROM-only
- cartridge integration with the bus
- clean separation between bus logic and cartridge-specific logic

##### I/O and boot

- general I/O registers with base read/write rules
- centralized MMIO metadata describing owner, access class, readable bits, writable bits, dynamic bits, reserved bits, read side effects, write side effects, and model-specific availability
- MMIO infrastructure for mixed registers composed from latched, dynamic, forced, and unimplemented bits
- boot ROM integration into the memory map
- correct boot ROM unmapping
- system-visible startup configuration
- explicit `RealBoot` and `SkipBoot` startup modes
- DMG-family boot-ROM kind selection for `DMG0`, `DMG`, and `MGB`
- `FF50`-driven handoff infrastructure in the bus mapping layer
- skip-boot initialization path with model-aware post-boot state entry
- centralized visible post-boot snapshot tables for CPU and I/O state
- explicit direct-boot policy for unreliable startup state such as WRAM, HRAM, and other non-deterministic regions

#### Done criteria

- the system can advance by T-cycles consistently
- all memory accesses go through `bus/`
- the memory map is modeled completely for DMG
- every DMG address region has an explicit owner, read behavior, write behavior, and blocked-access policy where applicable
- every MMIO address in `0xFF00-0xFF7F` and `0xFFFF` has an explicit routed owner and contract rather than accidental default byte-storage behavior
- mixed MMIO registers are represented as per-field contracts rather than as plain read/write bytes plus a coarse mask
- a functional ROM-only cartridge exists
- base I/O registers are connected to the bus
- the boot ROM can be mapped and unmapped correctly
- boot-ROM overlay versus cartridge visibility is controlled explicitly by bus-visible mapping state
- MMIO side effects occur on the access itself on the shared timeline rather than in an end-of-instruction cleanup pass
- `SkipBoot` reaches `0x0100` through explicit post-boot initialization rather than partial boot-ROM execution
- deterministic and cartridge-derived visible post-boot state are initialized through one documented path rather than scattered startup literals
- the infrastructure is ready for a later real-boot path to start CPU execution at `0x0000` with boot ROM mapped and hand off through a real `FF50` write once the CPU core exists

#### Risks if done late or incorrectly

- CPU built on unrealistic accesses
- PPU or DMA breaking later because real arbitration was missing
- boot ROM treated as a hack rather than real hardware
- rewriting the bus when introducing DMA, PPU, or MBCs

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
   Scope: `SB/SC` and the `NRxx` family.
   Acceptance criteria: the routed MMIO contract already encodes correct read/write policy, immediate register-side effects, and non-RAM-like behavior for these ranges; full transfer timing and full APU behavior remain owned by later subsystem phases.
6. Close absent and CGB-only register policy in DMG mode.
   Acceptance criteria: unavailable CGB-only MMIO registers do not behave like RAM, readback follows documented DMG fallback values, and writes follow an explicit ignored-or-stub policy.

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
- HALT
- STOP
- HALT bug

#### Done criteria

- the CPU core does not depend on an oversimplified full-instruction abstraction
- instructions generate their real bus accesses
- the timer advances with the global scheduler
- interrupts and HALT are integrated into the real execution flow
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

Integrate OAM DMA as a real transfer mechanism inside the system architecture, coordinated with the scheduler and bus.

#### Modules involved

- `dma/`
- `bus/`
- `scheduler/`
- `cpu/`
- `debugger/`

#### Deliverables

- writing to the DMA register triggers the transfer
- real temporal copy progression
- integration with bus arbitration
- observability of DMA start, progress, and completion

#### Done criteria

- the transfer is not implemented as an instantaneous memory copy
- arbitration correctly reflects DMA effects on concurrent accesses
- the system can trace DMA over time

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

#### Done criteria

- the PPU advances dot-by-dot
- Mode 3 is based on a pixel FIFO rather than deferred scanline rendering
- sprites and window participate inside the real pipeline
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
- interrupt generation where appropriate
- serial port functional at the emulated hardware level
- trace tools to observe both peripherals

#### Done criteria

- joypad and serial are decoupled from the frontend
- both are integrated through the bus and scheduler
- their interrupts and states are observable and testable

---

### Phase 6 — Real cartridges

25. **MBC1**
26. **MBC2**
27. **MBC3**
28. **MBC5**
29. **External RAM, battery, RTC, persistence**

#### Goal

Extend `cartridge/` from ROM-only to real commercial cartridge support without contaminating the rest of the core.

#### Modules involved

- `cartridge/`
- `bus/`
- `scheduler/`
- `debugger/`
- frontend/tooling persistence adapters

#### Deliverables

- banking support for MBC1
- banking support for MBC2
- banking and RTC support for MBC3
- banking support for MBC5
- functional external RAM
- portable persistence boundaries across frontends and tools
- clear separation between emulation logic and host storage APIs

#### Done criteria

- the bus uses a clean interface toward the cartridge
- each MBC lives inside `cartridge/` without polluting the rest of the system
- RTC and persistence are properly encapsulated
- persistence does not break portability between CLI, desktop, and web

#### Risks if integrated poorly

- cartridge logic spread throughout the bus
- persistence coupled directly to the core
- difficulty extending to more mappers or future variants

---

### Phase 7 — Audio

30. **General APU architecture**
31. **APU channel 1**
32. **APU channel 2**
33. **APU channel 3**
34. **APU channel 4**
35. **APU frame sequencer**
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
- final mixing
- DAC control
- power control
- audio edge cases
- clean interface between `gb-core` and frontend audio adapters

#### Done criteria

- each channel is independently verifiable
- the frame sequencer coordinates the subsystem correctly
- mixing and DACs are implemented on top of a stable channel base
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
29. External RAM, battery, RTC, persistence  

30. General APU architecture  
31. APU channel 1  
32. APU channel 2  
33. APU channel 3  
34. APU channel 4  
35. APU frame sequencer  
36. Mixing, output, DACs, power control, and audio edge cases  

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

### Phase 6 — Real cartridges

- None currently.

### Phase 7 — Audio

- None currently.

---

## Final notes

- This document defines the recommended implementation order, not necessarily the exact merge order if work happens in parallel.
- Whenever a later block requires additional observability, the `debugger/` infrastructure should be expanded incrementally without changing its transversal role.
- Any local simplification that contradicts the T-cycle model or the dot-by-dot PPU must be treated as explicit and documented technical debt.
- If a conflict appears between ease of implementation and temporal fidelity, this roadmap prioritizes temporal fidelity as long as the design remains maintainable.
