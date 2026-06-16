# External ROM suites

This document owns operational ROM-suite mechanics. Project-wide validation policy lives in [`../TESTING.md`](../TESTING.md), hardware reference priority lives in [`../REFERENCES.md`](../REFERENCES.md), and phase scope lives in [`../ROADMAP.md`](../ROADMAP.md).

## External ROM policy

External ROMs are regression inputs, not hardware documentation. Use them after consulting the owning hardware handbook and keep acceptance automated through typed oracles rather than manual LCD inspection.

Redistributable runnable assets are materialized under the gitignored `/test/<report>/` store from pinned source metadata and SHA-256 hashes. Committed fixtures, synthetic linked ROMs, and local report assets live under `crates/gb-test-runner/data/**`. Private firmware and commercial ROMs must stay outside the repository, outside `/test/`, and outside CI.

When ROM-suite output influences a go/no-go decision, compare status/artifacts against a clean baseline and name changed rows as improved, unchanged, or regressed. Do not infer “no regressions” from memory.

## Report inventory

`crates/gb-test-runner/data/reports.toml` is the registry for `cargo rom-fetch`, `cargo rom-suite`, and `cargo rom-suite-link`. It defines report IDs, store roots, source manifests, shared status/artifact defaults, optional family order, and `local = true` reports that do not fetch upstream sources.

Fetchable reports use `crates/gb-test-runner/data/<report>/sources.report.toml`. Each source manifest pins upstream Git repositories or ZIP release archives, fetch roots where applicable, target paths, and SHA-256 hashes for every materialized ROM or fixture. The local `linked` report has no source manifest because its ROMs and fixtures are committed under `crates/gb-test-runner/data/linked/`.

| Report | Runner | Purpose |
| --- | --- | --- |
| `gb-emulator-shootout` | `cargo rom-suite` | Promoted GB Emulator Shootout report used by `test-roms`. |
| `docboy` | `cargo rom-suite`, `cargo rom-suite-link` | DocBoy single-machine suites plus DocBoy DMG linked session suite. |
| `gbmicrotest` | `cargo rom-suite` | Flat gbmicrotest report. |
| `blargg` | `cargo rom-suite` | Standalone exploratory Blargg channel archive-backed by c-sp `game-boy-test-roms` v7.0, with GB Emulator Shootout framebuffer fixtures where the promoted Blargg manifests already use them. |
| `mooneye`, `ax6`, `little-things-gb`, `magen`, `mealybug-tearoom-tests`, `samesuite` | `cargo rom-suite` | Standalone exploratory report channels used by `test-roms-extra`; `mooneye` and `mealybug-tearoom-tests` are archive-backed by c-sp `game-boy-test-roms`, with Mealybug temporarily removed from the workflow matrix while its v7 inventory is validated manually. |
| `wilbertpol` | `cargo rom-suite` | Archive-backed standalone Mooneye-derived Wilbertpol channel; it is intentionally not mirrored by `test-roms-extra` until it has a verified green local baseline. |
| `linked` | `cargo rom-suite-link` | Repo-local synthetic linked-session fixtures. |

Wilbertpol ROMs are related to Mooneye but are compiled and pinned as independent assets; do not deduplicate Wilbertpol rows against Mooneye by relative path or name.

The standalone `blargg` report is archive-backed by the c-sp `game-boy-test-roms` v7.0 ZIP and materializes upstream `blargg/` ROMs under one family root per original Blargg folder, plus a dedicated `halt_bug` family. It runs both multi-ROMs and individual ROMs; framebuffer fixtures come from the c-sp ZIP for aggregate screenshots and from the pinned GB Emulator Shootout source for individual screenshots already used by the promoted Blargg oracles, including `oam_bug/7-timing_effect.png`.

The standalone `mooneye` report is archive-backed by the c-sp `game-boy-test-roms` v7.0 ZIP and materializes upstream `mooneye-test-suite/` under `/test/mooneye/mooneye/`. Its upstream `utils/` directory is excluded because those ROMs are helper utilities rather than pass/fail tests.

