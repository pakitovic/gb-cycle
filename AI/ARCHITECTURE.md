# Architecture

## Goals

- Prioritize hardware-accurate behavior.
- Keep the core modular enough to evolve from DMG-first to CGB and later SGB support.
- Keep portability high so the core remains platform-agnostic.
- Preserve determinism, debuggability, and testability from the start.

## Recommended high-level layout

Preferred long-term structure:

```text
crates/
  gb-core/
    src/
      cpu/
      ppu/
      apu/
      bus/
      dma/
      timer/
      joypad/
      serial/
      cartridge/
      boot/
      model/
      scheduler/
      debugger/
      lib.rs
  gb-test-runner/
  gb-cli/
  gb-desktop/
  gb-web/
tests/
AI/
```

For an early-stage repo, a simplified equivalent is acceptable as long as these boundaries stay visible.

## Rust module layout policy

- Use `foo.rs` for small subsystems.
- When a subsystem grows, prefer `foo.rs` plus `foo/` as the default production layout.
- Treat the top-level subsystem file as the facade for module declarations, re-exports, and narrow orchestration.
- Move hardware responsibilities into focused child files instead of letting timing-sensitive logic accumulate in one large file.
- Avoid layout churn during behavior work; structural migrations should be isolated when possible.

## Core design principles

- Model hardware by responsibilities, not by frontend features.
- Favor explicit state transitions over implicit side effects.
- Keep timing ownership clear.
- Treat MMIO registers as device interfaces with explicit contracts, not as generic stored bytes.
- Separate behavior specification from optimization strategy.
- Make room for CGB-specific extensions without spreading model checks everywhere.
- Use types to reflect hardware concepts such as model, interrupt source, PPU mode, and cartridge kind.

## Timing foundation

- The project timing foundation is T-cycle based from the start.
- M-cycles may be referenced for documentation or instruction summaries, but they are not the primary execution unit of the core.
- Shared subsystem scheduling should assume a common T-cycle timeline so CPU, PPU, timer, DMA, APU, and bus interactions can be modeled without coarse conversion layers.
- CPU execution on that timeline should be expressed through ordered fetch/read/write/internal steps rather than a black-box opcode plus aggregate duration.
- For the PPU, that shared T-cycle timeline is also the dot timeline; dot-by-dot behavior is the intended baseline.
- Long-running hardware operations triggered by MMIO writes, such as OAM DMA, should become explicit in-flight subsystem state on that shared timeline rather than immediate bulk side effects.

## Global scheduler policy

- The DMG core must have one global scheduler that advances the whole machine on one deterministic T-cycle timeline.
- A `GlobalScheduler` plus `step_t_cycle()`-style entry point, or an equally explicit equivalent, is the preferred architectural shape.
- Do not run PPU, timer, APU, serial, joypad, DMA, or interrupt production as unsynchronized threads or independently clocked loops inside the core.
- The recommended per-T-cycle phase order is:
  1. external event ingress
  2. master clock / shared system-counter tick
  3. resolution of free-running counter-derived edges
  4. autonomous peripheral ticks
  5. bus arbitration for the current T-cycle
  6. CPU micro-operation
  7. MMIO side-effect commit
  8. interrupt aggregation into `IF`
  9. CPU wake / interrupt-accept evaluation
- This phase order is an architectural contract for observable behavior, not a claim that Nintendo published one canonical internal scheduler.
- Another internal decomposition is acceptable only if it preserves the same observable dependencies for PPU mode visibility, DMA blocking, timer overflow delay, serial completion timing, joypad visible-edge timing, MMIO visibility, and CPU interrupt acceptance.
- Phase `3` is for events derived from the free-running shared clock. Device-local MMIO semantics such as `DIV` reset effects, `FF46` DMA start, `SC.7` transfer start, or `LCDC.7` LCD transitions still belong to the device that owns the register when the access commits in phase `7`.
- The scheduler should keep one cycle-local context object or equivalent that can carry at least:
  - the global T-cycle index
  - current-cycle external events
  - already-derived counter-edge signals
  - current ownership or arbitration facts
  - queued side effects and interrupt requests
- The scheduler coordinates ordering and synchronization points; it must not reimplement timer, PPU, DMA, serial, joypad, APU, cartridge, or CPU-local quirks internally.

## Console model policy

