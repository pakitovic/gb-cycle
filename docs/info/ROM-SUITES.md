# External ROM suites

This document owns operational ROM-suite mechanics: how external ROMs are materialized, which repo targets run which suite lanes, where reports land, and how private/local ROM manifests are used. Project-wide validation policy lives in [`../TESTING.md`](../TESTING.md), external reference order lives in [`../REFERENCES.md`](../REFERENCES.md), and phase scope lives in [`../ROADMAP.md`](../ROADMAP.md).

External ROMs are inputs, not source-of-truth hardware documentation. Use them to lock observable behavior after consulting the owning hardware handbook and the reference order in [`../REFERENCES.md`](../REFERENCES.md).

## Source inventory

- `crates/gb-test-runner/data/gb-emulator-shootout/sources.toml` is the source of truth for the promoted GB Emulator Shootout report catalog: upstream source IDs, pinned revisions, required file paths, SHA-256 hashes, source-family aliases, and materialized family names used by `make test-roms` and promoted `make run-*` targets.
- `crates/gb-test-runner/data/docboy/sources.toml` is the source of truth for the DocBoy single-machine report catalog: upstream source ID `docboy`, pinned revision, required ROM/result fixture paths, SHA-256 hashes, and materialized family names used by `make test-roms-docboy` and `make run-docboy-*` targets.
- `crates/gb-test-runner/data/gbmicrotest/sources.toml` is the source of truth for the gbmicrotest report catalog: local source ID `gbmicrotest`, the pinned DocBoy revision, the 438 required ROM paths, SHA-256 hashes, and the flat `/test/gbmicrotest/<rom>` materialization used by `make test-roms-gbmicrotest` and `make run-gbmicrotest`.
- `crates/gb-test-runner/data/sources.toml` remains the legacy source inventory for extra/internal lanes and the currently-unpromoted `docboy-dmg-linked-extra` assets; it intentionally does not duplicate promoted-only rows such as Blargg, DocBoy single-machine rows, or gbmicrotest rows.
- `crates/gb-test-runner/data/reports.toml` and the report-local `sources.report.toml` inventories are the report fetch metadata for non-legacy reports: `reports.toml` defines report IDs, store roots, inherited status/artifact/report file defaults, and optional report family order; each `sources.report.toml` groups pinned files by source and family with explicit sparse checkout paths, `target_root`, per-file `target`, and SHA-256. They do not replace the stable `sources.toml` files or Makefile fetch path yet.
- Active report ID `gb-emulator-shootout` uses upstream source ID `gbemu-shootout`; active report ID `docboy` uses upstream source ID `docboy`; active report ID `gbmicrotest` uses local source ID `gbmicrotest` against the pinned DocBoy repository; legacy extra/internal lanes still use the strictly required `gbemu-shootout` rows plus non-DocBoy `docboy` rows from the root source inventory. Fetching always uses temporary `git` checkouts of the pinned sources instead of user-supplied checkout roots.
- Active promoted materialized families are `acid`, `ashiepaws`, `ax6`, `blargg`, `cpp`, `daid`, `mealybug-tearoom-tests`, `mooneye`, and `samesuite` below `/test/gb-emulator-shootout/<family>/...`; active DocBoy materialized families are `docboy-dmg`, `docboy-cgb`, `docboy-cgb-dmg`, and `docboy-cgb-dmg-ext` below `/test/docboy/{dmg,cgb,cgb-dmg,cgb-dmg-ext}/...`; active gbmicrotest rows materialize directly below `/test/gbmicrotest/<rom>` without a nested family directory; active legacy materialized families are `ax6`, `little-things-gb`, `magen`, `mealybug-tearoom-tests`, `mooneye`, and `samesuite` below `/test/<family>/...`.
- Fixture provenance that matters for source selection, including temporary CasualPokePlayer SGB fixture material and SameSuite/DocBoy split ownership, belongs in [`../REFERENCES.md`](../REFERENCES.md) and the matching manifest or fixture notes, not in ad hoc command notes.
- Do not add direct upstream checkouts, generated ROMs, private firmware, commercial ROMs, differential output, or one-off local artifacts to git.

