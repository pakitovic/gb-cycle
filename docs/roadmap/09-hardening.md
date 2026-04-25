# Phase 9 — Final DMG hardening, differential validation, and closure

This phase is the roadmap home for the final DMG closure work. Parts of it should begin earlier, but the block only closes once the project can justify DMG correctness through layered evidence on the shared T-cycle model rather than through informal game compatibility.
It assumes the dedicated save-state and serialization infrastructure from Phase 8 already exists and uses it as part of closure evidence.

Status note (`2026-03-22`): the repo now starts a narrow early hardening lane
from this phase before APU work. The current early deliverables are:

- one explicit partial subsystem checklist in `docs/TESTING.md` that distinguishes
  repo-gated external evidence from internal-only evidence for the already-landed
  DMG subsystems
- one `gb-test-runner` catalog path,
  `cargo run -p gb-test-runner --bin run_rom_suite -- --list-detailed`, that
  exposes the built-in suite set together with oracle channel, capture, and
  retained-artifact policy
- one `gb-test-runner` checklist path,
  `cargo run -p gb-test-runner --bin run_rom_suite -- --early-checklist`, that
  exposes the current early hardening status per subsystem together with the
  evidence already landed and the still-open closure gaps
- one repo-gated PPU framebuffer-oracle suite,
  `cargo run -p gb-test-runner --bin run_rom_suite -- --suite acid-dmg-curated`,
  sourced from `GBEmulatorShootout` and now part of the supported external DMG
  block used by `make test-roms` and the GitHub `test-roms` workflow
- one workflow-managed PPU framebuffer-oracle suite,
  `cargo run -p gb-test-runner --bin run_rom_suite -- --suite mealybug-tearoom-dmg-curated [--failure-artifact-root <dir>]`,
  which uses a curated DMG subset from `GBEmulatorShootout` and the same
  committed-PNG oracle contract as `dmg-acid2`
- one workflow-managed DMG acceptance suite,
  `cargo run -p gb-test-runner --bin run_rom_suite -- --suite mooneye-acceptance-dmg-curated [--failure-artifact-root <dir>]`,
  which follows the active `GBEmulatorShootout` `testroms/mooneye.py`
  acceptance list, uses the upstream `mooneye` breakpoint/register result
  protocol instead of framebuffer fixtures, and provides broad hardening
  evidence without replacing later differential, replay, or save/load
  determinism closure
- one narrow differential end-of-test path,
  `cargo run -p gb-test-runner --bin run_differential -- --oracle sameboy [--oracle-layout <case-bundle|sameboy-tester>] [--oracle-artifact-root <dir>] --suite <suite-name>`,
  which compares the built-in suite's required-capture artifact against an
  imported oracle artifact bundle, enforces `Strict`, and archives local
  context plus the compared oracle artifact on divergence. The current path
  also reports the first differing byte or pixel inside the compared final
  artifact, even though full instruction-level or short-window first-divergence
  tooling is still deferred. The current `sameboy-tester` layout support is
  intentionally framebuffer-only and is aimed at PPU/image-oracle cases such as
  `dmg-acid2`. When the oracle root is omitted, the repo-local default is
  `/.oracles/<oracle>/<layout>/`
- one SameBoy Tester materialization path,
  `cargo run -p gb-test-runner --bin run_sameboy_tester -- --suite <suite-name> [--oracle-root <dir>] [--sameboy-root <dir> | --tester-binary <path>]`,
  which stages ROMs under the oracle root, runs SameBoy's internal `tester`
  target, and produces `.bmp` / `.tga` plus `.log` artifacts in the exact
  `sameboy-tester` layout consumed by `run_differential`. The repo-local
  default for this path is `/.oracles/sameboy/sameboy-tester/` for oracle
  outputs, and the wrapper intentionally leaves SameBoy's own boot-ROM path
  under SameBoy's control instead of trying to share local firmware selection
  with `gb-test-runner`

This does not count as closing Phase `9.2` or `9.3`; fuller SameBoy
differential launch automation, first-divergence windows, save/load determinism,
and the final DMG matrix still remain Phase `7/8/9` work.

