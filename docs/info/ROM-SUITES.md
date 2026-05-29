# External ROM suites

This document owns operational ROM-suite mechanics: how external ROMs are materialized, which repo targets run which suite lanes, where reports land, and how private/local ROM manifests are used. Project-wide validation policy lives in [`../TESTING.md`](../TESTING.md), external reference order lives in [`../REFERENCES.md`](../REFERENCES.md), and phase scope lives in [`../ROADMAP.md`](../ROADMAP.md).

External ROMs are inputs, not source-of-truth hardware documentation. Use them to lock observable behavior after consulting the owning hardware handbook and the reference order in [`../REFERENCES.md`](../REFERENCES.md).

## Source inventory

- `crates/gb-test-runner/data/gb-emulator-shootout/sources.toml` is the source of truth for the promoted GB Emulator Shootout report catalog: upstream source IDs, pinned revisions, required file paths, SHA-256 hashes, source-family aliases, and materialized family names used by `make test-roms` and `make test-roms-cgb`.
- `crates/gb-test-runner/data/sources.toml` remains the legacy source inventory for extra/internal and DocBoy lanes until those reports are split into their own catalogs; it intentionally does not duplicate promoted-only rows such as Blargg.
- Active report ID `gb-emulator-shootout` uses upstream source ID `gbemu-shootout`; legacy extra/internal lanes still use the strictly required `gbemu-shootout` rows plus `docboy` from the root source inventory. Fetching always uses temporary `git` checkouts of the pinned sources instead of user-supplied checkout roots.
- Active promoted materialized families are `acid`, `ashiepaws`, `ax6`, `blargg`, `cpp`, `daid`, `mealybug-tearoom-tests`, `mooneye`, and `samesuite` below `/test/gb-emulator-shootout/<family>/...`; active legacy materialized families are `ax6`, `gbmicrotest`, `little-things-gb`, `magen`, `mealybug-tearoom-tests`, `mooneye`, `samesuite`, `docboy-dmg`, `docboy-cgb`, `docboy-cgb-dmg`, and `docboy-cgb-dmg-ext` below `/test/<family>/...`.
- Fixture provenance that matters for source selection, including temporary CasualPokePlayer SGB fixture material and SameSuite/DocBoy split ownership, belongs in [`../REFERENCES.md`](../REFERENCES.md) and the matching manifest or fixture notes, not in ad hoc command notes.
- Do not add direct upstream checkouts, generated ROMs, private firmware, commercial ROMs, differential output, or one-off local artifacts to git.

## Fetching and store layout

```bash
make fetch-test-roms REPORT=legacy FAMILIES=samesuite
make fetch-test-roms REPORT=legacy FAMILIES="ax6 samesuite"
make fetch-test-roms REPORT=gb-emulator-shootout FAMILIES=blargg
make fetch-test-roms REPORT=gb-emulator-shootout FAMILIES="blargg acid samesuite"
scripts/fetch.sh legacy samesuite
scripts/fetch.sh gb-emulator-shootout acid
cargo run --release -q -p gb-test-runner --bin fetch_test_roms -- legacy samesuite
cargo run --release -q -p gb-test-runner --bin fetch_test_roms -- gb-emulator-shootout acid
```

`make fetch-test-roms REPORT=... FAMILIES=...` and `scripts/fetch.sh <report> <family> [family ...]` run `fetch_test_roms`, verify the pinned source files from the selected source inventory, materialize the runnable store, and remove temporary source checkouts. Direct fetches must name a report plus one or more explicit families; omitting the report, omitting families, passing the legacy `all` selector, or passing `null` is rejected. Pass `REPORT=legacy` for Make or positional `legacy` for the script/CLI to use the legacy extra/DocBoy store below `/test/`; pass `REPORT=gb-emulator-shootout` for Make or positional `gb-emulator-shootout` for the script/CLI to use `crates/gb-test-runner/data/gb-emulator-shootout/sources.toml` and materialize promoted families such as `blargg` below `/test/gb-emulator-shootout/`. Every `make run-*` ROM target fetches its own required family before execution, so a direct target is self-contained.

