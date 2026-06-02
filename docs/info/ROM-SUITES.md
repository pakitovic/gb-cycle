# External ROM suites

This document owns operational ROM-suite mechanics: how external ROMs are materialized, which repo targets run which suite lanes, where reports land, and how private/local ROM manifests are used. Project-wide validation policy lives in [`../TESTING.md`](../TESTING.md), external reference order lives in [`../REFERENCES.md`](../REFERENCES.md), and phase scope lives in [`../ROADMAP.md`](../ROADMAP.md).

External ROMs are inputs, not source-of-truth hardware documentation. Use them to lock observable behavior after consulting the owning hardware handbook and the reference order in [`../REFERENCES.md`](../REFERENCES.md).

## Source inventory

- `crates/gb-test-runner/data/reports.toml` is the report registry for `cargo rom-fetch` and `cargo rom-suite`: it defines report IDs, store roots, inherited status/artifact/report file defaults, optional report family order, and `local = true` for repo-local reports that deliberately do not fetch upstream sources.
- Each fetchable report-local `sources.report.toml` is the source of truth for the new report fetch path: it groups pinned files by upstream source and family with explicit sparse checkout paths, `target_root`, per-file `target`, and SHA-256. Active fetchable inventories are `gb-emulator-shootout/sources.report.toml`, `docboy/sources.report.toml`, `gbmicrotest/sources.report.toml`, `mooneye/sources.report.toml`, `ax6/sources.report.toml`, `little-things-gb/sources.report.toml`, `magen/sources.report.toml`, `mealybug-tearoom-tests/sources.report.toml`, and `samesuite/sources.report.toml`; local report `linked` omits `sources.report.toml` because its ROMs and fixtures live under `crates/gb-test-runner/data/linked/`.
- The old single-machine and linked-session root manifest inventories have been retired; new external single-machine lanes must use `*.suite.toml` plus a report-local `sources.report.toml`, and new linked-session lanes must use `*.link.suite.toml` plus the same report registry. Internal linked-session fixtures live under local report `crates/gb-test-runner/data/linked/*.link.suite.toml`; DocBoy linked now lives under `crates/gb-test-runner/data/docboy/docboy-dmg.link.suite.toml` and uses `docboy/sources.report.toml` for ROM materialization.
- Active report ID `gb-emulator-shootout` uses upstream source ID `gbemu-shootout`; report IDs `docboy`, `gbmicrotest`, `little-things-gb`, `magen`, and `samesuite` use DocBoy rows where their `sources.report.toml` declares them; report IDs `mooneye`, `ax6`, and `mealybug-tearoom-tests` use GBEmulatorShootout rows; report ID `linked` is local-only and fetchless. Fetching always uses temporary `git` checkouts of the pinned sources instead of user-supplied checkout roots.
- Active promoted materialized families are `acid`, `ashiepaws`, `ax6`, `blargg`, `cpp`, `daid`, `mealybug-tearoom-tests`, `mooneye`, and `samesuite` below `/test/gb-emulator-shootout/<family>/...`; active DocBoy materialized families are `docboy-dmg`, `docboy-cgb`, `docboy-cgb-dmg`, and `docboy-cgb-dmg-ext` below `/test/docboy/{dmg,cgb,cgb-dmg,cgb-dmg-ext}/...`; active gbmicrotest rows materialize directly below `/test/gbmicrotest/<rom>` without a nested family directory; the standalone exploratory reports materialize below `/test/mooneye/mooneye/`, `/test/ax6/ax6/`, `/test/little-things-gb/little-things-gb/`, `/test/magen/magen/`, `/test/mealybug-tearoom-tests/mealybug-tearoom-tests/`, and `/test/samesuite/samesuite/`.
- Fixture provenance that matters for source selection, including temporary CasualPokePlayer SGB fixture material and SameSuite/DocBoy split ownership, belongs in [`../REFERENCES.md`](../REFERENCES.md) and the matching manifest or fixture notes, not in ad hoc command notes.
- Do not add direct upstream checkouts, generated ROMs, private firmware, commercial ROMs, differential output, or one-off local artifacts to git.

