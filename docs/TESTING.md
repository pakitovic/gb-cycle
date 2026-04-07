# Testing

## Testing strategy

Use multiple layers:

- focused unit tests
- subsystem integration tests
- ROM-based validation
- oracle comparisons where useful
- determinism, replay, and longer-running regression coverage

## Authority and scope

- This document owns project-wide validation policy and cross-subsystem testing expectations.
- Detailed subsystem-specific checklists remain owned by the matching `docs/hardware/*.md` handbook.
- When this file repeats subsystem expectations for planning convenience, the subsystem handbook remains the behavioral authority and this file should be updated to match it.
- `docs/ROADMAP.md` may mention validation goals by phase, but it does not replace this testing policy.

## Validation priorities

Every subsystem change should aim to leave behind one of these:

- a focused automated test for the local invariant
- a ROM-based reproduction case
- a documented oracle comparison when timing or ordering is under review
- a characterization test before structural refactors in behavior-sensitive code

## Final DMG hardening policy

- Do not treat "boots a few commercial games" or "one final framebuffer looks right" as DMG closure.
- Final DMG hardening must demonstrate, at the same time, subsystem-local correctness, deterministic behavior on the shared T-cycle timeline, differential agreement with trusted oracles, and enough tooling to explain a divergence without blind refactoring.
- Use a formal multilayer validation matrix rather than one oversized suite or informal compatibility anecdotes.
- No single layer substitutes for another: unit tests do not replace test ROMs, test ROMs do not replace differential comparison, and differential comparison does not replace determinism or replay.

## Final DMG hardening matrix

- `Layer A`: focused unit tests per subsystem, including CPU micro-ops and state machine behavior, interrupts and `IME` / `HALT` / `STOP`, timer and divider edges, bus arbitration, PPU mode timing plus fetcher/FIFOs/window/sprites, DMA, joypad, serial, cartridge/MBC behavior, APU channel and mixer behavior, and exact save-state restore paths.
- `Layer B`: short integration tests for timing-coupled interactions such as timer plus interrupts, DMA plus bus plus CPU, PPU plus DMA plus VRAM/OAM blocking, serial plus interrupt plus scheduler, joypad plus `STOP` / `HALT` plus interrupt, cartridge plus bus plus boot-ROM overlay, and APU plus `DIV-APU` plus MMIO writes.
- `Layer C`: external test ROM suites grouped by hardware category and run through automation rather than manual screen inspection.
- `Layer D`: differential comparison against trusted emulator oracles, with the ability to localize the first divergence instead of reporting only one final mismatch.
- `Layer E`: determinism, replay, save/load determinism, soak, and regression-retention coverage over medium and long runs.
- For this T-cycle-based core, the matrix should support several validation granularities: end of test, end of instruction, and short per-T-cycle windows when a timing-sensitive divergence needs to be isolated.
- Every corrected bug should end up attached to at least one permanent layer in this matrix.

## Early Phase 9 partial checklist

Before full Phase `7/8/9` closure exists, keep one explicit partial-hardening
checklist for the already landed subsystems. This checklist is not final DMG
closure; it is the minimum "do not keep flying blind" matrix used while the
project is still bringing up later hardware blocks such as APU and save states.

- `CPU`: repo gate present. Current evidence: Phase `2` synthetic timing ROMs plus
  curated external Blargg DMG individual ROM coverage. Remaining final-closure gaps:
  differential oracle workflow, replay / determinism, and broader boot-path
  arbitration.
- `Interrupts`: repo gate present. Current evidence: Phase `2` interrupt timing ROMs,
  Blargg `halt_bug`, and the interrupt-heavy `cpu_instrs` cases. Remaining final-closure
  gaps: dedicated differential traces and longer-running determinism coverage.
- `Timer`: internal gate only. Current evidence: Phase `2` timer and interrupt-service
  synthetic ROMs plus unit coverage in `gb-core`. Remaining final-closure gaps:
  dedicated external timing suites and differential oracle coverage.
- `Bus`: repo gate present. Current evidence: bus/DMA/CPU integration coverage plus
  external Blargg `mem_timing`, `mem_timing-2`, and their individual ROMs. Remaining
  final-closure gaps: broader cartridge-family oracle comparisons and save/load-era
  determinism checks.
- `DMA`: internal gate only. Current evidence: closed Phase `3` unit and integration
  coverage. Remaining final-closure gaps: a promoted external ROM slice and later
  differential tooling.
- `APU`: repo gate present. Current evidence: repo-local APU MMIO/power-readback
  coverage, repo-local unit and machine integration coverage for the explicit
  `DAC -> NR51 -> NR50 -> HPF` output path and the current non-intrusive
  post-HPF snapshot capture boundary, plus the full curated
  `blargg-dmg-curated` family now promoted into the repo-gated external DMG
  block, including Blargg `dmg_sound 01..12`.
  Remaining final-closure gaps: differential oracle coverage and later
  frontend/export validation beyond that now-explicit core-owned snapshot
  capture boundary.
- `PPU`: repo gate present for the currently closed OAM-corruption slice. Current
  evidence: Phase `4` synthetic OAM-corruption ROMs, curated Blargg `oam_bug`
  singles `1..6,8`, a repo-gated `dmg-acid2` framebuffer oracle, and the
  curated `acid/which.gb` informational DMG execution lane.
  Remaining final-closure gaps: a green repo-gated `mealybug-tearoom` slice,
  broader rendering/timing differential coverage, and the still-deferred
  non-curated exploratory ROMs.
- `Cartridge`: repo gate present for the currently closed mapper-oracle slice.
  Current evidence: unit and integration coverage for `NoMbc`, `Mbc1`, `Mbc2`,
  `Mbc3`, `Mbc5`, hardware-style persistence, the built-in
  `phase-6-cartridge-oracle` differential-ready synthetic suite in
  `gb-test-runner`, and retained SameBoy `case-bundle` `serial_hex` artifacts
  for the shipped `MBC1`, `MBC2`, `MBC3`, and `MBC5` mapper edge cases.
  Remaining final-closure gaps: explicit `M161` identification, broader
  special-cartridge closure, and later save/load determinism once Phase `8`
  exists.
- `Joypad` / `Serial`: internal gate only. Current evidence: closed Phase `5`
  synthetic coverage and subsystem tests. Remaining final-closure gaps: promoted
  external closure ROMs and differential comparison hooks.

This checklist should move only when one of the following becomes true:

- a subsystem gains a repo-gated external suite with explicit pass/fail policy
- a subsystem gains first-divergence differential tooling against a trusted oracle
- a previously internal-only block acquires deterministic replay or save/load
  continuation evidence
- a discovered regression forces the checklist back from "repo gate present" to
  "internal gate only" until the external evidence is green again

## External ROM harness policy

