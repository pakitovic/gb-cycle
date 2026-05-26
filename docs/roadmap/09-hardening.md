# Phase 9 — Final DMG hardening, differential validation, and closure

This phase records the completed practical DMG closure work. The project closes Phase `9` through layered evidence on the shared T-cycle model rather than through informal game compatibility, using the dedicated save-state and serialization infrastructure from Phase `8` as part of the accepted closure signal.

Closure status (`2026-04-28`): Phase `9.5` is closed for the practical DMG hardening scope. The accepted closure evidence is:

- one explicit subsystem closure checklist in `docs/TESTING.md` that records the accepted repo-gated and internal evidence for the landed DMG subsystems
- one `gb-test-runner` catalog path, `cargo run -p gb-test-runner --bin run_rom_suite -- --list-detailed`, that exposes the built-in suite set together with oracle channel, capture, and retained-artifact policy
- one `gb-test-runner` checklist path, `cargo run -p gb-test-runner --bin run_rom_suite -- --early-checklist`, that exposes the accepted Phase `9` closure evidence per subsystem
- one repo-gated PPU framebuffer-oracle suite, `cargo run -p gb-test-runner --bin run_rom_suite -- --suite acid-dmg-curated`, sourced from `GBEmulatorShootout` and now part of the supported external DMG block used by `make test-roms` and the GitHub `test-roms` workflow
- one workflow-managed PPU framebuffer-oracle suite, `cargo run -p gb-test-runner --bin run_rom_suite -- --suite mealybug-tearoom-dmg-curated [--failure-artifact-root <dir>]`, which uses a curated DMG subset from `GBEmulatorShootout` and the same committed-PNG oracle contract as `dmg-acid2`
- one workflow-managed DMG acceptance lane, with the full local suite `cargo run -p gb-test-runner --bin run_rom_suite -- --suite mooneye-acceptance-dmg-curated [--failure-artifact-root <dir>]` and the CI-sized chunks `mooneye-dmg-acceptance-manual`, `mooneye-dmg-emulator-mbc1-mbc5`, and `mooneye-dmg-emulator-mbc2`, which follow the active `GBEmulatorShootout` `testroms/mooneye.py` acceptance list, use the upstream `mooneye` breakpoint/register result protocol instead of framebuffer fixtures, and provide broad hardening evidence for the accepted Phase `9` closure matrix
- one narrow differential end-of-test path, `cargo run -p gb-test-runner --bin run_differential -- --oracle sameboy [--oracle-layout <case-bundle|sameboy-tester>] [--oracle-artifact-root <dir>] --suite <suite-name>`, which compares the built-in suite's required-capture artifact against an imported oracle artifact bundle, enforces `Strict`, and archives local context plus the compared oracle artifact on divergence. The current path also reports the first differing byte or pixel inside the compared final artifact. When the oracle root is omitted, the repo-local default is `/.oracles/<oracle>/<layout>/`
- one LibSameBoy case-bundle materialization path, `cargo run -p gb-test-runner --bin run_sameboy_case_bundle -- --suite <suite-name> [--oracle-root <dir>] [--sameboy-root <dir> | --runner-binary <path>] [--build-if-missing]`, which builds SameBoy's `lib` target when requested, compiles the repo-owned helper against `libsameboy`, applies suite startup memory writes, and produces portable `serial_hex.txt` or `framebuffer.pgm` artifacts in the `case-bundle` layout consumed by `run_differential`
- one first-divergence probe lane, `cargo run -p gb-test-runner --bin run_first_divergence -- --oracle sameboy --suite <suite-name> [--case <case-id>] [--probe-interval-tcycles <n>] [--compare-mode <framebuffer|state>] [--allow-divergence] [--build-if-missing]`, which captures local and LibSameBoy JSONL probe streams under `/.oracles/sameboy/first-divergence/<case-id>/`, compares normalized framebuffer hashes by default, and preserves CPU, timer, IRQ, PPU register, memory-hash, and serial context for the first mismatching short window
- one SameBoy-eligible Mealybug differential suite, `mealybug-tearoom-dmg-sameboy-differential`, which keeps the Phase `9` Mealybug SameBoy lane limited to rows where GBEmulatorShootout updated on March 22, 2026 marks SameBoy as `PASS`; the full `mealybug-tearoom-dmg-curated` local gate still keeps all 24 curated fixture cases
- one accepted Phase `9` determinism lane, `cargo run -p gb-test-runner --bin run_determinism -- --suite <suite-name> [--case <case-id>]`, which performs deterministic replay, in-memory save/load continuation, and mismatched-metadata restore rejection checks for strict built-in ROM cases
- stable Makefile helpers for repeated Phase `9` local work: `phase9-determinism-smoke`, `phase9-determinism-local`, `phase9-sameboy-cartridge-oracles`, `phase9-diff-cartridge`, `phase9-sameboy-acid-oracles`, `phase9-diff-acid`, `phase9-sameboy-mealybug-oracles`, `phase9-diff-mealybug`, `phase9-sameboy-ashiepaws-oracles`, `phase9-diff-ashiepaws`, and `phase9-first-divergence-ashiepaws`