## Fetching and store layout

```bash
cargo rom-fetch gb-emulator-shootout
cargo rom-fetch docboy
cargo rom-fetch gbmicrotest
cargo rom-fetch mooneye
cargo rom-fetch ax6
cargo rom-fetch little-things-gb
cargo rom-fetch magen
cargo rom-fetch mealybug-tearoom-tests
cargo rom-fetch samesuite
cargo rom-fetch gb-emulator-shootout blargg acid mooneye
cargo rom-fetch samesuite samesuite
cargo run --release -q -p gb-test-runner --bin fetch -- gb-emulator-shootout acid
```

`cargo rom-fetch <report> [family ...]` is the local Cargo alias for `cargo run --release -q -p gb-test-runner --bin fetch -- <report> [family ...]`, the report fetch path for the report registry and source metadata contract. It reads `reports.toml`, rejects reports marked `local = true`, then reads the selected report's `sources.report.toml`, selects all report families when no family is provided, validates any explicit family selection, sparse-checks out only the selected source-family roots, verifies every selected file hash, and copies each file to `test/<report-store>/<target_root>/<target>` while preserving the report's inherited `.status`, `.artifacts`, and Markdown report file. Reports may override the global `status_dir`, `artifact_dir`, or `report_file`; if a report omits `family_order`, `fetch` derives a deterministic alphabetical family order from the report-local `sources.report.toml`. The report registry intentionally excludes `legacy`; `cargo rom-suite` invokes this report fetch path automatically for any selected fetchable family with missing or stale materialized files.

The generated `/test/` store is gitignored. Each runtime report owns `/test/<report-store>/.status/`, `/test/<report-store>/.artifacts/`, and `/test/<report-store>/test-report.md` if a Markdown report is later rendered for that channel. The promoted GB Emulator Shootout report stores families below `/test/gb-emulator-shootout/<family>/`; DocBoy stores its four single-machine target roots below `/test/docboy/{dmg,cgb,cgb-dmg,cgb-dmg-ext}/`; gbmicrotest materializes flat ROM paths directly below `/test/gbmicrotest/<rom>`; standalone exploratory reports store their single source family below `/test/mooneye/mooneye/`, `/test/ax6/ax6/`, `/test/little-things-gb/little-things-gb/`, `/test/magen/magen/`, `/test/mealybug-tearoom-tests/mealybug-tearoom-tests/`, or `/test/samesuite/samesuite/`. Local report `linked` resolves assets from `crates/gb-test-runner/data/linked/` while reserving runtime status/artifacts under `/test/linked/`, not under the local asset directory. Upstream fixtures declared in `sources.report.toml` are materialized with the selected family; committed fixtures that are not shipped by the pinned upstream trees remain under `crates/gb-test-runner/data/<report>/fixtures/**` and are referenced from `*.suite.toml` with `local = true` plus paths relative to that report data directory.

## Manifest rules

