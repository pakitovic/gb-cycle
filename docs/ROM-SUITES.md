# External ROM suites

The repository keeps synthetic ROM fixtures under version control, but official external ROM suites stay outside git in a repo-managed local store.

## Fetching ROMs

```bash
make fetch-test-roms
make fetch-test-roms FAMILIES=blargg
make fetch-test-roms FAMILIES="blargg acid"
```

- `make fetch-test-roms` fetches the pinned upstream source from `GBEmulatorShootout` into a temporary checkout, materializes the curated runnable store under `/.roms/test/`, and removes the raw checkout afterwards.
- By default it fetches `all`, but it can materialize one or more explicit families through `FAMILIES=...`.
- The pinned upstream source is always `GBEmulatorShootout`, recorded in `crates/gb-test-runner/data/sources.toml`.

## ROM store layout

`/.roms/test/` is organized by family:

```text
/.roms/test/acid/
/.roms/test/blargg/
/.roms/test/daid/
/.roms/test/hacktix/
/.roms/test/mealybug-tearoom-tests/
/.roms/test/mooneye/
```

Each curated family directory contains only the ROMs currently listed in the matching manifest under `crates/gb-test-runner/data/*.toml`.

## Running suites

```bash
make test-roms         # fetch if needed + run all local curated DMG suites
make run-blargg        # curated Blargg DMG family (includes dmg_sound 01..12)
make run-acid          # curated Acid DMG family
make run-daid          # exploratory daid DMG subset
make run-cpp           # curated cpp MBC3 subset
make run-hacktix       # curated hacktix DMG subset
make run-mealybug      # exploratory mealybug-tearoom DMG subset
make run-mooneye       # exploratory mooneye DMG acceptance subset
```

Each `make run-*` target is autosufficient and materializes its own curated family before execution.

### Direct runner invocations

```bash
# Run a specific built-in suite
cargo run -p gb-test-runner --bin run_rom_suite -- --suite acid-dmg-curated

# Run full Blargg family (including dmg_sound)
cargo run -p gb-test-runner --bin run_rom_suite -- --suite blargg-dmg-curated

# List all built-in suites and oracle channels
cargo run -p gb-test-runner --bin run_rom_suite -- --list-detailed

# Show early hardening status by subsystem
cargo run -p gb-test-runner --bin run_rom_suite -- --early-checklist
```

### Retaining failure artifacts

```bash
# Mealybug mismatch artifacts
cargo run -p gb-test-runner --bin run_rom_suite -- \
  --suite mealybug-tearoom-dmg-curated \
  --failure-artifact-root .artifacts/mealybug-curated

# Mooneye failing snapshots
cargo run -p gb-test-runner --bin run_rom_suite -- \
  --suite mooneye-acceptance-dmg-curated \
  --failure-artifact-root .artifacts/mooneye-acceptance
```

## Test report

The runner updates `/.roms/test/test-report.md` with a `family | rom | status` table when a curated family suite executes, using `✅`, `❌` and `ℹ️` in the status column, adding a `non-failing/total` summary in the header, and keeping each family's curated ROM order from the manifest.

## Curated family details

### Acid

Mixes one blocking framebuffer oracle `dmg-acid2.gb` with one informational framebuffer capture case `which.gb`, matching the upstream `GBEmulatorShootout` classification.

### Blargg

Uses only individual ROMs from `GBEmulatorShootout` (not multi-ROM bundles such as `cpu_instrs.gb`). Includes the DMG `dmg_sound 01..12` individual ROMs.

The upstream `oam_bug/7-timing_effect.gb`, CGB-only ROMs, and other still-red cases stay outside the default managed block until intentionally promoted.

### Hacktix

Tracks `bully.gb` and `strikethrough.gb` from `GBEmulatorShootout`, uses framebuffer fixtures; exercised by the GitHub `test-roms` workflow.

### Cpp

Curated `cpp` MBC3 subset; exercised by the GitHub `test-roms` workflow.

### Daid

Exploratory DMG subset; mixes framebuffer fixtures, one multi-fixture framebuffer oracle for `ppu_scanline_bgp.gb`, and one informational framebuffer capture case `rom_and_ram.gb`.

### Mealybug-tearoom

Exploratory DMG subset; remains outside the GitHub `test-roms` workflow because it still diverges from upstream framebuffer fixtures under `Strict`.

### Mooneye

Exploratory DMG acceptance subset following the active `GBEmulatorShootout` `testroms/mooneye.py` acceptance list. Uses the upstream `mooneye` breakpoint/register result protocol instead of framebuffer oracles. Stays outside the GitHub `test-roms` workflow until remaining failures are triaged.