- Test ROM execution must be automatable; manual inspection of the LCD is an auxiliary debugging aid, not the primary acceptance path.
- When working on already-known external ROM failures, timing regressions, or exploratory PPU/MMIO fixes, always preserve a baseline snapshot of `/.roms/test/test-report.md` before making the validation run, preserve the final report again after the run, and compare the two before deciding whether the iteration is worth keeping.
- That baseline/final comparison is mandatory for go/no-go decisions on exploratory ROM-driven work. Save the raw markdown report first, typically as explicit `test-report-before.md` and `test-report-after.md` artifacts. Do not require rendered images of the report unless a task explicitly asks for them.
- If the current tree is not already a clean baseline, compare against a clean reference such as `main` in a separate branch or worktree rather than hand-waving the previous state from memory.
- Do not summarize a ROM-driven iteration as "no regressions" unless the before/after report comparison has been done explicitly and the changed rows have been named.
- The harness should support at least framebuffer capture and serial / link-port capture when the ROM exposes machine-readable output there.
- When an external suite prints self-validating text through a documented screen-console protocol, the harness should also support a typed text-extraction path for that protocol rather than falling back to a circular framebuffer fixture generated by this project.
- Prefer serial / link-port capture for suites such as Blargg `cpu_instrs` when that path is available, because it avoids treating a scrolling framebuffer as the primary machine-readable result channel.
- For the current official Blargg bring-up, the harness now supports three machine-readable channels: serial, cartridge-RAM text/status output, and the upstream `console.s` BG-map text console used by screen-driven ROMs such as `halt_bug`.
- Before promoting a third-party ROM into the built-in automated catalog, verify whether it is listed in `GBEmulatorShootout` and record any explicit exception instead of assuming the ROM belongs in the default curated set.
- A ROM omitted from `GBEmulatorShootout` may still be useful for exploratory debugging, but it must stay out of the repo-managed built-in suites until the reason for including it anyway is documented explicitly.
- Each ROM case should define a timeout, an explicit pass/fail rule, and retained failure artifacts such as serial output, framebuffer output, trace excerpts, and optional snapshots.
- For the current curated Mooneye DMG acceptance slice, retain at least snapshot plus serial artifacts on failure. Many cases still use the register-signature oracle for pass/fail, but keeping serial alongside the snapshot shortens diagnosis when a ROM emits a more specific failure reason before falling into the common Mooneye stop loop.
- The exploratory `daid` DMG slice currently mixes ordinary framebuffer fixtures, one multi-fixture framebuffer oracle for `ppu_scanline_bgp.gb`, and one informational framebuffer capture for `rom_and_ram.gb`; keep that family out of the default repo-gated block until its expected green set is explicit. The informational `rom_and_ram.gb` case currently runs under `Permissive` because its `NoMbc` header uses a legacy RAM-size declaration that the project treats as a warning outside `Strict`.
- When a ROM needs deterministic host-side interaction, the typed case metadata should also carry the external stimulus schedule explicitly instead of burying that behavior in ad hoc test-only closures.
- During early Phase `0`, `gb-test-runner` could begin as a contract-only crate, but it should already own typed ROM-case and suite metadata including console model, startup mode, execution mode, emulation-progress timeout, explicit pass/fail rule, external stimulus schedule when needed, requested captures, and retained failure-artifact policy.
- In the current baseline, `gb-test-runner` is already an executable harness as well: it can load typed suites, run ROMs on the shared T-cycle machine, capture serial / framebuffer / snapshot artifacts, and preserve failure outputs without relying on a frontend.
- Typed ROM-case metadata may also carry deterministic startup memory writes when a curated oracle depends on one explicit post-boot memory artifact that the current `SkipBoot` baseline does not synthesize yet. Keep that path narrow, document the provenance of the bytes, and prefer boot-derived state such as the DMG trademark tile or logo VRAM/map bytes over ad hoc framebuffer patching. This currently covers curated mealybug cases that intentionally reuse tile `0x19` from the DMG boot ROM instead of uploading their own tile data, plus the curated `hacktix/bully.gb` DMG case that checks the boot-derived logo VRAM seed under `SkipBoot`.
- When a typed suite is landed before its redistributable assets, reserve the exact ROM and trace filenames in the repo with per-phase README stubs so later automation and oracle work reuse one stable target contract instead of inventing new names ad hoc.
- Repo-managed external ROM assets should keep only one persistent workspace-local
  gitignored layer: the curated runnable store under `/.roms/test/`. Any raw
  upstream checkout used to materialize that store should be temporary and cleaned
  up after the fetch command completes.
- The external-ROM fetch workflow must run all git commands from inside its
  temporary checkout or fixture repository and must not rewrite the invoking
  repo's local git config while doing so. Test-only commit identity should be
  supplied through per-command environment variables rather than persisted
  `user.name` / `user.email` entries.
- The curated fetch command should support both full-store materialization and
  explicit family subsets so repo-gated and exploratory `make test-*` targets
  can remain autosufficient without forcing unrelated families to be fetched
  first.
- The upstream source, pinned revision, and required-file hashes belong in the
  versioned manifest `crates/gb-test-runner/data/sources.toml`.
- The runnable curated families belong under `/.roms/test/<family>/`, using one
  checked-in manifest per family under `crates/gb-test-runner/data/*.toml`
  so supported ROMs can be added or commented explicitly without editing runner code.
- `gb-test-runner` may accept explicit environment-variable roots for curated suites,
  but the default automation path should also resolve the repo-managed curated store
  automatically so developers and CI do not need ad hoc local clones or handwritten
  path setup.
- Curated family runs should update `/.roms/test/test-report.md` with a simple
  per-ROM status table so repo-managed `PASS` / `FAIL` / `INFO` state stays
  visible without re-reading logs; the markdown view should render those states
  as `✅`, `❌`, and `ℹ️` rather than repeating the raw persisted strings. The
  report header should also include a `non-failing/total` summary for the exact
  set of persisted rows currently rendered in that markdown file, counting both
  `PASS` and `INFO` in the numerator, so a first partial run reports only its
  own rows while later partial updates keep counting the broader persisted
  context already present in the report.
  When multiple curated families are present in the report, they should render
  in the fixed order `acid`, `blargg`, `daid`, `ax6`, `mooneye`, `samesuite`,
  `hacktix`, `cpp`, `mealybug-tearoom-tests`; families with no persisted cases
  should not appear at all. Within each populated family, rows should follow
  the curated family manifest order instead of being alphabetized by ROM
  filename.
- Keep redistributable external test ROMs and non-redistributable commercial ROMs in separate stores. The current local-only commercial bucket is `/.roms/local-commercial/`, and it must remain outside CI, docs about official closure, and public automation targets.
- For ad hoc local commercial-ROM bring-up that still needs deterministic host
  input, prefer `gb-test-runner --manifest <path>` over growing `gb-cli`
  into a second harness. That manifest path should carry the typed case
  contract directly, including model, startup mode, timeout, oracle, and any
  scheduled joypad stimuli.
- Keep local boot ROM images under the repo-managed gitignored `/.roms/bootrom/`
  store, using the canonical filenames from `gb-core` (`dmg0_boot.bin`,
  `dmg_boot.bin`, `mgb_boot.bin`) so local real-boot runs do not depend on ad
  hoc per-machine paths.
- For the current DMG-family store, `gb-test-runner` treats those boot ROM
  assets as pinned local inputs rather than arbitrary filenames: strict-mode
  `RealBoot` verifies the observed SHA-256 against the expected
  `dmg0/dmg/mgb` hashes before execution so local bring-up does not silently
  proceed on the wrong firmware bytes.
- For ad hoc local `gb-cli` bring-up with `RealBoot`, keep the default no-limit
  behavior tied to the actual boot path instead of reusing the `SkipBoot`
  budget blindly: the CLI should treat boot-ROM handoff as the semantic start
  of the post-boot run window, while still retaining a finite safety cap when
  `FF50` never unmapped.
- Keep one repo-local ignored real-boot regression for those verified DMG-family
  assets so `dmg0`, `dmg`, and `mgb` can be exercised one by one against the
  real core, real bus, and real `FF50` handoff path without baking machine-local
  paths into `gb-core`. The current coverage lives in
  `crates/gb-test-runner/tests/external.rs` and uses a minimal valid `NoMBC`
  cartridge header whose first post-handoff opcode deliberately traps, making
  the next-fetch cartridge entry point observable after the boot ROM unmaps.
