# gb-cli

Headless CLI runner for Game Boy and Game Boy Color product models.

## Subcommands

### `inspect-rom`

Print cartridge header information:

```bash
cargo run -p gb-cli -- inspect-rom path/to/rom.gb
```

Use `--mode <strict|permissive|experimental>` when the inspection should evaluate cartridge-loader compatibility under a specific policy preset; the default is `strict`.

The report includes the declared ROM-size fields from `0x0148` (`rom_size_bytes`, `rom_bank_count`) plus loader-resolved fields (`effective_rom_size_bytes`, `effective_rom_bank_count`, `rom_size_source`). `rom_size_source=declared-exact` means the loaded mapper uses the header declaration directly; `rom_size_source=permissive-rounded-actual` means a tolerant policy admitted a supported mapper with contradictory size metadata and derived the mapper capacity from the file length. Rejected ROMs report these effective fields as `unknown` because no cartridge device was constructed.

### `run`

Execute a ROM headlessly:

```bash
cargo run -p gb-cli -- run path/to/rom.gb --tcycles 5000 --serial-out .artifacts/serial.bin
```

`--test-runner` marks the run as host-light automation without changing console model, startup mode, execution mode, T-cycle stepping, frame limits, or RTC behavior. The headless CLI already has no SDL menu, window title, audio playback, gamepad, rewind, or settings persistence, so the flag is primarily a shared frontend contract and still allows explicit artifacts such as `--serial-out`, `--framebuffer-out`, `--trace-out`, `--state-out`, and explicit `--save-dir` persistence.

`--benchmark <path>` loads one portable benchmark TOML through `gb-benchmark` instead of taking a positional ROM path. The TOML owns the ROM path, console model, startup mode, compatibility mode, optional DMG grey palette, screenshot/stat toggles, and one or more fresh `[[run]]` entries. Each run starts from a new machine, applies expanded input pulses through `Machine::set_joypad_button_pressed`, uses its own duration as the final screenshot time, is bounded by the matching T-cycle duration even if frame-origin progress freezes, and writes `gb-cli/<artifact-id>.png` plus `gb-cli/<artifact-id>-stats.toml` when enabled. Concise run inputs use `[[run.input]]` with `button` or `buttons`, exactly one of `frame` / `second` / `tcycle`, optional `hold_frames`, and optional `repeat_every_frames`; relative `rom` paths resolve against the TOML file directory, and `id` / `run.id` must use only ASCII letters, digits, `-`, and `_` because they form artifact filenames; the old top-level `duration_seconds` plus `[[stimulus]]` format is not accepted.

`scripts/run-benchmark.sh <case-dir> --gb-cli` is the local helper path when CLI benchmark artifacts should be compared with desktop artifacts: it reads `*.toml` directly under `<case-dir>`, writes artifacts under `scripts/benchmark/` next to the script, and includes the `gb-cli` columns in `scripts/benchmark/index.html` only when `--gb-cli` is explicitly present. Before launching either frontend, the helper resolves each case `rom` relative to the TOML file, skips cases whose ROM is missing, empty, or unreadable, and only runs benchmarks for the remaining valid cases. For a single case, `scripts/run-benchmark.sh --test path/to/game.toml --gb-cli` may omit `<case-dir>` because the helper infers it from the TOML path. Use `--normalize-case` to rename existing case TOMLs to the exact ROM filename stem, and use `--rom-dir <rom-dir> --generate-cases [--template <case.toml>]` to generate normalized case TOMLs for every `.gb` or `.gbc` ROM while keeping artifact `id` values safe for benchmark output filenames.

### `saves`

Convert between GB Cycle cartridge persistence and the raw `.sav` layout used by SameBoy and mGBA:

```bash
cargo run -p gb-cli -- saves export path/to/rom.gb path/to/out.sav --save-dir path/to/saves
cargo run -p gb-cli -- saves import path/to/rom.gb path/to/in.sav --save-dir path/to/saves
```

Both commands load the ROM first, then validate the save payload against the cartridge mapper and persistence profile instead of inferring compatibility from the filename. `saves export` reads the current authoritative P1 runtime save first (`.sav` when external-stable, `.gbsav` for internal-only or fallback state); for external-stable carts with no `.sav`, it also probes the legacy internal `.gbsav` envelope for explicit migration. `saves import` reads the selected external path exactly as provided, so extensionless files are valid when the caller supplies one, and then writes the same authoritative P1 storage format that `run --save-dir` would use for the mapper: `.sav` when lossless externally, `.gbsav` only for fallback-only state. Use `--save-key <key>` when the save key differs from the ROM stem.

## Console models