- Built-in single-machine external ROM suites live only as report-local `*.suite.toml` manifests under `crates/gb-test-runner/data/<report>/`, with materialization metadata in that report's `sources.report.toml`; the repo must not add legacy single-machine root manifests or report-local non-`.suite.toml` copies. Linked-session manifests use `*.link.suite.toml`, `[[case]]` plus `[[case.participant]]`; local report `linked` keeps ROM/fixture paths relative to `crates/gb-test-runner/data/linked/`, while fetchable linked reports such as DocBoy keep ROM paths relative to their source family `target_root` and committed fixtures relative to the report data directory with `local = true`.
- Every built-in single-machine external ROM suite must have a dedicated `*.suite.toml` manifest; chunked promoted lanes such as `blargg-cpu-instrs`, `blargg-dmg-sound`, `blargg-timing-memory-oam`, `mooneye-acceptance-manual-misc`, `mooneye-emulator-mbc1-mbc5`, and `mooneye-emulator-mbc2` are standalone suite manifests, not Rust-filtered views of a larger family catalog.
- `cargo rom-suite` reads report-local `*.suite.toml` manifests with structured inline oracles and materialized fixture paths relative to each source family `target_root`. Committed report fixtures use `local = true` and resolve under `crates/gb-test-runner/data/<report>/`. Every `*.suite.toml` must declare `report = "<report-id>"`; the runner rejects manifests with a missing or mismatched report so a suite cannot be executed from the wrong report store.
- Case metadata that is shared by most or all rows should be declared once in the manifest header and overridden only by the rows that differ; this includes source selection, console/revision/startup/execution mode, report suffixing, timeout, oracle, expected text, memory expectations, stimuli, fixtures, and check timing.
- Omit `execution_mode` in built-in manifests for the default `Strict` mode; set it only for intentional `permissive` or `experimental` rows with an explicit reason in the owning manifest or doc.
- Do not declare a suite subsystem in manifests; external ROM suite grouping is owned by report, family, suite name, and case metadata rather than a separate manifest-level classification.
- Use `disabled = true` only with a non-empty `comment = "..."`; disabled rows are for explicit overfit, duplicate, impossible, upstream-disabled, or CI-budget cases, not for quietly hiding a failing oracle. `cargo rom-suite` and `cargo rom-suite-link` validate the comment and skip those rows, so migrated DocBoy disabled cases stay cataloged with their reason without becoming executable cases.
- Use `report_console_suffix = true` only when the same upstream report label needs console-disambiguated rows such as `(DMG)` or `(GBC)`.
- Prefer typed oracles (`serial-contains`, `fibonacci-result`, `memory-byte-equals`, framebuffer fixtures, RGB555 framebuffer fixtures, explicitly named tolerance fixtures, trace fixtures, linked participant oracles) over manual visual inspection; new `*.suite.toml` manifests use `fibonacci-result` for the Mooneye-style register-signature protocol.
- Use `fixture = "..."` for single-fixture oracles and `fixture = ["...", "..."]` for multi-reference framebuffer fixtures; do not add a separate `fixtures` field. For fixtures materialized from `sources.report.toml`, keep paths relative to the selected source family `target_root`; for committed fixtures in `crates/gb-test-runner/data/<report>/fixtures/**`, add `local = true` and keep paths confined to that report data directory, for example `oracle = { type = "framebuffer", local = true, fixture = "fixtures/cpp/sgb-ext-test.sgb.png" }`.
- Keep synthetic linked-session fixtures under `crates/gb-test-runner/data/linked/fixtures/**`; linked-session outputs currently retain artifacts and stdout summaries rather than Markdown report rows.

## Cargo report targets

Makefile ROM-suite aggregate and member targets have been removed. Fetch report assets with `cargo rom-fetch <report> [family ...]` when you want an explicit materialization step, or run `cargo rom-suite <report> [--suite <suite>]` directly and let the runner auto-fetch missing or stale selected families. `cargo rom-suite` keeps running later cases in a selected suite/report after earlier case failures and returns non-zero at the end if any selected case failed. Linked-session suites such as `cargo rom-suite-link docboy --suite docboy-dmg-link` print participant-scoped status to stdout and retain failure artifacts, but they do not currently append Markdown report rows.

## Promoted cargo suite catalog

