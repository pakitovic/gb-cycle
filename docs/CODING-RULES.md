# Coding Rules

## Rust style goals

- Prefer clarity and explicitness.
- Keep modules cohesive and small.
- Avoid over-engineering.
- Make invariants visible in types where practical.

## Recommended practices

- Use descriptive names tied to hardware meaning.
- Name identifiers after the hardware fact they represent, not the internal compensation path: avoid encoding phase numbers, table-lookup strategy, or refetch tactics into names (prefer a short hardware-meaningful name over e.g. `startup_visible_tile2_phase6_refetch_prefers_tilemap_row`). A long name that describes a seam is a smell that the seam should not exist.
- Prefer enums and small structs over magic numbers.
- Keep `pub` visibility narrow.
- Separate state mutation from derived calculations when possible.
- Keep debug logging and tracing easy to enable in development.
- Prefer pure APIs when reasonable, especially around decode, derived state, and reusable helpers.
- Make facts, inferences, and design choices easy to distinguish in code comments and docs.
- Keep code and code comments in English.
- Use explicit integer types when register width, wrapping, masking, or overflow semantics matter.
- Use named constants for registers, bit masks, timing windows, and memory ranges.
- When adding new production code, add unit tests or integration tests with it unless a documented limitation blocks that in the same change.

## Current repo conventions

- Keep top-level subsystem files as facades for module declarations, re-exports, and narrow orchestration; move growing hardware logic into focused child modules such as `bus/*`, `ppu/*`, `apu/*`, `cartridge/*`, `machine/*`, `external_port/*`, or `link/*`.
- When a module is split as `foo.rs` plus `foo/*.rs`, keep `foo.rs` as a facade; when tests are split as `foo/test.rs` plus `foo/test/*.rs`, keep `foo/test.rs` as a facade too, and put shared test fixtures/helpers in `foo/test/common.rs` or another focused child module instead of the facade; if `foo/test/common.rs` grows too much, split it as `foo/test/common.rs` plus `foo/test/common/*.rs` with the same facade rule.
- Prefer typed boundary objects over loosely related parameters or booleans: `Policy`, `Config`, `Snapshot`, `Status`, `StartupState`, `Payload`, and `Report` types should make ownership and intent explicit.
- Use `pub(crate)` / `pub(super)` for cross-child collaboration inside a subsystem; reserve public exports for APIs intentionally re-exported from the crate facade or consumed by another crate.
- Keep constructors and reset/startup helpers explicit when hardware state is not a generic Rust default; only derive or implement `Default` when that value is an intentional repo policy, not just convenient zeroing.
- Prefer small query/accessor methods over exposing raw fields when a caller wants a semantic fact such as capability, current mode, selected target, or persistence profile.
- Keep snapshot/debug DTOs observational. They may expose internal state for tests and diagnostics, but normal runtime behavior should not depend on reconstructing hidden state from a debug snapshot.
- Keep compatibility and validation decisions centralized in typed policy paths instead of scattering ad hoc `strict` booleans, filename heuristics, or frontend-side mapper/model guesses.
- Preserve exact hardware terminology in names where it improves reviewability (`T-cycle`, `dot`, `LY`, `STAT`, `JOYP`, `DIV-APU`, `MBC3`, `DMG-07`), and use helper names that say whether they operate on raw register state or derived effective state.

## Architecture discipline

- Separate core, platform/frontend, persistence, debugger/tooling, and tests.
- Avoid coupling CPU, PPU, APU, timer, or bus code to UI and I/O details.
- Avoid circular dependencies between subsystems.
- Avoid implicit temporal logic hidden inside convenience APIs.
- Every hardware quirk should have an explicit home in code.
- Default to private items and widen visibility only when the subsystem boundary requires it.
- Keep `gb-core` free of platform APIs, file dialogs, SDL/web/audio-device code, filesystem save mechanics, and host wall-clock policy except through explicit injected boundaries.
- Keep `gb-persistence` responsible for cartridge-save storage policy (`.sav/.saN` primary when lossless, `.gbsav/.gbsaN` envelope fallback otherwise), safe file replacement, elapsed-time integration, and external `.sav` conversion; do not move those storage mechanics into mapper runtime code.
- Keep hardware-style cartridge persistence, whole-machine save states, debugger snapshots, replay metadata, and frontend settings as separate systems with separate DTOs and validation rules.
- Keep `machine/` as composition and lifecycle orchestration, not as a dumping ground for subsystem behavior that belongs in CPU, bus, PPU, APU, DMA, timer, serial, external port, link, boot, interrupts, joypad, or cartridge owners.

## Timing-sensitive code