- The core must expose an explicit console model concept.
- At minimum, plan for `DMG0`, `DMG`, `MGB`, and future `CGB`.
- The current implementation target should behave as a DMG-family core, while still distinguishing observable model differences.
- Design the DMG-family core with future CGB integration in mind, but do not introduce CGB implementation complexity before it is needed.
- The goal is to avoid major later refactors, not to prematurely model every CGB-only path.
- Boot ROM behavior and startup-visible quirks must be model-aware rather than treated as one generic DMG state.
- `DMG0`, `DMG`, and `MGB` should share one DMG-family hardware core unless evidence shows a true hardware-level divergence that matters to emulation.
- CGB must enter as an extension of the shared architecture, not as a second emulator with duplicated subsystems.
- No critical subsystem should be rigidly tied to a single hardware variant if that would block natural extension to other models.

## DMG-first, CGB-ready policy

- The base core should implement DMG-family behavior only until DMG timing and correctness are stable.
- "Prepared for CGB" means leaving explicit extension seams, not implementing partial CGB logic ahead of time.
- Shared subsystems should be designed so later support for banked VRAM, banked WRAM, extra CGB I/O registers, HDMA, palette state, and double speed can be added without re-architecting the whole core.
- Avoid rigid fixed-size assumptions in subsystem interfaces when the hardware family naturally extends them later.
- Keep the common GB model solid first; do not dilute DMG timing work by mixing in unfinished CGB behavior.
- When CGB arrives, prefer one standard CGB model before attempting fine-grained CGB hardware revision support.
- Architecture should allow the same core to run in DMG-family mode or CGB mode without duplicating subsystem implementations.

## Compatibility-policy architecture

- Compatibility policy is a loader/config contract around the T-cycle core, not a second hardware model.
- The project should expose a typed execution-mode concept such as `ExecutionMode::{Strict, Permissive, Experimental}` rather than scattered booleans.
- The project should expose one central `CompatibilityPolicy`-style structure, or an equally explicit equivalent, that at least carries:
  - `execution_mode`
  - `validation_policy`
  - `heuristic_policy`
  - `override_policy`
  - `diagnostic_policy`
- One central decision point should translate typed cartridge classification, the active compatibility policy, and explicit manual overrides into a final load decision.
- That decision point must not reparse cartridge headers differently per frontend, per mode, or per call site.
- For already supported hardware, switching execution mode must not change T-cycle-visible hardware truth such as timing, arbitration, memory mapping, IRQ behavior, or mapper semantics.
- Mode changes are allowed to affect admission, validation severity, heuristic enablement, manual overrides, diagnostics, and access to explicitly experimental implementations.
- Any temporary exception where a mode changes supported-hardware runtime behavior should be documented as technical debt rather than normalized as ordinary behavior.
- `Strict` is the oracle and CI mode for official accuracy claims.
- `Permissive` is the intended tolerant interactive mode for ordinary users once frontends expose mode selection.
- `Experimental` is for research, bring-up, and partial hardware paths; it must not be treated as evidence for official accuracy claims.
- Save states, replays, and official test artifacts must record the execution mode and active overrides that produced them.
- Restoring or replaying under a different execution mode should fail by default unless a later explicit developer-only conversion workflow is designed on top of the recorded metadata.

## Suggested subsystem boundaries

- CPU: instruction flow, register state, decode/execution state, fine-grained fetch/read/write/internal steps, IME state, interrupt acceptance/dispatch, HALT/STOP semantics, and micro-operation visibility for timing-sensitive hardware interactions
- Bus: address decoding, subsystem routing, dynamic mapping, visible access ordering, temporal arbitration of blocked accesses, delegation to the base cartridge interface for cartridge-owned regions, and routing of access attempts that carry hardware-visible side effects such as DMG OAM corruption triggers
- Memory and MMIO: WRAM, HRAM, echo behavior, plain storage ownership, and only simple storage-backed MMIO state whose semantics are not owned by a dedicated subsystem
- Interrupt controller: IF/IE state, interrupt request paths, priority-ordered pending selection, and acknowledge flow
- Timer: DIV/TIMA/TMA/TAC behavior and edge-sensitive increment logic
- PPU: LCD modes, fetcher/FIFO behavior, rendering state, VRAM/OAM restrictions, Mode 2 OAM-scan row state, and DMG-family OAM corruption behavior
- DMA: OAM DMA and future HDMA scheduling and blocking rules
- APU: per-channel digital generation, DAC state, frame-sequencer / `DIV-APU`, channel-active state, mixing / HPF state, and host-export boundary, but not output backends
- Joypad and serial: hardware-visible registers and signaling
- Cartridge and MBC: cartridge-header parsing, header-driven device selection, ROM/RAM banking, RTC, rumble, and mapper-specific behavior
- Boot ROM and model config: power-up state, revision differences, direct-boot setup
- Model-specific extensions: CGB and later SGB

