# Modularization — Creating `gb-api`

#### Purpose

This document specifies the creation of `gb-api`, a narrow and stable facade crate that sits between the frontends (`gb-desktop`, `gb-cli`, `gb-benchmark`, and future host integrations) and `gb-core`. The goal is to give consumers a small, plain-old-data (POD) contract — a `trait Emulator` plus frontier types for configuration, framebuffer, audio, input, cartridge persistence, and an opaque save-state blob — instead of importing 200+ symbols from `gb-core` directly. The plan is deliberately scoped to `gb-api` alone. A future `gb-base` (the home of pure domain types) is **not** built here, but the facade is **pre-wired** so that introducing it later is a localized change: every domain-type re-export is centralized behind one module, the contract lives in one place, and persistence is decoupled via an opaque versioned byte blob produced by `gb-persistence`. `gb-base`, a feature-split `gb-core-2`, a C FFI `gb-lib`, and `gb-libretro` are explicitly out of scope and mentioned only as what this work unlocks.

---

## 1. Current state

The workspace has six members: `gb-core`, `gb-test-runner`, `gb-benchmark`, `gb-cli`, `gb-persistence`, `gb-desktop` (workspace version `0.2.1`). All frontends depend on `gb-core` directly.

Concrete observations from the current code:

- **`gb-core` exposes a very wide surface.** `crates/gb-core/src/lib.rs` has 21 `pub use` blocks re-exporting well over 200 symbols across `apu`, `boot`, `bus`, `cartridge`, `cpu`, `debugger`, `dma`, `external_port`, `interrupts`, `joypad`, `link`, `machine`, `model`, `ppu`, `rewind`, `save_state`, `scheduler`, `serial`, and `sgb`. A consumer that only wants "run a ROM, get a framebuffer" imports from the same namespace as a consumer that wants register-level CPU traces.
- **The central type is generic over tracing.** `Machine<S: TraceSink + TraceSnapshotProvider>` (`machine.rs`, default `Machine<S = TraceBuffer>` at `machine.rs:190`) is parameterized by its trace sink. `Machine::new` builds `Machine<TraceBuffer>` (full in-memory trace), `Machine::new_summary` builds `Machine<TraceSummaryBuffer>` (summary-only), and `Machine::with_tracer` accepts a custom sink (`machine.rs:296`, `:302`, `:333`). This generic leaks debugger/tracing concepts into every consumer that names the type.
- **A frame is run by polling, one T-cycle at a time.** Frontends call `step_t_cycle()` (`machine/step.rs:1160`) in a loop and detect a frame boundary by watching `at_frame_origin()` transition `false -> true`, where the origin is `ppu().ly() == 0 && ppu().line_dot() == 0`. In `gb-cli` this is the loop in `crates/gb-cli/src/run/execution.rs:117-172` with the origin check defined at `crates/gb-cli/src/run/machine.rs:60-64`. Per step the loop also drains serial output via `take_serial_output_bytes()` and applies queued joypad input.
- **Per-frame host work is not a clean step-and-present loop.** `gb-desktop`'s `step_until_next_frame` (`crates/gb-desktop/src/frontend/frame_loop.rs:605-800+`) does substantial per-T-cycle work *inside* the stepping loop: audio capture reads `&Apu` every cycle (`audio.rs:249` `capture_t_cycle(apu: &Apu)`, called at `frame_loop.rs:798`), RTC sync per cycle, printer drain, trace/watch capture, benchmark stimulus, host event polling every `INPUT_POLL_SLICE_T_CYCLES`, and ~50 frame-telemetry counters sampling `ppu().ly()` / `line_dot()` / `mode0_start_dot()` / `lcd_state()` mid-frame. A whole-frame black box that only drains audio *after* the frame cannot reproduce per-cycle APU sampling or mid-frame polling.
- **Save files and save states are two different codecs.** Battery-RAM/RTC save *files* are orchestrated by the CLI run path's save session (`crates/gb-cli/src/run/save_session.rs`), which calls `machine.cartridge().persistence_metadata()` (`:41`, `:115`) and `machine.cartridge().persistent_state()` (`:55`, `:57`, `:108` …) at frame boundaries and `machine.restore_cartridge_persistent_state(&state)` (`:80`) on load. The underlying accessors are `pub(in crate::cartridge)` on the device/MBC types (`cartridge/device.rs:294,362`), reachable only through `Machine::cartridge() -> &CartridgeSlot`. Whole-machine save *states* are a separate path: `Machine::capture_save_state()` (`machine.rs:649`) returns the structured serde type `MachineSaveState`, and the *versioned byte codec* (magic + `u16` version + metadata + `ciborium` payload) lives in `gb-persistence` (`machine_state/envelope.rs`: `MachineSaveStateEnvelope`, `encode_machine_save_state_envelope` / `decode_machine_save_state_envelope`), which `gb-cli` already uses via `run/state.rs`.
- **Consumer coupling differs sharply.**
  - `gb-desktop` **needs internals**: it imports snapshot types (`CpuSnapshot`, `PpuSnapshot`, `ApuSnapshot`, …), tracing observers (`MachineStepObserver`, `MachineStepRegion`), cartridge introspection (`CartridgeDiagnostic`, `CartridgeMappedRomWindow`), rewind (`MachineRewindBuffer`), and multi-machine link types (`LinkedMachines`, `Dmg07Port`, `PokemonMysteryGiftSession`, …). It introspects the emulator at every level, and its play loop does per-cycle audio/telemetry work (above).
  - `gb-cli`'s **run path does not** need internals. Its `CliMachine` (`crates/gb-cli/src/run/machine.rs`) is an enum dispatching over `Machine<TraceBuffer>` and `Machine<TraceSummaryBuffer>`, and its run loop uses only the clean public surface: `new`/`new_summary`, `load_cartridge`, `step_t_cycle`, `set_joypad_button_pressed`, `take_serial_output_bytes`, `ppu().framebuffer()`/`cgb_framebuffer_rgb555()`/`sgb_*_framebuffer_rgb555()`, `capture_save_state`/`restore_save_state`, and cartridge persistence accessors. The CLI's *other* subcommands (test, inspect, trace) genuinely need `gb-core` and stay on it.
  - `gb-benchmark` is the lightest: it imports only `JoypadButton` and the constants `DMG_T_CYCLES_PER_FRAME` and `DMG_T_CYCLES_PER_SECOND`.