The standalone `mealybug-tearoom-tests` report materializes the complete c-sp `game-boy-test-roms` v7.0 `mealybug-tearoom-tests/` archive inventory under `/test/mealybug-tearoom-tests/mealybug-tearoom-tests/`. The c-sp import is split into one suite manifest per upstream folder: `dma` and `mbc` use the Fibonacci pass/fail signature, while `ppu` uses strict framebuffer fixtures for active DMG-CPU-C and CPU CGB C/D lanes, three source-tracked DocBoy `cgb_dmg_mode` CPU-CGB-D fixtures for the `m3_wx_4/5/6_change` rows not shipped with c-sp CGB fixtures, and CPU-CGB-C/D rows for `m3_lcdc_win_en_change_multiple_wx` that temporarily use the source-tracked DocBoy fixture because upstream Mealybug `expected/CPU CGB C/D` PNG files are placeholders. DocBoy targets are materialized under `ppu/` alongside the c-sp ROMs and fixtures inside the report store so the suite has a single PPU asset root. DMG-CPU-B fixture lanes and `ppu/win_without_bg.gb` remain listed as disabled cases with comments because the current runner does not expose DMG-CPU-B as an active Game Boy revision and the window-without-BG ROM has no compatible framebuffer fixture in the archive.

Wilbertpol's upstream `utils/` directory contains helper utilities rather than pass/fail tests. Do not add `utils/dump_boot_hwio.gb` to the Wilbertpol source manifest or suites, because it jumps to the memory-dump helper and terminates without the Fibonacci pass signature.

Mooneye and Wilbertpol `madness/mgb_oam_dma_halt_sprites.gb` are MGB-specific visual OAM-DMA/HALT edge cases. Keep the manifest model at `model = "mgb"` and the upstream framebuffer fixture wiring in place, but keep the cases disabled until the current gb-cycle framebuffer mismatch is investigated.

## Fetching and store layout

```bash
cargo rom-fetch gb-emulator-shootout
cargo rom-fetch gb-emulator-shootout blargg acid
cargo rom-fetch docboy
```

`cargo rom-fetch <report> [family ...]` materializes all report families when no family is provided, or only the explicit families otherwise. It rejects `local = true` reports, uses temporary pinned `git` checkouts or verified ZIP archives, verifies hashes, and preserves report runtime directories such as `.status` and `.artifacts`. Remote ZIP sources use `curl` for the download step before archive-hash validation.

`cargo rom-suite` and `cargo rom-suite-link` auto-fetch missing or stale assets for fetchable reports before running selected cases, so explicit `cargo rom-fetch` is only needed when you want a separate materialization step.

`cargo rom-suite <report>` clears the selected single-machine suite status files and artifact directories before generating new single-machine status, so each selected suite starts from a clean status/artifact snapshot without deleting linked-session evidence that shares the report runtime root. `cargo rom-suite` waits until report/suite/case selection, asset materialization, manifest/oracle loading, boot-ROM preflight validation, and thread-pool setup succeed before clearing, so an invalid `--suite`, `--case`, manifest, fixture, fetch, checksum, boot-ROM, or thread setup does not wipe prior evidence. `cargo rom-report <report>` delegates runtime cleanup to that guarded `cargo rom-suite <report>` run instead of clearing first itself. Copy any previous runtime tree before rerunning if you need a before/after comparison.

Runtime files are scoped by report:

- ROMs and fetched fixtures: `/test/<report-store>/...`.
- Single-machine status: `/test/<report-store>/.status/<suite>.toml`.
- Linked-session status: `/test/<report-store>/.status/<suite>.toml`.
- Failure artifacts: `/test/<report-store>/.artifacts/<suite>/<case>/`.
- Rendered single-machine report views: `/test/<report-store>/test-report.md` and optionally `/test/<report-store>/test-report.html`.
- Local `linked` assets: `crates/gb-test-runner/data/linked/**`, with runtime status/artifacts still under `/test/linked/`.

Rendered report files are derived from the current single-machine suite `.status` files produced by the delegated run; regenerate them with `cargo rom-report <report>`, which reruns the report and relies on `cargo rom-suite` to clear stale selected-suite runtime data only after suite preflight succeeds. Status files that do not correspond to a current single-machine `*.suite.toml`, including linked-session status files, are ignored by the single-machine report renderer.

## Manifest rules

