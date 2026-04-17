# SERIAL-EXT

Roadmap for external serial-port peripherals beyond the current Phase `5`
baseline. This document covers three long-term targets:

- `DMG-04` Game Link Cable
- `DMG-07` 4-Player Adapter
- Game Boy Printer

The goal is to extend the repo's existing serial baseline without collapsing
per-console serial hardware, external-port attachments, and multi-console
session orchestration into one premature abstraction.

## Scope

- `gb-core` ownership of serial-port-visible hardware interactions and
  attachment semantics
- `gb-test-runner` ownership of deterministic validation, multi-machine test
  sessions, and retained artifacts
- `gb-desktop` ownership of external-port UX, player-slot input profiles,
  audio/video presentation, and session controls

## Authority boundaries

- This document owns the **recommended implementation order** for external
  serial peripherals and linked sessions.
- `hardware/SERIAL.md` remains the authority for serial hardware behavior,
  timing, MMIO semantics, and peer-boundary rules.
- `ARCHITECTURE.md` remains the authority for scheduler phases, subsystem
  ownership, and crate boundaries.
- `TESTING.md` remains the authority for validation policy.
- This file does **not** redefine DMG serial hardware truth; it sequences work
  on top of that truth.

## Decisions locked in now

These decisions should be treated as stable defaults unless stronger hardware
evidence or a clearer architectural need overrides them later.

1. **Default external port state is `NONE`.**
   The emulated console starts with nothing connected, matching an ordinary
   standalone Game Boy.
2. **`gb-core` outputs remain neutral.**
   Printer work should produce typed page/raster data, not PNG encoding or
   frontend-specific presentation artifacts.
3. **Player-slot abstraction lives in `gb-desktop`, not `gb-core`.**
   `PlayerSlot::{P1, P2, P3, P4}` is a host/session concern and should remain
   reusable for both linked Game Boy sessions and possible future `SGB` /
   `SGB2` host UX.
4. **Do not let `DMG-07` distort the first phases.**
   Early phases should solve `Printer` and `DMG-04` cleanly before the
   4-player adapter starts constraining interfaces.
5. **Do not force printer and link cable into one generic trait-object design
   too early.**
   Prefer typed, snapshot-friendly state and explicit ownership boundaries over
   maximal extensibility.
6. **Two-machine and four-machine link support require shared timeline
   coordination.**
   The hard problem is not windowing; it is advancing multiple consoles on one
   shared T-cycle timeline with the repo's scheduler phase rules intact.

## Phase 0 glossary

Phase `0` uses the following vocabulary. Later docs and code comments should
preserve these distinctions even if final Rust type names evolve.

| Term | Meaning | Primary owner |
|------|---------|---------------|
| `serial hardware` | The per-console serial controller: `SB`, `SC`, transfer state, clock selection, bit shifting, and completion IRQ timing. | `gb-core::serial` |
| `external port` | The physical handheld connector shared by link cable and printer workflows. | hardware concept |
| `external-port attachment` | What is currently connected to one console's external port: `None`, `Printer`, `GameLinkDmg04`, or `FourPlayerAdapterDmg07`. | `gb-core`, outside local serial shifting logic |
| `serial endpoint` | The narrow per-console boundary that supplies incoming serial bits, external slave clocks, and disconnected/open-line behavior to local serial hardware. | attachment or linked-session layer |
| `linked session` | A shared owner that advances multiple `Machine` instances and any cable/adapter topology on one common T-cycle timeline. | `gb-core`, outside local serial hardware |
| `player slot` | Host-facing player identity such as `P1..P4`, including input, mute, and view policy. | `gb-desktop` |

Two clarifications are intentionally locked in:

- `Printer` and `Game Link` share the same physical external port, but printer
  support is still a single-console attachment problem rather than a
  multi-console linked-session problem.
- A `serial endpoint` is narrower than an attachment or linked session. It is
  the immediate signal boundary seen by the serial controller, not the full
  owner of topology, UI, or multi-machine scheduling.

## Final UX target

The long-term desktop UX target is one `EXT. PORT` menu with:

- `NONE`
- `PRINTER`
- `GAME LINK`
- `4 PLAYER ADAPTER`

Those options do not need to become available at once. Until a mode is fully
implemented, `gb-desktop` may expose it as disabled or as a documented TODO.

