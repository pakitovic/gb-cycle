# gb-cycle

A hardware-accuracy-focused Game Boy emulator written in Rust.

The core's base temporal unit is `1 T-cycle`, advanced on one shared scheduler timeline.
The target CPU foundation is a `fetch / decode / execute` core with real bus accesses.
The target graphics foundation is a `dot-by-dot` PPU with `tile fetcher + pixel FIFO`.

## Goals

- Prioritize behavior faithful to real hardware.
- Keep the core portable and decoupled from any frontend.
- Make validation through tests and reference ROMs straightforward.
- Build the core from the start on a `T-cycle` timeline, not an `M-cycle` timeline.
- Model the CPU from the start as a real fetch/decode/execute flow, not as opaque opcodes with aggregated duration.
- Model the PPU from the start as a real pipeline, not as a scanline renderer.

## Current structure

The canonical structure and ownership boundaries are defined in `AI/ARCHITECTURE.md`.
If this summary differs from `AI/ARCHITECTURE.md`, `AI/ARCHITECTURE.md` takes precedence.
The current workspace already uses the `crates/`-based layout, leaving other components as future extensions.

```text
crates/
  gb-core/    Pure emulation logic
  gb-test-runner/  Typed ROM harness, executable suites, and validation helpers
  gb-cli/     Current CLI frontend
  gb-persistence/  Host-side cartridge save backends and format
AI/           Architecture, roadmap, and technical documentation
Makefile      Local verification pipeline and utilities
```

Mid-term planned extensions, not yet materialized as separate crates:

- `gb-desktop`
- `gb-web`
- additional tooling such as richer debugger and utilities
- broader integration tests and ROM suites

### Requirements

- Rust `1.93.1` via `rustup`
- Workspace MSRV: `1.93`

## Tooling

This repository uses:

- `rustfmt` for formatting
- `clippy` for linting
- `cargo-llvm-cov` for coverage
- `cargo-deny` for dependency, advisory and license checks
- `typos` for spellchecking

### Install local tooling

```bash
make setup
```

`make setup` configures the repository git hooks and installs the required local cargo tools:

- `cargo-llvm-cov`
- `cargo-deny`
- `typos-cli`

### Coverage

```bash
make coverage
cargo cov
cargo cov-check
cargo cov-html
cargo cov-lcov
```

`cargo cov-check` currently gates aggregate `>=90%` line, region, and function
coverage across `gb-core`, `gb-test-runner`, and `gb-persistence`.

### Full local pipeline

```bash
make ci
make test-roms
make coverage
```

Before opening or updating a PR, run at least `make ci` locally.
When changing CI, coverage, dependency policy, repo tooling, or the external ROM workflow, run `make test-roms` and `make coverage` locally as well so the external DMG gate and coverage pipeline do not first fail in GitHub Actions.
`make` defaults to `make ci`, and the configured pre-push hook also runs `make ci`.
Use Conventional Commits for commit messages and PR titles so the repository history and review metadata follow the same naming scheme.

### External ROM suites

The repository keeps synthetic ROM fixtures under version control, but official
external ROM suites stay outside git in a repo-managed local store.

```bash
make fetch-test-roms
make fetch-test-roms FAMILIES=blargg
make fetch-test-roms FAMILIES="blargg acid"
make test-roms
make test-roms-all
make run-blargg
make run-acid
make run-daid
make run-hacktix
make run-mealybug
make run-mooneye
```

- `make fetch-test-roms` fetches the pinned upstream source from
  `GBEmulatorShootout` into a temporary checkout, materializes the curated
  runnable store under `/.roms/test/`, and removes the raw checkout afterwards;
  by default it fetches `all`, but it can also materialize one or more
  explicit families through `FAMILIES=...`
- the pinned upstream source for redistributable ROM suites is now always
  `GBEmulatorShootout`, recorded in
  `crates/gb-test-runner/data/sources.toml`
- `/.roms/test/` is organized by family, for example:
  `/.roms/test/acid/`,
  `/.roms/test/blargg/`,
  `/.roms/test/daid/`,
  `/.roms/test/hacktix/`,
  `/.roms/test/mealybug-tearoom-tests/`,
  `/.roms/test/mooneye/`
- each curated family directory contains only the ROMs currently listed in the
  matching manifest under `crates/gb-test-runner/data/*.toml`
