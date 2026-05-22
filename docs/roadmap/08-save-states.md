# Phase 8 — Full emulator save states and global serialization strategy

38. **Whole-machine snapshot contract and ownership**
39. **Global serialization envelope, versioning, and metadata**
40. **Core save/load restore path and validation**

#### Goal

Establish one explicit full-emulator save-state system, separate from cartridge persistence, only after the hardware subsystems already own their live runtime state and before final DMG closure depends on save/load determinism. Phase `6` cartridge persistence is intentionally not a substitute for this whole-machine snapshot block.

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

- `gb-core::save_state` owns `MachineSaveState`, `MachineSaveStateMetadata`, `MachineSaveStateRestoreError`, and typed subsystem state boundaries for scheduler, machine runtime events, CPU, bus, PPU, DMA, timer, APU, joypad, serial, SGB host, boot, external port, interrupts, and cartridge runtime state.
- `Machine::capture_save_state()` captures at the stable boundary between public T-cycle steps. The scheduler portion records `next_t_cycle`; machine-local pending external events are captured separately from scheduler timing.
- `Machine::restore_save_state()` validates metadata before mutation, then restores owned subsystem fields directly. It does not replay MMIO writes and does not apply cartridge RTC elapsed off-session time.
- Mandatory metadata includes console model, hardware revision, operating mode, host platform, startup mode, compatibility policy / execution mode / overrides, `next_t_cycle`, loaded cartridge kind and ROM fingerprint, plus boot-ROM mapping state and fingerprint when a boot ROM applies.
- During the current local-development contract, `.gbstate` payload DTOs are current-only; additive or schema-breaking fields may reject older local slot files, and those files should be recreated instead of migrated.
- `gb-persistence` owns the `.gbstate` envelope (`GBSTATE\0`, current version `9`, extension `.gbstate`) separately from cartridge battery-save storage (`.sav/.saN` primary when lossless, `.gbsav/.gbsaN` envelope fallback otherwise). Decode rejects unsupported non-current versions, invalid magic, corrupt/truncated payloads, trailing bytes, unknown metadata tags, and envelope/payload metadata mismatches.
- Phase 8 keeps `MachineSaveState` cloneable and usable entirely in memory. That is the future rewind hook: a later phase can add a frame/subframe ring buffer, compression, deltas, and UI without adding disk or timestamp policy to `gb-core`.

#### Phase 8.1 semantic hardening contract

- Keep the public `MachineSaveState` API and `.gbstate` envelope stable; this hardening step must not introduce rewind UI, ring buffers, compression, deltas, or a schema-breaking DTO conversion.
- Validate restore semantics through one reusable continuation harness that captures a save state, forks an uninterrupted continuation, dirties and restores the original machine, then compares post-restore continuation state.
- Coverage must include CPU mid-instruction / HALT / pending IME, PPU Mode 3 fetch/FIFO/window/OBJ state, active DMA with restart state, timer overflow pipeline, serial transfers in flight, active APU output state, and representative cartridge runtime state for NoMBC, MBC1, MBC2, MBC3 RAM+RTC, MBC5, and Pocket Camera.
- A later Phase 8.2 may convert mirror-style subsystem wrappers into explicit durable DTOs, but only after the Phase 8.1 semantic coverage is green.

#### Phase 8.2 DTO durability contract

- Convert subsystem save-state wrappers from root runtime clones into explicit owner DTOs with runtime-to-DTO and DTO-to-runtime conversion paths.
- Keep `Machine::capture_save_state`, `Machine::restore_save_state`, `MachineSaveState`, metadata, and restore error APIs stable; only the `.gbstate` payload contract changes.
- Keep `.gbstate` within the local development contract, keep magic/extension unchanged, and reject unsupported non-current versions instead of adding compatibility migrations.
- Keep rewind, ring buffers, compression, deltas, and UI out of this phase; the in-memory `MachineSaveState` remains cloneable and disk-independent.

