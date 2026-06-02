# Testing

## Scope

This document owns project-wide validation policy, cross-subsystem testing expectations, closure criteria, and CI stratification. Detailed hardware checklists live in the matching `docs/hardware/*.md` handbook; external ROM fetching/running/reporting details live in [`docs/info/ROM-SUITES.md`](info/ROM-SUITES.md); phase context lives in [`docs/ROADMAP.md`](ROADMAP.md) and `docs/roadmap/*.md`.

Keep this file policy-focused. If a suite list, command catalogue, manifest example, or report layout needs operational detail, put it in [`docs/info/ROM-SUITES.md`](info/ROM-SUITES.md) and link to it instead of duplicating it here.

## Validation layers

Use several independent layers because no single signal proves hardware accuracy:

| Layer | Purpose |
| --- | --- |
| Unit tests | Local invariants, small state machines, bit/register behavior, mapper helpers, and pure policy decisions. |
| Integration tests | Timing-coupled behavior across CPU, bus, DMA, PPU, timer, interrupts, APU, joypad, serial, boot, cartridge, save-state, and machine boundaries. |
| ROM-based validation | Redistributable synthetic and third-party ROMs with machine-readable pass/fail channels, framebuffer oracles, serial/text output, snapshots, or retained traces. |
| Differential oracles | Manual/external comparison against trusted emulators or hardware research when timing or ordering is ambiguous. |
| Determinism and retention | Replay, save/load continuation, rewind restore paths, and permanent regression assets for fixed bugs. |

Every meaningful subsystem change should leave behind at least one durable asset in this matrix. Unit tests do not replace ROM suites, ROM suites do not replace differential study, and differential study does not replace deterministic replay or regression retention.

## Default expectations for changes

- New production code should normally add or update automated unit or integration coverage in the same change.
- Use local module tests for local invariants; use top-level `tests/` only when behavior crosses public APIs or subsystem boundaries.
- Move large inline test blocks into co-located test modules such as `foo/tests.rs` rather than unrelated catch-all files.
- Add ROM-based validation when the behavior is only meaningful through a ROM, boot path, mapper interaction, framebuffer oracle, serial output, or linked-session contract.
- If immediate automated coverage is impractical, state the reason, remaining risk, and intended follow-up in the change report; add a [`docs/TODO.md`](TODO.md) entry when the gap is concrete and non-trivial.
- For timing-sensitive refactors, characterize the current behavior first when possible, then change the implementation.
- Instrumentation, traces, debug snapshots, and failure-artifact capture must not alter hardware-visible ordering or mutate the core timeline.

## Execution modes and evidence

`Strict` is the only mode that counts for CI oracle evidence, differential comparison, DMG closure, CGB promotion, and official accuracy claims. `Permissive` may cover tolerant loader behavior for odd but unambiguous cartridges, but it must not change runtime semantics for admitted supported hardware. `Experimental` is for research, partial hardware paths, or heuristic bring-up and must stay segregated from closure metrics.

Mode-sensitive tests should assert both the admission decision and the warning/error metadata. Curated manifests may opt individual cases into non-`Strict` only when the case itself requires that policy; do not use suite-wide silent overrides.

Save states, replay logs, retained artifacts, and official reports must record the execution mode and active overrides that produced them. Restoring or replaying under a different execution mode should fail by default unless an explicit developer conversion workflow exists and is tested separately as non-oracle behavior.

## External ROM policy

External ROM execution must be automatable. Manual LCD inspection is useful for debugging, but acceptance should prefer typed oracles such as serial output, cartridge-RAM status, text-console extraction, register-signature breakpoints, framebuffer fixtures, snapshots, or short traces.

When working on already-known external ROM failures, timing regressions, exploratory PPU/MMIO fixes, or any ROM-driven iteration whose output may influence a go/no-go decision, preserve a baseline copy of the matching report before the run and a final copy after the run, then compare them explicitly. Use `/test/gb-emulator-shootout/` for promoted GB Emulator Shootout suites, `/test/docboy/` for large DocBoy single-machine suites, `/test/gbmicrotest/` for the gbmicrotest report suite, and standalone exploratory report directories such as `/test/mooneye/`, `/test/ax6/`, `/test/little-things-gb/`, `/test/magen/`, `/test/mealybug-tearoom-tests/`, or `/test/samesuite/`. Name changed rows before summarizing the result as improved, unchanged, or regressed.