## Detailed module responsibility guide

This section complements `Suggested subsystem boundaries` by mapping the
intended source layout to concrete ownership. The goal is to keep one canonical
reference for module responsibilities without forcing every early-stage refactor
to immediately materialize as a separate directory.

### `model/`

- DMG-family hardware model definitions
- system base types
- enums for hardware variants and shared configuration
- structural core configuration
- shared compatibility-policy types such as execution mode and override metadata
- architectural extension points for future variants such as CGB

### `scheduler/`

- global T-cycle stepping
- temporal coordination between subsystems
- stable per-T-cycle subsystem stepping order
- explicit global synchronization points
- orchestration between CPU, PPU, DMA, timer, APU, and peripherals
- ingress of timestamped external events such as host input changes or external serial clocks
- explicit separation between free-running device ticks, bus arbitration, MMIO commit, interrupt aggregation, and CPU wake / accept points
- cycle-local context and tracing support so phase order remains visible in code and logs
- subsystem-facing APIs that keep stage boundaries explicit, for example free-running tick, CPU step, MMIO commit, and interrupt-request aggregation steps, even if final names differ

### `bus/`

- memory reads and writes
- memory-map region resolution
- one central address-decode path over the full `0x0000-0xFFFF` map
- one pure address-router layer that resolves nominal domain and region ownership without owning live timing
- access arbitration
- integration of cartridge, VRAM, WRAM, OAM, I/O, HRAM, IE, and boot ROM mapping
- domain-oriented controllers or handlers for cartridge, VRAM, WRAM, OAM, boot-overlay, unusable-space, and IO/HRAM/IE behavior
- modeling of access restrictions and conflicts when hardware makes them visible
- two-layer arbitration made of decode / nominal ownership followed by requester-aware access policy
- requester-facing or device-facing views for VRAM and OAM so PPU and DMA do not borrow raw backing arrays directly from unrelated call sites
- routing of OAM and `FEA0-FEFF` access attempts and CPU-provided address-bearing micro-events into the DMG-family OAM corruption path when applicable
- MMIO routing to the subsystem-owned register contract for each mapped address
- one source of truth for MMIO ownership, model availability, access class, and read/write side-effect policy
- if a docboy-like internal domain is introduced, prefer `IoHram` / `Internal` naming over `CpuBus`; keep WRAM explicit instead of burying it inside a generic external or CPU-named bus so future CGB banking remains visible in the architecture
- For the current DMG-first repo baseline, a concrete split of `bus.rs` plus `bus/router.rs`, `bus/iohram.rs`, `bus/wram.rs`, `bus/video.rs`, and `bus/view.rs` is the preferred shape.
- In that split, `IoHram` owns routed `FFxx`, `HRAM`, and `IE` behavior; WRAM remains a separate explicit domain so later CGB bankability does not get buried inside an internal bus.
- Video-domain acquisition or release is scheduler-visible state, not router behavior; ownership changes for `VRAM` and `OAM` should stay synchronized to the shared T-cycle timeline around PPU and DMA ticks rather than being invented inside the router.

### `memory/` or bus-owned storage helpers

- WRAM and HRAM backing storage
- echo-RAM alias backing without duplicate storage
- plain storage ownership for regions that are not device-defined MMIO, plus any simple storage-backed MMIO fields that remain bus-owned and have no dedicated device semantics
- explicit uninitialized-memory policy inputs for direct-boot paths
- narrow storage helpers that remain subordinate to bus-owned address decode and access policy

### `interrupts/` or a tightly scoped interrupt-controller component

- `IF` and `IE` ownership
- interrupt-source bookkeeping
- centralized interrupt request / clear helpers
- fixed-priority pending selection
- MMIO exposure of `FF0F` and `FFFF`
- separation between controller-owned request state and CPU-owned `IME` / acceptance flow
- aggregation of source requests into `IF` without collapsing that step into CPU dispatch

### `boot/`

