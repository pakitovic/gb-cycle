# gb-cli

Headless CLI runner for Game Boy and Game Boy Color product models.

## Subcommands

### `inspect-rom`

Print cartridge header information:

```bash
cargo run -p gb-cli -- inspect-rom path/to/rom.gb
```

### `run`

Execute a ROM headlessly:

```bash
cargo run -p gb-cli -- run path/to/rom.gb --tcycles 5000 --serial-out .artifacts/serial.bin
```

`--test-runner` marks the run as host-light automation without changing console model, startup mode, execution mode, T-cycle stepping, frame limits, or RTC behavior. The headless CLI already has no SDL menu, window title, audio playback, gamepad, rewind, or settings persistence, so the flag is primarily a shared frontend contract and still allows explicit artifacts such as `--serial-out`, `--framebuffer-out`, `--trace-out`, `--state-out`, and explicit `--save-dir` persistence.

### `saves`

Convert between GB Cycle's internal `.gbsav` envelope and the raw `.sav` layout used by SameBoy and mGBA:

```bash
cargo run -p gb-cli -- saves export path/to/rom.gb path/to/out.sav --save-dir path/to/saves
cargo run -p gb-cli -- saves import path/to/rom.gb path/to/in.sav --save-dir path/to/saves
```

Both commands load the ROM first, then validate the save payload against the cartridge mapper and persistence profile instead of inferring compatibility from the filename. `saves import` reads the selected external path exactly as provided, so extensionless files are valid when the caller supplies one. Use `--save-key <key>` when the internal `.gbsav` key differs from the ROM stem. By default the derived key preserves the ROM's exact filename stem, so `Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).gb` maps to `Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).gbsav`.

## Console models

`run` exposes the hardware-profile model names `DMG`, `MGB`, `LGB`, and `CGB` through `--model`; the previous product names `game-boy`, `pocket`, `light`, and `color`, plus the legacy aliases `dmg0`, `dmg`, `mgb`, and `cgb`, are not accepted. The selected model chooses the default `RealBoot` firmware kind for that product (`dmg_boot.bin`, `mgb_boot.bin`, or `cgb_boot.bin`), while concrete boot-ROM image handling remains owned by the boot-ROM search path and verification options.

## Startup and compatibility

- `run` supports `skip-boot`, `custom-boot`, and `real-boot`, plus `strict`, `permissive`, and `experimental` compatibility modes.
- `custom-boot` is a direct-start path for boot-logo-inspecting ROMs: it uses the same CPU/IO/hidden startup baseline as `skip-boot` and overlays the DMG boot-logo VRAM/map seed without loading a boot ROM asset.
- `real-boot` looks for boot ROM assets only in `GB_CYCLE_BOOT_ROM_ROOT` or an explicit `--boot-rom-dir` and can verify the expected boot ROM SHA-256 hashes.

## Output options

- `--framebuffer-out` writes the final `160x144` framebuffer as a binary PGM image, or as a real PNG when the output path ends in `.png`.
- `--palette grey` maps DMG-family framebuffer shade indices through the same `DMG_GREY_DISPLAY_PALETTE` grey RGB values used by `gb-desktop`, but only when the final effective `--model` is `DMG`; the option is parsed and ignored for `MGB`, `LGB`, and `CGB`. For PGM artifacts the override writes an 8-bit grey PGM (`maxval 255`) instead of the default raw shade-index PGM (`maxval 3`), while CGB PNG output continues to use the core RGB555 framebuffer directly.
- `--serial-out` writes captured serial output to a file.
- `--trace-out` writes the in-memory scheduler trace text for the run.

## Battery saves

`--save-dir` loads and stores battery-backed cartridge persistence using the host-side `.gbsav` format from `gb-persistence`. Default `.gbsav` names use the exact ROM filename stem plus the `.gbsav` extension; only path separators, control characters, and portable-filesystem reserved characters require an explicit `--save-key`. When a derived exact-stem save is missing, load/export paths still probe the previous sanitized underscore-style key so existing saves migrate naturally on the next write.

`gb-cli saves export` writes emulator-compatible `.sav` files at the host boundary without changing the internal `.gbsav` format. Linear cartridge RAM is exported as raw bytes; `MBC3` RTC saves append the shared `48`-byte little-endian RTC suffix used by SameBoy/mGBA; `MBC2` export defaults to mGBA's `256`-byte packed format while import accepts both mGBA packed saves and SameBoy's `512`-byte one-byte-per-nibble layout. Mapper/profile combinations without a safe external mapping fail explicitly instead of producing partial saves.

## Default stop conditions

If neither `--frames` nor `--tcycles` is provided:

- `skip-boot` and `custom-boot` stop after `120` completed frames by default.
- `real-boot` stops after boot-ROM handoff plus `120` completed post-handoff frames with a `480`-frame safety cap.