- Single-machine suites live as `crates/gb-test-runner/data/<report>/*.suite.toml`.
- Linked-session suites live as `crates/gb-test-runner/data/<report>/*.link.suite.toml`.
- Every suite manifest must declare `report = "<report-id>"`; mismatches are rejected.
- Every suite should declare common metadata once in the header and override only rows that differ.
- Unknown manifest keys are rejected so typos and stale per-case overrides cannot silently fall back to header defaults.
- `execution_mode` is omitted for default `Strict`; set it only for intentional `permissive` or `experimental` cases.
- `disabled = true` requires a non-empty `comment = "..."`.
- Use `report_model_suffix = true` in the header or a `[[case]]` only when the same upstream report label needs model-disambiguated rows; case values override the header and status `rom` text receives `(DMG)`, `(GBC)`, `(AGB)`, `(SGB)`, or `(SGB2)`.
- Use `report_revision_suffix = true` independently from `report_model_suffix` when the same upstream report label also needs CPU/revision-disambiguated rows; case values override the header and status `rom` text receives uppercase hyphenated revision labels such as `(DMG-CPU-C)`, `(CPU-CGB-C)`, or `(CPU-AGB-A)`. When both suffixes are enabled, the status text is ordered as `rom.gb (GBC) (CPU-CGB-C)`.
- Report runtime paths from `reports.toml` must stay inside the report store: `store_dir` may be empty for flat reports, but `status_dir`, `artifact_dir`, `report_file`, and `sources` must be non-empty relative paths without absolute, parent, or current-directory components.
- Do not add root-level legacy manifests, ad hoc suite copies, or direct upstream checkout paths.

Linked manifests use `[[case]]` plus `[[case.participant]]`, explicit participant IDs, and `topology = "dmg04"`, `topology = "dmg07"`, or `topology = "cgb-ir"`. `report_model_suffix` and `report_revision_suffix` are single-machine suite properties in this iteration; linked participant status keeps the manifest ROM path.

## Oracles and fixtures

Use structured inline oracle tables in `*.suite.toml` and `*.link.suite.toml`:

```toml
oracle = { type = "serial-contains", expected = "Passed" }
oracle = { type = "fibonacci-result" }
oracle = { type = "fibonacci-result", legacy = true }
oracle = { type = "memory-byte-equals", address = 65520, value = 1, fail_value = 2 }
oracle = { type = "framebuffer", mode = "until-match", source = "cgb", fixture = "ppu/example.png" }
oracle = { type = "snapshot", target_participant = "left", fixture = "fixtures/dmg04/left.snapshot" }
oracle = { type = "serial-hex-exact", target_participant = "receiver", expected = "B2" }
oracle = { type = "trace" }
```

`fibonacci-result` defaults to the current Mooneye-style `0x40` breakpoint or terminal loop signal, including the `0x40 0x00 0x18 0xFD` loop used by older promoted assets and the compact `0x40 0x18 0xFE` loop used by the c-sp Mooneye ZIP. Set `legacy = true` only for old Mooneye-derived ROMs such as Wilbertpol that finish on undefined opcode `0xED` with the same Fibonacci register signature; when legacy mode observes `0xED` without the pass signature, the case fails immediately instead of running until timeout.

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

`cargo rom-suite <report> [--suite <suite>] [--case <case>] [--threads <n>] [--boot-rom-dir <dir>]` validates report/suite/case selection and boot-ROM preflight, clears only the selected single-machine suite `.status/<suite>.toml` files and `.artifacts/<suite>/` directories, then executes single-machine `*.suite.toml` manifests through `gb_core::Machine`. It ignores `*.link.suite.toml`, keeps running later cases after failures, writes per-suite status and failure artifacts, and returns non-zero if any selected case fails. `--case` requires `--suite`.

`cargo rom-suite-link <report> [--suite <suite>] [--case <case>] [--threads <n>] [--boot-rom-dir <dir>]` executes linked-session `*.link.suite.toml` manifests. It collects participant-scoped serial, snapshot, framebuffer, and trace observations and uses the same oracle catalog as single-machine suites.

Cases run in parallel by default through Rayon. Use `--threads <n>` to cap local parallelism; CI matrix jobs normally omit it.

Supported `model` values are `dmg`, `mgb`, `cgb`, `agb`, `sgb`, and `sgb2`. Supported `startup` values are `skip-boot`, `custom-boot`, and `real-boot`; omitted startup defaults to `skip-boot`.

## Rendering reports