#### Phase 8.4 core rewind contract

- `gb-core::rewind` owns a single-machine `MachineRewindBuffer` that stores full in-memory `MachineSaveState` snapshots in a bounded FIFO ring buffer. It does not touch `.gbstate` files, cartridge battery-save files, disk, timestamps, host input, or frontend UI.
- The default policy targets roughly ten seconds of history, always supports frame-boundary captures, supports configurable subframe captures, and enforces an estimated-byte cap with oldest-snapshot eviction while retaining at least the newest snapshot.
- Rewind restore goes only through `Machine::restore_save_state()`, preserving the existing metadata compatibility checks and direct subsystem restore path.
- Phase 8.4 measures full-snapshot memory pressure through `MachineRewindStats`; Phase 8.7 replaces the original fixed baseline with `MachineSaveState::deep_size_bytes()`, a deterministic accounting of inline DTO storage plus owned dynamic snapshot payload bytes, excluding allocator overhead and process RSS. Compression and deltas remain optional future optimizations only if telemetry justifies them; coordinated multi-machine rewind is intentionally outside the Phase 8 scope.

#### Phase 8.5 host `.gbstate` integration contract

- `gb-cli run` exposes reproducible host I/O through `--state-in <file.gbstate>` and `--state-out <file.gbstate>`. The CLI loads the ROM first, restores `--state-in` through `Machine::restore_save_state()`, runs the configured frame/T-cycle budget, then captures and writes `--state-out`.
- When `--state-in` is present, the CLI does not load a pre-existing P1 cartridge battery-save file (`.sav` or `.gbsav`, depending on mapper storage policy) before restore. If cartridge persistence is enabled for the run, the save session baseline is initialized from the restored cartridge state so an unrelated cartridge save file is not immediately flushed over or mixed into the full-machine state.
- After a ROM is loaded, `gb-desktop` exposes root-menu actions immediately below `OPEN RECENT`: `SAVE STATE`, `LOAD STATE`, `STATE SLOT N`, and `AUTOLOAD OFF` / `AUTOLOAD SLOT N`. The launcher/no-ROM root menu hides those state actions to keep startup uncluttered. The slot selector is runtime-only and cycles through slots `1` through `4` without moving the menu selection away from `STATE SLOT N`; `LOAD STATE` stays visible but disabled until the selected ROM-related slot file exists. The persisted autoload selector cycles through `OFF` and slots `1` through `4`; after `OPEN ROM` / `OPEN RECENT`, desktop restores the configured slot only when the `.gbstate` file exists and silently continues from normal boot when it does not.
- Desktop slots are single-machine only and live next to the ROM under `<rom-dir>/states/<state-key>.slot<N>.gbstate`. The `state-key` follows the current save-key policy, including explicit keys, but does not require cartridge persistence to be enabled.
- Desktop load reads the selected slot, decodes the current `.gbstate` format, restores through `Machine::restore_save_state()`, clears live input and audio queue state, resynchronizes host pacing/RTC bookkeeping, and does not apply elapsed RTC off-session time. Failed load leaves the machine untouched; linked `DMG-04` 2-player Game Link and `DMG-07` 4-Player Adapter sessions keep save/load state disabled by design and are not a Phase 8 target.
- The `.gbstate` host integration boundary is now format version `9`; this local-development v9 includes the core-visible `HardwareRevision` axis plus the Slice 0/1/2/3/4/5/6 `SgbHost` payload and SGB profile metadata, including startup/acceptance, JOYP packet transport state, SGB-aware boot-ROM asset identity and SGB/SGB2 boot-ROM payload slots, SGB palette/LCD composition state, pending and completed 4 KiB `_TRN` transfer state, border tile/tilemap/palette state, `MASK_EN` mask/freeze state, active SGB attribute map, packed ATF memory, system palette memory, `PAL_PRI` state, `MLT_REQ` mode/selected-player state, SGB per-player input-slot masks plus pending input-slot changes, typed host backend request state for `SOUND` / `SOU_TRN` / `DATA_SND` / `DATA_TRN` / `JUMP`, and selected SGB NTSC/PAL/SGB2 NTSC profile identity, and intentionally does not migrate or accept older incompatible slot files. Rewind continues to store in-memory `MachineSaveState` DTOs directly and does not depend on the host file version. Phase 8.8 reused the same host integration boundary after the ROM-less cartridge DTO update.

