# External ROM suites

The repository keeps synthetic ROM fixtures under version control, but official external ROM suites stay outside git in a repo-managed local store.

## Fetching ROMs

```bash
make fetch-test-roms
make fetch-test-roms FAMILIES=blargg
make fetch-test-roms FAMILIES="blargg acid"
```

- `make fetch-test-roms` fetches the pinned upstream source from `GBEmulatorShootout` into a temporary checkout, materializes the curated runnable store under `/.roms/test/`, and removes the raw checkout afterwards.
- By default it fetches `all`, but it can materialize one or more explicit families through `FAMILIES=...`.
- The pinned upstream source is always `GBEmulatorShootout`, recorded in `crates/gb-test-runner/data/sources.toml`.

## ROM store layout

`/.roms/test/` is organized by family:

```text
/.roms/test/acid/
/.roms/test/blargg/
/.roms/test/daid/
/.roms/test/hacktix/
/.roms/test/mealybug-tearoom-tests/
/.roms/test/mooneye/
```

Each curated family directory contains only the ROMs currently listed in the matching manifest under `crates/gb-test-runner/data/*.toml`.

## Running suites

```bash
make test-roms         # fetch if needed + run all local curated DMG suites
make test-roms-real-boot # fetch if needed + run all local curated DMG suites through verified RealBoot
make run-blargg        # curated Blargg DMG family (includes dmg_sound 01..12)
make run-blargg-cpu-instrs # Blargg CPU instruction chunk used by CI
make run-blargg-dmg-sound # Blargg DMG sound chunk used by CI
make run-blargg-timing-memory-oam # Blargg timing/memory/OAM chunk used by CI
make run-acid          # curated Acid DMG family
make run-daid          # exploratory daid DMG subset
make run-cpp           # curated cpp MBC3 subset
make run-hacktix       # curated hacktix DMG subset
make run-mealybug      # exploratory mealybug-tearoom DMG subset
make run-mooneye       # exploratory mooneye DMG acceptance subset
make run-mooneye-acceptance # Mooneye acceptance/manual chunk used by CI
make run-mooneye-mbc1-mbc5 # Mooneye emulator-only MBC1/MBC5 chunk used by CI
make run-mooneye-mbc2  # Mooneye emulator-only MBC2 chunk used by CI
make test-roms-cgb     # fetch if needed + run the promoted green local curated CGB suites
make test-roms-cgb-real-boot # fetch if needed + run the promoted green local curated CGB suites through verified RealBoot
make run-cgb-smoke     # manifest-backed Phase 10 CGB smoke suite
make run-cgb-boot-div  # manifest-backed Phase 10 CGB boot DIV suite
make run-cgb-boot-hwio # exploratory/internal Phase 10 CGB boot HWIO suite
make run-cgb-speed     # manifest-backed Phase 10 CGB KEY1/speed suite
make run-cgb-ppu-basic # manifest-backed Phase 10 CGB PPU baseline suite
make phase9-determinism-smoke # replay/save-load smoke checks for Phase 2 and Phase 6 fixtures
make phase9-determinism-local # replay/save-load sample across CPU/interrupts, Mooneye Timer/DMA, Acid/Mealybug PPU, cartridge, and one APU Blargg case
make phase9-diff-cartridge    # compare Phase 6 cartridge artifacts against SameBoy case-bundle output
make phase9-diff-acid         # compare Acid framebuffer artifacts against LibSameBoy case-bundle output
make phase9-diff-mealybug     # compare the SameBoy-PASS Mealybug framebuffer subset against LibSameBoy case-bundle output
make phase9-diff-hacktix      # compare Hacktix framebuffer artifacts against LibSameBoy case-bundle output
make phase9-first-divergence-hacktix # capture Hacktix local/LibSameBoy first-divergence probe windows
```

