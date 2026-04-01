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

The canonical structure and ownership boundaries are defined in `docs/ARCHITECTURE.md`.
If this summary differs from `docs/ARCHITECTURE.md`, `docs/ARCHITECTURE.md` takes precedence.
The current workspace already uses the `crates/`-based layout, leaving other components as future extensions.

```text
crates/
  gb-core/    Pure emulation logic
  gb-test-runner/  Typed ROM harness, executable suites, and validation helpers
  gb-cli/     Current CLI frontend
  gb-desktop/ SDL3 desktop frontend
  gb-persistence/  Host-side cartridge save backends and format
docs/           Architecture, roadmap, and technical documentation
Makefile      Local verification pipeline and utilities
```

Mid-term planned extensions, not yet materialized as separate crates:

- `gb-web`
- additional tooling such as richer debugger and utilities
- broader integration tests and ROM suites

### `gb-cli`

The workspace now includes a headless CLI runner for the DMG family:

```bash
cargo run -p gb-cli -- inspect-rom path/to/rom.gb
cargo run -p gb-cli -- run path/to/rom.gb --tcycles 5000 --serial-out .artifacts/serial.bin
```

- `run` currently exposes the DMG-family models `dmg0`, `dmg`, and `mgb`
- `run` supports `skip-boot` and `real-boot`, plus `strict`, `permissive`, and `experimental` compatibility modes
- `real-boot` looks for boot ROM assets in `GB_CYCLE_BOOT_ROM_ROOT` or the repo-local `/.roms/bootrom/` store and can verify the expected DMG-family SHA-256 hashes
- `--framebuffer-out` writes the final `160x144` framebuffer as a binary PGM image, or as a real PNG when the output path ends in `.png`
- `--trace-out` writes the in-memory scheduler trace text for the run
- `--save-dir` loads and stores battery-backed cartridge persistence using the host-side `.gbsav` format from `gb-persistence`
- if neither `--frames` nor `--tcycles` is provided, `skip-boot` stops after `120` completed frames by default, while `real-boot` stops after boot-ROM handoff plus `120` completed post-handoff frames with a `480`-frame safety cap

### `gb-desktop`

The workspace now also includes an SDL3-based desktop frontend scaffold:

```bash
cargo run -p gb-desktop -- [path/to/rom.gb]
```

- it currently opens a desktop window, renders the live `160x144` framebuffer, maps keyboard and SDL3 gamepad input to the joypad path, supports basic gamepad hotplug plus preferred-device selection and remappable gamepad bindings, plays audio through SDL3, persists host-side desktop settings plus the last `Open ROM` directory and a recent-ROM history across runs, shows an in-window pause/menu overlay with native SDL3 `Open ROM` filtered to common Game Boy ROM extensions plus frontend-owned `video`, `audio`, `input`, and `system` submenus, exposes `OPEN RECENT` from the root overlay whenever recent ROMs exist, allows recent entries to relaunch directly from that submenu, allows in-window keyboard joypad rebinding from `INPUT -> KEYBOARD`, dedicated host-side keyboard menu rebinding from `INPUT -> KB MENU`, frontend hotkey rebinding from `INPUT -> HOTKEYS`, and SDL gamepad rebinding from `INPUT -> GAMEPAD` plus dedicated SDL gamepad menu rebinding from `INPUT -> PAD MENU` with immediate runtime effect, lets the overlay adjust frontend-owned `VIDEO` settings such as fullscreen, `vsync`, window scale, integer presentation, and in-window stats HUD visibility, lets `AUDIO` toggle mute and cycle host volume, exposes `DEFAULTS` reset actions inside `VIDEO`, `AUDIO`, and `INPUT` so the frontend can restore host-side settings and bindings without touching CLI config, shows the current active gamepad in the `GAMEPAD` submenu, can pin or clear the preferred device from that same UI, can move gamepad focus to the last used controller whenever no preferred device is currently locked, can start without a ROM and wait in a launcher-style root menu that stays open until a ROM is selected, and loads/saves battery-backed cartridge persistence
- desktop battery saves now default to a frontend-owned debounced auto-flush policy: once cartridge persistence changes, the frontend writes a safe replacement save after roughly `2s`, and still forces a flush on ROM changes and shutdown
- the `SAVE BATTERY` menu action is only exposed when the desktop save policy is explicitly set to `manual`; under the default auto-flush policies the overlay hides that action instead of showing a redundant manual save entry
- the window title now also reports live FPS, average frame time, relative emulation speed, and a frontend-side breakdown of average emulation, render, pacing, and audio-queue timing; the gameplay view also renders a compact in-window performance HUD with those same frontend metrics, and the HUD can be shown or hidden from `VIDEO` or through a dedicated remappable hotkey
- user-facing desktop failures such as ROM open/load errors now surface through native SDL3 message boxes instead of only writing to `stderr`, while technical diagnostics still remain available in terminal output
- it reuses the same DMG-family startup model, startup mode, execution mode, boot-ROM search, and battery-save concepts as `gb-cli`
- host audio playback now consumes a typed post-HPF sample-capture boundary from `gb-core`, so the desktop frontend only performs final host-side `f32` normalization and SDL3 queueing instead of owning APU semantics
- persisted desktop settings live under the platform config directory by default, or under `GB_CYCLE_DESKTOP_SETTINGS_PATH` when that environment variable is set; those settings now include frontend video scale, `vsync`, integer-presentation, and stats-HUD visibility choices, frontend audio volume and mute state, keyboard joypad bindings, keyboard menu bindings, frontend hotkeys, gamepad bindings, gamepad menu bindings, the preferred SDL gamepad identity changed through the overlay UI, the last opened directory, and a recent-ROM history

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
cargo cov-check
cargo cov-html
```

`cargo cov-check` currently gates aggregate `>=90%` line, region, and function
coverage across `gb-core`, `gb-test-runner`, and `gb-persistence`.
`make coverage` runs `cargo cov-html` and writes the workspace HTML report under
`target/llvm-cov/html/`.

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
make run-blargg
make run-acid
make run-daid
make run-cpp
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
  `✅`, `❌` and `ℹ️` in the status column, adding a `non-failing/total`
  summary in the `# Test Report (...)` header, and keeping each family's
  curated ROM order from the manifest
