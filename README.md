# gb-cycle

A hardware-accuracy-focused Game Boy / Game Boy Color / Super Game Boy emulator written in Rust, developed with support from AI-assisted tooling.

## Current implementation highlights

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
| Features | Frontend-agnostic `gb-core`, battery saves, save states, rewind, fast forward, real boot-ROM `DMG`/`CGB`/`SGB`/`SGB2`, Game Boy Printer, [Pokémon Pikachu Color](docs/info/CGB-INFRARED.md#pok%C3%A9mon-pikachu-color), [Custom GSC Mystery Gift IR Sender](docs/info/CGB-INFRARED.md#custom-gsc-mystery-gift-ir-sender) |
| Validation | [GBEmulatorShootout fork](https://pakitovic.github.io/GBEmulatorShootout/) currently reports `gb-cycle` green on every counted ROM-test row in the fork (`264/264` in the latest generated dashboard). |

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
Makefile           Local setup, hooks, CI, and coverage entry points
scripts/           Developer helper scripts
```

## Quick start

### `gb-cli`

Use `gb-cli` for headless ROM inspection, deterministic short runs, serial/framebuffer artifacts, and whole-machine `.gbstate` save/load checks.

```bash
# Inspect a ROM header
cargo run -p gb-cli -- inspect-rom path/to/rom.gb

# Run headless with serial capture
cargo run -p gb-cli -- run path/to/rom.gb --tcycles 5000 --serial-out .artifacts/serial.bin

# Force the Game Boy Color model and export the final RGB555 framebuffer as PNG
cargo run -p gb-cli -- run path/to/rom.gbc --model CGB --frames 120 --framebuffer-out .artifacts/frame.png

# Run an SGB-enhanced game and export the native 256x224 SGB host frame
cargo run -p gb-cli -- run path/to/rom.gb --model SGB --frames 120 --framebuffer-out .artifacts/sgb.png

# Select original SGB PAL, or export SGB/SGB2 LCD-only PNG without the host border
cargo run -p gb-cli -- run path/to/rom.gb --model SGB --sgb-standard pal --framebuffer-out .artifacts/sgb-pal.png
cargo run -p gb-cli -- run path/to/rom.gb --model SGB2 --border-off --framebuffer-out .artifacts/sgb2-lcd.png

# Save and restore a whole-machine .gbstate
cargo run -p gb-cli -- run path/to/rom.gb --tcycles 5000 --state-out .artifacts/run.gbstate
cargo run -p gb-cli -- run path/to/rom.gb --state-in .artifacts/run.gbstate --tcycles 5000
```

### `gb-desktop`

Use `gb-desktop` for the SDL3 frontend, real-time play, menus, local link/IR sessions, SGB presentation toggles, save states, rewind, and audio/video diagnostics.

```bash
# Launch the SDL3 frontend in release for real-time speed
cargo run --release -p gb-desktop -- [path/to/rom.gb]

# Launch a CGB ROM with direct RGB555 presentation
cargo run --release -p gb-desktop -- path/to/rom.gbc --model CGB

# Launch an SGB/SGB2 profile; CONFIG -> SYSTEM exposes MODEL, REV, VIDEO, START, and BORDER
cargo run --release -p gb-desktop -- path/to/sgb-enhanced.gb --model SGB
cargo run --release -p gb-desktop -- path/to/sgb-enhanced.gb --model SGB2 --startup real-boot --boot-rom-dir "$HOME/emu/roms/bootrom"

# Launch a local DMG-04 two-player Game Link session
cargo run --release -p gb-desktop -- path/to/p1.gb --link-rom path/to/p2.gb
```

### `gb-benchmark`

Use the shared `gb-benchmark` TOML contract through `scripts/run-benchmark.sh` for desktop-first benchmark batches, optional matching CLI artifacts, and direct frontend benchmark runs.

```bash
# Create a sample portable benchmark case
scripts/run-benchmark.sh --sample

# Run every case in a directory through gb-desktop and generate scripts/benchmark/index.html
scripts/run-benchmark.sh path/to/benchmark-cases

# Add matching gb-cli artifacts and columns to the same benchmark report
scripts/run-benchmark.sh path/to/benchmark-cases --gb-cli

# Run one benchmark TOML directly through either frontend
cargo run -p gb-cli -- run --benchmark path/to/game.toml
cargo run --release -p gb-desktop -- --benchmark path/to/game.toml
```

See [docs/info/CLI.md](docs/info/CLI.md) and [docs/info/DESKTOP.md](docs/info/DESKTOP.md) for full usage details.

## Release packages

Run the [`release`](.github/workflows/release.yml) GitHub Actions workflow to publish the GitHub Release and platform packages:

- `gb-cycle-windows-x86_64.zip`
- `gb-cycle-linux-x86_64.tar.gz`
- `gb-cycle-macos-aarch64.zip`

## Tooling

### Install local tooling

```bash
make setup
```

### Coverage

```bash
make coverage-check
make coverage
```

### Full local pipeline

```bash
make ci
```

### ROM-suite validation

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