The generated `/test/` store is gitignored. The promoted GB Emulator Shootout report owns `/test/gb-emulator-shootout/.status/`, `/test/gb-emulator-shootout/.artifacts/` when failure artifacts are requested, `/test/gb-emulator-shootout/test-report.md`, and report-local family directories such as `/test/gb-emulator-shootout/acid/`, `/test/gb-emulator-shootout/blargg/`, `/test/gb-emulator-shootout/mooneye/`, and `/test/gb-emulator-shootout/samesuite/`. Its upstream framebuffer oracle PNG fixtures are materialized into those report-local family directories from `crates/gb-test-runner/data/gb-emulator-shootout/sources.toml`; `cgb-acid2.png` stays the pinned GBEmulatorShootout fixture and its manifest row names `framebuffer-rgb555-grayscale-tolerance-fixture`, which converts the core RGB555 framebuffer and PNG fixture to grayscale and accepts per-pixel absolute luma differences up to `50` to mirror the shootout image comparator. Only the three SGB oracle fixtures missing from the pinned upstream commits stay committed under `crates/gb-test-runner/data/gb-emulator-shootout/fixtures/**`. Legacy extra/DocBoy lanes still use root-level directories such as `/test/docboy/dmg/`, `/test/docboy/cgb/`, `/test/docboy/cgb-dmg/`, `/test/docboy/cgb-dmg-ext/`, `/test/gbmicrotest/`, `/test/little-things-gb/`, and `/test/magen/`, write Makefile-requested failure artifacts under `/test/.artifacts/`, and keep their long-lived committed fixtures and oracle assets under `crates/gb-test-runner/data/fixtures/**`.

## Manifest rules

- Built-in promoted GB Emulator Shootout manifests live under `crates/gb-test-runner/data/gb-emulator-shootout/*.toml`; built-in legacy extra/DocBoy manifests remain under `crates/gb-test-runner/data/*.toml`; local/private manifests passed with `--manifest` may live outside the repo.
- Every built-in external ROM suite must have a dedicated manifest; chunked promoted lanes such as `blargg-cpu-instrs`, `blargg-dmg-sound`, `blargg-timing-memory-oam`, `mooneye-acceptance-manual`, `mooneye-emulator-mbc1-mbc5`, and `mooneye-emulator-mbc2` are standalone `*.toml` files, not Rust-filtered views of a larger family catalog.
- Omit `execution_mode` in built-in manifests for the default `Strict` mode; set it only for intentional `permissive` or `experimental` rows with an explicit reason in the owning manifest or doc.
- Do not declare a suite subsystem in manifests; external ROM suite grouping is owned by report, family, suite name, and case metadata rather than a separate manifest-level classification.
- Use `disabled = true` only with a non-empty `comment = "..."`; disabled rows are for explicit overfit, duplicate, impossible, upstream-disabled, or CI-budget cases, not for quietly hiding a failing oracle.
- Use `report_model_suffix = true` only when the same upstream report label needs model-disambiguated rows such as `(DMG)` or `(GBC)`.
- Prefer typed oracles (`serial-contains`, `mooneye-result`, `memory-byte-equals`, framebuffer fixtures, RGB555 framebuffer fixtures, explicitly named tolerance fixtures, trace fixtures, linked participant oracles) over manual visual inspection.
- Keep synthetic linked-session fixtures under `crates/gb-test-runner/data/fixtures/linked/**`; linked-session outputs currently retain artifacts and stdout summaries rather than Markdown report rows.

## Aggregate targets