## Phase ordering summary

| Phase | Primary owner | Goal |
|------:|---------------|------|
| 0 | `docs/`, `gb-core` | Lock terminology, ownership, and external-port contracts |
| 1 | `gb-core` | Introduce a stable external-port seam without changing current serial behavior |
| 2 | `gb-core` | Implement Game Boy Printer in the core with neutral output data |
| 3 | `gb-core` | Add linked-session infrastructure without desktop UX |
| 4 | `gb-core` | Implement the real `DMG-04` two-console cable in the core |
| 5 | `gb-test-runner` | Add deterministic linked-session validation and artifacts |
| 6 | `gb-desktop` | Add `EXT. PORT` UX and printer-facing desktop workflows |
| 7 | `gb-desktop` | Add local `DMG-04` linked-session UX for two consoles |
| 8 | `gb-desktop` | Generalize player-slot and per-player host input/audio policy |
| 9 | `gb-core`, `gb-test-runner` | Implement and validate `DMG-07` for 2/3/4 consoles |
| 10 | `gb-desktop` | Add `DMG-07` visible local UX |

## Phase 0 — Terminology, ownership, and contracts

### Goal

Define the vocabulary and seams before changing behavior.

### Core outcomes

- Introduce one stable external-port taxonomy for planning and code comments,
  such as:
  - `None`
  - `Printer`
  - `GameLinkDmg04`
  - `FourPlayerAdapterDmg07`
- Document three distinct layers:
  - per-console serial hardware
  - external-port attachment semantics
  - linked multi-console session orchestration
- Record explicitly that `Printer` and `Game Link` share the same physical port
  but are not the same problem architecturally.

### Crates / files

- `docs/SERIAL-EXT.md`
- `docs/hardware/SERIAL.md`
- `docs/ARCHITECTURE.md` if ownership wording changes are needed

### Validation gate

- No behavior changes
- Existing serial tests remain untouched and green

### Done criteria

- The terminology is precise enough that later code does not need to guess
  whether something belongs to `serial`, an attachment, or a linked session.

## Phase 1 — External-port seam in `gb-core`

### Goal

Prepare `gb-core` for real attachments while preserving the current Phase `5`
serial baseline.

### Core outcomes

- Introduce a stable, snapshot-friendly representation of what is attached to
  the port.
- Keep the current disconnected / loopback baseline working without behavioral
  regressions.
- Separate:
  - attachment identity
  - attachment runtime state
  - reset/startup policy for attachment state

### Design guidance

- Do **not** commit early to a generic `Box<dyn Trait>` as the only solution.
- Favor typed state that remains easy to trace, snapshot, compare, and later
  serialize.
- Keep the default state as explicit `None` / disconnected.

### Crates / files

- `crates/gb-core/src/serial.rs`
- possible new child modules under `crates/gb-core/src/serial/`
- `crates/gb-core/src/machine.rs`
- `crates/gb-core/tests/serial.rs`

### Validation gate

- All existing serial unit and integration tests pass unchanged
- Add characterization tests if the refactor touches snapshot or trace shape

### Done criteria

- The repo has a real external-port seam, but current DMG serial behavior is
  still preserved.

## Phase 2 — Game Boy Printer in `gb-core`

### Goal

Implement the printer as the first real external device on the port.

### Core outcomes

- Add printer attachment state and protocol handling in `gb-core`
- Produce neutral printer output data, for example typed page/raster results
- Keep printer behavior opt-in through explicit attachment selection

### Design guidance

- Do not make the printer connected by default
- Treat printer output as core-owned logical data, not frontend-owned images
- Record any intentionally deferred protocol details explicitly, rather than
  leaving them implicit

### Expected protocol scope

The current Phase `2` v1 target is:

- command framing and checksums
- `INIT`, `DATA`, `PRINT`, `STATUS`
- empty `DATA` packet before `PRINT`
- packet timeout handling
- deterministic busy / status progression
- typed printed-page output from `gb-core`

Explicitly deferred from v1:

- compressed packet payloads
- detailed real-time printer-busy timing beyond the deterministic status
  progression
- frontend-specific image export or preview concerns

### Crates / files

- `crates/gb-core/src/external_port/printer.rs`
- `crates/gb-core/tests/printer.rs`
- fixtures under `crates/gb-core/tests/fixtures/`