#### Phase 8.6 desktop rewind integration

- `gb-desktop` owns a single-machine `MachineRewindBuffer` using `MachineRewindConfig::default()` and records frame/subframe snapshots only during normal unpaused runtime. The buffer is cleared on ROM load, reset/reconfigure, `.gbstate` load, and linked-session transitions.
- The default `Left Shift` hotkey performs continuous hold-to-rewind by restoring older snapshots instead of advancing normal emulation; `F1`/`F2` save/load `.gbstate`, `1`..`4` select the active state slot, `F9` flushes cartridge save data, and `F12` resets. Phase 8.7 removes the earlier one-step root `REWIND` action so rewind UX stays hold-based plus the `SYSTEM -> REWIND` policy submenu.
- Successful rewind restore uses the same host cleanup boundary as `.gbstate` load: clear live input/audio queue, reset frame pacing and host RTC sync, and reset the active cartridge-save baseline. Phase 8.6 intentionally excludes CLI rewind, persistent rewind history, compression, deltas, and linked-session rewind for `DMG-04` 2-player Game Link / `DMG-07` 4-Player Adapter sessions.

#### Phase 8.7 rewind polish, configuration, and telemetry

- Rewind configuration is now desktop-only persistent settings with defaults matching Phase 8.6 capture policy plus faster playback: enabled, ten seconds of history, one subframe capture per frame, 2x hold-to-rewind playback mapped to four snapshot restores per presented frame, and a 256 MiB accounted-payload cap. `SYSTEM -> REWIND` cycles the bounded option sets; rebuilding the core buffer clears old history whenever capture/capacity policy changes, while playback `SPEED` changes keep the existing history.
- The compact HUD is the detailed rewind feedback surface: `RW OFF`, `RW EMPTY`, available seconds plus snapshot count, active rewind state, and `MEM used/limit` based on core-accounted `MachineSaveState` payload bytes. Held rewind also renders a separate top-right `<< REW` indicator independent of the stats HUD, suppressed when rewind is off or unsupported. Empty-history rewind is treated as a normal UX state: no modal error is shown and held rewind skips normal advancement for that frame.
- Phase 8.7 still stores full snapshots only. CLI rewind is not planned because the supported rewind surface is desktop hold-to-rewind and the behavior is already covered by core/desktop tests. Linked-session save states and rewind for `DMG-04` 2-player Game Link and `DMG-07` 4-Player Adapter are also intentionally unsupported. Compression, delta encoding, and richer frontend telemetry/settings remain optional future work only if measured memory or UX data justifies them.

#### Phase 8.8 ROM-less cartridge save-state DTOs