#### Goal

Close the DMG core with a formal validation matrix, strong differential and determinism tooling, and explicit closure criteria that leave no major blind hardware areas behind.

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
- deterministic replay, save/load determinism, and longer-running soak coverage
- minimum closure-ready debugging tooling: traces, breakpoints, watchpoints, snapshots, and targeted subsystem viewers
- explicit DMG closure checklist covering internal suites, external suites, differential comparison, determinism, save/load determinism, and primary cartridge families

#### Recommended sequencing inside Phase 9

1. Formalize the DMG hardening matrix and closure severity policy.
   Scope: define layers `A/B/C/D/E`, `must-pass` versus non-blocking categories, minimum DMG closure suites, and the rule that no single layer substitutes for another.
   Acceptance criteria: the project docs name the closure layers explicitly, identify the blocking hardware areas for DMG closure, and define a stable checklist instead of relying on informal compatibility claims.
2. Build the external ROM harness and minimum closure suites.
   Scope: automate CPU / interrupt ROMs, `dmg-acid2`, and `mealybug-tearoom-tests`; support framebuffer and serial capture; define timeouts, pass/fail rules, and retained artifacts; and keep explicit reserved follow-up slots for broader closure suites such as Mooneye / Gekkio coverage, SameSuite, GB Accuracy Tests, 144p Test Suite, and MBC3 RTC-focused ROMs.
   Acceptance criteria: the minimum DMG closure ROM suites run without manual screen inspection, every case has a timeout plus explicit pass/fail policy, and the harness can preserve enough output to debug failures offline.
3. Add differential comparison against SameBoy.
   Scope: end-of-test comparison, end-of-instruction comparison, short T-cycle-window comparison, and first-divergence localization with archived context.
   Acceptance criteria: SameBoy acts as the DMG oracle for the covered scenarios, and the tooling can report the first divergence instead of only a final mismatch.
4. Close the minimum debugging and inspection tooling.
   Scope: instruction / micro-op / short-window T-cycle tracing, breakpoints and watchpoints on `PC`, memory, MMIO, and cartridge-bank state, plus fast inspection of CPU, scheduler, bus owner, PPU mode / dot / `LY`, DMA, timer, APU, and cartridge / MBC state.
   Acceptance criteria: a blocking divergence can be localized without a long blind rerun, and the project has practical viewers or equivalent dumps for PPU, cartridge / MBC, APU, and IRQ state.
5. Lock determinism, replay, save/load determinism, soak, and regression retention.
   Scope: same-ROM replay with identical execution mode, explicit overrides, input stream, and injected time source, mid-run save/load equivalence, longer-running soak cases, and a permanent regression path for every important hardening bug.
   Acceptance criteria: repeated runs converge exactly under the same recorded mode, overrides, inputs, and injected time source; save/load continuation matches uninterrupted execution; mismatched-mode restore is rejected by default; soak coverage includes at least one real game plus long-running synthetic coverage; and fixed hardening bugs leave behind permanent regression assets.

#### Done criteria

- core unit and short integration suites for the blocking DMG areas are green
- the minimum external closure suites are green
- differential comparison either shows no unexplained divergence in the covered scenarios or records the remaining arbitrations explicitly
- deterministic replay and save/load determinism are green under `Strict`, with execution-mode metadata recorded in the relevant artifacts
- no severe open correctness bugs remain in `NoMbc`, `Mbc1`, `Mbc2`, `Mbc3`, or `Mbc5`
- the project has an explicit DMG closure checklist instead of relying on a general compatibility impression
- the repo-owned coverage gate follows `docs/TESTING.md`; this roadmap must not
  duplicate concrete per-crate percentages. The single primary source for the
  active `--fail-under-*` thresholds is `.cargo/config.toml`.

#### Risks if omitted or overly simplified

- false confidence from a few booting games or one passing smoke suite
- unresolved blind spots in scheduler ordering, timing, or cartridge behavior
- repeated rediscovery of the same bugs because regressions were never turned into permanent assets
- inability to explain oracle divergences without expensive manual debugging sessions