The aggregate `make test-roms-real-boot` and `make test-roms-cgb-real-boot` ROM-suite targets are local-only validation lanes. They require `GB_CYCLE_BOOT_ROM_ROOT` to point at a private boot-ROM directory with canonical filenames such as `dmg_boot.bin` or `cgb_boot.bin`, set `GB_CYCLE_TEST_ROM_STARTUP=real-boot` while invoking the normal `run-*` suite targets, run clean `RealBoot` without synthetic `SkipBoot` startup profiles, start each case timeout after the `FF50` handoff, and update the same `/.roms/test/test-report.md` rows as the default targets. Re-run `make test-roms` after a DMG RealBoot pass or `make test-roms-cgb` after a CGB RealBoot pass if you want the report to reflect the default SkipBoot baseline again. The CGB aggregate target `make test-roms-cgb-real-boot` drives the same promoted-green CGB suite list as `make test-roms-cgb`, uses strict canonical `cgb_boot.bin` size/hash verification, and clears both startup-memory and startup-timer profiles through the `run_rom_suite` CLI so the executed boot firmware owns the handoff state.

Each `make run-*` target is autosufficient and materializes its own curated family before execution.

### Direct runner invocations

```bash
# Run a specific built-in suite
cargo run -p gb-test-runner --bin run_rom_suite -- --suite acid-dmg-curated

# Run full Blargg family (including dmg_sound)
cargo run -p gb-test-runner --bin run_rom_suite -- --suite blargg-dmg-curated

# Run a CI-sized Blargg chunk
cargo run -p gb-test-runner --bin run_rom_suite -- --suite blargg-dmg-cpu-instrs

# List all built-in suites and oracle channels
cargo run -p gb-test-runner --bin run_rom_suite -- --list-detailed

# Show early hardening status by subsystem
cargo run -p gb-test-runner --bin run_rom_suite -- --early-checklist

# Run deterministic replay plus in-memory save/load continuation checks
cargo run -p gb-test-runner --bin run_determinism -- --suite phase-2-cpu-timing
```

### Retaining failure artifacts

```bash
# Mealybug mismatch artifacts
cargo run -p gb-test-runner --bin run_rom_suite -- \
  --suite mealybug-tearoom-dmg-curated \
  --failure-artifact-root .artifacts/mealybug-curated

# Mooneye failing snapshots
cargo run -p gb-test-runner --bin run_rom_suite -- \
  --suite mooneye-acceptance-dmg-curated \
  --failure-artifact-root .artifacts/mooneye-acceptance
```

## Test report

The runner updates `/.roms/test/test-report.md` with a `family | rom | status` table when a curated family suite executes, using `✅`, `❌` and `ℹ️` in the status column, adding a `non-failing/total` summary in the header, and keeping each family's pinned GBEmulatorShootout source order from `crates/gb-test-runner/data/sources.toml`. Same-ROM model variants are ordered DMG before GBC, and manifest order is only the fallback for cases without a pinned source path.

For GBEmulatorShootout rows whose label includes a model suffix, the associated manifest case must carry both `console = "dmg"` or `console = "cgb"` and `report_model_suffix = true`; this keeps rows such as `which.gb (DMG)` and `which.gb (GBC)` visible without adding a suffix to rows whose upstream label has none.

## Curated family details

### Acid

Mixes one blocking framebuffer oracle `dmg-acid2.gb` with one informational framebuffer capture case `which.gb`, matching the upstream `GBEmulatorShootout` classification.

### Blargg

Uses only individual ROMs from `GBEmulatorShootout` (not multi-ROM bundles such as `cpu_instrs.gb`). Includes the DMG `dmg_sound 01..12` individual ROMs. The full built-in suite remains `blargg-dmg-curated`; CI runs the same case set through three filtered chunks, `blargg-dmg-cpu-instrs`, `blargg-dmg-sound`, and `blargg-dmg-timing-memory-oam`, so the CPU-heavy and APU-heavy cases do not keep one Blargg matrix job much longer than the smaller ROM-suite jobs.

The upstream `oam_bug/7-timing_effect.gb`, CGB-only ROMs, and other still-red cases stay outside the default managed block until intentionally promoted.

### Hacktix

Tracks `bully.gb` and `strikethrough.gb` from `GBEmulatorShootout`, uses framebuffer fixtures; exercised by the GitHub `test-roms` workflow.

### Cpp

Curated `cpp` MBC3 subset; exercised by the GitHub `test-roms` workflow.

### Daid

