# gb-cycle

A hardware-accuracy-focused Game Boy / Game Boy Color / Super Game Boy emulator written in Rust, developed with support from AI-assisted tooling.

## Current implementation highlights

| Domain | Highlight |
| --- | --- |
| Core architecture | Frontend-agnostic `gb-core` separated from CLI, desktop, persistence, and ROM-runner crates so the DMG, CGB, and SGB-family hardware paths stay portable, deterministic, and testable. |
| Scheduler | One deterministic shared `T-cycle` timeline coordinates CPU, PPU, timer, speed switching, DMA, APU, serial, joypad, link, and MMIO side effects. |
| CPU | `T-cycle`-accurate micro-op core with real opcode, immediate, stack, interrupt-service, `HALT`, `STOP`, and native-CGB speed-switch bus traffic. |
| PPU | `T-cycle`-accurate dot pipeline with explicit fetcher/FIFO stages, variable `Mode 3`, live MMIO effects, DMG OAM-corruption coverage, CGB VRAM-bank attributes, palettes, priority composition, and an RGB555 framebuffer. |
| DMA / bus / memory | Requester-aware arbitration with DMG and CGB OAM DMA policies, native-CGB VRAM/WRAM banking, GDMA/HDMA, blocked VRAM/OAM semantics, and explicit MMIO ownership. |
| Timer / speed / interrupts | Falling-edge timer model with delayed `TIMA` reload/request timing, native-CGB `KEY1` normal/double-speed domains, centralized `IF` / `IE` ownership, and scheduler-visible IRQ aggregation. |
| APU | Shared-timeline four-channel audio core with `DIV-APU` / frame-sequencer timing, DMG and CGB channel quirks, CGB `PCM12` / `PCM34` taps, HPF, and host-facing sample export. |
| Joypad / serial / external I/O | Hardware-owned `JOYP`, `SB`, `SC`, and CGB `RP` semantics with visible-edge interrupts, DMG and native-CGB serial timing including `SC.1` high speed, explicit link / IR endpoint boundaries, Game Boy Printer protocol, `DMG-04` cable sessions, `DMG-07` 2/3/4-player topology, SGB `MLT_REQ` host-controller multiplexing, native CGB-to-CGB infrared sessions, the Pokémon Pikachu Color / Pokémon Pikachu 2 GS / Pocket Pikachu Color Mystery Gift accessory model, and a selectable western GSC Mystery Gift sender accessory. |
| Cartridges | Header-driven mapper model covering `NoMBC`, `MBC1`, `MBC2`, `MBC3` / `MBC30`, `MBC5`, `MBC6`, `MBC7`, `MMM01`, `M161`, `HuC1`, `HuC3`, `Pocket Camera`, RTC, flash / EEPROM / accelerometer paths, rumble-capable metadata, and separate host persistence. |
| Boot / startup | Real boot-ROM handoff plus model-aware `SkipBoot` state synthesis for DMG-family, CGB-family, and SGB-family profiles, including CGB boot-window routing, SGB/SGB2 `sgb_boot.bin` / `sgb2_boot.bin` asset selection, header-driven native/compatibility/SGB command policy, and coherent first post-boot timer, PPU, and APU state. |
| Frontends | `gb-cli` and the SDL3 `gb-desktop` frontend share model/startup/execution-mode semantics; the desktop frontend renders CGB and SGB RGB555 output directly, keeps DMG-family presentation palettes host-side, and supports SGB/SGB2 model selection, SGB PAL/NTSC selection, SGB border presentation toggles, SGB2 Game Link, printer, camera, Game Link, CGB IR, Pokémon Pikachu 2 Mystery Gift, custom GSC Mystery Gift item/decoration sends, audio/video diagnostics, battery saves, save states, rewind, and Fast Forward. |
| Benchmarking | Shared `gb-benchmark` case parsing, deterministic input scheduling, artifact naming, and stats serialization let `gb-cli`, `gb-desktop`, and `scripts/run-benchmark.sh` run the same portable one-file-per-game benchmark contracts. |
| Save states / rewind | Versioned `.gbstate` v1 whole-machine save/load with metadata-checked restore, deterministic continuation coverage, CGB state coverage, and core-owned rewind snapshots exposed by desktop hold-to-rewind. |
| Debugging / tooling | Typed traces, breakpoints, watchpoints, subsystem snapshots, RGB555 / grayscale framebuffer artifacts, differential comparison, and first-divergence probes provide practical localization paths for timing-sensitive failures. |
| Validation | Phase 9 DMG closure keeps the `167/167` curated external report (`165` passing, `2` informational), Phase 10 adds promoted CGB ROM gates for smoke, boot/DIV, speed, PPU, DMA, audio, and RTC coverage through local Make targets and the GitHub `test-roms` matrix, green non-DocBoy extra/internal suites run through the separate GitHub `test-roms-extra` matrix, and Phase 11 now covers SGB/SGB2 architecture, packets, palettes, borders, advanced coloring, multiplayer, profiles, and SGB2 link behavior with focused synthetic tests plus informational SameSuite SGB rows. |

