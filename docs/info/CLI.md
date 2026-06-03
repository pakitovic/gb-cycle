# gb-cli

`gb-cli` is the headless frontend and tooling surface for running one ROM, inspecting cartridge metadata, converting cartridge saves, and producing benchmark artifacts without SDL. It must reuse `gb-core` hardware policy rather than redefining model, startup, timing, compatibility, or persistence behavior. For the model-axis contract see [`MODEL-AXES.md`](MODEL-AXES.md); for boot/startup semantics see [`../hardware/BOOT-ROM.md`](../hardware/BOOT-ROM.md); for ROM-suite automation see [`ROM-SUITES.md`](ROM-SUITES.md).

## Commands

| Command | Purpose |
| --- | --- |
| `gb-cli run <rom> [options]` | Execute one ROM headlessly and optionally write serial, framebuffer, trace, save-state, or cartridge-save artifacts. |
| `gb-cli run --benchmark <case.toml> [--test-runner]` | Execute one portable benchmark case TOML through the CLI frontend. |
| `gb-cli inspect-rom <rom> [--mode <strict|permissive|experimental>]` | Parse the cartridge header and report loader compatibility/classification details. |
| `gb-cli saves export <rom> <out.sav> --save-dir <dir> [--save-key <key>]` | Convert GB Cycle cartridge persistence to an external `.sav` file. |
| `gb-cli saves import <rom> <in.sav> --save-dir <dir> [--save-key <key>]` | Import an external `.sav` into the runtime persistence format used by `gb-cli run --save-dir`. |

Use `cargo run -p gb-cli -- <command> ...` during development, or `gb-cli <command> ...` when running an installed binary.

## `inspect-rom`

```bash
cargo run -p gb-cli -- inspect-rom path/to/rom.gb --mode strict
```

`inspect-rom` prints stable `key=value` lines for cartridge header and loader policy diagnostics. The report includes declared ROM-size fields from `0x0148` (`rom_size_bytes`, `rom_bank_count`) plus loader-resolved fields (`effective_rom_size_bytes`, `effective_rom_bank_count`, `rom_size_source`). Rejected ROMs report effective layout fields as `unknown` because no cartridge device was constructed.

Use `--mode permissive` only when intentionally inspecting tolerant loader policy, such as supported MBC5 ROMs with contradictory size metadata. Keep `strict` for oracle, CI, and accuracy-related inspection.

## `run` basics

```bash
cargo run -p gb-cli -- run path/to/rom.gb --tcycles 5000 --serial-out .artifacts/serial.bin
cargo run -p gb-cli -- run path/to/rom.gb --frames 120 --framebuffer-out .artifacts/final.png
```

`run` advances the same T-cycle core used by tests and desktop. If neither `--frames` nor `--tcycles` is provided, `skip-boot` and `custom-boot` stop after `120` completed frames, while `real-boot` stops after boot-ROM handoff plus `120` completed post-handoff frames with a `480`-frame safety cap if handoff never arrives.

Completed serial bytes can stream to stdout with `--serial-stdout`, be captured at the end with `--serial-out <path>`, or both. Run summaries, diagnostics, save metadata, artifact paths, executed T-cycles, completed frames, and serial byte counts are written as stderr `key=value` lines so scripts can parse stdout independently.

## Models, revisions, startup, and mode

`--model` accepts only the public hardware-profile names `DMG`, `MGB`, `LGB`, `CGB`, `SGB`, and `SGB2`. Older aliases such as `game-boy`, `pocket`, `light`, `color`, `dmg`, `mgb`, or `cgb` are not accepted.

`--revision <dmg-cpu-c|cpu-mgb|cpu-cgb-c|cpu-cgb-d|cpu-cgb-e>` selects an active hardware revision for the chosen model and invalid pairs are rejected. `SGB` and `SGB2` use the DMG-compatible GB core behind an SGB host shell; `--sgb-standard <ntsc|pal>` is valid only with `--model SGB`, defaults to `ntsc`, and is rejected for `SGB2` because SGB2 uses its fixed NTSC profile.

`--startup <skip-boot|custom-boot|real-boot>` selects the startup path. `skip-boot` is the fast direct-start tooling path; `custom-boot` adds reset/boot-facing direct-start behavior used by selected fixtures; `real-boot` executes a private firmware image and requires `--boot-rom-dir <dir>` unless verification is explicitly relaxed.

`RealBoot` derives the firmware filename from the effective model/revision/profile. Examples include `dmg_boot.bin`, `mgb_boot.bin`, `cgb_boot.bin`, `cgbE_boot.bin`, `sgb_boot.bin`, and `sgb2_boot.bin`. `--boot-rom` no longer exists; use `--boot-rom-dir <private-dir>` plus `--boot-rom-verify <off|warn|strict>` when overriding firmware lookup or verification.

`--mode <strict|permissive|experimental>` selects cartridge loader and compatibility policy around the same hardware model. `strict` is the default and the only mode suitable for oracle and accuracy claims; `permissive` is for tolerant tooling; `experimental` is for explicitly partial/research paths.