Workflow-managed DMG subset; mixes framebuffer fixtures, one multi-fixture framebuffer oracle for `ppu_scanline_bgp.gb`, an absolute grayscale fixture for `stop_instr.gb (DMG)`, and one informational framebuffer capture case `rom_and_ram.gb`.

### Mealybug-tearoom

Workflow-managed DMG subset using committed framebuffer fixtures for the curated green cases from `GBEmulatorShootout`; exercised by the GitHub `test-roms` workflow.

The full local gate remains `mealybug-tearoom-dmg-curated` and keeps all 24 curated cases, including cases where gb-cycle passes but the current GBEmulatorShootout table marks SameBoy as `FAIL`.

The Phase `9` SameBoy differential uses the narrower built-in suite `mealybug-tearoom-dmg-sameboy-differential`, which excludes the nine Mealybug rows that GBEmulatorShootout updated on March 22, 2026 marks as SameBoy non-PASS: `mealybug-m3-lcdc-bg-en-change`, `mealybug-m3-lcdc-bg-map-change`, `mealybug-m3-lcdc-obj-size-change`, `mealybug-m3-lcdc-obj-size-change-scx`, `mealybug-m3-lcdc-tile-sel-change`, `mealybug-m3-lcdc-tile-sel-win-change`, `mealybug-m3-lcdc-win-en-change-multiple-wx`, `mealybug-m3-lcdc-win-map-change`, and `mealybug-m3-scy-change`.

Do not treat those excluded cases as gb-cycle regressions just because `mealybug-tearoom-dmg-curated` diverges from SameBoy; for Phase `9.3`, the full local fixture gate is accepted as the gb-cycle signal and the SameBoy divergence is recorded as an oracle limitation unless stronger hardware-facing evidence or another passing oracle supersedes it.

### Mooneye

Workflow-managed DMG acceptance subset following the active `GBEmulatorShootout` `testroms/mooneye.py` acceptance list. Uses the upstream `mooneye` breakpoint/register result protocol instead of framebuffer oracles, with the documented manual sprite-priority exception handled by a committed framebuffer fixture; this is broad hardening evidence for the accepted Phase `9` closure matrix. The full built-in suite remains `mooneye-acceptance-dmg-curated`; CI runs the same case set through three filtered chunks, `mooneye-dmg-acceptance-manual`, `mooneye-dmg-emulator-mbc1-mbc5`, and `mooneye-dmg-emulator-mbc2`, so the mapper-heavy cases do not keep one Mooneye matrix job much longer than the smaller ROM-suite jobs.

## Exploratory CGB suites

```sh
make run-cgb-smoke
make run-cgb-boot-div
make run-cgb-boot-hwio
make run-cgb-speed
make run-cgb-ppu-basic
make run-cgb-dma
make run-cgb-audio-blargg
make run-cgb-audio-samesuite
make test-roms-cgb-real-boot
```

