# Testing

## Testing strategy

Use multiple layers:

- focused unit tests
- subsystem integration tests
- ROM-based validation
- oracle comparisons where useful
- determinism, replay, and regression-retention coverage

## Authority and scope

- This document owns project-wide validation policy and cross-subsystem testing expectations.
- Detailed subsystem-specific checklists remain owned by the matching `docs/hardware/*.md` handbook.
- Detailed external ROM suite operator workflows, current suite membership, fetch commands, differential-oracle commands, and commercial-ROM manifest examples live in `docs/testing/ROM-SUITES.md`; this file should stay focused on validation policy.
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
- `Layer E`: determinism, replay, save/load determinism, and regression-retention coverage over accepted closure lanes.
- For this T-cycle-based core, the matrix should support several validation granularities: end of test, end of instruction, and short per-T-cycle windows when a timing-sensitive divergence needs to be isolated.
- Every corrected bug should end up attached to at least one permanent layer in this matrix.

## Phase 9 DMG closure checklist

Phase `9.5` closes the practical DMG hardening scope with the evidence below. This checklist records the accepted closure signal and intentionally does not create follow-up backlog for cartridge long-tail, external Joypad oracle discovery, exhaustive subsystem differential lanes, or soak expansion when no active severe correctness bug depends on them.

- `CPU`: repo gate present. Current evidence: Phase `2` synthetic timing ROMs plus curated external Blargg DMG individual ROM coverage.
- `Interrupts`: repo gate present. Current evidence: Phase `2` interrupt timing ROMs, Blargg `halt_bug`, and the interrupt-heavy `cpu_instrs` cases.
- `Timer`: repo gate present. Current evidence: Phase `2` timer and interrupt-service synthetic ROMs, unit coverage in `gb-core`, and the Mooneye acceptance timer cases in `mooneye-acceptance-dmg-curated`.
- `Bus`: repo gate present. Current evidence: bus/DMA/CPU integration coverage plus external Blargg `mem_timing`, `mem_timing-2`, and their individual ROMs.
- `DMA`: repo gate present. Current evidence: closed Phase `3` unit and integration coverage plus Mooneye OAM DMA acceptance cases in `mooneye-acceptance-dmg-curated`.
- `APU`: repo gate present. Current evidence: repo-local APU MMIO/power-readback coverage, repo-local unit and machine integration coverage for the explicit `DAC -> NR51 -> NR50 -> HPF` output path and the current non-intrusive post-HPF snapshot capture boundary, plus the full curated `blargg-dmg-curated` family promoted into the repo-gated external DMG block, including Blargg `dmg_sound 01..12`.
- `PPU`: repo gate present for the closed OAM-corruption and framebuffer-oracle slices. Current evidence: Phase `4` synthetic OAM-corruption ROMs, curated Blargg `oam_bug` singles `1..6,8`, a repo-gated `dmg-acid2` framebuffer oracle, the curated `acid/which.gb` informational DMG execution lane, the workflow-managed `mealybug-tearoom-tests` framebuffer oracle slice locally revalidated green on April 27, 2026, `mealybug-tearoom-dmg-sameboy-differential` locally matched against LibSameBoy case-bundle artifacts on April 27, 2026 for rows where SameBoy is a passing GBEmulatorShootout oracle, the nine SameBoy-non-PASS Mealybug rows documented as a SameBoy oracle limitation rather than gb-cycle regressions, and `hacktix-dmg-curated` locally matched against LibSameBoy case-bundle artifacts on April 28, 2026.
- `Cartridge`: repo gate present for the primary mapper-oracle slice. Current evidence: unit and integration coverage for `NoMbc`, `Mbc1`, `Mbc2`, `Mbc3`, `Mbc5`, hardware-style persistence, the built-in `phase-6-cartridge-oracle` suite, LibSameBoy `case-bundle` differential artifacts locally materialized and compared green on April 27, 2026, and the Phase `9` save/load determinism smoke covering that suite.
- `Joypad`: internal gate accepted for Phase `9` closure. Current evidence: closed Phase `5` synthetic coverage and subsystem tests.
- `Serial`: repo gate present. Current evidence: closed Phase `5` synthetic coverage, subsystem tests, and the Mooneye acceptance serial alignment case in `mooneye-acceptance-dmg-curated`.

