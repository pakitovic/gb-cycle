# Phase 9 — Final DMG hardening, differential validation, and closure

This phase records the completed practical DMG closure work. The project closes Phase `9` through layered evidence on the shared T-cycle model rather than through informal game compatibility, using the dedicated save-state and serialization infrastructure from Phase `8` as part of the accepted closure signal.

Closure status (`2026-04-28`): Phase `9.5` is closed for the practical DMG hardening scope. The accepted closure evidence is:

- one explicit subsystem closure checklist in [`docs/TESTING.md`](../TESTING.md) that records the accepted repo-gated and internal evidence for the landed DMG subsystems
- one `gb-test-runner` catalog path, `cargo run -p gb-test-runner --bin run_rom_suite -- --list-detailed`, that exposes the built-in suite set together with oracle channel, capture, and retained-artifact policy
- one `gb-test-runner` checklist path, `cargo run -p gb-test-runner --bin run_rom_suite -- --early-checklist`, that exposes the accepted Phase `9` closure evidence per subsystem
- one repo-gated PPU framebuffer-oracle suite, `cargo run -p gb-test-runner --bin run_rom_suite -- --suite acid`, sourced from `GBEmulatorShootout` and now part of the supported external DMG block used by `make test-roms` and the GitHub `test-roms` workflow
- one workflow-managed PPU framebuffer-oracle suite, `cargo run -p gb-test-runner --bin run_rom_suite -- --suite mealybug-tearoom-tests [--failure-artifact-root <dir>]`, which uses a curated DMG subset from `GBEmulatorShootout` and the same committed-PNG oracle contract as `dmg-acid2`
- one workflow-managed DMG acceptance lane, with the chunk suites `mooneye-acceptance-manual`, `mooneye-emulator-mbc1-mbc5`, and `mooneye-emulator-mbc2`, which follow the active `GBEmulatorShootout` `testroms/mooneye.py` acceptance list, use the upstream `mooneye` breakpoint/register result protocol instead of framebuffer fixtures, and provide broad hardening evidence for the accepted Phase `9` closure matrix
- one accepted automated determinism test lane in `gb-test-runner`, which performs deterministic replay, in-memory save/load continuation, and mismatched-metadata restore rejection checks for strict built-in ROM cases

Historical local validation on April 27 and April 28, 2026 used repo-local LibSameBoy materialization, differential, first-divergence, and manual determinism helpers to close the original Phase `9.2`, `9.3`, and `9.4` evidence. Those helpers and the `phase9-*` Make targets are now retired: SameBoy oracle generation/comparison lives in a separate local SameBoy-oriented repository and is run manually when investigation needs it, while this repo keeps the accepted closure checklist, ROM suites, and automated determinism tests.

#### Goal

Close the practical DMG core with a formal validation matrix, differential and determinism tooling, and explicit closure criteria based on completed evidence.

#### Modules involved

- `tests/`
- `gb-test-runner/`
- `debugger/`
- `scheduler/`
- subsystem cores as needed for per-area traces and inspections
- frontend or tooling adapters only where they are needed for capture, visualization, or artifact export

#### Deliverables

- formal DMG hardening matrix with layers `A/B/C/D/E`, severity classes, and explicit `must-pass` areas
- automated external-ROM harness with timeout, pass/fail policy, framebuffer and serial capture, and retained failure artifacts
- historical SameBoy cross-check evidence, now maintained outside repo-local automation
- deterministic replay and save/load determinism through automated cargo-test coverage
- minimum closure-ready debugging tooling: traces, breakpoints, watchpoints, snapshots, and targeted subsystem viewers
- explicit DMG closure checklist covering internal suites, external suites, manual external cross-check context, determinism, save/load determinism, and primary cartridge families

#### Completed sequencing inside Phase 9

1. Formalize the DMG hardening matrix and closure severity policy.
   Scope: define layers `A/B/C/D/E`, `must-pass` versus non-blocking categories, minimum DMG closure suites, and the rule that no single layer substitutes for another.
   Acceptance criteria: the project docs name the closure layers explicitly, identify the blocking hardware areas for DMG closure, and define a stable checklist instead of relying on informal compatibility claims.
2. Build the external ROM harness and minimum closure suites.
   Scope: automate CPU / interrupt ROMs, `dmg-acid2`, `mealybug-tearoom-tests`, Mooneye acceptance, Blargg DMG, and the Phase `6` cartridge oracle; support framebuffer and serial capture; define timeouts, pass/fail rules, and retained artifacts.
   Acceptance criteria: the minimum DMG closure ROM suites run without manual screen inspection, every case has a timeout plus explicit pass/fail policy, and the harness can preserve enough output to debug failures offline.
3. Add differential comparison against SameBoy.
   Scope: end-of-test comparison for the accepted SameBoy lanes plus first-divergence localization with archived probe context.
   Acceptance criteria: SameBoy acted as the DMG oracle for the covered scenarios during Phase `9` closure, accepted oracle limitations were documented, and the active long-term workflow moved to a separate local SameBoy-oriented repository instead of remaining as gb-cycle Make targets or binaries.
4. Close the minimum debugging and inspection tooling.
   Scope: instruction / micro-op / short-window T-cycle tracing, breakpoints and watchpoints on `PC`, memory, MMIO, and cartridge-bank state, plus fast inspection of CPU, scheduler, bus owner, PPU mode / dot / `LY`, DMA, timer, APU, and cartridge / MBC state.
   Acceptance criteria: a blocking divergence can be localized without a long blind rerun, and the project has practical viewers or equivalent dumps for PPU, cartridge / MBC, APU, and IRQ state.
5. Lock determinism, replay, save/load determinism, and regression retention.
   Scope: same-ROM replay with identical execution mode, explicit overrides, input stream, and injected time source, mid-run save/load equivalence through automated determinism tests, and permanent regression assets for important hardening bugs fixed during this phase.
   Acceptance criteria: repeated runs converge exactly under the same recorded mode, overrides, inputs, and injected time source for the accepted closure lanes; save/load continuation matches uninterrupted execution; mismatched-mode restore is rejected by default; and fixed hardening bugs leave behind permanent regression assets.

#### Done criteria

- core unit and short integration suites for the blocking DMG areas are green
- the minimum external closure suites are green
- historical differential comparison showed no unexplained divergence in the covered scenarios and recorded accepted oracle arbitrations explicitly; current reruns are external/manual rather than repo-local automation
- deterministic replay and save/load determinism are green under `Strict`, with execution-mode metadata recorded in the relevant artifacts
- no severe open correctness bugs remain in `NoMbc`, `Mbc1`, `Mbc2`, `Mbc3`, or `Mbc5`
- the project has an explicit DMG closure checklist instead of relying on a general compatibility impression
- the repo-owned coverage gate follows [`docs/TESTING.md`](../TESTING.md); this roadmap must not duplicate concrete per-crate percentages. The single primary source for the active `--fail-under-*` thresholds is `.cargo/config.toml`.

#### Risks if omitted or overly simplified

- false confidence from a few booting games or one passing smoke suite
- unresolved blind spots in scheduler ordering, timing, or cartridge behavior
- repeated rediscovery of the same bugs because regressions were never turned into permanent assets
- inability to explain oracle divergences without expensive manual debugging sessions