- `cgb-smoke` is the Phase `10` CGB catalog suite, not a repo-gated DMG closure lane; its ROM inventory is declared in `crates/gb-test-runner/data/sources.toml`, its suite definition is `crates/gb-test-runner/data/cgb-smoke.toml`, the cases use the default centralized CGB `SkipBoot` handoff like the DMG `which.gb` informational lane, and `make run-cgb-smoke` fetches `mooneye acid` before invoking `run_rom_suite` without requiring boot ROM assets. Verified CGB `RealBoot` coverage for the same rows remains available through `make test-roms-cgb-real-boot`.
- `cgb-boot-div` is the Phase `10` CGB boot/DIV timing gate, not a repo-gated DMG closure lane; its ROM inventory is declared in `crates/gb-test-runner/data/sources.toml`, its suite definition is `crates/gb-test-runner/data/cgb-boot-div.toml`, it uses the default centralized CGB `SkipBoot` handoff so CI does not need boot ROM assets, and `make run-cgb-boot-div` fetches `mooneye` before invoking `run_rom_suite`.
- `cgb-boot-div` currently runs Mooneye `misc/boot_div-cgbABCDE.gb` on `ConsoleModel::GameBoyColor` with a blocking `mooneye-result` oracle. It validates the CGB handoff/DIV timer baseline through the centralized CGB `SkipBoot` state; optional local `RealBoot` comparison remains available only through explicit `GB_CYCLE_TEST_ROM_STARTUP=real-boot` runs.
- `cgb-boot-hwio` is the Slice `6` exploratory/internal CGB HWIO snapshot suite, not a blocking DMG or CGB aggregate signal yet; its suite definition is `crates/gb-test-runner/data/cgb-boot-hwio.toml`, its source row is `testroms/mooneye/misc/boot_hwio-C.gb` with pinned SHA-256 in `sources.toml`, it uses the default centralized CGB `SkipBoot` handoff so CI does not need boot ROM assets, `make run-cgb-boot-hwio` fetches `mooneye`, and the current oracle is `info-snapshot` until expected HWIO values and promotion rules are documented.
- `cgb-speed` is the Phase `10` Slice `2` exploratory CGB speed-domain suite, not a repo-gated DMG closure lane; its ROM inventory is declared in `crates/gb-test-runner/data/sources.toml`, its suite definition is `crates/gb-test-runner/data/cgb-speed.toml`, and `make run-cgb-speed` fetches `daid blargg` before invoking `run_rom_suite`.
- `cgb-speed` now promotes Daid `stop_instr.gb (GBC)` to a blocking final `framebuffer-rgb555-grayscale-fixture` using `crates/gb-test-runner/data/fixtures/daid/stop_instr.gbc.png`, preserving the absolute solid-black STOP result through a grayscale decode of the CGB RGB555 framebuffer; `stop_instr_gbc_mode3.gb` is a blocking rank-normalized `framebuffer-rgb555-fixture` using `crates/gb-test-runner/data/fixtures/daid/stop_instr_gbc_mode3.png`, matching the SameBoy/GBEmulatorShootout PASS screen where CGB STOP entered during Mode `3` leaves the LCD displaying the PASS text; `speed_switch_timing_div.gbc`, `speed_switch_timing_ly.gbc`, and `speed_switch_timing_stat.gbc` are blocking rank-normalized `framebuffer-rgb555-fixture` oracles using their matching `crates/gb-test-runner/data/fixtures/daid/speed_switch_timing_*.png` artifacts. These Daid cases use a `180`-frame budget so the terminal STOP or timing output has been presented to the framebuffer before comparison. Blargg `interrupt_time.gb` is promoted to a blocking `blargg-console-contains` oracle with expected text `Passed` and a `1800`-frame budget, because the CGB run emits its result through the upstream BG-map console rather than serial. Every current `cgb-speed` row now has a blocking oracle.
- `cgb-ppu-basic` is the Phase `10` Slice `4` CGB PPU baseline promotion suite, not a repo-gated DMG closure lane; its ROM inventory is declared in `crates/gb-test-runner/data/sources.toml`, its suite definition is `crates/gb-test-runner/data/cgb-ppu-basic.toml`, and `make run-cgb-ppu-basic` fetches `samesuite daid acid hacktix` before invoking `run_rom_suite`.
- `cgb-ppu-basic` currently contains four blocking rows in roadmap order: SameSuite `ppu/blocking_bgpi_increase.gb`, using the `framebuffer-rgb555-fixture` oracle at `crates/gb-test-runner/data/fixtures/samesuite/ppu/blocking_bgpi_increase.png`; Daid `ppu_scanline_bgp.gb (GBC)`, using the `framebuffer-rgb555-fixture` oracle against `crates/gb-test-runner/data/fixtures/daid/ppu_scanline_bgp.gbc.png`; Acid `cgb-acid2.gbc`, using the `framebuffer-rgb555-fixture` oracle against `crates/gb-test-runner/data/fixtures/acid/cgb-acid2-cgb.png`; and Hacktix `bully.gb (GBC)`, using the `framebuffer-rgb555-fixture` oracle against `crates/gb-test-runner/data/fixtures/hacktix/bully.cgb.png`, the boot-logo VRAM startup-memory profile already used by the DMG Hacktix row, and the explicit `hacktix-cgb-bully-div` startup-timer profile so BullyGB's unconfirmed initial-`DIV` check does not replace the Mooneye-owned CGB direct-boot timer baseline.
- `cgb-audio-blargg` is the Phase `10` Slice `7` CGB audio baseline suite, not a repo-gated DMG closure lane; its ROM inventory is declared in `crates/gb-test-runner/data/sources.toml`, its suite definition is `crates/gb-test-runner/data/cgb-audio-blargg.toml`, and `make run-cgb-audio-blargg` fetches `blargg` before invoking `run_rom_suite`.
- `cgb-audio-blargg` contains the twelve upstream Blargg `cgb_sound` individual ROMs `01-registers.gb` through `12-wave.gb`, runs each on `ConsoleModel::GameBoyColor`, uses `blargg-memory-text-contains` with expected text `Passed`, retains memory-text plus snapshot failure artifacts, and is the first CGB audio ROM gate before promoting deeper SameSuite APU rows.
- `cgb-audio-samesuite` is the Phase `10` Slice `7` advanced CGB APU suite; its ROM inventory and PNG fixture hashes are declared in `crates/gb-test-runner/data/sources.toml`, its suite definition is `crates/gb-test-runner/data/cgb-audio-samesuite.toml`, and `make run-cgb-audio-samesuite` fetches `samesuite` before invoking `run_rom_suite` with retained RGB555 framebuffer and snapshot artifacts. The manifest tracks all `61` SameSuite APU rows in roadmap coarse-to-fine order, but the target is standalone rather than part of `make test-roms-cgb` until the shared APU primitives are promoted enough for the suite to become a green closure gate.
- Keep exploratory CGB suites outside the DMG `make test-roms` and GitHub `test-roms` workflow until promoted intentionally; CGB failures during bring-up should produce retained artifacts without changing the accepted DMG `167/167` signal, while `make test-roms-cgb` aggregates the green CGB suite targets promoted by Phase `10` slices and `make test-roms-cgb-real-boot` reruns that same aggregate through verified CGB RealBoot for local closure evidence.

