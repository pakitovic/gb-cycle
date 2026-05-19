# gb-cycle

A hardware-accuracy-focused Game Boy / Game Boy Color emulator written in Rust, developed with support from AI-assisted tooling such as Codex and Claude.

## Current implementation highlights

| Domain | Highlight |
| --- | --- |
| Core architecture | Frontend-agnostic `gb-core` separated from CLI, desktop, persistence, and ROM-runner crates so the DMG and CGB hardware paths stay portable, deterministic, and testable. |
| Scheduler | One deterministic shared `T-cycle` timeline coordinates CPU, PPU, timer, speed switching, DMA, APU, serial, joypad, link, and MMIO side effects. |
| CPU | `T-cycle`-accurate micro-op core with real opcode, immediate, stack, interrupt-service, `HALT`, `STOP`, and native-CGB speed-switch bus traffic. |
| PPU | `T-cycle`-accurate dot pipeline with explicit fetcher/FIFO stages, variable `Mode 3`, live MMIO effects, DMG OAM-corruption coverage, CGB VRAM-bank attributes, palettes, priority composition, and an RGB555 framebuffer. |
| DMA / bus / memory | Requester-aware arbitration with DMG and CGB OAM DMA policies, native-CGB VRAM/WRAM banking, GDMA/HDMA, blocked VRAM/OAM semantics, and explicit MMIO ownership. |
| Timer / speed / interrupts | Falling-edge timer model with delayed `TIMA` reload/request timing, native-CGB `KEY1` normal/double-speed domains, centralized `IF` / `IE` ownership, and scheduler-visible IRQ aggregation. |
| APU | Shared-timeline four-channel audio core with `DIV-APU` / frame-sequencer timing, DMG and CGB channel quirks, CGB `PCM12` / `PCM34` taps, HPF, and host-facing sample export. |
| Joypad / serial / external I/O | Hardware-owned `JOYP`, `SB`, and `SC` semantics with visible-edge interrupts, DMG and native-CGB serial timing including `SC.1` high speed, CGB `RP` baseline, explicit link-endpoint boundaries, Game Boy Printer protocol, `DMG-04` cable sessions, and `DMG-07` 2/3/4-player topology. |
| Cartridges | Header-driven mapper model covering `NoMBC`, `MBC1`, `MBC2`, `MBC3` / `MBC30`, `MBC5`, `MBC6`, `MBC7`, `MMM01`, `M161`, `HuC1`, `HuC3`, `Pocket Camera`, RTC, flash / EEPROM / accelerometer paths, rumble-capable metadata, and separate host persistence. |
| Boot / startup | Real boot-ROM handoff plus model-aware `SkipBoot` state synthesis for DMG-family and CGB-family models, including CGB boot-window routing, header-driven native/compatibility mode selection, and coherent first post-boot timer, PPU, and APU state. |
| Frontends | `gb-cli` and the SDL3 `gb-desktop` frontend share model/startup/execution-mode semantics; the desktop frontend renders CGB RGB555 output directly, keeps DMG-family presentation palettes host-side, and supports printer, camera, link, audio/video diagnostics, save states, rewind, and Fast Forward. |
| Benchmarking | Shared `gb-benchmark` case parsing, deterministic input scheduling, artifact naming, and stats serialization let `gb-cli`, `gb-desktop`, and `scripts/run-benchmark.sh` run the same portable one-file-per-game benchmark contracts. |
| Save states / rewind | Versioned `.gbstate` v3 whole-machine save/load with metadata-checked restore, deterministic continuation coverage, CGB state coverage, and core-owned rewind snapshots exposed by desktop hold-to-rewind. |
| Debugging / tooling | Typed traces, breakpoints, watchpoints, subsystem snapshots, RGB555 / grayscale framebuffer artifacts, differential comparison, and first-divergence probes provide practical localization paths for timing-sensitive failures. |
| Validation | Phase 9 DMG closure keeps the `167/167` curated external report (`165` passing, `2` informational) while Phase 10 adds promoted CGB ROM gates for smoke, boot/DIV, speed, PPU, DMA, audio, and RTC coverage through local Make targets and the GitHub `test-roms` matrix. |

## Current structure

The canonical structure and ownership boundaries are defined in `docs/ARCHITECTURE.md`.
If this summary differs from `docs/ARCHITECTURE.md`, `docs/ARCHITECTURE.md` takes precedence.
The current workspace uses the `crates/`-based layout below.