- Keep imported differential oracle artifacts under the repo-managed gitignored
  `/.oracles/<oracle>/<layout>/` tree instead of scattering them under `/tmp`,
  so repeated validation runs have one visible workspace-local location.
- For ad hoc local framebuffer inspection outside `gb-test-runner`, keep
  `gb-cli --framebuffer-out` extension-driven: a `.png` target should emit a
  real grayscale PNG for human viewing, while non-`.png` targets may stay on
  the lighter raw-PGM path used for low-friction local artifacts.
- For manifest-driven local `gb-test-runner` cases that capture framebuffer,
  export the resulting PNG beside the ROM using the ROM stem so one-off local
  commercial bring-up does not require a separate artifact-root convention.
- The minimum DMG closure baseline should include automated CPU / interrupt coverage
  through curated Blargg DMG automation sourced from `GBEmulatorShootout`,
  curated Acid DMG coverage for basic PPU validation, and `mealybug-tearoom-tests`
  for fine PPU rendering / timing validation.
- Keep explicit roadmap space for broader closure suites such as Mooneye / Gekkio coverage, SameSuite, GB Accuracy Tests, 144p Test Suite, and MBC3 RTC-focused ROMs.
- `gb-test-runner` should expose a human-readable catalog of the built-in suites
  and their active oracle channels. The current CLI entry point is
  `cargo run -p gb-test-runner --bin run_rom_suite -- --list-detailed`.
- The same harness should also expose the current early hardening checklist so
  the repo can answer "what is externally gated already and what is still
  internal only?" without re-reading the docs. The current CLI entry point is
  `cargo run -p gb-test-runner --bin run_rom_suite -- --early-checklist`.
- The current early PPU hardening lane also includes the curated Acid DMG family
  under
  `cargo run -p gb-test-runner --bin run_rom_suite -- --suite acid-dmg-curated`.
- That family currently mixes one repo-gated framebuffer-oracle case
  `dmg-acid2.gb` with one non-blocking informational framebuffer case
  `which.gb`, mirroring the `GBEmulatorShootout` classification rather than
  forcing a synthetic pass/fail oracle where upstream does not define one.
- The current early PPU hardening lane also includes one non-gated exploratory
  framebuffer suite for `mealybug-tearoom-tests` under
  `cargo run -p gb-test-runner --bin run_rom_suite -- --suite mealybug-tearoom-dmg-curated [--failure-artifact-root <dir>]`.
  This suite uses a curated DMG-only subset sourced from `GBEmulatorShootout`
  and the same committed-PNG oracle contract as `dmg-acid2`, but it is
  currently red under `Strict`. The local `make test-roms` aggregator still
  runs it for visibility, but it remains outside the GitHub `test-roms`
  workflow until the underlying PPU mismatches are corrected.
- The current DMG framebuffer lane also includes one workflow-managed
  `hacktix` suite under
  `cargo run -p gb-test-runner --bin run_rom_suite -- --suite hacktix-dmg-curated [--failure-artifact-root <dir>]`.
  This suite currently tracks the `GBEmulatorShootout` `hacktix` subset
  `bully.gb` plus `strikethrough.gb`, runs those ROMs on the default DMG
  model, and uses the same committed-PNG framebuffer-oracle contract as the
  other screenshot-based curated families. It is exercised by
  `make run-hacktix`, the local `make test-roms` aggregator, and the GitHub
  `test-roms` workflow.
- The current exploratory DMG `mooneye` lane also includes one non-gated
  suite under
  `cargo run -p gb-test-runner --bin run_rom_suite -- --suite mooneye-acceptance-dmg-curated [--failure-artifact-root <dir>]`.
  This suite follows the active `GBEmulatorShootout`
  `testroms/mooneye.py` DMG list rather than inventing a local file list: it
  keeps the upstream `acceptance/*` entries plus the DMG
  `emulator-only/mbc1/*`, `emulator-only/mbc2/*`, `emulator-only/mbc5/*`, and
  `manual-only/sprite_priority.gb` cases that appear before the CGB-only
  `misc/*` block. It runs those ROMs on the default DMG model. Most cases use
  the upstream `mooneye` pass/fail breakpoint protocol via the documented
  register signature at `LD B,B`; the single `manual-only/sprite_priority.gb`
  exception instead uses the committed framebuffer fixture
  `crates/gb-test-runner/data/fixtures/mooneye/sprite_priority.dmg.png`,
  matching the upstream manual-test classification and the reference PNG
  shipped by `GBEmulatorShootout`. Because the runner samples once per
  T-cycle, treat the immediate post-breakpoint `nop; jr -3` halt loop as the
  same terminal condition when those registers still match the documented
  pass/fail signature. It is intentionally exploratory for now: the local
  `make test-roms` aggregator runs it, but it stays outside the GitHub
  `test-roms` workflow until its remaining `acceptance/ppu/*` failures are triaged and
  the experimental `emulator-only/mbc1/multicart_rom_8Mb.gb` heuristic path is
  either retired or promoted under a documented strict-mode contract.
- The current early `9.3` MVP also includes one imported-oracle end-of-test
  differential path under
  `cargo run -p gb-test-runner --bin run_differential -- --oracle sameboy [--oracle-layout <case-bundle|sameboy-tester>] [--oracle-artifact-root <dir>] --suite <suite-name>`.
  This path enforces `Strict`, compares the suite's required-capture artifact
  against an imported oracle artifact bundle, and archives local context on
  divergence; it intentionally does not yet provide end-of-instruction or
  short-window first-divergence tracing, but it
  now does report the first differing byte or pixel inside the compared final
  artifact. The current `sameboy-tester` layout support is limited to
  framebuffer-oracle cases because SameBoy's internal tester emits image plus
  log artifacts rather than the serial or memory-text channels used by the
  Blargg text suites. When `--oracle-artifact-root` is omitted, the repo-local
  default is `/.oracles/<oracle>/<layout>/`.
- The current built-in cartridge lane for that differential path is
  `phase-6-cartridge-oracle`, which reuses the retained synthetic Phase `6`
  `MBC1`, `MBC2`, `MBC3`, and `MBC5` ROM fixtures under a stable
  `TestSubsystem::Cartridge` suite contract. That lane currently uses the
  `case-bundle` layout rather than `sameboy-tester`, because the `MBC3` case
  needs explicit pre-run RTC advancement and the relevant compared artifact is a
  portable `serial_hex.txt` payload rather than a framebuffer image.
- The repo now also includes a companion SameBoy `case-bundle` materialization
  command under
  `cargo run -p gb-test-runner --bin run_sameboy_case_bundle -- --suite <suite-name> [--oracle-root <dir>] [--sameboy-root <dir> | --runner-binary <path>] [--build-if-missing]`.
  This command executes the selected suite through a small `libsameboy`-backed
  helper, writes portable cartridge-lane artifacts such as
  `/.oracles/sameboy/case-bundle/<case-id>/serial_hex.txt`, and is the intended
  oracle-materialization path for `phase-6-cartridge-oracle`.