This checklist should change only when a discovered regression invalidates one of the accepted closure signals above or when new completed evidence replaces an existing accepted signal.

## External ROM harness policy

- Test ROM execution must be automatable; manual inspection of the LCD is an auxiliary debugging aid, not the primary acceptance path.
- When working on already-known external ROM failures, timing regressions, or exploratory PPU/MMIO fixes, always preserve a baseline snapshot of `/.roms/test/test-report.md` before making the validation run, preserve the final report again after the run, and compare the two before deciding whether the iteration is worth keeping.
- That baseline/final comparison is mandatory for go/no-go decisions on exploratory ROM-driven work. Save the raw markdown report first, typically as explicit `test-report-before.md` and `test-report-after.md` artifacts. Do not require rendered images of the report unless a task explicitly asks for them.
- If the current tree is not already a clean baseline, compare against a clean reference such as `main` in a separate branch or worktree rather than hand-waving the previous state from memory.
- Do not summarize a ROM-driven iteration as "no regressions" unless the before/after report comparison has been done explicitly and the changed rows have been named.
- The harness should support at least framebuffer capture and serial / link-port capture when the ROM exposes machine-readable output there.
- When an external suite prints self-validating text through a documented screen-console protocol, the harness should also support a typed text-extraction path for that protocol rather than falling back to a circular framebuffer fixture generated by this project.
- Prefer serial / link-port capture for suites such as Blargg `cpu_instrs` when that path is available, because it avoids treating a scrolling framebuffer as the primary machine-readable result channel.
- For the current Blargg lane, the harness supports three machine-readable channels: serial, cartridge-RAM text/status output, and the upstream `console.s` BG-map text console used by screen-driven ROMs such as `halt_bug`.
- The curated `blargg/halt_bug.gb` case currently runs under `Permissive` in the harness, not because the execution model itself is non-oracle, but because the shipped header declares `MBC1+RAM` with `0x0149 = 0x00`; the project keeps `MBC1` validation strict in `gb-core` and confines that legacy-header compatibility escape hatch to the ROM manifest.
- Before promoting a third-party ROM into the built-in automated catalog, verify whether it is listed in `GBEmulatorShootout` and record any explicit exception instead of assuming the ROM belongs in the default curated set.
- A ROM omitted from `GBEmulatorShootout` may still be useful for exploratory debugging, but it must stay out of the repo-managed built-in suites until the reason for including it anyway is documented explicitly.
- Each ROM case should define a timeout, an explicit pass/fail rule, and retained failure artifacts such as serial output, framebuffer output, trace excerpts, and optional snapshots.
- For the current curated Mooneye DMG acceptance slice, retain at least snapshot plus serial artifacts on failure. Many cases still use the register-signature oracle for pass/fail, but keeping serial alongside the snapshot shortens diagnosis when a ROM emits a more specific failure reason before falling into the common Mooneye stop loop.
- The workflow-managed `daid` DMG slice currently mixes ordinary framebuffer fixtures, one multi-fixture framebuffer oracle for `ppu_scanline_bgp.gb`, and one informational framebuffer capture for `rom_and_ram.gb`. The informational `rom_and_ram.gb` case currently runs under `Permissive` because its `NoMbc` header uses a legacy RAM-size declaration that the project treats as a warning outside `Strict`; keep that status visible instead of counting it as a normal strict-mode pass.
- When a ROM needs deterministic host-side interaction, the typed case metadata should also carry the external stimulus schedule explicitly instead of burying that behavior in ad hoc test-only closures.
- `gb-test-runner` owns typed ROM-case and suite metadata including console model, startup mode, execution mode, emulation-progress timeout, explicit pass/fail rule, external stimulus schedule when needed, requested captures, and retained failure-artifact policy.
- `gb-test-runner` is also the executable harness for these suites: it can load typed suites, run ROMs on the shared T-cycle machine, capture serial / framebuffer / snapshot artifacts, and preserve failure outputs without relying on a frontend.
- Typed ROM-case metadata may also carry deterministic startup memory writes when a curated oracle depends on one explicit post-boot memory artifact that the current `SkipBoot` baseline does not synthesize yet. Keep that path narrow, document the provenance of the bytes, and prefer boot-derived state such as the DMG trademark tile or logo VRAM/map bytes over ad hoc framebuffer patching. This currently covers curated mealybug cases that intentionally reuse tile `0x19` from the DMG boot ROM instead of uploading their own tile data, plus the curated `hacktix/bully.gb` DMG case that checks the boot-derived logo VRAM seed under `SkipBoot`.
- When a typed suite is landed before its redistributable assets, reserve the exact ROM and trace filenames in the repo with per-phase README stubs so later automation and oracle work reuse one stable target contract instead of inventing new names ad hoc.
- Repo-managed external ROM assets should keep only one persistent workspace-local gitignored layer: the curated runnable store under `/.roms/test/`. Any raw upstream checkout used to materialize that store should be temporary and cleaned up after the fetch command completes.
- The external-ROM fetch workflow must run all git commands from inside its temporary checkout or fixture repository and must not rewrite the invoking repo's local git config while doing so. Test-only commit identity should be supplied through per-command environment variables rather than persisted `user.name` / `user.email` entries.
- The curated fetch command should support both full-store materialization and explicit family subsets so repo-gated and exploratory `make test-*` targets can remain autosufficient without forcing unrelated families to be fetched first.
- The upstream source, pinned revision, and required-file hashes belong in the versioned manifest `crates/gb-test-runner/data/sources.toml`.
- The runnable curated families belong under `/.roms/test/<family>/`, using one checked-in manifest per family under `crates/gb-test-runner/data/*.toml` so supported ROMs can be added or commented explicitly without editing runner code.
- `gb-test-runner` may accept explicit environment-variable roots for curated suites, but the default automation path should also resolve the repo-managed curated store automatically so developers and CI do not need ad hoc local clones or handwritten path setup.
- Curated family runs should update `/.roms/test/test-report.md` with a simple per-ROM status table so repo-managed `PASS` / `FAIL` / `INFO` state stays visible without re-reading logs; the markdown view should render those states as `✅`, `❌`, and `ℹ️` rather than repeating the raw persisted strings.
- The report header should include a `non-failing/total` summary for the exact set of persisted rows currently rendered in that markdown file, counting both `PASS` and `INFO` in the numerator, so a first partial run reports only its own rows while later partial updates keep counting the broader persisted context already present in the report.
- When multiple curated families are present in the report, they should render in the fixed order `acid`, `blargg`, `daid`, `ax6`, `mooneye`, `samesuite`, `hacktix`, `cpp`, `mealybug-tearoom-tests`; families with no persisted cases should not appear at all.
- Within each populated family, rows should follow the curated family manifest order instead of being alphabetized by ROM filename.
- Keep redistributable external test ROMs and non-redistributable commercial ROMs in separate stores. The current local-only commercial bucket is `/.roms/local-commercial/`, and it must remain outside CI, docs about official closure, and public automation targets.
- For ad hoc local commercial-ROM bring-up that still needs deterministic host input, prefer `gb-test-runner --manifest <path>` over growing `gb-cli` into a second harness. That manifest path should carry the typed case contract directly, including model, startup mode, timeout, oracle, and any scheduled joypad stimuli.
- Keep local boot ROM images under the repo-managed gitignored `/.roms/bootrom/` store, using the canonical filenames from `gb-core` (`dmg0_boot.bin`, `dmg_boot.bin`, `mgb_boot.bin`) so local real-boot runs do not depend on ad hoc per-machine paths.
- For the current DMG-family store, `gb-test-runner` treats those boot ROM assets as pinned local inputs rather than arbitrary filenames: strict-mode `RealBoot` verifies the observed SHA-256 against the expected `dmg0/dmg/mgb` hashes before execution so local bring-up does not silently proceed on the wrong firmware bytes.
- For ad hoc local `gb-cli` bring-up with `RealBoot`, keep the default no-limit behavior tied to the actual boot path instead of reusing the `SkipBoot` budget blindly: the CLI should treat boot-ROM handoff as the semantic start of the post-boot run window, while still retaining a finite safety cap when `FF50` never unmapped.
- Keep the canonical repo-local ignored DMG-family real-boot regression matrix in `crates/gb-test-runner/tests/external.rs`, with verified `dmg0`, `dmg`, and `mgb` valid-handoff cases plus DMG invalid-logo, invalid-checksum, and FF-filled-header no-handoff cases. The valid path should assert the real `FF50` handoff, compare the resulting cartridge-entry state against the centralized direct-boot snapshot (`BootController::direct_boot_state()`) for the same model, and emit one exact serial fingerprint artifact so failures leave a legible entry-state record without requiring firmware dumps in CI.
- Keep imported differential oracle artifacts under the repo-managed gitignored `/.oracles/<oracle>/<layout>/` tree instead of scattering them under `/tmp`, so repeated validation runs have one visible workspace-local location.
- For ad hoc local framebuffer inspection outside `gb-test-runner`, keep `gb-cli --framebuffer-out` extension-driven: a `.png` target should emit a real grayscale PNG for human viewing, while non-`.png` targets may stay on the lighter raw-PGM path used for low-friction local artifacts.
- For manifest-driven local `gb-test-runner` cases that capture framebuffer, export the resulting PNG beside the ROM using the ROM stem so one-off local commercial bring-up does not require a separate artifact-root convention.
- The minimum DMG closure baseline should include automated CPU / interrupt coverage through curated Blargg DMG automation sourced from `GBEmulatorShootout`, curated Acid DMG coverage for basic PPU validation, and `mealybug-tearoom-tests` for fine PPU rendering / timing validation.
- `gb-test-runner` should expose a human-readable catalog of the built-in suites and their active oracle channels. The current CLI entry point is `cargo run -p gb-test-runner --bin run_rom_suite -- --list-detailed`.
- The same harness should also expose the current hardening checklist so the repo can answer "what is externally gated already and what is still internal only?" without re-reading the docs. The current CLI entry point is `cargo run -p gb-test-runner --bin run_rom_suite -- --early-checklist`.
- The current PPU hardening lane also includes the curated Acid DMG family under `cargo run -p gb-test-runner --bin run_rom_suite -- --suite acid-dmg-curated`.
- That family currently mixes one repo-gated framebuffer-oracle case `dmg-acid2.gb` with one non-blocking informational framebuffer case `which.gb`, mirroring the `GBEmulatorShootout` classification rather than forcing a synthetic pass/fail oracle where upstream does not define one.
- The current PPU hardening lane also includes one workflow-managed framebuffer suite for `mealybug-tearoom-tests` under `cargo run -p gb-test-runner --bin run_rom_suite -- --suite mealybug-tearoom-dmg-curated [--failure-artifact-root <dir>]`. This suite uses a curated DMG-only subset sourced from `GBEmulatorShootout` and the same committed-PNG oracle contract as `dmg-acid2`. It is exercised by `make run-mealybug`, the local `make test-roms` aggregator, and the GitHub `test-roms` workflow; if a future exploratory case in this family turns red, demote or split that case before claiming the workflow-managed suite is green.
- The current DMG framebuffer lane also includes one workflow-managed `hacktix` suite under `cargo run -p gb-test-runner --bin run_rom_suite -- --suite hacktix-dmg-curated [--failure-artifact-root <dir>]`. This suite currently tracks the `GBEmulatorShootout` `hacktix` subset `bully.gb` plus `strikethrough.gb`, runs those ROMs on the default DMG model, and uses the same committed-PNG framebuffer-oracle contract as the other screenshot-based curated families. It is exercised by `make run-hacktix`, the local `make test-roms` aggregator, and the GitHub `test-roms` workflow.
- The current DMG `mooneye` lane includes one workflow-managed suite under `cargo run -p gb-test-runner --bin run_rom_suite -- --suite mooneye-acceptance-dmg-curated [--failure-artifact-root <dir>]`.
- That suite follows the active `GBEmulatorShootout` `testroms/mooneye.py` DMG list rather than inventing a local file list: it keeps the upstream `acceptance/*` entries plus the DMG `emulator-only/mbc1/*`, `emulator-only/mbc2/*`, `emulator-only/mbc5/*`, and `manual-only/sprite_priority.gb` cases that appear before the CGB-only `misc/*` block, and it runs those ROMs on the default DMG model.
- Most Mooneye cases use the upstream `mooneye` pass/fail breakpoint protocol via the documented register signature at `LD B,B`; the single `manual-only/sprite_priority.gb` exception instead uses the committed framebuffer fixture `crates/gb-test-runner/data/fixtures/mooneye/sprite_priority.dmg.png`, matching the upstream manual-test classification and the reference PNG shipped by `GBEmulatorShootout`.
- Because the runner samples once per T-cycle, treat the immediate post-breakpoint `nop; jr -3` halt loop as the same terminal condition when those registers still match the documented pass/fail signature.
- The Mooneye lane is exercised by `make run-mooneye`, the local `make test-roms` aggregator, and the GitHub `test-roms` workflow; keep it documented as broad hardening evidence for the accepted Phase `9` closure matrix.
- The current Phase `9.3` closure lane includes one imported-oracle end-of-test differential path under `cargo run -p gb-test-runner --bin run_differential -- --oracle sameboy [--oracle-layout <case-bundle|sameboy-tester>] [--oracle-artifact-root <dir>] --suite <suite-name>`. This path enforces `Strict`, compares the suite's required-capture artifact against an imported oracle artifact bundle, and archives local context on divergence; it reports the first differing byte or pixel inside the compared final artifact. When `--oracle-artifact-root` is omitted, the repo-local default is `/.oracles/<oracle>/<layout>/`.
- The current built-in cartridge lane for that differential path is `phase-6-cartridge-oracle`, which reuses the retained synthetic Phase `6` `MBC1`, `MBC2`, `MBC3`, and `MBC5` ROM fixtures under a stable `TestSubsystem::Cartridge` suite contract. That lane uses the `case-bundle` layout because the `MBC3` case needs explicit pre-run RTC advancement and the relevant compared artifact is a portable `serial_hex.txt` payload.
- The current built-in Mealybug lane for the SameBoy framebuffer differential is `mealybug-tearoom-dmg-sameboy-differential`, not the full `mealybug-tearoom-dmg-curated` local gate. This split is intentional: GBEmulatorShootout updated on March 22, 2026 marks SameBoy as `FAIL` for `m3_lcdc_bg_en_change.gb`, `m3_lcdc_bg_map_change.gb`, `m3_lcdc_obj_size_change.gb`, `m3_lcdc_obj_size_change_scx.gb`, `m3_lcdc_tile_sel_change.gb`, `m3_lcdc_tile_sel_win_change.gb`, `m3_lcdc_win_en_change_multiple_wx.gb`, `m3_lcdc_win_map_change.gb`, and `m3_scy_change.gb`; for Phase `9.3`, those passing gb-cycle fixture cases are treated as a documented SameBoy oracle limitation rather than active gb-cycle blockers.
- The repo now also includes a companion LibSameBoy `case-bundle` materialization command under `cargo run -p gb-test-runner --bin run_sameboy_case_bundle -- --suite <suite-name> [--oracle-root <dir>] [--sameboy-root <dir> | --runner-binary <path>] [--build-if-missing]`. This command executes the selected suite through a small `libsameboy`-backed helper, applies suite startup memory writes, writes portable artifacts such as `/.oracles/sameboy/case-bundle/<case-id>/serial_hex.txt` or `/.oracles/sameboy/case-bundle/<case-id>/framebuffer.pgm`, and is the intended oracle-materialization path for Phase `9` cartridge and framebuffer differentials.
- The repo now includes a companion first-divergence probe command under `cargo run -p gb-test-runner --bin run_first_divergence -- --oracle sameboy --suite <suite-name> [--case <case-id>] [--probe-interval-tcycles <n>] [--compare-mode <framebuffer|state>] [--allow-divergence] [--build-if-missing]`. This command runs the local core and the same LibSameBoy helper to emit JSONL probe streams under `/.oracles/sameboy/first-divergence/<case-id>/`, compares normalized framebuffer hashes by default, and stores CPU registers, timer/IRQ registers, PPU timing/register values, raw VRAM/OAM/WRAM/HRAM hashes, and serial output as context for the first mismatching window; `--compare-mode state` expands comparison to the captured state fields, while `--allow-divergence` is for exploratory localization targets that should report a known window without failing the Make target.
- The repo still includes the SameBoy Tester wrapper under `cargo run -p gb-test-runner --bin run_sameboy_tester -- --suite <suite-name> [--oracle-root <dir>] [--sameboy-root <dir> | --tester-binary <path>]` for compatibility with existing `sameboy-tester` artifact layouts, but the Phase `9` Make targets use the LibSameBoy case-bundle path so serial and framebuffer oracle generation share one controlled helper.
- The accepted Phase `9.4` determinism lane is `cargo run -p gb-test-runner --bin run_determinism -- --suite <suite-name> [--case <case-id>] [--save-at-tcycles <n>] [--continuation-tcycles <n>]`. It runs two independent replays, compares final in-memory save states plus serial output, restores from a mid-run `MachineSaveState`, verifies the continuation against uninterrupted execution, and checks that a mismatched console-model restore is rejected. Only `Strict` cases count as passing determinism evidence; non-`Strict` cases fail fast.
- Makefile Phase `9` helpers keep repeated local validation stable: `phase9-determinism-smoke`, `phase9-determinism-local`, `phase9-sameboy-cartridge-oracles`, `phase9-diff-cartridge`, `phase9-sameboy-acid-oracles`, `phase9-diff-acid`, `phase9-sameboy-mealybug-oracles`, `phase9-diff-mealybug`, `phase9-sameboy-hacktix-oracles`, `phase9-diff-hacktix`, and `phase9-first-divergence-hacktix`.

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