- Cartridge runtime save-state DTOs no longer embed immutable cartridge ROM bytes. They capture only mapper-owned mutable state: RAM payloads, selected banks, enable flags, RTC/camera runtime state, registers, and other cartridge-local latches needed to continue execution over the already-loaded ROM.
- `Machine::restore_save_state()` still validates metadata first, including ROM fingerprint, cartridge kind, model, hardware revision, execution mode, overrides, startup mode, and boot-ROM fingerprint/mapping state. It then validates that the cartridge DTO matches the currently loaded device and mutable payload shapes before mutating CPU, PPU, APU, scheduler, memory, or other subsystems.
- Restore rebuilds cartridge runtime in place over the loaded compatible ROM instead of reconstructing a cartridge device from serialized ROM bytes. Mapper mismatches, RAM-length mismatches, camera-frame shape mismatches, corrupt payloads, and wrong `.gbstate` versions fail before partially applying the restore.
- `.gbstate` keeps the same `GBSTATE\0` magic and `.gbstate` extension. Unsupported non-current versions are rejected rather than migrated, so old incompatible desktop slot files must be recreated.
- Rewind continues to store full `MachineSaveState` snapshots in memory, but `MachineSaveState::deep_size_bytes()` now scales with mutable cartridge payloads rather than cartridge ROM size. The current memory remeasure confirms that typical MBC1/MBC3 titles fit the default `10s` / `256 MiB` / `1` subframe-per-frame policy, while RAM-heavy `128 KiB` cartridges and Pocket Camera can use the existing `MEMORY 512MB` or `SUBFR OFF` desktop options when longer history is desired.

#### Phase 8.9 desktop Fast Forward and host action bindings

- `gb-desktop` adds a frontend-only hold-to-Fast-Forward path. It advances multiple presented-frame steps per host frame according to the desktop `SPEED` option and skips intermediate presentation; it does not alter `gb-core` T-cycle timing, scheduler state, save-state payloads, or cartridge persistence semantics.
- The default Fast Forward hotkey is `Right Shift`. `SYSTEM -> F-FORWARD` owns the persistent policy with `F-FORWARD ON/OFF`, `SPEED 2x/4x/8x` (default `2x`, retuned so visible `2x` uses the old `4x` batch), `DEFAULTS`, and `RETURN`; `ON/OFF` controls availability, while `Right Shift` and the mappable controller action are the momentary holds that activate Fast Forward when availability is `ON`.
- While Fast Forward is active, the desktop HUD renders a top-right `FF >>` indicator using the same overlay slot as `<< REW`. Rewind keeps priority if both holds are active, so rewind restore and its `<< REW` indicator remain deterministic.
- Fast Forward suppresses SDL playback-audio capture/submission and rewind-history capture while held to avoid host queue/backlog work, then resumes playback cleanly after release. Audio recording remains attached to the core capture boundary and continues writing the captured hardware stream.
- The `INPUT -> GAMEPAD` submenu adds optional, persisted host-action bindings for `SAVE STATE`, `LOAD STATE`, `REWIND`, and `F-FORWARD`. They default to `NONE` so existing joypad/menu bindings do not change, and save/load/rewind availability keeps the same single-machine restrictions defined by Phase 8.5/8.6.

#### Phase 8 closure note

- Phase 8 is considered closed for the strict single-machine save-state and rewind scope: durable `.gbstate` DTOs, CLI/desktop `.gbstate` save/load, desktop slot/autoload UX, core rewind buffering, desktop hold-to-rewind, desktop Fast Forward host pacing, optional controller host-action bindings, and restore-continuation validation are all in place.
- CLI rewind is intentionally not required: manual CLI rewind is not a target workflow, and rewind correctness is validated through the core and desktop rewind tests.
- Linked-session save states and rewind for `DMG-04` 2-player Game Link and `DMG-07` 4-Player Adapter sessions are intentionally unsupported, not hidden blockers for Phase 8.
- Compression, delta encoding, manual `.gbstate` file dialogs, and richer telemetry/settings are not Phase 8 blockers. If future measured memory pressure makes rewind optimization necessary, evaluate keyframes plus deltas first, then optional compression, then any increase to desktop memory defaults.
- There are no remaining Phase 8 TODOs in `docs/TODO.md`; future save-state or rewind work should open a new focused roadmap item instead of reopening Phase 8 by default.

#### Risks if delayed or underspecified

- final hardening work lacks a stable save/load foundation
- frontend-specific storage decisions leak into core semantics
- restore paths reconstruct only visible registers and lose hidden temporal state
- cartridge persistence and whole-machine save states become conflated
- debugger or replay tooling grows a second incompatible serialization path