- The repo now also includes a companion SameBoy Tester materialization command
  under
  `cargo run -p gb-test-runner --bin run_sameboy_tester -- --suite <suite-name> [--oracle-root <dir>] [--sameboy-root <dir> | --tester-binary <path>]`.
  This command stages ROMs under the oracle root and emits the `.bmp` / `.tga`
  plus `.log` files in the `sameboy-tester` layout that `run_differential`
  already understands. When `--oracle-root` is omitted, the repo-local default
  is `/.oracles/sameboy/sameboy-tester/`. The wrapper intentionally does not
  override SameBoy's own boot-ROM path; keep this flow scoped to end-of-test
  imported-oracle materialization rather than boot-path arbitration.

## Differential oracle policy

- Use SameBoy as the default differential oracle for general DMG behavior whenever comparable observables are available.
- Use docboy as an approved secondary oracle for DMG PPU and bus work, especially for pixel FIFO, window timing, LCD restart behavior, and video-bus interaction where a second high-precision implementation helps localize a divergence.
- Differential comparison should support at least three granularities: end of test, end of instruction, and short T-cycle windows for reduced scenarios.
- Prefer a clear "first point of divergence" workflow over one final hash or framebuffer mismatch.
- A newly approved oracle may be used immediately for architectural consultation and manual differential study, but it should not count as closure evidence until its artifact-import or automation path is documented explicitly.

## Execution-mode validation policy

- `Strict` is the only mode that counts as the project's oracle path for CI, differential comparison, DMG closure, and official accuracy claims.
- `Permissive` is for tolerant interactive use and loader-validation coverage around odd but still unambiguous supported cartridges; it must not change the runtime semantics of admitted supported hardware.
- `Experimental` is for research and bring-up; its results must stay segregated from official closure metrics, oracle comparisons, and compatibility claims.
- Mode-sensitive loader tests should cover the documented category matrix for `Supported`, `PlannedVariant`, `DocumentedButUnsupported`, `ExperimentalHeuristic`, `AccessorySpecialCase`, and `UnknownCode`.
- When a test exercises heuristics, partial implementations, or manual overrides, the captured artifacts should say so explicitly rather than looking like ordinary strict-mode evidence.
- Curated ROM manifests may opt individual cases into `experimental` only when the case itself depends on a documented heuristic or partial implementation; that opt-in must live in the manifest, not as a silent suite-wide default or an implicit runner override.
- Differential comparison against SameBoy or docboy should always run under `Strict`, not under `Permissive` or `Experimental`.

## Validation tooling requirements

- Hardening-ready validation requires trace logging at instruction level, micro-op level, and short T-cycle windows.
- Breakpoints and watchpoints should cover at least `PC`, memory addresses, MMIO registers, and cartridge-bank or mapper-visible state.
- Fast state inspection should expose CPU, scheduler, current bus owner, PPU mode / dot / `LY`, active DMA state, timer pipeline state, APU state, and cartridge / MBC state.
- Specialized debug views for PPU internals, cartridge / MBC banks and raw registers, APU channels plus final mix, and IRQ / `IF` / `IE` / `IME` state are strongly recommended because they shorten divergence analysis substantially.
- Instrumentation must not alter the core's hardware-visible behavior or reorder the shared T-cycle timeline.

## New-code baseline

- New production code should normally introduce automated unit tests or integration tests in the same change.
- Prefer unit tests for local logic and integration tests when the behavior only becomes meaningful across subsystem boundaries.
- Treat "code first, tests later" as an exception that must be justified explicitly, not as the default workflow.
- Before opening or updating a pull request, run at least `make ci` locally so formatting, clippy, workspace tests, typos, and `cargo deny` do not first surface in CI.
- When a change touches CI, coverage, dependency policy, repo tooling, or other workflow-critical infrastructure, run `make test-roms` and `make coverage` locally as well before the PR is updated.
- The GitHub `ci` workflow is intentionally limited to formatting, linting, tests, typos, dependency policy, and the coverage gate. Keep external ROM execution out of that workflow.
- In that workflow, prefer one instrumented workspace `cargo llvm-cov --workspace --no-report` run plus per-crate `cargo llvm-cov report -p <crate> --fail-under-*` gates instead of a separate `cargo test --workspace` pass followed by coverage; the workspace tests should be paid for once.
- Repo-owned binary integration tests should resolve sibling executables from the active Cargo target directory rather than assuming only `target/debug` or runtime `CARGO_BIN_EXE_*` exports; coverage runs may build those binaries under an alternate root such as `target/llvm-cov-target/debug`.
- The GitHub `test-roms` workflow currently runs the workflow-managed non-CGB
  DMG suites sourced from `GBEmulatorShootout`: the curated Acid framebuffer
  oracle family, the full Blargg DMG family (`cpu_instrs 01..11`, `halt_bug`,
  `instr_timing`, `mem_timing 01..03`, `mem_timing-2 01..03`, `oam_bug 1..6,8`,
  and `dmg_sound 01..12`), plus the curated `hacktix` and `cpp` suites.
- The local `make test-roms` aggregator intentionally runs a broader set than
  the GitHub workflow today by also including the current exploratory `daid`,
  `mealybug-tearoom-tests`, and `mooneye` families.
- Keep multi-ROM bundles, CGB-only suites, and still-red exploratory ROMs out
  of the default external-ROM workflow until they are green and intentionally
  promoted.
- When a new workspace crate is created, add it to the repo-owned coverage gate
  immediately: wire a dedicated `cargo llvm-cov report -p <crate>
  --fail-under-*` alias into `.cargo/config.toml`, add that alias to the
  `coverage-check` target in `Makefile` so `make ci` exercises it, and default
  the new crate to `90/90/90` for lines/regions/functions whenever that is
  practical for the initial landing.
- For the current infrastructure-heavy stage, keep the repo-owned coverage gate per crate rather than aggregated across crates. The current temporary floors are `gb-core` at `96.00/95.95/98.43`, `gb-test-runner` at `84.22/84.63/78.87`, `gb-persistence` at `96.66/91.93/94.68`, `gb-cli` at `92.52/90.01/92.30`, and `gb-desktop` at `93.97/93.42/95.05` for lines/regions/functions respectively; do not satisfy those thresholds with hollow tests that only exercise trivial getters or app placeholders.
- When immediate automated coverage is temporarily impractical, record the missing test coverage, the reason it is deferred, and the remaining risk in the change report; add a roadmap TODO as well if the gap is concrete and non-trivial.
- ROM-based validation and oracle comparison complement automated tests; they do not replace the expectation that new code should usually leave behind unit or integration coverage.

## Phase 0 baseline test layout