The net picture: two consumers (`gb-cli` run path, `gb-benchmark`) want a narrow contract, the third (`gb-desktop`) wants a narrow contract for construction/presentation but keeps a per-cycle inner loop, yet all reach into the full `gb-core` namespace and name the tracing-generic `Machine<S>` directly.

---

## 2. Target architecture

The agreed layering, from consumers down to base:

```
frontends                      gb-desktop, gb-cli, gb-benchmark
(+ future gb-libretro, gb-lib)        │
                                      ▼
facade                             gb-api          ← THIS DOCUMENT
                                      │
implementation        gb-core  [+ future gb-core-2 by feature]   gb-persistence
                                      │
base                          gb-base  (FUTURE — not built here)
```

- **`gb-api`** is the only new crate created by this plan. It is a narrow facade: a `trait Emulator` (with a concrete `Gameboy` implementation backed by `gb_core::Machine`) plus POD frontier types. It must not leak `gb-core` internals — no tracing generic, no snapshot/trace types in its public surface.
- **`gb-api` depends on the existing `gb-core`**, plus `gb-persistence` behind the `persistence` feature (Section 3.3). No `gb-base` yet.
- `gb-persistence` continues to live at the implementation layer; `gb-api` treats save-state as an opaque versioned byte blob produced by `gb-persistence`, so persistence stays decoupled from any concrete core.

> **Scope of THIS document = `gb-api` only.**
> `gb-base`, a feature-split `gb-core-2`, the C FFI `gb-lib` (cdylib/staticlib), and `gb-libretro` are **future work**. They are referenced here solely as what this plan unlocks and as a single forward "future hook" (Section 6). Do not implement their phases as part of this work.

**What this unlocks later (out of scope here):**

- **`gb-base`** — extract pure domain/hardware types (console identities, cartridge format metadata, persistent-state shapes, the audio sample type, button mappings, timing constants) into a dependency-light base crate that both the cores and `gb-persistence` can share.
- **`gb-core-2`** — an alternative or feature-split core implementation behind the same `gb-api` contract.
- **`gb-lib`** — a C FFI surface (cdylib/staticlib) built on `gb-api`.
- **`gb-libretro`** — a RetroArch core, also built on `gb-api`.

---

## 3. Design principles

The single most important constraint is that the facade is **`gb-base`-ready**: the day `gb-base` exists, hooking it in must be a localized diff, not a workspace-wide churn. Everything below serves that constraint.

### 3.1 One re-export seam: `gb_api::types`

All domain-type re-exports are centralized behind a single module. Today it re-exports from `gb_core`; the day `gb-base` exists, this one file flips to `gb_base` and nothing else in the workspace changes.

```rust
// crates/gb-api/src/types.rs
//
// Single home for every domain-type re-export the facade exposes.
// When gb-base arrives, change the `gb_core` paths below to `gb_base`
// (see Section 6). No consumer import changes.

pub use gb_core::{
    // Console / hardware identity
    ConsoleModel, HardwareRevision, HostPlatform, OperatingMode,
    SgbVideoStandard, SgbHostProfile, StartupMode, CompatibilityPolicy,
    // Boot ROM assets (keyed per-asset bytes; see EmulatorConfig)
    BootRomAssets,
    // Input
    JoypadButton,
    // Audio sample (left/right i32)
    ApuHostSample,
    // Cartridge persistence (domain shapes only)
    PersistentCartState, CartridgePersistenceMetadata,
    // Timing constants
    DMG_T_CYCLES_PER_FRAME, DMG_T_CYCLES_PER_SECOND,
};
```

All of these genuinely live in `gb-core`'s re-exports (`lib.rs:83-115`) and exist in modules that can move to `gb-base` later (`model.rs`, `boot.rs:143`, `joypad.rs:12`, `apu/output.rs:24`, `cartridge.rs:1519`/`:1548`, `rewind.rs:9-10`). Consumers always write `gb_api::types::JoypadButton`, never `gb_core::JoypadButton`. The facade itself also imports domain types only through `crate::types`, so there is exactly one place that names the underlying crate.

`ExecutionMode` is deliberately **not** in the seam: it is a CLI-level concept, not a `gb-core` domain type, and is mapped to `CompatibilityPolicy` before construction (Section 4.1).

### 3.2 Single-home contract

The `trait Emulator` and the frontier POD types are defined in exactly one module (`gb_api::emulator` + `gb_api::frontier`). The contract never references `gb_core` types directly; it references them only via `crate::types`. This keeps the contract relocatable: when `gb-base` arrives, the trait can move to (or be re-exported from) `gb-base` without touching a single consumer, because consumers already depend on it through `gb_api`.

