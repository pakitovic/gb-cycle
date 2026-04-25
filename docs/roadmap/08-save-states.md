# Phase 8 — Full emulator save states and global serialization strategy

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

#### Phase 8 implementation contract

- `gb-core::save_state` owns `MachineSaveState`, `MachineSaveStateMetadata`, `MachineSaveStateRestoreError`, and typed subsystem state boundaries for scheduler, machine runtime events, CPU, bus, PPU, DMA, timer, APU, joypad, serial, boot, external port, interrupts, and cartridge runtime state.
- `Machine::capture_save_state()` captures at the stable boundary between public T-cycle steps. The scheduler portion records `next_t_cycle`; machine-local pending external events are captured separately from scheduler timing.
- `Machine::restore_save_state()` validates metadata before mutation, then restores owned subsystem fields directly. It does not replay MMIO writes and does not apply cartridge RTC elapsed off-session time.
- Mandatory metadata includes console model, operating mode, host platform, startup mode, compatibility policy / execution mode / overrides, `next_t_cycle`, loaded cartridge kind and ROM fingerprint, plus boot-ROM kind, mapping state, and fingerprint when a boot ROM applies.
- `gb-persistence` owns the `.gbstate` envelope (`GBSTATE\0`, version `1`, extension `.gbstate`) separately from `.gbsav`. Decode rejects unsupported versions, invalid magic, corrupt/truncated payloads, trailing bytes, unknown metadata tags, and envelope/payload metadata mismatches.
- Phase 8 keeps `MachineSaveState` cloneable and usable entirely in memory. That is the future rewind hook: a later phase can add a frame/subframe ring buffer, compression, deltas, and UI without adding disk or timestamp policy to `gb-core`.

#### Phase 8.1 semantic hardening contract

- Keep the public `MachineSaveState` API and `.gbstate` version `1` compatible; this hardening step must not introduce rewind UI, ring buffers, compression, deltas, or a schema-breaking DTO conversion.
- Validate restore semantics through one reusable continuation harness that captures a save state, forks an uninterrupted continuation, dirties and restores the original machine, then compares post-restore continuation state.
- Coverage must include CPU mid-instruction / HALT / pending IME, PPU Mode 3 fetch/FIFO/window/OBJ state, active DMA with restart state, timer overflow pipeline, serial transfers in flight, active APU output state, and representative cartridge runtime state for NoMBC, MBC1, MBC2, MBC3 RAM+RTC, MBC5, and Pocket Camera.
- A later Phase 8.2 may convert mirror-style subsystem wrappers into explicit durable DTOs, but only after the Phase 8.1 semantic coverage is green.

#### Risks if delayed or underspecified

- final hardening work lacks a stable save/load foundation
- frontend-specific storage decisions leak into core semantics
- restore paths reconstruct only visible registers and lose hidden temporal state
- cartridge persistence and whole-machine save states become conflated
- debugger or replay tooling grows a second incompatible serialization path