| Target | Lane | Report channel | Notes |
| --- | --- | --- | --- |
| `make test-roms` | Promoted local DMG/SGB aggregate | `/test/gb-emulator-shootout/test-report.md` | Runs `acid`, Blargg chunks, `daid`, Mooneye chunks, `ashiepaws`, `cpp`, `cpp-sgb`, `mealybug`, and fixture-backed `samesuite-sgb`; keeps running later children after earlier red rows and returns non-zero if any child fails. |
| `make test-roms-real-boot` | Local RealBoot rerun for the promoted DMG subset | `/test/gb-emulator-shootout/test-report.md` | Requires `GB_CYCLE_BOOT_ROM_ROOT`; excludes SGB targets until SGB/SGB2 RealBoot policy exists. |
| `make test-roms-extra` | Green extra/internal DMG/SGB aggregate | `/test/test-report-extra.md` | Runs `ax6`, `samesuite`, `mooneye-sgb-boot-regs`, `little-things-gb`, and `gbmicrotest`. |
| `make test-roms-extra-real-boot` | Local RealBoot rerun for the extra DMG subset | `/test/test-report-extra.md` | Requires `GB_CYCLE_BOOT_ROM_ROOT`; excludes SGB/SGB2 direct-start rows. |
| `make test-roms-cgb` | Promoted local CGB aggregate | `/test/gb-emulator-shootout/test-report.md` | Runs `cgb-smoke`, `cgb-boot-div`, `cgb-speed`, `cgb-ppu-basic`, `cgb-ppu-hard`, `cgb-dma`, `cgb-audio-blargg`, `cgb-audio-samesuite`, and `cgb-rtc`. |
| `make test-roms-cgb-real-boot` | Local RealBoot rerun for the promoted CGB aggregate | `/test/gb-emulator-shootout/test-report.md` | Requires `GB_CYCLE_BOOT_ROM_ROOT`; uses revision-derived private boot ROM filenames such as `cgb_boot.bin` or `cgbE_boot.bin`. |
| `make test-roms-cgb-extra` | Green extra/internal CGB aggregate | `/test/test-report-extra.md` | Runs `cgb-boot-hwio`, `mooneye-cgb`, `samesuite-cgb`, `magen-cgb`, `mealybug-cgb`, and `little-things-gb-cgb`; keeps running later children after earlier red rows and returns non-zero if any child fails. |
| `make test-roms-cgb-extra-real-boot` | Local RealBoot rerun for extra/internal CGB aggregate | `/test/test-report-extra.md` | Requires `GB_CYCLE_BOOT_ROM_ROOT`; useful for startup-policy comparison, not for redefining promoted CGB closure. |
| `make test-roms-docboy` | Large exploratory DocBoy single-machine plus linked DMG lane | `/test/test-report-docboy.md` for single-machine rows; stdout/artifacts for linked rows | Runs `docboy-dmg`, `docboy-cgb`, `docboy-cgb-dmg`, and `docboy-cgb-dmg-ext`; `run-docboy-dmg` also runs `docboy-dmg-linked-extra`. |
| `make test-roms-docboy-real-boot` | Local RealBoot rerun for DocBoy aggregate | `/test/test-report-docboy.md` for single-machine rows; stdout/artifacts for linked rows | Requires `GB_CYCLE_BOOT_ROM_ROOT`; intentionally stays local-only. |

Promoted DMG/SGB and promoted CGB rows share the `gb-emulator-shootout` report channel at `/test/gb-emulator-shootout/test-report.md`, so rerun the aggregate that matches the evidence you want before quoting a report count. Non-DocBoy extra/internal rows share the legacy `/test/test-report-extra.md`. Large DocBoy single-machine rows use the legacy `/test/test-report-docboy.md`. Legacy Makefile failure artifacts share `/test/.artifacts/`. Linked-session rows such as `docboy-dmg-linked-extra` and `linked-cgb-ir-smoke` print participant-scoped status to stdout and retain failure artifacts, but they do not currently append Markdown report rows.

## Promoted target catalog