- Until dedicated tooling such as `gb-test-runner` exists, keep the initial automated-validation baseline inside `crates/gb-core`.
- Keep subsystem-local invariant tests close to the production code under `crates/gb-core/src/**`.
- Keep public-API and cross-module smoke coverage under `crates/gb-core/tests/*.rs`.
- Keep shared integration-test helpers under `crates/gb-core/tests/common/`.
- Reserve `crates/gb-core/tests/fixtures/roms/` for ROM fixtures and synthetic cartridge images used by automated harnesses.
- Reserve `crates/gb-core/tests/fixtures/traces/` for golden trace artifacts and other debugger-facing snapshots.
- During early Phase `0`, prefer stable UTF-8 text trace fixtures with explicit `seq=`, `subsystem=`, `level=`, and quoted `message=` fields so ordering regressions can be locked down before richer scheduler-visible traces exist.
- Once `gb-test-runner` reserves typed phase-scoped ROM automation targets, keep the corresponding placeholder assets in matching `crates/gb-core/tests/fixtures/roms/<phase>/` and `crates/gb-core/tests/fixtures/traces/<phase>/` directories even if the actual ROMs or oracle traces are not checked in yet.
- During early Phase `0`, cover `ConsoleModel`, `StartupMode`, `ExecutionMode`, and `CompatibilityPolicy` defaults explicitly so DMG-first behavior and future CGB extension seams remain stable before scheduler and loader work land.
- During early Phase `0`, cover `SchedulerPhase` order, the global T-cycle counter, and `CycleContext` reset semantics explicitly so later subsystem work cannot smuggle timing assumptions in through accidental call order.
- During early Phase `0`, cover the single top-level `step_t_cycle()` machine entry point explicitly so the core keeps one deterministic timing boundary instead of growing multiple unsynchronized stepping APIs.
- During early Phase `0`, cover subsystem-boundary wiring explicitly so `Machine` ownership of CPU, bus, PPU, DMA, timer, boot, and cartridge remains stable before Phase `1` introduces hardware-visible behavior.
- During early Phase `0`, lock one deterministic trace order for stubbed CPU, bus, PPU, DMA, timer, boot, and cartridge hooks relative to scheduler phases so later subsystem implementation can grow observability without silently redefining trace chronology.
- During early Phase `0`, expose typed breakpoint and watchpoint contracts for `PC`, memory, MMIO, and cartridge-visible state even before CPU and bus evaluation hooks are fully wired, so debugger tooling can grow without redefining its public targets later.
- During early Phase `0`, treat debugger snapshots as typed inspection artifacts only; lock their contents with tests, but keep them explicitly separate from the whole-machine save-state work reserved for Phase `8`.
- Keep the Phase `0` baseline fast, deterministic, and frontend-independent; `gb-core` validation must not require `gb-cli`, desktop, web, or host-specific I/O.

## ROM-based validation policy

Map tests to the subsystem they validate:

- CPU and instruction behavior
- bus and mapper behavior
- timer and interrupt ordering
- PPU and LCD timing
- DMA and access blocking
- APU sequencing
- boot-state and model differences
- CGB-specific features

