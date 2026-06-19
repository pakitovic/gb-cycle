# External ROM suites

This document owns operational ROM-suite mechanics. Project-wide validation policy lives in [`../TESTING.md`](../TESTING.md), hardware reference priority lives in [`../REFERENCES.md`](../REFERENCES.md), and phase scope lives in [`../ROADMAP.md`](../ROADMAP.md).

## External ROM policy

External ROMs are regression inputs, not hardware documentation. Use them after consulting the owning hardware handbook and keep acceptance automated through typed oracles rather than manual LCD inspection.

Redistributable runnable assets are materialized under the gitignored `/test/<report>/` store from pinned source metadata and SHA-256 hashes. Committed fixtures, synthetic linked ROMs, and local report assets live under `crates/gb-test-runner/data/**`. Private firmware and commercial ROMs must stay outside the repository, outside `/test/`, and outside CI.

When ROM-suite output influences a go/no-go decision, compare status/artifacts against a clean baseline and name changed rows as improved, unchanged, or regressed. Do not infer “no regressions” from memory.

## Report inventory

`crates/gb-test-runner/data/reports.toml` is the registry for `cargo rom-fetch`, `cargo rom-suite`, and `cargo rom-suite-link`. It defines report IDs, store roots, source manifests, shared status/artifact defaults, optional family order, and `local = true` reports that do not fetch upstream sources.

Fetchable reports use `crates/gb-test-runner/data/<report>/sources.report.toml`. Each source manifest pins upstream Git repositories, ZIP release archives, or file-base release asset URLs, fetch roots where applicable, target paths, and SHA-256 hashes for every materialized ROM or fixture. The local `linked` report has no source manifest because its ROMs and fixtures are committed under `crates/gb-test-runner/data/linked/`.

| Report | Runner | Purpose |
| --- | --- | --- |
| `gb-emulator-shootout` | `cargo rom-suite` | Promoted GB Emulator Shootout report used by `test-roms`. |
| `docboy` | `cargo rom-suite`, `cargo rom-suite-link` | DocBoy single-machine suites plus DocBoy DMG linked session suite. |
| `gbmicrotest` | `cargo rom-suite` | Standalone GBMicrotest report archive-backed by c-sp `game-boy-test-roms` v7.0. |
| `acid` | `cargo rom-suite` | Standalone c-sp v7 Acid2/Acid Hell framebuffer report. |
| `age` | `cargo rom-suite` | Standalone c-sp v7 AGE regression report with Fibonacci and framebuffer oracles. |
| `blargg` | `cargo rom-suite` | Standalone exploratory Blargg channel archive-backed by c-sp `game-boy-test-roms` v7.0, with GB Emulator Shootout framebuffer fixtures where the promoted Blargg manifests already use them. |
| `mooneye`, `little-things-gb`, `turtle-tests`, `ashiepaws`, `nitro2k01`, `magen`, `mealybug-tearoom-tests`, `samesuite`, `rtc3test`, `mbc3-tester` | `cargo rom-suite` | Standalone exploratory report channels; `mooneye`, `little-things-gb`, `turtle-tests`, `ashiepaws`, `mealybug-tearoom-tests`, `rtc3test`, and `mbc3-tester` are archive-backed by c-sp `game-boy-test-roms`, `nitro2k01` is pinned to upstream nitro2k01 release assets with local placeholder framebuffer fixtures, and Little Things, TurtleTests, Ashiepaws, Mealybug, SameSuite, Nitro2k01, and MBC3 Tester stay out of `test-roms-extra` while their v7 inventories, manual fixtures, or exposed timing/admission gaps are validated manually. |
| `wilbertpol` | `cargo rom-suite` | Archive-backed standalone Mooneye-derived Wilbertpol channel. |
| `linked` | `cargo rom-suite-link` | Repo-local synthetic linked-session fixtures. |

Wilbertpol ROMs are related to Mooneye but are compiled and pinned as independent assets; do not deduplicate Wilbertpol rows against Mooneye by relative path or name.

The standalone `blargg` report is archive-backed by the c-sp `game-boy-test-roms` v7.0 ZIP and materializes upstream `blargg/` ROMs under one family root per original Blargg folder, plus a dedicated `halt_bug` family. It runs both multi-ROMs and individual ROMs; framebuffer fixtures come from the c-sp ZIP for aggregate screenshots and from the pinned GB Emulator Shootout source for individual screenshots already used by the promoted Blargg oracles, including `oam_bug/7-timing_effect.png`.