| Command | Suite(s) | Source family | Purpose |
| --- | --- | --- | --- |
| `cargo rom-suite gb-emulator-shootout --suite acid` | `acid` | `acid` | DMG Acid visual gate, CGB Acid2 and Acid Hell framebuffer gates, plus informational `which.gb` rows for DMG and CGB. |
| `cargo rom-suite gb-emulator-shootout --suite blargg-cpu-instrs`, `cargo rom-suite gb-emulator-shootout --suite blargg-dmg-sound`, `cargo rom-suite gb-emulator-shootout --suite blargg-timing-memory-oam` | `blargg-cpu-instrs`, `blargg-dmg-sound`, `blargg-timing-memory-oam` | `blargg` | CPU, timing, memory, OAM, the CGB `interrupt_time.gb` timing row, and DMG sound chunks; GitHub `test-roms` invokes these chunks as independent matrix lanes. |
| `cargo rom-suite gb-emulator-shootout --suite daid` | `daid` | `daid` | DMG framebuffer, compatibility smoke, CGB live-BGP, and CGB speed/STOP rows. |
| `cargo rom-suite gb-emulator-shootout --suite mooneye-acceptance-manual-misc`, `cargo rom-suite gb-emulator-shootout --suite mooneye-emulator-mbc1-mbc5`, `cargo rom-suite gb-emulator-shootout --suite mooneye-emulator-mbc2` | `mooneye-acceptance-manual-misc`, `mooneye-emulator-mbc1-mbc5`, `mooneye-emulator-mbc2` | `mooneye` | DMG acceptance/manual plus CGB misc rows, MBC1/MBC5, and MBC2 chunks; GitHub `test-roms` invokes these chunks as independent matrix lanes. |
| `cargo rom-suite gb-emulator-shootout --suite ashiepaws` | `ashiepaws` | `ashiepaws` | DMG and CGB Ashiepaws PPU/framebuffer curated subset. |
| `cargo rom-suite gb-emulator-shootout --suite cpp` | `cpp` | `cpp` | DMG MBC3/RTC curated subset plus the SGB packet-extension fixture row. |
| `cargo rom-suite gb-emulator-shootout --suite mealybug-tearoom-tests` | `mealybug-tearoom-tests` | `mealybug-tearoom-tests` | DMG PPU timing and LCD pipeline framebuffer rows. |
| `cargo rom-suite gb-emulator-shootout --suite samesuite` | `samesuite` | `samesuite` | Consolidated promoted SameSuite lane: SGB command/multiplayer fixtures and CGB PPU/DMA framebuffer rows. |
| `cargo rom-suite gb-emulator-shootout --suite blargg-cgb-sound` | `blargg-cgb-sound` | `blargg` | Blargg CGB sound framebuffer baseline. |
| `cargo rom-suite gb-emulator-shootout --suite samesuite-apu` | `samesuite-apu` | `samesuite` | Advanced SameSuite CGB APU framebuffer rows. |
| `cargo rom-suite gb-emulator-shootout --suite ax6` | `ax6` | `ax6` | CGB MBC3 RTC AX6 framebuffer rows. |

## Extra and exploratory report catalog

| Command | Suite(s) | Report channel | Purpose |
| --- | --- | --- | --- |
| `cargo rom-suite ax6 --suite ax6-dmg` | `ax6-dmg` | `/test/ax6/` | Extra/internal DMG MBC3 RTC AX6 rows with committed DMG fixtures. |
| `cargo rom-suite samesuite --suite samesuite-dmg` | `samesuite-dmg` | `/test/samesuite/` | Extra/internal DMG SameSuite APU/interrupt rows from GBEmulatorShootout and DocBoy. |
| `cargo rom-suite mooneye --suite mooneye-sgb` | `mooneye-sgb` | `/test/mooneye/` | SGB/SGB2 direct-start boot-register fingerprints using the structured `fibonacci-result` oracle. |
| `cargo rom-suite little-things-gb --suite little-things-gb-dmg` | `little-things-gb-dmg` | `/test/little-things-gb/` | DocBoy-sourced DMG `little-things-gb` rows. |
| `cargo rom-suite mooneye --suite mooneye-cgb` | `mooneye-cgb` | `/test/mooneye/` | Extra/internal Mooneye CGB PPU acceptance subset plus the CGB boot HWIO fingerprint row; disabled rows stay cataloged with comments. |
| `cargo rom-suite samesuite --suite samesuite-cgb` | `samesuite-cgb` | `/test/samesuite/` | Extra/internal DocBoy-sourced SameSuite CGB variant rows with CGB-D/CGB-E revision metadata. |
| `cargo rom-suite magen --suite magen-cgb` | `magen-cgb` | `/test/magen/` | Extra/internal DocBoy-sourced Magen CGB rows. |
| `cargo rom-suite mealybug-tearoom-tests --suite mealybug-tearoom-tests-cgb` | `mealybug-tearoom-tests-cgb` | `/test/mealybug-tearoom-tests/` | CGB companion of the Mealybug PPU rows. |
| `cargo rom-suite little-things-gb --suite little-things-gb-cgb` | `little-things-gb-cgb` | `/test/little-things-gb/` | CGB `whichboot.gb` startup/custom-boot evidence split out of DocBoy native CGB. |