## Current structure

The canonical structure and ownership boundaries are defined in `docs/ARCHITECTURE.md`.
If this summary differs from `docs/ARCHITECTURE.md`, `docs/ARCHITECTURE.md` takes precedence.
The current workspace uses the `crates/`-based layout below.

```text
crates/
  gb-core/         Pure DMG/CGB emulation core, hardware state, link/IR devices, debugger snapshots, and save-state / rewind DTOs
  gb-test-runner/  Typed ROM harness, DMG/CGB executable suites, differential tooling, determinism checks, and linked-session validation
  gb-benchmark/    Portable benchmark TOML parsing, deterministic joypad stimuli, shared artifact paths, and frontend-neutral stats
  gb-cli/          Headless CLI frontend, ROM inspection, battery-save runtime/conversion, and `.gbstate` run tooling
  gb-desktop/      SDL3 desktop frontend with CGB RGB555 presentation, local link/IR sessions, printer, Pocket Camera, audio/video diagnostics, battery saves, save states, rewind, and Fast Forward
  gb-persistence/  Host-side cartridge save storage (`.sav/.saN` primary plus `.gbsav/.gbsaN` fallback), external conversion, and `.gbstate` envelope formats

docs/              Architecture, roadmap, testing, frontend, hardware, and reference documentation
Makefile           Local verification pipeline, ROM-suite helpers, CGB gates, and Phase 9 differential/determinism utilities
scripts/           Benchmark and desktop development launch helpers
```

Future extensions that are intentionally not separate crates yet:

- `gb-web`
- richer debugger / devtools surfaces on top of the existing trace, snapshot, breakpoint, and watchpoint contracts
- SNES/SFC host-shell execution tooling for the deferred SGB startup/audio/16-bit slices

## Quick start