## Linked-session validation policy

- Linked-session behavior should be validated through `gb-test-runner` manifests and linked-session fixtures rather than through desktop presentation loops.
- Linked manifests should name participants and topology explicitly, including `DMG-04` cable sessions and `DMG-07` adapter-port assignment, instead of inferring identities from vector position.
- Prefer participant-scoped oracles such as per-participant `serial_hex`, snapshots, or trace fixtures when they express the contract more clearly than a large whole-session fixture.
- Retained linked-session artifacts should make the failing participant, topology, and expected contract obvious.
- Desktop tests for linked sessions should cover topology construction, input routing, slot mapping, audio/view policy, and menu behavior; they must not redefine serial, cable, adapter, or shared-T-cycle hardware rules.

## New-code baseline

- New production code should normally introduce automated unit tests or integration tests in the same change.
- Prefer unit tests for local logic and integration tests when the behavior only becomes meaningful across subsystem boundaries.
- Treat "code first, tests later" as an exception that must be justified explicitly, not as the default workflow.
- Before opening or updating a pull request, run at least `make ci` locally so formatting, clippy, workspace tests, typos, and `cargo deny` do not first surface in CI.
- When a change touches CI, coverage, dependency policy, repo tooling, or other workflow-critical infrastructure, run `make test-roms` and `make coverage` locally as well before the PR is updated.
- The GitHub `ci` workflow is intentionally limited to formatting, linting, tests, typos, dependency policy, and the coverage gate. Keep external ROM execution out of that workflow.
- In that workflow, prefer one instrumented workspace `cargo llvm-cov --workspace --no-report` run plus per-crate `cargo llvm-cov report -p <crate> --fail-under-*` gates instead of a separate `cargo test --workspace` pass followed by coverage; the workspace tests should be paid for once.
- Repo-owned binary integration tests should resolve sibling executables from the active Cargo target directory rather than assuming only `target/debug` or runtime `CARGO_BIN_EXE_*` exports; coverage runs may build those binaries under an alternate root such as `target/llvm-cov-target/debug`.
- The GitHub `test-roms` workflow currently runs the same workflow-managed non-CGB DMG families as the local `make test-roms` aggregator: `acid`, the full Blargg DMG family (`cpu_instrs 01..11`, `halt_bug`, `instr_timing`, `mem_timing 01..03`, `mem_timing-2 01..03`, `oam_bug 1..6,8`, and `dmg_sound 01..12`), `daid`, `hacktix`, `cpp`, `mooneye`, and `mealybug-tearoom-tests`.
- Keep multi-ROM bundles, CGB-only suites, and still-red exploratory ROMs out of the default external-ROM workflow until they are green and intentionally promoted.
- When a new workspace crate is created, add it to the repo-owned coverage gate immediately: wire a dedicated `cargo llvm-cov report -p <crate> --fail-under-*` alias into `.cargo/config.toml`, add that alias to the `coverage-check` target in `Makefile` so `make ci` exercises it, and default the new crate to at least `90+/90+/90+` for lines/regions/functions.
- For the current infrastructure-heavy stage, keep the repo-owned coverage gate per crate rather than aggregated across crates. The project requires at least `90+/90+/90+` line/region/function coverage for every repo-gated crate, but the authoritative per-crate thresholds are the `cargo cov-check-*` aliases in `.cargo/config.toml`; do not duplicate the concrete percentages in docs. Existing configured thresholds must never be reduced from their current `.cargo/config.toml` values. Raise them when coverage improves, and do not satisfy the gate with hollow tests that only exercise trivial getters or app placeholders.
- When immediate automated coverage is temporarily impractical, record the missing test coverage, the reason it is deferred, and the remaining risk in the change report; add a `docs/TODO.md` entry if the gap is concrete and non-trivial, and touch roadmap docs only when the phase scope or sequencing changes.
- ROM-based validation and oracle comparison complement automated tests; they do not replace the expectation that new code should usually leave behind unit or integration coverage.