- boot ROM assets and selection
- startup-mode selection such as real boot versus direct post-boot entry
- initial mapping state
- boot ROM unmapping
- model-aware post-boot initialization for explicit skip-boot paths
- centralized post-boot snapshot data and cartridge-derived startup adjustments
- coordination of subsystem-owned hidden-state synthesis needed for temporally coherent direct-boot entry
- startup-visible boot behavior from the system perspective

### `cpu/`

- SM83 core execution
- fetch / decode / execute at T-cycle granularity
- per-instruction reads, writes, and internal steps
- explicit address-bearing `16`-bit increment/decrement micro-events where hardware quirks depend on them
- interrupt acceptance and servicing
- HALT / STOP / HALT bug behavior

### `ppu/`

- LCD control state
- PPU mode sequencing
- OAM scan
- current Mode `2` OAM row tracking
- pixel fetcher
- pixel FIFO
- BG / window / OBJ mixing
- LCD-facing registers owned by the PPU path
- LY / LYC / STAT behavior
- DMG-family OAM corruption controller and formulas

### `dma/`

- shared DMA controller and active-transfer state
- DMG OAM DMA
- transfer lifecycle such as `Idle`, `Starting`, `Active`, `Completed`, and future `Cancelled`
- transfer timing over the shared scheduler timeline
- per-transfer timing policy, block granularity, and advance-condition modeling
- publication of CPU-impact and memory-region-impact state for bus and PPU consumption
- DMA-owned validation and normalization of source and destination contracts
- integration with bus arbitration
- architectural preparation for future GDMA and HDMA without scheduler redesign

### `timer/`

- DIV / TIMA / TMA / TAC
- edge-sensitive timer timing
- timer interrupt request generation
- ownership of the timer overflow pipeline, including the delayed `IF` request relative to logical overflow

### `joypad/`

- hardware-facing state of the `8` buttons
- `P1/JOYP` row-selection ownership
- visible low-nibble composition from the selected matrix rows
- previous-visible-state tracking or equivalent edge detection
- joypad interrupt generation through the shared interrupt-controller path
- input-driven wake signaling for CPU `STOP` integration
- separation between frontend input collection and emulated joypad semantics
- distinction between hardware-facing button changes, visible `JOYP` changes, joypad IRQ requests, and `STOP` wake signaling

### `serial/`

- `SB` and `SC` ownership
- bit-level serial transfer state
- internal-clock versus external-clock behavior
- peer or link-endpoint boundary
- serial interrupt generation through the shared interrupt-controller path
- separation between emulated serial hardware and any host transport implementation
- ownership of transfer-complete detection, `SC.7` clear timing, and completion-triggered serial IRQ requests

### `cartridge/`

- base cartridge interface
- typed cartridge-header parsing over `0x0100-0x014F`
- decoded cartridge capability model including cartridge type, ROM size, RAM size, CGB flag, and SGB flag
- explicit capability metadata for battery-backed RAM, RTC, and rumble derived from the validated header type
- central cartridge factory, compatibility-policy consumption, and validation policy
- typed loader result that separates supported cartridge construction from structured special / unsupported classification, preserving raw `0x0147`, detected name, category, and reason
- one central load-decision path that combines cartridge classification, compatibility policy, and explicit overrides into admit / warn / reject results
- concrete cartridge devices such as `NoMbcCartridge`, `Mbc1Cartridge`, `Mbc2Cartridge`, `Mbc3Cartridge`, and `Mbc5Cartridge`
- No MBC family support, including the `0x00`, `0x08`, and `0x09` header variants
- MBC implementations
- explicit supported-family taxonomy such as `NoMbc`, `Mbc1`, `Mbc2`, `Mbc3`, and `Mbc5`, plus structured unsupported categories rather than one opaque `Unsupported`
- reserved typed variant space for close derivatives such as `MBC30` and future `MBC1M`
- separate classification path for special multicarts, documented-but-unsupported mappers, accessory cartridges, experimental heuristics, and unknown codes
- explicit separation between raw mapper register state, header-derived wiring / variant metadata, mapper-local RAM organization, and helper logic that resolves effective ROM and RAM banks
- cartridge-visible RAM, whether external or mapper-local to the mapper
- RTC-backed cartridges
- bus-facing ownership of `0x0000-0x7FFF` and `0xA000-0xBFFF` through one stable device contract
- typed cartridge persistence contract for full cartridge-owned backing stores such as linear SRAM, banked SRAM, MBC2 nibble RAM, or MBC3 SRAM plus RTC
- host storage backends kept outside the core runtime API, with only a narrow typed cartridge-persistence surface exposed when needed