## Automation and benchmarks

`--test-runner` applies host-light runner defaults after parsing: `--mode permissive`, DMG `--palette grey`, and `--border-off`. It does not change model, startup, T-cycle stepping, stop limits, RTC behavior, save persistence, or explicit artifact requests.

`--benchmark <case.toml>` loads one portable benchmark TOML through `gb-benchmark`; the TOML owns the ROM path, model, startup, mode, palette, screenshot/stat toggles, duration, and run inputs. It cannot be combined with a positional ROM path or normal run options; `--test-runner` is the supported CLI-side override.

Because `--benchmark` cannot be combined with normal run options and the benchmark TOML has no boot-ROM directory field, CLI benchmark cases should not use `startup = "real-boot"` until an explicit benchmark boot-ROM input is added.

Benchmark cases use `[[run]]` plus `[[run.input]]` entries. `button` or `buttons` choose inputs, exactly one of `frame`, `second`, or `tcycle` chooses timing, and `hold_frames` / `repeat_every_frames` expand deterministic pulses. Relative `rom` paths resolve against the TOML file directory, and `id` / `run.id` may contain only ASCII letters, digits, `-`, and `_` because they form artifact filenames.

`cargo rom-bench <case-dir> --gb-cli` compares CLI artifacts with desktop benchmark artifacts, writes outputs under `test/bench/`, and includes `gb-cli` columns in `test/bench/index.html` only when `--gb-cli` is present. Use `--test <case.toml>` for one case, `--normalize-case` to rename case files to ROM filename stems, and `--rom-dir <dir> --generate-cases [--template <case.toml>]` to generate normalized cases for `.gb` / `.gbc` ROMs.

## Output artifacts

- `--framebuffer-out <path>` writes the final `160x144` GB LCD framebuffer as PGM, or PNG when the path ends in `.png`.
- `--model CGB` PNG output uses the CGB RGB555 framebuffer directly.
- `--model SGB` and `--model SGB2` PNG output uses the `256x224` SGB host RGB555 frame by default.
- `--border-off` is accepted for all models but only affects SGB/SGB2 PNG output, where it hides the host border and writes the SGB-colored `160x144` LCD RGB555 output instead.
- Non-PNG SGB/SGB2 framebuffer artifacts keep the legacy `160x144` shade-index PGM path for automation compatibility.
- `--palette grey` affects only final effective `--model DMG` output; for PGM it writes 8-bit grey values instead of raw shade indices, while CGB and SGB-family PNG output continues to use RGB555.
- `--trace-out <path>` writes the in-memory scheduler trace text for the run.
- `--state-in <path>` restores a full-machine `.gbstate` after loading the ROM, and `--state-out <path>` writes a full-machine `.gbstate` at the end of the run.

## Battery saves and save states

`--save-dir <dir>` loads and stores battery-backed cartridge persistence. Default save keys use the exact ROM filename stem; use `--save-key <key>` only when the stem is unsuitable or when sharing one ROM path with a different logical save identity.

`--save-policy <manual|on-close|on-write>` requires `--save-dir` and defaults to `on-close`. `manual` loads an existing save without automatic writes, `on-close` flushes changed persistence at run completion, and `on-write` also flushes changed persistence at frame boundaries after cartridge writes.

Runtime persistence writes external `.sav` for mapper states that have a safe lossless external layout and reserves `.gbsav` for fallback-only internal state such as HuC-3, non-representable MBC6 state, or future mapper state without a safe external contract. The `run` path does not auto-load older sanitized names or legacy `.gbsav` files for external-stable carts; use `gb-cli saves export` / `gb-cli saves import` for explicit migration.

Full-machine `.gbstate` files are separate from cartridge saves. Restore validates model, operating mode, host platform, SGB profile, startup mode, compatibility policy, loaded ROM fingerprint, and boot-ROM fingerprint before mutating core state.

## `saves` conversion

```bash
cargo run -p gb-cli -- saves export path/to/rom.gb path/to/out.sav --save-dir path/to/saves
cargo run -p gb-cli -- saves import path/to/rom.gb path/to/in.sav --save-dir path/to/saves
```

Both conversion commands load the ROM first, then validate the save payload against the mapper and persistence profile instead of inferring compatibility from the filename. `saves export` reads the authoritative runtime save first and may probe a legacy `.gbsav` fallback for explicit migration; `saves import` reads the external path exactly as provided and writes the same authoritative runtime format that `run --save-dir` would use.

Linear cartridge RAM exports as raw bytes. MBC3 RTC exports the shared `48`-byte little-endian RTC suffix used by common external save formats and imports both the older `44`-byte timestamp form and the `48`-byte form. MBC2 export defaults to mGBA's `256`-byte packed format and import accepts both that layout and the `512`-byte one-byte-per-nibble layout. Mapper/profile combinations without a safe external mapping fail explicitly instead of producing partial saves.