## Fetching and store layout

```bash
make fetch-test-roms REPORT=legacy FAMILIES=samesuite
make fetch-test-roms REPORT=legacy FAMILIES="ax6 samesuite"
make fetch-test-roms REPORT=docboy FAMILIES=docboy-dmg
make fetch-test-roms REPORT=gbmicrotest FAMILIES=gbmicrotest
make fetch-test-roms REPORT=gb-emulator-shootout FAMILIES=blargg
make fetch-test-roms REPORT=gb-emulator-shootout FAMILIES="blargg acid samesuite"
scripts/fetch.sh legacy samesuite
scripts/fetch.sh docboy docboy-dmg
scripts/fetch.sh gbmicrotest gbmicrotest
scripts/fetch.sh gb-emulator-shootout acid
cargo run --release -q -p gb-test-runner --bin fetch_test_roms -- legacy samesuite
cargo run --release -q -p gb-test-runner --bin fetch_test_roms -- docboy docboy-dmg
cargo run --release -q -p gb-test-runner --bin fetch_test_roms -- gbmicrotest gbmicrotest
cargo run --release -q -p gb-test-runner --bin fetch_test_roms -- gb-emulator-shootout acid
cargo rom-fetch docboy
cargo rom-fetch gbmicrotest
cargo rom-fetch gb-emulator-shootout
cargo rom-fetch docboy docboy-dmg
cargo rom-fetch gb-emulator-shootout blargg acid mooneye
cargo rom-fetch gb-emulator-shootout acid
cargo run --release -q -p gb-test-runner --bin fetch -- docboy
cargo run --release -q -p gb-test-runner --bin fetch -- gbmicrotest
cargo run --release -q -p gb-test-runner --bin fetch -- gb-emulator-shootout acid
```

`make fetch-test-roms REPORT=... FAMILIES=...` and `scripts/fetch.sh <report> <family> [family ...]` run `fetch_test_roms`, verify the pinned source files from the selected source inventory, materialize the runnable store, and remove temporary source checkouts. These stable `fetch_test_roms` direct fetches must name a report plus one or more explicit families; omitting the report, omitting families, passing the legacy `all` selector, or passing `null` is rejected. Pass `REPORT=legacy` for Make or positional `legacy` for the script/CLI to use the legacy extra/internal store below `/test/`; pass `REPORT=docboy` for Make or positional `docboy` for the script/CLI to use `crates/gb-test-runner/data/docboy/sources.toml` and materialize DocBoy single-machine families below `/test/docboy/`; pass `REPORT=gbmicrotest` for Make or positional `gbmicrotest` for the script/CLI to use `crates/gb-test-runner/data/gbmicrotest/sources.toml` and materialize gbmicrotest ROMs directly below `/test/gbmicrotest/<rom>`; pass `REPORT=gb-emulator-shootout` for Make or positional `gb-emulator-shootout` for the script/CLI to use `crates/gb-test-runner/data/gb-emulator-shootout/sources.toml` and materialize promoted families such as `blargg` below `/test/gb-emulator-shootout/`. Every `make run-*` ROM target fetches its own required family before execution, so a direct target is self-contained.

`cargo rom-fetch <report> [family ...]` is the local Cargo alias for `cargo run --release -q -p gb-test-runner --bin fetch -- <report> [family ...]`, the report fetch path for the report registry and source metadata contract without changing the Makefile or `fetch_test_roms` yet. It reads `reports.toml`, then the selected report's `sources.report.toml`, selects all report families when no family is provided, validates any explicit family selection, sparse-checks out only the selected source-family roots, verifies every selected file hash, and copies each file to `test/<report-store>/<target_root>/<target>` while preserving the report's inherited `.status`, `.artifacts`, and Markdown report file. Reports may override the global `status_dir`, `artifact_dir`, or `report_file`; if a report omits `family_order`, `fetch` derives a deterministic alphabetical family order from the report-local `sources.report.toml`. The report registry intentionally excludes `legacy`; use `fetch_test_roms` for legacy extra/internal materialization until those lanes get their own reports.