## CI integration

- `make ci` stays as the fast local pre-push gate and does not fetch or run external ROM suites; it includes the Rust checks plus the coverage threshold gate through `cargo cov-check`.
- `make test-roms` fetches the curated ROM store if needed and runs all local curated DMG suites currently wired in `Makefile`: `acid`, `blargg`, `daid`, `hacktix`, `cpp`, `mealybug-tearoom-tests`, and `mooneye`.
- GitHub uses two workflows: `ci` for Rust checks plus coverage, `test-roms` for the workflow-managed ROM subset currently exercised in CI: `acid`, `blargg`, `hacktix`, and `cpp`.

## Commercial ROM testing

Keep private commercial ROMs out of the curated store; use the separate gitignored `/.roms/local-commercial/` directory for local-only assets that must never be referenced by CI.

For ad hoc local commercial-ROM bring-up, `run_rom_suite` accepts `--manifest <path>` with typed per-case metadata and deterministic joypad stimuli. When a manifest-driven case captures the framebuffer, the runner writes a sibling PNG next to the ROM using the ROM stem.

### Example manifest

```toml
version = 1

[[case]]
id = "tetris-dmg-start"
rom = ".roms/local-commercial/tetris.gb"
console = "dmg"
startup = "real-boot"
mode = "strict"
timeout_frames = 760
oracle = "info-framebuffer"

[[case.stimulus]]
frame = 650
button = "start"
pressed = true

[[case.stimulus]]
frame = 690
button = "start"
pressed = false
```

```bash
cargo run -p gb-test-runner --bin run_rom_suite -- --manifest .artifacts/tetris-start.toml
```

The final framebuffer PNG lands next to the ROM as `/.roms/local-commercial/tetris.png`.

## Differential oracle testing

### SameBoy Tester

To materialize SameBoy Tester artifacts under a compatible oracle root:

```bash
cargo run -p gb-test-runner --bin run_sameboy_tester -- \
  --sameboy-root /path/to/SameBoy \
  --suite acid-dmg-curated \
  --image-format bmp \
  --build-if-missing
```

This stages ROMs under the default repo-local oracle root `/.oracles/sameboy/sameboy-tester/`, runs SameBoy's internal `tester` binary, and leaves `.bmp` / `.tga` plus `.log` artifacts in the `sameboy-tester` layout that `run_differential` can consume directly.

SameBoy Tester always boots through a boot ROM, so this path is best suited to end-of-test framebuffer convergence rather than boot-path arbitration. The current wrapper intentionally does not override SameBoy's boot-ROM path. If you need a specific SameBoy firmware choice for oracle generation, control it from the SameBoy checkout or build itself rather than through `gb-test-runner`.

### Running differentials

```bash
cargo run -p gb-test-runner --bin run_differential -- \
  --oracle sameboy \
  --oracle-layout sameboy-tester \
  --suite acid-dmg-curated
```

If `--oracle-artifact-root` is omitted, the default repo-local root is `/.oracles/<oracle>/<layout>/`.

#### Layouts

- **`sameboy-tester`** — framebuffer-only; expects SameBoy Tester artifacts mirrored by ROM-relative path (e.g. `acid/dmg-acid2.bmp`).
- **`case-bundle`** (default) — oracle root contains one subdirectory per case id using the same artifact filenames that `gb-test-runner` emits locally (`serial.txt`, `memory_text_output.txt`, `blargg_console.txt`, `framebuffer.png`, `trace.txt`).

### Case-bundle oracle example

The built-in cartridge mapper oracle lane uses the `case-bundle` layout:

```bash
cargo run -p gb-test-runner --bin run_sameboy_case_bundle -- \
  --suite phase-6-cartridge-oracle \
  --sameboy-root /path/to/SameBoy \
  --build-if-missing

cargo run -p gb-test-runner --bin run_differential -- \
  --oracle sameboy \
  --suite phase-6-cartridge-oracle
```

That suite compares retained synthetic `MBC1`, `MBC2`, `MBC3`, and `MBC5` Phase `6` `serial_hex` artifacts, and its `MBC3` case includes explicit pre-run RTC advancement in the typed runner metadata.

## Other local-only assets

Repo-managed local-only support assets live under gitignored roots:

- `/.roms/bootrom/` — DMG/MGB boot ROM images.
- `/.oracles/<oracle>/<layout>/` — imported differential oracle artifacts.

## Environment variables

- `GB_CYCLE_BOOT_ROM_ROOT` — override boot ROM search path.
- `GB_CYCLE_TEST_ROM_ROOT` — override test ROM root; if unset, `gb-test-runner` falls back to the default curated store automatically.