## Current test layout and fixture ownership

- Keep subsystem-local invariant tests close to the production code under `crates/gb-core/src/**`; when an inline test block becomes too large, move it to a co-located module such as `foo/tests.rs` rather than to an unrelated catch-all file.
- Keep public-API and cross-module smoke coverage under `crates/gb-core/tests/*.rs`, with shared integration-test helpers under `crates/gb-core/tests/common/`.
- Keep core-owned synthetic ROM fixtures and golden traces under `crates/gb-core/tests/fixtures/roms/` and `crates/gb-core/tests/fixtures/traces/`; reserve `crates/gb-test-runner/data/**` for runner-owned manifests, external-suite fixtures, linked-session fixtures, and committed oracle artifacts.
- Keep runnable redistributable external ROMs in the gitignored `/.roms/test/` store and imported differential oracle artifacts in the gitignored `/.oracles/<oracle>/<layout>/` tree; do not scatter long-lived validation assets under `/tmp`.
- Keep `ConsoleModel`, `StartupMode`, `ExecutionMode`, and `CompatibilityPolicy` defaults covered explicitly so DMG-first behavior and future CGB seams cannot drift silently.
- Keep `SchedulerPhase` order, the global T-cycle counter, `CycleContext` reset semantics, and the single top-level `Machine::step_t_cycle()` boundary covered so subsystem work cannot introduce unsynchronized stepping APIs.
- Keep subsystem-boundary wiring covered at the `Machine` level so CPU, bus, PPU, DMA, timer, boot, cartridge, APU, joypad, serial, and external-port ownership remains visible to tests.
- Keep typed breakpoint and watchpoint contracts for `PC`, memory, MMIO, and cartridge-visible state covered through debugger-facing tests, and keep debugger snapshots as inspection artifacts separate from whole-machine save states.
- Keep core validation fast, deterministic, and frontend-independent; `gb-core` tests must not require `gb-cli`, desktop, web, SDL, host audio, file dialogs, or other host-specific I/O.