The generated `/test/` store is gitignored. The promoted GB Emulator Shootout report owns `/test/gb-emulator-shootout/.status/`, `/test/gb-emulator-shootout/.artifacts/` when failure artifacts are requested, `/test/gb-emulator-shootout/test-report.md`, and report-local family directories such as `/test/gb-emulator-shootout/acid/`, `/test/gb-emulator-shootout/blargg/`, `/test/gb-emulator-shootout/mooneye/`, and `/test/gb-emulator-shootout/samesuite/`. Its upstream framebuffer oracle PNG fixtures are materialized into those report-local family directories from `crates/gb-test-runner/data/gb-emulator-shootout/sources.toml`; `cgb-acid2.png` stays the pinned GBEmulatorShootout fixture and its manifest row names `framebuffer-rgb555-grayscale-tolerance-fixture`, which converts the core RGB555 framebuffer and PNG fixture to grayscale and accepts per-pixel absolute luma differences up to `50` to mirror the shootout image comparator. Only the three SGB oracle fixtures missing from the pinned upstream commits stay committed under `crates/gb-test-runner/data/gb-emulator-shootout/fixtures/**`. The DocBoy report owns `/test/docboy/.status/`, `/test/docboy/.artifacts/` when failure artifacts are requested, `/test/docboy/test-report.md`, and report-local directories `/test/docboy/dmg/`, `/test/docboy/cgb/`, `/test/docboy/cgb-dmg/`, and `/test/docboy/cgb-dmg-ext/`; its committed single-machine framebuffer fixtures live under `crates/gb-test-runner/data/docboy/fixtures/**`. The gbmicrotest report owns `/test/gbmicrotest/.status/`, `/test/gbmicrotest/.artifacts/` when failure artifacts are requested, `/test/gbmicrotest/test-report.md`, and flat ROM directories such as `/test/gbmicrotest/boot/`, `/test/gbmicrotest/dma/`, `/test/gbmicrotest/interrupts/`, and `/test/gbmicrotest/ppu/`; it has no PNG fixtures. Legacy extra/internal lanes still use root-level directories such as `/test/little-things-gb/` and `/test/magen/`, write Makefile-requested failure artifacts under `/test/.artifacts/`, and keep their long-lived committed fixtures and oracle assets under `crates/gb-test-runner/data/fixtures/**`.

## Manifest rules

- Built-in promoted GB Emulator Shootout manifests live under `crates/gb-test-runner/data/gb-emulator-shootout/*.toml`; built-in DocBoy single-machine manifests live under `crates/gb-test-runner/data/docboy/*.toml`; the stable gbmicrotest manifest lives under `crates/gb-test-runner/data/gbmicrotest/gbmicrotest.toml` and its new runner manifest copy lives under `crates/gb-test-runner/data/gbmicrotest/gbmicrotest.suite.toml`; built-in legacy extra/internal manifests remain under `crates/gb-test-runner/data/*.toml`; local/private manifests passed with `--manifest` may live outside the repo.
- Every built-in external ROM suite must have a dedicated manifest; chunked promoted lanes such as `blargg-cpu-instrs`, `blargg-dmg-sound`, `blargg-timing-memory-oam`, `mooneye-acceptance-manual-misc`, `mooneye-emulator-mbc1-mbc5`, and `mooneye-emulator-mbc2` are standalone `*.toml` files, not Rust-filtered views of a larger family catalog.
- `cargo rom-suite` reads report-local `*.suite.toml` manifests; the promoted GB Emulator Shootout catalog has a `*.suite.toml` copy for every current `crates/gb-test-runner/data/gb-emulator-shootout/*.toml` suite, with structured inline oracles and fixture paths relative to each source family `target_root`.
- Case metadata that is shared by most or all rows should be declared once in the manifest header and overridden only by the rows that differ; this includes source selection, console/revision/startup/execution mode, report suffixing, timeout, oracle, expected text, memory expectations, stimuli, fixtures, and check timing.
- Omit `execution_mode` in built-in manifests for the default `Strict` mode; set it only for intentional `permissive` or `experimental` rows with an explicit reason in the owning manifest or doc.
- Do not declare a suite subsystem in manifests; external ROM suite grouping is owned by report, family, suite name, and case metadata rather than a separate manifest-level classification.
- Use `disabled = true` only with a non-empty `comment = "..."`; disabled rows are for explicit overfit, duplicate, impossible, upstream-disabled, or CI-budget cases, not for quietly hiding a failing oracle.
- Use `report_console_suffix = true` only when the same upstream report label needs console-disambiguated rows such as `(DMG)` or `(GBC)`.
- Prefer typed oracles (`serial-contains`, `fibonacci-result`, `mooneye-result`, `memory-byte-equals`, framebuffer fixtures, RGB555 framebuffer fixtures, explicitly named tolerance fixtures, trace fixtures, linked participant oracles) over manual visual inspection.
- Use `fixture = "..."` for single-fixture oracles and `fixture = ["...", "..."]` for multi-reference framebuffer fixtures; do not add a separate `fixtures` field.
- Keep synthetic linked-session fixtures under `crates/gb-test-runner/data/fixtures/linked/**`; linked-session outputs currently retain artifacts and stdout summaries rather than Markdown report rows.