```rust
// crates/gb-api/src/emulator.rs
use crate::frontier::{
    AudioBatch, EmulatorConfig, EmulatorError, Framebuffer, FrameOutput, SaveStateBlob,
};
use crate::types::{CartridgePersistenceMetadata, JoypadButton, PersistentCartState};

/// View handed to a per-cycle callback during `run_frame_with`.
/// Borrows the live machine so a host can sample audio and telemetry
/// at T-cycle granularity, matching gb-desktop's inner loop.
pub struct CycleView<'a> {
    pub audio_sample: crate::types::ApuHostSample,
    pub machine: &'a dyn EmulatorObserve, // narrow read-only telemetry surface
}

/// The narrow, stable contract every frontend programs against.
/// No tracing generic, no snapshot types, no `gb-core` paths.
pub trait Emulator {
    /// Construct from a POD config (hardware revision, boot mode, host platform, …).
    fn new(config: EmulatorConfig) -> Result<Self, EmulatorError>
    where
        Self: Sized;

    /// Load ROM bytes and reset runtime, preserving input state.
    fn load_rom(&mut self, rom: Vec<u8>) -> Result<(), EmulatorError>;

    /// Advance exactly one whole video frame. Implementations step T-cycles
    /// internally and stop at the next frame origin. Audio for the frame is
    /// retrievable via `take_audio`. Sufficient for the CLI run path and any
    /// headless host.
    fn run_frame(&mut self) -> Result<FrameOutput, EmulatorError>;

    /// Advance one whole video frame, invoking `per_cycle` after every stepped
    /// T-cycle with a borrowed `CycleView`. This is the hook gb-desktop's
    /// interactive loop uses for per-cycle audio capture, RTC sync, event
    /// polling, and telemetry; `run_frame` is the `per_cycle = |_| {}` case.
    fn run_frame_with(
        &mut self,
        per_cycle: &mut dyn FnMut(&CycleView<'_>),
    ) -> Result<FrameOutput, EmulatorError>;

    /// Stable 160x144 video output of the last completed frame.
    fn framebuffer(&self) -> Framebuffer<'_>;

    /// Drain audio produced during the last `run_frame`/`run_frame_with`.
    fn take_audio(&mut self) -> AudioBatch;

    /// Drain serial output bytes produced during the last frame.
    fn take_serial_output(&mut self) -> Vec<u8>;

    /// Queue an input edge for the next stepped cycle.
    fn set_button(&mut self, button: JoypadButton, pressed: bool);

    /// Capture the cartridge's battery/RTC persistent state (save-file domain).
    fn cartridge_persistent_state(&self) -> PersistentCartState;

    /// Static persistence metadata for the loaded cartridge (save-file domain).
    fn cartridge_persistence_metadata(&self) -> CartridgePersistenceMetadata;

    /// Restore cartridge battery/RTC state from a loaded save file.
    fn restore_cartridge_persistent_state(
        &mut self,
        state: &PersistentCartState,
    ) -> Result<(), EmulatorError>;

    /// Reload the current ROM and reset runtime.
    fn reset(&mut self) -> Result<(), EmulatorError>;
}

/// Whole-machine save state. Gated behind the `persistence` feature because
/// the versioned byte codec lives in `gb-persistence` (Section 3.3).
#[cfg(feature = "persistence")]
pub trait EmulatorSaveState: Emulator {
    /// Snapshot whole-machine state as an opaque, versioned byte blob.
    fn save_state(&self) -> Result<SaveStateBlob, EmulatorError>;

    /// Restore from a blob; validates magic, version, and metadata.
    fn restore_state(&mut self, blob: &SaveStateBlob) -> Result<(), EmulatorError>;
}
```

`save_state` / `restore_state` are split into a `persistence`-gated `EmulatorSaveState` extension trait, not the base `Emulator` trait, because their encoder lives in `gb-persistence` (Section 3.3). Under `--no-default-features` the core `Emulator` contract still compiles; only whole-machine save-state is unavailable, which is correct for a minimal FFI core.

### 3.3 Opaque, versioned save-state blob (codec home = `gb-persistence`)

The facade never exposes `MachineSaveState`, `MachineSaveStateRestoreError`, or `PersistentCartState` byte layouts as its contract. `save_state()` returns a `SaveStateBlob` (a versioned `Vec<u8>` wrapper); `restore_state()` consumes one.

The opaque boundary is the **`gb-persistence` envelope**, *not* the `gb-core` struct. Inside the concrete `Gameboy`, `save_state()` calls `MachineSaveStateEnvelope::new(machine.capture_save_state())` and `encode_machine_save_state_envelope(&envelope)` to produce the magic-tagged, version-stamped bytes; `restore_state()` calls `decode_machine_save_state_envelope(&blob.bytes)` and feeds the structured payload into `Machine::restore_save_state()`. `MachineSaveState` is therefore a *codec implementation detail* of `gb-persistence`, never part of the facade surface, so a future `gb-base`/`gb-core-2` can carry its own snapshot type behind the same blob without changing any frontend. This is exactly the path `gb-cli` already uses through `run/state.rs`.

Because the encoder lives in `gb-persistence`, this whole capability is gated behind the `persistence` feature (the `EmulatorSaveState` extension trait, Section 3.2). The blob format carries its own magic + `u16` version tag for forward/backward handling.