## CI integration

- `make ci` stays as the fast local pre-push gate and does not fetch or run external ROM suites; it includes the Rust checks plus the coverage threshold gate through `cargo cov-check`.
- `make test-roms` fetches the curated ROM store if needed and runs all local curated DMG suites currently wired in `Makefile`: `acid`, the full Blargg lane via the three `run-blargg-*` chunks, `daid`, `hacktix`, `cpp`, `mealybug-tearoom-tests`, and the full Mooneye lane via the three `run-mooneye-*` chunks.
- GitHub uses two workflows: `ci` for Rust checks plus coverage, `test-roms` for the workflow-managed ROM subset currently exercised in CI: `acid`, `blargg-cpu-instrs`, `blargg-dmg-sound`, `blargg-timing-memory-oam`, `daid`, `hacktix`, `cpp`, `mooneye-acceptance`, `mooneye-mbc1-mbc5`, `mooneye-mbc2`, and `mealybug-tearoom-tests`.
- The GitHub `test-roms` workflow fans those suites out through a matrix; every matrix child performs its own checkout, Rust toolchain setup, and Rust cache restore because GitHub-hosted runners are isolated per job.

## Commercial ROM testing

Keep private commercial ROMs out of the curated store and outside repository-managed ROM stores. For local-only smoke, point a manifest at developer-owned external storage through an explicit `external_rom_root_key`; do not document or standardize the private filesystem path in the repo, and never reference those assets from CI.

For ad hoc local commercial-ROM bring-up, `run_rom_suite` accepts `--manifest <path>` with typed per-case metadata and deterministic joypad stimuli. When a manifest-driven case captures the framebuffer, the runner writes a sibling PNG next to the ROM using the ROM stem; CGB-family captures export the RGB555 framebuffer channel when present, while DMG-family captures keep the grayscale PNG conversion.

### Example manifest

```toml
version = 1

[[case]]
id = "tetris-dmg-start"
rom = "tetris.gb"
external_rom_root_key = "GB_CYCLE_PRIVATE_ROM_ROOT"
console = "dmg"
startup = "real-boot"
mode = "strict"
timeout_frames = 760
oracle = "info-framebuffer"

[[case.stimulus]]
frame = 650
button = "start"
pressed = true

[[case.stimulus]]
frame = 690
button = "start"
pressed = false
```