For boot behavior, cover both real boot ROM execution and direct-boot presets when those modes exist.
Include coverage for explicit real-boot versus skip-boot modes, `FF50` handoff timing, boot-ROM overlay versus cartridge visibility, valid versus invalid logo/checksum outcomes, missing-cartridge or `0xFF` header behavior, and model-specific register state such as DMG versus MGB `A` at cartridge entry whenever suitable tests exist.
For direct-boot presets, include model-specific CPU state at `PC = 0x0100`, checksum-derived `F` on DMG/MGB, immediate I/O readback of the published post-boot snapshot, and continuity checks that the first timer, PPU, and APU ticks are coherent with that snapshot rather than restarting from zeroed hidden state.
Include explicit tests for unreliable post-boot state policy, such as WRAM, HRAM, cartridge RAM whether external or mapper-local, `OBP0`, and `OBP1`, without presenting those policy choices as proven hardware constants.
For PPU behavior, prioritize tests that expose dot timing, variable Mode 3 length, fetcher/FIFO correctness, STAT timing, and sprite interaction.
Include coverage for Mode 2 OAM blocking, OAM-order sprite selection, and the `10`-sprites-per-scanline limit when suitable tests exist.
Include sprite-selection tests that prove `Y` drives Mode 2 selection while `X` visibility does not prevent a sprite from consuming one of the `10` slots.
Include DMG OBJ/OBJ priority tests that distinguish selection priority from drawing priority and verify smaller `X` then earlier OAM order.
Include OBJ transparency tests that verify color index `0` is transparent rather than visible white output.
Include BG/OBJ mixing tests that verify the winning OBJ pixel is chosen before the BG-over-OBJ rule is applied.
Include DMG STAT quirk coverage and avoid assuming the same result on GBC-in-DMG-mode without validation.
Include coverage for Mode 3 startup cost, SCX-dependent timing, window-trigger timing, and sprite-induced stalls when suitable tests exist.
Include sprite-edge tests for top and bottom clipping, `8x8` versus `8x16`, and the `SCX & 7` plus `X = 0` timing-sensitive path when suitable tests exist.
Include mid-frame `LCDC.1` and `LCDC.2` toggle coverage when sprite fetch cancel and size-change behavior are implemented.
Include window tests that separate WY latch timing from WX trigger timing and verify WY is latched at Mode 2 start rather than recomputed continuously during the line.
Include tests for BG FIFO clear and fetcher restart when the window starts mid-scanline.
Include tests for the internal window line counter, including reset during VBlank and increment only on lines where window rendering really starts.
Include tests for `WX = 0`, `WX = 166`, and mid-frame `WX`/`WY`/`LCDC.5` glitches when suitable tests exist.
Include DMG tests that verify `LCDC.0 = 0` suppresses window rendering even if `LCDC.5 = 1`.
Include tests where window start or window glitches alter the BG/window stream seen by later sprite mixing without spuriously clearing OBJ FIFO state.
Include tests for live `STAT` readback composition, especially writable enable bits plus live mode and live coincidence bits.
Include tests that `LY` spans `0..=153`, including coincidence behavior during VBlank and across the `153 -> 0` transition.
Include tests that writing `LYC` reevaluates coincidence and the LCD STAT source immediately on the current dot.
Include tests for each LCD STAT mode-source enable path for Mode `0`, Mode `1`, and Mode `2`.
Include LCD STAT tests that verify one shared internal rising-edge source line, including STAT blocking when consecutive enabled sources keep that line high.
Include tests that Mode `3` never acts as a direct LCD STAT interrupt source.
Include tests where entering VBlank can request both VBlank and LCD STAT Mode `1` without collapsing them into one source.
Include DMG-family tests for the `STAT` write quirk in Mode `0`, Mode `1`, Mode `2`, coincidence-active cases, and a negative case for Mode `3`.
Include tests that the mode reported by `STAT` matches the same live state used by the bus for VRAM/OAM blocking decisions.
Include LCD off/on tests for `STAT.mode = 0`, release of ordinary LCD-mode VRAM/OAM restrictions, and re-enable without stale LCD STAT line or coincidence carry-over.
Include tests for `LCDC.7: 1 -> 0` causing immediate LCD/PPU disable, LCD-off white output, and release of ordinary VRAM/OAM mode restrictions.
Include tests for `LCDC.7: 0 -> 1` causing immediate internal PPU restart while visible output stays blank for the first full frame.
Include tests that LCD disable clears in-flight fetcher/FIFO/window/object state so re-enable does not resume a corrupted partial scanline.
Include tests that LCD-off accessibility and DMA-specific blocking still compose correctly instead of one silently erasing the other.
Include tests for one explicit LY policy at disable, during steady LCD-off state, and across LCD re-enable.
Include tests that mid-scanline `LCDC.7` writes take effect immediately rather than waiting for scanline or frame end.
Include DMG-family OAM corruption tests that distinguish ordinary Mode `2` OAM/`FEA0-FEFF` triggers from generic blocked OAM behavior in other modes.
Include tests that the current Mode `2` OAM row is exposed deterministically one row per `4` dots and that the first row remains immune to the basic corruption patterns.
Include tests for distinct read-corruption and write-corruption formulas and for the dedicated `read + inc/dec` versus `write + inc/dec` paths, including the previous-row mutation and copy behavior of the complex `read + inc/dec` case.
Include instruction-family tests for OAM corruption triggers covering `inc rr` / `dec rr`, `[hli]` / `[hld]`, `push` / `pop`, `call` / `ret` / `rst`, interrupt service, and executing from OAM.
Include model-gating tests where DMG-family models trigger the bug and CGB-family models do not.
For CPU execution behavior, include opcode fetch under boot-ROM/cartridge mapping, `imm8`/`imm16` fetch order, register-versus-`(HL)` timing differences, taken-versus-untaken conditional paths, stack byte order, CB-prefix double-fetch behavior, and instructions with internal no-bus steps whenever suitable tests exist.
For CPU interrupt-control behavior, include IE/IF register behavior, delayed `EI`, immediate `DI`, fixed interrupt priority, vector dispatch, `RETI`, `HALT` wake-up semantics, `HALT` bug activation/effect, and separate `STOP` coverage whenever suitable tests exist.
For joypad behavior, include `JOYP` mixed-register readback, high readback on bits `7-6`, active-low matrix semantics, separate button-row versus d-pad-row selection, simultaneous-row selection, visible `High -> Low` interrupt generation, repeated visible transitions, and the documented repo `STOP` wake policy whenever suitable tests exist.
For the current repo DMG-family baseline, treat that `STOP` wake policy as selection-independent button-press wake on the hardware-facing `8` buttons, while still keeping joypad-interrupt generation tied to visible `JOYP` low-nibble transitions.
For serial behavior, include `SB` / `SC` ownership and mixed-register semantics, forced-high DMG readback of the non-functional `SC` bits, DMG master-mode `8192` Hz transfer timing, slave-mode externally clocked progress, disconnected-peer `0xFF` reception, loopback or scripted-peer coverage, intermediate `SB` states during shifting, and serial IRQ request only on eighth-shift completion whenever suitable tests exist.
For timer behavior, include internal-counter-derived `DIV`, DIV-write glitches, TAC-write glitches, falling-edge TIMA increments, overflow-window behavior, separate TIMA/TMA write cases before/during/after reload, and timer interrupt timing through `IF` and CPU-visible servicing whenever suitable tests exist.
For bus behavior, include blocked-access cases, boot ROM remapping, next-fetch behavior after `FF50`, and DMA-related contention whenever suitable tests exist.
Include direct-boot routing checks that verify boot ROM is already unmapped, the ordinary cartridge ROM map is visible again across `0x0000-0x7FFF`, and DMG-mode reads of CGB-only registers return `0xFF` whenever suitable tests exist.
Include region-contract tests for fixed ROM, switchable ROM, VRAM, cartridge external space, WRAM, echo RAM, OAM, unusable space, MMIO, HRAM, and `IE`, including aliasing, blocked-access semantics, and ownership-by-device whenever suitable tests exist.
For cartridge loading and mapper selection, include tests for header parsing of the legacy/CGB title split around `0x0143`, plus `0x0143`, `0x0146`, `0x0147`, `0x0148`, and `0x0149`, explicit ROM-size versus file-size validation, structured unsupported-type diagnostics, and the `MBC2` internal-RAM special case whenever suitable tests exist.
Include cartridge-loader classification tests for `Supported`, `PlannedVariant`, `DocumentedButUnsupported`, `ExperimentalHeuristic`, `AccessorySpecialCase`, and `UnknownCode`, preserving the raw `0x0147` byte, detected cartridge name, category, and diagnostic reason.
Include tests that documented special cartridges such as `MBC30`, `MMM01`, `M161`, `HuC1`, `HuC-3`, `MBC6`, `MBC7`, `Pocket Camera`, and `Bandai TAMA5` are identified explicitly and do not silently fall back to nearby supported mappers.
Keep heuristic `EMS` / `Bung` / `Wisdom Tree` detection behind an explicit experimental loader policy in tests as well; strict-mode coverage should confirm that those heuristics stay disabled by default.
For the `No MBC` family, include explicit coverage for `0x00`, `0x08`, and `0x09`, linear `0x0000-0x7FFF` reads with no bank state, ignored `0x0000-0x7FFF` writes, `32 KiB` ROM validation, and explicit diagnostics when No MBC declares impossible ROM or RAM sizes.
Include tests that `0xA000-0xBFFF` distinguishes absent RAM from present linear `8 KiB` RAM and that battery only changes persistence expectations rather than the visible map.
Use No MBC as the first closed cartridge baseline for boot-overlay and post-boot routing checks so `FF50` handoff, `0x0100-0x014F` visibility, and ordinary cartridge reads are validated before mapper banking enters the picture.
For MBC1, include explicit coverage for header types `0x01`, `0x02`, and `0x03`, deterministic power-up state, RAM-enable decode, raw `5`-bit primary bank-register behavior, and the rule that the primary-register `0 -> 1` translation happens before final ROM-size masking rather than after it.
Include MBC1 tests for `0x4000-0x7FFF` bank selection across `32 KiB`, `64 KiB`, `128 KiB`, `256 KiB`, `512 KiB`, `1 MiB`, and `2 MiB`, including dedicated access cases for banks `0x01`, `0x1F`, `0x21`, `0x41`, and `0x61`, the documented small-ROM case where bank `0` can appear in the high region after masking, and the large-ROM anomaly where `0x20`, `0x40`, and `0x60` resolve as `0x21`, `0x41`, and `0x61` in the switchable region.
Include MBC1 tests for mode `0` versus mode `1`, low-region bank changes on large-ROM cartridges, fixed `8 KiB` RAM versus banked `32 KiB` RAM behavior, disabled-RAM open-bus reads plus ignored writes, immediate visibility of `0x0000-0x7FFF` mapper writes to later accesses on the shared T-cycle timeline, and explicit diagnostics for impossible ROM-size / RAM-size / wiring combinations.
For MBC2, include explicit coverage for header types `0x05` and `0x06`, deterministic power-up state, address-bit-`8` decode across `0x0000-0x3FFF`, raw `4`-bit ROM-bank-register behavior, and the documented `0 -> 1` translation for the switchable ROM window.
Include MBC2 tests for `0x4000-0x7FFF` bank selection across the supported ROM sizes, explicit `256 KiB` maximum validation, and clear diagnostics when an MBC2 image exceeds that ROM limit or declares inconsistent RAM metadata.
Include MBC2 tests for internal `512 x 4-bit` RAM, low-nibble write masking, the chosen high-nibble readback policy, low-`9`-bit echo aliasing between `0xA000-0xA1FF` and `0xA200-0xBFFF`, disabled-RAM open-bus reads plus ignored writes, immediate visibility of MBC2 control writes to later accesses on the shared T-cycle timeline, battery-backed persistence on `0x06`, and warning/error policy when `0x0149 != 0x00` without reinterpreting the cartridge as ordinary external SRAM.
For MBC3, include explicit coverage for header types `0x0F`, `0x10`, `0x11`, `0x12`, and `0x13`, deterministic power-up state, raw `7`-bit ROM-bank-register behavior, typed RAM-versus-reserved-versus-RTC selector handling, and the documented `0 -> 1` translation for the switchable ROM window.
Include MBC3 tests for `0x4000-0x7FFF` bank selection across supported ROM sizes up to `2 MiB`, ordinary access to banks `0x20`, `0x40`, and `0x60`, RAM banking across the accepted standard `2 KiB` / `8 KiB` / `32 KiB` SRAM declarations, and explicit reservation or diagnostics for MBC30-like `64 KiB` SRAM declarations.
Include MBC3 tests for RAM / RTC enable behavior, low-nibble decoding of the `0x4000-0x5FFF` selector, standard RAM-bank selectors `0x00..=0x03`, reserved selector values `0x04..=0x07`, the rule that reserved selectors do not alias ordinary SRAM banks, a first accepted latch on `0x00 -> 0x01`, the explicit zero-snapshot policy before that first valid latch, compatibility-covered follow-up non-zero relatches after a valid snapshot exists, live-versus-latched RTC state, visible-bit RTC register retention before time advancement, halt/carry/day-counter behavior, disabled-RAM / RTC policy, and powered-off elapsed-time handling through an injected deterministic clock.
When validating the MBC3 selector ambiguity specifically, retain at least one external oracle that exercises `0x04..=0x07` and repeated upper-bit variants such as `0x14..=0x27`; the current curated `cpp/rtc-invalid-banks-test.gb` case is the regression that keeps the project on explicit reserved-selector semantics despite the broader `$00-$07` wording in current `Pan Docs`.
For MBC5, include explicit coverage for header types `0x19`, `0x1A`, `0x1B`, `0x1C`, `0x1D`, and `0x1E`, deterministic power-up state, raw low-`8` plus high-`1` ROM-bank-register behavior, and the rule that bank `0` remains valid in `0x4000-0x7FFF`.
Include MBC5 tests for `0x4000-0x7FFF` bank selection across supported ROM sizes up to `8 MiB`, including bank `0`, bank `0x1FF`, the `0xFF -> 0x100` boundary, and real-size masking without any MBC1/MBC3-style `0 -> 1` translation.
Include MBC5 tests for RAM-enable behavior, disabled-RAM open-bus reads plus ignored writes, linear `8 KiB` bank selection for `8 KiB`, `32 KiB`, and `128 KiB` SRAM configurations, the absence of any MBC1-style dual banking mode, and the rule that header variants without RAM do not expose fake SRAM semantics merely because the RAM-bank register exists.
Include rumble-capable MBC5 tests that prove `bit 3` of the control register in `0x4000-0x5FFF` updates observable `rumble_on`, that the state remains active until software clears it, and that rumble handling stays distinct from effective RAM-bank selection rather than collapsing both meanings into one opaque integer.
Include MBC5 validation tests for ROM sizes above `8 MiB`, impossible RAM declarations, no-RAM header variants with nonzero `0x0149`, and the failure case where a rumble-capable header is loaded without exposing observable rumble state.
For cartridge persistence, include tests that the saved hardware-style payload is the complete cartridge backing store rather than the currently visible `0xA000-0xBFFF` window, including linear SRAM on `No MBC`, banked SRAM on `MBC1`, `MBC3`, and `MBC5`, plus nibble RAM on `MBC2`.
Include persistence tests that `0x0147` capability decoding, not filename heuristics or `0x0149` alone, decides whether a cartridge auto-produces hardware-style saves, and that cartridges with non-persistent RAM do not do so by default.
Include persistence tests that `ram_enabled` gating does not affect the saved payload contents and that disabled-but-existing cartridge RAM can still round-trip through persistence.
Include `MBC3` persistence tests that serialize live RTC state plus elapsed-time bookkeeping, restore powered-off advancement from an injected deterministic clock, and do not confuse the latched RTC snapshot with the persistent live clock.
Include contract-level tests for in-memory and disk save backends, format versioning, explicit load/save APIs, save-on-close, forced save, optional auto-flush-after-write behavior, and atomic file replacement when storage robustness is under test.

