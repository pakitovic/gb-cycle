# Architecture

## Scope

This document owns the project-level crate layout, core ownership boundaries, scheduler shape, persistence boundaries, and portability rules. Detailed hardware behavior lives in `docs/hardware/*.md`; public model-axis guidance lives in [`info/MODEL-AXES.md`](info/MODEL-AXES.md); shared timing terminology lives in [`info/TIMING-AND-ACCURACY.md`](info/TIMING-AND-ACCURACY.md); validation policy lives in [`TESTING.md`](TESTING.md) and [`info/ROM-SUITES.md`](info/ROM-SUITES.md).

Keep this file architectural: it should say where behavior belongs and which contracts must stay stable, not duplicate subsystem handbooks or frontend manuals.

## Goals

- Prioritize hardware-accurate behavior over convenience.
- Keep one shared GB core that can run DMG-family, CGB-family, and SGB/SGB2 host-shell configurations without duplicating subsystems.
- Model execution on a deterministic T-cycle timeline so CPU, PPU, DMA, timer, APU, serial, joypad, bus, interrupts, and cartridge behavior remain observable and testable.
- Keep `gb-core` platform-agnostic so CLI, desktop, tests, benchmarks, tools, and future WebAssembly frontends reuse the same core-facing contracts.
- Preserve debuggability and explicit ownership when adding accuracy, performance, or new hardware families.

## Repository layout

```text
crates/
  gb-core/          hardware model, scheduler, machine composition, save-state and rewind contracts
  gb-persistence/   host storage policy for cartridge saves and .gbstate envelopes
  gb-test-runner/   automated ROM-suite execution, manifests, fixtures, and reports
  gb-benchmark/     portable benchmark TOML parsing, run expansion, stimulus, and artifact metadata
  gb-cli/           headless frontend/tooling surface
  gb-desktop/       SDL3 desktop frontend and host UX

docs/
  info/             cross-cutting guidance, frontend usage, ROM-suite operation, model axes, and timing vocabulary
  hardware/         subsystem hardware behavior and MMIO/timing semantics
  roadmap/          implementation phase context
```

Future frontends should adapt these contracts instead of introducing platform-specific APIs inside `gb-core`.

## Crate ownership

| Crate | Owns | Must not own |
| --- | --- | --- |
| `gb-core` | Emulated hardware state, T-cycle stepping, subsystem contracts, machine composition, typed cartridge persistence payloads, whole-machine save-state DTOs, rewind buffer primitives, debug snapshots | Disk paths, UI state, audio/video backends, host input APIs, frontend settings, release packaging |
| `gb-persistence` | Durable cartridge-save policy, `.gbsav` fallback envelopes, external `.sav/.saN` conversion when lossless, `.gbstate` envelopes, safe replacement, timestamps, host paths | Live hardware semantics, mapper behavior, scheduler state mutation |
| `gb-test-runner` | Automated ROM-suite catalogues, manifest parsing, fixture materialization, runner contracts, deterministic report generation, CI-facing validation helpers | Frontend UX |
| `gb-benchmark` | Shared benchmark cases, deterministic input stimulus, one-file-per-game benchmark contract, stats serialization, artifact path conventions, and the `cargo rom-bench` batch orchestrator | Frontend-specific rendering/audio/input implementations |
| `gb-cli` | Headless user/tooling commands, run budgets, state import/export orchestration, report presentation | Hardware shortcuts, duplicated loader policy, host UI state |
| `gb-desktop` | SDL3 windows, presentation, audio backend configuration, menus, dialogs, settings persistence, controller selection, host hotkeys, desktop rewind integration | Core hardware state, cartridge semantics, independent timing model |

## Core invariants