The standalone `acid` report is archive-backed by the c-sp `game-boy-test-roms` v7.0 ZIP and materializes only `dmg-acid2/`, `cgb-acid2/`, and `cgb-acid-hell/` under matching family roots. It uses one suite manifest per upstream folder and strict framebuffer fixtures for DMG and CGB rows, including exact CGB framebuffer comparison for the CGB-only Acid2 and Acid Hell fixtures.

The standalone `age` report is archive-backed by the c-sp `game-boy-test-roms` v7.0 ZIP and materializes the runnable ROMs and PNG fixtures from upstream `age-test-roms/` under one `age` source family at `/test/age/age/`, preserving the AGE folder structure including the nested `speed-switch/caution` folder. It keeps one suite manifest per AGE folder. AGE ROM-only rows use the upstream `0x40` terminal opcode plus Fibonacci register signature through `fibonacci-result` with `fail_on_terminal_non_pass = true`, so a terminal `0x40` with any non-pass register tuple records a failure instead of waiting for timeout; `m3-bg-bgp`, `m3-bg-lcdc`, and `m3-bg-scx` execute one exact RGB framebuffer `until-match` row per upstream PNG fixture. Filenames or fixtures tagged `cgbBC`, `cgbBCE`, `ncmBC`, or `ncmBCE` run on CPU-CGB-C because CPU-CGB-B is not an active runner revision, `cgbE` and `ncmE` rows run on CPU-CGB-E, and CPU-CGB-D is not added because AGE does not list it as compatible. The `speed-switch/caution` ROMs stay active because the upstream warning concerns instability on repeated execution on real hardware, while the emulator report uses them only as deterministic HALT/STOP speed-switch oracles. The initial local baseline is intentionally red at 0/51 non-failing rows: Fibonacci rows currently time out or reach pass registers without the runner's terminal signal, and framebuffer rows mismatch upstream fixtures. The report is published by `rom-reports-pages` but stays out of `test-roms-extra` until the baseline is investigated.

The standalone `mooneye` report is archive-backed by the c-sp `game-boy-test-roms` v7.0 ZIP and materializes upstream `mooneye-test-suite/` under `/test/mooneye/mooneye/`. Its upstream `utils/` directory is excluded because those ROMs are helper utilities rather than pass/fail tests.

The standalone `gbmicrotest` report materializes c-sp `game-boy-test-roms` v7.0 `gbmicrotest/` under `/test/gbmicrotest/gbmicrotest/`. The suite targets DMG-CPU-C, uses the FF82 `memory-byte-equals` pass/fail byte with `fail_value = 0xFF`, preserves the RealBoot exceptions by basename, and keeps `is_if_set_during_ime0.gb` at a longer timeout. Rows that are debug/probe loops, visual-only candidates, or VRAM-displayed measurements are disabled with comments until a framebuffer fixture or non-perturbing VRAM oracle exists, while `halt_op_dupe_delay.gb`, `stat_write_glitch_l154_d.gb`, and `mbc1_rom_banks.gb` remain active known failures for later hardware/core or loader work.

The standalone `little-things-gb` report materializes only the c-sp `game-boy-test-roms` v7.0 `little-things-gb` suite under `/test/little-things-gb/little-things-gb/`; the previous DocBoy-sourced `old_little-things-gb` `double-halt-cancel.gb` and `whichboot.gb` DMG/CGB suites plus their local fixtures were retired in favor of the dedicated `nitro2k01` report. The c-sp `firstwhite` DMG/CGB rows use upstream framebuffer fixtures and are green with `until-match`; the c-sp `tellinglys` DMG/CGB rows are active with deterministic button stimuli and currently fail by reaching the upstream fail framebuffer, exposing a joypad interrupt timing entropy gap rather than a fetch or fixture problem. The GitHub `test-roms-extra` matrix entry is temporarily commented out until that gap is fixed or the suite policy is changed.

The standalone `turtle-tests` report materializes the c-sp `game-boy-test-roms` v7.0 `turtle-tests/window_y_trigger/` and `turtle-tests/window_y_trigger_wx_offscreen/` folders under one `turtle-tests` report family rooted at `/test/turtle-tests/turtle-tests/`. Each suite runs one DMG row with the upstream framebuffer fixture, an exact `until-match` framebuffer oracle, and a `30`-frame timeout matching upstream guidance that screenshot comparison after about half a second is sufficient; CGB rows are deferred until a separate compatibility-mode fixture or baseline policy is chosen. The initial local baseline is intentionally red at 1/2 non-failing rows: `window_y_trigger.gb` passes, while `window_y_trigger_wx_offscreen.gb` mismatches the upstream fixture and now anchors the existing PPU window-glitch follow-up. The report is published by `rom-reports-pages`, but is not part of `test-roms-extra` while its baseline remains exploratory.