| Target | Suite(s) | Source family | Purpose |
| --- | --- | --- | --- |
| `make run-acid` | `acid-dmg-curated` | `acid` | DMG Acid visual gate plus informational `which.gb`. |
| `make run-blargg`, `make run-blargg-cpu-instrs`, `make run-blargg-dmg-sound`, `make run-blargg-timing-memory-oam` | `blargg-cpu-instrs`, `blargg-dmg-sound`, `blargg-timing-memory-oam` | `blargg` | CPU, timing, memory, OAM, and DMG sound chunks; `run-blargg` fetches once and runs the chunk suites with collect-and-continue so the aggregate mirrors CI matrix lanes. |
| `make run-daid` | `daid-dmg-curated` | `daid` | DMG framebuffer and compatibility smoke rows. |
| `make run-mooneye`, `make run-mooneye-acceptance`, `make run-mooneye-mbc1-mbc5`, `make run-mooneye-mbc2` | `mooneye-acceptance-manual`, `mooneye-emulator-mbc1-mbc5`, `mooneye-emulator-mbc2` | `mooneye` | DMG acceptance/manual plus MBC1/MBC5 and MBC2 chunks; `run-mooneye` fetches once and runs the chunk suites with collect-and-continue. |
| `make run-ashiepaws` | `ashiepaws-dmg-curated` | `ashiepaws` | DMG PPU/framebuffer curated subset. |
| `make run-cpp` | `cpp-dmg-curated` | `cpp` | DMG MBC3/RTC curated subset. |
| `make run-cpp-sgb` | `cpp-sgb` | `cpp` | Fixture-backed SGB packet-extension row, promoted to the main report without claiming full SGB closure. |
| `make run-mealybug` | `mealybug-tearoom-dmg-curated` | `mealybug-tearoom-tests` | DMG PPU timing and LCD pipeline framebuffer rows. |
| `make run-samesuite-sgb` | `samesuite-sgb` | `samesuite` | Fixture-backed SGB command/multiplayer bring-up rows from SameSuite material. |
| `make run-cgb-smoke` | `cgb-smoke` | `mooneye acid` | CGB boot-register/visual smoke catalog. |
| `make run-cgb-boot-div` | `cgb-boot-div` | `mooneye` | CGB boot/DIV timing gate. |
| `make run-cgb-speed` | `cgb-speed` | `daid blargg` | KEY1/double-speed, STOP, DIV/LY/STAT speed-domain evidence. |
| `make run-cgb-ppu-basic` | `cgb-ppu-basic` | `samesuite daid acid ashiepaws` | Baseline promoted CGB PPU framebuffer evidence. |
| `make run-cgb-ppu-hard` | `cgb-ppu-hard` | `acid` | Native-CGB hard PPU Acid row. |
| `make run-cgb-dma` | `cgb-dma` | `samesuite` | CGB GDMA/HDMA fixture-backed rows. |
| `make run-cgb-audio-blargg` | `cgb-audio-blargg` | `blargg` | CGB Blargg sound memory-text baseline. |
| `make run-cgb-audio-samesuite` | `cgb-audio-samesuite` | `samesuite` | Advanced SameSuite CGB APU framebuffer rows. |
| `make run-cgb-rtc` | `cgb-rtc` | `ax6` | CGB MBC3 RTC AX6 framebuffer rows. |

## Extra and exploratory target catalog