- Model hardware by ownership boundaries, not by frontend features.
- Keep MMIO registers as device interfaces with explicit read/write/side-effect contracts, not generic byte storage unless the register is truly storage-backed.
- Prefer strongly typed model, mode, interrupt, requester, cartridge, save-state, and capability data over booleans or stringly typed policy.
- Keep hidden global state out of the core; lifecycle, reset, ROM replacement, and host ingress must enter through explicit machine/session boundaries.
- Do not let timing-sensitive behavior depend on host frame cadence, host threads, or whole-instruction CPU completion.
- Avoid layout churn during behavior fixes; split modules when it clarifies ownership, and keep behavior-neutral migrations separate when possible.

## Timing and scheduler contract

The project timing foundation is T-cycle based. M-cycles may appear in documentation or instruction summaries, but core execution, arbitration, and subsystem synchronization must be representable on one shared T-cycle timeline.

The DMG/CGB/SGB core uses one deterministic machine scheduler. A `GlobalScheduler` plus `step_t_cycle()`-style entry point, or an equally explicit equivalent, is the preferred shape.

Recommended observable per-T-cycle phase order:

1. external event ingress
2. master clock / shared system-counter tick
3. resolution of free-running counter-derived edges
4. autonomous peripheral ticks
5. bus arbitration for the current T-cycle
6. CPU micro-operation
7. MMIO side-effect commit
8. interrupt aggregation into `IF`
9. CPU wake / interrupt-accept evaluation

This phase order is an architectural contract for observable dependencies, not a claim that Nintendo published one canonical internal scheduler. Another internal decomposition is acceptable only if it preserves the same visible ordering for PPU mode visibility, DMA blocking, timer overflow delay, serial completion timing, joypad visible-edge IRQs, MMIO visibility, same-cycle timer queued-request opcode preemption, and CPU interrupt acceptance.

The scheduler coordinates ordering, cycle-local context, trace points, and synchronization; it must not reimplement timer, PPU, DMA, serial, joypad, APU, cartridge, or CPU-local quirks. Idle fast paths may skip subsystem calls only when the owning subsystem exposes that no hardware-visible work is pending for the current T-cycle.

## Model and compatibility policy

The public model surface uses separate axes instead of one catch-all enum: `ConsoleModel` for silicon family/revision baseline, `OperatingMode` for software-visible GB/CGB compatibility mode, and `HostPlatform` for the surrounding shell such as handheld, `SGB`, or `SGB2`. `CapabilitySet` is the derived view shared subsystems may query for high-level facts such as CGB extensions, DMG-family silicon quirks, DMG software contract, or SGB host enhancements.

`ConsoleModel::GameBoyColor` plus `OperatingMode::GbCompatible` represents CGB-family silicon running monochrome software; it is not the same hardware as DMG-family silicon. `SGB` and `SGB2` enter through `HostPlatform` around the shared GB core, not through a cloned DMG emulator path.

The base core must preserve DMG-family closure while CGB behavior extends the same scheduler, bus, CPU, PPU, DMA, APU, timer, serial, cartridge, persistence, and save-state contracts. Implemented CGB behavior should live behind explicit subsystem ownership and capability gates; avoid ad hoc product-model checks that spread hardware policy across unrelated modules.

Compatibility policy is a loader/config contract around the T-cycle core, not a second hardware model. The central `CompatibilityPolicy` combines `ExecutionMode::{Strict, Permissive, Experimental}`, validation policy, heuristic policy, override policy, and diagnostic policy into one load/admit/warn/reject decision path. Switching mode must not change T-cycle-visible truth for already-supported hardware; it may affect admission, validation severity, heuristic enablement, manual overrides, diagnostics, and access to explicitly experimental implementations.

`Strict` is the oracle and CI mode for official accuracy claims. `Permissive` is tolerant interactive/tooling mode. `Experimental` is for research and partial hardware paths and must not be used as evidence for official accuracy claims. Save states, replays, and official artifacts must record execution mode and active overrides; restoring or replaying under a different mode should fail by default unless an explicit conversion workflow exists.

## `gb-core` module ownership