```bash
cargo run -p gb-test-runner --bin run_rom_suite -- --manifest .artifacts/tetris-start.toml
```

The final framebuffer PNG lands beside the resolved private ROM path using the ROM stem, and that artifact remains outside repo-managed oracle stores.

### Audio and menu investigation

For audio issues that depend on deterministic in-game inputs, prefer a manifest-driven local case with `oracle = "info-trace"` and a tight `timeout_tcycles` or `timeout_frames` window around the menu interaction you want to inspect.

- `trace.txt` is a rolling recent-history window rather than an unbounded full-run log; the current runner keeps the most recent `8192` T-cycles so the artifact stays focused on the final interaction window.
- CPU trace lines already include the last bus access, so APU MMIO writes remain visible there as `last_bus_activity=write@0xFFxx=0xyy`.
- The current APU scheduler trace now records powered state, `NR50`, `NR51`, live `NR52`, active and DAC masks, per-channel digital outputs, and current mixer/HPF output each traced T-cycle, which makes short menu-driven audio regressions inspectable without involving SDL.

Example shape:

```toml
version = 1

[[case]]
id = "pokemon-gold-menu-audio"
rom = "pokemon_gold.gbc"
external_rom_root_key = "GB_CYCLE_PRIVATE_ROM_ROOT"
console = "dmg"
startup = "real-boot"
mode = "strict"
timeout_frames = 920
oracle = "info-trace"

[[case.stimulus]]
frame = 870
button = "start"
pressed = true

[[case.stimulus]]
frame = 874
button = "start"
pressed = false
```

## Determinism and save/load continuation

`run_determinism` is the accepted Phase `9` in-memory determinism lane:

```bash
cargo run -p gb-test-runner --bin run_determinism -- --suite phase-2-cpu-timing
cargo run -p gb-test-runner --bin run_determinism -- --suite phase-6-cartridge-oracle --save-at-tcycles 1024 --continuation-tcycles 1024
make phase9-determinism-smoke
make phase9-determinism-local
```

For each selected strict case, the runner performs two independent replays, compares the final `MachineSaveState` plus serial output, captures a mid-run save state, dirties and restores the machine, checks continuation against the uninterrupted run, and verifies that a mismatched console-model restore is rejected. Non-`Strict` cases intentionally fail fast so this path remains usable as closure evidence instead of permissive compatibility evidence.

## Differential oracle testing

### LibSameBoy case-bundle artifacts

The Phase `9` SameBoy materialization path uses a small repo-owned C helper linked against SameBoy's `lib` target:

```bash
cargo run -p gb-test-runner --bin run_sameboy_case_bundle -- \
  --suite phase-6-cartridge-oracle \
  --sameboy-root /path/to/SameBoy \
  --build-if-missing

cargo run -p gb-test-runner --bin run_sameboy_case_bundle -- \
  --suite acid-dmg-curated \
  --sameboy-root /path/to/SameBoy \
  --build-if-missing
```

When `--oracle-root` is omitted, artifacts are written under `/.oracles/sameboy/case-bundle/<case-id>/`. Serial-hex cases emit `serial_hex.txt`; framebuffer cases emit `framebuffer.pgm`. The helper applies the suite's startup cartridge RTC seconds, startup memory writes, and the synthetic SkipBoot internal `DIV` phase before execution, so cartridge and PPU lanes share the same controlled LibSameBoy entrypoint instead of depending on a prebuilt SameBoy application binary.

### First-divergence probe windows

The Phase `9` first-divergence lane reuses the LibSameBoy helper but asks it for periodic JSONL probes instead of only final artifacts:

```bash
cargo run -p gb-test-runner --bin run_first_divergence -- \
  --oracle sameboy \
  --suite hacktix-dmg-curated \
  --probe-interval-tcycles 70224 \
  --build-if-missing
```

