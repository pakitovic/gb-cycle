# Testing

## Scope

This document owns project-wide validation policy, evidence expectations, closure criteria, and CI stratification. Put suite inventories, command catalogues, manifest details, and report layouts in [`docs/info/ROM-SUITES.md`](info/ROM-SUITES.md); put hardware edge cases in the matching `docs/hardware/*.md` handbook.

## Validation layers

Use several independent layers because no single signal proves hardware accuracy:

| Layer | Purpose |
| --- | --- |
| Unit tests | Local invariants, small state machines, bit/register behavior, mapper helpers, and pure policy decisions. |
| Integration tests | Timing-coupled behavior across CPU, bus, DMA, PPU, timer, interrupts, APU, joypad, serial, boot, cartridge, save-state, and machine boundaries. |
| ROM-based validation | Redistributable synthetic and third-party ROMs with serial, memory, framebuffer, snapshot, trace, or linked-session oracles. |
| Differential oracles | Comparison against hardware research, real hardware captures, or trusted implementations when ordering is ambiguous. |
| Determinism and retention | Replay, save/load continuation, rewind restore paths, and retained regression assets. |

Every meaningful subsystem change should leave durable evidence in the right layer. Unit tests do not replace ROM suites, ROM suites do not replace differential study, and differential study does not replace deterministic replay or regression retention.

## Default expectations for changes

- New production code should normally add or update automated unit, integration, or ROM-suite coverage in the same change.
- Use local module tests for local invariants; use top-level `tests/` only for public APIs or subsystem boundaries.
- Add ROM-based validation when behavior only becomes meaningful through a ROM, boot path, mapper interaction, framebuffer, serial output, or linked-session contract.
- For timing-sensitive refactors, characterize current behavior first when practical.
- If automated coverage is deferred, state the risk and follow-up; add a [`docs/TODO.md`](TODO.md) entry when the gap is concrete.
- Instrumentation, traces, debug snapshots, and failure artifacts must not alter hardware-visible ordering.

## Execution modes and evidence

`Strict` is the only mode that counts for CI oracle evidence, differential comparison, closure claims, and official accuracy status. `Permissive` may admit odd but unambiguous cartridges without changing runtime semantics. `Experimental` is for research or partial hardware paths and must stay out of closure metrics.

Mode-sensitive tests should assert the admission decision and warning/error metadata. Save states, replay logs, retained artifacts, and reports must record the execution mode and active overrides that produced them.

## External ROM policy

External ROM evidence must be automatable. Manual LCD inspection is useful for debugging, but acceptance should rely on typed oracles: serial output, memory status, register signatures, framebuffer fixtures, snapshots, or traces.

`gb-test-runner` owns report-local `*.suite.toml` and `*.link.suite.toml` metadata. Redistributable runnable assets are materialized under the gitignored `/test/<report>/` store from report sources and hashes; committed oracle fixtures live under `crates/gb-test-runner/data/**`. Raw upstream checkouts should be temporary.

When a ROM-driven change affects a go/no-go decision, compare before/after report status from a clean baseline and name changed rows as improved, unchanged, or regressed. Do not infer “no regressions” from memory.

Private commercial ROMs must stay outside the repository, outside CI, and outside official closure claims. Detailed report lists, commands, RealBoot usage, and local-only workflows live in [`docs/info/ROM-SUITES.md`](info/ROM-SUITES.md).

## Differential oracle policy

Use hardware documentation and subsystem handbooks first. Use trusted implementations or hardware captures only when they provide a comparable observable.

Differential work should preserve the first useful divergence point, execution mode, ROM/input/time source, and enough snapshot or trace context to rerun the comparison. A localized divergence is more useful than a final hash or framebuffer mismatch.

## Determinism, save-state, and rewind policy

Core execution should be deterministic for the same ROM, model configuration, startup mode, execution mode, input stream, external stimulus, and injected time source. RTC, RealBoot handoff, and ROM-suite clocks must not depend on host wall-clock timing.

