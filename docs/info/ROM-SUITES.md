# External ROM suites

This document owns operational ROM-suite mechanics. Project-wide validation policy lives in [`../TESTING.md`](../TESTING.md), hardware reference priority lives in [`../REFERENCES.md`](../REFERENCES.md), and phase scope lives in [`../ROADMAP.md`](../ROADMAP.md).

## External ROM policy

External ROMs are regression inputs, not hardware documentation. Use them after consulting the owning hardware handbook and keep acceptance automated through typed oracles rather than manual LCD inspection.

Redistributable runnable assets are materialized under the gitignored `/test/<report>/` store from pinned source metadata and SHA-256 hashes. Committed fixtures, synthetic linked ROMs, and local report assets live under `crates/gb-test-runner/data/**`. Private firmware and commercial ROMs must stay outside the repository, outside `/test/`, and outside CI.

When ROM-suite output influences a go/no-go decision, compare status/artifacts against a clean baseline and name changed rows as improved, unchanged, or regressed. Do not infer “no regressions” from memory.

## Report inventory

`crates/gb-test-runner/data/reports.toml` is the registry for `cargo rom-fetch`, `cargo rom-suite`, and `cargo rom-suite-link`. It defines report IDs, store roots, source manifests, shared status/artifact defaults, optional family order, and `local = true` reports that do not fetch upstream sources.

Fetchable reports use `crates/gb-test-runner/data/<report>/sources.report.toml`. Each source manifest pins upstream repositories, sparse checkout roots, target paths, and SHA-256 hashes. The local `linked` report has no source manifest because its ROMs and fixtures are committed under `crates/gb-test-runner/data/linked/`.

| Report | Runner | Purpose |
| --- | --- | --- |
| `gb-emulator-shootout` | `cargo rom-suite` | Promoted GB Emulator Shootout report used by `test-roms`. |
| `docboy` | `cargo rom-suite`, `cargo rom-suite-link` | DocBoy single-machine suites plus DocBoy DMG linked session suite. |
| `gbmicrotest` | `cargo rom-suite` | Flat gbmicrotest report. |
| `mooneye`, `ax6`, `little-things-gb`, `magen`, `mealybug-tearoom-tests`, `samesuite` | `cargo rom-suite` | Standalone exploratory report channels used by `test-roms-extra`. |
| `linked` | `cargo rom-suite-link` | Repo-local synthetic linked-session fixtures. |

## Fetching and store layout

```bash
cargo rom-fetch gb-emulator-shootout
cargo rom-fetch gb-emulator-shootout blargg acid
cargo rom-fetch docboy
```

`cargo rom-fetch <report> [family ...]` materializes all report families when no family is provided, or only the explicit families otherwise. It rejects `local = true` reports, uses temporary pinned `git` checkouts, verifies hashes, and preserves report runtime directories such as `.status` and `.artifacts`.

`cargo rom-suite` and `cargo rom-suite-link` auto-fetch missing or stale assets for fetchable reports before running selected cases, so explicit `cargo rom-fetch` is only needed when you want a separate materialization step.

Runtime files are scoped by report:

- ROMs and fetched fixtures: `/test/<report-store>/...`.
- Single-machine status: `/test/<report-store>/.status/<suite>.toml`.
- Linked-session status: `/test/<report-store>/.status/<suite>.toml`.
- Failure artifacts: `/test/<report-store>/.artifacts/<suite>/<case>/`.
- Rendered single-machine report views: `/test/<report-store>/test-report.md` and optionally `/test/<report-store>/test-report.html`.
- Local `linked` assets: `crates/gb-test-runner/data/linked/**`, with runtime status/artifacts still under `/test/linked/`.

Rendered report files are derived from `.status`; regenerate them with `cargo rom-report <report>` after running or updating a suite.

## Manifest rules

- Single-machine suites live as `crates/gb-test-runner/data/<report>/*.suite.toml`.
- Linked-session suites live as `crates/gb-test-runner/data/<report>/*.link.suite.toml`.
- Every suite manifest must declare `report = "<report-id>"`; mismatches are rejected.
- Every suite should declare common metadata once in the header and override only rows that differ.
- `execution_mode` is omitted for default `Strict`; set it only for intentional `permissive` or `experimental` cases.
- `disabled = true` requires a non-empty `comment = "..."`.
- Use `report_console_suffix = true` in the header or a `[[case]]` only when the same upstream report label needs console-disambiguated rows; case values override the header and status `rom` text receives `(DMG)`, `(GBC)`, `(SGB)`, or `(SGB2)`.
- Do not add root-level legacy manifests, ad hoc suite copies, or direct upstream checkout paths.

Linked manifests use `[[case]]` plus `[[case.participant]]`, explicit participant IDs, and `topology = "dmg04"`, `topology = "dmg07"`, or `topology = "cgb-ir"`.

## Oracles and fixtures

Use structured inline oracle tables in `*.suite.toml` and `*.link.suite.toml`:

```toml
oracle = { type = "serial-contains", expected = "Passed" }
oracle = { type = "fibonacci-result" }
oracle = { type = "memory-byte-equals", address = 65520, value = 1, fail_value = 2 }
oracle = { type = "framebuffer", mode = "until-match", source = "cgb", fixture = "ppu/example.png" }
oracle = { type = "snapshot", target_participant = "left", fixture = "fixtures/dmg04/left.snapshot" }
oracle = { type = "serial-hex-exact", target_participant = "receiver", expected = "B2" }
oracle = { type = "trace" }
```