### 3.4 Feature-gated optional subsystems

Optional subsystems are Cargo features in `gb-api`, off by default unless a frontend opts in, so future thin artifacts (`gb-lib`, `gb-libretro`) compile only what they use. Note that *save-file* orchestration (cartridge battery/RTC, on the base `Emulator` trait) and *whole-machine save-state* encoding (the `gb-persistence` envelope) are distinct concerns gated separately:

- `persistence` — whole-machine save-state encode/decode via `gb-persistence` (`MachineSaveStateEnvelope` + `encode`/`decode`). Enables the `EmulatorSaveState` extension trait. Pulls `gb-persistence`.
- `audio` — `AudioBatch` collection and host-rate helpers. With the feature off, `take_audio` returns an empty `AudioBatch` and `CycleView::audio_sample` is still populated (it is a plain core read), so per-cycle hosts work without the batching helpers.

Cartridge battery/RTC save-file capture/restore (`cartridge_persistent_state` etc.) is on the *base* trait and needs no feature — it reads `gb-core` domain shapes directly with no `gb-persistence` codec. The default build for the in-tree frontends enables both `persistence` and `audio`; a future FFI core could take `default-features = false` plus only what it needs.

---

## 4. The `gb-api` surface

Two parts: the `trait Emulator` method set (Section 3.2) and the POD frontier types below. Each is justified by a concrete consumer need from the findings.

### 4.1 Frontier POD types

```rust
// crates/gb-api/src/frontier.rs
use crate::types::{
    ApuHostSample, BootRomAssets, CompatibilityPolicy, ConsoleModel, HardwareRevision,
    HostPlatform, OperatingMode, SgbHostProfile, StartupMode,
};

/// Construction config. Carries the same hardware-identity choices as
/// `gb_core::MachineConfig`, but is owned by the facade and translated to a
/// `MachineConfig` in `gameboy.rs` (see notes below — it is NOT a 1:1 mirror).
pub struct EmulatorConfig {
    pub console_model: ConsoleModel,
    pub operating_mode: OperatingMode,
    pub revision: HardwareRevision,
    pub host_platform: HostPlatform,
    pub sgb_profile: Option<SgbHostProfile>,
    pub startup_mode: StartupMode,
    pub compatibility: CompatibilityPolicy,
    /// Boot ROM assets. For consoles needing a single blob (DMG/MGB), the
    /// frontend may pass a one-asset `BootRomAssets`; CGB/SGB need keyed
    /// assets, which a raw `Vec<u8>` cannot express, so the facade takes
    /// `BootRomAssets` directly rather than raw bytes.
    pub boot_rom_assets: BootRomAssets,
}
```

Translation notes (`gameboy.rs`, P1):

- `gb_core::MachineConfig` (`model.rs:618`) has fields `console_model, operating_mode, revision, host_platform, sgb_profile, startup_mode, boot_rom_assets: BootRomAssets, compatibility`. There is **no** `execution_mode` field and boot ROMs are **not** raw bytes.
- `ExecutionMode` (a CLI concept) is mapped to `CompatibilityPolicy` by the caller (the CLI does this today via `compatibility_for_execution_mode`, `gb-cli` `report.rs:32-38`); the facade takes the resulting `CompatibilityPolicy` so it stays a pure domain type.
- `boot_rom_assets` maps straight through to `MachineConfig` via `with_boot_rom_assets` (`model.rs:679`); frontends build `BootRomAssets` with `BootRomAssets::with_bytes` / `with_asset_bytes` (`boot.rs:264,273`).

```rust
/// Borrowed video output for the last completed frame.
pub enum Framebuffer<'a> {
    /// DMG/CGB core indices: 160x144 `u8` (DMG = 2-bit index 0..=3,
    /// CGB = palette index). From `ppu().framebuffer()`.
    Indexed { width: u16, height: u16, pixels: &'a [u8] },
    /// CGB true colour: 160x144 RGB555 `u16`. From `cgb_framebuffer_rgb555()`.
    Rgb555 { width: u16, height: u16, pixels: &'a [u16] },
    /// SGB frame including border, RGB555. From `sgb_framebuffer_rgb555()`.
    SgbRgb555 { width: u16, height: u16, pixels: &'a [u16] },
}

/// Stereo audio produced during a frame. `take_audio()` drains this.
/// Sample type and width come straight from the seam — `ApuHostSample`
/// (`apu/output.rs:24`, `{ left: i32, right: i32 }`) — so a future gb-base
/// swap flips the whole audio surface with the Section 6 path swap and
/// touches no call site.
pub struct AudioBatch {
    pub samples: Vec<ApuHostSample>,
}

/// What a single frame produced, for a frontend that wants it in one return
/// value rather than separate getters.
pub struct FrameOutput {
    pub frame_index: u64,
    pub serial_bytes: Vec<u8>,
}

/// Opaque, versioned whole-machine snapshot. Layout is private to gb-api;
/// the bytes are produced by the `gb-persistence` envelope (Section 3.3).
#[cfg(feature = "persistence")]
pub struct SaveStateBlob {
    pub(crate) bytes: Vec<u8>,
}

#[derive(Debug)]
pub enum EmulatorError {
    CartridgeLoad(String),
    StateRestore(String),
    Config(String),
}
```

### 4.2 Method-to-consumer justification