## Cartridge persistence versus full save-state coverage

- Cartridge-persistence tests validate cartridge-owned backing stores and RTC state only; they must not require CPU, PPU, APU, WRAM, or other console-state serialization.
- Full-emulator save-state tests validate whole-machine snapshot ownership, hidden temporal-state restore, and save/load continuation determinism under the recorded execution mode and overrides.

Keep hardware-style cartridge persistence tests separate from full-emulator save-state tests; the former must not require CPU, PPU, APU, WRAM, or other console-state serialization.
For DMA behavior, include `FF46` source-page selection, DMG echo-alias source behavior above `DFFF`, full `160`-byte copy correctness, the documented `640`-dot / `160`-M-cycle burst body, the current Mooneye-backed one-full-M-cycle post-`FF46` start seam before CPU OAM blocking begins, the distinct first-byte commit on elapsed T-cycle `8`, the corresponding `Completed` transition on elapsed T-cycle `648`, restart timing when `FF46` is written during an active burst, transfer-progress timing, source-bus-aware CPU blocking during DMA, `FF46` readback/restart accessibility during DMA, HRAM accessibility during DMA, and OAM/LCD interaction whenever suitable tests exist.
For APU behavior, include tests that `NR52` power-off clears ordinary audio registers, preserves wave RAM accessibility, and does not reset the `DIV-APU` source relationship whenever suitable tests exist.
Include tests that `DIV-APU` advances from the falling edge of `DIV` bit `4`, including `DIV`-write-induced extra ticks when the edge is produced.
Include tests that the frame sequencer clocks length, envelope, and CH1 sweep without becoming the waveform timer for the channels themselves.
Include tests that `dac_enabled` and `channel_active` stay distinct, that DAC-off forces channel-off, and that `NR52` reports active channels rather than DAC-enabled channels.
Include tests that `NRx4` trigger writes act immediately on the shared timeline and do not activate a channel whose DAC is off.
Include tests for `NR51` stereo routing, `NR50` master-volume semantics including the documented "0 behaves like factor 1" rule, and HPF/DC-offset-sensitive mixer state changes whenever suitable tests exist.
Include APU output-path tests that resolved channel digital outputs feed per-channel DAC conversion before stereo mixing, including the documented negative-slope `0..15 -> -1..1` enabled-DAC mapping.
Include tests that DAC-off output remains distinct from "inactive channel with DAC still enabled" rather than collapsing both cases into one fake digital-`0` path.
Include tests that `NR51` routing changes, `NR50` volume changes, and DAC enable changes affect the live analog mix immediately and generate the documented pop-producing DC-offset transitions through the HPF.
Include tests that the left/right HPF state is persistent and stateful across captured samples instead of acting like a memoryless host post-process.
Include tests that host-facing sample-rate or buffer-size changes do not alter core APU timing, mixer semantics, HPF behavior, or pop generation, and that the core can be validated without a real audio backend.
Include CH1 tests for `NR10`-`NR14` ownership, `NR13` write-only behavior, and immediate `NR14` trigger/length-enable semantics.
Include CH1 tests for period timer cadence, duty-step progression, retrigger-not-resetting-duty-step behavior, and period-write delay until the current sample ends.
Include CH1 tests for length expiry, envelope progression, and the rule that envelope volume reaching `0` does not disable the channel.
Include CH1 sweep tests for trigger-time shadow copy, immediate overflow check, timed writeback, second overflow check, and the rule that `NR13` / `NR14` writes do not update the sweep shadow automatically.
Include dedicated CH1 quirk tests for envelope/sweep timer-reload semantics where programmed pace or period `0` behaves as `8`, extra length clocking, low frequency-timer bits on trigger, and the first-duty-step-after-power-on path whenever suitable tests exist.
Include CH2 tests for `NR21`-`NR24` ownership, `NR23` write-only behavior, and immediate `NR24` trigger/length-enable semantics.
Include CH2 tests for period timer cadence, duty-step progression, retrigger-not-resetting-duty-step behavior, and period-write delay until the current sample ends.
Include CH2 tests for DAC-off behavior, length expiry, envelope progression, and the rule that envelope volume reaching `0` does not disable the channel.
Include dedicated CH2 quirk tests for envelope timer-reload semantics where programmed pace or period `0` behaves as `8`, extra length clocking, low frequency-timer bits on trigger, and the first-duty-step-after-power-on path whenever suitable tests exist.
Include CH3 tests for `NR30`-`NR34` ownership, `NR31`/`NR33` write-only behavior, and wave RAM persistence across `NR52` power-off.
Include CH3 tests for period timer cadence at one tick every `2` dots, `32`-sample index progression, buffered sample fetch from wave RAM, and period-write delay until after the next wave-RAM read.
Include CH3 tests for DAC-off behavior, trigger-not-refilling-the-sample-buffer behavior, length expiry, and `NR32` digital output-level semantics distinct from DAC-off or analog mixer volume.
Include dedicated CH3 quirk tests for digital-`0` startup state, skipped-first-sample / first-buffer behavior, active-channel wave-RAM access policy, trigger-with-length-0 behavior, and DMG-family retrigger corruption keyed both to the exact byte-read position and to the affected aligned source block whenever suitable tests exist.
Include CH4 tests for `NR41`-`NR44` ownership, `NR41` write-only behavior, and immediate `NR44` trigger/length-enable semantics.
Include CH4 tests for `noise_timer` cadence, LFSR progression, and decoded `NR43` behavior including divider `0 -> 0.5`, live width-mode changes, and clock-shift `14` / `15` suppressing LFSR clocks.
Include CH4 tests for DAC-off behavior, trigger-time reset of envelope/LFSR/timer state, length expiry, envelope progression, and the rule that envelope volume reaching `0` does not disable the channel.
Include dedicated CH4 quirk tests for ordinary `15`-bit mode, ordinary `7`-bit mode, lock-up on `15 -> 7` transitions in the documented all-ones states, retrigger recovery from lock-up, and extra length clocking whenever suitable tests exist.