### `apu/`

- global audio architecture
- `NR50`, `NR51`, and `NR52` ownership
- channel state machines
- per-channel digital output, DAC-enable state, and active-state tracking
- frame sequencer
- `DIV-APU` ownership derived from the shared divider timeline
- mixing logic
- DAC and output-facing emulated state
- stereo master-volume and HPF state
- host-facing sample/export boundary kept separate from hardware stepping

### `debugger/`

- tracing
- breakpoints
- watchpoints
- snapshots
- state inspection
- freeze typed breakpoint/watchpoint target categories early enough that `PC`,
  memory, MMIO, and cartridge-visible state do not need a public API redesign
  once CPU and bus hooks become real
- targeted subsystem viewers or equivalent structured dumps for CPU, scheduler, PPU, DMA, APU, IRQ, and cartridge state
- internal analysis and comparison tools
- utilities for synchronization and trace-debug workflows

## State persistence and snapshot boundaries

### Cartridge persistence

- The powered-on core remains T-cycle driven. Cartridge persistence is a boundary around cartridge-owned state, not a second bus or scheduler path.
- If the core exposes persistence hooks, keep them narrow and typed, for example through `PersistentCartState` or `CartridgePersistentPayload`, and make the cartridge implementation the owner of payload semantics.
- That contract should be able to represent no persistent storage, persistent RAM only, persistent RTC only, or combined RAM plus RTC without forcing the backend to reverse-engineer mapper details from the visible `0xA000-0xBFFF` window.
- Storage backends such as disk or in-memory adapters should own serialization format, versioning, file naming, path mapping, timestamps, and atomic replacement policy, not cartridge semantics.
- Frontend and tooling layers may decide when to flush, such as on close, on explicit manual save, or via optional auto-flush, but they should do so through the persistence backend rather than through bus hooks or cartridge-local file I/O.
- Tests and tools must be able to use an in-memory persistence backend so cartridge persistence can be validated without host file I/O.

### Full emulator save states and replays

- Cartridge persistence and emulator save states must remain separate systems. Cartridge persistence stores only cartridge-owned hardware state; emulator save states may snapshot the whole machine.
- Whole-machine save states should capture subsystem-owned live state through explicit typed snapshot contracts rather than by reverse-engineering hidden state from MMIO readback.
- Cartridge data included inside a whole-machine save state should enter through cartridge-owned runtime snapshot semantics, not by reusing the hardware-style persistence payload as a proxy for full console state.
- Emulator save states and replays should preserve the execution mode and active compatibility overrides that were in effect when they were created.
- Restoring or replaying under a different execution mode should fail by default unless a later explicit conversion workflow is designed on top of recorded metadata.
- Debugger or tooling snapshots should layer on top of the same core-owned save-state contracts instead of creating a second incompatible serialization path.

## Module mapping notes

- `Memory and MMIO` may remain a dedicated module or stay split across bus-owned
  storage helpers and subsystem-owned registers, but ownership must stay
  explicit.
- `Interrupt controller` may exist as its own module or as a tightly scoped core
  component, but `IF` / `IE` ownership must remain distinct from CPU-owned
  `IME`, `halted`, and `stopped` state.
- `model/`, `scheduler/`, and `debugger/` are architectural modules even if an
  early repository stage temporarily keeps some of their code in fewer files.

## Ownership boundary notes