```text
crates/
  gb-core/         Pure DMG/CGB emulation core, hardware state, debugger snapshots, and save-state / rewind DTOs
  gb-test-runner/  Typed ROM harness, DMG/CGB executable suites, differential tooling, determinism checks, and linked-session validation
  gb-benchmark/    Portable benchmark TOML parsing, deterministic joypad stimuli, shared artifact paths, and frontend-neutral stats
  gb-cli/          Headless CLI frontend, ROM inspection, save conversion, and `.gbstate` run tooling
  gb-desktop/      SDL3 desktop frontend with CGB RGB555 presentation, local link sessions, printer, Pocket Camera, audio/video diagnostics, save states, rewind, and Fast Forward
  gb-persistence/  Host-side cartridge save storage (`.sav/.saN` primary plus `.gbsav/.gbsaN` fallback), external conversion, and `.gbstate` envelope formats

docs/              Architecture, roadmap, testing, frontend, hardware, and reference documentation
Makefile           Local verification pipeline, ROM-suite helpers, CGB gates, and Phase 9 differential/determinism utilities
scripts/           Benchmark and desktop development launch helpers
```

Future extensions that are intentionally not separate crates yet:

- `gb-web`
- richer debugger / devtools surfaces on top of the existing trace, snapshot, breakpoint, and watchpoint contracts
- SGB / SGB2-focused tooling once that host-shell model lands

## Quick start

```bash
# CLI: inspect a ROM header
cargo run -p gb-cli -- inspect-rom path/to/rom.gb

# CLI: headless run with serial capture
cargo run -p gb-cli -- run path/to/rom.gb --tcycles 5000 --serial-out .artifacts/serial.bin

# CLI: force the Game Boy Color model and export the final RGB555 framebuffer as PNG
cargo run -p gb-cli -- run path/to/rom.gbc --model CGB --frames 120 --framebuffer-out .artifacts/frame.png

# CLI: save and restore a whole-machine .gbstate
cargo run -p gb-cli -- run path/to/rom.gb --tcycles 5000 --state-out .artifacts/run.gbstate
cargo run -p gb-cli -- run path/to/rom.gb --state-in .artifacts/run.gbstate --tcycles 5000

# Desktop: launch the SDL3 frontend in release for real-time speed
cargo run --release -p gb-desktop -- [path/to/rom.gb]

# Desktop: launch a CGB ROM with direct RGB555 presentation
cargo run --release -p gb-desktop -- path/to/rom.gbc --model CGB

# Desktop: launch a local DMG-04 two-player Game Link session
cargo run --release -p gb-desktop -- path/to/p1.gb --link-rom path/to/p2.gb

# Benchmarks: create a sample portable case and run a case directory through desktop
scripts/run-benchmark.sh --sample
scripts/run-benchmark.sh path/to/benchmark-cases
```

See [docs/frontends/CLI.md](docs/frontends/CLI.md) and [docs/frontends/DESKTOP.md](docs/frontends/DESKTOP.md) for full usage details.

## Release packages

Tag pushes matching `v*` build the SDL3 desktop frontend with the `release-max` profile and attach packaged artifacts to the GitHub Release:

- `gb-cycle-windows-x86_64.zip`
- `gb-cycle-linux-x86_64.tar.gz`
- `gb-cycle-macos-aarch64.zip`

The macOS release is Apple Silicon only. The bundle is ad-hoc signed for internal consistency, but it is not notarized with Apple Developer ID credentials, so a downloaded ZIP may need the normal macOS Privacy & Security "Open Anyway" override on first launch.

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
make test-roms-cgb
make coverage
```

### External ROM suites

See [docs/testing/ROM-SUITES.md](docs/testing/ROM-SUITES.md) for the full external ROM suite workflow: fetching, running, promoted DMG and CGB gates, extra/internal CGB lanes, RealBoot reruns, differential oracles, determinism lanes, and private manifest-based commercial ROM smoke workflows.

### Benchmark helper

`scripts/run-benchmark.sh` runs portable benchmark TOML cases through `gb-desktop` by default and can add matching `gb-cli` artifacts with `--gb-cli`. It can also create a sample case, normalize case filenames, generate cases from a ROM directory, rewrite ROM roots, run a single `--test` case, and skip missing/empty/unreadable ROM paths before launching either frontend.

## Documentation

See [docs/index.md](docs/index.md) for the full reading order, document authority boundaries, and handbook index.

## Acknowledgements

gb-cycle is an independent emulator, but its hardware-fidelity work benefits heavily from the Game Boy emulation community. Special thanks to:

- [SameBoy](https://github.com/LIJI32/SameBoy), for its high-accuracy DMG/CGB implementation, mature tester/oracle paths, and readable hardware behavior cross-checks.
- [DocBoy](https://github.com/Docheinstein/docboy) and the [docboy-test-suite](https://github.com/Docheinstein/docboy-test-suite/), for precision-focused emulator architecture ideas and high-value timing, PPU, APU, bus, and linked-session tests.
- [GBE+](https://github.com/shonumi/gbe-plus), for its broad accessory/peripheral coverage and practical examples around less common Game Boy hardware.

These projects are used as references, examples, and inspiration; primary documentation, hardware research, and explicit tests remain the source of truth for gb-cycle behavior. See [docs/REFERENCES.md](docs/REFERENCES.md) for the project consultation policy.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