`run` exposes the hardware-profile model names `DMG`, `MGB`, `LGB`, and `CGB` through `--model`; the previous product names `game-boy`, `pocket`, `light`, and `color`, plus the legacy aliases `dmg0`, `dmg`, `mgb`, and `cgb`, are not accepted. `--revision <dmg-cpu-c|cpu-mgb|cpu-cgb-c|cpu-cgb-d|cpu-cgb-e>` selects the active hardware revision for the chosen model; invalid model/revision pairs are rejected, so `--model CGB --revision cpu-cgb-e` is valid and `--model DMG --revision cpu-cgb-e` is not. `RealBoot` derives the concrete firmware filename from the effective model/revision pair (`dmg_boot.bin`, `mgb_boot.bin`, `cgb_boot.bin`, or `cgbE_boot.bin`) and `--boot-rom` no longer exists; a CGB-E RealBoot validation uses `--model CGB --revision cpu-cgb-e --boot-rom-dir <private-dir>` with a directory containing `cgbE_boot.bin`.

## Startup and compatibility

- `run` supports `skip-boot`, `custom-boot`, and `real-boot`, plus `strict`, `permissive`, and `experimental` compatibility modes.
- `--mode permissive` may load explicit supported MBC5 homebrew/public-domain ROMs whose `0x0148` ROM-size metadata is unsupported or contradicts the file length; the core emits loader diagnostics, pads missing ROM bytes with `0xFF`, and keeps MBC5 banking semantics unchanged. Keep `strict` for oracle runs, differential comparisons, and accuracy claims.
- `custom-boot` is a direct-start path for boot-logo-inspecting ROMs: it uses the same CPU/IO/hidden startup baseline as `skip-boot` and overlays the DMG boot-logo VRAM/map seed without loading a boot ROM asset.
- `real-boot` looks for the revision-derived boot ROM asset only in `GB_CYCLE_BOOT_ROM_ROOT` or an explicit `--boot-rom-dir`; `--boot-rom-verify <off|warn|strict>` controls whether a missing boot-ROM root or expected SHA-256 mismatch is ignored, reported as a warning, or rejected, and defaults to `strict`. `skip-boot` and `custom-boot` do not read boot-ROM bytes; they use the direct-start state for the selected model/revision.

## Output options

- `--framebuffer-out` writes the final `160x144` framebuffer as a binary PGM image, or as a real PNG when the output path ends in `.png`.
- `--palette grey` maps DMG-family framebuffer shade indices through the same `DMG_GREY_DISPLAY_PALETTE` grey RGB values used by `gb-desktop`, but only when the final effective `--model` is `DMG`; the option is parsed and ignored for `MGB`, `LGB`, and `CGB`. For PGM artifacts the override writes an 8-bit grey PGM (`maxval 255`) instead of the default raw shade-index PGM (`maxval 3`), while CGB PNG output continues to use the core RGB555 framebuffer directly.
- `--serial-out` writes captured serial output to a file.
- `--serial-stdout` streams completed serial bytes to stdout as they arrive.
- `--trace-out` writes the in-memory scheduler trace text for the run.

## Battery saves

`--save-dir` loads and stores battery-backed cartridge persistence using `.sav` as the P1 runtime file when the mapper state is representable without loss in an external save. Default names use the exact ROM filename stem, so `Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).gb` maps to `Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).sav` for external-stable cartridges; only path separators, control characters, and portable-filesystem reserved characters require an explicit `--save-key`. The CLI `run` path does not probe older sanitized save names or auto-load legacy `.gbsav` files for external-stable carts; use `gb-cli saves export` / `gb-cli saves import` for explicit legacy migration when needed.

`--save-policy <manual|on-close|on-write>` selects automatic flush behavior when `--save-dir` is present and defaults to `on-close`; `manual` loads any existing save without automatic writes, `on-close` writes changed persistence at run completion, and `on-write` also flushes changed persistence at frame boundaries after cartridge writes.

`gb-cli saves export` writes emulator-compatible `.sav` files at the host boundary without changing existing source files. Runtime saving writes `.sav` for external-stable mappers, and reserves `.gbsav` for HuC-3, non-representable MBC6 state, and future mapper state without a safe external contract; export follows that runtime path first so a save created by `run --save-dir` or `saves import` can be exported again, while still probing a legacy `.gbsav` as a migration fallback when no authoritative runtime save exists. Linear cartridge RAM is exported as raw bytes; `MBC3` RTC saves export the shared `48`-byte little-endian RTC suffix used by SameBoy/mGBA and import both the older `44`-byte timestamp form and the `48`-byte form; `MBC2` export defaults to mGBA's `256`-byte packed format while import accepts both mGBA packed saves and SameBoy's `512`-byte one-byte-per-nibble layout. Mapper/profile combinations without a safe external mapping fail explicitly for conversion and use the internal fallback for runtime storage instead of producing partial saves.

## Default stop conditions

If neither `--frames` nor `--tcycles` is provided:

- `skip-boot` and `custom-boot` stop after `120` completed frames by default.
- `real-boot` stops after boot-ROM handoff plus `120` completed post-handoff frames with a `480`-frame safety cap.
