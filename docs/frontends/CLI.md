# gb-cli

Headless CLI runner for the DMG family.

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

### `saves`

Convert between GB Cycle's internal `.gbsav` envelope and the raw `.sav` layout used by SameBoy and mGBA:

```bash
cargo run -p gb-cli -- saves export path/to/rom.gb path/to/out.sav --save-dir path/to/saves
cargo run -p gb-cli -- saves import path/to/rom.gb path/to/in.sav --save-dir path/to/saves
```

Both commands load the ROM first, then validate the save payload against the cartridge mapper and persistence profile instead of inferring compatibility from the filename. `saves import` reads the selected external path exactly as provided, so extensionless files are valid when the caller supplies one. Use `--save-key <key>` when the internal `.gbsav` key differs from the ROM stem. By default the derived key preserves the ROM's exact filename stem, so `Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).gb` maps to `Legend of Zelda, The - Link's Awakening (USA, Europe) (Rev 2).gbsav`.

## Console models

`run` currently exposes the DMG-family models `dmg0`, `dmg`, and `mgb`.

## Startup and compatibility

- `run` supports `skip-boot` and `real-boot`, plus `strict`, `permissive`, and `experimental` compatibility modes.
- `real-boot` looks for boot ROM assets in `GB_CYCLE_BOOT_ROM_ROOT` or the repo-local `/.roms/bootrom/` store and can verify the expected DMG-family SHA-256 hashes.

## Output options

- `--framebuffer-out` writes the final `160x144` framebuffer as a binary PGM image, or as a real PNG when the output path ends in `.png`.
- `--serial-out` writes captured serial output to a file.
- `--trace-out` writes the in-memory scheduler trace text for the run.

## Battery saves

`--save-dir` loads and stores battery-backed cartridge persistence using the host-side `.gbsav` format from `gb-persistence`. Default `.gbsav` names use the exact ROM filename stem plus the `.gbsav` extension; only path separators, control characters, and portable-filesystem reserved characters require an explicit `--save-key`. When a derived exact-stem save is missing, load/export paths still probe the previous sanitized underscore-style key so existing saves migrate naturally on the next write.

`gb-cli saves export` writes emulator-compatible `.sav` files at the host boundary without changing the internal `.gbsav` format. Linear cartridge RAM is exported as raw bytes; `MBC3` RTC saves append the shared `48`-byte little-endian RTC suffix used by SameBoy/mGBA; `MBC2` export defaults to mGBA's `256`-byte packed format while import accepts both mGBA packed saves and SameBoy's `512`-byte one-byte-per-nibble layout. Mapper/profile combinations without a safe external mapping fail explicitly instead of producing partial saves.

## Default stop conditions

If neither `--frames` nor `--tcycles` is provided:

- `skip-boot` stops after `120` completed frames by default.
- `real-boot` stops after boot-ROM handoff plus `120` completed post-handoff frames with a `480`-frame safety cap.