## Aggregate targets

| Target | Lane | Report channel | Notes |
| --- | --- | --- | --- |
| `make test-roms` | Promoted local aggregate | `/test/gb-emulator-shootout/test-report.md` | Runs `acid`, Blargg chunks, `daid`, Mooneye chunks, `ashiepaws`, `cpp`, `mealybug`, promoted `samesuite`, `blargg-cgb-sound`, `samesuite-apu`, and `ax6`; keeps running later children after earlier red rows and returns non-zero if any child fails. |
| `make test-roms-real-boot` | Local RealBoot rerun for the promoted RealBoot-compatible subset | `/test/gb-emulator-shootout/test-report.md` | Requires `GB_CYCLE_BOOT_ROM_ROOT`; reruns promoted targets that have a verified RealBoot startup policy, including the dedicated promoted CGB-only chunks. |
| `make test-roms-extra` | Green extra/internal DMG/SGB aggregate | `/test/test-report-extra.md` | Runs `ax6-dmg-extra`, `samesuite-dmg-extra`, `mooneye-sgb-boot-regs`, and `little-things-gb`. |
| `make test-roms-extra-real-boot` | Local RealBoot rerun for the extra DMG subset | `/test/test-report-extra.md` | Requires `GB_CYCLE_BOOT_ROM_ROOT`; excludes SGB/SGB2 direct-start rows. |
| `make test-roms-cgb-extra` | Green extra/internal CGB aggregate | `/test/test-report-extra.md` | Runs `cgb-boot-hwio`, `mooneye-cgb`, `samesuite-cgb`, `magen-cgb`, `mealybug-cgb`, and `little-things-gb-cgb`; keeps running later children after earlier red rows and returns non-zero if any child fails. |
| `make test-roms-cgb-extra-real-boot` | Local RealBoot rerun for extra/internal CGB aggregate | `/test/test-report-extra.md` | Requires `GB_CYCLE_BOOT_ROM_ROOT`; useful for startup-policy comparison, not for redefining promoted CGB closure. |
| `make test-roms-docboy` | Large exploratory DocBoy single-machine aggregate | `/test/docboy/test-report.md` | Runs `docboy-dmg`, `docboy-cgb`, `docboy-cgb-dmg`, and `docboy-cgb-dmg-ext`; linked `docboy-dmg-linked-extra` remains outside this aggregate. |
| `make test-roms-docboy-real-boot` | Local RealBoot rerun for DocBoy single-machine aggregate | `/test/docboy/test-report.md` | Requires `GB_CYCLE_BOOT_ROM_ROOT`; intentionally stays local-only and excludes linked sessions. |
| `make test-roms-gbmicrotest` | gbmicrotest report aggregate | `/test/gbmicrotest/test-report.md` | Runs the standalone `gbmicrotest` suite sourced from the pinned DocBoy tree and materialized without a nested family directory. |
| `make test-roms-gbmicrotest-real-boot` | Local RealBoot rerun for gbmicrotest | `/test/gbmicrotest/test-report.md` | Requires `GB_CYCLE_BOOT_ROM_ROOT`; useful for startup-policy comparison of the gbmicrotest reset-facing rows. |