### Validation gate

- protocol unit tests
- golden output tests from non-commercial synthetic inputs
- optional later ROM-based integration once the typed output path is stable

### Done criteria

- A printer can be attached explicitly, games can speak the protocol, and the
  core can expose deterministic printer output artifacts without any desktop
  UI dependency.

## Phase 3 — Linked-session infrastructure without desktop UX

### Goal

Create the shared multi-machine session layer before implementing real linked
console modes.

### Core outcomes

- Introduce one linked-session owner for multiple `Machine` instances
- Advance all participating consoles on one shared T-cycle timeline
- Preserve the repo's scheduler phase contract across all linked consoles

### Design guidance

- Do not model this as “run machine A for a full T-cycle, then machine B”
- Prefer per-phase coordination over whole-cycle sequencing
- Keep link-session orchestration separate from the local serial hardware model

### Crates / files

- `crates/gb-core/src/link/`
- supporting `Machine` stepping changes in `crates/gb-core/src/machine/`
- `gb-core` tests for shared stepping

### Validation gate

- deterministic stepping tests
- timeline-coherence tests
- no desktop dependency

### Done criteria

- The repo can host multiple `Machine` instances under one deterministic shared
  scheduler timeline, even before a real cable or adapter is attached.

## Phase 4 — `DMG-04` core implementation in `gb-core`

### Goal

Implement the real two-console `DMG-04` Game Link Cable in the core on top of
the linked-session infrastructure from Phase `3`.

### Core outcomes

- Add a passive `DMG-04` cable model
- Support real two-console byte exchange over shared `SCK` timing
- Preserve master/slave semantics from the existing serial hardware baseline

### Design guidance

- The hard requirement is shared timing correctness, not desktop presentation
- Keep cable behavior passive and hardware-shaped; the cable is not a packet
  protocol of its own

### Crates / files

- `gb-core` linked-session and cable modules

### Validation gate

- unit tests for cable routing and disconnected behavior
- integration tests with two `Machine`s exchanging bytes bidirectionally
- retained traces for at least one complete transfer chronology

### Done criteria

- Two linked Game Boys can exchange real serial traffic through `DMG-04` in
  `gb-core`, with deterministic core-owned coverage before the dedicated
  runner-facing harness phase begins.

## Phase 5 — `gb-test-runner` linked-session validation and artifacts

### Goal

Mature the linked-session validation path in `gb-test-runner` before any
desktop-linked UX becomes a primary validation path.

### `gb-test-runner` outcomes

- Add manifest/session support for two linked consoles
- Allow per-console ROMs, input schedules, timeouts, and retained artifacts
- Add combined traces or paired artifacts when link timing matters
- Expose a public linked-session CLI path and a small built-in linked-suite
  registry so the harness is usable outside unit tests
- Keep linked-session execution deterministic and reproducible outside any
  desktop presentation loop

### Design guidance

- `gb-test-runner` should validate linked-session behavior, not reimplement it
- Keep linked-session suite/case types parallel to the existing single-machine
  `RomSuite` / `RomTestCase` contract rather than overloading it
- Keep session manifests explicit enough that later `DMG-07` and printer cases
  can reuse the same harness vocabulary where appropriate

### Crates / files

- `crates/gb-test-runner/`
- `linked_session_manifest.rs`, `linked_session_runner.rs`, and retained
  artifact helpers
- reserved repo-owned linked-session suites under
  `crates/gb-test-runner/data/`, starting with `linked-dmg04-smoke.toml`
  and fixtures under `crates/gb-test-runner/data/fixtures/linked/`

### Validation gate

- manifest-driven linked-session runs
- retained paired artifacts or combined traces for at least one complete
  `DMG-04` exchange flow
- deterministic rerun coverage for the same linked-session case

### Done criteria

- The repo has a first-class non-desktop validation path for linked sessions,
  and later desktop work does not need to invent its own correctness oracle.

### Suggested internal rollout

#### Phase 5.1 — linked-session harness baseline

- Land linked-session manifest types parallel to the single-machine suite
  contract
- Land a `LinkedSessionRunner` over `gb_core::LinkedMachines`
- Check in one repo-owned `DMG-04` smoke suite under
  `crates/gb-test-runner/data/` with deterministic retained fixtures

#### Phase 5.2 — linked-session CLI and built-in suite registry