- the runner updates `/.roms/test/test-report.md` with a simple
  `family | rom | status` table when a curated family suite executes, using
  `✅`, `❌` and `ℹ️` in the status column and keeping each family's curated ROM
  order from the manifest
- repo-managed local-only support assets now also live under gitignored roots
  inside the workspace:
  `/.roms/bootrom/` for DMG/MGB boot ROM images and
  `/.oracles/<oracle>/<layout>/` for imported differential oracle artifacts
- `make ci` stays as the fast local pre-push gate and does not fetch or run
  external ROM suites; it includes the Rust checks plus the coverage threshold
  gate through `cargo cov-check`
- `make test-roms` fetches the curated ROM store if needed and runs the
  repository-gated green external DMG block; that block intentionally includes
  only the currently supported non-APU, non-CGB suites
- `make coverage` emits `lcov.info` through `cargo cov-lcov`
- GitHub uses two workflows:
  `ci` for Rust checks plus coverage
  `test-roms` for the supported external DMG block
- `make test-roms` runs the same repository-gated external DMG block explicitly:
  the curated supported Blargg DMG family
  `blargg-dmg-curated`
  and the curated Acid DMG family
  `acid-dmg-curated`
- the curated Acid DMG family mixes one blocking framebuffer oracle
  `dmg-acid2.gb` with one informational framebuffer capture case `which.gb`,
  matching the upstream `GBEmulatorShootout` classification
- `make run-blargg` runs the curated supported Blargg DMG family
- `make run-acid` runs the curated supported Acid DMG family
- `make run-daid` runs the current exploratory `daid` DMG subset and updates
  `/.roms/test/test-report.md`
- each `make run-*` target is autosufficient and materializes its own curated
  family before execution
- `make run-hacktix` runs the current exploratory `hacktix` DMG subset and
  updates `/.roms/test/test-report.md`
- `make run-mealybug` runs the current exploratory `mealybug-tearoom` DMG
  subset and updates `/.roms/test/test-report.md`
- `make run-mooneye` runs the current exploratory `mooneye` DMG acceptance
  subset and updates `/.roms/test/test-report.md`
- the current curated Blargg family intentionally uses only individual ROMs
  from `GBEmulatorShootout`; it does not use multi-ROM bundles such as
  `cpu_instrs.gb`
- the upstream `oam_bug/7-timing_effect.gb`, APU suites, CGB-only ROMs, and
  other still-red cases stay outside the default managed block until they are
  intentionally promoted
- one exploratory `mealybug-tearoom` DMG subset is also integrated as
  `mealybug-tearoom-dmg-curated`, but it is currently outside the default
  gate because it still diverges from the upstream framebuffer fixtures under
  `Strict`
- one exploratory `mooneye` DMG acceptance subset is also integrated as
  `mooneye-acceptance-dmg-curated`; it follows the active
  `GBEmulatorShootout` `testroms/mooneye.py` acceptance list, uses the upstream
  `mooneye` breakpoint/register result protocol instead of framebuffer oracles,
  and stays outside the default gate while the remaining failures are being
  triaged
- one exploratory `daid` DMG subset is also integrated as `daid-dmg-curated`;
  it mixes framebuffer fixtures, one multi-fixture framebuffer oracle for
  `ppu_scanline_bgp.gb`, and one informational framebuffer capture case
  `rom_and_ram.gb`
- one exploratory `hacktix` DMG subset is also integrated as
  `hacktix-dmg-curated`; it currently tracks `bully.gb` and
  `strikethrough.gb` from `GBEmulatorShootout` and uses framebuffer fixtures
- if `GB_CYCLE_TEST_ROM_ROOT` is unset, `gb-test-runner` falls back to the
  default curated store automatically
- keep private commercial ROMs out of that path; use the separate gitignored
  `/.roms/local-commercial/` directory for local-only assets that must never be
  referenced by CI
- to audit the current built-in suites and their oracle channels without reading
  the source, run:

```bash
cargo run -p gb-test-runner --bin run_rom_suite -- --list-detailed
```

- to audit the current early hardening status by subsystem, run:

```bash
cargo run -p gb-test-runner --bin run_rom_suite -- --early-checklist
```

- to run the curated Acid DMG family directly once the test store is
  materialized, run:

```bash
cargo run -p gb-test-runner --bin run_rom_suite -- --suite acid-dmg-curated
```

- to run the curated supported Blargg DMG family, run:

```bash
cargo run -p gb-test-runner --bin run_rom_suite -- --suite blargg-dmg-curated
```

