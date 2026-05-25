# Curated ROM Fixtures

This directory documents the repo-managed contract for redistributable external ROM suites used by `gb-test-runner`.

## Stores

- Curated runnable store: `/test/`
- Curated root env var override: `GB_CYCLE_TEST_ROM_ROOT`

`make fetch-test-roms` downloads the pinned upstream source(s) declared in `crates/gb-test-runner/data/sources.toml` into temporary checkout(s) and then materializes the curated runnable families under `/test/`. By default it fetches `all`, but it also accepts one or more explicit families through `FAMILIES=...`.
`make test-roms` performs that fetch/materialization step automatically before running all local curated DMG suites currently wired in the `Makefile`. The GitHub `test-roms` workflow runs the workflow-managed subset of those suites.

Most redistributable suite assets come from `GBEmulatorShootout`; source-specific exceptions such as DocBoy are also pinned in `sources.toml` and must declare their materialized family/ROM alias explicitly. The runnable store keeps only the ROMs currently listed in the family manifests under `crates/gb-test-runner/data/*.toml`.
The repo-managed framebuffer fixtures checked into this tree are stored as human-visible `PNG` images; the runner still accepts legacy `PGM` fixtures during the transition, but `PNG` is now the default checked-in oracle format.

DocBoy logical families use `docboy-dmg`, `docboy-cgb`, `docboy-cgb-dmg`, and `docboy-cgb-dmg-ext` in reports and manifests, while their runnable ROMs live together under `/test/docboy/{dmg,cgb,cgb-dmg,cgb-dmg-ext}/` so the experimental upstream matrix stays grouped on disk.

Each curated family directory contains:

- only the currently supported ROM assets for that family

The runner updates `/test/test-report.md` with a simple `family | rom | status` table after promoted curated family runs, while non-DocBoy extra/internal suites such as `ax6-dmg-extra`, `samesuite-dmg-extra`, `samesuite-cgb-extra`, `magen-cgb-extra`, `little-things-gb-cgb-extra`, `gbmicrotest-dmg-extra`, `little-things-gb-dmg-extra`, and `cgb-boot-hwio` update `/test/test-report-extra.md`, and the large DocBoy single-machine suites `docboy-dmg-extra`, `docboy-cgb-extra`, `docboy-cgb-dmg-extra`, and `docboy-cgb-dmg-ext-extra` update `/test/test-report-docboy.md`; all reports use `✅`, `❌`, and `ℹ️` in the status column. The `# Test Report (...)` header also summarizes `non-failing/total` across the exact persisted rows rendered in that file, counting both `PASS` and `INFO` in the numerator, so a first partial run reports only its own rows while later partial updates keep the full persisted context. The report keeps a fixed family inventory order: `acid`, `blargg`, `daid`, `ax6`, `mooneye`, `samesuite`, `magen`, `gbmicrotest`, `docboy-dmg`, `docboy-cgb`, `docboy-cgb-dmg`, `docboy-cgb-dmg-ext`, `hacktix`, `cpp`, `mealybug-tearoom-tests`, `little-things-gb`. Families that have not produced persisted case statuses do not appear in the table.

## Current curated families

- `acid-dmg-curated`
  source family: `acid`
  current status: workflow-managed
  oracle mix: framebuffer fixture plus informational framebuffer capture
- `blargg-dmg-curated`
  source family: `blargg`
  current status: workflow-managed
  oracle mix: serial, Blargg BG-map console text, and cartridge RAM text
  supported scope: individual ROMs only
  current green set: `cpu_instrs 01..11`, `halt_bug`,
  `mem_timing 01..03`, `mem_timing-2 01..03`, `oam_bug 1..6,8`, and
  `dmg_sound 01..12`
- `ax6-dmg-extra`
  source family: `ax6`
  current status: extra/internal local-only
  oracle mix: framebuffer fixture
  fixture ownership: committed DMG grayscale fixtures `rtc3test-*.dmg.png` generated from the DMG runner output, separate from the upstream AX6 CGB PNG references used by `cgb-rtc`
