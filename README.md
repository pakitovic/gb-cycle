# gb-cycle

A hardware-accuracy-focused Game Boy emulator written in Rust.

## Current implementation highlights

| Domain | Highlight |
| --- | --- |
| Core architecture | Frontend-agnostic `gb-core` separated from CLI, desktop, persistence, and ROM-runner crates so the emulator stays portable and testable. |
| Scheduler | One deterministic shared `T-cycle` timeline coordinates CPU, PPU, timer, DMA, APU, and MMIO side effects. |
| CPU | `T-cycle`-accurate micro-op core with real opcode, immediate, stack, and interrupt-service bus traffic. |
| PPU | `T-cycle`-accurate dot pipeline with explicit fetcher/FIFO stages, variable `Mode 3`, live MMIO effects, and DMG OAM-corruption coverage. |
| DMA / bus | Requester-aware arbitration with DMG OAM DMA timing, blocked VRAM/OAM access semantics, and explicit MMIO ownership. |
| Timer / interrupts | Falling-edge timer model with delayed `TIMA` reload/request timing, centralized `IF` / `IE` ownership, and scheduler-visible IRQ aggregation. |
| APU | Shared-timeline four-channel DMG audio core with `DIV-APU` / frame-sequencer timing, channel quirks, HPF, and host-facing sample export. |
| Joypad / serial | Hardware-owned `JOYP`, `SB`, and `SC` semantics with visible-edge interrupts, bit-shift transfers, and explicit link-endpoint boundaries. |
| Cartridges | Header-driven mapper model with `NoMBC`, `MBC1`, `MBC2`, `MBC3`, `MBC5`, RTC support, and separate host persistence. |
| Boot / startup | Real boot-ROM handoff plus model-aware `SkipBoot` state synthesis that keeps first post-boot timer, PPU, and APU behavior coherent. |
| Validation | Current curated DMG external report is `167/167` entries (`165` passing, `2` informational) plus focused unit and integration coverage. |

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

# Desktop: launch the SDL3 frontend in release for real-time speed
cargo run --release -p gb-desktop -- [path/to/rom.gb]
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

### External ROM suites

See [docs/ROM-SUITES.md](docs/ROM-SUITES.md) for the full external ROM suite workflow: fetching, running, differential oracles, and commercial ROM testing.

## Documentation

See [docs/index.md](docs/index.md) for the full reading order, document authority boundaries, and handbook index.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