Promoted family chunks and promoted CGB rows share the `gb-emulator-shootout` report channel at `/test/gb-emulator-shootout/test-report.md`, so rerun the aggregate that matches the evidence you want before quoting a report count. Non-DocBoy extra/internal rows share the legacy `/test/test-report-extra.md`. Large DocBoy single-machine rows share the `docboy` report channel at `/test/docboy/test-report.md`. gbmicrotest rows share the `gbmicrotest` report channel at `/test/gbmicrotest/test-report.md`. Legacy Makefile failure artifacts share `/test/.artifacts/`, DocBoy Makefile failure artifacts live under `/test/docboy/.artifacts/`, and gbmicrotest Makefile failure artifacts live under `/test/gbmicrotest/.artifacts/`. Linked-session rows such as `docboy-dmg-linked-extra` and `linked-cgb-ir-smoke` print participant-scoped status to stdout and retain failure artifacts, but they do not currently append Markdown report rows.

## Promoted target catalog

| Target | Suite(s) | Source family | Purpose |
| --- | --- | --- | --- |
| `make run-acid` | `acid` | `acid` | DMG Acid visual gate, CGB Acid2 and Acid Hell framebuffer gates, plus informational `which.gb` rows for DMG and CGB. |
| `make run-blargg-cpu-instrs`, `make run-blargg-dmg-sound`, `make run-blargg-timing-memory-oam` | `blargg-cpu-instrs`, `blargg-dmg-sound`, `blargg-timing-memory-oam` | `blargg` | CPU, timing, memory, OAM, the CGB `interrupt_time.gb` timing row, and DMG sound chunks; `make test-roms` invokes these chunks directly with collect-and-continue so the aggregate mirrors CI matrix lanes. |
| `make run-daid` | `daid` | `daid` | DMG framebuffer, compatibility smoke, CGB live-BGP, and CGB speed/STOP rows. |
| `make run-mooneye-acceptance`, `make run-mooneye-mbc1-mbc5`, `make run-mooneye-mbc2` | `mooneye-acceptance-manual-misc`, `mooneye-emulator-mbc1-mbc5`, `mooneye-emulator-mbc2` | `mooneye` | DMG acceptance/manual plus CGB misc rows, MBC1/MBC5, and MBC2 chunks; `make test-roms` invokes these chunks directly with collect-and-continue. |
| `make run-ashiepaws` | `ashiepaws` | `ashiepaws` | DMG and CGB Ashiepaws PPU/framebuffer curated subset. |
| `make run-cpp` | `cpp` | `cpp` | DMG MBC3/RTC curated subset plus the SGB packet-extension fixture row. |
| `make run-mealybug` | `mealybug-tearoom-tests` | `mealybug-tearoom-tests` | DMG PPU timing and LCD pipeline framebuffer rows. |
| `make run-samesuite` | `samesuite` | `samesuite`, `mooneye` | Consolidated promoted SameSuite lane: SGB command/multiplayer fixtures, CGB PPU/DMA framebuffer rows, and Mooneye CGB misc boot/DIV rows. |
| `make run-blargg-cgb-sound` | `blargg-cgb-sound` | `blargg` | Blargg CGB sound memory-text baseline. |
| `make run-samesuite-apu` | `samesuite-apu` | `samesuite` | Advanced SameSuite CGB APU framebuffer rows. |
| `make run-ax6-cgb` | `ax6` | `ax6` | CGB MBC3 RTC AX6 framebuffer rows. |

## Extra and exploratory target catalog