- to run the current exploratory `mealybug-tearoom` DMG subset and retain its
  mismatch artifacts, run:

```bash
cargo run -p gb-test-runner --bin run_rom_suite -- \
  --suite mealybug-tearoom-dmg-curated \
  --failure-artifact-root .artifacts/mealybug-curated
```

- to run the current exploratory `mooneye` DMG acceptance subset and retain the
  failing snapshots, run:

```bash
cargo run -p gb-test-runner --bin run_rom_suite -- \
  --suite mooneye-acceptance-dmg-curated \
  --failure-artifact-root .artifacts/mooneye-acceptance
```

- to compare one built-in suite against imported SameBoy artifacts,
  run:

```bash
cargo run -p gb-test-runner --bin run_differential -- \
  --oracle sameboy \
  --oracle-layout sameboy-tester \
  --suite acid-dmg-curated
```

  If `--oracle-artifact-root` is omitted, the default repo-local root is
  `/.oracles/<oracle>/<layout>/`, so for this example the default is
  `/.oracles/sameboy/sameboy-tester/`.

  The default layout is `case-bundle`, where the oracle root contains one
  subdirectory per case id using the same artifact filenames that
  `gb-test-runner` already emits locally, such as `serial.txt`,
  `memory_text_output.txt`, `blargg_console.txt`, `framebuffer.png`, or
  `trace.txt`.

  The `sameboy-tester` layout is currently framebuffer-only. It expects SameBoy
  Tester artifacts mirrored by ROM-relative path, for example
  `acid/dmg-acid2.bmp` under the oracle root.

- to materialize those SameBoy Tester artifacts under a compatible oracle root,
  run:

```bash
cargo run -p gb-test-runner --bin run_sameboy_tester -- \
  --sameboy-root /path/to/SameBoy \
  --suite acid-dmg-curated \
  --image-format bmp \
  --build-if-missing
```

  This stages ROMs under the default repo-local oracle root
  `/.oracles/sameboy/sameboy-tester/`, runs SameBoy's internal `tester`
  binary, and leaves `.bmp` / `.tga` plus `.log` artifacts there in the
  `sameboy-tester` layout that `run_differential` can consume directly.
  SameBoy Tester always boots through a boot ROM, so this path is best suited
  to end-of-test framebuffer convergence rather than boot-path arbitration.
  The current wrapper intentionally does not override SameBoy's boot-ROM path.
  If you need a specific SameBoy firmware choice for oracle generation, control
  it from the SameBoy checkout or build itself rather than through
  `gb-test-runner`.


## Documentation

Before implementing subsystems, read the main handbooks in `AI/` first:

- `AI/index.md`
- `AI/ARCHITECTURE.md`
- `AI/CODING-RULES.md`
- `AI/EXECUTION.md`
- `AI/REFERENCES.md`
- `AI/ROADMAP.md`
- `AI/TESTING.md`
- `AI/TIMING-AND-ACCURACY.md`
- `AI/hardware/*.md`

The documentation hierarchy, in summary, is:

- `AI/index.md` as the entry point for reading order and document authority boundaries
- `AI/ARCHITECTURE.md` for layout, ownership, and subsystem boundaries
- `AI/ARCHITECTURE.md` also for the central compatibility-policy structure, execution-mode ownership boundaries, and the top-level separation between cartridge persistence and full emulator save states
- `AI/TIMING-AND-ACCURACY.md` for shared timing vocabulary and project-wide timing constraints
- `AI/ARCHITECTURE.md` plus `AI/TIMING-AND-ACCURACY.md` together for the global T-cycle scheduler contract
- `AI/EXECUTION.md` and `AI/CODING-RULES.md` for workflow and code-change discipline
- `AI/REFERENCES.md` for source and oracle consultation order
- `AI/hardware/*.md` for the behavior and contracts of the corresponding subsystem
- `AI/hardware/CARTRIDGES-MBC.md` specifically for mapper classification, special-cartridge taxonomy, cartridge-side compatibility-category policy, and cartridge persistence rules distinct from full emulator save states
- `AI/TESTING.md` for the global validation, differential, determinism, DMG-hardening policy, and official `Strict` CI/oracle usage
- `AI/ROADMAP.md` for recommended implementation order, phase context, and outstanding TODOs

Use `AI/research/*.md` as secondary comparison material when you need implementation examples, additional validation, or comparison against reference oracles.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