## Direct runner usage

```bash
cargo rom-suite gb-emulator-shootout --suite blargg-cpu-instrs --case blargg-cpu-instrs-01-special
cargo rom-suite mooneye --suite mooneye-cgb --case mooneye-cgb-ppu-intr-2-mode0-timing-sprites
cargo rom-suite samesuite --suite samesuite-cgb
```

Use `cargo rom-suite` for report-local external ROM suites; it always writes failure artifacts under the selected report's `.artifacts/<suite>/<case>/`. Local/private one-off ROM checks should be promoted into a report-local `*.suite.toml` before relying on them as repeatable evidence.

`cargo rom-suite <report-id> [--suite <suite-name>] [--case <case-id>] [--threads <n>] [--boot-rom-dir <dir>]` is the local Cargo alias for `cargo run --release -q -p gb-test-runner --bin suite -- <report-id> [--suite <suite-name>] [--case <case-id>] [--threads <n>] [--boot-rom-dir <dir>]` and is the new report-local runner path for single-machine `*.suite.toml` manifests; it deliberately ignores `*.link.suite.toml`, which are owned by `cargo rom-suite-link`. It executes report-local `*.suite.toml` manifests directly through `gb_core::Machine`, resolves ROMs under `/test/<report-store>/<source target_root>/<rom>` using the selected report's `sources.report.toml` metadata for fetchable reports, verifies the selected families' materialized ROMs and fixtures against the SHA-256 hashes in `sources.report.toml` before full oracle loading, auto-runs the report fetch path for any missing or stale selected family, applies manifest `[[case.stimulus]]` joypad button transitions at explicit T-cycles or frames, writes only the selected suite status under that report's `.status/`, writes failure artifacts under `test/<report-store>/.artifacts/<suite-name>/<case-id>/`, does not write Markdown reports, and `--case` is valid only with `--suite`; `target_root` may be empty for flat mono-family reports such as gbmicrotest. Reports marked `local = true` skip auto-fetch because they have no `sources.report.toml`; if such a report contains no regular `*.suite.toml`, `cargo rom-suite` fails with the normal empty-report error. Cases may set `console = "dmg"`, `console = "cgb"`, `console = "sgb"`, or `console = "sgb2"`, where SGB/SGB2 use a DMG core model with the corresponding host platform profile. Cases may set `revision = "cpu-cgb-d"` or another supported hardware revision when a row depends on revision-specific CGB behavior; omitted revisions use the selected console default. Cases may set `startup = "skip-boot"`, `startup = "custom-boot"`, or `startup = "real-boot"`; omitted startup defaults to `skip-boot`, and `--boot-rom-dir <dir>` forces all selected cases to `real-boot` after strictly verifying only the boot ROM assets required by their console/host profiles. Cases may set `execution_mode = "permissive"` or `execution_mode = "experimental"` when cartridge validation or mapper heuristics need the same compatibility policy as the legacy runner. The command streams a `suite <name>: running <n> cases` line before each suite so report-wide runs show progress while long suites execute. The runner advances MBC3 RTC deterministically from emulated T-cycles during normal execution and RealBoot handoff, so RTC ROMs do not depend on host wall-clock time. Cases run in parallel by default through Rayon; use `--threads <n>` to limit a local run to a positive number of worker threads, especially for full-report local runs, and omit it in CI matrix jobs unless the runner needs a CPU cap.