| Target or command | Suite(s) | Report channel | Purpose |
| --- | --- | --- | --- |
| `make run-ax6` | `ax6-dmg-extra` | `/test/test-report-extra.md` | Extra/internal DMG MBC3 RTC AX6 rows. |
| `make run-samesuite` | `samesuite-dmg-extra` | `/test/test-report-extra.md` | Extra/internal DMG SameSuite APU/interrupt rows. |
| `make run-mooneye-sgb-boot-regs` | `mooneye-sgb-boot-regs-extra` | `/test/test-report-extra.md` | SGB/SGB2 direct-start boot-register fingerprints. |
| `make run-little-things-gb` | `little-things-gb-dmg-extra` | `/test/test-report-extra.md` | DocBoy-sourced DMG `little-things-gb` rows. |
| `make run-gbmicrotest` | `gbmicrotest-dmg-extra` | `/test/test-report-extra.md` | Large DocBoy-sourced DMG memory-byte microtest corpus. |
| `make run-cgb-boot-hwio` | `cgb-boot-hwio` | `/test/test-report-extra.md` | Extra/internal CGB boot HWIO fingerprint row. |
| `make run-mooneye-cgb` | `mooneye-cgb-extra` | `/test/test-report-extra.md` | Extra/internal Mooneye CGB PPU acceptance subset. |
| `make run-samesuite-cgb` | `samesuite-cgb-extra` | `/test/test-report-extra.md` | Extra/internal DocBoy-sourced SameSuite CGB variant rows. |
| `make run-magen-cgb` | `magen-cgb-extra` | `/test/test-report-extra.md` | Extra/internal DocBoy-sourced Magen CGB rows. |
| `make run-mealybug-cgb` | `mealybug-tearoom-cgb-extra` | `/test/test-report-extra.md` | CGB companion of the Mealybug PPU rows, kept out of promoted CGB closure until promoted intentionally. |
| `make run-little-things-gb-cgb` | `little-things-gb-cgb-extra` | `/test/test-report-extra.md` | CGB `whichboot.gb` startup/custom-boot evidence split out of DocBoy native CGB. |
| `make run-docboy-dmg` | `docboy-dmg-extra` plus `docboy-dmg-linked-extra` | `/test/test-report-docboy.md` plus linked stdout/artifacts | Large DocBoy DMG single-machine corpus plus serial two-player linked rows. |
| `make run-docboy-cgb` | `docboy-cgb-extra` | `/test/test-report-docboy.md` | Large DocBoy native-CGB corpus; intentionally outside promoted CGB and GitHub ROM gates. |
| `make run-docboy-cgb-dmg` | `docboy-cgb-dmg-extra` | `/test/test-report-docboy.md` | Large DocBoy CGB GB-compatible corpus; red bring-up lane until compatibility gaps are closed. |
| `make run-docboy-cgb-dmg-ext` | `docboy-cgb-dmg-ext-extra` | `/test/test-report-docboy.md` | Narrow experimental CGB DMG-ext register-profile lane. |
| `cargo run -p gb-test-runner --bin run_linked_session -- --suite linked-cgb-ir-smoke` | `linked-cgb-ir-smoke` | stdout/artifacts only | Internal synthetic CGB-to-CGB IR smoke; not part of Make aggregates or GitHub ROM workflows. |
| `cargo run -p gb-test-runner --bin run_linked_session -- --suite linked-dmg04-smoke` | `linked-dmg04-smoke` | stdout/artifacts only | Internal synthetic DMG-04 cable smoke used by runner/core tests. |
| `cargo run -p gb-test-runner --bin run_linked_session -- --suite linked-dmg04-contracts` | `linked-dmg04-contracts` | stdout/artifacts only | Internal DMG-04 participant-oracle contract suite. |
| `cargo run -p gb-test-runner --bin run_linked_session -- --suite linked-dmg07-smoke` | `linked-dmg07-smoke` | stdout/artifacts only | Internal DMG-07 adapter topology smoke. |

## Direct runner usage

```bash
cargo run -p gb-test-runner --bin run_rom_suite -- --list-detailed
cargo run -p gb-test-runner --bin run_rom_suite -- --suite cgb-dma --failure-artifact-root test/gb-emulator-shootout/.artifacts/cgb-dma
cargo run -p gb-test-runner --bin run_rom_suite -- --suite mooneye-cgb-extra --case mooneye-cgb-ppu-intr-2-mode0-timing-sprites
cargo run -p gb-test-runner --bin run_linked_session -- --suite docboy-dmg-linked-extra --failure-artifact-root test/.artifacts/docboy-dmg-linked
cargo run -p gb-test-runner --bin run_rom_suite -- --manifest .artifacts/local-private-smoke.toml
```

Use `--failure-artifact-root` whenever a failing row may need screenshots, memory text, snapshots, traces, or linked-session participant artifacts for diagnosis. Makefile ROM-suite targets go through the shared `RUN_ROM_SUITE` wrapper, default to `ROM_PROFILE=release-max`, and write promoted GB Emulator Shootout artifacts below `test/gb-emulator-shootout/.artifacts/` or legacy artifacts below `test/.artifacts/`; override locally with `ROM_PROFILE=release make <target>` only when iterating on compile time rather than final timing evidence.

