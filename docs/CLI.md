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

`--save-dir` loads and stores battery-backed cartridge persistence using the host-side `.gbsav` format from `gb-persistence`.

## Default stop conditions

If neither `--frames` nor `--tcycles` is provided:

- `skip-boot` stops after `120` completed frames by default.
- `real-boot` stops after boot-ROM handoff plus `120` completed post-handoff frames with a `480`-frame safety cap.