New `*.suite.toml` oracles use structured inline tables. `serial-contains` remains `oracle = { type = "serial-contains", expected = "Passed" }`; `fibonacci-result` uses `oracle = { type = "fibonacci-result" }` for the Mooneye-style register-signature protocol with CI-friendly `Passed` / `Failed` results; `memory-byte-equals` uses `oracle = { type = "memory-byte-equals", address = 65520, value = 1, fail_value = 2 }` for a single CI-friendly memory byte gate with optional fail value; framebuffer checks use the reusable `oracle = { type = "framebuffer", fixture = "..." }` form, with relative fixture paths resolved from the same report-local source `target_root` as ROM paths. Add `local = true` to framebuffer oracle tables when `fixture` points to a committed report fixture under `crates/gb-test-runner/data/<report>/`; local fixture paths may be a string or array, must be relative, and must not contain `..`. A case-level `oracle` without `type` overlays the global oracle table, including `local = true`, while a case-level `oracle` with `type` replaces it completely. Framebuffer defaults are `mode = "final"`, `source = "dmg"`, `projection = "palette-rank"`, and `compare = "exact"`. Use `mode = "until-match"` with `check_interval_tcycles = 100000` or optional `check_at_tcycles = <exact-tcycle>` for polling/point-in-time framebuffer gates, `source = "cgb"` for the core RGB555 framebuffer, `projection = "grayscale"` and `compare = "grayscale-tolerance"` for compatibility-tolerance rows, and `mode = "info"` for CI-successful informational framebuffer captures that do not compare against a fixture. Linked-session oracles in the new catalog use compact names: `snapshot` compares either the combined linked snapshot or a `target_participant` snapshot against a fixture, `serial-hex-exact` compares one participant serial stream by `target_participant`, and `trace` is a CI-successful informational oracle.

`cargo rom-suite-link <report-id> [--suite <suite-name>] [--case <case-id>] [--threads <n>] [--boot-rom-dir <dir>]` is the local Cargo alias for `cargo run --release -q -p gb-test-runner --bin suite_link -- <report-id> [--suite <suite-name>] [--case <case-id>] [--threads <n>] [--boot-rom-dir <dir>]` and executes linked-session `*.link.suite.toml` manifests such as local report `linked` and fetchable report `docboy`. Fetchable reports verify and materialize selected linked families through their report-local `sources.report.toml`; reports marked `local = true` skip fetch and resolve ROMs/fixtures from the local report data directory. Linked manifests use `[[case]]` plus `[[case.participant]]`, `topology = "dmg04"`, `topology = "dmg07"`, or `topology = "cgb-ir"`, `timeout_tcycles`, and optional `startup = "skip-boot"`, `startup = "custom-boot"`, or `startup = "real-boot"` at global, case, or participant scope. Like `cargo rom-suite`, `--boot-rom-dir <dir>` forces all selected linked participants to RealBoot after strict verification through the new boot ROM library; manifests that declare RealBoot fail without that explicit flag. The runner writes status under `test/<report-store>/.status/<suite>.toml`, failure artifacts under `test/<report-store>/.artifacts/<suite>/<case>/`, keeps running after case failures, and returns non-zero at the end if any selected linked case failed.

## RealBoot policy

Legacy RealBoot ROM-suite runs require `GB_CYCLE_BOOT_ROM_ROOT` to point at private firmware assets with canonical filenames derived from the manifest revision, such as `dmg_boot.bin`, `mgb_boot.bin`, `cgb_boot.bin`, or `cgbE_boot.bin`. The new `cargo rom-suite` and `cargo rom-suite-link` paths do not read `GB_CYCLE_BOOT_ROM_ROOT` or any startup override environment variable; pass `--boot-rom-dir <dir>` explicitly when you want to force selected single-machine cases or linked participants through RealBoot. Default skip/custom-boot lanes do not read firmware bytes and must derive revision-specific behavior from manifest `revision` and the core `MachineConfig::revision` axis.

Use RealBoot runs as local comparison/closure evidence, not as a replacement for the default manifest startup lane. After a RealBoot run, rerun the matching non-RealBoot command if you want status/artifacts to represent the default skip/custom-boot baseline again.

## Reports and before/after workflow

The new `cargo rom-suite` runner writes per-suite status TOML under `/test/<report-store>/.status/*.toml` and failure artifacts under `/test/<report-store>/.artifacts/<suite>/<case>/`; `cargo rom-suite-link` follows the same runtime-root policy for linked local report status/artifacts under `/test/linked/`. Neither runner renders Markdown reports yet. Current status roots are `/test/gb-emulator-shootout/.status/`, `/test/docboy/.status/`, `/test/gbmicrotest/.status/`, `/test/mooneye/.status/`, `/test/ax6/.status/`, `/test/little-things-gb/.status/`, `/test/magen/.status/`, `/test/mealybug-tearoom-tests/.status/`, `/test/samesuite/.status/`, and `/test/linked/.status/`.

