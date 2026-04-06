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

## Quick start

```bash
# CLI: inspect a ROM header
cargo run -p gb-cli -- inspect-rom path/to/rom.gb

# CLI: headless run with serial capture
cargo run -p gb-cli -- run path/to/rom.gb --tcycles 5000 --serial-out .artifacts/serial.bin

# Desktop: launch the SDL3 frontend
cargo run -p gb-desktop -- [path/to/rom.gb]
```

See [docs/CLI.md](docs/CLI.md) and [docs/DESKTOP.md](docs/DESKTOP.md) for full usage details.

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

See [docs/ROM-SUITES.md](docs/ROM-SUITES.md) for the full external ROM suite workflow: fetching, running, differential oracles, and commercial ROM testing.

## Documentation

See [docs/index.md](docs/index.md) for the full reading order, document authority boundaries, and handbook index.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