- `samesuite-dmg-extra`
  source family: `samesuite`
  current status: extra/internal local-only
  oracle mix: framebuffer fixture
  fixture ownership: shares the upstream SameSuite `div_write_trigger*.png` PNG references with `cgb-audio-samesuite`; DocBoy `interrupt/ei_delay_halt.png` is committed locally
- `samesuite-cgb-extra`
  source family: `samesuite`
  current status: extra/internal local-only
  oracle mix: RGB555 framebuffer fixture
  fixture ownership: committed DocBoy-sourced CGB SameSuite PNG fixtures under `samesuite-cgb/`
- `magen-cgb-extra`
  source family: `magen`
  current status: extra/internal local-only
  oracle mix: RGB555 framebuffer until-match fixtures
  fixture ownership: committed DocBoy-sourced Magen CGB PNG fixtures under `magen/`
- `little-things-gb-dmg-extra`
  source family: `little-things-gb`
  current status: extra/internal local-only
  oracle mix: framebuffer fixture
  fixture ownership: committed DocBoy DMG fixtures for `double-halt-cancel.png` and `whichboot.png`; `whichboot.gb` uses `startup = "custom-boot"` so its boot-logo/map checks see the core DMG boot-logo VRAM seed without requiring private boot ROM assets
- `little-things-gb-cgb-extra`
  source family: `little-things-gb`
  current status: extra/internal local-only
  oracle mix: framebuffer fixture
  fixture ownership: committed DocBoy CGB `whichboot.png` fixture under `little-things-gb-cgb/`; `whichboot.gb` uses `startup = "custom-boot"` on `console = "cgb"` and reports as `whichboot.gb (GBC)`
- `daid-dmg-curated`
  source family: `daid`
  current status: exploratory local-only
  oracle mix: framebuffer fixture, framebuffer fixture set, and informational framebuffer capture
- `hacktix-dmg-curated`
  source family: `hacktix`
  current status: workflow-managed and Phase 9 LibSameBoy differential matched
  oracle mix: framebuffer fixture
- `cpp-dmg-curated`
  source family: `cpp`
  current status: workflow-managed
  oracle mix: framebuffer fixture
- `mealybug-tearoom-dmg-curated`
  source family: `mealybug-tearoom-tests`
  current status: exploratory local-only
  oracle: framebuffer fixture
- `mealybug-tearoom-dmg-sameboy-differential`
  source family: `mealybug-tearoom-tests`
  current status: Phase 9 SameBoy-PASS differential subset
  oracle: framebuffer fixture
- `mealybug-tearoom-cgb-extra`
  source family: `mealybug-tearoom-tests`
  current status: extra/internal local-only
  oracle mix: RGB555 framebuffer fixtures
  fixture ownership: committed CGB fixtures under `mealybug-cgb/`; `m3_lcdc_win_en_change_multiple_wx.png` is an experimental real-CGB-C/D capture and the runner path-specifically quantizes its sparse near-black, near-green, and forced-white capture noise before RGB555 fixture comparison
- `mooneye-acceptance-dmg-curated`
  source family: `mooneye`
  current status: exploratory local-only
  oracle: Mooneye breakpoint/register result plus retained serial output
- `docboy-dmg-extra`
  source family: `docboy-dmg`
  current status: DocBoy extra/internal local-only
  oracle mix: memory-byte and framebuffer until-match fixtures
  fixture ownership: committed DocBoy `tests/results/dmg/docboy` PNG fixtures under `docboy-dmg/`; linked-session DocBoy rows retain participant artifacts but do not write markdown report rows
- `docboy-cgb-extra`
  source family: `docboy-cgb`
  current status: DocBoy CGB extra/internal local-only
  oracle mix: memory-byte and RGB555 framebuffer until-match fixtures
  fixture ownership: committed DocBoy `tests/results/cgb` PNG fixtures under `docboy-cgb/`; upstream-disabled rows without runnable ROM or fixture coverage stay disabled in the manifest