- Add a dedicated linked-session CLI path
- Add a small built-in linked-suite registry rooted in repo-owned manifests
- Support running one linked suite or one linked session outside unit tests

#### Phase 5.3 — participant-scoped linked oracles

##### Goal

Make linked-session validation expressive enough to describe protocol-level
expectations without relying mostly on whole-session trace or snapshot fixtures.

##### `gb-test-runner` outcomes

- Add participant-scoped linked oracles such as:
  - exact serial text / serial-hex expectations per participant
  - participant snapshot fixtures
  - participant trace fixtures where warranted
- The first useful slice should be participant-scoped exact `serial_hex`
  expectations, because they express `DMG-04` byte exchange contracts compactly
  without whole-session fixtures
- The next useful slice should be participant-scoped snapshot fixtures, so the
  harness can pin one participant's full final machine state without forcing a
  whole-session snapshot fixture
- After that, participant-scoped trace fixtures should cover cases where the
  participant's execution chronology matters more than the final snapshot alone
- Extend linked-session manifests so expectations can target the whole session
  or one named participant explicitly
- Normalize retained artifact naming for session-level vs participant-level
  outputs so failures stay easy to inspect and compare
- Add at least one richer `DMG-04` linked suite beyond the initial smoke case,
  covering behaviors that matter to the cable protocol rather than only the
  final state, starting with stale-byte reuse through compact participant
  `serial_hex` contracts, then unsupported double-master behavior through
  participant snapshot contracts, and then open-line behavior for a
  non-participating far end

##### Design guidance

- Prefer compact, protocol-shaped oracles over large whole-session fixtures
  when they can express the same contract more clearly
- Keep participant-scoped contracts explicit; do not infer target participants
  from list position when an ID can be named
- Preserve session-level fixtures as a useful escape hatch, but stop treating
  them as the only practical correctness oracle
- Keep the design friendly to later `DMG-07` reuse without adding `DMG-07`
  semantics to the `DMG-04` phase

##### Crates / files

- `crates/gb-test-runner/src/linked_session_manifest.rs`
- `crates/gb-test-runner/src/linked_session_runner.rs`
- `crates/gb-test-runner/src/run_linked_session_cli.rs`
- linked-session manifests and fixtures under
  `crates/gb-test-runner/data/`
- contract and binary coverage under `crates/gb-test-runner/tests/`

##### Validation gate

- manifest-driven linked-session cases with participant-scoped expectations
- deterministic rerun coverage for the same participant-scoped oracle
- CLI coverage for linked suites that validate per-participant outcomes
- retained failure artifacts that make it obvious which participant contract
  failed and why

##### Done criteria

- `gb-test-runner` can express and validate linked-session behavior in compact,
  participant-aware contracts
- growing `DMG-04` coverage no longer depends mainly on large whole-session
  fixtures
- the harness is ready to support a broader `DMG-04` suite and, later, more
  complex linked topologies

## Phase 6 — Desktop `EXT. PORT` UX and printer-facing workflows

### Goal

Expose the external-port concept in `gb-desktop` and make printer attachment
usable from the frontend.

### Desktop outcomes

- Add `EXT. PORT` menu entry with:
  - `NONE`
  - `PRINTER`
  - `GAME LINK`
  - `4 PLAYER ADAPTER`
- Allow `NONE` and `PRINTER` to be selectable once those modes exist
- Allow `GAME LINK` and `4 PLAYER ADAPTER` to remain disabled until their
  corresponding phases close
- Add desktop-owned handling for printer output presentation or export

### Design guidance

- Desktop should consume typed printer output from `gb-core`
- Desktop should not invent printer protocol behavior of its own

### Crates / files

- `crates/gb-desktop/src/main.rs`
- `crates/gb-desktop/src/menu.rs`
- desktop settings/config if attachment persistence is added

### Validation gate

- frontend tests for menu state and attachment switching
- printer export/presentation tests around desktop-owned formatting

### Done criteria

- The desktop frontend can switch the external port between disconnected and
  printer modes, and printer output is usable without changing core behavior.

## Phase 7 — Desktop local `DMG-04` session for two consoles

### Goal

Expose a local two-console `DMG-04` session in `gb-desktop`.

### Desktop outcomes