- Avoid clever shortcuts that hide ordering.
- Prefer code that mirrors hardware phases when accuracy matters.
- Comment non-obvious hardware quirks with source references.
- Do not replace fine-grained timing with "every N instructions" models unless the subsystem documentation explicitly justifies it.
- Make wrapping, overflow, and edge-triggered intent explicit in code.
- Use `TCycle`, dot, scheduler-phase, or subsystem-local timing vocabulary explicitly instead of smuggling timing through host frames, wall-clock sleeps, callback cadence, or instruction-count summaries.
- Model long-running work as explicit in-flight state with visible lifecycle, ownership, and completion rules; avoid one-shot helpers that silently perform an entire DMA, serial byte, audio tick block, capture, or scanline side effect at once.
- Keep MMIO side effects owned by the register's subsystem and make same-cycle versus delayed visibility explicit.
- For timing-sensitive comparisons or regression hunts, prefer retained traces/snapshots/artifacts that identify the first divergent hardware-visible event over broad final-state assertions alone.

## Hardware-fidelity discipline (no compensation seams)

Learned from the PPU Mode 3 cleanup (see the appendix in `docs/roadmap/04-ppu-fix.md`): compensation tables breed compensation tables. These rules are binding for any timing-sensitive subsystem (PPU, CPU, timer, DMA, interrupts, bus).

- **Never encode timing as a per-`sprite_x` / per-`SCX` / per-pixel "observed phase" correction array.** Model a mid-line / timing-sensitive register effect by reading the register live at the actual hardware step (e.g. inside the fetcher sub-step that consumes it) plus a single uniform write-observation latency keyed by console. A per-index table is curve fit — it encodes the *symptom* of a step that lands on the wrong dot, not the hardware. If a behavior seems to need such a table, the underlying fetch/timing step is on the wrong dot: fix the dot, do not tabulate the offset.
- **Collapse, never accrete.** A new mechanism must REPLACE the one it supersedes in the same change, not sit beside it; the net count of compensation paths must go DOWN. A change that passes a test while leaving the old seam in place (green-with-seams) is a failure, not a fix — revert it.
- **A console gate must reflect real silicon, never a fitted constant.** When you branch on `is_cgb_family()` (or model/revision), it must encode a documented hardware difference (e.g. DMG re-reads SCY per byte, CGB latches once and defers writes by a fixed observation latency). A magic per-console offset whose only justification is "it makes test ROM X pass" is forbidden — it is a curve-fit seam in a phase-model costume.
- **Make timing emergent from the state machine; cross-check dot-by-dot before compensating.** Prefer landing the FSM step on the hardware dot (validated against the oracles: SameBoy read-only, DocBoy modifiable and same dot-origin) over adding a layer that corrects a known-early/late step. Reach for a compensation only after the dot-by-dot diff proves the FSM itself cannot express the behavior, and document why in the owning `docs/*` file.

## Error handling

- Use explicit error types at boundaries.
- Avoid panics in normal emulator runtime paths.
- Reserve assertions for invariant violations in development and tests.
- Error messages crossing crate, CLI, frontend, persistence, or test-runner boundaries should include the relevant typed context such as model, startup mode, execution mode, cartridge classification, path, or suite/case id.
- Do not hide unsupported hardware, unsupported file formats, incompatible save-state versions, or unsafe external `.sav` mappings behind best-effort fallbacks; fail explicitly with actionable diagnostics.

## Performance

- Correctness first.
- Benchmark before optimizing.
- Optimize with local reasoning and measurable effect.

Only introduce an optimization when all of this is true:

1. A correct and testable implementation exists first.
2. The optimization has a measurable hypothesis.
3. It preserves the clarity of the hardware model.
4. Its cost in maintainability and portability is understood.

Usually welcome:

- dispatch tables when they simplify CPU hot paths
- cache-friendly layouts in measured bottlenecks
- branch reduction in known hot paths
- optional instrumentation or tracing modes

Avoid without evidence:

- merging unrelated hardware modules
- obscuring real clocking behind broad abstractions
- `unsafe` without a strong reason
- parallelism that harms determinism
- caching derived state without a clear invalidation owner
- moving host-side batching or background work across the core boundary when that would make hardware-visible timing harder to observe

## Refactor policy

- Structural refactors should be behavior-neutral.
- Do not mix layout churn with timing fixes unless the change cannot be separated cleanly.
- For CPU, PPU, APU, timer, interrupt, DMA, and bus paths, keep or add characterization tests before reshaping the code.
- For new logic in those paths, prefer shipping the behavior together with direct automated coverage rather than relying only on manual validation or later follow-up tests.
- When moving files or splitting modules, preserve public API names and diagnostics unless the behavior change is the actual goal and is documented separately.
- When a refactor crosses docs authority boundaries, update the owning `docs/*` file in the same change instead of leaving architecture, timing, testing, or hardware rules to be inferred from code.