| Surface element | Backed by (`gb-core` / `gb-persistence`) | Consumer that needs it |
| --- | --- | --- |
| `Emulator::new(EmulatorConfig)` | `Machine::new` / `new_summary` + `MachineConfig` (`machine.rs:296,302`) | `gb-cli` run path (`run/execution.rs:67`), `gb-desktop` play loop |
| `load_rom(Vec<u8>)` | `Machine::load_cartridge` (`machine/access.rs:155`) | `gb-cli` run (`execution.rs:69`), `gb-desktop` |
| `run_frame()` | loop over `step_t_cycle` (`step.rs:1160`) + `at_frame_origin` (`run/machine.rs:62`) | `gb-cli` run (`execution.rs:136-171`), headless hosts |
| `run_frame_with(per_cycle)` | same loop, invoking `per_cycle` each cycle; `CycleView.audio_sample` from `apu().host_output_sample()` (`apu.rs:474`) | `gb-desktop` interactive loop (per-cycle audio/RTC/telemetry, `frame_loop.rs:605-800+`) |
| `framebuffer()` | `ppu().framebuffer()` (`ppu/api.rs:1571`), `cgb_framebuffer_rgb555()` (`ppu/api.rs:1575`), `sgb_framebuffer_rgb555()` (`machine.rs:485`) | `gb-cli` run (`execution.rs:181-190`), `gb-desktop` |
| `take_audio()` | `apu().host_output_sample()` (`apu.rs:474`) collected per step; `ApuHostSample` (`apu/output.rs:24`) | `gb-desktop` (audio output); empty when `audio` feature off |
| `take_serial_output()` | `take_serial_output_bytes()` (`machine.rs:527`) | `gb-cli` run (`execution.rs:146`) |
| `set_button(JoypadButton, bool)` | `set_joypad_button_pressed` (`machine.rs:568`) | `gb-cli` run (`execution.rs:132`), `gb-benchmark`, `gb-desktop` |
| `cartridge_persistent_state()` | `Machine::cartridge().persistent_state()` (`cartridge/device.rs:362`) → `PersistentCartState` | `gb-cli` run `--save-dir` (`save_session.rs:55,82,108`), `gb-desktop` |
| `cartridge_persistence_metadata()` | `Machine::cartridge().persistence_metadata()` (`cartridge/device.rs:294`) → `CartridgePersistenceMetadata` | `gb-cli` run `--save-dir` (`save_session.rs:41,115`) |
| `restore_cartridge_persistent_state(&PersistentCartState)` | `Machine::restore_cartridge_persistent_state` (`save_session.rs:80` call site) | `gb-cli` run `--save-dir`, `gb-desktop` |
| `save_state()` / `restore_state()` *(feature `persistence`)* | `capture_save_state` / `restore_save_state` (`machine.rs:649,685`) wrapped by `MachineSaveStateEnvelope` + `encode`/`decode` (`gb-persistence` `machine_state/envelope.rs`) | `gb-cli` run (`execution.rs:203`), `gb-desktop` |
| `reset()` | `load_cartridge` re-entry | `gb-desktop` |
| `EmulatorConfig` → `MachineConfig` | translated in `gameboy.rs` (Section 4.1) | all three consumers |
| `types::{DMG_T_CYCLES_PER_FRAME, DMG_T_CYCLES_PER_SECOND}` | `rewind.rs:9,10` | `gb-benchmark` (frame pacing / speed %) |