- The scheduler owns phase order, cycle context, and subsystem call order; it must not become a second implementation of timer, PPU, DMA, serial, joypad, or CPU rules.
- The boot subsystem owns firmware assets, model-aware boot configuration, and boot-ROM enable/disable state.
- The boot subsystem also owns the source-of-truth startup snapshot for direct-boot entry, while the target subsystems still own the live semantics of their registers once execution begins.
- The DMA subsystem owns transfer state and transfer requests over time.
- The DMA subsystem also owns transfer-kind-specific validation, lifecycle, progress, CPU-impact policy, and region-impact publication; the bus and PPU should consume that common state instead of reverse-engineering DMA behavior from MMIO register details.
- The PPU owns LCD mode state and the rules that determine when VRAM/OAM are accessible.
- The PPU also owns the live Mode `2` OAM-row state and the DMG-family OAM corruption formulas, while the bus routes relevant access attempts and the CPU exposes the micro-events needed to classify IDU-driven triggers.
- The interrupt controller owns `IF`/`IE` register state and pending-request bookkeeping, while the CPU owns `IME`, `halted`, `stopped`, and the final decision to accept and service an interrupt.
- Frontends, test harnesses, and tooling should submit abstract button press/release state changes rather than prebuilt `JOYP` bytes or direct CPU wake requests.
- The joypad subsystem owns the translation from host-facing button state plus `P1` row selection into visible `JOYP` readback, joypad interrupt requests, and any input-driven `STOP` wake signal.
- Frontends, test harnesses, and tooling should provide serial peers, scripted bits, loopback, or external clock pulses through a serial-endpoint boundary rather than by writing received bytes directly into `SB`.
- The serial subsystem owns the translation from MMIO-visible `SB` / `SC` plus peer-provided bits and clocks into live transfer progress, `SB` intermediate state, and serial interrupt requests.
- The timer owns the shared divider/system-counter state and visible `DIV`, while the APU owns `DIV-APU`, frame-sequencer state, channel-active state, DAC state, mixer state, and HPF state derived from that shared timing source.
- The bus owns central decode, requester arbitration, and blocked-access policy; CPU, DMA, and future transfer engines must not bypass that one policy path with caller-specific memory shortcuts.
- The bus applies boot mapping, DMA contention, and blocked-access semantics using that subsystem state; CPU code should not embed those rules directly.
- The bus owns address decode and MMIO dispatch, but the device that owns a register must own its read, write, and side-effect semantics.
- The interrupt controller owns `IF` / `IE` state and fixed-priority pending selection, but it does not decide when the CPU actually accepts an interrupt or wakes from `HALT` / `STOP`.
- The cartridge owns the meaning of persistent RAM and RTC content, while the save backend owns durable storage mechanics such as file format, paths, versioning, and atomic replacement.
- Save-state machinery must not be smuggled into the cartridge persistence boundary; CPU, PPU, APU, WRAM, and other console-owned state belong to a different system.
- MMIO metadata should be centralized enough that readable bits, writable bits, dynamic bits, reserved bits, and model-specific availability are not re-declared ad hoc in several modules.
- CPU code, DMA helpers, and frontend input/audio/video layers must not bypass MMIO-owned subsystem state by poking internal register-shaped fields directly.
- The memory subsystem owns plain storage regions such as WRAM and HRAM; it must not bypass bus-visible access restrictions defined elsewhere.
- Shared scheduling must allow CPU, DMA, PPU, timer, and other actors to make progress on the same T-cycle timeline so arbitration remains observable.
- Shared scheduling must not depend on whole-instruction CPU completion; it should be able to observe CPU fetches, operand reads, stack traffic, and internal steps while the rest of the hardware continues to advance.
- Input events must enter that same shared scheduling model as changes to hardware-facing button state; they must not live only on a host video-frame cadence if that would hide `JOYP`, interrupt, or `STOP`-wake ordering.
- Serial peer activity and external serial clock pulses must enter that same shared scheduling model rather than living on host transport threads or timers that bypass the core timeline.

## Boot ROM architecture policy

- Treat boot ROM as firmware executed by the real CPU model, not as a fake initialization script.
- Keep DMG-family hardware separate from boot ROM assets: one hardware core, multiple selectable boot ROM images.
- Boot ROM selection should depend on the console model and support at least real boot ROM execution, custom boot ROM injection, and direct boot without firmware.
- Direct-boot helpers are a testing and tooling feature, not a replacement for real boot ROM execution.
- The boot subsystem should not assume every model uses the same boot firmware size or address mapping layout; keep those details inside the boot and bus design, not spread through unrelated subsystems.

## Portability policy

- No platform-specific APIs inside the emulation core.
- Keep file I/O, audio output, video output, and input outside the core.
- Use traits or narrow interfaces where frontend services must be injected.
- The same core should be usable by CLI tools, desktop apps, benchmarks, tests, and WebAssembly.

## Scalability policy

- New hardware quirks must be added behind well-defined subsystem boundaries.
- Avoid spreading model checks across unrelated modules.
- Centralize model and revision capabilities.
- Do not couple DMG-only shortcuts into APIs that would block CGB banking, palettes, HDMA, or double speed later.
- Prefer capability-driven branching from a shared model description over ad hoc per-subsystem variant checks.
- Prefer bus-side dynamic mapping and access-state rules over flattening everything into static memory ownership tables.