```bash
cargo rom-report gb-emulator-shootout
cargo rom-report gb-emulator-shootout --html
```

`cargo rom-report <report>` validates that the report has single-machine suite manifests, runs `cargo rom-suite <report>`, and renders the fresh current single-machine status files into `test/<report-store>/test-report.md`, using `report_file` and `family_order` from `crates/gb-test-runner/data/reports.toml`. The delegated suite run owns selected single-machine `.status/<suite>.toml` and `.artifacts/<suite>/` cleanup after preflight; if it fails before reaching that guarded cleanup point, `cargo rom-report` preserves existing evidence and returns an error instead of rendering stale statuses. The renderer filters status files to current single-machine suite names so mixed reports such as `docboy` can retain linked-session status/artifacts beside single-machine output. The header records the report id, the non-failing/total count, and the reproduction command such as `cargo rom-report gb-emulator-shootout`; `PASS` and `INFO` rows count as non-failing, while `FAIL` rows do not.

Fetchable report rows are sorted by `family_order`, then by each family's pinned `sources.report.toml` ROM order, then by same-ROM model variant order, then by suite/case order and lexical fallback for rows not present in the source manifest.

Reports that only contain linked-session manifests are rejected before cleanup because `cargo rom-report` is a single-machine report renderer; use `cargo rom-suite-link` and linked status/artifacts directly for those reports. Suite case failures during the `cargo rom-report <report>` regeneration still produce a rendered report after the delegated suite runner has cleared and written fresh status, so use the report rows rather than the command exit as the compatibility signal.

Pass `--html` to also write `test/<report-store>/test-report.html` from the same status model. The command is local and refreshes stale single-machine runtime data through the delegated guarded suite run; publishing the HTML requires a separate operator or GitHub Actions workflow. The manual `rom-reports-pages.yml` workflow publishes the curated HTML report set to GitHub Pages, and a successful non-dry-run `release.yml` dispatches that workflow from the new release tag.

## RealBoot

`cargo rom-suite` and `cargo rom-suite-link` do not use startup or boot-ROM environment variables. Pass `--boot-rom-dir <dir>` explicitly to force all selected cases or participants through RealBoot.

The directory must contain the required private firmware assets with canonical filenames such as `dmg_boot.bin`, `mgb_boot.bin`, `cgb_boot.bin`, `cgbE_boot.bin`, `cgb_agb0_boot.bin`, or `cgb_agb_boot.bin`. The runner verifies only the assets required by the selected model/host profiles.

Use RealBoot runs as local comparison evidence. Rerun the matching default startup command afterward when status/artifacts should represent the baseline lane again.

## Before/after workflow

For ROM-driven fixes or regressions, copy the relevant `/test/<report>/` status/artifact tree before the change, rerun the suite, copy the final tree, and compare changed rows explicitly. This copy must happen before running `cargo rom-suite` or a `cargo rom-report` command that reaches the delegated suite cleanup point for the suites being compared, because the selected single-machine suite `.status/<suite>.toml` files and `.artifacts/<suite>/` directories are cleared before fresh case execution starts.

Same-ROM model variants are ordered DMG before MGB before GBC before AGB before SGB before SGB2 when report suffixes are enabled. Empty report categories are not materialized.

## CI integration

- Local pre-commit checks and `make coverage` do not fetch or run external ROM suites.
- GitHub `ci` mirrors Rust checks and coverage.
- GitHub `test-roms` runs the promoted `gb-emulator-shootout` matrix with `cargo rom-suite gb-emulator-shootout --suite <suite>`.
- GitHub `test-roms-extra` runs explicitly promoted standalone report lanes with `cargo rom-suite <report>`; `mealybug-tearoom-tests` is temporarily commented out while the c-sp v7 inventory is validated manually, and `wilbertpol` stays out of this workflow until a green local baseline is verified and promotion is intentional.
- RealBoot, commercial, red, linked, and local-only lanes stay outside GitHub ROM workflows unless promoted intentionally.

## Private and commercial ROMs

Keep commercial ROMs and private firmware outside the repo, outside `/test/`, and outside CI. If a private ROM check becomes useful as repeatable evidence, keep the manifest private or promote only redistributable assets and public metadata.

For local investigation, prefer tight timeouts, explicit stimuli, and typed informational oracles such as framebuffer info, trace, serial, or memory-byte checks.
