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
   Acceptance criteria: each channel can later receive its own waveform timer without changing the master frame-sequencer architecture, and known follow-up work such as extra length clocking, CH3 wave-RAM quirks, and envelope zombie-mode remains explicitly tracked rather than implicit.

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
29. External RAM, battery, RTC, persistence  

30. General APU architecture
31. APU frame sequencer
32. APU channel 1
33. APU channel 2
34. APU channel 3
35. APU channel 4
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