Framebuffer defaults are `mode = "final"`, `source = "dmg"`, `projection = "palette-rank"`, and `compare = "exact"`. Use `mode = "until-match"` with `check_interval_tcycles` or `check_at_tcycles` for polling/point-in-time checks, `source = "cgb"` for RGB555 output, `projection = "grayscale"` plus `compare = "grayscale-tolerance"` only for explicitly tolerated fixtures, and `mode = "info"` for CI-successful captures that do not compare.

Framebuffer `fixture = "..."` and `fixture = ["...", "..."]` are both valid. Fetched framebuffer fixtures are relative to the selected source family target root. Committed framebuffer fixtures use `local = true`, resolve under `crates/gb-test-runner/data/<report>/`, and must stay relative without `..`. Snapshot fixtures use a single `fixture = "..."` path resolved from the report asset root.

## Running suites

```bash
cargo rom-suite gb-emulator-shootout
cargo rom-suite gb-emulator-shootout --suite blargg-cpu-instrs --case blargg-cpu-instrs-01-special
cargo rom-suite gbmicrotest
cargo rom-suite-link linked
cargo rom-suite-link docboy --suite docboy-dmg-link
```

`cargo rom-suite <report> [--suite <suite>] [--case <case>] [--threads <n>] [--boot-rom-dir <dir>]` executes single-machine `*.suite.toml` manifests through `gb_core::Machine`. It ignores `*.link.suite.toml`, keeps running later cases after failures, writes per-suite status and failure artifacts, and returns non-zero if any selected case fails. `--case` requires `--suite`.

`cargo rom-suite-link <report> [--suite <suite>] [--case <case>] [--threads <n>] [--boot-rom-dir <dir>]` executes linked-session `*.link.suite.toml` manifests. It collects participant-scoped serial, snapshot, framebuffer, and trace observations and uses the same oracle catalog as single-machine suites.

Cases run in parallel by default through Rayon. Use `--threads <n>` to cap local parallelism; CI matrix jobs normally omit it.

Supported `console` values are `dmg`, `cgb`, `sgb`, and `sgb2`. Supported `startup` values are `skip-boot`, `custom-boot`, and `real-boot`; omitted startup defaults to `skip-boot`.

## Rendering reports

```bash
cargo rom-report gb-emulator-shootout
cargo rom-report gb-emulator-shootout --html
```

`cargo rom-report <report>` renders the report-local single-machine `.status` files into `test/<report-store>/test-report.md`, using `report_file` and `family_order` from `crates/gb-test-runner/data/reports.toml`. The header records the report id, the non-failing/total count, and the reproduction command such as `cargo rom-report gb-emulator-shootout`; `PASS` and `INFO` rows count as non-failing, while `FAIL` rows do not.

If `test/<report-store>/.status` is missing or contains no `*.toml` status files, `cargo rom-report <report>` first runs `cargo rom-suite <report>` and then renders any status written by that run. Suite failures still produce a rendered report when status exists, so use the report rows rather than the command exit as the compatibility signal.

Pass `--html` to also write `test/<report-store>/test-report.html` from the same status model. The command is local and passive; publishing the HTML requires a separate operator or GitHub Actions workflow.

## RealBoot

`cargo rom-suite` and `cargo rom-suite-link` do not use startup or boot-ROM environment variables. Pass `--boot-rom-dir <dir>` explicitly to force all selected cases or participants through RealBoot.

The directory must contain the required private firmware assets with canonical filenames such as `dmg_boot.bin`, `mgb_boot.bin`, `cgb_boot.bin`, or `cgbE_boot.bin`. The runner verifies only the assets required by the selected console/host profiles.

Use RealBoot runs as local comparison evidence. Rerun the matching default startup command afterward when status/artifacts should represent the baseline lane again.

## Before/after workflow

For ROM-driven fixes or regressions, copy the relevant `/test/<report>/` status/artifact tree before the change, rerun the suite, copy the final tree, and compare changed rows explicitly.

Same-ROM console variants are ordered DMG before GBC before SGB before SGB2 when report suffixes are enabled. Empty report categories are not materialized.

## CI integration

- Local pre-commit checks and `make coverage` do not fetch or run external ROM suites.
- GitHub `ci` mirrors Rust checks and coverage.
- GitHub `test-roms` runs the promoted `gb-emulator-shootout` matrix with `cargo rom-suite gb-emulator-shootout --suite <suite>`.
- GitHub `test-roms-extra` runs standalone report lanes with `cargo rom-suite <report>`.
- RealBoot, commercial, red, linked, and local-only lanes stay outside GitHub ROM workflows unless promoted intentionally.

## Private and commercial ROMs

Keep commercial ROMs and private firmware outside the repo, outside `/test/`, and outside CI. If a private ROM check becomes useful as repeatable evidence, keep the manifest private or promote only redistributable assets and public metadata.

For local investigation, prefer tight timeouts, explicit stimuli, and typed informational oracles such as framebuffer info, trace, serial, or memory-byte checks.