| Target or command | Suite(s) | Report channel | Purpose |
| --- | --- | --- | --- |
| `make run-ax6-dmg` | `ax6-dmg-extra` | `/test/test-report-extra.md` | Extra/internal DMG MBC3 RTC AX6 rows. |
| `make run-samesuite-dmg-extra` | `samesuite-dmg-extra` | `/test/test-report-extra.md` | Extra/internal DMG SameSuite APU/interrupt rows. |
| `make run-mooneye-sgb-boot-regs` | `mooneye-sgb-boot-regs-extra` | `/test/test-report-extra.md` | SGB/SGB2 direct-start boot-register fingerprints. |
| `make run-little-things-gb` | `little-things-gb-dmg-extra` | `/test/test-report-extra.md` | DocBoy-sourced DMG `little-things-gb` rows. |
| `make run-gbmicrotest` | `gbmicrotest` | `/test/gbmicrotest/test-report.md` | Large DocBoy-sourced DMG memory-byte microtest corpus, materialized directly below `/test/gbmicrotest/<rom>`. |
| `make run-cgb-boot-hwio` | `cgb-boot-hwio` | `/test/test-report-extra.md` | Extra/internal CGB boot HWIO fingerprint row. |
| `make run-mooneye-cgb` | `mooneye-cgb-extra` | `/test/test-report-extra.md` | Extra/internal Mooneye CGB PPU acceptance subset. |
| `make run-samesuite-cgb` | `samesuite-cgb-extra` | `/test/test-report-extra.md` | Extra/internal DocBoy-sourced SameSuite CGB variant rows. |
| `make run-magen-cgb` | `magen-cgb-extra` | `/test/test-report-extra.md` | Extra/internal DocBoy-sourced Magen CGB rows. |
| `make run-mealybug-cgb` | `mealybug-tearoom-cgb-extra` | `/test/test-report-extra.md` | CGB companion of the Mealybug PPU rows, kept out of promoted CGB closure until promoted intentionally. |
| `make run-little-things-gb-cgb` | `little-things-gb-cgb-extra` | `/test/test-report-extra.md` | CGB `whichboot.gb` startup/custom-boot evidence split out of DocBoy native CGB. |
| `make run-docboy-dmg` | `docboy-dmg` | `/test/docboy/test-report.md` | Large DocBoy DMG single-machine corpus; serial two-player linked rows stay in the separate `docboy-dmg-linked-extra` suite for now. |
| `make run-docboy-cgb` | `docboy-cgb` | `/test/docboy/test-report.md` | Large DocBoy native-CGB corpus; intentionally outside promoted CGB and GitHub ROM gates. |
| `make run-docboy-cgb-dmg` | `docboy-cgb-dmg` | `/test/docboy/test-report.md` | Large DocBoy CGB GB-compatible corpus; red bring-up lane until compatibility gaps are closed. |
| `make run-docboy-cgb-dmg-ext` | `docboy-cgb-dmg-ext` | `/test/docboy/test-report.md` | Narrow experimental CGB DMG-ext register-profile lane. |
| `cargo run -p gb-test-runner --bin run_linked_session -- --suite linked-cgb-ir-smoke` | `linked-cgb-ir-smoke` | stdout/artifacts only | Internal synthetic CGB-to-CGB IR smoke; not part of Make aggregates or GitHub ROM workflows. |
| `cargo run -p gb-test-runner --bin run_linked_session -- --suite linked-dmg04-smoke` | `linked-dmg04-smoke` | stdout/artifacts only | Internal synthetic DMG-04 cable smoke used by runner/core tests. |
| `cargo run -p gb-test-runner --bin run_linked_session -- --suite linked-dmg04-contracts` | `linked-dmg04-contracts` | stdout/artifacts only | Internal DMG-04 participant-oracle contract suite. |
| `cargo run -p gb-test-runner --bin run_linked_session -- --suite linked-dmg07-smoke` | `linked-dmg07-smoke` | stdout/artifacts only | Internal DMG-07 adapter topology smoke. |

## Direct runner usage

```bash
cargo run -p gb-test-runner --bin run_rom_suite -- --suite samesuite --failure-artifact-root test/gb-emulator-shootout/.artifacts/samesuite
cargo run -p gb-test-runner --bin run_rom_suite -- --suite mooneye-cgb-extra --case mooneye-cgb-ppu-intr-2-mode0-timing-sprites
cargo run -p gb-test-runner --bin run_rom_suite -- --manifest .artifacts/local-private-smoke.toml
cargo rom-suite gb-emulator-shootout --suite blargg-cpu-instrs --case blargg-cpu-instrs-01-special
```