```bash
# CLI: inspect a ROM header
cargo run -p gb-cli -- inspect-rom path/to/rom.gb

# CLI: headless run with serial capture
cargo run -p gb-cli -- run path/to/rom.gb --tcycles 5000 --serial-out .artifacts/serial.bin

# CLI: force the Game Boy Color model and export the final RGB555 framebuffer as PNG
cargo run -p gb-cli -- run path/to/rom.gbc --model CGB --frames 120 --framebuffer-out .artifacts/frame.png

# CLI: run an SGB-enhanced game and export the native 256x224 SGB host frame
cargo run -p gb-cli -- run path/to/rom.gb --model SGB --frames 120 --framebuffer-out .artifacts/sgb.png

# CLI: original SGB PAL profile, or SGB/SGB2 LCD-only PNG without the host border
cargo run -p gb-cli -- run path/to/rom.gb --model SGB --sgb-standard pal --framebuffer-out .artifacts/sgb-pal.png
cargo run -p gb-cli -- run path/to/rom.gb --model SGB2 --border-off --framebuffer-out .artifacts/sgb2-lcd.png

# CLI: save and restore a whole-machine .gbstate
cargo run -p gb-cli -- run path/to/rom.gb --tcycles 5000 --state-out .artifacts/run.gbstate
cargo run -p gb-cli -- run path/to/rom.gb --state-in .artifacts/run.gbstate --tcycles 5000

# Desktop: launch the SDL3 frontend in release for real-time speed
cargo run --release -p gb-desktop -- [path/to/rom.gb]

# Desktop: launch a CGB ROM with direct RGB555 presentation
cargo run --release -p gb-desktop -- path/to/rom.gbc --model CGB

# Desktop: launch an SGB/SGB2 profile; CONFIG -> SYSTEM exposes MODEL, REV, VIDEO, START, and BORDER
cargo run --release -p gb-desktop -- path/to/sgb-enhanced.gb --model SGB
cargo run --release -p gb-desktop -- path/to/sgb-enhanced.gb --model SGB2 --startup real-boot --boot-rom-dir "$HOME/emu/roms/bootrom"

# Desktop: launch a local DMG-04 two-player Game Link session
cargo run --release -p gb-desktop -- path/to/p1.gb --link-rom path/to/p2.gb

# Desktop: CGB infrared, Pokémon Pikachu 2, and GSC Mystery Gift are selected at runtime from the overlay GBC IR submenu
cargo run --release -p gb-desktop -- path/to/pokemon-crystal.gbc --model CGB

# Benchmarks: create a sample portable case and run a case directory through desktop
scripts/run-benchmark.sh --sample
scripts/run-benchmark.sh path/to/benchmark-cases
```

See [docs/frontends/CLI.md](docs/frontends/CLI.md) and [docs/frontends/DESKTOP.md](docs/frontends/DESKTOP.md) for full usage details.

## Super Game Boy / Super Game Boy 2

SGB support is implemented as a DMG-compatible GB core plus a pluggable SGB/SNES host shell, not as CGB mode and not as a duplicated GB core. The public profiles are `SGB` / `SUPER GB` and `SGB2` / `SUPER GB2`; original SGB supports `NTSC` and `PAL` host profiles, while SGB2 is fixed to `NTSC`, uses the corrected GB clock, and exposes the physical Game Link port through the existing link topology.

The current public milestone covers Phase 11 slices 0-6: SGB/SGB2 architecture and save-state contracts, `SkipBoot` / `RealBoot` asset selection (`sgb_boot.bin` / `sgb2_boot.bin`), JOYP packet transport and SGB-header unlock policy, base SGB palette commands and BIOS title/default palettes for DMG-only games, `_TRN` transfer capture, static/dynamic border composition, `MASK_EN`, advanced attribute coloring, `PAL_TRN` / `PAL_SET` / `ATTR_TRN` / `ATTR_SET` / `PAL_PRI`, `MLT_REQ` multiplayer with P1-P4 host controller slots, SGB PAL/NTSC/SGB2 NTSC timing facts, and SGB2 Game Link availability.

`gb-desktop` exposes this through `CONFIG -> SYSTEM -> MODEL SUPER GB` / `SUPER GB2`, `REV SGB-CPU 01` / `CPU SGB2`, `VIDEO NTSC/PAL`, `START SKIP/REAL`, and `BORDER ON/OFF`; the no-ROM launcher keeps the handheld-size window until a ROM is loaded. `gb-cli` exposes the same profiles with `--model SGB|SGB2`, `--sgb-standard ntsc|pal` for original SGB, and `--border-off` for LCD-only SGB PNG captures. RealBoot examples can use a private boot-ROM root such as `$HOME/emu/roms/bootrom`.

The final three SGB slices are deliberately deferred to a later milestone: Slice 7 will model the SNES/SFC-side SGB startup shell, built-in generic border, logo animation, and jingle; Slice 8 will implement general SGB special audio (`SOUND` / `SOU_TRN`); Slice 9 will implement SNES-side data transfer and 16-bit execution (`DATA_SND`, `DATA_TRN`, `JUMP`) for titles such as Space Invaders. Because those host-firmware features are not contained in the 256-byte GB-side `sgb_boot.bin` / `sgb2_boot.bin`, current RealBoot correctly executes the GB-side boot asset but does not yet show the real-hardware SGB firmware animation or built-in startup border before cartridge-side transfers.