## Recommended external validation sources

- GBEmulatorShootout `testroms`
- blargg test ROMs
- Mooneye tests
- dmg-acid2 / cgb-acid2
- mealybug-tearoom-tests
- SameSuite
- GB Accuracy Tests
- 144p Test Suite
- MBC3 RTC test ROMs

## Behavioral cross-check policy

When a change affects observable timing or ordering:

- compare against SameBoy when possible
- compare against another trusted oracle when that helps isolate behavior
- record intentional deviations and their reason

## Determinism policy

- Core execution should be deterministic for the same inputs, model configuration, execution mode, and explicit override set.
- Tests should prefer reproducible stepping and explicit expected state over fuzzy assertions.
- Instrumentation should not change hardware-visible behavior.
- Battery-backed RTC persistence tests must use an injected or otherwise explicit time source rather than the host wall clock.
- Determinism coverage should include replay from the same ROM plus input stream, save/load determinism, and at least some longer-running soak cases.
- "Same ROM + same execution mode + same explicit overrides + same input stream + same injected time source => same result" is the intended project contract.
- Save/load determinism should prove that saving, restoring, and continuing produces the same result as uninterrupted execution under the same recorded execution mode.
- Save states and replay logs should record the execution mode and active overrides that produced them.
- Restoring or replaying under a different execution mode should be rejected by default; if a later explicit developer conversion path is added, tests should cover that path separately and mark it as non-oracle.
- Soak coverage should include at least one real game, one longer-running test ROM, and one or two cases with APU activity plus banked cartridges.

## Regression policy

- Every fixed bug should leave behind at least one permanent regression asset.
- Use a focused unit or integration test when the bug is local to one subsystem or one cross-subsystem interaction.
- Use a ROM-based reproduction case when the bug is systemic or easiest to demonstrate through an external suite.
- Use a stored differential case when the bug was discovered by comparison against SameBoy or another explicit oracle.
- Keep regression organization by subsystem or hardware area so repeated failures do not disappear into one catch-all bucket.
- Differential regressions should preserve enough reproduction context to rerun them quickly, including the ROM, execution mode, active overrides, input stream, injected seed or time source when relevant, first divergence point, and an optional snapshot when that reduces debug time.

## Severity and DMG closure policy

- Classify failures by closure impact instead of treating all red tests as equally important.
- Treat scheduler ordering, CPU / interrupts, timer, PPU timing, DMA, primary cartridge families, basic joypad / serial behavior, and save/load determinism as `must-pass` DMG closure areas.
- Treat finer APU behavior, serial edge cases beyond the baseline path, and RTC-specific long-tail work as high-importance compatibility items unless the roadmap explicitly promotes them to `must-pass`.
- Keep optional tooling polish, special-cartridge heuristics, and experimental paths outside the `must-pass` gate unless they are needed to explain or reproduce a blocking core bug.
- Do not declare DMG closed while `NoMbc`, `Mbc1`, `Mbc2`, `Mbc3`, or `Mbc5` still have open severe correctness bugs, or while CPU / interrupts, timer, PPU, DMA, basic joypad / serial, or save/load determinism coverage is still failing.
- Keep a project-visible DMG closure checklist that includes internal core suites, minimum external suites, differential comparison, determinism, save/load determinism, and primary cartridge-family status.

## CI stratification policy

- The regular CI path should always run critical unit tests, critical short integration tests, a stable subset of external ROMs, and save/load determinism coverage under `Strict`.
- Coverage thresholds enforced with `cargo-llvm-cov --fail-under-*` must be checked per repo-gated crate, not as one aggregated multi-crate report, so a strong crate cannot hide a weaker one behind a shared total.
- New workspace crates should join that per-crate gate in the same change that
  introduces them unless a documented blocker exists; if the crate cannot meet
  the intended `90/90/90` floor yet, record the temporary lower floor
  explicitly in this file and in `docs/ROADMAP.md` rather than leaving the crate
  outside `make ci`.
- For the current repo-owned coverage gate, prefer one clean instrumented workspace run with `cargo llvm-cov --workspace --no-report`, then evaluate the gated crates separately with `cargo llvm-cov report -p <crate> --fail-under-*` so the signal stays per-crate without paying for repeated test execution. This keeps `make ci` from re-running the same tests once via `cargo test` and again via coverage while still enforcing floors for the full current workspace surface. The current temporary per-crate floors are `gb-core` `96.00/95.95/98.43`, `gb-test-runner` `84.22/84.63/78.87`, `gb-persistence` `96.66/91.93/94.68`, `gb-cli` `92.52/90.01/92.30`, and `gb-desktop` `57.14/60.32/61.22` for lines/regions/functions.
- Experimental suites may exist in nightly or manual jobs, but they must publish artifacts separately and must not gate or dilute the official strict-mode closure signal.
- Longer differential runs, soak tests, and broader external ROM inventories may live in nightly or manual suites, but they must remain documented and runnable.
- When external ROMs are part of CI, the workflow should fetch them through the same repo-managed manifest-driven path used locally instead of embedding one-off download logic per job.
- Failure artifacts should include enough information to debug without rerunning blindly, such as logs, optional snapshots, framebuffer output when relevant, and a diff against the reference output when one exists.

## Test organization policy

- Prefer local module tests for unit-level coverage.
- Use top-level `tests/` for integration coverage only.
- When module tests outgrow an inline `tests` block, move them to a co-located test facade such as `foo/tests.rs`.

## Bug traceability policy

- Bug fixes should keep a reproducible description: ROM or test case, observed behavior, and expected behavior.
- For CPU, PPU, timer, interrupts, DMA, memory map, and boot behavior, prefer writing the failing test first when practical.

## Boot and startup policy

- When direct-boot presets are used in tests, document the assumed register and memory state explicitly.
- Document separately which parts of the direct-boot preset are deterministic, cartridge-derived, unreliable by hardware, or synthesized hidden state needed for temporal continuity.
- Keep tests that exercise real boot ROM execution separate from tests that start after boot.