| Module(s) | Owns |
| --- | --- |
| `model/`, `speed.rs`, `sgb.rs` | Public model axes, capability derivation, speed profile data, SGB profile data, compatibility-policy types, and shared configuration that other subsystems consume without redefining product taxonomy. |
| `scheduler/` | Global T-cycle stepping, ordered subsystem calls, cycle-local context, host-ingress boundary, traceable synchronization points, and separation between free-running ticks, arbitration, MMIO commit, interrupt aggregation, and CPU wake/accept. |
| `machine/` | Composition of one configured console, startup/reset/ROM replacement orchestration, public stepping APIs, host-ingress queues, observer hooks, cartridge persistence access, Pocket Camera frame injection, printer-page collection, and debug snapshots. |
| `cpu/` | SM83 registers, fetch/decode/execute state, T-cycle-level reads/writes/internal steps, address-bearing increment/decrement micro-events, IME, interrupt acceptance/dispatch, `HALT`, `STOP`, and HALT bug behavior. |
| `bus/` | Central address decode, requester-aware access policy, dynamic mapping, boot overlay, WRAM/HRAM and simple storage-backed regions, MMIO dispatch, video-domain access state, OAM corruption trigger routing, CGB infrared `RP` register ownership, and observability metadata. |
| `boot/` | Boot ROM assets and selection, real/custom/skip boot startup modes, boot mapping, model-aware direct-boot snapshots, `FF50` handoff policy, and startup-visible boot behavior from the system perspective. |
| `interrupts/` | `IF`/`IE` register state, source request bookkeeping, request/clear helpers, fixed-priority pending selection, MMIO exposure, and aggregation into `IF`; CPU still owns `IME`, wake, and service acceptance. |
| `timer/` | Shared divider/system-counter-derived `DIV`, `TIMA/TMA/TAC`, edge-sensitive increments, overflow pipeline, and delayed timer interrupt request timing. |
| `ppu/` | LCD control, mode sequencing, OAM scan, Mode `2` row tracking, pixel fetcher/FIFO, BG/window/OBJ mixing, LCD-facing registers, `LY/LYC/STAT`, CGB palettes/VRAM behavior, and DMG-family OAM corruption formulas. |
| `dma/` | OAM DMA, GDMA/HDMA lifecycle and timing, active-transfer state, CPU-impact and memory-region-impact publication, per-transfer validation, bus-arbitration integration, and cancellation/completion state. |
| `apu/` | `NR50/NR51/NR52`, channel state machines, DAC-enable and channel-active state, frame sequencer / `DIV-APU`, mixing, HPF, sample capture, and host sample/export boundary without owning audio backends. |
| `joypad/` | Hardware-facing button matrix, `P1/JOYP` row selection, visible low-nibble composition, edge detection, joypad IRQ requests, and input-driven `STOP` wake signaling from abstract button transitions. |
| `serial/` | `SB/SC`, bit-level transfer state, internal and external clock behavior, peer bit/clock boundary, transfer-complete timing, `SC.7` clear timing, and serial IRQ requests. |
| `external_port/` | Attachment identity and runtime state for loopback, printer, `DMG-04`, and `DMG-07`; printer protocol state; attachment reset/startup policy; per-console endpoint snapshots; conversion to the narrow serial-peer/external-clock boundary. |
| `link/` | Multi-console T-cycle session orchestration, `DMG-04` cable routing, `DMG-07` adapter topology, native CGB IR optical-pair routing, Pokémon Pikachu Color and Mystery Gift protocol helpers, and separation from frontend player-slot UX. |
| `cartridge/` | Header parsing, typed classification, central load decision, mapper/device construction, ROM/RAM banking, RTC, rumble, flash/EEPROM/sensor behavior, cartridge-visible RAM, and typed cartridge-persistence payload semantics. |
| `save_state.rs`, `rewind.rs` | Core-owned whole-machine save-state DTOs, capture/restore boundaries, restore validation, frame/subframe rewind ring buffers, deterministic memory telemetry, and debug/tooling reuse without owning host storage. |
| `debugger/` | Tracing, trace summaries, breakpoints, watchpoints, structured subsystem snapshots, comparison/debug utilities, and observability infrastructure without taking ownership of subsystem behavior. |