- Run one linked two-console session from one desktop process
- Present two console outputs locally
- Route host input independently to each console
- Default to muting the second console unless the user chooses otherwise

### Design guidance

- Session ownership matters more than final visual layout
- One window with two panels and two separate windows are both acceptable UX
  targets later; the first requirement is one correctly coordinated linked
  session

### Crates / files

- `crates/gb-desktop/src/main.rs`
- session/UI helpers as needed

### Validation gate

- frontend tests for linked-session creation
- tests for per-console input routing
- tests proving desktop is not advancing the linked consoles from multiple
  independent loops

### Done criteria

- `gb-desktop` can run a real local `DMG-04` linked session for two consoles
  on top of the already-validated core and harness infrastructure.

## Phase 8 — Player-slot abstraction and per-player host policy

### Goal

Generalize desktop input/audio/player handling so later multi-console and
future `SGB` / `SGB2` work can reuse the same host-side player model.

### Desktop outcomes

- Add reusable player slots:
  - `P1`
  - `P2`
  - `P3`
  - `P4`
- Bind input profiles by player slot, not by ad hoc linked-session wiring
- Keep audio mute defaults and per-player UI policy in frontend code

### Design guidance

- This abstraction belongs in `gb-desktop`, not `gb-core`
- A player slot is not automatically the same thing as a `Machine`
- Different session types may map slots to visible outputs differently later

### Crates / files

- `crates/gb-desktop/src/input.rs`
- desktop settings/config
- session-layer frontend code

### Validation gate

- frontend tests for binding resolution by player slot
- migration/setting tests if configuration persistence changes

### Done criteria

- Desktop player management is reusable for 2-player link, 4-player adapter,
  and future host-level multiplayer UX without reopening core serial logic.

## Phase 9 — `DMG-07` in `gb-core` and `gb-test-runner`

### Goal

Implement the real `DMG-07` 4-Player Adapter only after the printer and
two-console `DMG-04` stack are stable.

### Core outcomes

- Add explicit `DMG-07` attachment/session behavior
- Support 2-, 3-, and 4-console participation
- Keep `DMG-07` distinct from the passive `DMG-04` cable model

### `gb-test-runner` outcomes

- Extend linked manifests to 2/3/4 linked participants
- Add deterministic adapter-focused tests and retained traces

### Design guidance

- The adapter is active hardware and should remain architecturally separate
  from a plain cable
- Sparse occupancy must be treated as a first-class case, not a corner case

### Crates / files

- `gb-core` adapter/session modules
- `gb-test-runner` multi-participant manifests and execution path

### Validation gate

- unit tests for adapter-specific behavior
- integration tests for 2/3/4 attached consoles
- retained traces for adapter-state transitions where timing matters

### Done criteria

- `gb-core` and `gb-test-runner` can deterministically run real `DMG-07`
  sessions before desktop UX for the adapter lands.

## Phase 10 — Desktop local `DMG-07` UX

### Goal

Expose `DMG-07` sessions in the desktop frontend after the core and harness
path are already validated.

### Desktop outcomes

- Enable `4 PLAYER ADAPTER` in `EXT. PORT`
- Run 2-, 3-, or 4-console local sessions
- Present video/audio/input per player slot
- Default to leaving only `P1` unmuted unless the user changes it

### Design guidance

- Keep UX policy in the frontend
- Keep `gb-core` unaware of desktop muting, window layout, or host device
  assignment

### Crates / files

- `crates/gb-desktop/src/main.rs`
- session/UI/input/audio helpers

### Validation gate

- frontend tests for session composition by slot count
- per-player routing and mute-default tests
- regression tests confirming that desktop does not bypass linked-session core
  ownership

### Done criteria

- `gb-desktop` can run real local `DMG-07` sessions with 2 to 4 players on top
  of the already-validated core and harness behavior.

## Cross-phase rules

- Do not make `Printer`, `DMG-04`, or `DMG-07` connected by default.
- Do not treat multi-console desktop presentation as proof that the underlying
  linked timing model is correct; core and harness validation come first.
- Do not push frontend concerns such as PNG encoding, mute defaults, or player
  binding policy into `gb-core`.
- Do not use `DMG-07` as the justification for speculative complexity in the
  first printer or `DMG-04` phases.
- Every phase that changes behavior should leave behind targeted automated
  tests and update the matching docs in the same change.