The standalone `ashiepaws` report materializes the c-sp `game-boy-test-roms` v7.0 `bully/`, `strikethrough/`, and `scribbltests/` folders as separate source families under `/test/ashiepaws/`. Active rows use upstream framebuffer fixtures with `until-match`, run DMG plus explicit CPU-CGB-D RealBoot lanes where fixtures exist, and keep `scribbltests/fairylake/fairylake.gb`, `scribbltests/statcount/statcount.gb`, and `scribbltests/winpos/winpos.gb` as disabled rows because the v7.0 archive does not ship direct single-frame PNG pass fixtures for those ROMs. The initial local baseline is intentionally red at 6/14 non-failing rows: CGB `bully`, CGB `strikethrough`, DMG/CGB `lycscx`, DMG/CGB `scxly`, and DMG/CGB `statcount-auto` expose current runner gaps while the older promoted GB Emulator Shootout Ashiepaws assets remain green. The report is published by `rom-reports-pages` with explicit boot-ROM provisioning, but is not part of `test-roms-extra` while the standalone inventory is baselined.

The standalone `nitro2k01` report materializes selected official nitro2k01 release assets under `/test/nitro2k01/`: `whichboot.gb` v1.1 from `nitro2k01/whichboot.gb`, plus `windesync-validate.gb`, `double-halt-cancel.gb`, and `double-halt-cancel-gbconly.gb` from `nitro2k01/little-things-gb`. Its committed local framebuffer fixtures live flat under `crates/gb-test-runner/data/nitro2k01/fixtures/*.png`; the suite starts with committed placeholder images so manifests and report wiring can validate, and those files are replaced manually as final fixtures become available. The `whichboot` matrix sets top-level `startup = "real-boot"` so every model row executes through verified boot ROM firmware when the local runner passes `--boot-rom-dir <dir>`, and includes the default AGB row but not an explicit AGB0 row because the current upstream ROM does not expose a useful AGB-vs-AGB0 oracle difference. The `double-halt-cancel` rows render model suffixes so the shared `double-halt-cancel.gb` DMG and CGB rows remain distinct in reports.

The standalone `magen` report materializes the official alloncm/MagenTests 0.5.0 release assets under `/test/magen/magen/` and uses the committed local framebuffer fixtures under `crates/gb-test-runner/data/magen/fixtures/`. Magen uses `file_base_url` source fetching because the official release publishes each `.gbc` as an individual asset rather than a single ZIP archive or committed build output.

The standalone `mealybug-tearoom-tests` report materializes the complete c-sp `game-boy-test-roms` v7.0 `mealybug-tearoom-tests/` archive inventory under `/test/mealybug-tearoom-tests/mealybug-tearoom-tests/`. The c-sp import is split into one suite manifest per upstream folder: `dma` and `mbc` use the Fibonacci pass/fail signature, while `ppu` uses strict framebuffer fixtures for active DMG-CPU-C and CPU CGB C/D lanes, three source-tracked DocBoy `cgb_dmg_mode` CPU-CGB-D fixtures for the `m3_wx_4/5/6_change` rows not shipped with c-sp CGB fixtures, and CPU-CGB-C/D rows for `m3_lcdc_win_en_change_multiple_wx` that temporarily use the source-tracked DocBoy fixture because upstream Mealybug `expected/CPU CGB C/D` PNG files are placeholders. DocBoy targets are materialized under `ppu/` alongside the c-sp ROMs and fixtures inside the report store so the suite has a single PPU asset root. DMG-CPU-B fixture lanes and `ppu/win_without_bg.gb` remain listed as disabled cases with comments because the current runner does not expose DMG-CPU-B as an active Game Boy revision and the window-without-BG ROM has no compatible framebuffer fixture in the archive.