## CGB infrared, Pokémon Pikachu 2, and GSC Mystery Gift

`gb-core` models CGB infrared as bus-owned `RP` state plus explicit optical topologies. Native CGB-to-CGB IR sessions route light between two independent `Machine` instances, while accessory sessions pair one CGB `Machine` with a protocol device that only injects external IR light into the sensor.

`gb-desktop` exposes CGB infrared through the `GBC IR` overlay submenu when `CONFIG -> SYSTEM -> MODEL GB COLOR` is active. The root label reports `IR: NONE`, `IR: SAME GAME`, `IR: SELECT GAME`, `IR: PIKACHU 2`, or `IR: MYSTERY GIFT`; the submenu marks the active mode with `✓`, keeps `HELPER ON/OFF` for the top-right IR timing helper, and disables save states / rewind while an IR session is active.

`IR -> SAME GAME` clones the loaded CGB ROM into a fresh second console with an isolated P2 save slot. `IR -> SELECT GAME` asks for a second CGB ROM and supports different Gold / Silver / Crystal cartridges on the two IR sides, matching Mystery Gift station and two-console flows without treating IR as a Game Link cable mode.

The native CGB-to-CGB infrared path has been locally tested successfully with Pokémon Gold / Silver / Crystal, Super Mario Bros. DX, Pokémon Trading Card Game, Donkey Kong Country, Pokémon Pinball, and Perfect Dark.

`IR -> PIKACHU 2` enables the Pokémon Pikachu Color / Pokémon Pikachu 2 GS / Pocket Pikachu Color accessory model for western Pokémon Gold, Silver, and Crystal. The implementation generates the PP2 Mystery Gift protocol rather than replaying external waveform data, acts as PP2 role A, mirrors the receiving game's supported western region code (`0x90`, `0x96`, `0x99`, `0x9A`, or `0x9F`), re-arms after each successful send, and currently leaves Japanese / Korean validation as future work.

The `PIKACHU 2` gift selector is enabled only after `PIKACHU 2 ✓` is active and cycles the documented watt tiers: `1W EON MAIL`, `100W BERRY`, `200W BITTER BERRY`, `300W GREAT BALL`, `400W MAX REPEL`, `500W ETHER`, `600W MIRACLEBERRY`, `700W GOLD BERRY`, `800W ELIXIR`, `900W REVIVE`, and `999W RARE CANDY`.

`IR -> MYSTERY GIFT` enables a custom western Pokémon Gold / Silver / Crystal Mystery Gift sender. It uses the same generated role-A IR protocol helper as `PIKACHU 2`, sends only the first 20-byte payload with version `0x03`, ID `0x0000`, trainer name `GB-CYCLE`, western region auto-detection, and no Trainer House team payload. `GIFT ITEM` / `GIFT DECORATION` selects the payload type and the gift selector cycles the documented `0x00..=0x24` table by name only, such as `BERRY`, `EON MAIL`, `WEEDLE DOLL`, and `TENTACOOL DOLL`; long labels scroll in the same way as `PIKACHU 2`.

The custom Mystery Gift selector displays only these uppercase names, without the internal gift code:

| `GIFT ITEM` | `GIFT DECORATION` |
| --- | --- |
| `BERRY` | `JIGGLYPUFF DOLL` |
| `PRZCUREBERRY` | `POLIWAG DOLL` |
| `MINT BERRY` | `DIGLETT DOLL` |
| `ICE BERRY` | `STARYU DOLL` |
| `BURNT BERRY` | `MAGIKARP DOLL` |
| `PSNCUREBERRY` | `ODDISH DOLL` |
| `GUARD SPEC.` | `GENGAR DOLL` |
| `X DEFEND` | `SHELLDER DOLL` |
| `X ATTACK` | `GRIMER DOLL` |
| `BITTER BERRY` | `VOLTORB DOLL` |
| `DIRE HIT` | `CLEFAIRY POSTER` |
| `X SPECIAL` | `JIGGLYPUFF POSTER` |
| `X ACCURACY` | `SUPER NES` |
| `EON MAIL` | `WEEDLE DOLL` |
| `MORPH MAIL` | `GEODUDE DOLL` |
| `MUSIC MAIL` | `MACHOP DOLL` |
| `MIRACLEBERRY` | `MAGNA PLANT` |
| `GOLD BERRY` | `TROPIC PLANT` |
| `REVIVE` | `NES` |
| `GREAT BALL` | `NINTENDO 64` |
| `SUPER REPEL` | `BULBASAUR DOLL` |
| `MAX REPEL` | `SQUIRTLE DOLL` |
| `ELIXIR` | `PINK BED` |
| `ETHER` | `POLKADOT BED` |
| `WATER STONE` | `RED CARPET` |
| `FIRE STONE` | `BLUE CARPET` |
| `LEAF STONE` | `YELLOW CARPET` |
| `THUNDERSTONE` | `GREEN CARPET` |
| `MAX ETHER` | `JUMBO PLANT` |
| `MAX ELIXIR` | `VIRTUAL BOY` |
| `MAX REVIVE` | `BIG ONIX DOLL` |
| `SCOPE LENS` | `PIKACHU POSTER` |
| `HP UP` | `BIG LAPRAS DOLL` |
| `PP UP` | `SURF PIKACHU DOLL` |
| `RARE CANDY` | `PIKACHU BED` |
| `BLUESKY MAIL` | `UNOWN DOLL` |
| `MIRAGE MAIL` | `TENTACOOL DOLL` |

## Release packages

Tag pushes matching `v*` build the SDL3 desktop frontend with the `release-max` profile and attach packaged artifacts to the GitHub Release:

- `gb-cycle-windows-x86_64.zip`
- `gb-cycle-linux-x86_64.tar.gz`
- `gb-cycle-macos-aarch64.zip`

Prepare a new version on a normal pull request before running the manual `release-version` GitHub Actions workflow from `main`. Use `scripts/bump-workspace-version.sh 0.1.7` to update every workspace crate package version plus internal workspace dependency requirements and `Cargo.lock`, then merge that change to `main`.

Pass the SemVer crate version to `release-version` without the tag prefix, for example `0.1.7`; `v0.1.7` is also accepted and normalized. The workflow rejects versions older than the current aligned workspace version, verifies that the workspace already matches the requested version, creates the annotated `v<version>` tag, creates the GitHub Release, and explicitly dispatches the three platform release workflows at that tag so their assets attach to the release. Set `dry_run` when you only want validation without creating the tag, GitHub Release, or package workflows. Prerelease versions such as `0.1.7-rc.1` create GitHub prereleases; SemVer build metadata is intentionally not accepted for crate-release automation.

The workflow needs repository Actions permission to write contents and dispatch workflows. The `main` branch can stay protected because version bumps are merged through pull requests; the workflow only pushes the release tag.

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
make test-roms-extra
make test-roms-cgb
make test-roms-cgb-extra
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
- [bayleef](https://projectpokemon.org/home/forums/topic/43930-mystery-gift-reverse-engineering-of-ir-protocol/#comment-232992), for the ProjectPokemon post [“Mystery Gift: Reverse Engineering of IR Protocol”](https://projectpokemon.org/home/forums/topic/43930-mystery-gift-reverse-engineering-of-ir-protocol/#comment-232992), which documents the Generation 2 IR Mystery Gift protocol and Pokémon Pikachu 2 GS behavior.

These projects are used as references, examples, and inspiration; primary documentation, hardware research, and explicit tests remain the source of truth for gb-cycle behavior. See [docs/REFERENCES.md](docs/REFERENCES.md) for the project consultation policy.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