Use `--failure-artifact-root` whenever a failing row may need screenshots, memory text, snapshots, traces, or linked-session participant artifacts for diagnosis. Makefile ROM-suite targets go through the shared `RUN_ROM_SUITE` wrapper, default to `ROM_PROFILE=release-max`, and write promoted GB Emulator Shootout artifacts below `test/gb-emulator-shootout/.artifacts/`, DocBoy single-machine artifacts below `test/docboy/.artifacts/`, gbmicrotest artifacts below `test/gbmicrotest/.artifacts/`, or legacy artifacts below `test/.artifacts/`; override locally with `ROM_PROFILE=release make <target>` only when iterating on compile time rather than final timing evidence.

`cargo rom-suite <report-id> [--suite <suite-name>] [--case <case-id>] [--threads <n>] [--boot-rom-dir <dir>]` is the local Cargo alias for `cargo run --release -q -p gb-test-runner --bin suite -- <report-id> [--suite <suite-name>] [--case <case-id>] [--threads <n>] [--boot-rom-dir <dir>]` and is the new report-local runner path. It executes report-local `*.suite.toml` manifests directly through `gb_core::Machine`, resolves ROMs under `/test/<report-store>/<source target_root>/<rom>` using the selected report's `sources.report.toml` metadata, writes only the selected suite status under that report's `.status/`, writes failure artifacts under `test/<report-store>/.artifacts/<suite-name>/<case-id>/`, does not write Markdown reports, and `--case` is valid only with `--suite`; `target_root` may be empty for flat mono-family reports such as gbmicrotest. Cases may set `console = "dmg"`, `console = "cgb"`, `console = "sgb"`, or `console = "sgb2"`, where SGB/SGB2 use a DMG core model with the corresponding host platform profile. Cases may set `startup = "skip-boot"`, `startup = "custom-boot"`, or `startup = "real-boot"`; omitted startup defaults to `skip-boot`, and `--boot-rom-dir <dir>` forces all selected cases to `real-boot` after strictly verifying only the boot ROM assets required by their console/host profiles. Cases may set `execution_mode = "permissive"` or `execution_mode = "experimental"` when cartridge validation or mapper heuristics need the same compatibility policy as the legacy runner. The command streams a `suite <name>: running <n> cases` line before each suite so report-wide runs show progress while long suites execute. Cases run in parallel by default through Rayon; use `--threads <n>` to limit a local run to a positive number of worker threads, especially for full-report local runs, and omit it in CI matrix jobs unless the runner needs a CPU cap.

New `*.suite.toml` oracles use structured inline tables. `serial-contains` remains `oracle = { type = "serial-contains", expected = "Passed" }`; `fibonacci-result` uses `oracle = { type = "fibonacci-result" }` for the Mooneye-style register-signature protocol with CI-friendly `Passed` / `Failed` results; `memory-byte-equals` uses `oracle = { type = "memory-byte-equals", address = 65520, value = 1, fail_value = 2 }` for a single CI-friendly memory byte gate with optional fail value; framebuffer checks use the reusable `oracle = { type = "framebuffer", fixture = "..." }` form, with relative fixture paths resolved from the same report-local source `target_root` as ROM paths. A case-level `oracle` without `type` overlays the global oracle table, while a case-level `oracle` with `type` replaces it completely. Framebuffer defaults are `mode = "final"`, `source = "dmg"`, `projection = "palette-rank"`, and `compare = "exact"`. Use `mode = "until-match"` with `check_interval_tcycles = 100000` or optional `check_at_tcycles = <exact-tcycle>` for polling/point-in-time framebuffer gates, `source = "cgb"` for the core RGB555 framebuffer, `projection = "grayscale"` and `compare = "grayscale-tolerance"` for compatibility-tolerance rows, and `mode = "info"` for CI-successful informational framebuffer captures that do not compare against a fixture.

## RealBoot policy