- repo-managed local-only support assets now also live under gitignored roots
  inside the workspace:
  `/.roms/bootrom/` for DMG/MGB boot ROM images and
  `/.oracles/<oracle>/<layout>/` for imported differential oracle artifacts
- `make ci` stays as the fast local pre-push gate and does not fetch or run
  external ROM suites; it includes the Rust checks plus the coverage threshold
  gate through `cargo cov-check`
- `make coverage` runs `cargo cov-html` and emits the workspace HTML coverage
  report under `target/llvm-cov/html/`
- `make test-roms` fetches the curated ROM store if needed and runs all local
  curated DMG suites currently wired in `Makefile`:
  `acid`, `blargg`, `daid`, `hacktix`, `cpp`, `mealybug-tearoom-tests`, and
  `mooneye`
- GitHub uses two workflows:
  `ci` for Rust checks plus coverage
  `test-roms` for the workflow-managed ROM subset currently exercised in CI:
  `acid`, `blargg`, `hacktix`, and `cpp`
- the curated Acid DMG family mixes one blocking framebuffer oracle
  `dmg-acid2.gb` with one informational framebuffer capture case `which.gb`,
  matching the upstream `GBEmulatorShootout` classification
- `make run-blargg` runs the curated Blargg DMG family, including
  `dmg_sound 01..12`
- `make run-acid` runs the curated supported Acid DMG family
- `make run-daid` runs the current exploratory `daid` DMG subset and updates
  `/.roms/test/test-report.md`
- each `make run-*` target is autosufficient and materializes its own curated
  family before execution
- `make run-hacktix` runs the curated `hacktix` DMG subset and updates
  `/.roms/test/test-report.md`; it is also part of the GitHub `test-roms`
  workflow
- `make run-cpp` runs the curated `cpp` MBC3 subset and updates
  `/.roms/test/test-report.md`; it is also part of the GitHub `test-roms`
  workflow
- `make run-mealybug` runs the current exploratory `mealybug-tearoom` DMG
  subset and updates `/.roms/test/test-report.md`
- `make run-mooneye` runs the current exploratory `mooneye` DMG acceptance
  subset and updates `/.roms/test/test-report.md`
- the current curated Blargg family intentionally uses only individual ROMs
  from `GBEmulatorShootout`; it does not use multi-ROM bundles such as
  `cpu_instrs.gb`
- the full curated Blargg family now includes the DMG `dmg_sound 01..12`
  individual ROMs from `GBEmulatorShootout`, and that audio slice is now
  intentionally promoted into the curated local block used by `make run-blargg`
  and `make test-roms`
- the upstream `oam_bug/7-timing_effect.gb`, CGB-only ROMs, and other still-red
  cases stay outside the default managed block until they are intentionally
  promoted
- one exploratory `mealybug-tearoom` DMG subset is also integrated as
  `mealybug-tearoom-dmg-curated`; the local `make test-roms` aggregator runs
  it for visibility, but it remains outside the GitHub `test-roms` workflow
  because it still diverges from the upstream framebuffer fixtures under
  `Strict`