## ROM-based validation policy

Map every ROM case to the subsystem and acceptance channel it validates, and keep the detailed edge-case checklist in the owning hardware handbook rather than duplicating it here.

- CPU and instruction behavior should prefer serial, text-console, register-signature, or snapshot oracles that can identify the failing instruction path without relying on manual framebuffer inspection.
- Timer, interrupt, bus, DMA, and scheduler tests should name the ordering contract being validated and should retain enough trace or snapshot context to isolate a first T-cycle or instruction-level divergence.
- PPU and LCD timing tests should distinguish framebuffer-oracle coverage from informational captures, and should document whether a fixture is a committed project oracle, an upstream reference artifact, or an exploratory artifact that cannot count as strict closure.
- APU sequencing tests should separate core-owned timing and mixer semantics from frontend audio delivery; external Blargg `dmg_sound` coverage is repo-gated evidence, but it does not replace channel-local or mixer/HPF unit coverage.
- Boot and startup tests should cover both `RealBoot` and `SkipBoot` when the scenario depends on startup state, including `FF50` handoff, boot-ROM overlay versus cartridge visibility, valid versus invalid logo/checksum paths, model-specific cartridge-entry state, and direct-boot hidden-state continuity for timer, PPU, and APU.
- Joypad, serial, link, and adapter tests should prefer typed runner manifests or linked-session fixtures when host interaction or multiple participants are part of the contract; desktop presentation loops must not become the hardware oracle.
- Cartridge and mapper tests should cover header parsing, explicit support-category diagnostics, visible bank behavior, persistence capability, and special-cartridge identification; strict-mode tests must confirm that experimental heuristics stay disabled by default.
- CGB-only ROMs and model-specific CGB behavior must stay out of the DMG closure signal until CGB support is intentionally implemented and documented.
- When a new subsystem edge case is important enough to document, add the detailed behavior to the owning `docs/hardware/*.md` file and keep this section as the project-wide validation routing policy.