Legacy RealBoot ROM-suite runs require `GB_CYCLE_BOOT_ROM_ROOT` to point at private firmware assets with canonical filenames derived from the manifest revision, such as `dmg_boot.bin`, `mgb_boot.bin`, `cgb_boot.bin`, or `cgbE_boot.bin`. The new `cargo rom-suite` path does not read `GB_CYCLE_BOOT_ROM_ROOT`; pass `--boot-rom-dir <dir>` explicitly when you want to force selected `*.suite.toml` cases through RealBoot. Default skip/custom-boot lanes do not read firmware bytes and must derive revision-specific behavior from manifest `revision` and the core `MachineConfig::revision` axis.

Use RealBoot aggregates as local comparison/closure evidence, not as a replacement for the default manifest startup lane. After a RealBoot run, rerun the matching non-RealBoot aggregate if you want the report file to represent the default skip/custom-boot baseline again.

## Reports and before/after workflow

The runner writes `family | rom | status` Markdown tables with `✅`, `❌`, and `ℹ️` rows plus a `non-failing/total` header summary. Promoted GB Emulator Shootout status files live under `/test/gb-emulator-shootout/.status/*.toml`, DocBoy single-machine status files live under `/test/docboy/.status/*.toml`, gbmicrotest status lives under `/test/gbmicrotest/.status/gbmicrotest.toml`, and legacy extra/internal status files live under `/test/.status/*.toml`. Status files are interpreted by owning suite before shared upstream-family fallback, so reused ROMs can keep separate promoted, extra, and DocBoy labels.

When working on known external-ROM failures, timing regressions, exploratory PPU/MMIO fixes, or any ROM-driven change that may influence a go/no-go decision, copy the matching report before the work, rerun the suite, copy the final report, and compare changed rows explicitly before keeping the change. Use `/test/gb-emulator-shootout/test-report.md` for promoted GB Emulator Shootout suites, `/test/test-report-extra.md` for non-DocBoy extra/internal single-machine suites, `/test/docboy/test-report.md` for large DocBoy single-machine suites, and `/test/gbmicrotest/test-report.md` for gbmicrotest.

Same-ROM console variants are ordered DMG before GBC before SGB before SGB2 when report suffixes are enabled. Empty report categories are not materialized; for example, a DocBoy-only run should not create or preserve empty promoted/extra report files.

## CI integration

- `make ci` remains the fast local pre-push gate and does not fetch or run external ROM suites; it covers formatting, linting, typos, dependency policy, workspace tests, and coverage gates.
- GitHub `ci` mirrors the Rust checks and coverage gate.
- GitHub `test-roms` fans out promoted family chunks and promoted CGB targets as matrix children: `acid`, Blargg chunks, `daid`, `ashiepaws`, `cpp`, `samesuite`, Mooneye chunks, `mealybug`, `blargg-cgb-sound`, `samesuite-apu`, and `ax6`.
- GitHub `test-roms-extra` fans out the green non-DocBoy extra/internal targets: `ax6-dmg`, `samesuite-dmg-extra`, `little-things-gb`, `mooneye-sgb-boot-regs`, `cgb-boot-hwio`, `mooneye-cgb`, `samesuite-cgb`, `magen-cgb`, `mealybug-cgb`, and `little-things-gb-cgb`.
- GitHub `test-roms-gbmicrotest` runs `make test-roms-gbmicrotest` as the standalone gbmicrotest report lane.
- DocBoy aggregate targets, RealBoot targets, private commercial manifests, and red/experimental local investigations stay outside GitHub ROM workflows unless promoted intentionally.

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
- `GB_CYCLE_TEST_ROM_ROOT` — optional override for the global materialized ROM-store root; if unset, `gb-test-runner` uses the default `/test/` store. Report-scoped suites append the report ID below this root, so `GB_CYCLE_TEST_ROM_ROOT=/tmp/roms` makes the promoted report store `/tmp/roms/gb-emulator-shootout/` and the gbmicrotest report store `/tmp/roms/gbmicrotest/`, not `/tmp/roms/` directly.
- `GB_CYCLE_TEST_ROM_STARTUP` — local startup override for `run_rom_suite` and `run_linked_session`; omit it to preserve manifest startup, use `skip-boot` or `custom-boot` for direct-start lanes, and use `real-boot` only with `GB_CYCLE_BOOT_ROM_ROOT`.
