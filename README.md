# gb-cycle

A hardware-accuracy-focused Game Boy emulator written in Rust.

## Current implementation highlights

| Domain | Highlight |
| --- | --- |
| Core architecture | Frontend-agnostic `gb-core` separated from CLI, desktop, persistence, and ROM-runner crates so the emulator stays portable and testable. |
| Scheduler | One deterministic shared `T-cycle` timeline coordinates CPU, PPU, timer, DMA, APU, serial, joypad, link, and MMIO side effects. |
| CPU | `T-cycle`-accurate micro-op core with real opcode, immediate, stack, and interrupt-service bus traffic. |
| PPU | `T-cycle`-accurate dot pipeline with explicit fetcher/FIFO stages, variable `Mode 3`, live MMIO effects, and DMG OAM-corruption coverage. |
| DMA / bus | Requester-aware arbitration with DMG OAM DMA timing, blocked VRAM/OAM access semantics, and explicit MMIO ownership. |
| Timer / interrupts | Falling-edge timer model with delayed `TIMA` reload/request timing, centralized `IF` / `IE` ownership, and scheduler-visible IRQ aggregation. |
| APU | Shared-timeline four-channel DMG audio core with `DIV-APU` / frame-sequencer timing, channel quirks, HPF, and host-facing sample export. |
| Joypad / serial | Hardware-owned `JOYP`, `SB`, and `SC` semantics with visible-edge interrupts, bit-shift transfers, and explicit link-endpoint boundaries. |
| External port | Explicit external-port attachment model with Game Boy Printer protocol, `DMG-04` cable sessions, and `DMG-07` 2/3/4-player adapter topology on the shared T-cycle timeline. |
| Cartridges | Header-driven mapper model covering `NoMBC`, `MBC1`, `MBC2`, `MBC3`, `MBC5`, `MBC7`, `MMM01`, `M161`, `HuC1`, `HuC3`, `Pocket Camera`, RTC, MBC5 rumble, MBC7 accelerometer / EEPROM, and separate host persistence. |
| Boot / startup | Real boot-ROM handoff plus model-aware `SkipBoot` state synthesis that keeps first post-boot timer, PPU, and APU behavior coherent. |
| Save states / rewind | Versioned `.gbstate` v3 whole-machine save/load with metadata-checked restore, deterministic continuation coverage, and core-owned rewind snapshots exposed by desktop hold-to-rewind. |
| Debugging / tooling | Typed traces, breakpoints, watchpoints, subsystem snapshots, differential artifact comparison, and first-divergence probes provide practical localization paths for timing-sensitive failures. |
| Validation | Phase 9 DMG closure evidence combines the `167/167` curated external report (`165` passing, `2` informational), repo/workflow ROM gates, SameBoy differential tooling, first-divergence probes, and determinism/save-load lanes. |

## Current structure

The canonical structure and ownership boundaries are defined in `docs/ARCHITECTURE.md`.
If this summary differs from `docs/ARCHITECTURE.md`, `docs/ARCHITECTURE.md` takes precedence.
The current workspace uses the `crates/`-based layout below.

```text
crates/
  gb-core/         Pure emulation core, hardware state, debugger snapshots, and save-state / rewind DTOs
  gb-test-runner/  Typed ROM harness, executable suites, differential tooling, determinism checks, and linked-session validation
  gb-cli/          Headless CLI frontend, ROM inspection, save conversion, and `.gbstate` run tooling
  gb-desktop/      SDL3 desktop frontend with local link sessions, printer, Pocket Camera, audio/video diagnostics, save states, rewind, and Fast Forward
  gb-persistence/  Host-side `.gbsav`, external `.sav`, and `.gbstate` envelope formats

docs/              Architecture, roadmap, testing, frontend, and technical documentation
Makefile           Local verification pipeline, ROM-suite helpers, and Phase 9 differential/determinism utilities
```

Future extensions that are intentionally not separate crates yet:

- `gb-web`
- richer debugger / devtools surfaces on top of the existing trace, snapshot, breakpoint, and watchpoint contracts
- CGB- and SGB-focused tooling as those hardware models land

## Quick start

```bash
# CLI: inspect a ROM header
cargo run -p gb-cli -- inspect-rom path/to/rom.gb

# CLI: headless run with serial capture
cargo run -p gb-cli -- run path/to/rom.gb --tcycles 5000 --serial-out .artifacts/serial.bin

# CLI: save and restore a whole-machine .gbstate
cargo run -p gb-cli -- run path/to/rom.gb --tcycles 5000 --state-out .artifacts/run.gbstate
cargo run -p gb-cli -- run path/to/rom.gb --state-in .artifacts/run.gbstate --tcycles 5000

# Desktop: launch the SDL3 frontend in release for real-time speed
cargo run --release -p gb-desktop -- [path/to/rom.gb]

# Desktop: launch a local DMG-04 two-player Game Link session
cargo run --release -p gb-desktop -- path/to/p1.gb --link-rom path/to/p2.gb
```

See [docs/frontends/CLI.md](docs/frontends/CLI.md) and [docs/frontends/DESKTOP.md](docs/frontends/DESKTOP.md) for full usage details.

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
make coverage-check
make coverage
```

`make coverage-check` performs one workspace coverage sweep and enforces the current per-crate line, region, and function thresholds configured in `.cargo/config.toml` for `gb-core`, `gb-test-runner`, `gb-persistence`, `gb-cli`, and `gb-desktop`.
`make coverage` runs `cargo cov-html` and writes the workspace HTML report under `target/llvm-cov/html/`.

### Full local pipeline

```bash
make ci
make test-roms
make coverage
```

### External ROM suites

See [docs/testing/ROM-SUITES.md](docs/testing/ROM-SUITES.md) for the full external ROM suite workflow: fetching, running, differential oracles, determinism lanes, and private manifest-based commercial ROM smoke workflows.

## Documentation

See [docs/index.md](docs/index.md) for the full reading order, document authority boundaries, and handbook index.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