The bus may route a register, access attempt, or address-bearing micro-event, but the device that owns the hardware behavior owns the semantics. CPU, DMA, frontends, tests, and tools must not bypass bus/MMIO/subsystem contracts by poking internal register-shaped fields directly.

## Cartridge, persistence, save-state, and rewind boundaries

Cartridge persistence stores cartridge-owned hardware state only. The core exposes typed `PersistentCartState`-style payloads; cartridge implementations own payload semantics; storage backends own serialization, paths, versioning, timestamps, atomic replacement, and external conversion.

Runtime frontends treat `.sav` for P1 and `.sa2/.sa3/.sa4` for linked slots as authoritative when mapper state has a lossless external representation. `.gbsav/.gbsaN` remains the lossless fallback for mappers or future hardware state without a documented raw-save contract. Legacy `.gbsav` files for external-stable carts are intentionally not auto-loaded; users need explicit migration tooling.

Whole-machine save states are separate from cartridge persistence. `MachineSaveState` is the core-owned save-state boundary, distinct from `MachineSnapshot` debug inspection and cartridge battery-save persistence. Capture happens at a stable public T-cycle boundary; restore validates model, operating mode, host platform, SGB profile, startup mode, compatibility policy, loaded ROM fingerprint, and boot-ROM fingerprint before mutating subsystem-owned state directly, without replaying MMIO writes.

The `.gbstate` envelope belongs to `gb-persistence`, uses `GBSTATE\0` magic and format version `1`, and carries host metadata around the core payload. During active development, the payload schema is current-only; incompatible local slot files may be rejected and recreated instead of migrated.

Rewind is layered over repeated in-memory `MachineSaveState` capture/restore. The core owns the ring buffer and memory telemetry; `gb-desktop` owns host capture cadence, hotkeys, menu settings, HUD indicators, input/audio/pacing cleanup, and clearing rewind history after externally loaded state jumps. Multi-machine rewind coordination, compression, deltas, and debugger-grade reverse T-cycle stepping remain outside the current core contract.

## Frontend, tooling, and host boundaries

Frontends and tools submit abstract hardware-facing events, not precomposed hardware state. Joypad input enters as button transitions; serial peers provide bits or external clocks; camera and printer seams use typed core boundaries; ROM replacement restarts the emulated hardware and scheduler timeline through the configured startup path.

Frontend-owned state includes windows, audio devices, video presentation, frame pacing, `vsync`, file dialogs, ROM filters, recent files, settings persistence, controller discovery/selection, menu navigation, hotkeys, user-facing warnings, performance HUDs, and lifecycle flush policy. These concerns must not leak into `gb-core` as alternate hardware modes.

`gb-benchmark` centralizes benchmark case parsing, deterministic stimulus, artifact names, result metadata, and the `cargo rom-bench` batch orchestrator so `gb-cli` and `gb-desktop` do not diverge in benchmark behavior. `gb-test-runner` centralizes automated validation workflows; manual external oracles and one-off emulator comparisons remain operator workflows rather than core architecture.

## Evolution guardrails

- Add new hardware quirks behind explicit subsystem ownership and document them in the matching hardware handbook.
- Prefer capability-driven branching from shared model data over scattered model checks.
- Avoid DMG-only API shortcuts that would block CGB banking, palettes, HDMA, double speed, AGB-family compatibility, SGB host behavior, or later SNES/SFC-side execution.
- Keep bus-side dynamic mapping and requester-aware access policy explicit instead of flattening everything into static storage tables.
- Keep model, timing, persistence, and validation policies centralized enough that frontends and tools cannot accidentally fork hardware truth.
- Optimize only after correctness and observability are preserved; fast paths must not hide pending MMIO, interrupt, DMA, serial, video-domain, or joypad edges.