If the working tree is not a clean baseline, compare against a clean reference such as `main` in a separate branch or worktree. Do not infer "no regressions" from memory.

`gb-test-runner` owns typed ROM-case and suite metadata: console model, startup mode, execution mode, timeout, pass/fail rule, external stimulus, requested captures, report/catalog classification, and retained failure artifacts. Repo-managed redistributable ROMs live in the gitignored `/test/` store materialized from manifests and source hashes; promoted GB Emulator Shootout rows live below `/test/gb-emulator-shootout/`, DocBoy single-machine rows live below `/test/docboy/`, gbmicrotest rows live directly below `/test/gbmicrotest/<rom>`, and standalone exploratory rows live below their report store roots such as `/test/mooneye/`, `/test/ax6/`, `/test/little-things-gb/`, `/test/magen/`, `/test/mealybug-tearoom-tests/`, or `/test/samesuite/`. Raw upstream checkouts should be temporary. Private commercial ROMs must stay outside the repository, outside CI, and outside official closure claims.

Detailed suite names, fetch commands, report ordering, RealBoot overrides, commercial local manifests, and environment variables live in [`docs/info/ROM-SUITES.md`](info/ROM-SUITES.md).

## Differential oracle policy

Use real hardware documentation and subsystem handbooks first. Use trusted implementation or hardware-research cross-checks only when they provide a comparable observable for the behavior under investigation.

Differential work should preserve the first useful divergence point, execution mode, active overrides, ROM/input/time source, and any snapshot or trace window needed to rerun the comparison. A final hash or framebuffer mismatch is less useful than a localized divergence.

## Determinism, save-state, and rewind policy

Core execution should be deterministic for the same ROM, model configuration, startup mode, execution mode, explicit overrides, input stream, external stimulus, and injected time source. RTC, RealBoot handoff, and ROM-suite clocks must use injected or otherwise explicit time sources rather than host wall-clock timing.

Automated determinism and save/load continuation coverage lives in cargo tests, including `gb-test-runner` determinism tests; it is not exposed through a manual `run_determinism` CLI. Those tests should compare independent replays, final `MachineSaveState` payloads, serial output, mid-run save/restore continuation, and rejection of incompatible restore metadata.

Cartridge persistence tests validate cartridge-owned backing stores, EEPROM payloads, flash/RTC state, and external-save conversion. Whole-machine save-state tests validate subsystem-owned snapshots, hidden temporal state, metadata compatibility, and continuation determinism. Rewind tests validate repeated in-memory save-state capture/restore through the same core restore path; frontend tests may cover host cleanup, HUD, settings, and buffer policy, but must not redefine core restore semantics.

## Closure and severity policy

Classify failures by closure impact instead of treating all red tests equally. Scheduler ordering, CPU/interrupts, timer, PPU timing, DMA, primary cartridge families, basic joypad/serial behavior, and save/load determinism are must-pass areas for DMG-family closure. Finer APU behavior, advanced serial/link edges, RTC long-tail behavior, and special-cartridge heuristics become must-pass only when promoted by roadmap or by a blocking correctness bug.

Phase `9` accepted practical DMG closure with repo gates for CPU, interrupts, timer, bus, DMA, APU, PPU, primary cartridges, joypad, and serial, plus automated determinism/save-load coverage. CGB suites and CGB-only mapper/audio/RTC lanes are promoted independently and must not be counted as DMG closure evidence unless explicitly framed as compatibility confidence.

Do not declare a closure area healthy while its accepted strict-mode gate is failing, while a severe correctness regression is open, or while a bug fix lacks any permanent regression asset.

## CI and coverage policy