**Explicitly NOT on the facade** (these stay on a direct `gb-core` dependency for the consumer that needs them — `gb-desktop` and the CLI's non-run subcommands): `MachineStepObserver`/`MachineStepRegion`, all `*Snapshot` types, `MachineRewindBuffer`/rewind config, `LinkedMachines`/`Dmg07Port`, the Pokémon IR session types, `CartridgeDiagnostic`/`CartridgeMappedRomWindow`, `Tracer`/`TraceBuffer`/`TraceSummaryBuffer`, and the `Machine<S>` tracing generic itself (`machine.rs:190`). The facade picks the buffered/summary specialization internally and never exposes `S`.

---

## 5. Phases

Each phase is a sequence of commits on the single branch (Section 8) and ends with the full CI gate green. `gb-test-runner` is untouched throughout (it stays on `gb-core` directly). The consumer order is **cleanest proof first**: the `gb-cli` run path is the smallest, most self-contained loop (no per-cycle host work), so it migrates before `gb-desktop`, whose interactive loop is the hardest consumer and exercises the `run_frame_with` hook.

### P0 — Scaffold `gb-api` and the `types` seam

**Goal.** Create an empty-but-real `gb-api` crate with the single re-export seam in place and nothing else, so the seam exists before any contract is written.

**Steps.**
1. Add `crates/gb-api` to the workspace `members` list in `Cargo.toml`.
2. Create `crates/gb-api/Cargo.toml` (version `0.2.1`, edition matching the workspace) depending on `gb-core`; add an optional `gb-persistence` dependency and declare `persistence` (pulls `gb-persistence`) and `audio` features, both default-on for in-tree frontends (logic wired in P5).
3. Create `crates/gb-api/src/lib.rs` with module declarations `pub mod types;` and stubs for `frontier`/`emulator` (added in P1).
4. Create `crates/gb-api/src/types.rs` with the centralized `pub use gb_core::{…}` block (Section 3.1), including `BootRomAssets`, `ApuHostSample`, `PersistentCartState`, and `CartridgePersistenceMetadata`.
5. Add a doc note at the top of `types.rs` recording the `gb-base` flip procedure (Section 6).

**Files touched.** `Cargo.toml`; new `crates/gb-api/Cargo.toml`, `crates/gb-api/src/lib.rs`, `crates/gb-api/src/types.rs`.

**Definition of Done.** `gb-api` compiles and is a workspace member; `gb_api::types::JoypadButton`, `::ApuHostSample`, `::CartridgePersistenceMetadata`, and `::BootRomAssets` all resolve; no consumer changed yet.

**CI gate.** `cargo fmt-check`, `cargo lint`, `cargo tests`, `cargo rom-report blargg` all green.

**Risk & rollback.** Negligible — additive only. Rollback = drop the crate dir and the `members` entry.

### P1 — Define the contract and implement it for `gb_core::Machine`

**Goal.** Land `trait Emulator` (and the `persistence`-gated `EmulatorSaveState`), the POD frontier types, and a concrete `Gameboy` backed by `Machine`, with a parity test proving the facade produces the same frame output as direct `gb-core` use.

**Steps.**
1. Add `crates/gb-api/src/frontier.rs` (Section 4.1) and `crates/gb-api/src/emulator.rs` (Section 3.2), referencing domain types only via `crate::types`.
2. Add `crates/gb-api/src/gameboy.rs` defining `pub struct Gameboy { inner: Machine<TraceSummaryBuffer>, … }` and `impl Emulator for Gameboy` (plus `impl EmulatorSaveState` under `#[cfg(feature = "persistence")]`). Translate `EmulatorConfig` → `MachineConfig` per Section 4.1 (`ExecutionMode` is the caller's job; `boot_rom_assets` via `with_boot_rom_assets`). Implement `run_frame_with` as the `step_t_cycle` + `at_frame_origin` loop mirroring `run/execution.rs:117-172`, invoking `per_cycle` after each step with a `CycleView` built from `apu().host_output_sample()`; implement `run_frame` as `run_frame_with(&mut |_| {})`. Map `Framebuffer` variants from the PPU getters; route cartridge persistence through `cartridge().persistent_state()` / `persistence_metadata()` / `restore_cartridge_persistent_state`; wrap `capture_save_state`/`restore_save_state` in the `gb-persistence` envelope (Section 3.3).
3. Re-export the public contract from `lib.rs`: `pub use emulator::{Emulator, CycleView}; pub use gameboy::Gameboy; pub use frontier::*;` (and `EmulatorSaveState` under the feature).
4. Add `crates/gb-api/tests/parity.rs`: run a fixed frame count of a named in-tree ROM (e.g. a blargg `cpu_instrs` sub-ROM already vendored for the report) through both `Gameboy` and a hand-written direct-`Machine` loop. Assert byte-equality on **each `Framebuffer` variant the ROM exercises** (Indexed for DMG; add CGB `Rgb555` and SGB `SgbRgb555` cases with the appropriate config), on `take_serial_output`, and — gathering audio via `run_frame_with` — on the per-frame `AudioBatch` (`ApuHostSample` sequence). If audio parity cannot be proven, that is itself the signal the per-cycle hook is wired wrong.
5. Add a public-API leakage guard: a `#[test]` (cargo-public-api snapshot, or a documented `cargo doc --no-deps` audit step checked in CI) asserting no `gb_core::*` path appears in `gb-api`'s public signatures.

**Files touched.** New `crates/gb-api/src/frontier.rs`, `emulator.rs`, `gameboy.rs`, `tests/parity.rs`, the API-leakage guard test; edit `crates/gb-api/src/lib.rs`.

**Definition of Done.** `Gameboy` implements `Emulator` (and `EmulatorSaveState` under `persistence`); the parity test passes on every exercised framebuffer variant plus serial and audio; the API-leakage guard test is green (concrete check, not a manual eyeball).

**CI gate.** Full gate green (parity + leakage tests run under `cargo tests`).

**Risk & rollback.** Subtle frame-boundary or per-cycle audio mismatch vs. the CLI/desktop loops. Mitigated by the parity test asserting byte-equality on framebuffer, serial, and audio. Rollback = revert the contract commits; P0 seam survives.

### P2 — Migrate the `gb-cli` run path (cleanest proof first)

**Goal.** Move only the `run` subcommand onto `gb-api`, including `--save-dir` save-file orchestration; keep the direct `gb-core` dependency for `test`/`inspect`/`trace` tooling.

**Steps.**
1. Add `gb-api` (with `persistence` + `audio` features) to `crates/gb-cli/Cargo.toml` (keep `gb-core`).
2. Replace `CliMachine`'s run usage (`crates/gb-cli/src/run/machine.rs`, `run/execution.rs`) with `gb_api::Gameboy`. The enum-over-`TraceBuffer`/`TraceSummaryBuffer` dispatch collapses — the facade owns the sink choice — so `CliMachine` either disappears for the run path or wraps `Gameboy`.
3. Map CLI options to `EmulatorConfig` (translate `ExecutionMode` → `CompatibilityPolicy`, build `BootRomAssets`); keep benchmark-stimulus application via `set_button`; keep serial drain via `take_serial_output`; keep whole-machine save/restore via the `SaveStateBlob` API.
4. Re-point the save session (`crates/gb-cli/src/run/save_session.rs`) onto the facade: `cartridge_persistent_state()` / `cartridge_persistence_metadata()` / `restore_cartridge_persistent_state()` replace the direct `machine.cartridge()…` calls, so `--save-dir` battery/RTC flushing keeps working through `gb-api`.
5. Confirm non-run subcommands still import `gb_core` directly and are unchanged.

**Files touched.** `crates/gb-cli/Cargo.toml`; `crates/gb-cli/src/run/*` (including `save_session.rs`). Non-run subcommands untouched.

**Definition of Done.** `cargo rom-report blargg` (driven through the CLI run path) passes via the facade; `--save-dir` save-file create/load round-trips through the facade; CLI test/inspect tooling unchanged.

**CI gate.** Full gate green — `cargo rom-report blargg` here directly exercises the migrated path.

**Risk & rollback.** ROM-report regression if `run_frame` diverges from the old loop, or save-file regression if the persistence accessors are miswired. Mitigated by the P1 parity test + running the blargg report and a `--save-dir` round-trip before committing. Rollback = revert run-path commits.

### P3 — Migrate `gb-desktop`'s play loop

**Goal.** Move `gb-desktop`'s **play loop** (config → load → run frame → present → input → save/restore) onto `gb-api` via `run_frame_with`, while leaving its debugger/inspection features on the direct `gb-core` dependency.

**Steps.**
1. Add `gb-api` (with `persistence` + `audio` features) to `crates/gb-desktop/Cargo.toml`, alongside the retained `gb-core` dependency.
2. Replace the play-loop construction/stepping/presentation calls with `gb_api::{Gameboy, Emulator}` and `gb_api::types::*`. Drive `step_until_next_frame`'s per-cycle work (audio `capture_t_cycle`, RTC sync, event polling, telemetry counters) through `run_frame_with`'s `CycleView`; route battery/RTC saves through `cartridge_persistent_state` and whole-machine saves through the `SaveStateBlob` API. Switch domain imports (`JoypadButton`, `HardwareRevision`, `ConsoleModel`, `StartupMode`, `ApuHostSample`, …) from `gb_core::` to `gb_api::types::`.
3. Telemetry that reads internals not on `CycleView` (e.g. raw `mode0_start_dot()`/`lcd_state()` for the diagnostic overlays) stays on the direct `gb-core` handle, as do debugger, snapshot, rewind, link, and IR-session code — these are deliberately not on the facade.
4. A/B the migrated loop against the retained `gb-core` path for audio/timing parity before removing any old code.

**Files touched.** `crates/gb-desktop/Cargo.toml`; the desktop run/present/input/audio modules (`frontend/frame_loop.rs`, `audio.rs`, …). No `gb-core` changes.

**Definition of Done.** `gb-desktop` builds and runs a ROM through `gb-api` with per-cycle audio intact; debugger/telemetry features still compile via `gb-core`.

**CI gate.** Full gate green.

**Risk & rollback.** Audio/timing regressions in the interactive loop — the hardest consumer. Mitigated by `run_frame_with` (the same per-cycle hook the P1 parity test exercises) and keeping `gb-core` available for A/B during migration. Rollback = revert desktop commits; `gb-api` unaffected.

### P4 — Migrate `gb-benchmark`

**Goal.** Move `gb-benchmark` onto `gb-api` for its emulation surface.

**Steps.**
1. Add `gb-api` to `crates/gb-benchmark/Cargo.toml`; remove the direct `gb-core` dependency if nothing else needs it.
2. Switch `JoypadButton` and `DMG_T_CYCLES_PER_FRAME` / `DMG_T_CYCLES_PER_SECOND` imports to `gb_api::types::*` (verified the only `gb-core` symbols used, across `stimulus.rs`, `timing.rs`, `case/input.rs`).
3. If the benchmark steps a machine, route it through `Gameboy`; otherwise it only needs the type/const re-exports.

**Files touched.** `crates/gb-benchmark/Cargo.toml`; benchmark stimulus/timing modules.

**Definition of Done.** `gb-benchmark` builds against `gb-api` only (or `gb-api` + a justified residual).

**CI gate.** Full gate green.

**Risk & rollback.** Lowest-risk phase (three symbols). Rollback = restore the `gb-core` dependency.

### P5 — Feature-gate optional subsystems

**Goal.** Finalize the `persistence` and `audio` features in `gb-api` so downstream artifacts can include only what they use.

**Steps.**
1. In `crates/gb-api/Cargo.toml`, make `persistence` pull `gb-persistence` (and enable the `EmulatorSaveState` trait + `SaveStateBlob`) and `audio` pull the audio-collection code; both default-on for the in-tree frontends, both omittable.
2. Gate the relevant `gameboy.rs` code paths behind `#[cfg(feature = "…")]`: `save_state`/`restore_state` under `persistence`; `take_audio` batching under `audio` (returns an empty `AudioBatch` when off — `CycleView::audio_sample` is unaffected). The base `Emulator` method set (including cartridge battery/RTC persistence, which needs no codec) is stable regardless of features.
3. Verify each frontend enables exactly the features it uses.

**Files touched.** `crates/gb-api/Cargo.toml`, `crates/gb-api/src/gameboy.rs`, frontend `Cargo.toml` feature selections.

**Definition of Done.** All four feature builds are green, enforced by an explicit command set in the gate below.

**CI gate.** Full gate green, plus the feature-matrix build/test commands run for this phase:

- `cargo build -p gb-api --no-default-features`
- `cargo build -p gb-api --no-default-features --features persistence`
- `cargo build -p gb-api --no-default-features --features audio`
- `cargo test -p gb-api --no-default-features --features "persistence audio"`

**Risk & rollback.** Feature-flag build breakage. Mitigated by the explicit build matrix above. Rollback = collapse features back to default-on.

> **Out of scope — unlocked next, separate document.** `gb-lib` (C FFI cdylib/staticlib) and `gb-libretro` are not part of this plan. They are unblocked once P0–P5 land and will be specified in their own document.

---

## 6. Future: hooking `gb-base`

When `gb-base` is later created (extracting the pure domain types — `ConsoleModel`, `HardwareRevision`, `HostPlatform`, `JoypadButton`, `ApuHostSample`, `PersistentCartState`, `CartridgePersistenceMetadata`, `BootRomAssets`, the SGB/timing constants), the change set is intentionally tiny because of the seam in Section 3.1:

1. **`gb_api::types` flips its re-export source** from `gb_core` to `gb_base`. One file.
2. **The contract relocates or re-exports.** `trait Emulator` either moves into `gb-base` and is re-exported from `gb-api`, or stays in `gb-api` re-exporting `gb-base` types — either way consumers keep importing `gb_api::Emulator`, so no consumer changes.
3. **The cores depend on `gb-base`.** `gb-core` (and any `gb-core-2`) re-export or consume the domain types from `gb-base` instead of defining them.
4. **`gb-persistence` depends on `gb-base`** for the domain shapes it serializes, staying off any concrete core.
5. **The save-state codec stays opaque.** Because `SaveStateBlob` already wraps the `gb-persistence` envelope (Section 3.3) and never exposes `MachineSaveState`, a future core swaps its own snapshot type behind the same blob — no persistence call site in the frontends changes, and the change is confined to `gb-persistence` + `gameboy.rs`.

The diff in `gb_api::types` is literally a path swap:

```rust
// BEFORE (this plan — gb-base does not exist yet)
pub use gb_core::{
    ConsoleModel, HardwareRevision, HostPlatform, JoypadButton, ApuHostSample,
    PersistentCartState, CartridgePersistenceMetadata, BootRomAssets,
    DMG_T_CYCLES_PER_FRAME, DMG_T_CYCLES_PER_SECOND,
    /* … */
};
```

```rust
// AFTER (the day gb-base lands)
pub use gb_base::{
    ConsoleModel, HardwareRevision, HostPlatform, JoypadButton, ApuHostSample,
    PersistentCartState, CartridgePersistenceMetadata, BootRomAssets,
    DMG_T_CYCLES_PER_FRAME, DMG_T_CYCLES_PER_SECOND,
    /* … */
};
```

Because every frontend already imports these as `gb_api::types::*`, the contract never names `gb_core` directly, and the audio sample type and save-state blob are both routed through the seam/envelope, the `gb-base` introduction is a localized diff confined to `gb-api`, `gb-persistence`, and the cores' internal dependency edge — not a workspace-wide rename.

> **Forward note (FFI, not this plan).** The `release-max` profile sets `panic = "abort"`, which is correct for the in-tree binaries but would abort a foreign host (e.g. RetroArch) if a panic crossed the FFI boundary. The future `gb-lib`/`gb-libretro` crates will need `panic = "unwind"` plus `catch_unwind` at every exported boundary. Recorded here only so the constraint is not forgotten; no action in P0–P5.

---

## 7. Non-goals

- **No `gb-base`, `gb-core-2`, `gb-lib`, or `gb-libretro` in this work.** They are unlocked, not built.
- **No change to `gb-core`'s internal architecture or public surface.** `gb-api` is purely additive on top of the existing core; the wide `gb-core` re-exports remain available for consumers that legitimately need internals.
- **`gb-test-runner` is out of scope** and stays on `gb-core` directly.
- **`gb-desktop`'s debugger/inspection/rewind/link/IR features stay on `gb-core` directly** — only the play loop moves to `gb-api`, and even then telemetry that reads internals not on `CycleView` keeps a direct `gb-core` handle.
- **The `gb-cli` `test`/`inspect`/`trace` subcommands stay on `gb-core` directly** — only the `run` path moves.
- **No emulation-accuracy changes.** This is a structural refactor; ROM behavior must be byte-identical (enforced by the P1 parity test on framebuffer/serial/audio and the blargg report).
- **No merge to `main`.** See Section 8.

---

## 8. CI gates and branch / commit policy

**Per-phase CI gate (mandatory, every phase ends green).**

- `cargo fmt-check`
- `cargo lint`
- `cargo tests`
- `cargo rom-report blargg`

P5 additionally enforces the feature-matrix build/test commands listed in its CI gate.

**Broader checks (run during normal verification, not part of the four-command per-phase gate).** `cargo deny check`, full `cargo rom-suite`, and `cargo llvm-cov` per-crate thresholds — `gb-core` 95.80%, `gb-benchmark` 71.50%, `gb-persistence` 96.30%, `gb-cli` 90%, `gb-desktop` 90%, with `gb-api` added to the threshold list once it carries logic in P1. Coverage is **not** asserted in any phase's Definition of Done, because the four-command gate does not run `cargo llvm-cov`; each phase's DoD is verifiable with exactly the commands that phase's gate runs.

**Branch and commit policy.**

- All work accumulates as commits on a single descriptive branch: `modularization/gb-api`.
- **Never propose merging to `main`.** Merging is a separate, explicit decision by the maintainer; this plan only accumulates commits on the branch.
- All changes are agent-agnostic and tooling-neutral.
- Repo docs, code, and commit messages are written in English; commits carry no AI/assistant attribution.