The standalone `samesuite` report materializes only the c-sp `game-boy-test-roms` v7.0 `same-suite/` archive under the `samesuite` family using the upstream folder structure. The c-sp v7 SameSuite rows are split into folder-scoped manifests and use `fibonacci-result` because upstream finishes on opcode `0x40` with the standard Fibonacci pass registers; the former DocBoy/GBEmulatorShootout framebuffer-only rows and local framebuffer fixtures have been retired after validation. The CGB-A/B-specific rows stay disabled until those revisions are explicit active runner targets.

The standalone `rtc3test` report materializes the c-sp `game-boy-test-roms` v7.0 `rtc3test/` archive folder as three menu-selected families: `basic-tests`, `range-tests`, and `sub-second-writes`. Each suite runs the same upstream `rtc3test.gb` on default DMG and CGB revisions with deterministic frame-based button stimuli, report model suffixes, and strict framebuffer `until-match` oracles against the upstream DMG/CGB result screenshots; CGB rows use the runner's RGB555 `palette-rank` framebuffer oracle rather than an exact RGB oracle for the documented c-sp palette.

The standalone `mbc3-tester` report materializes the c-sp `game-boy-test-roms` v7.0 `mbc3-tester/` archive folder under `/test/mbc3-tester/mbc3-tester/`. It has one strict framebuffer suite with DMG and CGB model-suffixed rows, a `40`-frame timeout matching upstream guidance, and the upstream DMG/CGB screenshots as fixtures; the report is intentionally red in the current baseline because `mbc3-tester.gb` declares a `4 MiB` ROM-only MBC3 header (`0x11`, ROM size `0x07`, RAM size `0x00`) that the loader rejects under the standard `2 MiB` MBC3 limit instead of treating as an MBC30 variant.

Wilbertpol's upstream `utils/` directory contains helper utilities rather than pass/fail tests. Do not add `utils/dump_boot_hwio.gb` to the Wilbertpol source manifest or suites, because it jumps to the memory-dump helper and terminates without the Fibonacci pass signature.

Mooneye and Wilbertpol `madness/mgb_oam_dma_halt_sprites.gb` are MGB-specific visual OAM-DMA/HALT edge cases. Keep the manifest model at `model = "mgb"` and the upstream framebuffer fixture wiring in place, but keep the cases disabled until the current gb-cycle framebuffer mismatch is investigated.

## Fetching and store layout

```bash
cargo rom-fetch gb-emulator-shootout
cargo rom-fetch gb-emulator-shootout blargg acid
cargo rom-fetch docboy
cargo rom-fetch --boot-rom "$HOME/emu/roms/bootrom"
```

`cargo rom-fetch <report> [family ...]` materializes all report families when no family is provided, or only the explicit families otherwise. It rejects `local = true` reports, uses temporary pinned `git` checkouts, verified ZIP archives, or hashed file-base release assets, verifies hashes, and preserves report runtime directories such as `.status` and `.artifacts`. Remote ZIP and file-base sources use `curl` for the download step before file-hash validation.

`cargo rom-fetch --boot-rom <dir>` is an explicit firmware convenience path that reads `crates/gb-test-runner/data/sources.boot-rom.toml`, downloads the pinned boot ROM files from `https://gbdev.gg8.se/files/roms/bootroms/`, verifies declared size and SHA-256 before materialization, and writes only the manifest-declared canonical filenames into `<dir>` without deleting unrelated files. Suite and report commands never invoke this mode implicitly; local operators or intentionally promoted CI lanes such as selected `test-roms` suites and `gbmicrotest` must call it explicitly before passing `--boot-rom-dir <dir>`.

`cargo rom-suite` and `cargo rom-suite-link` auto-fetch missing or stale assets for fetchable reports before running selected cases, so explicit `cargo rom-fetch` is only needed when you want a separate materialization step.

`cargo rom-suite <report>` clears the selected single-machine suite status files and artifact directories before generating new single-machine status, so each selected suite starts from a clean status/artifact snapshot without deleting linked-session evidence that shares the report runtime root. `cargo rom-suite` waits until report/suite/case selection, asset materialization, manifest/oracle loading, boot-ROM preflight validation, and thread-pool setup succeed before clearing, so an invalid `--suite`, `--case`, manifest, fixture, fetch, checksum, boot-ROM, or thread setup does not wipe prior evidence. `cargo rom-report <report>` delegates runtime cleanup to that guarded `cargo rom-suite <report>` run instead of clearing first itself. Copy any previous runtime tree before rerunning if you need a before/after comparison.

Runtime files are scoped by report:

- ROMs and fetched fixtures: `/test/<report-store>/...`.
- Single-machine status: `/test/<report-store>/.status/<suite>.json`.
- Linked-session status: `/test/<report-store>/.status/<suite>.json`.
- Failure artifacts: `/test/<report-store>/.artifacts/<suite>/<case>/`.
- Rendered single-machine report views: `/test/<report-store>/test-report.md` and optionally `/test/<report-store>/.status/index.html`.
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
oracle = { type = "fibonacci-result", fail_on_terminal_non_pass = true }
oracle = { type = "memory-byte-equals", address = 65520, value = 1, fail_value = 2 }
oracle = { type = "framebuffer", mode = "until-match", source = "cgb", fixture = "ppu/example.png" }
oracle = { type = "snapshot", target_participant = "left", fixture = "fixtures/dmg04/left.snapshot" }
oracle = { type = "serial-hex-exact", target_participant = "receiver", expected = "B2" }
oracle = { type = "trace" }
```

`fibonacci-result` defaults to the current Mooneye/SameSuite-style `0x40` breakpoint or terminal signal, including the `0x40 0x00 0x18 0xFD` loop used by older promoted assets, the compact `0x40 0x18 0xFE` loop used by the c-sp Mooneye ZIP, and the `0x40 0x76` breakpoint-then-HALT sequence used by c-sp v7 SameSuite. Set `legacy = true` only for old Mooneye-derived ROMs such as Wilbertpol that finish on undefined opcode `0xED` with the same Fibonacci register signature; when legacy mode observes `0xED` without the pass signature, the case fails immediately instead of running until timeout. Set `fail_on_terminal_non_pass = true` only for AGE-style ROMs where the upstream contract defines terminal `0x40` plus any non-Fibonacci register tuple as a completed failing test.

Framebuffer defaults are `mode = "final"`, `source = "dmg"`, `projection = "palette-rank"`, and `compare = "exact"`. Use `mode = "until-match"` with `check_interval_tcycles` or `check_at_tcycles` for polling/point-in-time checks, `source = "cgb"` for RGB555 output, `projection = "rgb"` for exact RGB888 comparisons against DMG grayscale expanded to RGB or CGB RGB555 expanded to RGB888, `projection = "grayscale"` plus `compare = "grayscale-tolerance"` only for explicitly tolerated fixtures, and `mode = "info"` for CI-successful captures that do not compare.

Framebuffer `fixture = "..."` and `fixture = ["...", "..."]` are both valid. Fetched framebuffer fixtures are relative to the selected source family target root. Committed framebuffer fixtures use `local = true`, resolve under `crates/gb-test-runner/data/<report>/`, and must stay relative without `..`. Snapshot fixtures use a single `fixture = "..."` path resolved from the report asset root.

## Running suites

```bash
cargo rom-suite gb-emulator-shootout
cargo rom-suite gb-emulator-shootout --suite blargg-cpu-instrs --case blargg-cpu-instrs-01-special
cargo rom-suite gbmicrotest
cargo rom-suite-link linked
cargo rom-suite-link docboy --suite docboy-dmg-link
```

`cargo rom-suite <report> [--suite <suite>] [--case <case>] [--threads <n>] [--boot-rom-dir <dir>] [--force-real-boot]` validates report/suite/case selection and boot-ROM preflight, clears only the selected single-machine suite `.status/<suite>.json` files and `.artifacts/<suite>/` directories, then executes single-machine `*.suite.toml` manifests through `gb_core::Machine`. It ignores `*.link.suite.toml`, keeps running later cases after failures, writes per-suite status and failure artifacts, and returns non-zero if any selected case fails. `--case` requires `--suite`.

`cargo rom-suite-link <report> [--suite <suite>] [--case <case>] [--threads <n>] [--boot-rom-dir <dir>]` executes linked-session `*.link.suite.toml` manifests. It collects participant-scoped serial, snapshot, framebuffer, and trace observations and uses the same oracle catalog as single-machine suites.

Cases run in parallel by default through Rayon. Use `--threads <n>` to cap local parallelism; CI matrix jobs normally omit it.

Supported `model` values are `dmg`, `mgb`, `cgb`, `agb`, `sgb`, and `sgb2`. Supported `startup` values are `skip-boot`, `custom-boot`, and `real-boot`; omitted startup defaults to `skip-boot`.

## Rendering reports

```bash
cargo rom-report gb-emulator-shootout
cargo rom-report gb-emulator-shootout --html
cargo rom-report gb-emulator-shootout --boot-rom-dir "$HOME/emu/roms/bootrom"
cargo rom-report gb-emulator-shootout --boot-rom-dir "$HOME/emu/roms/bootrom" --force-real-boot
cargo rom-report --index _site
```

`cargo rom-report <report>` validates that the report has single-machine suite manifests, runs `cargo rom-suite <report>`, and renders the fresh current single-machine status files into `test/<report-store>/test-report.md`, using `report_file` and `family_order` from `crates/gb-test-runner/data/reports.toml`. Pass `--boot-rom-dir <dir>` to forward the same directory to the delegated `cargo rom-suite <report> --boot-rom-dir <dir>` run; this supplies verified boot ROM assets to cases whose manifest resolves `startup = "real-boot"` without changing the startup mode of other cases. Add `--force-real-boot` with `--boot-rom-dir <dir>` to preserve the explicit local comparison lane that forces every selected single-machine report case through RealBoot. The delegated suite run owns selected single-machine `.status/<suite>.json` and `.artifacts/<suite>/` cleanup after preflight; if it fails before reaching that guarded cleanup point, `cargo rom-report` preserves existing evidence and returns an error instead of rendering stale statuses. The renderer filters status files to current single-machine suite names so mixed reports such as `docboy` can retain linked-session status/artifacts beside single-machine output, and `.status/summary.json` is reserved for report aggregation rather than suite status. The header records the report id, the non-failing/total count, and the reproduction command such as `cargo rom-report gb-emulator-shootout`; report commands generated with `--boot-rom-dir` record a non-private placeholder such as `cargo rom-report gb-emulator-shootout --boot-rom-dir <dir>`, and forced RealBoot reports append `--force-real-boot`, instead of recording the local boot-ROM directory. `PASS` and `INFO` rows count as non-failing, while `FAIL` rows do not.

Fetchable report rows are sorted by `family_order`, then by each family's pinned `sources.report.toml` ROM order, then by same-ROM model variant order, then by suite/case order and lexical fallback for rows not present in the source manifest.

Reports that only contain linked-session manifests are rejected before cleanup because `cargo rom-report` is a single-machine report renderer; use `cargo rom-suite-link` and linked status/artifacts directly for those reports. Suite case failures during the `cargo rom-report <report>` regeneration still produce a rendered report after the delegated suite runner has cleared and written fresh status, so use the report rows rather than the command exit as the compatibility signal.

Pass `--html` to also write `test/<report-store>/.status/index.html` from the same JSON-backed status model using the Askama template in `crates/gb-test-runner/templates/report/report.html`; `test-report.html` is not produced. Every normal report render also writes a derived JSON summary sidecar such as `test/<report-store>/.status/summary.json` containing `report_id`, `non_failing_cases`, and `total_cases`; the sidecar is local/intermediate input for page assembly and avoids scraping report HTML. Use `cargo rom-report --index <dir>` to passively publish already materialized report HTML into `<dir>/reports/<report>/index.html` and render `<dir>/index.html` from `crates/gb-test-runner/data/rom-reports-pages.json` order plus `test/<report-store>/.status/summary.json`; reports missing `.status/index.html` or `.status/summary.json` are omitted, `summary.json` is not copied into the final web directory, and the generated timestamp is stored as UTC/epoch metadata while the page formats it with the viewer browser timezone. The command does not rerun suites, does not accept an explicit report list, and keeps `test-report.md` as the local Markdown report output. The manual `rom-reports-pages.yml` workflow publishes the curated HTML report set to GitHub Pages, and a successful non-dry-run `release.yml` dispatches that workflow from the new release tag.

## RealBoot

`cargo rom-suite` and `cargo rom-suite-link` do not use startup or boot-ROM environment variables. For single-machine `cargo rom-suite`, pass `--boot-rom-dir <dir>` to provide verified assets to selected cases whose manifest resolves `startup = "real-boot"`; cases without explicit or inherited RealBoot keep their manifest startup mode, and the directory is ignored when no selected single-machine case needs RealBoot assets. Add `--force-real-boot` with `--boot-rom-dir <dir>` to force every selected single-machine case through RealBoot for a full report, suite, or case selection. For `cargo rom-suite-link`, `--boot-rom-dir <dir>` still forces all selected linked participants through RealBoot.

The directory must contain the required private firmware assets with canonical filenames such as `dmg_boot.bin`, `mgb_boot.bin`, `cgb_boot.bin`, `cgbE_boot.bin`, `cgb_agb0_boot.bin`, or `cgb_agb_boot.bin`. The runner verifies only the assets required by the selected RealBoot model/host profiles.

Use `cargo rom-fetch --boot-rom <dir>` only as an explicit setup step when you want the test-runner tooling to populate that boot-ROM directory from the pinned source manifest; this remains separate from report or suite auto-fetch and must be wired deliberately by local operators or promoted CI lanes.

Use RealBoot runs as local comparison evidence. Rerun the matching default startup command afterward when status/artifacts should represent the baseline lane again.

## Before/after workflow

For ROM-driven fixes or regressions, copy the relevant `/test/<report>/` status/artifact tree before the change, rerun the suite, copy the final tree, and compare changed rows explicitly. This copy must happen before running `cargo rom-suite` or a `cargo rom-report` command that reaches the delegated suite cleanup point for the suites being compared, because the selected single-machine suite `.status/<suite>.json` files and `.artifacts/<suite>/` directories are cleared before fresh case execution starts.

Same-ROM model variants are ordered DMG before MGB before GBC before AGB before SGB before SGB2 when report suffixes are enabled. Empty report categories are not materialized.

## CI integration

- Local pre-commit checks and `make coverage` do not fetch or run external ROM suites.
- GitHub `ci` mirrors Rust checks and coverage.
- GitHub `test-roms` runs the promoted `gb-emulator-shootout` matrix with `cargo rom-suite gb-emulator-shootout --suite <suite>`; rows that intentionally depend on manifest-declared RealBoot, currently `ashiepaws` and `mealybug-tearoom-tests`, first run `cargo rom-fetch --boot-rom <dir>` into runner-local temporary storage and then pass `--boot-rom-dir <dir>`.
- GitHub `test-roms-extra` runs explicitly promoted standalone report lanes with `cargo rom-suite <report>`; any promoted row that intentionally depends on manifest-declared RealBoot must first run `cargo rom-fetch --boot-rom <dir>` into runner-local temporary storage and then pass `--boot-rom-dir <dir>`. `gbmicrotest` is temporarily commented out while the c-sp v7 import is baselined, `rtc3test` remains the replacement for the removed standalone `ax6` report lane, `little-things-gb` is temporarily commented out during the c-sp v7 Telling LYs joypad IRQ entropy investigation, `age`, `turtle-tests`, and `ashiepaws` stay Pages-only while their c-sp v7 inventories are baselined, `nitro2k01` stays local while placeholder framebuffer fixtures are replaced with manual fixtures, standalone `mealybug-tearoom-tests` plus `samesuite` are temporarily commented out while their c-sp v7 inventories are validated manually, and `mbc3-tester` stays out of this CI lane because it is intentionally Strict-red until the MBC30 ROM-only admission policy is resolved.
- GitHub `rom-reports-pages` reads its curated report metadata from `crates/gb-test-runner/data/rom-reports-pages.json`; RealBoot-backed rows, currently `gb-emulator-shootout`, `gbmicrotest`, `ashiepaws`, and `nitro2k01`, set `boot_roms: true` so the workflow fetches pinned boot ROM assets into runner-local temporary storage and passes `--boot-rom-dir <dir>` to `cargo rom-report <report> --html`, preserving manifest-declared RealBoot without forcing unrelated cases. Render jobs upload each report's `.status/` directory as an intermediate artifact, and the assemble job reconstructs `test/<report>/.status/` before running `cargo rom-report --index _site`; the published index follows the JSON metadata order, includes non-failing/total counts and a green check only when all executed rows are non-failing, and does not publish `summary.json`.
- RealBoot, commercial, red, linked, and local-only lanes stay outside GitHub ROM workflows unless promoted intentionally; any promoted RealBoot lane must fetch pinned boot ROM assets into runner-local temporary storage and pass `--boot-rom-dir <dir>` explicitly.

## Private and commercial ROMs

Keep commercial ROMs and private firmware outside the repo, outside `/test/`, and outside CI. Publicly pinned boot ROM assets may be fetched into runner-local temporary storage only by explicitly promoted RealBoot lanes. If a private ROM check becomes useful as repeatable evidence, keep the manifest private or promote only redistributable assets and public metadata.

For local investigation, prefer tight timeouts, explicit stimuli, and typed informational oracles such as framebuffer info, trace, serial, or memory-byte checks.