When `--probe-root` is omitted, local and SameBoy probe streams are written under `/.oracles/sameboy/first-divergence/<case-id>/local_probes.jsonl` and `sameboy_probes.jsonl`. The default `--compare-mode framebuffer` compares normalized framebuffer hashes and keeps CPU registers, timer/IRQ registers, PPU timing/register values, raw VRAM/OAM/WRAM/HRAM hashes, and serial output as context; `--compare-mode state` compares all captured state fields except probe timestamp drift. Use `--allow-divergence` for exploratory Make targets such as `phase9-first-divergence-hacktix`, where the command should report the first known intermediate timing window while still returning success for local investigation.

### SameBoy Tester compatibility path

To materialize SameBoy Tester artifacts under a compatible oracle root:

```bash
cargo run -p gb-test-runner --bin run_sameboy_tester -- \
  --sameboy-root /path/to/SameBoy \
  --suite acid-dmg-curated \
  --image-format bmp \
  --build-if-missing
```

This stages ROMs under the default repo-local oracle root `/.oracles/sameboy/sameboy-tester/`, runs SameBoy's internal `tester` binary, and leaves `.bmp` / `.tga` plus `.log` artifacts in the `sameboy-tester` layout that `run_differential` can consume directly.

SameBoy Tester always boots through a boot ROM, so this path is best suited to end-of-test framebuffer convergence rather than boot-path arbitration. The current wrapper intentionally does not override SameBoy's boot-ROM path. If you need a specific SameBoy firmware choice for oracle generation, control it from the SameBoy checkout or build itself rather than through `gb-test-runner`.

### Running differentials

```bash
cargo run -p gb-test-runner --bin run_differential -- \
  --oracle sameboy \
  --oracle-layout case-bundle \
  --suite acid-dmg-curated
```

If `--oracle-artifact-root` is omitted, the default repo-local root is `/.oracles/<oracle>/<layout>/`.

For Mealybug Phase `9` closure, use `--suite mealybug-tearoom-dmg-sameboy-differential` rather than the full local `mealybug-tearoom-dmg-curated` gate, because the full gate intentionally includes rows where SameBoy is not a passing GBEmulatorShootout oracle.

#### Layouts

- **`case-bundle`** (default) — oracle root contains one subdirectory per case id using the same artifact filenames that `gb-test-runner` emits locally or a legacy `framebuffer.pgm` for LibSameBoy framebuffer captures (`serial.txt`, `memory_text_output.txt`, `blargg_console.txt`, `framebuffer.png`, `framebuffer.pgm`, `trace.txt`).
- **`sameboy-tester`** — framebuffer-only compatibility layout; expects SameBoy Tester artifacts mirrored by ROM-relative path (e.g. `acid/dmg-acid2.bmp`).

### Case-bundle oracle example

The built-in cartridge mapper oracle lane uses the `case-bundle` layout:

```bash
cargo run -p gb-test-runner --bin run_sameboy_case_bundle -- \
  --suite phase-6-cartridge-oracle \
  --sameboy-root /path/to/SameBoy \
  --build-if-missing

cargo run -p gb-test-runner --bin run_differential -- \
  --oracle sameboy \
  --suite phase-6-cartridge-oracle
```

That suite compares retained synthetic `MBC1`, `MBC2`, `MBC3`, and `MBC5` Phase `6` `serial_hex` artifacts, and its `MBC3` case includes explicit pre-run RTC advancement in the typed runner metadata.

## Other local-only assets

Repo-managed local-only support assets live under gitignored roots:

- `/.oracles/<oracle>/<layout>/` — imported differential oracle artifacts.

## Environment variables

- `GB_CYCLE_BOOT_ROM_ROOT` — boot ROM search path for private firmware assets; there is no repo-local default boot ROM directory.
- `GB_CYCLE_TEST_ROM_ROOT` — override test ROM root; if unset, `gb-test-runner` falls back to the default curated store automatically.
- `GB_CYCLE_TEST_ROM_STARTUP` — ROM-suite startup override for local ignored external harness runs and the `run_rom_suite` CLI; omit it to preserve each suite manifest's declared startup mode, use `skip-boot` to force synthetic direct boot even for manifests that default to `RealBoot`, and use `real-boot` only with `GB_CYCLE_BOOT_ROM_ROOT` for clean boot-ROM execution that clears synthetic startup-memory and startup-timer profiles.