- `docboy-cgb-dmg-extra`
  source family: `docboy-cgb-dmg`
  current status: DocBoy CGB extra/internal local-only
  oracle mix: memory-byte and framebuffer until-match fixtures
  fixture ownership: committed DocBoy `tests/results/cgb_dmg_mode` PNG fixtures under `docboy-cgb-dmg/`; the upstream-disabled Mealybug `m3_lcdc_win_en_change_multiple_wx` row has no committed fixture and remains disabled in the manifest
- `docboy-cgb-dmg-ext-extra`
  source family: `docboy-cgb-dmg-ext`
  current status: DocBoy CGB extra/internal local-only
  oracle mix: memory-byte
  fixture ownership: no framebuffer fixtures; the suite exercises the narrow DocBoy CGB DMG-ext memory-result rows

## Commands

Fetch and materialize the curated store:

```bash
make fetch-test-roms
make fetch-test-roms FAMILIES=blargg
make fetch-test-roms FAMILIES="blargg acid"
```

Run all local curated DMG suites wired into `make test-roms`:

```bash
make test-roms
```

Run one family directly through `make`:

```bash
make run-daid
make run-cpp
make run-blargg
make run-acid
make run-hacktix
make run-mealybug
make run-mooneye
make run-little-things-gb
make run-little-things-gb-cgb
make run-samesuite-cgb
make run-magen-cgb
make run-docboy-dmg
make run-docboy-cgb
make run-docboy-cgb-dmg
make run-docboy-cgb-dmg-ext
make test-roms-docboy
```

Each `make run-*` target materializes its own family before executing and updates `/test/test-report.md` for promoted suites, `/test/test-report-extra.md` for non-DocBoy extra/internal suites, or `/test/test-report-docboy.md` for the large DocBoy single-machine suites. The currently exploratory local-only families include `ax6`, `daid`, `docboy-dmg`, `docboy-cgb`, `docboy-cgb-dmg`, `docboy-cgb-dmg-ext`, `little-things-gb`, `magen`, `mealybug-tearoom-tests`, `mooneye`, and `samesuite` (DMG and CGB extra variants).

Run one curated family directly and update the report:

```bash
cargo run -p gb-test-runner --bin run_rom_suite -- --suite blargg-dmg-curated
```

Run the curated Acid family and retain mismatch artifacts:

```bash
cargo run -p gb-test-runner --bin run_rom_suite -- \
  --suite acid-dmg-curated \
  --failure-artifact-root .artifacts/acid
```

Compare the curated Acid family against LibSameBoy case-bundle artifacts:

```bash
cargo run -p gb-test-runner --bin run_differential -- \
  --oracle sameboy \
  --oracle-layout case-bundle \
  --suite acid-dmg-curated
```

Compare the SameBoy-PASS Mealybug subset against LibSameBoy case-bundle artifacts:

```bash
cargo run -p gb-test-runner --bin run_differential -- \
  --oracle sameboy \
  --oracle-layout case-bundle \
  --suite mealybug-tearoom-dmg-sameboy-differential
```

Compare the curated Hacktix family against LibSameBoy case-bundle artifacts:

```bash
cargo run -p gb-test-runner --bin run_differential -- \
  --oracle sameboy \
  --oracle-layout case-bundle \
  --suite hacktix-dmg-curated
```

Capture Hacktix first-divergence probe windows against LibSameBoy:

```bash
cargo run -p gb-test-runner --bin run_first_divergence -- \
  --oracle sameboy \
  --suite hacktix-dmg-curated \
  --probe-interval-tcycles 70224 \
  --build-if-missing \
  --allow-divergence
```

The `case-bundle` oracle layout stores one directory per case id under
`/.oracles/sameboy/case-bundle/`, for example `dmg-acid2/framebuffer.pgm`.

Materialize those SameBoy artifacts:

```bash
cargo run -p gb-test-runner --bin run_sameboy_case_bundle -- \
  --sameboy-root /path/to/SameBoy \
  --suite acid-dmg-curated \
  --build-if-missing
```

Commercial or otherwise non-redistributable ROMs do not belong in these stores. Keep them outside the repository in private developer-owned locations, reference them only through explicit local manifests or environment-rooted paths, and keep them out of CI.