- one exploratory `mooneye` DMG acceptance subset is also integrated as
  `mooneye-acceptance-dmg-curated`; it follows the active
  `GBEmulatorShootout` `testroms/mooneye.py` acceptance list, uses the upstream
  `mooneye` breakpoint/register result protocol instead of framebuffer oracles,
  and is currently run by the local `make test-roms` aggregator while staying
  outside the GitHub `test-roms` workflow until the remaining failures are
  triaged
- one exploratory `daid` DMG subset is also integrated as `daid-dmg-curated`;
  it mixes framebuffer fixtures, one multi-fixture framebuffer oracle for
  `ppu_scanline_bgp.gb`, and one informational framebuffer capture case
  `rom_and_ram.gb`
- one workflow-managed `hacktix` DMG subset is also integrated as
  `hacktix-dmg-curated`; it currently tracks `bully.gb` and
  `strikethrough.gb` from `GBEmulatorShootout`, uses framebuffer fixtures, and
  is now exercised by the GitHub `test-roms` workflow
- if `GB_CYCLE_TEST_ROM_ROOT` is unset, `gb-test-runner` falls back to the
  default curated store automatically
- keep private commercial ROMs out of that path; use the separate gitignored
  `/.roms/local-commercial/` directory for local-only assets that must never be
  referenced by CI
- for ad hoc local commercial-ROM bring-up, `run_rom_suite` also accepts
  `--manifest <path>` with typed per-case metadata and deterministic joypad
  stimuli; when a manifest-driven case captures the framebuffer, the runner
  writes a sibling PNG next to the ROM using the ROM stem
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

- to run the full curated Blargg DMG family, including the now-promoted Phase
  `7` `dmg_sound` slice, run:

```bash
cargo run -p gb-test-runner --bin run_rom_suite -- --suite blargg-dmg-curated
```

- to drive one local commercial ROM with real boot plus deterministic `Start`
  input, write a manifest like this and run it with `--manifest`:

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

  The final framebuffer PNG lands next to the ROM as
  `/.roms/local-commercial/tetris.png`.

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

  The built-in cartridge mapper oracle lane uses that `case-bundle` layout:

```bash
cargo run -p gb-test-runner --bin run_sameboy_case_bundle -- \
  --suite phase-6-cartridge-oracle \
  --sameboy-root /path/to/SameBoy \
  --build-if-missing

cargo run -p gb-test-runner --bin run_differential -- \
  --oracle sameboy \
  --suite phase-6-cartridge-oracle
```

  That suite compares retained synthetic `MBC1`, `MBC2`, `MBC3`, and `MBC5`
  Phase `6` `serial_hex` artifacts, and its `MBC3` case includes explicit
  pre-run RTC advancement in the typed runner metadata, so it currently belongs
  on the generic `case-bundle` differential path rather than the framebuffer-
  only `sameboy-tester` path.

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

Before implementing subsystems, read the main handbooks in `docs/` first:

- `docs/index.md`
- `docs/ARCHITECTURE.md`
- `docs/CODING-RULES.md`
- `docs/EXECUTION.md`
- `docs/REFERENCES.md`
- `docs/ROADMAP.md`
- `docs/TESTING.md`
- `docs/TIMING-AND-ACCURACY.md`
- `docs/hardware/*.md`

The documentation hierarchy, in summary, is:

- `docs/index.md` as the entry point for reading order and document authority boundaries
- `docs/ARCHITECTURE.md` for layout, ownership, and subsystem boundaries
- `docs/ARCHITECTURE.md` also for the central compatibility-policy structure, execution-mode ownership boundaries, and the top-level separation between cartridge persistence and full emulator save states
- `docs/TIMING-AND-ACCURACY.md` for shared timing vocabulary and project-wide timing constraints
- `docs/ARCHITECTURE.md` plus `docs/TIMING-AND-ACCURACY.md` together for the global T-cycle scheduler contract
- `docs/EXECUTION.md` and `docs/CODING-RULES.md` for workflow and code-change discipline
- `docs/REFERENCES.md` for source and oracle consultation order
- `docs/hardware/*.md` for the behavior and contracts of the corresponding subsystem
- `docs/hardware/CARTRIDGES-MBC.md` specifically for mapper classification, special-cartridge taxonomy, cartridge-side compatibility-category policy, and cartridge persistence rules distinct from full emulator save states
- `docs/TESTING.md` for the global validation, differential, determinism, DMG-hardening policy, and official `Strict` CI/oracle usage
- `docs/ROADMAP.md` for recommended implementation order, phase context, and outstanding TODOs

Use `docs/research/*.md` as secondary comparison material when you need implementation examples, additional validation, or comparison against reference oracles.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