## Cartridge persistence, save-state, and rewind coverage

- Cartridge-persistence tests validate cartridge-owned backing stores and RTC state only; they must not require CPU, PPU, APU, WRAM, or other console-state serialization.
- Full-emulator save-state tests validate whole-machine snapshot ownership, hidden temporal-state restore, metadata compatibility, and save/load continuation determinism under the recorded execution mode and overrides.
- Rewind tests validate repeated in-memory save-state capture/restore through the same core restore path used by explicit save/load; frontend tests may cover host cleanup, HUD, settings, and buffer policy, but they must not redefine core restore semantics.
- Cartridge state included in a whole-machine save state should be validated as mapper-owned runtime snapshot state, not as a shortcut through the hardware-style `.gbsav` persistence payload.
- Save/load and rewind validation should include at least one timing-sensitive mid-run restore and at least one banked-cartridge scenario before being counted as Phase `9` closure evidence; `run_determinism`, `phase9-determinism-smoke`, and `phase9-determinism-local` provide the accepted in-memory save/load continuation lane across Phase `2`, Mooneye Timer/DMA, Acid/Mealybug PPU, Phase `6` cartridge, and one Blargg APU case.

## DMA and APU validation focus

- DMA tests should cover `FF46` source-page selection, DMG echo-alias source behavior above `DFFF`, `160`-byte copy correctness, the `640`-dot / `160`-M-cycle burst body, the current one-full-M-cycle post-`FF46` start seam, first-byte commit timing, completion timing, restart behavior, source-bus-aware CPU blocking, HRAM accessibility, and OAM/LCD composition whenever suitable tests exist.
- APU tests should cover `NR52` power, wave RAM preservation, `DIV-APU` falling-edge timing, frame-sequencer clocks, per-channel DAC-enabled versus active state, trigger ordering, channel-local timing quirks, `NR50` / `NR51` routing, DAC conversion, HPF persistence, DC-offset/pop behavior, and the rule that host sample cadence must not feed back into core timing.
- Detailed per-channel APU edge-case lists belong in `docs/hardware/APU.md`; this section only records the cross-project validation focus.

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
- Determinism coverage should include replay from the same ROM plus input stream and save/load determinism for the accepted closure lanes.
- "Same ROM + same execution mode + same explicit overrides + same input stream + same injected time source => same result" is the intended project contract.
- Save/load determinism should prove that saving, restoring, and continuing produces the same result as uninterrupted execution under the same recorded execution mode.
- Save states and replay logs should record the execution mode and active overrides that produced them.
- Restoring or replaying under a different execution mode should be rejected by default; if a later explicit developer conversion path is added, tests should cover that path separately and mark it as non-oracle.

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
- New workspace crates should join that per-crate gate in the same change that introduces them, with at least the project-wide `90+/90+/90+` line/region/function floor.
- For the current repo-owned coverage gate, prefer one clean instrumented workspace run with `cargo llvm-cov --workspace --no-report`, then evaluate the gated crates separately with `cargo llvm-cov report -p <crate> --fail-under-*` so the signal stays per-crate without paying for repeated test execution. This keeps `make ci` from re-running the same tests once via `cargo test` and again via coverage while still enforcing floors for the full current workspace surface. `.cargo/config.toml` is the single primary source for the concrete `--fail-under-*` values, and those configured values must never decrease from their current setting.
- Experimental suites may exist in nightly or manual jobs, but they must publish artifacts separately and must not gate or dilute the official strict-mode closure signal.
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
