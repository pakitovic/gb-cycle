# External ROM Fixtures

This directory documents the repo-managed contract for redistributable external
ROM suites used by `gb-test-runner`.

## Stores

- Curated runnable store: `/.roms/test/`
- Curated root env var override: `GB_CYCLE_TEST_ROM_ROOT`

`make fetch-test-roms` downloads the pinned upstream source declared in
`crates/gb-test-runner/test-rom-families/sources.toml` into a temporary checkout and
then materializes the curated runnable families under `/.roms/test/`. By
default it fetches `all`, but it also accepts one or more explicit families
through `FAMILIES=...`.
`make test` performs that fetch/materialization step automatically before
running the repo-gated external DMG block.

The upstream source for redistributable suites is `GBEmulatorShootout`. The
runnable store keeps only the ROMs currently listed in the family manifests
under `crates/gb-test-runner/test-rom-families/*.toml`.

Each curated family directory contains:

- only the currently supported ROM assets for that family

The runner updates `/.roms/test/test-report.md` with a simple
`family | rom | status` table after curated family runs, using `✅`, `❌`, and
`ℹ️` in the status column. The report keeps a fixed family inventory order:
`acid`, `blargg`, `daid`, `ax6`, `mooneye`, `samesuite`, `hacktix`, `cpp`,
`mealybug-tearoom-tests`. Families that have not produced persisted case
statuses do not appear in the table.

## Current curated families

- `acid-dmg-curated`
  source family: `acid`
  current repo-gated status: green
  oracle mix: framebuffer fixture plus informational framebuffer capture
- `blargg-dmg-curated`
  source family: `blargg`
  current repo-gated status: green
  oracle mix: serial, Blargg BG-map console text, and cartridge RAM text
  supported scope: individual ROMs only
  current green set: `cpu_instrs 01..11`, `halt_bug`,
  `mem_timing 01..03`, `mem_timing-2 01..03`, and `oam_bug 1..6,8`
- `mealybug-tearoom-dmg-curated`
  source family: `mealybug-tearoom-tests`
  current status: exploratory, not repo-gated
  oracle: framebuffer fixture
- `mooneye-acceptance-dmg-curated`
  source family: `mooneye`
  current status: exploratory, not repo-gated
  oracle: Mooneye breakpoint/register result plus retained serial output

## Commands

Fetch and materialize the curated store:

```bash
make fetch-test-roms
make fetch-test-roms FAMILIES=blargg
make fetch-test-roms FAMILIES="blargg acid"
```

Run the repo-gated external DMG block:

```bash
make test
```

Run the exploratory curated Mealybug or Mooneye families:

```bash
make test-mealybug
make test-mooneye
```

Those exploratory targets update `/.roms/test/test-report.md` even when the
suite still contains failing ROM cases, and each `make test-*` target
materializes its own family before executing.

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

Compare the curated Acid family against SameBoy tester artifacts:

```bash
cargo run -p gb-test-runner --bin run_differential -- \
  --oracle sameboy \
  --oracle-layout sameboy-tester \
  --suite acid-dmg-curated
```

The `sameboy-tester` oracle layout mirrors ROM-relative paths under
`/.oracles/sameboy/sameboy-tester/`, for example `acid/dmg-acid2.bmp`.

Materialize those SameBoy artifacts:

```bash
cargo run -p gb-test-runner --bin run_sameboy_tester -- \
  --sameboy-root /path/to/SameBoy \
  --suite acid-dmg-curated \
  --image-format bmp \
  --build-if-missing
```

Commercial or otherwise non-redistributable ROMs do not belong in these
stores. Keep them under the separate gitignored `/.roms/local-commercial/`
root and out of CI.