`make ci` is the fast local pre-push gate and should remain independent of external ROM fetching. It covers formatting, linting, workspace tests, typos, dependency policy, and the configured coverage gate. Run narrower tests first during development, then `make ci` before publishing changes; run external ROM targets when the change affects ROM-suite behavior, timing-sensitive hardware, workflow-managed suites, or release confidence.

Coverage thresholds are enforced per repo-gated crate, not as one aggregate workspace percentage. The concrete `cargo llvm-cov --fail-under-*` values in `.cargo/config.toml` are authoritative and must not be lowered. New workspace crates should join the per-crate coverage gate in the same change that introduces them.

GitHub workflows are stratified: `ci` runs Rust checks plus coverage; `test-roms` runs the promoted strict ROM subset; `test-roms-extra` runs standalone exploratory report lanes; `test-roms-gbmicrotest` runs gbmicrotest in its own report channel. RealBoot, commercial, red, linked, or local-only lanes must publish separately and must not dilute the strict closure signal.

Failure artifacts should make reruns unnecessary for first diagnosis: include logs, status rows, serial/text output, framebuffer output when relevant, snapshots, trace windows, and diffs against reference outputs when available.

## Fixture and artifact ownership

- Keep subsystem-local tests near production code under `crates/gb-core/src/**`.
- Keep public API and cross-module smoke coverage under `crates/gb-core/tests/*.rs`, with helpers under `crates/gb-core/tests/common/`.
- Keep core-owned synthetic ROM fixtures and golden traces under `crates/gb-core/tests/fixtures/roms/` and `crates/gb-core/tests/fixtures/traces/`.
- Keep runner-owned manifests, external-suite fixtures, linked-session fixtures, and committed oracle artifacts under `crates/gb-test-runner/data/**`.
- Keep runnable redistributable external ROMs in the gitignored `/test/` store; report-scoped stores such as `/test/gb-emulator-shootout/`, `/test/docboy/`, and `/test/gbmicrotest/` must stay below that global root and long-lived validation assets must not be scattered under `/tmp`.
- Keep commercial ROMs in developer-owned private locations only; local manifests may reference them with manifest-relative or absolute paths, but repo docs and CI must not standardize private paths.

## Linked-session policy

Validate linked-session behavior through `gb-test-runner` manifests and linked fixtures rather than desktop presentation loops. Manifests should name participants and topology explicitly, including `DMG-04` cable sessions, `DMG-07` adapter-port assignment, and CGB IR optical-pair sessions.

Prefer participant-scoped oracles such as per-participant serial output, snapshots, or trace fixtures when they express the contract more clearly than a large whole-session fixture. Desktop tests may cover topology construction, player-slot UX, input routing, audio/view policy, and menus, but they must not redefine serial, cable, adapter, IR, or shared-T-cycle hardware rules.

## Hardware-area focus

Keep detailed edge-case lists in the owning hardware handbook and use this section only as routing guidance.

- CPU, interrupts, timer, DMA, bus, and scheduler tests should name the ordering contract under test and retain enough trace or snapshot context to isolate instruction-level or T-cycle-level divergence.
- PPU and LCD tests should distinguish blocking framebuffer oracles from informational captures and document whether each fixture is an upstream reference, committed project oracle, or exploratory artifact; any tolerance-based image comparison must be an explicitly named oracle rather than a hidden weakening of a strict framebuffer fixture.
- APU tests should separate core timing/mixer semantics from frontend audio delivery; external Blargg `dmg_sound` evidence complements, but does not replace, channel-local and mixer/HPF unit coverage.
- Boot/startup tests should distinguish `RealBoot`, `SkipBoot`, and `CustomBoot`, including `FF50` handoff, boot overlay visibility, model-specific cartridge-entry state, and hidden-state continuity for timer, PPU, and APU.
- Cartridge and mapper tests should cover header parsing, support-category diagnostics, bank behavior, persistence capability, and special-cartridge identification; MBC6 remains automated through cargo tests and runner execution tests rather than a manual Makefile or built-in suite target.
- Joypad, serial, link, and adapter tests should enter through typed core/runner seams instead of frontend host loops.
- CGB-only ROMs and model-specific CGB behavior must stay out of DMG closure metrics until explicitly promoted as CGB evidence.