When working on known external-ROM failures, timing regressions, exploratory PPU/MMIO fixes, or any ROM-driven change that may influence a go/no-go decision, copy the matching report status/artifact tree before the work, rerun the suite, copy the final status/artifact tree, and compare changed rows explicitly before keeping the change. Use `/test/gb-emulator-shootout/` for promoted GB Emulator Shootout suites, `/test/docboy/` for large DocBoy single-machine suites, `/test/gbmicrotest/` for gbmicrotest, and the standalone report directory for exploratory reports such as `/test/mooneye/`, `/test/ax6/`, `/test/little-things-gb/`, `/test/magen/`, `/test/mealybug-tearoom-tests/`, or `/test/samesuite/`.

Same-ROM console variants are ordered DMG before GBC before SGB before SGB2 when report suffixes are enabled. Empty report categories are not materialized; for example, a DocBoy-only run should not create or preserve empty promoted/extra report files.

## CI integration

- `make ci` remains the fast local pre-push gate and does not fetch or run external ROM suites; it covers formatting, linting, typos, dependency policy, workspace tests, and coverage gates.
- GitHub `ci` mirrors the Rust checks and coverage gate.
- GitHub `test-roms` fans out promoted `gb-emulator-shootout` suites as matrix children; each child runs `cargo rom-suite gb-emulator-shootout --suite <suite>` and relies on on-demand report fetch.
- GitHub `test-roms-extra` fans out standalone exploratory report lanes: `ax6`, `samesuite`, `little-things-gb`, `mooneye`, `magen`, `mealybug-tearoom-tests`, and `gbmicrotest`; each child runs `cargo rom-suite <report>` and relies on on-demand report fetch.
- RealBoot targets, private commercial manifests, linked sessions, and red/experimental local investigations stay outside GitHub ROM workflows unless promoted intentionally.

## Private and commercial ROM manifests

Keep commercial ROMs and private firmware outside the repo, outside `/test/`, and outside CI. For local-only smoke or audio/menu investigation, use a private manifest next to the local ROM files or with absolute ROM paths; do not standardize private paths in docs or commit generated private artifacts.

```toml
suite_name = "private-smoke"
family = "private-commercial"

[[case]]
id = "private-start-button"
rom = "commercial/example.gb"
console = "dmg"
startup = "real-boot"
mode = "strict"
timeout_frames = 760
oracle = "info-framebuffer"

[[case.stimulus]]
frame = 650
button = "start"
pressed = true
```

Manifest-driven framebuffer exports land beside the resolved private ROM path using the ROM stem. For deterministic audio/menu bugs, prefer a short local `info-trace`, `trace-fixture`, serial, memory-byte, or framebuffer oracle with explicit frame/T-cycle stimuli and a tight timeout window.

## Determinism and save/load continuation

Deterministic replay plus in-memory save/load continuation coverage is automated through cargo tests in `crates/gb-test-runner/src/determinism.rs`; it is intentionally not exposed through a manual `run_determinism` CLI. These tests compare independent replays, final `MachineSaveState` payloads, serial output, mid-run save/restore continuation, and incompatible restore rejection. Non-`Strict` cases fail fast so this path remains closure evidence rather than permissive compatibility evidence.

## Environment variables

- `GB_CYCLE_BOOT_ROM_ROOT` — private boot-ROM search path for RealBoot suite runs.
- `GB_CYCLE_TEST_ROM_ROOT` — retained only by older in-crate test helpers; the report runners materialize under the repository `test/` directory and do not read this variable.
- `GB_CYCLE_TEST_ROM_STARTUP` — retained only by older in-crate startup tests; `cargo rom-suite` and `cargo rom-suite-link` never read it and use explicit `startup` manifest fields plus `--boot-rom-dir <dir>` instead.