## RealBoot policy

RealBoot ROM-suite runs require `GB_CYCLE_BOOT_ROM_ROOT` to point at private firmware assets with canonical filenames derived from the manifest revision, such as `dmg_boot.bin`, `mgb_boot.bin`, `cgb_boot.bin`, or `cgbE_boot.bin`. Default skip/custom-boot lanes do not read firmware bytes and must derive revision-specific behavior from manifest `revision` and the core `MachineConfig::revision` axis.

Use RealBoot aggregates as local comparison/closure evidence, not as a replacement for the default manifest startup lane. After a RealBoot run, rerun the matching non-RealBoot aggregate if you want the report file to represent the default skip/custom-boot baseline again.

## Reports and before/after workflow

The runner writes `family | rom | status` Markdown tables with `✅`, `❌`, and `ℹ️` rows plus a `non-failing/total` header summary. Promoted GB Emulator Shootout status files live under `/test/gb-emulator-shootout/.status/*.toml`; legacy extra/DocBoy status files still live under `/test/.status/*.toml`. Status files are interpreted by owning suite before shared upstream-family fallback, so reused ROMs can keep separate promoted, extra, and DocBoy labels.

When working on known external-ROM failures, timing regressions, exploratory PPU/MMIO fixes, or any ROM-driven change that may influence a go/no-go decision, copy the matching report before the work, rerun the suite, copy the final report, and compare changed rows explicitly before keeping the change. Use `/test/gb-emulator-shootout/test-report.md` for promoted GB Emulator Shootout suites, `/test/test-report-extra.md` for non-DocBoy extra/internal single-machine suites, and `/test/test-report-docboy.md` for large DocBoy single-machine suites.

Same-ROM model variants are ordered DMG before GBC before SGB before SGB2 when report suffixes are enabled. Empty report categories are not materialized; for example, a DocBoy-only run should not create or preserve empty promoted/extra report files.

## CI integration

- `make ci` remains the fast local pre-push gate and does not fetch or run external ROM suites; it covers formatting, linting, typos, dependency policy, workspace tests, and coverage gates.
- GitHub `ci` mirrors the Rust checks and coverage gate.
- GitHub `test-roms` fans out promoted DMG/SGB and promoted CGB targets as matrix children: `acid`, Blargg chunks, `daid`, `ashiepaws`, `cpp`, `cpp-sgb`, `samesuite-sgb`, Mooneye chunks, `mealybug`, `cgb-smoke`, `cgb-boot-div`, `cgb-speed`, `cgb-ppu-basic`, `cgb-ppu-hard`, `cgb-dma`, `cgb-audio-blargg`, `cgb-audio-samesuite`, and `cgb-rtc`.
- GitHub `test-roms-extra` fans out the green non-DocBoy extra/internal targets: `ax6`, `samesuite`, `little-things-gb`, `gbmicrotest`, `mooneye-sgb-boot-regs`, `cgb-boot-hwio`, `mooneye-cgb`, `samesuite-cgb`, `magen-cgb`, `mealybug-cgb`, and `little-things-gb-cgb`.
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
- `GB_CYCLE_TEST_ROM_ROOT` — optional override for the global materialized ROM-store root; if unset, `gb-test-runner` uses the default `/test/` store. Report-scoped suites append the report ID below this root, so `GB_CYCLE_TEST_ROM_ROOT=/tmp/roms` makes the promoted report store `/tmp/roms/gb-emulator-shootout/`, not `/tmp/roms/` directly.
- `GB_CYCLE_TEST_ROM_STARTUP` — local startup override for `run_rom_suite` and `run_linked_session`; omit it to preserve manifest startup, use `skip-boot` or `custom-boot` for direct-start lanes, and use `real-boot` only with `GB_CYCLE_BOOT_ROM_ROOT`.