Local validation on April 27, 2026 materialized `phase-6-cartridge-oracle` with LibSameBoy and matched `make phase9-diff-cartridge`, satisfying the Phase `9.2` cartridge lane; it also matched `make phase9-diff-mealybug` for the SameBoy-eligible `mealybug-tearoom-dmg-sameboy-differential` subset after documenting rows where GBEmulatorShootout marks SameBoy as `FAIL` as SameBoy oracle limitations. Local validation on April 28, 2026 matched `make phase9-diff-ashiepaws`, satisfying the curated Ashiepaws framebuffer differential slice, and `run_first_divergence` now produces short-window LibSameBoy probe context for divergence localization. Together with `phase9-determinism-smoke`, `phase9-determinism-local`, the repo-gated external ROM lanes, and the closure checklist in `docs/TESTING.md`, this closes Phase `9.5` for the practical DMG scope.

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
- differential comparison tooling for SameBoy with first-divergence reporting and short T-cycle windows
- deterministic replay and save/load determinism through the accepted `run_determinism` lanes
- minimum closure-ready debugging tooling: traces, breakpoints, watchpoints, snapshots, and targeted subsystem viewers
- explicit DMG closure checklist covering internal suites, external suites, differential comparison, determinism, save/load determinism, and primary cartridge families

#### Completed sequencing inside Phase 9

1. Formalize the DMG hardening matrix and closure severity policy.
   Scope: define layers `A/B/C/D/E`, `must-pass` versus non-blocking categories, minimum DMG closure suites, and the rule that no single layer substitutes for another.
   Acceptance criteria: the project docs name the closure layers explicitly, identify the blocking hardware areas for DMG closure, and define a stable checklist instead of relying on informal compatibility claims.
2. Build the external ROM harness and minimum closure suites.
   Scope: automate CPU / interrupt ROMs, `dmg-acid2`, `mealybug-tearoom-tests`, Mooneye acceptance, Blargg DMG, and the Phase `6` cartridge oracle; support framebuffer and serial capture; define timeouts, pass/fail rules, and retained artifacts.
   Acceptance criteria: the minimum DMG closure ROM suites run without manual screen inspection, every case has a timeout plus explicit pass/fail policy, and the harness can preserve enough output to debug failures offline.
3. Add differential comparison against SameBoy.
   Scope: end-of-test comparison for the accepted SameBoy lanes plus first-divergence localization with archived probe context.
   Acceptance criteria: SameBoy acts as the DMG oracle for the covered scenarios, accepted oracle limitations are documented, and the tooling can preserve first-divergence context instead of only a final mismatch.
4. Close the minimum debugging and inspection tooling.
   Scope: instruction / micro-op / short-window T-cycle tracing, breakpoints and watchpoints on `PC`, memory, MMIO, and cartridge-bank state, plus fast inspection of CPU, scheduler, bus owner, PPU mode / dot / `LY`, DMA, timer, APU, and cartridge / MBC state.
   Acceptance criteria: a blocking divergence can be localized without a long blind rerun, and the project has practical viewers or equivalent dumps for PPU, cartridge / MBC, APU, and IRQ state.
5. Lock determinism, replay, save/load determinism, and regression retention.
   Scope: same-ROM replay with identical execution mode, explicit overrides, input stream, and injected time source, mid-run save/load equivalence through `run_determinism`, and permanent regression assets for important hardening bugs fixed during this phase.
   Acceptance criteria: repeated runs converge exactly under the same recorded mode, overrides, inputs, and injected time source for the accepted closure lanes; save/load continuation matches uninterrupted execution; mismatched-mode restore is rejected by default; and fixed hardening bugs leave behind permanent regression assets.

#### Done criteria

- core unit and short integration suites for the blocking DMG areas are green
- the minimum external closure suites are green
- differential comparison shows no unexplained divergence in the covered scenarios and records accepted oracle arbitrations explicitly
- deterministic replay and save/load determinism are green under `Strict`, with execution-mode metadata recorded in the relevant artifacts
- no severe open correctness bugs remain in `NoMbc`, `Mbc1`, `Mbc2`, `Mbc3`, or `Mbc5`
- the project has an explicit DMG closure checklist instead of relying on a general compatibility impression
- the repo-owned coverage gate follows `docs/TESTING.md`; this roadmap must not duplicate concrete per-crate percentages. The single primary source for the active `--fail-under-*` thresholds is `.cargo/config.toml`.

#### Risks if omitted or overly simplified

- false confidence from a few booting games or one passing smoke suite
- unresolved blind spots in scheduler ordering, timing, or cartridge behavior
- repeated rediscovery of the same bugs because regressions were never turned into permanent assets
- inability to explain oracle divergences without expensive manual debugging sessions