Determinism, save/load continuation, cartridge persistence, and rewind coverage should live close to the core and runner code. Tests should compare independent replays, saved state payloads, continuation after restore, serial output, and rejection of incompatible restore metadata.

## Closure and severity policy

Classify failures by closure impact. Scheduler ordering, CPU/interrupts, timer, PPU timing, DMA, primary cartridge families, basic joypad/serial behavior, and save/load determinism are must-pass areas for DMG-family closure. APU detail, advanced serial/link edges, RTC long-tail behavior, and special-cartridge heuristics become must-pass when promoted by roadmap or by a blocking correctness bug.

Do not declare a closure area healthy while its strict gate is failing, while a severe correctness regression is open, or while a bug fix lacks a permanent regression asset. CGB suites and CGB-only mapper/audio/RTC lanes are promoted independently and must not be counted as DMG closure evidence unless explicitly framed as compatibility confidence.

## CI and coverage policy

`cargo fmt-check`, `cargo lint`, `typos`, and `cargo deny-check` are the local pre-commit checks. `make coverage` runs workspace coverage, enforces per-crate gates, and emits the HTML report; run ROM suites separately when a change affects ROM-suite behavior, timing-sensitive hardware, workflow-managed suites, or release confidence.

Coverage thresholds are enforced per repo-gated crate. The concrete `.cargo/config.toml` coverage aliases are authoritative and must not be lowered without an explicit rationale.

GitHub workflows are stratified: `ci` runs Rust checks plus coverage; `test-roms` runs promoted strict ROM reports; `test-roms-extra` runs standalone exploratory report lanes. RealBoot, commercial, red, linked, and local-only lanes must publish separately and must not dilute the strict closure signal.

Failure artifacts should make first diagnosis possible without rerunning: include logs, status rows, serial/text output, framebuffer output, snapshots, traces, and diffs when relevant.

## Fixture and artifact ownership

- Keep subsystem-local tests near production code under `crates/gb-core/src/**`.
- Keep public API and cross-module smoke coverage under `crates/gb-core/tests/*.rs`, with helpers under `crates/gb-core/tests/common/`.
- Keep core-owned synthetic ROM fixtures and golden traces under `crates/gb-core/tests/fixtures/`.
- Keep runner-owned manifests, external-suite fixtures, linked-session fixtures, and committed oracle artifacts under `crates/gb-test-runner/data/**`.
- Keep runnable redistributable external ROMs in the gitignored `/test/<report>/` stores.
- Keep commercial ROMs in developer-owned private locations only.

## Linked-session policy

Validate linked-session behavior through `gb-test-runner` manifests and linked fixtures rather than desktop presentation loops. Manifests should name participants and topology explicitly, including `DMG-04`, `DMG-07`, and CGB IR sessions.

Prefer participant-scoped serial, snapshot, framebuffer, or trace oracles when they express the contract more clearly than a whole-session fixture. Desktop tests may cover UX and host integration, but they must not redefine serial, cable, adapter, IR, or shared-T-cycle hardware rules.

## Hardware-area focus

Keep detailed edge-case lists in the owning hardware handbook and use this section only as routing guidance.

- CPU, interrupts, timer, DMA, bus, and scheduler tests should name the ordering contract and retain enough trace or snapshot context to isolate T-cycle-level divergence.
- PPU and LCD tests should distinguish blocking framebuffer oracles from informational captures; tolerance-based image comparison must be an explicitly named oracle.
- APU tests should separate core timing/mixer semantics from frontend audio delivery.
- Boot/startup tests should distinguish `RealBoot`, `SkipBoot`, and `CustomBoot`, including `FF50` handoff and model-specific cartridge-entry state.
- Cartridge and mapper tests should cover header parsing, diagnostics, bank behavior, persistence, and special-cartridge identification.
- Joypad, serial, link, and adapter tests should enter through typed core/runner seams instead of frontend host loops.
- CGB-only ROMs and model-specific CGB behavior must stay out of DMG closure metrics until explicitly promoted as CGB evidence.
