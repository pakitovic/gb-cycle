# gb-cycle

A hardware-accuracy-focused Game Boy / Game Boy Color / Game Boy Advance GB/C compatibility / Super Game Boy emulator written in Rust, developed with support from AI-assisted tooling.

## Implementation highlights

| Domain | Highlight |
| --- | --- |
| Scheduler | One deterministic shared `T-cycle` timeline coordinates CPU, PPU, timer, speed switching, DMA, APU, serial, joypad, link, and MMIO side effects. |
| CPU | `T-cycle`-accurate micro-op core with real opcode, immediate, stack, interrupt-service, `HALT`, `STOP`, and native-CGB speed-switch bus traffic. |
| PPU | `T-cycle`-accurate dot pipeline with explicit fetcher/FIFO stages, variable `Mode 3`, live MMIO effects, DMG OAM-corruption coverage, CGB VRAM-bank attributes, palettes, priority composition. |
| DMA / bus / memory | Requester-aware arbitration with DMG and CGB OAM DMA policies, native-CGB VRAM/WRAM banking, GDMA/HDMA, blocked VRAM/OAM semantics, and explicit MMIO ownership. |
| Timer / speed / interrupts | Falling-edge timer model with delayed `TIMA` reload/request timing, native-CGB `KEY1` normal/double-speed domains, centralized `IF` / `IE` ownership, and scheduler-visible IRQ aggregation. |
| APU | Shared-timeline four-channel audio core with `DIV-APU` / frame-sequencer timing, DMG and CGB channel quirks, CGB `PCM12` / `PCM34` taps, HPF. |
| Joypad / serial / external I/O | `JOYP`, `SB`, `SC` and CGB `RP` semantics with visible-edge interrupts, DMG and native-CGB serial timing including `SC.1` high speed, `DMG-04` game link, `DMG-07` 2/3/4-player adapter, SGB `MLT_REQ`, CGB-to-CGB infrared sessions. |
| Cartridges | `NoMBC`, `MBC1`, `MBC2`, `MBC3` / `MBC30`, `MBC5`, `MBC6`, `MBC7`, `MMM01`, `M161`, `HuC1`, `HuC3`, `Pocket Camera`, `RTC`, flash / EEPROM / accelerometer paths, rumble-capable metadata. |
| Features | Frontend-agnostic `gb-core`, battery saves, save states, rewind, fast forward, real boot-ROM `DMG`/`CGB`/`SGB`/`SGB2`/`AGB` GB/C compatibility for GBA Enhanced, Game Boy Printer, [Pokémon Pikachu Color](docs/info/INFRARED.md#pok%C3%A9mon-pikachu-color), [Custom GSC Mystery Gift IR Sender](docs/info/INFRARED.md#custom-gsc-mystery-gift-ir-sender) |
| Validation | [ROM-test reports](https://pakitovic.github.io/gb-cycle/) publish browsable snapshots for the maintained report lanes; the [GBEmulatorShootout fork](https://pakitovic.github.io/GBEmulatorShootout/) currently reports `gb-cycle` green on every counted row (`264/264` in the latest generated dashboard). |

## Current structure

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the canonical structure and ownership boundaries; the current workspace uses the `crates/`-based layout below.

```text
crates/
  gb-core/         Portable emulator core API and state model
  gb-test-runner/  Report-local ROM-suite fetchers, runners, oracles, and validation tooling
  gb-benchmark/    Benchmark case format and reporting helpers
  gb-cli/          Headless command-line frontend
  gb-desktop/      SDL3 desktop frontend
  gb-persistence/  Host persistence formats and conversion helpers

.cargo/           Cargo aliases for local runners and verification commands
docs/              Project handbook and subsystem notes
Makefile           Local setup, hooks, and coverage entry points
scripts/           Developer helper scripts
```

## Quick start

Use this section to choose the entry point. Concrete command examples live in the linked docs so new examples can grow there without expanding the README.

### `gb-cli`

Use `gb-cli` for headless ROM inspection, deterministic short runs, serial/framebuffer artifacts, and whole-machine `.gbstate` save/load checks. See [docs/info/CLI.md](docs/info/CLI.md#examples) for command examples and full usage details.

### `gb-desktop`

Use `gb-desktop` for the SDL3 frontend, real-time play, menus, local link/IR sessions, SGB presentation toggles, save states, rewind, and audio/video diagnostics. See [docs/info/DESKTOP.md](docs/info/DESKTOP.md#examples) for launch examples and full usage details.

### `gb-benchmark`

Use the shared `gb-benchmark` TOML contract through `cargo rom-bench` for desktop-first benchmark batches, optional matching CLI artifacts, and direct frontend benchmark runs. See [docs/info/BENCHMARK.md](docs/info/BENCHMARK.md#examples) for benchmark examples and full usage details.

## Release packages

Run the [`release`](.github/workflows/release.yml) GitHub Actions workflow to publish the GitHub Release and platform packages:

- `gb-cycle-windows-x86_64.zip`
- `gb-cycle-linux-x86_64.tar.gz`
- `gb-cycle-macos-aarch64.zip`

## Tooling

This repository uses [`rust-toolchain.toml`](rust-toolchain.toml) to pin the local Rust toolchain; the current minimum supported Rust version is 1.96.

### Setup local tooling and hooks

```bash
make setup
```

### Pre-commit checks

```bash
cargo fmt-check
cargo lint
typos
cargo deny-check
```

The configured pre-commit hook runs the same checks.

### Coverage

```bash
make coverage
```

`make coverage` runs the workspace coverage sweep, enforces per-crate gates, and emits the HTML report.

### ROM-suite validation

Browsable ROM-test report snapshots are published at [pakitovic.github.io/gb-cycle](https://pakitovic.github.io/gb-cycle/).

```bash
cargo rom-fetch gb-emulator-shootout
cargo rom-suite gb-emulator-shootout
cargo rom-suite gbmicrotest
cargo rom-suite-link linked
```

See [docs/info/ROM-SUITES.md](docs/info/ROM-SUITES.md) for `cargo rom-fetch`, `cargo rom-suite`, `cargo rom-suite-link`, promoted gates, RealBoot reruns with `--boot-rom-dir`, oracle comparisons, determinism lanes, and private smoke workflows.

## Documentation

See [docs/index.md](docs/index.md) for the full reading order, document authority boundaries, and handbook index.

## Acknowledgements

gb-cycle is an independent emulator, but its hardware-fidelity work benefits heavily from the Game Boy emulation community. Special thanks to:

- [SameBoy](https://github.com/LIJI32/SameBoy), for its high-accuracy DMG/CGB implementation, mature tester/oracle paths, and readable hardware behavior cross-checks.
- [DocBoy](https://github.com/Docheinstein/docboy) and the [docboy-test-suite](https://github.com/Docheinstein/docboy-test-suite/), for precision-focused emulator architecture ideas and high-value timing, PPU, APU, bus, and linked-session tests.
- [GBE+](https://github.com/shonumi/gbe-plus), for its broad accessory/peripheral coverage and practical examples around less common Game Boy hardware.
- [bayleef](https://projectpokemon.org/home/forums/topic/43930-mystery-gift-reverse-engineering-of-ir-protocol/#comment-232992), for the ProjectPokemon post “Mystery Gift: Reverse Engineering of IR Protocol”, which documents the Generation 2 IR Mystery Gift protocol and Pokémon Pikachu 2 GS behavior.

These projects are used as references, examples, and inspiration; primary documentation, hardware research, and explicit tests remain the source of truth for gb-cycle behavior. See [docs/REFERENCES.md](docs/REFERENCES.md) for the project consultation policy.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
